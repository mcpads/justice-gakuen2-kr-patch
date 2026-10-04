use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::Deserialize;

use super::mode_select_dispatcher::{
    mode_select_panel_index, mode_select_panel_index_state_address, mode_select_setup_entrypoint,
};
use super::model::{SurfaceInventory, SurfaceResolution, SurfaceTargetLayer, SurfaceTargetObject};
use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    MAIN_TEXT_RUNTIME_BASE, main_executable_text,
    profile::{
        CHARACTER_SELECT_COOPERATIVE_MENU_RECORD, CHARACTER_SELECT_OVERLAY_RECORDS,
        CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD, CHARACTER_SELECT_SELP1_TEXTURE_RECORD,
        CHARACTER_SELECT_TEXTURE_RECORDS, MAIN_EXECUTABLE_RECORD,
    },
};

const BINDING_KIND: &str = "justice_gakuen2_mode_select_character_select_route_binding";
const WORKER_REGISTRATION_ADDRESS: u32 = 0x8001_05ec;
const STANDARD_PIPELINE: &str = "standard";
const TRAINING_PIPELINE: &str = "training";
pub(super) const BINDING_FILE: &str = "character-select-routes.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterSelectRouteBindingDocument {
    kind: String,
    main_executable: MainExecutableDocument,
    primary_consumer: PrimaryConsumerDocument,
    additional_consumers: Vec<AdditionalConsumerDocument>,
    standard_pipeline: StandardPipelineDocument,
    training_pipeline: TrainingPipelineDocument,
    overlay_selector: OverlaySelectorDocument,
    entries: Vec<CharacterSelectEntryDocument>,
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
struct PrimaryConsumerDocument {
    overlay: OverlayRecordDocument,
    texture: TextureRecordDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdditionalConsumerDocument {
    overlay_slot: usize,
    overlay: OverlayRecordDocument,
    texture: TextureRecordDocument,
    auxiliary_texture: Option<TextureRecordDocument>,
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
struct TextureRecordDocument {
    path: String,
    stored_sha256: String,
    decoded_sha256: String,
    decoded_size: usize,
    catalog_index: i16,
    catalog_entry_sha256: String,
    load_call_address: String,
    catalog_index_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardPipelineDocument {
    worker_entrypoint: String,
    worker_dispatch_call_address: String,
    state_dispatcher_address: String,
    state_index_address: String,
    state_index_load_address: String,
    state_table_address: String,
    initial_state_case_address: String,
    initial_state_handler_address: String,
    initial_state_handler_call_address: String,
    initial_loader_call_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrainingPipelineDocument {
    worker_entrypoint: String,
    state_index_address: String,
    state_index_load_address: String,
    state_table_address: String,
    initial_state_case_address: String,
    initial_loader_call_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlaySelectorDocument {
    loader_address: String,
    mode_state_address: String,
    mode_state_load_address: String,
    selection_sequence_address: String,
    selected_slot_join_address: String,
    overlay_loader_address: String,
    overlay_load_call_address: String,
    catalog_index_address: String,
    catalog_index_base: i16,
    overlay_entry_pointer_address: String,
    overlay_entry_pointer_load_address: String,
    overlay_entry_call_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterSelectEntryDocument {
    surface_id: String,
    selection_asset_id: String,
    consumer_pipeline: String,
    setup_entrypoint: String,
    worker_pointer_high_address: String,
    worker_pointer_low_address: String,
    worker_registration_call_address: String,
    mode_state_value: usize,
    selected_overlay_slot: usize,
}

pub(super) fn validate_mode_select_character_select_routes(
    inventory: &SurfaceInventory,
    main_executable: &[u8],
    character_select_overlays: &[Vec<u8>],
    character_select_textures_stored: &[Vec<u8>],
    cooperative_menu_stored: &[u8],
) -> Result<()> {
    let binding = load_binding(inventory)?;
    ensure!(
        character_select_overlays.len() == CHARACTER_SELECT_OVERLAY_RECORDS.len()
            && character_select_textures_stored.len() == CHARACTER_SELECT_TEXTURE_RECORDS.len(),
        "MODE SELECT character-select source population changed"
    );
    validate_main_executable(&binding.main_executable, main_executable)?;
    validate_primary_consumer(
        &binding,
        main_executable,
        &character_select_overlays[0],
        &character_select_textures_stored[0],
    )?;
    validate_additional_consumers(
        &binding,
        main_executable,
        character_select_overlays,
        character_select_textures_stored,
        cooperative_menu_stored,
    )?;
    validate_overlay_selector(
        inventory,
        &binding.overlay_selector,
        &binding.primary_consumer.overlay,
        main_executable,
    )?;
    validate_standard_pipeline(
        &binding.standard_pipeline,
        &binding.overlay_selector,
        main_executable,
    )?;
    validate_training_pipeline(
        &binding.training_pipeline,
        &binding.overlay_selector,
        main_executable,
    )?;
    validate_entries(inventory, &binding, main_executable)
}

fn load_binding(inventory: &SurfaceInventory) -> Result<CharacterSelectRouteBindingDocument> {
    let binding_bytes = inventory
        .binding_files
        .get(BINDING_FILE)
        .with_context(|| format!("MODE SELECT surface inventory is missing {BINDING_FILE}"))?;
    let binding: CharacterSelectRouteBindingDocument = serde_json::from_slice(binding_bytes)
        .with_context(|| format!("failed to parse MODE SELECT binding {BINDING_FILE}"))?;
    ensure!(
        binding.kind == BINDING_KIND,
        "unsupported MODE SELECT character-select binding kind {:?}",
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
        "MODE SELECT character-select main-executable source changed"
    );
    ensure!(
        parse_address(&source.runtime_base, "main executable runtime base")?
            == MAIN_TEXT_RUNTIME_BASE
            && source.catalog_entry_size == 12,
        "MODE SELECT character-select main-executable layout changed"
    );
    parse_offset(
        &source.catalog_table_offset,
        "MODE SELECT source catalog table",
    )?;
    Ok(())
}

fn validate_primary_consumer(
    binding: &CharacterSelectRouteBindingDocument,
    main_executable: &[u8],
    primary_overlay: &[u8],
    primary_texture_stored: &[u8],
) -> Result<()> {
    let overlay = &binding.primary_consumer.overlay;
    let overlay_runtime_base = parse_address(&overlay.runtime_base, "PLSEL1 runtime base")?;
    let overlay_entrypoint = parse_address(&overlay.entrypoint, "PLSEL1 entrypoint")?;
    ensure!(
        overlay.path == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.path
            && overlay.sha256 == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.sha256
            && primary_overlay.len() == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.size
            && sha256_bytes(primary_overlay) == CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD.sha256
            && overlay_runtime_base == 0x800a_2000
            && read_u32(primary_overlay, 0, "PLSEL1 entry pointer")? == overlay_entrypoint,
        "MODE SELECT PLSEL1 source binding changed"
    );
    let overlay_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        overlay.catalog_index,
        &overlay.catalog_entry_sha256,
        "PLSEL1 overlay",
    )?;
    ensure!(
        read_u32(overlay_catalog_entry, 4, "PLSEL1 catalog byte count")?
            == u32::try_from(primary_overlay.len())?
            && read_u32(overlay_catalog_entry, 8, "PLSEL1 catalog entrypoint")?
                == overlay_entrypoint,
        "MODE SELECT PLSEL1 catalog record changed"
    );

    let texture = &binding.primary_consumer.texture;
    ensure!(
        texture.path == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.path
            && texture.stored_sha256 == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.stored_sha256
            && texture.decoded_sha256 == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_sha256
            && texture.decoded_size == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_size
            && sha256_bytes(primary_texture_stored)
                == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.stored_sha256,
        "MODE SELECT SELP1 stored source binding changed"
    );
    let primary_texture_decoded = decompress(primary_texture_stored, false)?;
    ensure!(
        primary_texture_decoded.len() == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_size
            && sha256_bytes(&primary_texture_decoded)
                == CHARACTER_SELECT_SELP1_TEXTURE_RECORD.decoded_sha256,
        "MODE SELECT SELP1 decoded source binding changed"
    );
    let texture_catalog_entry = catalog_entry(
        main_executable,
        &binding.main_executable,
        texture.catalog_index,
        &texture.catalog_entry_sha256,
        "SELP1 texture",
    )?;
    ensure!(
        read_u32(texture_catalog_entry, 4, "SELP1 catalog byte count")?
            == u32::try_from(primary_texture_stored.len())?,
        "MODE SELECT SELP1 catalog record changed"
    );
    let texture_load_call = parse_address(&texture.load_call_address, "SELP1 load call")?;
    let texture_catalog_index_address =
        parse_address(&texture.catalog_index_address, "SELP1 catalog-index setup")?;
    ensure!(
        texture_catalog_index_address == texture_load_call + 4,
        "MODE SELECT SELP1 load layout changed"
    );
    validate_instructions(
        primary_overlay,
        overlay_runtime_base,
        &[
            (
                texture_load_call,
                Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                },
            ),
            (
                texture_catalog_index_address,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: texture.catalog_index,
                },
            ),
        ],
        "MODE SELECT PLSEL1-to-SELP1 resource load",
    )
}

fn validate_additional_consumers(
    binding: &CharacterSelectRouteBindingDocument,
    main_executable: &[u8],
    character_select_overlays: &[Vec<u8>],
    character_select_textures_stored: &[Vec<u8>],
    cooperative_menu_stored: &[u8],
) -> Result<()> {
    let slots = binding
        .additional_consumers
        .iter()
        .map(|consumer| consumer.overlay_slot)
        .collect::<BTreeSet<_>>();
    ensure!(
        binding.additional_consumers.len() == 4 && slots == BTreeSet::from([1, 2, 3, 4]),
        "MODE SELECT additional character-select consumer population changed"
    );
    for consumer in &binding.additional_consumers {
        let slot = consumer.overlay_slot;
        let overlay_profile = CHARACTER_SELECT_OVERLAY_RECORDS[slot];
        let texture_profile = CHARACTER_SELECT_TEXTURE_RECORDS[slot];
        let overlay_bytes = &character_select_overlays[slot];
        let texture_stored = &character_select_textures_stored[slot];
        let overlay = &consumer.overlay;
        let texture = &consumer.texture;
        let overlay_runtime_base = parse_address(
            &overlay.runtime_base,
            "character-select overlay runtime base",
        )?;
        let overlay_entrypoint =
            parse_address(&overlay.entrypoint, "character-select overlay entrypoint")?;
        ensure!(
            overlay.path == overlay_profile.path
                && overlay.sha256 == overlay_profile.sha256
                && overlay_bytes.len() == overlay_profile.size
                && sha256_bytes(overlay_bytes) == overlay_profile.sha256
                && overlay_runtime_base == 0x800a_2000
                && overlay.catalog_index == 0x19 + i16::try_from(slot)?
                && read_u32(overlay_bytes, 0, "character-select overlay entry pointer")?
                    == overlay_entrypoint,
            "MODE SELECT character-select overlay source binding changed for slot {slot}"
        );
        let overlay_catalog_entry = catalog_entry(
            main_executable,
            &binding.main_executable,
            overlay.catalog_index,
            &overlay.catalog_entry_sha256,
            "character-select overlay",
        )?;
        ensure!(
            read_u32(
                overlay_catalog_entry,
                4,
                "character-select overlay catalog byte count"
            )? == u32::try_from(overlay_bytes.len())?
                && read_u32(
                    overlay_catalog_entry,
                    8,
                    "character-select overlay catalog entrypoint"
                )? == overlay_entrypoint,
            "MODE SELECT character-select overlay catalog record changed for slot {slot}"
        );

        ensure!(
            texture.path == texture_profile.path
                && texture.stored_sha256 == texture_profile.stored_sha256
                && texture.decoded_sha256 == texture_profile.decoded_sha256
                && texture.decoded_size == texture_profile.decoded_size
                && texture.catalog_index == 0x35 + i16::try_from(slot)?
                && sha256_bytes(texture_stored) == texture_profile.stored_sha256,
            "MODE SELECT character-select texture source binding changed for slot {slot}"
        );
        let texture_decoded = decompress(texture_stored, false)?;
        ensure!(
            texture_decoded.len() == texture_profile.decoded_size
                && sha256_bytes(&texture_decoded) == texture_profile.decoded_sha256,
            "MODE SELECT character-select decoded texture changed for slot {slot}"
        );
        let texture_catalog_entry = catalog_entry(
            main_executable,
            &binding.main_executable,
            texture.catalog_index,
            &texture.catalog_entry_sha256,
            "character-select texture",
        )?;
        ensure!(
            read_u32(
                texture_catalog_entry,
                4,
                "character-select texture catalog byte count"
            )? == u32::try_from(texture_stored.len())?,
            "MODE SELECT character-select texture catalog record changed for slot {slot}"
        );
        validate_texture_load(
            overlay_bytes,
            overlay_runtime_base,
            texture,
            "MODE SELECT character-select texture load",
        )?;

        match (slot, consumer.auxiliary_texture.as_ref()) {
            (4, Some(auxiliary)) => validate_cooperative_menu(
                binding,
                main_executable,
                overlay_bytes,
                overlay_runtime_base,
                auxiliary,
                cooperative_menu_stored,
            )?,
            (4, None) => anyhow::bail!(
                "MODE SELECT cooperative character-select consumer lost its auxiliary texture"
            ),
            (_, Some(_)) => anyhow::bail!(
                "MODE SELECT character-select slot {slot} gained an unsupported auxiliary texture"
            ),
            (_, None) => {}
        }
    }
    Ok(())
}

fn validate_cooperative_menu(
    binding: &CharacterSelectRouteBindingDocument,
    main_executable: &[u8],
    overlay_bytes: &[u8],
    overlay_runtime_base: u32,
    auxiliary: &TextureRecordDocument,
    cooperative_menu_stored: &[u8],
) -> Result<()> {
    let profile = CHARACTER_SELECT_COOPERATIVE_MENU_RECORD;
    ensure!(
        auxiliary.path == profile.path
            && auxiliary.stored_sha256 == profile.stored_sha256
            && auxiliary.decoded_sha256 == profile.decoded_sha256
            && auxiliary.decoded_size == profile.decoded_size
            && auxiliary.catalog_index == 0x2dd
            && sha256_bytes(cooperative_menu_stored) == profile.stored_sha256,
        "MODE SELECT cooperative-menu stored source binding changed"
    );
    let decoded = decompress(cooperative_menu_stored, true)?;
    ensure!(
        decoded.len() == profile.decoded_size && sha256_bytes(&decoded) == profile.decoded_sha256,
        "MODE SELECT cooperative-menu decoded source binding changed"
    );
    let catalog = catalog_entry(
        main_executable,
        &binding.main_executable,
        auxiliary.catalog_index,
        &auxiliary.catalog_entry_sha256,
        "cooperative-menu texture",
    )?;
    ensure!(
        read_u32(catalog, 4, "cooperative-menu catalog byte count")?
            == u32::try_from(cooperative_menu_stored.len())?,
        "MODE SELECT cooperative-menu catalog record changed"
    );
    validate_texture_load(
        overlay_bytes,
        overlay_runtime_base,
        auxiliary,
        "MODE SELECT cooperative-menu texture load",
    )
}

fn validate_texture_load(
    overlay: &[u8],
    overlay_runtime_base: u32,
    texture: &TextureRecordDocument,
    role: &str,
) -> Result<()> {
    let load_call = parse_address(&texture.load_call_address, role)?;
    let catalog_index_address = parse_address(&texture.catalog_index_address, role)?;
    ensure!(
        catalog_index_address == load_call + 4,
        "{role} layout changed"
    );
    validate_instructions(
        overlay,
        overlay_runtime_base,
        &[
            (
                load_call,
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
                    immediate: texture.catalog_index,
                },
            ),
        ],
        role,
    )
}

fn validate_overlay_selector(
    inventory: &SurfaceInventory,
    selector: &OverlaySelectorDocument,
    overlay: &OverlayRecordDocument,
    main_executable: &[u8],
) -> Result<()> {
    let loader = parse_address(&selector.loader_address, "character-select loader")?;
    let mode_state = parse_address(&selector.mode_state_address, "MODE SELECT panel state")?;
    let mode_state_load = parse_address(
        &selector.mode_state_load_address,
        "character-select mode-state load",
    )?;
    let selection = parse_address(
        &selector.selection_sequence_address,
        "character-select overlay selection",
    )?;
    let join = parse_address(
        &selector.selected_slot_join_address,
        "character-select selected-slot join",
    )?;
    ensure!(
        loader == 0x8002_7998
            && mode_state == mode_select_panel_index_state_address(inventory)?
            && mode_state_load + 0x0c == selection
            && selection + 0x5c == join,
        "MODE SELECT character-select selector layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                mode_state_load - 4,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: (mode_state >> 16) as u16,
                },
            ),
            (
                mode_state_load,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: mode_state as u16 as i16,
                },
            ),
            (
                selection,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 8,
                },
            ),
            (
                selection + 0x0c,
                Instruction::Beq {
                    rs: Register::V1,
                    rt: Register::V0,
                    target: selection + 0x48,
                },
            ),
            (
                selection + 0x10,
                Instruction::Slti {
                    rt: Register::V0,
                    rs: Register::V1,
                    immediate: 9,
                },
            ),
            (
                selection + 0x14,
                Instruction::Beq {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: selection + 0x2c,
                },
            ),
            (
                selection + 0x18,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 7,
                },
            ),
            (
                selection + 0x1c,
                Instruction::Beq {
                    rs: Register::V1,
                    rt: Register::V0,
                    target: join,
                },
            ),
            (
                selection + 0x20,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 4,
                },
            ),
            (selection + 0x24, Instruction::J { target: join }),
            (
                selection + 0x28,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                selection + 0x2c,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 9,
                },
            ),
            (
                selection + 0x30,
                Instruction::Beq {
                    rs: Register::V1,
                    rt: Register::V0,
                    target: selection + 0x58,
                },
            ),
            (
                selection + 0x34,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 10,
                },
            ),
            (
                selection + 0x38,
                Instruction::Beq {
                    rs: Register::V1,
                    rt: Register::V0,
                    target: selection + 0x50,
                },
            ),
            (
                selection + 0x3c,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (selection + 0x40, Instruction::J { target: join }),
            (selection + 0x48, Instruction::J { target: join }),
            (
                selection + 0x4c,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
            (selection + 0x50, Instruction::J { target: join }),
            (
                selection + 0x54,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 2,
                },
            ),
            (
                selection + 0x58,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 3,
                },
            ),
        ],
        "MODE SELECT character-select overlay selection",
    )?;

    let overlay_loader = parse_address(
        &selector.overlay_loader_address,
        "character-select overlay loader",
    )?;
    let overlay_load_call = parse_address(
        &selector.overlay_load_call_address,
        "character-select overlay load call",
    )?;
    let catalog_index_address = parse_address(
        &selector.catalog_index_address,
        "character-select overlay catalog-index setup",
    )?;
    let entry_pointer = parse_address(
        &selector.overlay_entry_pointer_address,
        "character-select overlay entry pointer",
    )?;
    let entry_pointer_load = parse_address(
        &selector.overlay_entry_pointer_load_address,
        "character-select overlay entry-pointer load",
    )?;
    let entry_call = parse_address(
        &selector.overlay_entry_call_address,
        "character-select overlay entry call",
    )?;
    ensure!(
        selector.catalog_index_base == overlay.catalog_index
            && overlay_load_call == selection + 0x74
            && catalog_index_address == overlay_load_call + 4
            && entry_pointer_load == catalog_index_address + 4
            && entry_call == entry_pointer_load + 0x0c,
        "MODE SELECT character-select overlay load layout changed"
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
                    rs: Register::S0,
                    immediate: selector.catalog_index_base,
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
        ],
        "MODE SELECT character-select overlay load",
    )
}

