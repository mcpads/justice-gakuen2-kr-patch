use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::difference_ranges;
use crate::source_disc::{MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE};
use crate::write_scope::changed_ranges_are_within;

use super::assets::parse_hex_usize;
use super::model::{OptionsAuthoredUnit, RecordsRuntimeBuildReport, RecordsRuntimeUnitBuild};

const MAIN_EXECUTABLE_OFFSET_DELTA: usize = 0x0801f0;

#[derive(Clone, Copy)]
struct MirroredUnit {
    id: &'static str,
    overlay_offset: usize,
    source_code_count: usize,
}

const MIRRORED_UNITS: [MirroredUnit; 27] = [
    mirrored("automatic_load_skipped", 0x0aa0, 13),
    mirrored("initialize_complete", 0x0b80, 7),
    mirrored("initialize_failed", 0x0884, 11),
    mirrored("load_failed", 0x0abc, 11),
    mirrored("no_data", 0x0af0, 9),
    mirrored("save_block_required", 0x0a24, 16),
    mirrored("records_await_input", 0x0610, 13),
    mirrored("records_save_complete", 0x062c, 7),
    mirrored("records_load_complete", 0x063c, 7),
    mirrored("records_data_error", 0x064c, 8),
    mirrored("records_memory_card_not_inserted", 0x0660, 16),
    mirrored("records_memory_card_corrupt", 0x0684, 14),
    mirrored("records_data_file_missing", 0x06a4, 10),
    mirrored("records_memory_card_full", 0x06bc, 14),
    mirrored("records_memory_card_unformatted", 0x06dc, 18),
    mirrored("records_initialize_confirmation", 0x0704, 8),
    mirrored("records_no", 0x0718, 3),
    mirrored("records_yes", 0x0720, 2),
    mirrored("records_save_failed", 0x0728, 11),
    mirrored("records_memory_card_check", 0x0740, 12),
    mirrored("records_saving", 0x075c, 7),
    mirrored("records_load_confirmation", 0x09a8, 8),
    mirrored("records_save_confirmation", 0x09bc, 8),
    mirrored("records_horizontal_help", 0x09d0, 13),
    mirrored("records_keep_memory_card_inserted", 0x09ec, 17),
    mirrored("records_overwrite_confirmation", 0x0a48, 8),
    mirrored("records_vertical_help", 0x0a5c, 10),
];

const fn mirrored(
    id: &'static str,
    overlay_offset: usize,
    source_code_count: usize,
) -> MirroredUnit {
    MirroredUnit {
        id,
        overlay_offset,
        source_code_count,
    }
}

pub(super) fn contains_unit(id: &str) -> bool {
    MIRRORED_UNITS.iter().any(|unit| unit.id == id)
}

pub(super) fn main_executable_expected_write_ranges() -> Vec<[usize; 2]> {
    main_executable_write_claims()
        .into_iter()
        .map(|(_, range)| range)
        .collect()
}

pub(super) fn main_executable_write_claims() -> Vec<(&'static str, [usize; 2])> {
    MIRRORED_UNITS
        .iter()
        .map(|unit| {
            let start = MAIN_EXECUTABLE_OFFSET_DELTA + unit.overlay_offset;
            (
                unit.id,
                [start, start + source_record_size(unit.source_code_count)],
            )
        })
        .collect()
}

