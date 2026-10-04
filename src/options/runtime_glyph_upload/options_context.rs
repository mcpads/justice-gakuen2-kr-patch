use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::compression::{compress, decompress};
use crate::pipeline::{difference_ranges, sha256_bytes};

use super::assembly::{
    MENU_CATALOG_INDEX, MENU_LOAD_CALL_RUNTIME_ADDRESS, MENU_LOAD_DESTINATION, MENU_LOADER_ADDRESS,
    MENU_TIM_PARSER_ADDRESS, MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES, MENU_TIM_RUNTIME_ADDRESSES,
    OPTIONS_HOOK_OFFSET, OPTIONS_HOOK_RUNTIME_ADDRESS, OPTIONS_INITIALIZER_ADDRESS,
    OPTIONS_PROGRAM_BYTE_CAPACITY, OPTIONS_PROGRAM_OFFSET, OPTIONS_PROGRAM_RUNTIME_ADDRESS,
    PayloadDecompression, RUNTIME_DECOMPRESSOR_ADDRESS, UPLOAD_ROUTINE_ADDRESS,
    build_options_upload_program, patch_options_initializer_call,
    verify_native_menu_restore_sequence,
};
use super::model::{DESCRIPTOR_BYTE_COUNT, GlyphUploadContextReport, OptionsAtlasRestoreReport};
use super::{
    OPTINFO_RUNTIME_BASE, PreparedGlyph, PreparedTextureUpload, encode_descriptor,
    ranges_are_allowed, upload_entry_report,
};
use crate::options::description_source::{ITEM_SLOT_SIZE, inspect_optinfo_slots};

pub(super) const SOURCE_TIM_BYTE_COUNT: usize = 0x8040;
// The description decoder addresses 12 columns by 10 rows of 20 x 24 cells.
// The final 16 pixel rows are unreachable by every admitted description stream.
// Their source-zero bytes are contiguous with the TIM alignment padding.
pub(super) const STORAGE_OFFSET_IN_SLOT: usize = SOURCE_TIM_BYTE_COUNT - 16 * 128;
pub(super) const PADDING_BYTE_COUNT: usize = ITEM_SLOT_SIZE - STORAGE_OFFSET_IN_SLOT;
pub(super) const COMPRESSED_PAYLOAD_SLOT_INDICES: [usize; 3] = [0, 1, 4];
pub(super) const DESCRIPTOR_SLOT_INDEX: usize = 3;
pub(super) const DESCRIPTOR_OFFSET: usize =
    DESCRIPTOR_SLOT_INDEX * ITEM_SLOT_SIZE + STORAGE_OFFSET_IN_SLOT;
pub(super) const SCRATCH_OFFSET: usize = 5 * ITEM_SLOT_SIZE;

pub(super) struct OptionsContextInstall {
    pub(super) report: GlyphUploadContextReport,
    pub(super) exit_restore_report: OptionsAtlasRestoreReport,
    pub(super) expected_overlay_write_ranges: Vec<[usize; 2]>,
    pub(super) expected_storage_write_ranges: Vec<[usize; 2]>,
}

struct PayloadChunk {
    raw_start: usize,
    encoded: Vec<u8>,
}

