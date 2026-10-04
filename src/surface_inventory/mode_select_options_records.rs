use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::Deserialize;

use super::mode_select_dispatcher::mode_select_setup_entrypoint;
use super::model::{SurfaceInventory, SurfaceResolution, SurfaceTargetLayer, SurfaceTargetObject};
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    MAIN_TEXT_RUNTIME_BASE, main_executable_text,
    profile::{MAIN_EXECUTABLE_RECORD, MENU_RECORD, OPTIONS_INFO_RECORD, OPTIONS_OVERLAY_RECORD},
};

const BINDING_KIND: &str = "justice_gakuen2_mode_select_options_records_binding";
pub(super) const BINDING_FILE: &str = "options-records.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsRecordsBindingDocument {
    kind: String,
    main_executable: MainExecutableDocument,
    options: OptionsEntryDocument,
    records: RecordsEntryDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MainExecutableDocument {
    path: String,
    sha256: String,
    runtime_base: String,
    catalog_table_offset: String,
    catalog_entry_size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsEntryDocument {
    surface_id: String,
    setup_entrypoint: String,
    overlay_loader_address: String,
    overlay_load_call_address: String,
    overlay_catalog_index_address: String,
    overlay_entry_pointer_address: String,
    overlay_entry_pointer_load_address: String,
    overlay_entry_call_address: String,
    overlay: OverlayRecordDocument,
    information_load_destination_address: String,
    information_load_destination_high_address: String,
    information_load_destination_low_address: String,
    information_load_call_address: String,
    information_catalog_index_address: String,
    information: InformationRecordDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayRecordDocument {
    path: String,
    sha256: String,
    runtime_base: String,
    entrypoint: String,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InformationRecordDocument {
    path: String,
    stored_sha256: String,
    decoded_sha256: String,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordsEntryDocument {
    surface_id: String,
    setup_entrypoint: String,
    native_setup_call_address: String,
    native_setup_address: String,
    native_setup_argument: i16,
    interactive_loop_call_address: String,
    interactive_loop_address: String,
    render_call_address: String,
    render_address: String,
}

pub(super) fn validate_mode_select_options_and_records(
    inventory: &SurfaceInventory,
    main_executable: &[u8],
    options_overlay: &[u8],
    options_information_stored: &[u8],
) -> Result<()> {
    let binding = load_binding(inventory)?;
    validate_main_executable(&binding.main_executable, main_executable)?;
    validate_options_entry(
        inventory,
        &binding,
        main_executable,
        options_overlay,
        options_information_stored,
    )?;
    validate_records_entry(inventory, &binding.records, main_executable)
}

fn load_binding(inventory: &SurfaceInventory) -> Result<OptionsRecordsBindingDocument> {
    let binding_bytes = inventory
        .binding_files
        .get(BINDING_FILE)
        .with_context(|| format!("MODE SELECT surface inventory is missing {BINDING_FILE}"))?;
    let binding: OptionsRecordsBindingDocument = serde_json::from_slice(binding_bytes)
        .with_context(|| format!("failed to parse MODE SELECT binding {BINDING_FILE}"))?;
    ensure!(
        binding.kind == BINDING_KIND,
        "unsupported MODE SELECT options/records binding kind {:?}",
        binding.kind
    );
    Ok(binding)
}

fn validate_main_executable(source: &MainExecutableDocument, main_executable: &[u8]) -> Result<()> {
    ensure!(
        source.path == MAIN_EXECUTABLE_RECORD.path
            && source.sha256 == MAIN_EXECUTABLE_RECORD.sha256
            && main_executable.len() == MAIN_EXECUTABLE_RECORD.size
            && sha256_bytes(main_executable) == MAIN_EXECUTABLE_RECORD.sha256,
        "MODE SELECT options/records main-executable source changed"
    );
    ensure!(
        parse_address(&source.runtime_base, "main executable runtime base")?
            == MAIN_TEXT_RUNTIME_BASE,
        "MODE SELECT options/records main-executable base changed"
    );
    ensure!(
        source.catalog_entry_size == 12,
        "MODE SELECT source catalog entry size changed"
    );
    parse_offset(
        &source.catalog_table_offset,
        "MODE SELECT source catalog table",
    )?;
    Ok(())
}

fn validate_options_entry(
    inventory: &SurfaceInventory,
    binding: &OptionsRecordsBindingDocument,
    main_executable: &[u8],
    options_overlay: &[u8],
    options_information_stored: &[u8],
) -> Result<()> {
    let options = &binding.options;
    validate_resolved_surface(
        inventory,
        &options.surface_id,
        "options/menu-renderer",
        &[
            (MENU_RECORD.path, SurfaceTargetLayer::DecodedRecord),
            (OPTIONS_OVERLAY_RECORD.path, SurfaceTargetLayer::IsoRecord),
            (OPTIONS_INFO_RECORD.path, SurfaceTargetLayer::DecodedRecord),
        ],
    )?;
    let setup_entrypoint = parse_address(&options.setup_entrypoint, "options setup entrypoint")?;
    ensure!(
        options.surface_id == "mode-select/entry/options"
            && setup_entrypoint == mode_select_setup_entrypoint(inventory, "options")?,
        "MODE SELECT options binding no longer matches its dispatcher entry"
    );

    let overlay_runtime_base =
        parse_address(&options.overlay.runtime_base, "options runtime base")?;
    let overlay_entrypoint = parse_address(&options.overlay.entrypoint, "options entrypoint")?;
    ensure!(
        options.overlay.path == OPTIONS_OVERLAY_RECORD.path
            && options.overlay.sha256 == OPTIONS_OVERLAY_RECORD.sha256
            && options_overlay.len() == OPTIONS_OVERLAY_RECORD.size
            && sha256_bytes(options_overlay) == OPTIONS_OVERLAY_RECORD.sha256
            && overlay_runtime_base == 0x800a_2000
            && read_u32(options_overlay, 0, "options overlay entry pointer")? == overlay_entrypoint,
        "MODE SELECT options overlay source binding changed"
    );
    let overlay_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        options.overlay.catalog_index,
        &options.overlay.catalog_entry_sha256,
        "options overlay",
    )?;
    ensure!(
        read_u32(overlay_catalog_entry, 4, "options catalog byte count")?
            == u32::try_from(options_overlay.len())?
            && read_u32(overlay_catalog_entry, 8, "options catalog entrypoint")?
                == overlay_entrypoint,
        "MODE SELECT options catalog record changed"
    );

    let overlay_loader = parse_address(&options.overlay_loader_address, "options overlay loader")?;
    let overlay_load_call = parse_address(
        &options.overlay_load_call_address,
        "options overlay load call",
    )?;
    let overlay_catalog_index_address = parse_address(
        &options.overlay_catalog_index_address,
        "options overlay catalog index",
    )?;
    let entry_pointer_address = parse_address(
        &options.overlay_entry_pointer_address,
        "options entry-pointer address",
    )?;
    let entry_pointer_load = parse_address(
        &options.overlay_entry_pointer_load_address,
        "options entry-pointer load",
    )?;
    let entry_call = parse_address(
        &options.overlay_entry_call_address,
        "options overlay entry call",
    )?;
    ensure!(
        overlay_load_call == setup_entrypoint + 0x10
            && overlay_catalog_index_address == overlay_load_call + 4,
        "MODE SELECT options setup layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                overlay_load_call,
                Instruction::Jal {
                    target: overlay_loader,
                },
            ),
            (
                overlay_catalog_index_address,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: options.overlay.catalog_index,
                },
            ),
            (
                entry_pointer_load,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (entry_pointer_address >> 16) as u16,
                },
            ),
            (
                entry_pointer_load + 4,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: entry_pointer_address as u16 as i16,
                },
            ),
            (
                entry_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
        ],
        "MODE SELECT options overlay load",
    )?;

    ensure!(
        options.information.path == OPTIONS_INFO_RECORD.path
            && options.information.stored_sha256 == OPTIONS_INFO_RECORD.stored_sha256
            && options.information.decoded_sha256 == OPTIONS_INFO_RECORD.decoded_sha256
            && sha256_bytes(options_information_stored) == OPTIONS_INFO_RECORD.stored_sha256,
        "MODE SELECT options-information source binding changed"
    );
    let information_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        options.information.catalog_index,
        &options.information.catalog_entry_sha256,
        "options information",
    )?;
    ensure!(
        read_u32(
            information_catalog_entry,
            4,
            "options-information catalog byte count"
        )? == u32::try_from(options_information_stored.len())?,
        "MODE SELECT options-information catalog record changed"
    );
    let information_destination = parse_address(
        &options.information_load_destination_address,
        "options-information load destination",
    )?;
    let information_destination_high = parse_address(
        &options.information_load_destination_high_address,
        "options-information destination high setup",
    )?;
    let information_destination_low = parse_address(
        &options.information_load_destination_low_address,
        "options-information destination low setup",
    )?;
    let information_load_call = parse_address(
        &options.information_load_call_address,
        "options-information load call",
    )?;
    let information_catalog_index_address = parse_address(
        &options.information_catalog_index_address,
        "options-information catalog index",
    )?;
    validate_instructions(
        options_overlay,
        overlay_runtime_base,
        &[
            (
                information_destination_high,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (information_destination >> 16) as u16,
                },
            ),
            (
                information_destination_low,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: information_destination as u16,
                },
            ),
            (
                information_load_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
            (
                information_catalog_index_address,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: options.information.catalog_index,
                },
            ),
        ],
        "MODE SELECT options-information load",
    )
}