fn validate_standard_pipeline(
    pipeline: &StandardPipelineDocument,
    selector: &OverlaySelectorDocument,
    main_executable: &[u8],
) -> Result<()> {
    let worker = parse_address(&pipeline.worker_entrypoint, "standard worker entrypoint")?;
    let worker_dispatch_call = parse_address(
        &pipeline.worker_dispatch_call_address,
        "standard worker dispatch call",
    )?;
    let state_dispatcher = parse_address(
        &pipeline.state_dispatcher_address,
        "standard state dispatcher",
    )?;
    let state_index = parse_address(&pipeline.state_index_address, "standard state index")?;
    let state_index_load = parse_address(
        &pipeline.state_index_load_address,
        "standard state-index load",
    )?;
    let state_table = parse_address(&pipeline.state_table_address, "standard state table")?;
    let initial_case = parse_address(
        &pipeline.initial_state_case_address,
        "standard initial-state case",
    )?;
    let initial_handler = parse_address(
        &pipeline.initial_state_handler_address,
        "standard initial-state handler",
    )?;
    let handler_call = parse_address(
        &pipeline.initial_state_handler_call_address,
        "standard initial-state handler call",
    )?;
    let loader_call = parse_address(
        &pipeline.initial_loader_call_address,
        "standard initial-loader call",
    )?;
    let loader = parse_address(&selector.loader_address, "character-select loader")?;
    ensure!(
        worker_dispatch_call == worker + 8
            && state_index == 0x801f_6480
            && handler_call == initial_case,
        "MODE SELECT standard character-select pipeline layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                worker_dispatch_call,
                Instruction::Jal {
                    target: state_dispatcher,
                },
            ),
            (
                state_index_load - 0x0c,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (state_index >> 16) as u16,
                },
            ),
            (
                state_index_load - 8,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: state_index as u16,
                },
            ),
            (
                state_index_load,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::A0,
                    offset: 0,
                },
            ),
            (
                state_index_load + 0x14,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: (state_table >> 16) as u16,
                },
            ),
            (
                state_index_load + 0x1c,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: state_table as u16 as i16,
                },
            ),
            (
                handler_call,
                Instruction::Jal {
                    target: initial_handler,
                },
            ),
            (loader_call, Instruction::Jal { target: loader }),
        ],
        "MODE SELECT standard character-select state route",
    )?;
    ensure!(
        read_main_u32(
            main_executable,
            state_table,
            "standard initial-state table entry"
        )? == initial_case,
        "MODE SELECT standard character-select initial-state target changed"
    );
    Ok(())
}