pub(super) fn install(
    glyphs: &[PreparedGlyph],
    background_uploads: &[PreparedTextureUpload],
    source_overlay: &[u8],
    output_overlay: &mut [u8],
    source_optinfo: &[u8],
    output_optinfo: &mut [u8],
    source_main_executable: &[u8],
) -> Result<OptionsContextInstall> {
    validate_source_padding(source_optinfo)?;
    ensure!(
        output_optinfo.len() == source_optinfo.len(),
        "OPTIONS contextual texture storage changed OPTINFO length"
    );
    let glyph_codes = glyphs
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        glyph_codes.len() == glyphs.len()
            && glyphs.iter().all(|glyph| !glyph.source_preservation)
            && background_uploads
                .iter()
                .all(|upload| upload.code.is_none() && !upload.source_preservation),
        "OPTIONS contextual texture population is malformed"
    );
    let mut uploads = glyphs
        .iter()
        .map(PreparedTextureUpload::from_glyph)
        .collect::<Result<Vec<_>>>()?;
    uploads.extend_from_slice(background_uploads);
    ensure!(
        !uploads.is_empty(),
        "OPTIONS contextual texture population is empty"
    );
    for upload in &uploads {
        let expected = usize::from(upload.vram_rect[2])
            .checked_mul(2)
            .and_then(|row_bytes| row_bytes.checked_mul(usize::from(upload.vram_rect[3])))
            .context("OPTIONS contextual texture payload size overflow")?;
        ensure!(
            upload.payload.len() == expected,
            "OPTIONS contextual texture payload geometry changed for {}",
            upload.role
        );
    }

    let chunks = compress_payload_chunks(&uploads)?;
    ensure!(
        chunks.len() <= COMPRESSED_PAYLOAD_SLOT_INDICES.len(),
        "OPTIONS compressed texture payload needs {} slots but only {} are reserved",
        chunks.len(),
        COMPRESSED_PAYLOAD_SLOT_INDICES.len()
    );
    let before_storage = output_optinfo.to_vec();
    let before_overlay = output_overlay.to_vec();
    let mut payload_ranges = Vec::with_capacity(chunks.len());
    let mut streams = Vec::with_capacity(chunks.len());
    for (chunk, slot_index) in chunks
        .iter()
        .zip(COMPRESSED_PAYLOAD_SLOT_INDICES.iter().copied())
    {
        let start = padding_offset(slot_index);
        let end = start + chunk.encoded.len();
        ensure!(
            end <= (slot_index + 1) * ITEM_SLOT_SIZE,
            "OPTIONS compressed texture payload exceeds OPTINFO slot {slot_index}"
        );
        ensure_storage_available(source_optinfo, output_optinfo, start, end)?;
        output_optinfo[start..end].copy_from_slice(&chunk.encoded);
        let input_start = runtime_address(start, "OPTIONS compressed payload")?;
        let input_end = runtime_address(end, "OPTIONS compressed payload end")?;
        ensure!(
            !runtime_decompressor_uses_ring_buffer_window(input_end),
            "OPTIONS compressed texture payload end {input_end:#010x} selects the runtime decompressor ring-buffer window"
        );
        let output_start = runtime_address(
            SCRATCH_OFFSET + chunk.raw_start,
            "OPTIONS decompressed payload",
        )?;
        payload_ranges.push([start, end]);
        streams.push(PayloadDecompression {
            input_start,
            input_end,
            output_start,
        });
    }

    let descriptor_byte_count = uploads.len() * DESCRIPTOR_BYTE_COUNT;
    let descriptor_range = [DESCRIPTOR_OFFSET, DESCRIPTOR_OFFSET + descriptor_byte_count];
    ensure!(
        descriptor_range[1] <= (DESCRIPTOR_SLOT_INDEX + 1) * ITEM_SLOT_SIZE,
        "OPTIONS contextual texture descriptors exceed their OPTINFO padding slot"
    );
    ensure_storage_available(
        source_optinfo,
        output_optinfo,
        descriptor_range[0],
        descriptor_range[1],
    )?;
    let scratch_end = uploads.iter().try_fold(SCRATCH_OFFSET, |offset, upload| {
        offset
            .checked_add(upload.payload.len())
            .context("OPTIONS contextual texture scratch range overflow")
    })?;
    ensure!(
        scratch_end <= super::RECORDS_PAYLOAD_STORAGE_START,
        "OPTIONS contextual texture scratch exceeds its arena or reaches Records storage"
    );
    let mut descriptors = Vec::with_capacity(descriptor_byte_count);
    let mut entries = Vec::with_capacity(uploads.len());
    let mut payload_offset = SCRATCH_OFFSET;
    for upload in &uploads {
        let payload_runtime_address =
            runtime_address(payload_offset, "OPTIONS contextual texture payload")?;
        encode_descriptor(&mut descriptors, upload.vram_rect, payload_runtime_address);
        entries.push(upload_entry_report(
            upload,
            payload_offset,
            payload_runtime_address,
        ));
        payload_offset += upload.payload.len();
    }
    ensure!(
        payload_offset == scratch_end && descriptors.len() == descriptor_byte_count,
        "OPTIONS contextual texture descriptor population changed"
    );
    output_optinfo[descriptor_range[0]..descriptor_range[1]].copy_from_slice(&descriptors);

    let descriptor_runtime_address =
        runtime_address(DESCRIPTOR_OFFSET, "OPTIONS contextual texture descriptors")?;
    let program =
        build_options_upload_program(descriptor_runtime_address, uploads.len(), &streams)?;
    write_overlay_program(
        source_overlay,
        output_overlay,
        OPTIONS_PROGRAM_OFFSET,
        OPTIONS_PROGRAM_BYTE_CAPACITY,
        &program.bytes,
        "OPTIONS contextual texture uploader",
    )?;
    patch_options_initializer_call(source_overlay, output_overlay)?;
    verify_native_menu_restore_sequence(source_main_executable)?;

    let mut expected_storage_write_ranges = payload_ranges.clone();
    expected_storage_write_ranges.push(descriptor_range);
    ensure!(
        ranges_are_allowed(
            &difference_ranges(&before_storage, output_optinfo),
            &expected_storage_write_ranges,
        ),
        "OPTIONS contextual texture upload escaped its OPTINFO Expected Writes"
    );
    let expected_overlay_write_ranges = vec![
        [OPTIONS_HOOK_OFFSET, OPTIONS_HOOK_OFFSET + 4],
        [
            OPTIONS_PROGRAM_OFFSET,
            OPTIONS_PROGRAM_OFFSET + program.bytes.len(),
        ],
    ];
    ensure!(
        ranges_are_allowed(
            &difference_ranges(&before_overlay, output_overlay),
            &expected_overlay_write_ranges,
        ),
        "OPTIONS contextual texture programs escaped their NEWOPT Expected Writes"
    );

    let raw_payload_byte_count = uploads.iter().map(|upload| upload.payload.len()).sum();
    let stored_payload_byte_count = chunks.iter().map(|chunk| chunk.encoded.len()).sum();
    Ok(OptionsContextInstall {
        report: GlyphUploadContextReport {
            context: "options".to_string(),
            entry_count: entries.len(),
            storage_path: "DAT2/OPTINFO.TIZ".to_string(),
            source_storage_padding_verified: true,
            hook_path: "DAT1/NEWOPT.BIN".to_string(),
            hook_offset: format!("0x{OPTIONS_HOOK_OFFSET:04x}"),
            hook_runtime_address: format!("0x{OPTIONS_HOOK_RUNTIME_ADDRESS:08x}"),
            original_call_address: format!("0x{OPTIONS_INITIALIZER_ADDRESS:08x}"),
            upload_routine_address: format!("0x{UPLOAD_ROUTINE_ADDRESS:08x}"),
            program_storage_path: "DAT1/NEWOPT.BIN".to_string(),
            program_offset: format!("0x{OPTIONS_PROGRAM_OFFSET:04x}"),
            program_runtime_address: format!("0x{OPTIONS_PROGRAM_RUNTIME_ADDRESS:08x}"),
            program_byte_count: program.bytes.len(),
            program_byte_capacity: OPTIONS_PROGRAM_BYTE_CAPACITY,
            program_sha256: sha256_bytes(&program.bytes),
            typed_instruction_count: program.typed_instruction_count,
            descriptor_offset: format!("0x{DESCRIPTOR_OFFSET:05x}"),
            descriptor_runtime_address: format!("0x{descriptor_runtime_address:08x}"),
            descriptor_byte_count,
            payload_byte_count: raw_payload_byte_count,
            stored_payload_byte_count,
            payload_stream_count: chunks.len(),
            payload_ranges,
            scratch_runtime_range: Some([
                format!(
                    "0x{:08x}",
                    runtime_address(SCRATCH_OFFSET, "OPTIONS scratch start")?
                ),
                format!(
                    "0x{:08x}",
                    runtime_address(scratch_end, "OPTIONS scratch end")?
                ),
            ]),
            typed_program_verified: true,
            runtime_execution_verified: false,
            entries,
        },
        exit_restore_report: OptionsAtlasRestoreReport {
            context: "options_exit_restore".to_string(),
            mechanism: "native_mode_select_menu_reload".to_string(),
            consumer_path: "SLPS_021.20".to_string(),
            menu_catalog_index: MENU_CATALOG_INDEX,
            menu_destination_address: format!("0x{MENU_LOAD_DESTINATION:08x}"),
            menu_load_call_runtime_address: format!("0x{MENU_LOAD_CALL_RUNTIME_ADDRESS:08x}"),
            menu_tim_runtime_addresses: MENU_TIM_RUNTIME_ADDRESSES
                .iter()
                .map(|address| format!("0x{address:08x}"))
                .collect(),
            menu_tim_parser_call_runtime_addresses: MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES
                .iter()
                .map(|address| format!("0x{address:08x}"))
                .collect(),
            menu_loader_address: format!("0x{MENU_LOADER_ADDRESS:08x}"),
            menu_tim_parser_address: format!("0x{MENU_TIM_PARSER_ADDRESS:08x}"),
            source_sequence_verified: true,
            manual_overlay_write_count: 0,
            runtime_execution_verified: false,
        },
        expected_overlay_write_ranges,
        expected_storage_write_ranges,
    })
}