pub(super) fn rebuild_records_runtime(
    source_overlay: &[u8],
    source_main_executable: &[u8],
    units: &[OptionsAuthoredUnit],
    rebuilt_units: &[(&str, &[u16])],
) -> Result<(Vec<u8>, RecordsRuntimeBuildReport)> {
    ensure!(
        source_main_executable.starts_with(b"PS-X EXE"),
        "records runtime source is not a PS-X EXE"
    );
    let mirrored_ids = MIRRORED_UNITS
        .iter()
        .map(|unit| unit.id)
        .collect::<BTreeSet<_>>();
    ensure!(
        units
            .iter()
            .filter(|unit| mirrored_ids.contains(unit.id.as_str()))
            .map(|unit| unit.id.as_str())
            .collect::<BTreeSet<_>>()
            == mirrored_ids,
        "records runtime unit membership changed"
    );
    ensure!(
        rebuilt_units
            .iter()
            .map(|(id, _)| *id)
            .collect::<BTreeSet<_>>()
            == mirrored_ids,
        "records runtime rebuilt unit membership changed"
    );

    let mut output = source_main_executable.to_vec();
    let allowed_ranges = main_executable_expected_write_ranges();
    let mut reports = Vec::with_capacity(MIRRORED_UNITS.len());
    for mirrored_unit in MIRRORED_UNITS {
        let id = mirrored_unit.id;
        let unit = units
            .iter()
            .find(|unit| unit.id == id)
            .with_context(|| format!("records runtime is missing {id}"))?;
        let overlay_offset = parse_hex_usize(&unit.source_offset, "records runtime offset")?;
        ensure!(
            overlay_offset == mirrored_unit.overlay_offset,
            "records runtime {id} NEWOPT offset changed"
        );
        ensure!(
            unit.source_codes.len() == mirrored_unit.source_code_count,
            "records runtime {id} source record size changed"
        );
        let source_record_byte_count = source_record_size(unit.source_codes.len());
        let overlay_end = overlay_offset
            .checked_add(source_record_byte_count)
            .context("records runtime NEWOPT range overflow")?;
        let executable_offset = MAIN_EXECUTABLE_OFFSET_DELTA
            .checked_add(overlay_offset)
            .context("records runtime executable offset overflow")?;
        let executable_end = executable_offset
            .checked_add(source_record_byte_count)
            .context("records runtime executable range overflow")?;
        let source_record = source_overlay
            .get(overlay_offset..overlay_end)
            .with_context(|| format!("records runtime {id} leaves NEWOPT"))?;
        ensure!(
            source_main_executable.get(executable_offset..executable_end) == Some(source_record),
            "records runtime {id} executable mirror changed"
        );

        let rebuilt_codes = rebuilt_units
            .iter()
            .find_map(|(rebuilt_id, codes)| (*rebuilt_id == id).then_some(*codes))
            .with_context(|| format!("records runtime is missing rebuilt {id}"))?;
        ensure!(
            rebuilt_codes.len() <= unit.source_codes.len(),
            "records runtime {id} no longer fits its source mirror"
        );
        let executable_record = output
            .get_mut(executable_offset..executable_end)
            .with_context(|| format!("records runtime {id} leaves the main executable"))?;
        executable_record.fill(0);
        write_record_prefix(executable_record, rebuilt_codes)?;

        let mut executable_pointer_offsets = Vec::with_capacity(unit.pointer_offsets.len());
        for pointer_offset in &unit.pointer_offsets {
            let overlay_pointer_offset =
                parse_hex_usize(pointer_offset, "records runtime pointer offset")?;
            let executable_pointer_offset = MAIN_EXECUTABLE_OFFSET_DELTA
                .checked_add(overlay_pointer_offset)
                .context("records runtime executable pointer offset overflow")?;
            ensure!(
                read_u32(source_main_executable, executable_pointer_offset)?
                    == runtime_address(executable_offset)?,
                "records runtime {id} executable pointer binding changed"
            );
            executable_pointer_offsets.push(format!("0x{executable_pointer_offset:06x}"));
        }
        ensure!(
            !executable_pointer_offsets.is_empty(),
            "records runtime {id} has no executable pointer binding"
        );
        reports.push(RecordsRuntimeUnitBuild {
            id: id.to_string(),
            overlay_offset: format!("0x{overlay_offset:04x}"),
            main_executable_offset: format!("0x{executable_offset:06x}"),
            main_executable_pointer_offsets: executable_pointer_offsets,
            source_record_size: source_record_byte_count,
            output_record_size: source_record_size(rebuilt_codes.len()),
        });
    }

    let changed = difference_ranges(source_main_executable, &output);
    ensure!(
        !changed.is_empty(),
        "records runtime build changed no bytes"
    );
    ensure!(
        changed_ranges_are_within(&changed, &allowed_ranges),
        "records runtime build escaped its Expected Writes"
    );
    Ok((
        output,
        RecordsRuntimeBuildReport {
            unit_count: reports.len(),
            source_records_match: true,
            source_pointer_bindings_match: true,
            changes_confined_to_owned_records: true,
            units: reports,
        },
    ))
}

const fn source_record_size(code_count: usize) -> usize {
    2 + code_count * 2
}

fn write_record_prefix(output: &mut [u8], codes: &[u16]) -> Result<()> {
    let byte_count = source_record_size(codes.len());
    ensure!(
        byte_count <= output.len(),
        "records runtime encoded record exceeds its source mirror"
    );
    output[..2].copy_from_slice(&u16::try_from(codes.len())?.to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let offset = 2 + index * 2;
        output[offset..offset + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

fn runtime_address(file_offset: usize) -> Result<u32> {
    let text_offset = file_offset
        .checked_sub(PSX_EXE_HEADER_SIZE)
        .context("records runtime offset precedes the PS-X EXE header")?;
    MAIN_TEXT_RUNTIME_BASE
        .checked_add(u32::try_from(text_offset)?)
        .context("records runtime address overflow")
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .context("records runtime pointer leaves the main executable")?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