fn validate_training_pipeline(
    pipeline: &TrainingPipelineDocument,
    selector: &OverlaySelectorDocument,
    main_executable: &[u8],
) -> Result<()> {
    let worker = parse_address(&pipeline.worker_entrypoint, "training worker entrypoint")?;
    let state_index = parse_address(&pipeline.state_index_address, "training state index")?;
    let state_index_load = parse_address(
        &pipeline.state_index_load_address,
        "training state-index load",
    )?;
    let state_table = parse_address(&pipeline.state_table_address, "training state table")?;
    let initial_case = parse_address(
        &pipeline.initial_state_case_address,
        "training initial-state case",
    )?;
    let loader_call = parse_address(
        &pipeline.initial_loader_call_address,
        "training initial-loader call",
    )?;
    let loader = parse_address(&selector.loader_address, "character-select loader")?;
    ensure!(
        state_index == 0x801f_6480
            && state_index_load == worker + 0x30
            && loader_call == initial_case,
        "MODE SELECT training character-select pipeline layout changed"
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                worker + 0x0c,
                Instruction::Lui {
                    rt: Register::S1,
                    immediate: ((state_table + 0x8000) >> 16) as u16,
                },
            ),
            (
                worker + 0x10,
                Instruction::Addiu {
                    rt: Register::S1,
                    rs: Register::S1,
                    immediate: state_table as u16 as i16,
                },
            ),
            (
                state_index_load - 4,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: (state_index >> 16) as u16,
                },
            ),
            (
                state_index_load,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: state_index as u16 as i16,
                },
            ),
            (loader_call, Instruction::Jal { target: loader }),
        ],
        "MODE SELECT training character-select state route",
    )?;
    ensure!(
        read_main_u32(
            main_executable,
            state_table,
            "training initial-state table entry"
        )? == initial_case,
        "MODE SELECT training character-select initial-state target changed"
    );
    Ok(())
}

