#[path = "records_context/storage.rs"]
mod storage;
#[path = "records_context/strips.rs"]
mod strips;

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::assembly::{
    BOOT_NOTICE_ENTRY_HOOK_ADDRESS, BOOT_NOTICE_ENTRY_HOOK_OFFSET, BOOT_NOTICE_EXIT_HOOK_ADDRESS,
    BOOT_NOTICE_EXIT_HOOK_OFFSET, BOOT_NOTICE_LOAD_ARGUMENT_OFFSET, BOOT_NOTICE_PROGRAM_ADDRESS,
    BOOT_NOTICE_PROGRAM_CAPACITY, BOOT_NOTICE_PROGRAM_OFFSET,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS, RECORDS_CARD_OPERATION_RETURN_ADDRESS,
    RECORDS_ENTRY_HOOK_OFFSET, RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS,
    RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY, RECORDS_ENTRY_PROGRAM_OFFSET,
    RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS, RECORDS_ENTRY_SETUP_ADDRESS, RECORDS_EXIT_HOOK_OFFSET,
    RECORDS_EXIT_HOOK_RUNTIME_ADDRESS, RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
    RECORDS_EXIT_RESTORE_PROGRAM_OFFSET, RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS,
    RECORDS_EXIT_TEARDOWN_ADDRESS, RECORDS_LOAD_RETURN_HOOK_OFFSET,
    RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS, RECORDS_SAVE_RETURN_HOOK_OFFSET,
    RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS, RecordsHookInstall, UPLOAD_ROUTINE_ADDRESS,
    build_boot_notice_upload_program, build_records_card_operation_refresh_program,
    build_records_entry_upload_program, build_records_exit_restore_program,
    patch_boot_notice_calls, patch_records_entry_setup_call, patch_records_exit_teardown_call,
    patch_records_load_return_call, patch_records_save_return_call,
};
use super::model::{
    BootNoticeGlyphReport, DESCRIPTOR_BYTE_COUNT, GlyphUploadContextReport,
    RecordsCardOperationRefreshReport,
};
use super::{
    MENU_RUNTIME_BASE, PreparedGlyph, encode_descriptor, entry_report, ranges_are_allowed,
    vram_rect,
};

pub(super) use storage::allocate_storage;
#[cfg(test)]
pub(super) use storage::{
    RECORDS_FIRST_STORAGE_START, RECORDS_STORAGE_END, RECORDS_STORAGE_SLOT_COUNT,
};

pub(super) struct RecordsContextInstall {
    pub(super) boot_notice_report: BootNoticeGlyphReport,
    pub(super) entry_report: GlyphUploadContextReport,
    pub(super) card_operation_refresh_report: RecordsCardOperationRefreshReport,
    pub(super) exit_restore_report: GlyphUploadContextReport,
    pub(super) expected_menu_write_ranges: Vec<[usize; 2]>,
    pub(super) main_executable_candidate: Vec<u8>,
    pub(super) machine_code_writes: Vec<RecordsHookInstall>,
}

