pub(crate) mod continue_names;
#[path = "title_overlay_runtime/packed_glyphs.rs"]
mod packed_glyphs;

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, decode, encode};

use crate::contextual_texture_upload::{
    ContextualMenuGlyph, ContextualMenuGlyphUploadEntry, ContextualMenuGlyphUploadReport,
    TEXTURE_UPLOAD_DESCRIPTOR_BYTE_COUNT, VRAM_UPLOAD_ROUTINE_ADDRESS,
    encode_texture_upload_descriptor, menu_atlas_vram_rect,
};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::write_scope::changed_ranges_are_within;

const OVERLAY_PATH: &str = "DAT1/MGTIT.BIZ";
const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const NATIVE_ENTRYPOINT: u32 = 0x800a_3b60;
const HOOK_OFFSET: usize = 0x1c9c;
const HOOK_RUNTIME_ADDRESS: u32 = OVERLAY_RUNTIME_BASE + HOOK_OFFSET as u32;
const ORIGINAL_CALL_ADDRESS: u32 = 0x800a_4c20;
const PROGRAM_OFFSET: usize = 0xa400;
const PROGRAM_BYTE_CAPACITY: usize = 0x100;
const DESCRIPTOR_OFFSET: usize = PROGRAM_OFFSET + PROGRAM_BYTE_CAPACITY;
const SOURCE_PADDING_END: usize = 0xbce8;

pub(crate) struct TitleGlyphUploadInstall {
    pub(crate) report: ContextualMenuGlyphUploadReport,
    pub(crate) write_claims: Vec<DecodedDataClaim>,
    pub(crate) storage_end: usize,
}

