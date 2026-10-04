use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::Deserialize;

use super::model::{ModeSelectDispatcherBuildReport, ModeSelectPanelSelection, SurfaceInventory};
use crate::pipeline::sha256_bytes;
use crate::psx_static_analysis::{ExecutableDomain, bounded_jump_tables::read_bounded_jump_table};
use crate::source_disc::{
    MAIN_TEXT_RUNTIME_BASE, main_executable_text,
    profile::{MAIN_EXECUTABLE_RECORD, MODE_SELECT_OVERLAY_RECORD},
};

const BINDING_KIND: &str = "justice_gakuen2_mode_select_dispatcher_binding";
const REPORT_KIND: &str = "justice_gakuen2_mode_select_dispatcher_validation";
pub(super) const BINDING_FILE: &str = "dispatcher.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatcherBindingDocument {
    kind: String,
    writer: SelectionWriterDocument,
    dispatcher: SelectionDispatcherDocument,
    entries: Vec<DispatcherEntryDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionWriterDocument {
    source: RuntimeSourceRecordDocument,
    panel_index_load_address: String,
    presentation_initializer: String,
    panel_index_store_address: String,
    panel_index_state_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionDispatcherDocument {
    source: RuntimeSourceRecordDocument,
    panel_index_load_address: String,
    transfer_address: String,
    jump_table_address: String,
    out_of_range_target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSourceRecordDocument {
    path: String,
    sha256: String,
    runtime_base: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatcherEntryDocument {
    panel_index: usize,
    selection_asset_id: String,
    case_address: String,
    setup_entrypoint: String,
}

pub(super) fn validate_mode_select_dispatcher(
    inventory: &SurfaceInventory,
    mode_select_overlay: &[u8],
    main_executable: &[u8],
    selections: &[ModeSelectPanelSelection<'_>],
) -> Result<ModeSelectDispatcherBuildReport> {
    let binding = load_dispatcher_binding(inventory)?;
    validate_source_record(
        &binding.writer.source,
        MODE_SELECT_OVERLAY_RECORD.path,
        MODE_SELECT_OVERLAY_RECORD.sha256,
        mode_select_overlay,
    )?;
    validate_source_record(
        &binding.dispatcher.source,
        MAIN_EXECUTABLE_RECORD.path,
        MAIN_EXECUTABLE_RECORD.sha256,
        main_executable,
    )?;

    let panel_index_state_address = parse_address(
        &binding.writer.panel_index_state_address,
        "MODE SELECT panel-index state",
    )?;
    validate_selection_writer(
        &binding.writer,
        mode_select_overlay,
        panel_index_state_address,
    )?;
    let jump_table_address = validate_selection_dispatcher(
        &binding.dispatcher,
        &binding.entries,
        main_executable,
        panel_index_state_address,
    )?;
    validate_entry_population(&binding.entries, selections)?;

    Ok(ModeSelectDispatcherBuildReport {
        kind: REPORT_KIND.to_string(),
        writer_record_path: binding.writer.source.path,
        dispatcher_record_path: binding.dispatcher.source.path,
        panel_index_state_address: format_address(panel_index_state_address),
        jump_table_address: format_address(jump_table_address),
        entry_count: binding.entries.len(),
        source_records_match: true,
        writer_sequence_matches: true,
        dispatcher_sequence_matches: true,
        panel_population_matches: true,
    })
}

pub(super) fn mode_select_setup_entrypoint(
    inventory: &SurfaceInventory,
    selection_asset_id: &str,
) -> Result<u32> {
    let binding = load_dispatcher_binding(inventory)?;
    let entry = binding
        .entries
        .iter()
        .find(|entry| entry.selection_asset_id == selection_asset_id)
        .with_context(|| {
            format!("MODE SELECT dispatcher has no {selection_asset_id} selection entry")
        })?;
    parse_address(
        &entry.setup_entrypoint,
        "MODE SELECT dispatcher setup entrypoint",
    )
}

pub(super) fn mode_select_panel_index(
    inventory: &SurfaceInventory,
    selection_asset_id: &str,
) -> Result<usize> {
    let binding = load_dispatcher_binding(inventory)?;
    binding
        .entries
        .iter()
        .find(|entry| entry.selection_asset_id == selection_asset_id)
        .map(|entry| entry.panel_index)
        .with_context(|| {
            format!("MODE SELECT dispatcher has no {selection_asset_id} selection entry")
        })
}

pub(super) fn mode_select_panel_index_state_address(inventory: &SurfaceInventory) -> Result<u32> {
    let binding = load_dispatcher_binding(inventory)?;
    parse_address(
        &binding.writer.panel_index_state_address,
        "MODE SELECT panel-index state",
    )
}

fn load_dispatcher_binding(inventory: &SurfaceInventory) -> Result<DispatcherBindingDocument> {
    let binding_bytes = inventory
        .binding_files
        .get(BINDING_FILE)
        .with_context(|| format!("MODE SELECT surface inventory is missing {BINDING_FILE}"))?;
    let binding: DispatcherBindingDocument = serde_json::from_slice(binding_bytes)
        .with_context(|| format!("failed to parse MODE SELECT binding {BINDING_FILE}"))?;
    ensure!(
        binding.kind == BINDING_KIND,
        "unsupported MODE SELECT binding kind {:?}",
        binding.kind
    );
    Ok(binding)
}

fn validate_source_record(
    source: &RuntimeSourceRecordDocument,
    expected_path: &str,
    expected_sha256: &str,
    bytes: &[u8],
) -> Result<()> {
    ensure!(
        source.path == expected_path
            && source.sha256 == expected_sha256
            && sha256_bytes(bytes) == expected_sha256,
        "MODE SELECT dispatcher source record changed for {expected_path}"
    );
    parse_address(&source.runtime_base, "MODE SELECT source runtime base")?;
    Ok(())
}

fn validate_selection_writer(
    writer: &SelectionWriterDocument,
    overlay: &[u8],
    panel_index_state_address: u32,
) -> Result<()> {
    let runtime_base = parse_address(&writer.source.runtime_base, "MODESEL runtime base")?;
    let load_address = parse_address(&writer.panel_index_load_address, "MODESEL panel-index load")?;
    let initializer = parse_address(
        &writer.presentation_initializer,
        "MODESEL presentation initializer",
    )?;
    let store_address = parse_address(
        &writer.panel_index_store_address,
        "MODESEL panel-index store",
    )?;
    let expected_store_address = load_address
        .checked_add(0x14)
        .context("MODESEL selection writer address overflow")?;
    ensure!(
        runtime_base == 0x800a_2000 && store_address == expected_store_address,
        "MODESEL selection writer layout changed"
    );
    let state_high = (panel_index_state_address >> 16) as u16;
    let state_low = panel_index_state_address as u16 as i16;
    let expected = [
        (
            load_address,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0,
            },
        ),
        (
            instruction_address(load_address, 4, "MODESEL selection writer")?,
            Instruction::Jal {
                target: initializer,
            },
        ),
        (
            instruction_address(load_address, 8, "MODESEL selection writer")?,
            Instruction::nop(),
        ),
        (
            instruction_address(load_address, 0x0c, "MODESEL selection writer")?,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::S0,
                offset: 0,
            },
        ),
        (
            instruction_address(load_address, 0x10, "MODESEL selection writer")?,
            Instruction::Lui {
                rt: Register::AT,
                immediate: state_high,
            },
        ),
        (
            store_address,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::AT,
                offset: state_low,
            },
        ),
    ];
    validate_instructions(overlay, runtime_base, &expected, "MODESEL selection writer")
}

fn validate_selection_dispatcher(
    dispatcher: &SelectionDispatcherDocument,
    entries: &[DispatcherEntryDocument],
    main_executable: &[u8],
    panel_index_state_address: u32,
) -> Result<u32> {
    ensure!(!entries.is_empty(), "MODE SELECT dispatcher has no entries");
    let runtime_base = parse_address(
        &dispatcher.source.runtime_base,
        "main executable runtime base",
    )?;
    ensure!(
        runtime_base == MAIN_TEXT_RUNTIME_BASE,
        "MODE SELECT dispatcher main executable base changed"
    );
    let text = main_executable_text(main_executable)?;
    let panel_index_load_address = parse_address(
        &dispatcher.panel_index_load_address,
        "MODE SELECT dispatcher panel-index load",
    )?;
    let transfer_address = parse_address(
        &dispatcher.transfer_address,
        "MODE SELECT dispatcher transfer",
    )?;
    let declared_table_address = parse_address(
        &dispatcher.jump_table_address,
        "MODE SELECT dispatcher jump table",
    )?;
    let out_of_range_target = parse_address(
        &dispatcher.out_of_range_target,
        "MODE SELECT dispatcher out-of-range target",
    )?;
    let state_high = (panel_index_state_address >> 16) as u16;
    let state_low = panel_index_state_address as u16 as i16;
    let selection_base_address = panel_index_load_address
        .checked_sub(4)
        .context("MODE SELECT dispatcher selection load underflow")?;
    validate_instructions(
        text,
        runtime_base,
        &[
            (
                selection_base_address,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: state_high,
                },
            ),
            (
                panel_index_load_address,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: state_low,
                },
            ),
        ],
        "MODE SELECT dispatcher selection reader",
    )?;

    let transfer_offset = runtime_offset(text, runtime_base, transfer_address)?;
    let domain = ExecutableDomain::full_image(text.len());
    let table = read_bounded_jump_table(text, runtime_base, &domain, transfer_offset)
        .context("MODE SELECT dispatcher is not a bounded typed jump table")?;
    ensure!(
        table.selector_register == Register::V1
            && table.targets.len() == entries.len()
            && table.table_address == declared_table_address,
        "MODE SELECT dispatcher jump-table contract changed"
    );
    let entry_count = i16::try_from(entries.len())?;
    let bound_address = transfer_address
        .checked_sub(0x1c)
        .context("MODE SELECT dispatcher bound address underflow")?;
    let branch_address = transfer_address
        .checked_sub(0x18)
        .context("MODE SELECT dispatcher branch address underflow")?;
    validate_instructions(
        text,
        runtime_base,
        &[
            (
                bound_address,
                Instruction::Sltiu {
                    rt: Register::V0,
                    rs: Register::V1,
                    immediate: entry_count,
                },
            ),
            (
                branch_address,
                Instruction::Beq {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: out_of_range_target,
                },
            ),
        ],
        "MODE SELECT dispatcher bound",
    )?;

    for (position, (entry, &actual_case)) in entries.iter().zip(&table.targets).enumerate() {
        let case_address = parse_address(&entry.case_address, "MODE SELECT case address")?;
        let setup_entrypoint =
            parse_address(&entry.setup_entrypoint, "MODE SELECT setup entrypoint")?;
        ensure!(
            entry.panel_index == position && case_address == actual_case,
            "MODE SELECT dispatcher table entry {position} changed"
        );
        validate_instructions(
            text,
            runtime_base,
            &[(
                case_address,
                Instruction::Jal {
                    target: setup_entrypoint,
                },
            )],
            "MODE SELECT dispatcher case",
        )?;
    }
    Ok(table.table_address)
}