fn validate_entries(
    inventory: &SurfaceInventory,
    binding: &CharacterSelectRouteBindingDocument,
    main_executable: &[u8],
) -> Result<()> {
    let expected_entries = BTreeSet::from([
        "cooperative",
        "league",
        "solo",
        "team",
        "tournament",
        "training",
        "versus",
    ]);
    let entry_ids = binding
        .entries
        .iter()
        .map(|entry| entry.selection_asset_id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        binding.entries.len() == expected_entries.len() && entry_ids == expected_entries,
        "MODE SELECT character-select entry population changed"
    );
    let standard_worker = parse_address(
        &binding.standard_pipeline.worker_entrypoint,
        "standard worker entrypoint",
    )?;
    let training_worker = parse_address(
        &binding.training_pipeline.worker_entrypoint,
        "training worker entrypoint",
    )?;
    for entry in &binding.entries {
        validate_resolved_surface(inventory, entry)?;
        let setup_entrypoint =
            parse_address(&entry.setup_entrypoint, "character-select setup entrypoint")?;
        ensure!(
            entry.surface_id == format!("mode-select/entry/{}", entry.selection_asset_id)
                && setup_entrypoint
                    == mode_select_setup_entrypoint(inventory, &entry.selection_asset_id)?
                && entry.mode_state_value
                    == mode_select_panel_index(inventory, &entry.selection_asset_id)?,
            "MODE SELECT {} binding no longer matches its dispatcher entry",
            entry.selection_asset_id
        );
        let worker = match entry.consumer_pipeline.as_str() {
            STANDARD_PIPELINE => standard_worker,
            TRAINING_PIPELINE => training_worker,
            other => {
                anyhow::bail!(
                    "MODE SELECT {} uses unsupported character-select pipeline {other:?}",
                    entry.selection_asset_id
                )
            }
        };
        let expected_slot = overlay_slot_for_mode(entry.mode_state_value);
        let selected_overlay = consumer_overlay(binding, expected_slot)?;
        ensure!(
            entry.selected_overlay_slot == expected_slot
                && i16::try_from(expected_slot)?
                    .checked_add(binding.overlay_selector.catalog_index_base)
                    == Some(selected_overlay.catalog_index),
            "MODE SELECT {} no longer selects its character-select overlay",
            entry.selection_asset_id
        );
        validate_worker_registration(entry, worker, main_executable)?;
    }
    Ok(())
}