fn compress_payload_chunks(uploads: &[PreparedTextureUpload]) -> Result<Vec<PayloadChunk>> {
    let capacity = compressed_payload_capacity()?;
    let mut chunks = Vec::new();
    let mut raw = Vec::new();
    let mut raw_start = 0usize;
    for upload in uploads {
        let mut candidate = raw.clone();
        candidate.extend_from_slice(&upload.payload);
        let candidate_encoded = compress(&candidate, 0)?;
        if !raw.is_empty() && candidate_encoded.len() > capacity {
            let encoded = compress(&raw, 0)?;
            ensure!(
                encoded.len() <= capacity && decompress(&encoded, false)? == raw,
                "OPTIONS contextual texture compression roundtrip failed"
            );
            let raw_len = raw.len();
            chunks.push(PayloadChunk { raw_start, encoded });
            raw_start += raw_len;
            raw = upload.payload.clone();
            ensure!(
                compress(&raw, 0)?.len() <= capacity,
                "one OPTIONS contextual texture cannot fit a compressed padding slot"
            );
        } else {
            raw = candidate;
        }
    }
    ensure!(
        !raw.is_empty(),
        "OPTIONS contextual texture compression produced no payload"
    );
    let encoded = compress(&raw, 0)?;
    ensure!(
        encoded.len() <= capacity && decompress(&encoded, false)? == raw,
        "OPTIONS contextual texture compression roundtrip failed"
    );
    chunks.push(PayloadChunk { raw_start, encoded });
    Ok(chunks)
}

