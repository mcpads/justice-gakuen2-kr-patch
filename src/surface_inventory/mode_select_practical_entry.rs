use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::Deserialize;

use super::mode_select_dispatcher::{mode_select_panel_index, mode_select_setup_entrypoint};
use super::model::{SurfaceInventory, SurfaceResolution, SurfaceTargetLayer, SurfaceTargetObject};
use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    MAIN_TEXT_RUNTIME_BASE, main_executable_text,
    profile::{
        CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD, CHARACTER_SELECT_SELP1_TEXTURE_RECORD,
        MAIN_EXECUTABLE_RECORD,
    },
};

const BINDING_KIND: &str = "justice_gakuen2_mode_select_practical_entry_binding";
pub(super) const BINDING_FILE: &str = "practical-entry.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PracticalEntryBindingDocument {
    kind: String,
    main_executable: MainExecutableDocument,
    entry: PracticalEntryDocument,
    overlay: OverlayDocument,
    texture: TextureDocument,
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
struct PracticalEntryDocument {
    surface_id: String,
    selection_asset_id: String,
    setup_entrypoint: String,
    mode_state_value: usize,
    overlay_loader_address: String,
    overlay_load_call_address: String,
    overlay_catalog_index_address: String,
    overlay_entry_pointer_address: String,
    overlay_entry_pointer_load_address: String,
    overlay_entry_call_address: String,
    phase_state_address: String,
    phase_state_clear_address: String,
    phase_state_load_address: String,
    interactive_phase_limit: i16,
    interactive_loop_branch_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayDocument {
    path: String,
    sha256: String,
    entrypoint: String,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureDocument {
    path: String,
    stored_sha256: String,
    decoded_sha256: String,
    decoded_size: usize,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

pub(super) fn validate_mode_select_practical_entry(
    inventory: &SurfaceInventory,
    main_executable: &[u8],
    primary_overlay: &[u8],
    primary_texture_stored: &[u8],
) -> Result<()> {
    let binding = load_binding(inventory)?;
    validate_main_executable(&binding.main_executable, main_executable)?;
    validate_surface(inventory, &binding.entry)?;
    validate_source_records(
        &binding,
        main_executable,
        primary_overlay,
        primary_texture_stored,
    )?;
    validate_direct_entry_route(inventory, &binding, main_executable)
}

fn load_binding(inventory: &SurfaceInventory) -> Result<PracticalEntryBindingDocument> {
    let binding_bytes = inventory
        .binding_files
        .get(BINDING_FILE)
        .with_context(|| format!("MODE SELECT surface inventory is missing {BINDING_FILE}"))?;
    let binding: PracticalEntryBindingDocument = serde_json::from_slice(binding_bytes)
        .with_context(|| format!("failed to parse MODE SELECT binding {BINDING_FILE}"))?;
    ensure!(
        binding.kind == BINDING_KIND,
        "unsupported MODE SELECT practical-entry binding kind {:?}",
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
        "MODE SELECT practical-entry main-executable source changed"
    );
    ensure!(
        parse_address(&source.runtime_base, "main executable runtime base")?
            == MAIN_TEXT_RUNTIME_BASE
            && source.catalog_entry_size == 12,
        "MODE SELECT practical-entry main-executable layout changed"
    );
    parse_offset(
        &source.catalog_table_offset,
        "MODE SELECT source catalog table",
    )?;
    Ok(())
}

fn validate_surface(inventory: &SurfaceInventory, entry: &PracticalEntryDocument) -> Result<()> {
    ensure!(
        entry.surface_id == "mode-select/entry/practical-exam-99"
            && entry.selection_asset_id == "practical_exam_99",
        "MODE SELECT practical-entry identity changed"
    );
    let node = inventory
        .nodes
        .iter()
        .find(|node| node.id == entry.surface_id)
        .context("surface inventory is missing the practical-exam entry")?;
    ensure!(
        node.resolution == SurfaceResolution::Resolved
            && node.consumer_class.as_deref() == Some("character-select/plsel1-renderer"),
        "MODE SELECT practical-exam consumer binding changed"
    );
    let targets = target_paths_and_layers(&node.target_objects);
    let expected = BTreeSet::from([
        (
            CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.path,
            SurfaceTargetLayer::IsoRecord,
        ),
        (
            CHARACTER_SELECT_SELP1_TEXTURE_RECORD.path,
            SurfaceTargetLayer::DecodedRecord,
        ),
    ]);
    ensure!(
        targets == expected,
        "MODE SELECT practical-exam target objects changed"
    );
    Ok(())
}

fn validate_source_records(
    binding: &PracticalEntryBindingDocument,
    main_executable: &[u8],
    primary_overlay: &[u8],
    primary_texture_stored: &[u8],
) -> Result<()> {
    let overlay = &binding.overlay;
    let overlay_entrypoint = parse_address(&overlay.entrypoint, "PLSEL1 entrypoint")?;
    ensure!(
        overlay.path == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.path
            && overlay.sha256 == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.sha256
            && primary_overlay.len() == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.size
            && sha256_bytes(primary_overlay) == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.sha256
            && read_u32(primary_overlay, 0, "PLSEL1 entry pointer")? == overlay_entrypoint,
        "MODE SELECT practical-entry PLSEL1 source changed"
    );
    let overlay_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        overlay.catalog_index,
        &overlay.catalog_entry_sha256,
        "practical-entry PLSEL1 overlay",
    )?;
    ensure!(
        read_u32(overlay_catalog_entry, 4, "PLSEL1 catalog byte count")?
            == u32::try_from(primary_overlay.len())?
            && read_u32(overlay_catalog_entry, 8, "PLSEL1 catalog entrypoint")?
                == overlay_entrypoint,
        "MODE SELECT practical-entry PLSEL1 catalog record changed"
    );

    let texture = &binding.texture;
    ensure!(
        texture.path == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.path
            && texture.stored_sha256 == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.stored_sha256
            && texture.decoded_sha256 == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_sha256
            && texture.decoded_size == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_size
            && sha256_bytes(primary_texture_stored)
                == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.stored_sha256,
        "MODE SELECT practical-entry SELP1 source changed"
    );
    let decoded = decompress(primary_texture_stored, false)?;
    ensure!(
        decoded.len() == texture.decoded_size && sha256_bytes(&decoded) == texture.decoded_sha256,
        "MODE SELECT practical-entry SELP1 decoded source changed"
    );
    let texture_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        texture.catalog_index,
        &texture.catalog_entry_sha256,
        "practical-entry SELP1 texture",
    )?;
    ensure!(
        read_u32(texture_catalog_entry, 4, "SELP1 catalog byte count")?
            == u32::try_from(primary_texture_stored.len())?,
        "MODE SELECT practical-entry SELP1 catalog record changed"
    );
    Ok(())
}

fn validate_direct_entry_route(
    inventory: &SurfaceInventory,
    binding: &PracticalEntryBindingDocument,
    main_executable: &[u8],
) -> Result<()> {
    let entry = &binding.entry;
    let setup = parse_address(&entry.setup_entrypoint, "practical-exam setup entrypoint")?;
    ensure!(
        setup == mode_select_setup_entrypoint(inventory, &entry.selection_asset_id)?
            && entry.mode_state_value
                == mode_select_panel_index(inventory, &entry.selection_asset_id)?,
        "MODE SELECT practical entry no longer matches dispatcher"
    );
    validate_direct_entry_route_for_inventory(binding, main_executable, setup)
}

fn validate_direct_entry_route_for_inventory(
    binding: &PracticalEntryBindingDocument,
    main_executable: &[u8],
    setup: u32,
) -> Result<()> {
    let entry = &binding.entry;
    let overlay_loader = parse_address(&entry.overlay_loader_address, "practical overlay loader")?;
    let overlay_load_call = parse_address(
        &entry.overlay_load_call_address,
        "practical overlay load call",
    )?;
    let catalog_index_address = parse_address(
        &entry.overlay_catalog_index_address,
        "practical overlay catalog-index setup",
    )?;
    let entry_pointer = parse_address(
        &entry.overlay_entry_pointer_address,
        "practical overlay entry pointer",
    )?;
    let entry_pointer_load = parse_address(
        &entry.overlay_entry_pointer_load_address,
        "practical overlay entry-pointer load",
    )?;
    let entry_call = parse_address(
        &entry.overlay_entry_call_address,
        "practical overlay entry call",
    )?;
    let phase_state = parse_address(&entry.phase_state_address, "practical phase state")?;
    let phase_clear = parse_address(
        &entry.phase_state_clear_address,
        "practical phase-state clear",
    )?;
    let phase_load = parse_address(
        &entry.phase_state_load_address,
        "practical phase-state load",
    )?;
    let loop_branch = parse_address(
        &entry.interactive_loop_branch_address,
        "practical interactive loop branch",
    )?;
    ensure!(
        setup == 0x8001_7564
            && overlay_load_call == setup + 0x94
            && catalog_index_address == overlay_load_call + 4
            && phase_clear == catalog_index_address + 0x10
            && entry_pointer_load == setup + 0xc8
            && entry_call == entry_pointer_load + 0x0c
            && phase_load == entry_call + 0x14
            && loop_branch == phase_load + 0x0c
            && entry.interactive_phase_limit == 2,
        "MODE SELECT practical direct-entry layout changed"
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
                catalog_index_address,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: binding.overlay.catalog_index,
                },
            ),
            (
                phase_clear,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: (phase_state >> 16) as u16,
                },
            ),
            (
                phase_clear + 4,
                Instruction::Sb {
                    rt: Register::ZERO,
                    base: Register::AT,
                    offset: phase_state as u16 as i16,
                },
            ),
            (
                entry_pointer_load,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (entry_pointer >> 16) as u16,
                },
            ),
            (
                entry_pointer_load + 4,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: entry_pointer as u16 as i16,
                },
            ),
            (
                entry_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
            (
                phase_load - 4,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (phase_state >> 16) as u16,
                },
            ),
            (
                phase_load,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: phase_state as u16 as i16,
                },
            ),
            (
                phase_load + 8,
                Instruction::Sltiu {
                    rt: Register::V0,
                    rs: Register::V0,
                    immediate: entry.interactive_phase_limit,
                },
            ),
            (
                loop_branch,
                Instruction::Bne {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: entry_pointer_load,
                },
            ),
        ],
        "MODE SELECT practical direct-entry route",
    )
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
    let text = main_executable_text(main_executable)?;
    for (address, expected_instruction) in expected {
        let offset = address
            .checked_sub(MAIN_TEXT_RUNTIME_BASE)
            .and_then(|offset| usize::try_from(offset).ok())
            .with_context(|| format!("{role} address precedes the main executable"))?;
        let bytes: [u8; 4] = text
            .get(offset..offset + 4)
            .with_context(|| format!("{role} instruction is outside the main executable"))?
            .try_into()?;
        let instruction = decode(u32::from_le_bytes(bytes), *address)
            .with_context(|| format!("failed to decode {role} at {address:#010x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "{role} changed at {address:#010x}: expected {expected_instruction:?}, found {instruction:?}"
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