fn validate_worker_registration(
    entry: &CharacterSelectEntryDocument,
    worker: u32,
    main_executable: &[u8],
) -> Result<()> {
    let pointer_high = parse_address(
        &entry.worker_pointer_high_address,
        "character-select worker-pointer high setup",
    )?;
    let pointer_low = parse_address(
        &entry.worker_pointer_low_address,
        "character-select worker-pointer low setup",
    )?;
    let registration_call = parse_address(
        &entry.worker_registration_call_address,
        "character-select worker registration call",
    )?;
    ensure!(
        pointer_high >= entry_setup_address(entry)?
            && pointer_low == pointer_high + 4
            && registration_call >= pointer_low + 4
            && registration_call < entry_setup_address(entry)? + 0x200,
        "MODE SELECT {} worker registration layout changed",
        entry.selection_asset_id
    );
    validate_main_instructions(
        main_executable,
        &[
            (
                pointer_high,
                Instruction::Lui {
                    rt: Register::A1,
                    immediate: ((worker + 0x8000) >> 16) as u16,
                },
            ),
            (
                pointer_low,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::A1,
                    immediate: worker as u16 as i16,
                },
            ),
            (
                registration_call,
                Instruction::Jal {
                    target: WORKER_REGISTRATION_ADDRESS,
                },
            ),
            (
                registration_call + 4,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
        ],
        "MODE SELECT character-select worker registration",
    )
}

