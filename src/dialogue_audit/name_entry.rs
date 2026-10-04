use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, PROFILE_ID, Register, decode};

use crate::pipeline::sha256_bytes;
use crate::source_disc::{SupportedSourceDisc, load_main_executable_image};

use super::name_entry_choice_descriptors::audit_name_entry_choice_descriptors;
use super::name_entry_input::audit_name_input_path;
use super::name_entry_model::{
    DialogueNameEntryAuditConfig, DialogueNameEntryAuditReport, DialogueNameEntryPageAudit,
    DialogueNameFieldAudit,
};
use super::name_entry_record_transport::audit_name_record_transport;
use super::name_entry_renderer::audit_name_renderer;
use super::name_entry_save_transport::audit_name_save_transport;
use super::name_entry_sex_choices::audit_sex_choice_surface;
use super::script_source::load_mgame_from_source;

pub(super) const OVERLAY_PATH: &str = "DAT1/MGENT.BIN";
pub(super) const OVERLAY_SHA256: &str =
    "eaf243e2d5a78919055b453611ea8b785766a6dc945966a1d5c0ce166714f70a";
const OVERLAY_SIZE: usize = 36_796;
pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x8017_a000;
pub(super) const PAGE_POINTER_TABLE_OFFSET: usize = 0x0d08;
const PAGE_POINTER_TABLE_TRAILER_OFFSET: usize = 0x0d14;
const PAGE_POINTER_TABLE_TRAILER: [u16; 2] = [PADDING_CODE, 0x0fff];
pub(super) const SELECTABLE_CODE_SEQUENCE_OFFSET: usize = 0x0d18;
const SELECTABLE_CODE_SEQUENCE_TERMINATOR_OFFSET: usize = 0x0ee0;
pub(super) const PADDING_CODE: u16 = 0x061e;
const MESSAGE_END_CODE: u16 = 0x3001;
const COPY_ROUTINE_OFFSET: usize = 0x6988;
const NICKNAME_COMPANION_BUFFER: u32 = 0x801f_1886;

pub(super) const PAGE_SPECS: [(&str, usize, usize, usize); 3] = [
    ("hiragana", 0x0b14, 90, 6),
    ("katakana", 0x0bc8, 90, 8),
    ("alphanumeric", 0x0c7c, 70, 8),
];

const FIELD_SPECS: [(&str, usize, usize, u32); 3] = [
    ("family_name", 0x12, 6, 0x801f_1866),
    ("given_name", 0x22, 6, 0x801f_1876),
    ("nickname", 0x32, 4, 0x801f_1896),
];

pub fn audit_dialogue_name_entry(
    config: &DialogueNameEntryAuditConfig,
) -> Result<DialogueNameEntryAuditReport> {
    let report = inspect_dialogue_name_entry(&config.cue)?;
    write_report(&config.output, &report)?;
    Ok(report)
}

pub(super) fn inspect_dialogue_name_entry(cue_path: &Path) -> Result<DialogueNameEntryAuditReport> {
    let source = SupportedSourceDisc::open(cue_path)?;
    let (overlay, source_bin_sha256, overlay_sha256) =
        load_supported_name_entry_overlay_from_source(&source)?;
    let mut report = analyze_name_entry_overlay(&overlay, source_bin_sha256, overlay_sha256)?;
    let executable = load_main_executable_image(source.image_path())?;
    report.save_transport = Some(audit_name_save_transport(
        &executable.path,
        &executable.data,
        executable
            .runtime_base
            .expect("validated main executable text has a runtime base"),
    )?);
    report.in_game_record_transport = Some(audit_name_record_transport(&load_mgame_from_source(
        &source,
    )?)?);
    Ok(report)
}

pub(super) fn load_supported_name_entry_overlay(
    cue_path: &Path,
) -> Result<(Vec<u8>, String, String)> {
    let source = SupportedSourceDisc::open(cue_path)?;
    load_supported_name_entry_overlay_from_source(&source)
}