pub(crate) fn install_title_glyph_upload(
    source_overlay: &[u8],
    output_overlay: &mut [u8],
    glyphs: &[ContextualMenuGlyph],
) -> Result<TitleGlyphUploadInstall> {
    ensure!(
        source_overlay.len() == SOURCE_PADDING_END && output_overlay.len() == source_overlay.len(),
        "MGTIT contextual glyph upload source extent changed"
    );
    ensure!(
        read_u32(source_overlay, 0)? == NATIVE_ENTRYPOINT,
        "MGTIT native entry pointer changed"
    );
    ensure!(
        decode_instruction(source_overlay, HOOK_OFFSET)?
            == Instruction::Jal {
                target: ORIGINAL_CALL_ADDRESS,
            }
            && decode_instruction(source_overlay, HOOK_OFFSET + 4)? == Instruction::nop(),
        "MGTIT initialization hook source changed"
    );
    ensure!(
        source_overlay[PROGRAM_OFFSET..SOURCE_PADDING_END]
            .iter()
            .all(|byte| *byte == 0),
        "MGTIT contextual glyph storage is no longer source-zero padding"
    );
    ensure!(
        output_overlay[PROGRAM_OFFSET..SOURCE_PADDING_END]
            == source_overlay[PROGRAM_OFFSET..SOURCE_PADDING_END],
        "MGTIT contextual glyph storage overlaps another writer"
    );
    let codes = glyphs
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        !glyphs.is_empty()
            && codes.len() == glyphs.len()
            && glyphs.iter().all(|glyph| {
                glyph.cell.width == 20
                    && glyph.cell.height == 20
                    && glyph.payload.len() == 20 * 20 / 2
            }),
        "MGTIT contextual glyph population is malformed"
    );

    let descriptor_byte_count = glyphs.len() * TEXTURE_UPLOAD_DESCRIPTOR_BYTE_COUNT;
    let payload_start = (DESCRIPTOR_OFFSET + descriptor_byte_count).next_multiple_of(16);
    let packed_payloads = glyphs
        .iter()
        .map(|glyph| packed_glyphs::pack(&glyph.payload))
        .collect::<Result<Vec<_>>>()?;
    let payload_byte_count = packed_payloads.iter().map(Vec::len).sum::<usize>();
    let payload_end = payload_start
        .checked_add(payload_byte_count)
        .context("MGTIT contextual glyph payload range overflow")?;
    ensure!(
        payload_end <= SOURCE_PADDING_END,
        "MGTIT contextual glyph payload exceeds source-zero padding"
    );

    let program_runtime_address = runtime_address(PROGRAM_OFFSET)?;
    let descriptor_runtime_address = runtime_address(DESCRIPTOR_OFFSET)?;
    let program = packed_glyphs::build_uploader(
        program_runtime_address,
        ORIGINAL_CALL_ADDRESS,
        descriptor_runtime_address,
        glyphs.len(),
    )?;
    ensure!(
        program.bytes.len() <= PROGRAM_BYTE_CAPACITY,
        "MGTIT contextual glyph program exceeds its fixed storage"
    );

    let before = output_overlay.to_vec();
    let hook = Instruction::Jal {
        target: program_runtime_address,
    };
    output_overlay[HOOK_OFFSET..HOOK_OFFSET + 4]
        .copy_from_slice(&encode(&hook, HOOK_RUNTIME_ADDRESS)?.to_le_bytes());
    output_overlay[PROGRAM_OFFSET..PROGRAM_OFFSET + program.bytes.len()]
        .copy_from_slice(&program.bytes);

    let mut descriptors = Vec::with_capacity(descriptor_byte_count);
    let mut entries = Vec::with_capacity(glyphs.len());
    let mut payload_offset = payload_start;
    for (glyph, packed_payload) in glyphs.iter().zip(&packed_payloads) {
        let rect = menu_atlas_vram_rect(glyph.cell)?;
        let payload_runtime_address = runtime_address(payload_offset)?;
        encode_texture_upload_descriptor(&mut descriptors, rect, payload_runtime_address);
        let payload_range = payload_offset..payload_offset + packed_payload.len();
        output_overlay[payload_range.clone()].copy_from_slice(packed_payload);
        entries.push(ContextualMenuGlyphUploadEntry {
            role: glyph.role.clone(),
            character: glyph.character,
            code: format!("0x{:04x}", glyph.code),
            cell: glyph.cell,
            vram_rect: rect,
            payload_offset: format!("0x{payload_offset:04x}"),
            payload_runtime_address: format!("0x{payload_runtime_address:08x}"),
            payload_sha256: sha256_bytes(packed_payload),
            gpu_payload_sha256: Some(sha256_bytes(&glyph.payload)),
        });
        payload_offset = payload_range.end;
    }
    ensure!(
        descriptors.len() == descriptor_byte_count && payload_offset == payload_end,
        "MGTIT contextual glyph descriptor or payload population changed"
    );
    output_overlay[DESCRIPTOR_OFFSET..DESCRIPTOR_OFFSET + descriptor_byte_count]
        .copy_from_slice(&descriptors);

    let expected_write_ranges = vec![
        [HOOK_OFFSET, HOOK_OFFSET + 4],
        [PROGRAM_OFFSET, PROGRAM_OFFSET + program.bytes.len()],
        [DESCRIPTOR_OFFSET, DESCRIPTOR_OFFSET + descriptor_byte_count],
        [payload_start, payload_end],
    ];
    ensure!(
        changed_ranges_are_within(
            &difference_ranges(&before, output_overlay),
            &expected_write_ranges,
        ),
        "MGTIT contextual glyph install escaped its Expected Writes"
    );
    ensure!(
        decode_instruction(output_overlay, HOOK_OFFSET)? == hook
            && decode_instruction(output_overlay, HOOK_OFFSET + 4)? == Instruction::nop()
            && output_overlay[..4] == source_overlay[..4],
        "MGTIT contextual glyph hook failed typed readback or changed the catalog-bound entry pointer"
    );

    Ok(TitleGlyphUploadInstall {
        storage_end: payload_end,
        report: ContextualMenuGlyphUploadReport {
            context: "mgtit".to_string(),
            payload_encoding: Some("palette-indexed-2bpp-zero-trimmed".to_string()),
            entry_count: glyphs.len(),
            global_menu_write_count: 0,
            source_menu_graphics_preserved: true,
            hook_path: OVERLAY_PATH.to_string(),
            hook_offset: format!("0x{HOOK_OFFSET:04x}"),
            hook_runtime_address: format!("0x{HOOK_RUNTIME_ADDRESS:08x}"),
            native_call_target: format!("0x{ORIGINAL_CALL_ADDRESS:08x}"),
            storage_path: OVERLAY_PATH.to_string(),
            program_offset: format!("0x{PROGRAM_OFFSET:04x}"),
            program_runtime_address: format!("0x{program_runtime_address:08x}"),
            program_byte_count: program.bytes.len(),
            program_byte_capacity: PROGRAM_BYTE_CAPACITY,
            program_sha256: sha256_bytes(&program.bytes),
            typed_instruction_count: program.typed_instruction_count,
            descriptor_offset: format!("0x{DESCRIPTOR_OFFSET:04x}"),
            descriptor_runtime_address: format!("0x{descriptor_runtime_address:08x}"),
            descriptor_byte_count,
            payload_byte_ranges: vec![[payload_start, payload_end]],
            payload_byte_count,
            source_storage_padding_verified: true,
            typed_program_verified: true,
            runtime_execution_verified: false,
            upload_routine_address: format!("0x{VRAM_UPLOAD_ROUTINE_ADDRESS:08x}"),
            entries,
        },
        write_claims: DecodedDataClaim::from_ranges(
            "mgtit:contextual-glyph-upload",
            "install MGTIT entry-scoped glyph upload wrapper, descriptors, and payloads",
            expected_write_ranges,
        ),
    })
}

fn runtime_address(offset: usize) -> Result<u32> {
    OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("MGTIT contextual glyph runtime address overflow")
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("MGTIT entry pointer is truncated")?
            .try_into()?,
    ))
}