fn validate_records_entry(
    inventory: &SurfaceInventory,
    records: &RecordsEntryDocument,
    main_executable: &[u8],
) -> Result<()> {
    validate_resolved_surface(
        inventory,
        &records.surface_id,
        "records/main-executable-renderer",
        &[
            (MENU_RECORD.path, SurfaceTargetLayer::DecodedRecord),
            (MAIN_EXECUTABLE_RECORD.path, SurfaceTargetLayer::IsoRecord),
        ],
    )?;
    let setup_entrypoint = parse_address(&records.setup_entrypoint, "records setup entrypoint")?;
    ensure!(
        records.surface_id == "mode-select/entry/records"
            && setup_entrypoint == mode_select_setup_entrypoint(inventory, "records")?,
        "MODE SELECT records binding no longer matches its dispatcher entry"
    );
    let native_setup_call = parse_address(
        &records.native_setup_call_address,
        "records native setup call",
    )?;
    let native_setup = parse_address(&records.native_setup_address, "records native setup")?;
    let interactive_loop_call = parse_address(
        &records.interactive_loop_call_address,
        "records interactive-loop call",
    )?;
    let interactive_loop = parse_address(
        &records.interactive_loop_address,
        "records interactive loop",
    )?;
    let render_call = parse_address(&records.render_call_address, "records render call")?;
    let render = parse_address(&records.render_address, "records renderer")?;
    ensure!(
        native_setup_call >= setup_entrypoint && interactive_loop_call > native_setup_call,
        "MODE SELECT records setup layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                native_setup_call,
                Instruction::Jal {
                    target: native_setup,
                },
            ),
            (
                native_setup_call + 4,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: records.native_setup_argument,
                },
            ),
            (
                interactive_loop_call,
                Instruction::Jal {
                    target: interactive_loop,
                },
            ),
            (render_call, Instruction::Jal { target: render }),
        ],
        "MODE SELECT records resident consumer",
    )
}