fn entry_setup_address(entry: &CharacterSelectEntryDocument) -> Result<u32> {
    parse_address(&entry.setup_entrypoint, "character-select setup entrypoint")
}

fn overlay_slot_for_mode(mode_state_value: usize) -> usize {
    match mode_state_value {
        7 => 4,
        8 => 1,
        9 => 3,
        10 => 2,
        _ => 0,
    }
}

fn consumer_overlay(
    binding: &CharacterSelectRouteBindingDocument,
    overlay_slot: usize,
) -> Result<&OverlayRecordDocument> {
    if overlay_slot == 0 {
        return Ok(&binding.primary_consumer.overlay);
    }
    binding
        .additional_consumers
        .iter()
        .find(|consumer| consumer.overlay_slot == overlay_slot)
        .map(|consumer| &consumer.overlay)
        .with_context(|| format!("missing character-select consumer for slot {overlay_slot}"))
}

fn consumer_class(selection_asset_id: &str) -> Result<&'static str> {
    match selection_asset_id {
        "solo" | "training" | "versus" => Ok("character-select/plsel1-renderer"),
        "team" => Ok("character-select/plsel2-team-renderer"),
        "league" => Ok("character-select/plsel3-league-renderer"),
        "tournament" => Ok("character-select/plsel4-tournament-renderer"),
        "cooperative" => Ok("character-select/plsel5-cooperative-menu-renderer"),
        other => anyhow::bail!("unsupported character-select surface {other:?}"),
    }
}

