use super::model::{OptionsAuthoredUnit, OptionsFontRole, OptionsReleaseStatus};
use super::records_runtime::rebuild_records_runtime;

const EXECUTABLE_OFFSET_DELTA: usize = 0x0801f0;

#[test]
fn records_runtime_encodes_every_bound_newopt_mirror_in_the_main_executable() {
    let units = units();
    let (source_overlay, mut source_main_executable) = source_buffers(&units);
    let rebuilt = rebuilt_units(&units);
    let rebuilt_slices = rebuilt
        .iter()
        .map(|(id, codes)| (id.as_str(), codes.as_slice()))
        .collect::<Vec<_>>();

    let (output, report) = rebuild_records_runtime(
        &source_overlay,
        &source_main_executable,
        &units,
        &rebuilt_slices,
    )
    .unwrap();

    assert_eq!(report.unit_count, units.len());
    for (unit, (_, rebuilt_codes)) in units.iter().zip(&rebuilt) {
        let offset =
            usize::from_str_radix(unit.source_offset.trim_start_matches("0x"), 16).unwrap();
        let source_size = 2 + unit.source_codes.len() * 2;
        let executable_offset = EXECUTABLE_OFFSET_DELTA + offset;
        assert_eq!(
            u16::from_le_bytes(
                output[executable_offset..executable_offset + 2]
                    .try_into()
                    .unwrap()
            ),
            u16::try_from(rebuilt_codes.len()).unwrap()
        );
        for (index, code) in rebuilt_codes.iter().enumerate() {
            let code_offset = executable_offset + 2 + index * 2;
            assert_eq!(
                u16::from_le_bytes(output[code_offset..code_offset + 2].try_into().unwrap()),
                *code
            );
        }
        let rebuilt_size = 2 + rebuilt_codes.len() * 2;
        assert!(
            output[executable_offset + rebuilt_size..executable_offset + source_size]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    source_main_executable[EXECUTABLE_OFFSET_DELTA + 0x0610] ^= 1;
    assert!(
        rebuild_records_runtime(
            &source_overlay,
            &source_main_executable,
            &units,
            &rebuilt_slices,
        )
        .is_err()
    );
}

#[test]
fn records_runtime_rejects_a_pointer_that_no_longer_targets_its_mirror() {
    let units = units();
    let (source_overlay, mut source_main_executable) = source_buffers(&units);
    let rebuilt = rebuilt_units(&units);
    let rebuilt_slices = rebuilt
        .iter()
        .map(|(id, codes)| (id.as_str(), codes.as_slice()))
        .collect::<Vec<_>>();
    let pointer_offset = EXECUTABLE_OFFSET_DELTA + 0x0c48;
    source_main_executable[pointer_offset..pointer_offset + 4].fill(0);

    let error = rebuild_records_runtime(
        &source_overlay,
        &source_main_executable,
        &units,
        &rebuilt_slices,
    )
    .unwrap_err();
    assert!(error.to_string().contains("pointer binding changed"));
}

fn units() -> Vec<OptionsAuthoredUnit> {
    [
        ("automatic_load_skipped", 0x0aa0, 0x0d70, 13),
        ("initialize_complete", 0x0b80, 0x0ddc, 7),
        ("initialize_failed", 0x0884, 0x0cf0, 11),
        ("load_failed", 0x0abc, 0x0d74, 11),
        ("no_data", 0x0af0, 0x0d84, 9),
        ("save_block_required", 0x0a24, 0x0d58, 16),
        ("records_await_input", 0x0610, 0x0c48, 13),
        ("records_save_complete", 0x062c, 0x0c4c, 7),
        ("records_load_complete", 0x063c, 0x0c50, 7),
        ("records_data_error", 0x064c, 0x0c54, 8),
        ("records_memory_card_not_inserted", 0x0660, 0x0c58, 16),
        ("records_memory_card_corrupt", 0x0684, 0x0c5c, 14),
        ("records_data_file_missing", 0x06a4, 0x0c60, 10),
        ("records_memory_card_full", 0x06bc, 0x0c64, 14),
        ("records_memory_card_unformatted", 0x06dc, 0x0c68, 18),
        ("records_initialize_confirmation", 0x0704, 0x0c6c, 8),
        ("records_no", 0x0718, 0x0c70, 3),
        ("records_yes", 0x0720, 0x0c74, 2),
        ("records_save_failed", 0x0728, 0x0c78, 11),
        ("records_memory_card_check", 0x0740, 0x0c7c, 12),
        ("records_saving", 0x075c, 0x0c80, 7),
        ("records_load_confirmation", 0x09a8, 0x0d40, 8),
        ("records_save_confirmation", 0x09bc, 0x0d44, 8),
        ("records_horizontal_help", 0x09d0, 0x0d48, 13),
        ("records_keep_memory_card_inserted", 0x09ec, 0x0d4c, 17),
        ("records_overwrite_confirmation", 0x0a48, 0x0d5c, 8),
        ("records_vertical_help", 0x0a5c, 0x0d60, 10),
    ]
    .into_iter()
    .map(
        |(id, offset, pointer_offset, code_count)| OptionsAuthoredUnit {
            id: id.to_string(),
            source_offset: format!("0x{offset:04x}"),
            pointer_offsets: vec![format!("0x{pointer_offset:04x}")],
            source_codes: (0..code_count)
                .map(|index| format!("0x{:04x}", 0x0100 + index))
                .collect(),
            source_text: id.to_string(),
            korean_text: id.to_string(),
            font_role: if id.ends_with("help") {
                OptionsFontRole::Help
            } else {
                OptionsFontRole::RecordsPrompt
            },
            release_status: OptionsReleaseStatus::NeedsHumanReview,
        },
    )
    .collect()
}

fn rebuilt_units(units: &[OptionsAuthoredUnit]) -> Vec<(String, Vec<u16>)> {
    units
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            let output_count = unit.source_codes.len().saturating_sub(index % 2);
            (
                unit.id.clone(),
                (0..output_count)
                    .map(|code_index| 0x0300 + u16::try_from(index + code_index).unwrap())
                    .collect(),
            )
        })
        .collect()
}