fn validate_resolved_surface(
    inventory: &SurfaceInventory,
    surface_id: &str,
    expected_consumer_class: &str,
    expected_targets: &[(&str, SurfaceTargetLayer)],
) -> Result<()> {
    let node = inventory
        .nodes
        .iter()
        .find(|node| node.id == surface_id)
        .with_context(|| format!("surface inventory is missing {surface_id}"))?;
    ensure!(
        node.resolution == SurfaceResolution::Resolved,
        "surface inventory {surface_id} is not resolved"
    );
    ensure!(
        node.consumer_class.as_deref() == Some(expected_consumer_class),
        "surface inventory {surface_id} consumer class changed"
    );
    let targets = target_paths_and_layers(&node.target_objects);
    let expected = expected_targets.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        targets == expected,
        "surface inventory {surface_id} target objects differ from its source-bound consumer"
    );
    Ok(())
}

fn target_paths_and_layers(
    targets: &[SurfaceTargetObject],
) -> BTreeSet<(&str, SurfaceTargetLayer)> {
    targets
        .iter()
        .map(|target| (target.path.as_str(), target.layer))
        .collect()
}

fn catalog_entry<'a>(
    main_executable: &'a [u8],
    source: &MainExecutableDocument,
    catalog_index: i16,
    expected_sha256: &str,
    role: &str,
) -> Result<&'a [u8]> {
    ensure!(catalog_index >= 0, "{role} catalog index is negative");
    let table_offset = parse_offset(&source.catalog_table_offset, "MODE SELECT catalog table")?;
    let entry_offset = usize::try_from(catalog_index)?
        .checked_mul(source.catalog_entry_size)
        .and_then(|offset| table_offset.checked_add(offset))
        .context("MODE SELECT catalog entry offset overflow")?;
    let entry_end = entry_offset
        .checked_add(source.catalog_entry_size)
        .context("MODE SELECT catalog entry range overflow")?;
    let entry = main_executable
        .get(entry_offset..entry_end)
        .with_context(|| format!("{role} catalog entry leaves the main executable"))?;
    ensure!(
        sha256_bytes(entry) == expected_sha256,
        "{role} catalog entry changed"
    );
    Ok(entry)
}

fn validate_main_instructions(
    main_executable: &[u8],
    expected: &[(u32, Instruction)],
    role: &str,
) -> Result<()> {
    validate_instructions(
        main_executable_text(main_executable)?,
        MAIN_TEXT_RUNTIME_BASE,
        expected,
        role,
    )
}

fn validate_instructions(
    data: &[u8],
    runtime_base: u32,
    expected: &[(u32, Instruction)],
    role: &str,
) -> Result<()> {
    for (address, expected_instruction) in expected {
        let offset = address
            .checked_sub(runtime_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .with_context(|| format!("{role} address precedes its runtime base"))?;
        let bytes: [u8; 4] = data
            .get(offset..offset + 4)
            .with_context(|| format!("{role} instruction is outside its source record"))?
            .try_into()?;
        let instruction = decode(u32::from_le_bytes(bytes), *address)
            .with_context(|| format!("failed to decode {role} at {address:#010x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "{role} changed at {address:#010x}"
        );
    }
    Ok(())
}

fn read_u32(data: &[u8], offset: usize, role: &str) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("{role} is truncated"))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
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
        "{role} is not an eight-digit lowercase address"
    );
    u32::from_str_radix(digits, 16).with_context(|| format!("failed to parse {role}"))
}

fn parse_offset(value: &str, role: &str) -> Result<usize> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("{role} is not a hexadecimal offset"))?;
    ensure!(
        !digits.is_empty()
            && digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{role} is not a lowercase hexadecimal offset"
    );
    usize::from_str_radix(digits, 16).with_context(|| format!("failed to parse {role}"))
}