fn validate_resolved_surface(
    inventory: &SurfaceInventory,
    entry: &CharacterSelectEntryDocument,
) -> Result<()> {
    let surface_id = &entry.surface_id;
    let node = inventory
        .nodes
        .iter()
        .find(|node| node.id == *surface_id)
        .with_context(|| format!("surface inventory is missing {surface_id}"))?;
    ensure!(
        node.resolution == SurfaceResolution::Resolved,
        "surface inventory {surface_id} is not resolved"
    );
    ensure!(
        node.consumer_class.as_deref() == Some(consumer_class(&entry.selection_asset_id)?),
        "surface inventory {surface_id} consumer class changed"
    );
    let targets = target_paths_and_layers(&node.target_objects);
    let slot = entry.selected_overlay_slot;
    let mut expected = BTreeSet::from([
        (
            CHARACTER_SELECT_OVERLAY_RECORDS[slot].path,
            SurfaceTargetLayer::IsoRecord,
        ),
        (
            CHARACTER_SELECT_TEXTURE_RECORDS[slot].path,
            SurfaceTargetLayer::DecodedRecord,
        ),
    ]);
    if entry.selection_asset_id == "cooperative" {
        expected.insert((
            CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.path,
            SurfaceTargetLayer::DecodedRecord,
        ));
    }
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

fn read_main_u32(main_executable: &[u8], address: u32, role: &str) -> Result<u32> {
    let text = main_executable_text(main_executable)?;
    let offset = address
        .checked_sub(MAIN_TEXT_RUNTIME_BASE)
        .and_then(|offset| usize::try_from(offset).ok())
        .with_context(|| format!("{role} precedes the main executable"))?;
    read_u32(text, offset, role)
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
