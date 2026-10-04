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
        BONUS_MENU_OVERLAY_RECORD, BONUS_MENU_RECORD, CompressedSourceRecordProfile,
        EDIT_REGISTRATION_OVERLAY_RECORD, EDIT_REGISTRATION_UI_RECORD, GORIN_MAIN_MENU_RECORD,
        GORIN_SELECTION_OVERLAY_RECORD, MAIN_EXECUTABLE_RECORD, MENU_RECORD, SourceRecordProfile,
        TITLE_MENU_OVERLAY_RECORD,
    },
};

const BINDING_KIND: &str = "justice_gakuen2_mode_select_direct_entry_routes_binding";
pub(super) const BINDING_FILE: &str = "direct-entry-routes.json";

pub(crate) struct ModeSelectDirectEntrySources<'a> {
    pub(crate) diary_overlay_stored: &'a [u8],
    pub(crate) menu_stored: &'a [u8],
    pub(crate) gorin_overlay: &'a [u8],
    pub(crate) gorin_menu_stored: &'a [u8],
    pub(crate) edit_overlay: &'a [u8],
    pub(crate) edit_ui_stored: &'a [u8],
    pub(crate) bonus_overlay: &'a [u8],
    pub(crate) bonus_menu_stored: &'a [u8],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectEntryRoutesDocument {
    kind: String,
    main_executable: MainExecutableDocument,
    diary: DiaryEntryDocument,
    gorin_festival: GorinEntryDocument,
    edit_registration: OverlayDrivenEntryDocument,
    bonus: OverlayDrivenEntryDocument,
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
struct DiaryEntryDocument {
    surface_id: String,
    selection_asset_id: String,
    setup_entrypoint: String,
    mode_state_value: usize,
    callback_entrypoint: String,
    callback_high_address: String,
    callback_low_address: String,
    callback_registration_address: String,
    callback_registration_call_address: String,
    callback_slot: i16,
    scheduler_address: String,
    scheduler_call_address: String,
    overlay_loader_address: String,
    overlay_load_call_address: String,
    overlay_catalog_index_address: String,
    overlay_entry_pointer_address: String,
    overlay_entry_pointer_load_address: String,
    overlay_entry_call_address: String,
    overlay: ExecutableCompressedRecordDocument,
    menu: CompressedRecordDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GorinEntryDocument {
    surface_id: String,
    selection_asset_id: String,
    setup_entrypoint: String,
    mode_state_value: usize,
    overlay_loader_address: String,
    overlay_load_call_address: String,
    overlay_catalog_index_address: String,
    texture_destination_address: String,
    texture_destination_high_address: String,
    texture_destination_low_address: String,
    texture_loader_address: String,
    texture_load_call_address: String,
    texture_catalog_index_address: String,
    texture_parser_address: String,
    texture_parser_call_addresses: Vec<String>,
    secondary_entry_pointer_address: String,
    secondary_entry_pointer_load_address: String,
    secondary_entry_call_address: String,
    primary_entry_pointer_address: String,
    primary_entry_pointer_load_address: String,
    primary_entry_call_address: String,
    overlay: RawRecordDocument,
    texture: CompressedRecordDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayDrivenEntryDocument {
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
    overlay: RawRecordDocument,
    texture_load: ImportedTextureLoadDocument,
    texture: CompressedRecordDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecordDocument {
    path: String,
    sha256: String,
    size: usize,
    runtime_base: String,
    entrypoints: Vec<String>,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutableCompressedRecordDocument {
    path: String,
    stored_sha256: String,
    decoded_sha256: String,
    decoded_size: usize,
    decoded_entrypoint: String,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompressedRecordDocument {
    path: String,
    stored_sha256: String,
    decoded_sha256: String,
    decoded_size: usize,
    catalog_index: i16,
    catalog_entry_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedTextureLoadDocument {
    destination_address: String,
    destination_high_address: String,
    destination_low_address: String,
    loader_table_address: String,
    loader_table_high_address: String,
    loader_table_load_address: String,
    loader_method_offset: i16,
    loader_method_load_address: String,
    loader_call_address: String,
    catalog_index_address: String,
    parser_table_address: String,
    parser_table_high_address: String,
    parser_table_load_address: String,
    parser_method_offset: i16,
    parser_method_load_address: String,
    parser_destination_high_address: String,
    parser_destination_low_address: String,
    parser_call_address: String,
}

pub(super) fn validate_mode_select_direct_entry_routes(
    inventory: &SurfaceInventory,
    main_executable: &[u8],
    sources: &ModeSelectDirectEntrySources<'_>,
) -> Result<()> {
    let binding = load_binding(inventory)?;
    validate_main_executable(&binding.main_executable, main_executable)?;
    validate_diary_entry(inventory, &binding, main_executable, sources)?;
    validate_gorin_entry(inventory, &binding, main_executable, sources)?;
    validate_overlay_driven_entry(
        inventory,
        &binding,
        main_executable,
        &binding.edit_registration,
        "edit-registration/kanri-renderer",
        &EDIT_REGISTRATION_OVERLAY_RECORD,
        &EDIT_REGISTRATION_UI_RECORD,
        sources.edit_overlay,
        sources.edit_ui_stored,
        0x8001_8070,
        0x8001_8090,
        0x8001_8098,
        0x8001_80a4,
        0x800a_36c8,
        0x800a_36e0,
        0x800a_36fc,
        &[0x800a_364c],
        "EDIT registration",
    )?;
    validate_overlay_driven_entry(
        inventory,
        &binding,
        main_executable,
        &binding.bonus,
        "bonus/koubai-main-menu-renderer",
        &BONUS_MENU_OVERLAY_RECORD,
        &BONUS_MENU_RECORD,
        sources.bonus_overlay,
        sources.bonus_menu_stored,
        0x8001_ec24,
        0x8001_ecb4,
        0x8001_ed60,
        0x8001_ed6c,
        0x800a_5f34,
        0x800a_5f4c,
        0x800a_5f68,
        &[0x800a_5c18],
        "bonus main menu",
    )
}

fn load_binding(inventory: &SurfaceInventory) -> Result<DirectEntryRoutesDocument> {
    let bytes = inventory
        .binding_files
        .get(BINDING_FILE)
        .with_context(|| format!("MODE SELECT surface inventory is missing {BINDING_FILE}"))?;
    let binding: DirectEntryRoutesDocument = serde_json::from_slice(bytes)
        .with_context(|| format!("failed to parse MODE SELECT binding {BINDING_FILE}"))?;
    ensure!(
        binding.kind == BINDING_KIND,
        "unsupported MODE SELECT direct-entry binding kind {:?}",
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
        "MODE SELECT direct-entry main-executable source changed"
    );
    ensure!(
        parse_address(&source.runtime_base, "main executable runtime base")?
            == MAIN_TEXT_RUNTIME_BASE
            && source.catalog_entry_size == 12
            && parse_offset(
                &source.catalog_table_offset,
                "MODE SELECT source catalog table",
            )? == 0x76ee8,
        "MODE SELECT direct-entry main-executable layout changed"
    );
    Ok(())
}

fn validate_diary_entry(
    inventory: &SurfaceInventory,
    binding: &DirectEntryRoutesDocument,
    main_executable: &[u8],
    sources: &ModeSelectDirectEntrySources<'_>,
) -> Result<()> {
    let entry = &binding.diary;
    ensure!(
        entry.surface_id == "mode-select/entry/diary" && entry.selection_asset_id == "diary",
        "MODE SELECT diary entry identity changed"
    );
    validate_surface(
        inventory,
        &entry.surface_id,
        "title-adjacent/mgtit-renderer",
        &[
            (
                TITLE_MENU_OVERLAY_RECORD.path,
                SurfaceTargetLayer::DecodedRecord,
            ),
            (MENU_RECORD.path, SurfaceTargetLayer::DecodedRecord),
        ],
    )?;
    validate_dispatcher_identity(
        inventory,
        &entry.selection_asset_id,
        &entry.setup_entrypoint,
        entry.mode_state_value,
    )?;
    validate_executable_compressed_record(
        &binding.main_executable,
        &entry.overlay,
        &TITLE_MENU_OVERLAY_RECORD,
        sources.diary_overlay_stored,
        main_executable,
        0x800a_3b60,
        "diary MGTIT overlay",
    )?;
    validate_compressed_record(
        &binding.main_executable,
        &entry.menu,
        &MENU_RECORD,
        sources.menu_stored,
        main_executable,
        "diary MENU atlas",
    )?;

    let setup = parse_address(&entry.setup_entrypoint, "diary setup entrypoint")?;
    let callback = parse_address(&entry.callback_entrypoint, "diary callback entrypoint")?;
    let callback_high = parse_address(&entry.callback_high_address, "diary callback high load")?;
    let callback_low = parse_address(&entry.callback_low_address, "diary callback low load")?;
    let registration = parse_address(
        &entry.callback_registration_address,
        "diary callback registration",
    )?;
    let registration_call = parse_address(
        &entry.callback_registration_call_address,
        "diary callback registration call",
    )?;
    let scheduler = parse_address(&entry.scheduler_address, "diary scheduler")?;
    let scheduler_call = parse_address(&entry.scheduler_call_address, "diary scheduler call")?;
    let overlay_loader = parse_address(&entry.overlay_loader_address, "diary overlay loader")?;
    let overlay_load_call =
        parse_address(&entry.overlay_load_call_address, "diary overlay load call")?;
    let overlay_catalog_index = parse_address(
        &entry.overlay_catalog_index_address,
        "diary overlay catalog index",
    )?;
    let entry_pointer = parse_address(
        &entry.overlay_entry_pointer_address,
        "diary overlay entry pointer",
    )?;
    let entry_pointer_load = parse_address(
        &entry.overlay_entry_pointer_load_address,
        "diary overlay entry-pointer load",
    )?;
    let entry_call = parse_address(
        &entry.overlay_entry_call_address,
        "diary overlay entry call",
    )?;
    ensure!(
        setup == 0x8001_7798
            && callback == 0x8002_675c
            && callback_high == setup + 0x60
            && callback_low == callback_high + 4
            && registration_call == callback_low + 4
            && scheduler_call == registration_call + 8
            && overlay_load_call == 0x8002_684c
            && overlay_catalog_index == overlay_load_call + 4
            && entry_pointer_load == 0x8002_6a00
            && entry_call == entry_pointer_load + 0x0c
            && entry.callback_slot == 1,
        "MODE SELECT diary direct-entry layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                callback_high,
                Instruction::Lui {
                    rt: Register::A1,
                    immediate: (callback >> 16) as u16,
                },
            ),
            (
                callback_low,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::A1,
                    immediate: callback as u16 as i16,
                },
            ),
            (
                registration_call,
                Instruction::Jal {
                    target: registration,
                },
            ),
            (
                registration_call + 4,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: entry.callback_slot,
                },
            ),
            (scheduler_call, Instruction::Jal { target: scheduler }),
            (
                overlay_load_call,
                Instruction::Jal {
                    target: overlay_loader,
                },
            ),
            (
                overlay_catalog_index,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: entry.overlay.catalog_index,
                },
            ),
        ],
        "MODE SELECT diary callback and overlay load",
    )?;
    validate_main_entry_call(
        main_executable,
        entry_pointer,
        entry_pointer_load,
        entry_call,
        "MODE SELECT diary overlay entry",
    )
}

fn validate_gorin_entry(
    inventory: &SurfaceInventory,
    binding: &DirectEntryRoutesDocument,
    main_executable: &[u8],
    sources: &ModeSelectDirectEntrySources<'_>,
) -> Result<()> {
    let entry = &binding.gorin_festival;
    ensure!(
        entry.surface_id == "mode-select/entry/gorin-festival"
            && entry.selection_asset_id == "gorin_festival",
        "MODE SELECT Gorin-festival entry identity changed"
    );
    validate_surface(
        inventory,
        &entry.surface_id,
        "gorin-festival/minisel-renderer",
        &[
            (
                GORIN_SELECTION_OVERLAY_RECORD.path,
                SurfaceTargetLayer::IsoRecord,
            ),
            (
                GORIN_MAIN_MENU_RECORD.path,
                SurfaceTargetLayer::DecodedRecord,
            ),
        ],
    )?;
    validate_dispatcher_identity(
        inventory,
        &entry.selection_asset_id,
        &entry.setup_entrypoint,
        entry.mode_state_value,
    )?;
    validate_raw_record(
        &binding.main_executable,
        &entry.overlay,
        &GORIN_SELECTION_OVERLAY_RECORD,
        sources.gorin_overlay,
        main_executable,
        &[0x800a_2aa8, 0x800a_3554],
        "Gorin MINISEL overlay",
    )?;
    validate_compressed_record(
        &binding.main_executable,
        &entry.texture,
        &GORIN_MAIN_MENU_RECORD,
        sources.gorin_menu_stored,
        main_executable,
        "Gorin MINITTL0 texture",
    )?;

    let setup = parse_address(&entry.setup_entrypoint, "Gorin setup entrypoint")?;
    let overlay_loader = parse_address(&entry.overlay_loader_address, "Gorin overlay loader")?;
    let overlay_load_call =
        parse_address(&entry.overlay_load_call_address, "Gorin overlay load call")?;
    let overlay_index = parse_address(
        &entry.overlay_catalog_index_address,
        "Gorin overlay catalog index",
    )?;
    let destination = parse_address(
        &entry.texture_destination_address,
        "Gorin texture destination",
    )?;
    let destination_high = parse_address(
        &entry.texture_destination_high_address,
        "Gorin texture destination high load",
    )?;
    let destination_low = parse_address(
        &entry.texture_destination_low_address,
        "Gorin texture destination low load",
    )?;
    let texture_loader = parse_address(&entry.texture_loader_address, "Gorin texture loader")?;
    let texture_load_call =
        parse_address(&entry.texture_load_call_address, "Gorin texture load call")?;
    let texture_index = parse_address(
        &entry.texture_catalog_index_address,
        "Gorin texture catalog index",
    )?;
    let parser = parse_address(&entry.texture_parser_address, "Gorin texture parser")?;
    let parser_calls = entry
        .texture_parser_call_addresses
        .iter()
        .map(|address| parse_address(address, "Gorin texture parser call"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        setup == 0x8001_7c4c
            && overlay_load_call == 0x8001_7d60
            && overlay_index == overlay_load_call + 4
            && destination == 0x800d_4000
            && destination_high == 0x8001_7d84
            && destination_low == destination_high + 4
            && texture_load_call == 0x8001_7d8c
            && texture_index == texture_load_call + 4
            && parser_calls == [0x8001_7d98, 0x8001_7da4],
        "MODE SELECT Gorin direct-entry layout changed"
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
                overlay_index,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: entry.overlay.catalog_index,
                },
            ),
            (
                destination_high,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (destination >> 16) as u16,
                },
            ),
            (
                destination_low,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: destination as u16,
                },
            ),
            (
                texture_load_call,
                Instruction::Jal {
                    target: texture_loader,
                },
            ),
            (
                texture_index,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: entry.texture.catalog_index,
                },
            ),
            (parser_calls[0], Instruction::Jal { target: parser }),
            (parser_calls[1], Instruction::Jal { target: parser }),
        ],
        "MODE SELECT Gorin overlay and texture load",
    )?;
    validate_main_entry_call(
        main_executable,
        parse_address(
            &entry.secondary_entry_pointer_address,
            "Gorin secondary entry pointer",
        )?,
        parse_address(
            &entry.secondary_entry_pointer_load_address,
            "Gorin secondary entry-pointer load",
        )?,
        parse_address(
            &entry.secondary_entry_call_address,
            "Gorin secondary entry call",
        )?,
        "MODE SELECT Gorin secondary overlay entry",
    )?;
    validate_main_entry_call(
        main_executable,
        parse_address(
            &entry.primary_entry_pointer_address,
            "Gorin primary entry pointer",
        )?,
        parse_address(
            &entry.primary_entry_pointer_load_address,
            "Gorin primary entry-pointer load",
        )?,
        parse_address(
            &entry.primary_entry_call_address,
            "Gorin primary entry call",
        )?,
        "MODE SELECT Gorin primary overlay entry",
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_overlay_driven_entry(
    inventory: &SurfaceInventory,
    binding: &DirectEntryRoutesDocument,
    main_executable: &[u8],
    entry: &OverlayDrivenEntryDocument,
    consumer_class: &str,
    overlay_profile: &SourceRecordProfile,
    texture_profile: &CompressedSourceRecordProfile,
    overlay: &[u8],
    texture_stored: &[u8],
    expected_setup: u32,
    expected_overlay_load_call: u32,
    expected_entry_pointer_load: u32,
    expected_entry_call: u32,
    expected_texture_load_start: u32,
    expected_texture_load_call: u32,
    expected_texture_parser_call: u32,
    expected_entrypoints: &[u32],
    role: &str,
) -> Result<()> {
    validate_surface(
        inventory,
        &entry.surface_id,
        consumer_class,
        &[
            (overlay_profile.path, SurfaceTargetLayer::IsoRecord),
            (texture_profile.path, SurfaceTargetLayer::DecodedRecord),
        ],
    )?;
    validate_dispatcher_identity(
        inventory,
        &entry.selection_asset_id,
        &entry.setup_entrypoint,
        entry.mode_state_value,
    )?;
    validate_raw_record(
        &binding.main_executable,
        &entry.overlay,
        overlay_profile,
        overlay,
        main_executable,
        expected_entrypoints,
        &format!("{role} overlay"),
    )?;
    validate_compressed_record(
        &binding.main_executable,
        &entry.texture,
        texture_profile,
        texture_stored,
        main_executable,
        &format!("{role} texture"),
    )?;

    let setup = parse_address(&entry.setup_entrypoint, &format!("{role} setup entrypoint"))?;
    let overlay_loader = parse_address(
        &entry.overlay_loader_address,
        &format!("{role} overlay loader"),
    )?;
    let overlay_load_call = parse_address(
        &entry.overlay_load_call_address,
        &format!("{role} overlay load call"),
    )?;
    let overlay_index = parse_address(
        &entry.overlay_catalog_index_address,
        &format!("{role} overlay catalog index"),
    )?;
    let entry_pointer = parse_address(
        &entry.overlay_entry_pointer_address,
        &format!("{role} overlay entry pointer"),
    )?;
    let entry_pointer_load = parse_address(
        &entry.overlay_entry_pointer_load_address,
        &format!("{role} overlay entry-pointer load"),
    )?;
    let entry_call = parse_address(
        &entry.overlay_entry_call_address,
        &format!("{role} overlay entry call"),
    )?;
    ensure!(
        setup == expected_setup
            && overlay_load_call == expected_overlay_load_call
            && overlay_index == overlay_load_call + 4
            && entry_pointer_load == expected_entry_pointer_load
            && entry_call == expected_entry_call,
        "MODE SELECT {role} main-executable route layout changed"
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
                overlay_index,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: entry.overlay.catalog_index,
                },
            ),
        ],
        &format!("MODE SELECT {role} overlay load"),
    )?;
    validate_main_entry_call(
        main_executable,
        entry_pointer,
        entry_pointer_load,
        entry_call,
        &format!("MODE SELECT {role} overlay entry"),
    )?;
    validate_imported_texture_load(
        &entry.texture_load,
        entry.texture.catalog_index,
        overlay,
        parse_address(&entry.overlay.runtime_base, &format!("{role} runtime base"))?,
        expected_texture_load_start,
        expected_texture_load_call,
        expected_texture_parser_call,
        role,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_imported_texture_load(
    load: &ImportedTextureLoadDocument,
    catalog_index: i16,
    overlay: &[u8],
    runtime_base: u32,
    expected_start: u32,
    expected_loader_call: u32,
    expected_parser_call: u32,
    role: &str,
) -> Result<()> {
    let destination = parse_address(
        &load.destination_address,
        &format!("{role} texture destination"),
    )?;
    let destination_high = parse_address(
        &load.destination_high_address,
        &format!("{role} texture destination high load"),
    )?;
    let destination_low = parse_address(
        &load.destination_low_address,
        &format!("{role} texture destination low load"),
    )?;
    let loader_table = parse_address(&load.loader_table_address, &format!("{role} loader table"))?;
    let loader_table_high = parse_address(
        &load.loader_table_high_address,
        &format!("{role} loader-table high load"),
    )?;
    let loader_table_load = parse_address(
        &load.loader_table_load_address,
        &format!("{role} loader-table load"),
    )?;
    let loader_method_load = parse_address(
        &load.loader_method_load_address,
        &format!("{role} loader-method load"),
    )?;
    let loader_call = parse_address(
        &load.loader_call_address,
        &format!("{role} texture-loader call"),
    )?;
    let catalog_index_address = parse_address(
        &load.catalog_index_address,
        &format!("{role} texture catalog index"),
    )?;
    let parser_table = parse_address(&load.parser_table_address, &format!("{role} parser table"))?;
    let parser_table_high = parse_address(
        &load.parser_table_high_address,
        &format!("{role} parser-table high load"),
    )?;
    let parser_table_load = parse_address(
        &load.parser_table_load_address,
        &format!("{role} parser-table load"),
    )?;
    let parser_method_load = parse_address(
        &load.parser_method_load_address,
        &format!("{role} parser-method load"),
    )?;
    let parser_destination_high = parse_address(
        &load.parser_destination_high_address,
        &format!("{role} parser destination high load"),
    )?;
    let parser_destination_low = parse_address(
        &load.parser_destination_low_address,
        &format!("{role} parser destination low load"),
    )?;
    let parser_call = parse_address(
        &load.parser_call_address,
        &format!("{role} texture-parser call"),
    )?;
    ensure!(
        runtime_base == 0x800a_2000
            && destination == 0x800d_4000
            && loader_table == 0x801f_6374
            && parser_table == 0x801f_6360
            && load.loader_method_offset == 0x14c
            && load.parser_method_offset == 0x150
            && destination_high == expected_start
            && loader_table_high == expected_start + 4
            && loader_table_load == expected_start + 8
            && destination_low == expected_start + 0x0c
            && loader_method_load == expected_start + 0x10
            && loader_call == expected_loader_call
            && catalog_index_address == loader_call + 4
            && parser_table_high == loader_call + 8
            && parser_table_load == loader_call + 0x0c
            && parser_destination_high == loader_call + 0x10
            && parser_method_load == loader_call + 0x14
            && parser_call == expected_parser_call
            && parser_destination_low == parser_call + 4,
        "MODE SELECT {role} imported texture-load layout changed"
    );
    validate_record_instructions(
        overlay,
        runtime_base,
        &[
            (
                destination_high,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (destination >> 16) as u16,
                },
            ),
            (
                loader_table_high,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (loader_table >> 16) as u16,
                },
            ),
            (
                loader_table_load,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: loader_table as u16 as i16,
                },
            ),
            (
                destination_low,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: destination as u16,
                },
            ),
            (
                loader_method_load,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: load.loader_method_offset,
                },
            ),
            (
                loader_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
            (
                catalog_index_address,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: catalog_index,
                },
            ),
            (
                parser_table_high,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (parser_table >> 16) as u16,
                },
            ),
            (
                parser_table_load,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: parser_table as u16 as i16,
                },
            ),
            (
                parser_destination_high,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (destination >> 16) as u16,
                },
            ),
            (
                parser_method_load,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: load.parser_method_offset,
                },
            ),
            (
                parser_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
            (
                parser_destination_low,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: destination as u16,
                },
            ),
        ],
        &format!("MODE SELECT {role} imported texture load"),
    )
}