pub(super) fn install(
    glyphs: &[PreparedGlyph],
    source_restorations: &[PreparedGlyph],
    source_menu: &[u8],
    output_menu: &mut [u8],
    source_main_executable: &[u8],
) -> Result<RecordsContextInstall> {
    validate_context_populations(glyphs, source_restorations)?;
    let entry_strips = strips::split_large_uploads(glyphs)?;
    let restore_strips = strips::split_large_uploads(source_restorations)?;
    let glyphs = entry_strips.as_slice();
    let source_restorations = restore_strips.as_slice();
    storage::validate_source_storage(source_menu)?;
    ensure!(
        output_menu.len() == source_menu.len(),
        "Records contextual glyph storage changed MENU length"
    );
    let entry_payload_sizes = glyphs
        .iter()
        .map(|glyph| glyph.payload.len())
        .collect::<Vec<_>>();
    let exit_restore_payload_sizes = source_restorations
        .iter()
        .map(|glyph| glyph.payload.len())
        .collect::<Vec<_>>();
    let layout = allocate_storage(&entry_payload_sizes, &exit_restore_payload_sizes)?;

    let entry_descriptor_runtime_address = MENU_RUNTIME_BASE
        .checked_add(u32::try_from(layout.entry_descriptor_offset)?)
        .context("Records entry descriptor address overflow")?;
    let entry_program =
        build_records_entry_upload_program(entry_descriptor_runtime_address, glyphs.len())?;
    let boot_notice_program =
        build_boot_notice_upload_program(entry_descriptor_runtime_address, glyphs.len())?;
    let card_operation_refresh_program = build_records_card_operation_refresh_program(
        entry_descriptor_runtime_address,
        glyphs.len(),
    )?;
    let exit_restore_descriptor_runtime_address = MENU_RUNTIME_BASE
        .checked_add(u32::try_from(layout.exit_restore_descriptor_offset)?)
        .context("Records exit restore descriptor address overflow")?;
    let exit_restore_program = build_records_exit_restore_program(
        exit_restore_descriptor_runtime_address,
        source_restorations.len(),
    )?;

    let mut expected_menu_write_ranges = vec![
        [
            BOOT_NOTICE_PROGRAM_OFFSET,
            BOOT_NOTICE_PROGRAM_OFFSET + boot_notice_program.bytes.len(),
        ],
        [
            RECORDS_ENTRY_PROGRAM_OFFSET,
            RECORDS_ENTRY_PROGRAM_OFFSET + entry_program.bytes.len(),
        ],
        [
            RECORDS_EXIT_RESTORE_PROGRAM_OFFSET,
            RECORDS_EXIT_RESTORE_PROGRAM_OFFSET + exit_restore_program.bytes.len(),
        ],
        [
            RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET,
            RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET
                + card_operation_refresh_program.bytes.len(),
        ],
    ];
    expected_menu_write_ranges.extend(layout.data_write_ranges.iter().copied());
    expected_menu_write_ranges.sort_unstable();
    for [start, end] in &expected_menu_write_ranges {
        ensure!(
            source_menu.get(*start..*end) == output_menu.get(*start..*end),
            "Records contextual glyph storage overlaps another MENU writer at {start:#x}..{end:#x}"
        );
    }
    let before_menu = output_menu.to_vec();
    let mut main_executable_candidate = source_main_executable.to_vec();

    write_program(
        output_menu,
        BOOT_NOTICE_PROGRAM_OFFSET,
        BOOT_NOTICE_PROGRAM_CAPACITY,
        &boot_notice_program.bytes,
    )?;

    write_program(
        output_menu,
        RECORDS_ENTRY_PROGRAM_OFFSET,
        RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY,
        &entry_program.bytes,
    )?;
    write_program(
        output_menu,
        RECORDS_EXIT_RESTORE_PROGRAM_OFFSET,
        RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
        &exit_restore_program.bytes,
    )?;
    write_program(
        output_menu,
        RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET,
        RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
        &card_operation_refresh_program.bytes,
    )?;
    let entry_entries = write_context_data(
        glyphs,
        &layout.entry_payload_offsets,
        layout.entry_descriptor_offset,
        output_menu,
    )?;
    let exit_restore_entries = write_context_data(
        source_restorations,
        &layout.exit_restore_payload_offsets,
        layout.exit_restore_descriptor_offset,
        output_menu,
    )?;

    let entry_hook =
        patch_records_entry_setup_call(source_main_executable, &mut main_executable_candidate)?;
    let exit_hook =
        patch_records_exit_teardown_call(source_main_executable, &mut main_executable_candidate)?;
    let load_return_hook =
        patch_records_load_return_call(source_main_executable, &mut main_executable_candidate)?;
    let save_return_hook =
        patch_records_save_return_call(source_main_executable, &mut main_executable_candidate)?;
    let mut machine_code_writes =
        patch_boot_notice_calls(source_main_executable, &mut main_executable_candidate)?;
    machine_code_writes.extend([entry_hook, exit_hook, load_return_hook, save_return_hook]);

    ensure!(
        ranges_are_allowed(
            &difference_ranges(&before_menu, output_menu),
            &expected_menu_write_ranges,
        ),
        "Records contextual glyph upload escaped its MENU Expected Writes"
    );
    let expected_executable_write_ranges = executable_expected_write_ranges();
    ensure!(
        ranges_are_allowed(
            &difference_ranges(source_main_executable, &main_executable_candidate),
            &expected_executable_write_ranges,
        ),
        "Records contextual glyph hooks escaped their main-executable Expected Writes"
    );

    let entry_descriptor_byte_count = glyphs.len() * DESCRIPTOR_BYTE_COUNT;
    let exit_restore_descriptor_byte_count = source_restorations.len() * DESCRIPTOR_BYTE_COUNT;
    Ok(RecordsContextInstall {
        boot_notice_report: BootNoticeGlyphReport {
            loaded_catalog_index: super::assembly::MENU_CATALOG_INDEX,
            loaded_menu_runtime_range: [
                format!("0x{MENU_RUNTIME_BASE:08x}"),
                format!("0x{:08x}", MENU_RUNTIME_BASE + output_menu.len() as u32),
            ],
            entry_hook_runtime_address: format!("0x{BOOT_NOTICE_ENTRY_HOOK_ADDRESS:08x}"),
            exit_hook_runtime_address: format!("0x{BOOT_NOTICE_EXIT_HOOK_ADDRESS:08x}"),
            program_runtime_address: format!("0x{BOOT_NOTICE_PROGRAM_ADDRESS:08x}"),
            program_byte_count: boot_notice_program.bytes.len(),
            program_sha256: sha256_bytes(&boot_notice_program.bytes),
            entry_count: glyphs.len(),
            descriptor_runtime_address: format!("0x{entry_descriptor_runtime_address:08x}"),
            restore_program_runtime_address: format!(
                "0x{RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS:08x}"
            ),
            reuses_records_payloads: true,
            runtime_execution_verified: false,
        },
        entry_report: GlyphUploadContextReport {
            context: "records".to_string(),
            entry_count: entry_entries.len(),
            storage_path: "DAT2/MENU.BIZ".to_string(),
            source_storage_padding_verified: true,
            hook_path: "SLPS_021.20".to_string(),
            hook_offset: format!("0x{RECORDS_ENTRY_HOOK_OFFSET:06x}"),
            hook_runtime_address: format!("0x{RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS:08x}"),
            original_call_address: format!("0x{RECORDS_ENTRY_SETUP_ADDRESS:08x}"),
            upload_routine_address: format!("0x{UPLOAD_ROUTINE_ADDRESS:08x}"),
            program_storage_path: "DAT2/MENU.BIZ".to_string(),
            program_offset: format!("0x{RECORDS_ENTRY_PROGRAM_OFFSET:05x}"),
            program_runtime_address: format!("0x{RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS:08x}"),
            program_byte_count: entry_program.bytes.len(),
            program_byte_capacity: RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY,
            program_sha256: sha256_bytes(&entry_program.bytes),
            typed_instruction_count: entry_program.typed_instruction_count,
            descriptor_offset: format!("0x{:05x}", layout.entry_descriptor_offset),
            descriptor_runtime_address: format!("0x{entry_descriptor_runtime_address:08x}"),
            descriptor_byte_count: entry_descriptor_byte_count,
            payload_byte_count: entry_payload_sizes.iter().sum(),
            stored_payload_byte_count: entry_payload_sizes.iter().sum(),
            payload_stream_count: 0,
            payload_ranges: layout.entry_payload_ranges(&entry_payload_sizes),
            scratch_runtime_range: None,
            typed_program_verified: true,
            runtime_execution_verified: false,
            entries: entry_entries,
        },
        card_operation_refresh_report: RecordsCardOperationRefreshReport {
            context: "records_card_operation_return_refresh".to_string(),
            entry_count: glyphs.len(),
            hook_path: "SLPS_021.20".to_string(),
            hook_offsets: vec![
                format!("0x{RECORDS_LOAD_RETURN_HOOK_OFFSET:06x}"),
                format!("0x{RECORDS_SAVE_RETURN_HOOK_OFFSET:06x}"),
            ],
            hook_runtime_addresses: vec![
                format!("0x{RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS:08x}"),
                format!("0x{RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS:08x}"),
            ],
            original_call_address: format!("0x{RECORDS_CARD_OPERATION_RETURN_ADDRESS:08x}"),
            upload_routine_address: format!("0x{UPLOAD_ROUTINE_ADDRESS:08x}"),
            program_storage_path: "DAT2/MENU.BIZ".to_string(),
            program_offset: format!("0x{RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET:05x}"),
            program_runtime_address: format!(
                "0x{RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS:08x}"
            ),
            program_byte_count: card_operation_refresh_program.bytes.len(),
            program_byte_capacity: RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
            program_sha256: sha256_bytes(&card_operation_refresh_program.bytes),
            typed_instruction_count: card_operation_refresh_program.typed_instruction_count,
            descriptor_offset: format!("0x{:05x}", layout.entry_descriptor_offset),
            descriptor_runtime_address: format!("0x{entry_descriptor_runtime_address:08x}"),
            descriptor_byte_count: entry_descriptor_byte_count,
            reused_payload_byte_count: entry_payload_sizes.iter().sum(),
            typed_program_verified: true,
            runtime_execution_verified: false,
        },
        exit_restore_report: GlyphUploadContextReport {
            context: "records_exit_restore".to_string(),
            entry_count: exit_restore_entries.len(),
            storage_path: "DAT2/MENU.BIZ".to_string(),
            source_storage_padding_verified: true,
            hook_path: "SLPS_021.20".to_string(),
            hook_offset: format!("0x{RECORDS_EXIT_HOOK_OFFSET:06x}"),
            hook_runtime_address: format!("0x{RECORDS_EXIT_HOOK_RUNTIME_ADDRESS:08x}"),
            original_call_address: format!("0x{RECORDS_EXIT_TEARDOWN_ADDRESS:08x}"),
            upload_routine_address: format!("0x{UPLOAD_ROUTINE_ADDRESS:08x}"),
            program_storage_path: "DAT2/MENU.BIZ".to_string(),
            program_offset: format!("0x{RECORDS_EXIT_RESTORE_PROGRAM_OFFSET:05x}"),
            program_runtime_address: format!(
                "0x{RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS:08x}"
            ),
            program_byte_count: exit_restore_program.bytes.len(),
            program_byte_capacity: RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
            program_sha256: sha256_bytes(&exit_restore_program.bytes),
            typed_instruction_count: exit_restore_program.typed_instruction_count,
            descriptor_offset: format!("0x{:05x}", layout.exit_restore_descriptor_offset),
            descriptor_runtime_address: format!("0x{exit_restore_descriptor_runtime_address:08x}"),
            descriptor_byte_count: exit_restore_descriptor_byte_count,
            payload_byte_count: exit_restore_payload_sizes.iter().sum(),
            stored_payload_byte_count: exit_restore_payload_sizes.iter().sum(),
            payload_stream_count: 0,
            payload_ranges: layout.exit_restore_payload_ranges(&exit_restore_payload_sizes),
            scratch_runtime_range: None,
            typed_program_verified: true,
            runtime_execution_verified: false,
            entries: exit_restore_entries,
        },
        expected_menu_write_ranges,
        main_executable_candidate,
        machine_code_writes,
    })
}