fn decode_instruction(bytes: &[u8], offset: usize) -> Result<Instruction> {
    decode(
        read_u32(bytes, offset)?,
        OVERLAY_RUNTIME_BASE
            .checked_add(u32::try_from(offset)?)
            .context("MGTIT instruction address overflow")?,
    )
    .context("failed to decode MGTIT instruction")
}

#[cfg(test)]
mod tests {
    use psx_r3000a::encode;

    use super::*;

    #[test]
    fn title_upload_keeps_the_catalog_entry_and_uses_only_mgtit_padding() {
        let source = source_overlay();
        let mut output = source.clone();

        let install = install_title_glyph_upload(&source, &mut output, &glyphs(19)).unwrap();

        assert_eq!(install.report.entry_count, 19);
        assert_eq!(install.report.global_menu_write_count, 0);
        assert_eq!(
            install.report.payload_byte_count,
            19 * packed_glyphs::MAX_PACKED_BYTES
        );
        let range = install.report.payload_byte_ranges[0];
        assert!(range[0] >= DESCRIPTOR_OFFSET + install.report.descriptor_byte_count);
        assert_eq!(range[0] % 16, 0);
        assert_eq!(range[1] - range[0], install.report.payload_byte_count);
        assert!(range[1] <= SOURCE_PADDING_END);
        assert_eq!(output[..4], source[..4]);
        assert_eq!(
            decode_instruction(&output, HOOK_OFFSET).unwrap(),
            Instruction::Jal {
                target: OVERLAY_RUNTIME_BASE + PROGRAM_OFFSET as u32,
            }
        );
    }

    #[test]
    fn title_upload_packs_larger_repertoires_but_rejects_padding_overflow() {
        let source = source_overlay();
        let mut output = source.clone();
        let install = install_title_glyph_upload(&source, &mut output, &glyphs(29)).unwrap();
        for entry in &install.report.entries {
            let offset = usize::from_str_radix(&entry.payload_offset[2..], 16).unwrap();
            let index = u16::from_str_radix(&entry.code[2..], 16).unwrap() - 0x100;
            assert_eq!(
                &output[offset..offset + packed_glyphs::MAX_PACKED_BYTES],
                packed_glyphs::pack(&[index as u8 + 1; 200]).unwrap()
            );
        }
        let mut output = source.clone();
        let error = install_title_glyph_upload(&source, &mut output, &glyphs(60))
            .err()
            .expect("out-of-padding payload must fail");
        assert!(error.to_string().contains("exceeds source-zero padding"));
        assert_eq!(output, source);
    }

    #[test]
    fn title_upload_rejects_padding_that_has_an_existing_owner() {
        let mut source = source_overlay();
        source[PROGRAM_OFFSET] = 1;
        let mut output = source.clone();

        let error = install_title_glyph_upload(&source, &mut output, &glyphs(1))
            .err()
            .expect("changed MGTIT padding must be rejected");

        assert!(error.to_string().contains("no longer source-zero padding"));
    }

    #[test]
    fn title_upload_rejects_a_changed_native_setup_call() {
        let mut source = source_overlay();
        source[HOOK_OFFSET..HOOK_OFFSET + 4].fill(0);
        let mut output = source.clone();

        let error = install_title_glyph_upload(&source, &mut output, &glyphs(1))
            .err()
            .expect("changed MGTIT native call must be rejected");

        assert!(
            error
                .to_string()
                .contains("initialization hook source changed")
        );
    }

    fn source_overlay() -> Vec<u8> {
        let mut source = vec![0; SOURCE_PADDING_END];
        source[..4].copy_from_slice(&NATIVE_ENTRYPOINT.to_le_bytes());
        write_instruction(
            &mut source,
            HOOK_OFFSET,
            Instruction::Jal {
                target: ORIGINAL_CALL_ADDRESS,
            },
        );
        write_instruction(&mut source, HOOK_OFFSET + 4, Instruction::nop());
        source
    }

    fn glyphs(count: usize) -> Vec<ContextualMenuGlyph> {
        (0..count)
            .map(|index| ContextualMenuGlyph {
                role: "title".to_string(),
                character: char::from_u32(0xac00 + index as u32).unwrap(),
                code: 0x0100 + index as u16,
                cell: crate::tim::Cell {
                    x: (index % 16) * 20,
                    y: (1 + index / 16) * 20,
                    width: 20,
                    height: 20,
                },
                payload: vec![u8::try_from(index + 1).unwrap(); 200],
            })
            .collect()
    }

    fn write_instruction(output: &mut [u8], offset: usize, instruction: Instruction) {
        let address = OVERLAY_RUNTIME_BASE + offset as u32;
        output[offset..offset + 4]
            .copy_from_slice(&encode(&instruction, address).unwrap().to_le_bytes());
    }
}