// The decompressor chooses scratch behavior from the input end address.
// A stream filling slot 1 exactly lands in its ring-buffer window.
fn compressed_payload_capacity() -> Result<usize> {
    (1..=PADDING_BYTE_COUNT)
        .take_while(|length| {
            COMPRESSED_PAYLOAD_SLOT_INDICES.iter().all(|slot| {
                let end = OPTINFO_RUNTIME_BASE + (padding_offset(*slot) + length) as u32;
                !runtime_decompressor_uses_ring_buffer_window(end)
            })
        })
        .last()
        .context("OPTIONS compressed slots have no safe decompressor extent")
}

fn padding_offset(slot_index: usize) -> usize {
    slot_index * ITEM_SLOT_SIZE + STORAGE_OFFSET_IN_SLOT
}

fn ensure_storage_available(source: &[u8], output: &[u8], start: usize, end: usize) -> Result<()> {
    ensure!(
        source.get(start..end) == output.get(start..end),
        "OPTIONS contextual texture storage overlaps another OPTINFO writer at {start:#x}..{end:#x}"
    );
    ensure!(
        source[start..end].iter().all(|byte| *byte == 0),
        "OPTIONS contextual texture source storage is not zero at {start:#x}..{end:#x}"
    );
    Ok(())
}

fn write_overlay_program(
    source: &[u8],
    output: &mut [u8],
    offset: usize,
    capacity: usize,
    program: &[u8],
    context: &str,
) -> Result<()> {
    ensure!(
        program.len() <= capacity
            && source
                .get(offset..offset + capacity)
                .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
            && output.get(offset..offset + capacity) == source.get(offset..offset + capacity),
        "{context} storage is no longer untouched NEWOPT padding"
    );
    output[offset..offset + program.len()].copy_from_slice(program);
    Ok(())
}

fn runtime_address(offset: usize, context: &str) -> Result<u32> {
    OPTINFO_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .with_context(|| format!("{context} address overflow"))
}

pub(super) fn runtime_decompressor_uses_ring_buffer_window(input_end: u32) -> bool {
    input_end.wrapping_add(0x7fe0_7000) & 0x7800 == 0x4000
}

fn validate_source_padding(source: &[u8]) -> Result<()> {
    use crate::options::description_stream::{CELL_HEIGHT, CELL_ROWS};
    let slots = inspect_optinfo_slots(source)?;
    ensure!(
        CELL_ROWS * CELL_HEIGHT == 240
            && STORAGE_OFFSET_IN_SLOT == 0x40 + CELL_ROWS * CELL_HEIGHT * 128,
        "Options description cells overlap contextual upload storage"
    );
    ensure!(
        slots
            .iter()
            .all(|slot| slot.total_size == SOURCE_TIM_BYTE_COUNT),
        "OPTINFO source TIM size changed"
    );
    for item_index in 0..5 {
        let start = padding_offset(item_index);
        let end = (item_index + 1) * ITEM_SLOT_SIZE;
        ensure!(
            source[start..end].iter().all(|byte| *byte == 0),
            "OPTINFO item {item_index} padding is no longer zero"
        );
    }
    ensure!(
        RUNTIME_DECOMPRESSOR_ADDRESS == 0x8001_5eac,
        "OPTIONS runtime decompressor binding changed"
    );
    Ok(())
}

#[cfg(test)]
mod payload_boundary_tests {
    use super::*;

    #[test]
    fn exact_full_slot_is_split_before_ring_buffer_selection() {
        let capacity = compressed_payload_capacity().unwrap();
        assert_eq!(
            OPTINFO_RUNTIME_BASE + (padding_offset(1) + PADDING_BYTE_COUNT) as u32,
            0x800e_5000
        );
        assert!(runtime_decompressor_uses_ring_buffer_window(0x800e_5000));
        assert!(capacity < PADDING_BYTE_COUNT);
        for slot in COMPRESSED_PAYLOAD_SLOT_INDICES {
            for length in 1..=capacity {
                assert!(!runtime_decompressor_uses_ring_buffer_window(
                    OPTINFO_RUNTIME_BASE + (padding_offset(slot) + length) as u32
                ));
            }
        }
    }
}