fn validate_dispatcher_identity(
    inventory: &SurfaceInventory,
    selection_asset_id: &str,
    setup_entrypoint: &str,
    mode_state_value: usize,
) -> Result<()> {
    ensure!(
        parse_address(
            setup_entrypoint,
            "MODE SELECT direct-entry setup entrypoint"
        )? == mode_select_setup_entrypoint(inventory, selection_asset_id)?
            && mode_state_value == mode_select_panel_index(inventory, selection_asset_id)?,
        "MODE SELECT {selection_asset_id} direct entry no longer matches dispatcher"
    );
    Ok(())
}

fn validate_surface(
    inventory: &SurfaceInventory,
    surface_id: &str,
    consumer_class: &str,
    expected_targets: &[(&str, SurfaceTargetLayer)],
) -> Result<()> {
    let node = inventory
        .nodes
        .iter()
        .find(|node| node.id == surface_id)
        .with_context(|| format!("surface inventory is missing {surface_id}"))?;
    ensure!(
        node.resolution == SurfaceResolution::Resolved
            && node.consumer_class.as_deref() == Some(consumer_class),
        "MODE SELECT {surface_id} consumer binding changed"
    );
    let targets = target_paths_and_layers(&node.target_objects);
    let expected = expected_targets.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        targets == expected,
        "MODE SELECT {surface_id} target objects changed"
    );
    Ok(())
}