fn validate_context_populations(
    glyphs: &[PreparedGlyph],
    source_restorations: &[PreparedGlyph],
) -> Result<()> {
    ensure!(
        !glyphs.is_empty() && source_restorations.len() == glyphs.len(),
        "Records entry and exit contextual glyph populations changed"
    );
    let entry_codes = glyphs
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    let restore_codes = source_restorations
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        entry_codes.len() == glyphs.len()
            && entry_codes == restore_codes
            && glyphs.iter().all(|glyph| !glyph.source_preservation)
            && source_restorations
                .iter()
                .all(|glyph| glyph.source_preservation)
            && glyphs
                .iter()
                .zip(source_restorations)
                .all(|(entry, restore)| {
                    entry.code == restore.code
                        && entry.cell == restore.cell
                        && entry.payload.len() == restore.payload.len()
                        && entry.payload.len() == entry.cell.width / 2 * entry.cell.height
                }),
        "Records exit restore population does not exactly match the entry-scoped overwritten cells"
    );
    Ok(())
}

fn write_context_data(
    glyphs: &[PreparedGlyph],
    payload_offsets: &[usize],
    descriptor_offset: usize,
    output_menu: &mut [u8],
) -> Result<Vec<super::model::GlyphUploadEntry>> {
    ensure!(
        payload_offsets.len() == glyphs.len(),
        "Records payload allocation does not match its glyph population"
    );
    let mut descriptors = Vec::with_capacity(glyphs.len() * DESCRIPTOR_BYTE_COUNT);
    let mut entries = Vec::with_capacity(glyphs.len());
    for (glyph, payload_offset) in glyphs.iter().zip(payload_offsets.iter().copied()) {
        output_menu[payload_offset..payload_offset + glyph.payload.len()]
            .copy_from_slice(&glyph.payload);
        let payload_runtime_address = MENU_RUNTIME_BASE
            .checked_add(u32::try_from(payload_offset)?)
            .context("Records contextual glyph payload address overflow")?;
        let rect = vram_rect(glyph)?;
        encode_descriptor(&mut descriptors, rect, payload_runtime_address);
        entries.push(entry_report(
            glyph,
            rect,
            payload_offset,
            payload_runtime_address,
        ));
    }
    let descriptor_end = descriptor_offset + descriptors.len();
    output_menu[descriptor_offset..descriptor_end].copy_from_slice(&descriptors);
    Ok(entries)
}