fn source_buffers(units: &[OptionsAuthoredUnit]) -> (Vec<u8>, Vec<u8>) {
    let mut overlay = vec![0; 0x0de0];
    let mut executable = vec![0; 0x081000];
    executable[..8].copy_from_slice(b"PS-X EXE");
    for unit in units {
        let offset =
            usize::from_str_radix(unit.source_offset.trim_start_matches("0x"), 16).unwrap();
        let pointer_offset =
            usize::from_str_radix(unit.pointer_offsets[0].trim_start_matches("0x"), 16).unwrap();
        let codes = unit
            .source_codes
            .iter()
            .map(|code| u16::from_str_radix(code.trim_start_matches("0x"), 16).unwrap())
            .collect::<Vec<_>>();
        write_record(&mut overlay, offset, &codes);
        let executable_offset = EXECUTABLE_OFFSET_DELTA + offset;
        write_record(&mut executable, executable_offset, &codes);
        let runtime_address = 0x8001_0000 + u32::try_from(executable_offset - 0x800).unwrap();
        let executable_pointer_offset = EXECUTABLE_OFFSET_DELTA + pointer_offset;
        executable[executable_pointer_offset..executable_pointer_offset + 4]
            .copy_from_slice(&runtime_address.to_le_bytes());
    }
    (overlay, executable)
}

fn write_record(output: &mut [u8], offset: usize, codes: &[u16]) {
    output[offset..offset + 2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let code_offset = offset + 2 + index * 2;
        output[code_offset..code_offset + 2].copy_from_slice(&code.to_le_bytes());
    }
}