pub(super) fn load_supported_name_entry_overlay_from_source(
    source: &SupportedSourceDisc,
) -> Result<(Vec<u8>, String, String)> {
    let (_, overlay) = source.read_record(OVERLAY_PATH)?;
    ensure!(
        overlay.len() == OVERLAY_SIZE,
        "unexpected {OVERLAY_PATH} size"
    );
    let overlay_sha256 = sha256_bytes(&overlay);
    ensure!(
        overlay_sha256 == OVERLAY_SHA256,
        "unsupported {OVERLAY_PATH} SHA-256: {overlay_sha256}"
    );
    Ok((
        overlay,
        source.source_bin_sha256().to_string(),
        overlay_sha256,
    ))
}

pub(super) fn analyze_name_entry_overlay(
    overlay: &[u8],
    source_bin_sha256: String,
    overlay_sha256: String,
) -> Result<DialogueNameEntryAuditReport> {
    ensure!(
        overlay.len() == OVERLAY_SIZE,
        "unexpected {OVERLAY_PATH} size"
    );
    validate_name_routines(overlay)?;
    ensure!(
        little_u16_words(bounded_slice(
            overlay,
            PAGE_POINTER_TABLE_TRAILER_OFFSET,
            PAGE_POINTER_TABLE_TRAILER.len() * 2,
        )?) == PAGE_POINTER_TABLE_TRAILER,
        "name-entry page-pointer table trailer changed"
    );

    let mut pages = Vec::new();
    let mut selectable_codes = Vec::new();
    for (index, (name, offset, cell_count, expected_padding_count)) in
        PAGE_SPECS.into_iter().enumerate()
    {
        let pointer = little_u32(overlay, PAGE_POINTER_TABLE_OFFSET + index * 4)?;
        let expected_pointer = OVERLAY_RUNTIME_BASE + u32::try_from(offset)?;
        ensure!(pointer == expected_pointer, "{name} page pointer changed");
        let bytes = bounded_slice(overlay, offset, cell_count * 2)?;
        let cells = little_u16_words(bytes);
        let page_selectable_codes: Vec<_> = cells
            .iter()
            .copied()
            .filter(|code| *code != PADDING_CODE)
            .collect();
        let padding_cell_count = cells.len() - page_selectable_codes.len();
        ensure!(
            padding_cell_count == expected_padding_count,
            "{name} page padding population changed"
        );
        let unique_selectable_code_count = page_selectable_codes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len();
        selectable_codes.extend(page_selectable_codes);
        pages.push(DialogueNameEntryPageAudit {
            name: name.to_string(),
            file_offset: hex_offset(offset),
            runtime_address: hex_address(expected_pointer),
            cell_count: cells.len(),
            padding_cell_count,
            selectable_cell_count: cells.len() - padding_cell_count,
            unique_selectable_code_count,
            source_cells_sha256: sha256_bytes(bytes),
            source_cells: cells.into_iter().map(hex_code).collect(),
        });
    }

    let source_sequence_bytes = bounded_slice(
        overlay,
        SELECTABLE_CODE_SEQUENCE_OFFSET,
        selectable_codes.len() * 2,
    )?;
    ensure!(
        little_u16_words(source_sequence_bytes) == selectable_codes,
        "name-entry selectable code sequence differs from visible page cells"
    );
    ensure!(
        little_u16(overlay, SELECTABLE_CODE_SEQUENCE_TERMINATOR_OFFSET)? == u16::MAX,
        "name-entry selectable code sequence terminator changed"
    );
    let unique_selectable_code_count = selectable_codes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    let fields = FIELD_SPECS
        .into_iter()
        .map(
            |(name, source_record_offset, visible_glyph_capacity, destination)| {
                DialogueNameFieldAudit {
                    name: name.to_string(),
                    source_record_offset: hex_offset(source_record_offset),
                    visible_glyph_capacity,
                    destination_buffer_runtime_address: hex_address(destination),
                    terminator_runtime_address: hex_address(
                        destination + u32::try_from(visible_glyph_capacity * 2).unwrap(),
                    ),
                }
            },
        )
        .collect();
    let choice_surfaces = audit_name_entry_choice_descriptors(overlay)?;
    let sex_choice_surface = audit_sex_choice_surface(overlay)?;
    let input = audit_name_input_path(overlay)?;
    let renderer = audit_name_renderer(overlay)?;

    Ok(DialogueNameEntryAuditReport {
        kind: "Justice Gakuen 2 dialogue name-entry source audit".to_string(),
        source_bin_sha256,
        overlay_path: OVERLAY_PATH.to_string(),
        overlay_sha256,
        overlay_size: overlay.len(),
        overlay_runtime_base: hex_address(OVERLAY_RUNTIME_BASE),
        typed_isa_profile: PROFILE_ID.to_string(),
        page_pointer_table_file_offset: hex_offset(PAGE_POINTER_TABLE_OFFSET),
        page_pointer_table_runtime_address: hex_address(
            OVERLAY_RUNTIME_BASE + u32::try_from(PAGE_POINTER_TABLE_OFFSET)?,
        ),
        pages,
        source_selectable_cell_count: selectable_codes.len(),
        source_unique_selectable_code_count: unique_selectable_code_count,
        source_duplicate_selectable_code_count: selectable_codes.len()
            - unique_selectable_code_count,
        selectable_code_sequence_file_offset: hex_offset(SELECTABLE_CODE_SEQUENCE_OFFSET),
        selectable_code_sequence_sha256: sha256_bytes(source_sequence_bytes),
        selectable_code_sequence_terminator_file_offset: hex_offset(
            SELECTABLE_CODE_SEQUENCE_TERMINATOR_OFFSET,
        ),
        fields,
        choice_surfaces,
        sex_choice_surface,
        input,
        renderer,
        save_transport: None,
        in_game_record_transport: None,
        message_end_code: hex_code(MESSAGE_END_CODE),
        copy_routine_runtime_address: hex_address(
            OVERLAY_RUNTIME_BASE + u32::try_from(COPY_ROUTINE_OFFSET)?,
        ),
        nickname_companion_buffer_runtime_address: hex_address(NICKNAME_COMPANION_BUFFER),
    })
}