fn validate_raw_record(
    main_source: &MainExecutableDocument,
    source: &RawRecordDocument,
    profile: &SourceRecordProfile,
    bytes: &[u8],
    main_executable: &[u8],
    expected_entrypoints: &[u32],
    role: &str,
) -> Result<()> {
    ensure!(
        source.path == profile.path
            && source.sha256 == profile.sha256
            && source.size == profile.size
            && bytes.len() == profile.size
            && sha256_bytes(bytes) == profile.sha256,
        "MODE SELECT {role} source changed"
    );
    ensure!(
        parse_address(&source.runtime_base, &format!("{role} runtime base"))? == 0x800a_2000,
        "MODE SELECT {role} runtime base changed"
    );
    let entrypoints = source
        .entrypoints
        .iter()
        .map(|entrypoint| parse_address(entrypoint, &format!("{role} entrypoint")))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        entrypoints == expected_entrypoints,
        "MODE SELECT {role} entrypoint population changed"
    );
    for (index, entrypoint) in entrypoints.iter().enumerate() {
        ensure!(
            read_u32(bytes, index * 4, &format!("{role} entry pointer"))? == *entrypoint,
            "MODE SELECT {role} entry pointer changed"
        );
    }
    let catalog = catalog_entry(
        main_executable,
        main_source,
        source.catalog_index,
        &source.catalog_entry_sha256,
        role,
    )?;
    ensure!(
        read_u32(catalog, 4, &format!("{role} catalog byte count"))? == u32::try_from(bytes.len())?
            && read_u32(catalog, 8, &format!("{role} catalog entrypoint"))?
                == expected_entrypoints[0],
        "MODE SELECT {role} catalog record changed"
    );
    Ok(())
}