fn validate_entry_population(
    entries: &[DispatcherEntryDocument],
    selections: &[ModeSelectPanelSelection<'_>],
) -> Result<()> {
    ensure!(
        entries.len() == selections.len(),
        "MODE SELECT dispatcher population differs from built mode population"
    );
    let mut assets_by_panel = BTreeMap::new();
    for selection in selections {
        ensure!(
            selection.panel_index < entries.len()
                && assets_by_panel
                    .insert(selection.panel_index, selection.asset_id)
                    .is_none(),
            "MODE SELECT build contains an invalid or duplicate panel index {}",
            selection.panel_index
        );
    }
    for (position, entry) in entries.iter().enumerate() {
        ensure!(
            entry.panel_index == position
                && assets_by_panel.get(&position).copied()
                    == Some(entry.selection_asset_id.as_str()),
            "MODE SELECT dispatcher panel {position} differs from the built panel identity"
        );
    }
    Ok(())
}

fn validate_instructions(
    data: &[u8],
    runtime_base: u32,
    expected: &[(u32, Instruction)],
    role: &str,
) -> Result<()> {
    for (address, expected_instruction) in expected {
        let offset = runtime_offset(data, runtime_base, *address)?;
        let bytes: [u8; 4] = data[offset..offset + 4]
            .try_into()
            .expect("validated R3000A instruction range");
        let instruction = decode(u32::from_le_bytes(bytes), *address)
            .with_context(|| format!("failed to decode {role} at {}", format_address(*address)))?;
        ensure!(
            instruction == *expected_instruction,
            "{role} changed at {}",
            format_address(*address)
        );
    }
    Ok(())
}

fn runtime_offset(data: &[u8], runtime_base: u32, address: u32) -> Result<usize> {
    let offset = address
        .checked_sub(runtime_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .context("MODE SELECT binding address precedes its source runtime base")?;
    let end = offset
        .checked_add(4)
        .context("MODE SELECT binding instruction range overflow")?;
    ensure!(
        offset.is_multiple_of(4) && end <= data.len(),
        "MODE SELECT binding address {} is outside its source record",
        format_address(address)
    );
    Ok(offset)
}

fn instruction_address(base: u32, delta: u32, role: &str) -> Result<u32> {
    base.checked_add(delta)
        .with_context(|| format!("{role} address overflow"))
}

fn parse_address(value: &str, role: &str) -> Result<u32> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("{role} is not a hexadecimal address"))?;
    ensure!(
        digits.len() == 8
            && digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{role} is not a canonical lowercase address"
    );
    Ok(u32::from_str_radix(digits, 16)?)
}

fn format_address(address: u32) -> String {
    format!("0x{address:08x}")
}