fn validate_name_routines(overlay: &[u8]) -> Result<()> {
    for (offset, expected) in [
        (
            0x6804,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 5,
            },
        ),
        (
            0x6828,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 5,
            },
        ),
        (
            0x684c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 3,
            },
        ),
        (
            0x69d4,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x12,
            },
        ),
        (
            0x69dc,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x22,
            },
        ),
        (
            0x69e0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x32,
            },
        ),
        (
            0x69f0,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::A2,
                offset: 0,
            },
        ),
    ] {
        let actual = decode_instruction(overlay, offset)?;
        ensure!(actual == expected, "name producer changed at {offset:#x}");
    }
    for offset in [0x6818, 0x683c, 0x6860] {
        let actual = decode_instruction(overlay, offset)?;
        ensure!(
            actual
                == Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: i16::try_from(MESSAGE_END_CODE).unwrap(),
                },
            "name terminator producer changed at {offset:#x}"
        );
    }
    ensure!(
        decode_instruction(overlay, COPY_ROUTINE_OFFSET)?
            == Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        "name destination selector changed"
    );
    ensure!(
        decode_instruction(overlay, 0x6a14)?
            == Instruction::Lui {
                rt: Register::A3,
                immediate: 0x801f,
            }
            && decode_instruction(overlay, 0x6a18)?
                == Instruction::Ori {
                    rt: Register::A3,
                    rs: Register::A3,
                    immediate: 0x1886,
                },
        "nickname companion destination changed"
    );
    Ok(())
}

pub(super) fn decode_instruction(data: &[u8], offset: usize) -> Result<Instruction> {
    let word = u32::from_le_bytes(bounded_slice(data, offset, 4)?.try_into()?);
    let pc = OVERLAY_RUNTIME_BASE + u32::try_from(offset)?;
    decode(word, pc).with_context(|| format!("failed to decode instruction at {pc:#010x}"))
}

fn little_u16(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bounded_slice(data, offset, 2)?.try_into()?,
    ))
}

fn little_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bounded_slice(data, offset, 4)?.try_into()?,
    ))
}

fn little_u16_words(data: &[u8]) -> Vec<u16> {
    data.as_chunks::<2>()
        .0
        .iter()
        .map(|word| u16::from_le_bytes(*word))
        .collect()
}

pub(super) fn bounded_slice(data: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset.checked_add(length).context("slice end overflow")?;
    ensure!(end <= data.len(), "truncated name-entry overlay data");
    Ok(&data[offset..end])
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:04x}")
}

fn hex_code(value: u16) -> String {
    format!("0x{value:04x}")
}

fn write_report(path: &Path, report: &DialogueNameEntryAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