fn validate_executable_compressed_record(
    main_source: &MainExecutableDocument,
    source: &ExecutableCompressedRecordDocument,
    profile: &CompressedSourceRecordProfile,
    stored: &[u8],
    main_executable: &[u8],
    expected_entrypoint: u32,
    role: &str,
) -> Result<()> {
    ensure!(
        source.path == profile.path
            && source.stored_sha256 == profile.stored_sha256
            && source.decoded_sha256 == profile.decoded_sha256
            && source.decoded_size == profile.decoded_size
            && sha256_bytes(stored) == profile.stored_sha256,
        "MODE SELECT {role} stored source changed"
    );
    let decoded = decompress(stored, false)?;
    ensure!(
        decoded.len() == profile.decoded_size
            && sha256_bytes(&decoded) == profile.decoded_sha256
            && parse_address(&source.decoded_entrypoint, &format!("{role} entrypoint"))?
                == expected_entrypoint
            && read_u32(&decoded, 0, &format!("{role} entry pointer"))? == expected_entrypoint,
        "MODE SELECT {role} decoded source changed"
    );
    validate_compressed_catalog_record(
        main_source,
        source.catalog_index,
        &source.catalog_entry_sha256,
        stored,
        main_executable,
        role,
    )
}

fn validate_compressed_record(
    main_source: &MainExecutableDocument,
    source: &CompressedRecordDocument,
    profile: &CompressedSourceRecordProfile,
    stored: &[u8],
    main_executable: &[u8],
    role: &str,
) -> Result<()> {
    ensure!(
        source.path == profile.path
            && source.stored_sha256 == profile.stored_sha256
            && source.decoded_sha256 == profile.decoded_sha256
            && source.decoded_size == profile.decoded_size
            && sha256_bytes(stored) == profile.stored_sha256,
        "MODE SELECT {role} stored source changed"
    );
    let decoded = decompress(stored, false)?;
    ensure!(
        decoded.len() == profile.decoded_size && sha256_bytes(&decoded) == profile.decoded_sha256,
        "MODE SELECT {role} decoded source changed"
    );
    validate_compressed_catalog_record(
        main_source,
        source.catalog_index,
        &source.catalog_entry_sha256,
        stored,
        main_executable,
        role,
    )
}