fn write_program(
    output_menu: &mut [u8],
    offset: usize,
    capacity: usize,
    program: &[u8],
) -> Result<()> {
    ensure!(
        program.len() <= capacity,
        "Records program exceeds its owned slot"
    );
    let destination = &mut output_menu[offset..offset + capacity];
    destination.fill(0);
    destination[..program.len()].copy_from_slice(program);
    Ok(())
}

pub(super) fn executable_expected_write_ranges() -> Vec<[usize; 2]> {
    vec![
        [
            BOOT_NOTICE_LOAD_ARGUMENT_OFFSET,
            BOOT_NOTICE_LOAD_ARGUMENT_OFFSET + 4,
        ],
        [
            BOOT_NOTICE_ENTRY_HOOK_OFFSET,
            BOOT_NOTICE_ENTRY_HOOK_OFFSET + 4,
        ],
        [
            BOOT_NOTICE_EXIT_HOOK_OFFSET,
            BOOT_NOTICE_EXIT_HOOK_OFFSET + 4,
        ],
        [RECORDS_ENTRY_HOOK_OFFSET, RECORDS_ENTRY_HOOK_OFFSET + 4],
        [RECORDS_EXIT_HOOK_OFFSET, RECORDS_EXIT_HOOK_OFFSET + 4],
        [
            RECORDS_LOAD_RETURN_HOOK_OFFSET,
            RECORDS_LOAD_RETURN_HOOK_OFFSET + 4,
        ],
        [
            RECORDS_SAVE_RETURN_HOOK_OFFSET,
            RECORDS_SAVE_RETURN_HOOK_OFFSET + 4,
        ],
    ]
}