fn validate_compressed_catalog_record(
    main_source: &MainExecutableDocument,
    catalog_index: i16,
    catalog_entry_sha256: &str,
    stored: &[u8],
    main_executable: &[u8],
    role: &str,
) -> Result<()> {
    let catalog = catalog_entry(
        main_executable,
        main_source,
        catalog_index,
        catalog_entry_sha256,
        role,
    )?;
    ensure!(
        read_u32(catalog, 4, &format!("{role} catalog byte count"))?
            == u32::try_from(stored.len())?,
        "MODE SELECT {role} catalog record changed"
    );
    Ok(())
}

fn validate_main_entry_call(
    main_executable: &[u8],
    pointer: u32,
    pointer_load: u32,
    call: u32,
    role: &str,
) -> Result<()> {
    ensure!(
        pointer_load + 0x0c == call,
        "{role} entry-call layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                pointer_load,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: (pointer >> 16) as u16,
                },
            ),
            (
                pointer_load + 4,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: pointer as u16 as i16,
                },
            ),
            (
                call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
        ],
        role,
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
        "MODE SELECT {role} catalog entry changed"
    );
    Ok(entry)
}

fn validate_main_instructions(
    main_executable: &[u8],
    expected: &[(u32, Instruction)],
    role: &str,
) -> Result<()> {
    validate_record_instructions(
        main_executable_text(main_executable)?,
        MAIN_TEXT_RUNTIME_BASE,
        expected,
        role,
    )
}

fn validate_record_instructions(
    bytes: &[u8],
    runtime_base: u32,
    expected: &[(u32, Instruction)],
    role: &str,
) -> Result<()> {
    for (address, expected_instruction) in expected {
        let offset = address
            .checked_sub(runtime_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .with_context(|| format!("{role} address precedes its source record"))?;
        let word: [u8; 4] = bytes
            .get(offset..offset + 4)
            .with_context(|| format!("{role} instruction is outside its source record"))?
            .try_into()?;
        let instruction = decode(u32::from_le_bytes(word), *address)
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
