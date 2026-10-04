use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::dialogue_runtime_bootstrap::{
    DIRECT_NAME_HUD_STORE_ORIGIN, NICKNAME_HUD_GLYPH_STORE_ORIGIN,
    nickname_hud_persistent_cell_address, nickname_hud_reused_scene_cell_address,
};
use super::runtime::{emit_component_resolver, emit_glyph_fill_materializer};
use super::{
    NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_CELL_BYTES, NAME_GLYPH_PACK_STORAGE_BYTES, NameField,
    NameGlyphConsumerLayout, NameInputRuntimePackLayout, NicknameHudGlyphLayout, SYMBOL_KEYS,
    SharedNameOutlineRuntimeProgram,
};

#[path = "dialogue_runtime/ascii_resolver.rs"]
mod ascii_resolver;
#[path = "dialogue_runtime/cache_cell.rs"]
mod cache_cell;
#[path = "dialogue_runtime/mgame_reload_wrapper.rs"]
mod mgame_reload_wrapper;
#[cfg(test)]
#[path = "dialogue_runtime/mgame_reload_wrapper_tests.rs"]
mod mgame_reload_wrapper_tests;
#[path = "dialogue_runtime/mgame_runtime_repair.rs"]
mod mgame_runtime_repair;
#[cfg(test)]
#[path = "dialogue_runtime/mgame_runtime_repair_tests.rs"]
mod mgame_runtime_repair_tests;
#[path = "dialogue_runtime/name_consumer.rs"]
mod name_consumer;
#[path = "dialogue_runtime/nickname_hud_render_wrapper.rs"]
mod nickname_hud_render_wrapper;
#[path = "dialogue_runtime/nickname_hud_scaler.rs"]
mod nickname_hud_scaler;
#[path = "dialogue_runtime/nickname_hud_uploader.rs"]
mod nickname_hud_uploader;
#[path = "dialogue_runtime/pack_reader.rs"]
mod pack_reader;
use ascii_resolver::{ascii_name_legacy_indices, emit_ascii_name_code_resolver};
use cache_cell::emit_dialogue_cache_cell_clearer;
use mgame_reload_wrapper::emit_mgame_reload_wrapper;
pub(in crate::name_input) use mgame_runtime_repair::build_mgame_runtime_repair_program;
pub use mgame_runtime_repair::{
    MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
    MgameRuntimeRepairProgram, MgameRuntimeRepairProgramReport,
};
use name_consumer::{
    NICKNAME_ADDRESS, RELATIONSHIP_NAME_ADDRESS, RELATIONSHIP_NAME_FIELD_CATEGORY_REGISTER,
    RELATIONSHIP_NAME_RECORD_CELL_CAPACITY, emit_dialogue_name_consumer,
};
use nickname_hud_render_wrapper::emit_nickname_hud_render_wrapper;
use nickname_hud_scaler::emit_nickname_hud_glyph_scaler;
use nickname_hud_uploader::{NICKNAME_HUD_VRAM_CELL_RECTS, emit_nickname_hud_glyph_uploader};
use pack_reader::emit_dialogue_pack_byte_reader;
pub const NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN: u32 = 0x8009_7ad0;
pub const NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY: usize = 0x0d00;
pub const NAME_DIALOGUE_RUNTIME_ORIGIN: u32 = 0x8009_a400;
pub const NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY: usize = 0x0900;
pub const NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN: u32 = 0x800c_ad94;
pub const NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY: usize = 0x0910;
pub const MGAME_RELOAD_WRAPPER_ORIGIN: u32 = 0x8001_0cac;
pub const MGAME_RELOAD_WRAPPER_BYTE_CAPACITY: usize = 0x007c;

const CURRENT_ATLAS_POINTER_OFFSET: usize = 0;
const SYMBOL_TABLE_OFFSET: usize = 4;
const DATA_PREFIX_ALIGNMENT: usize = 16;
const DIALOGUE_CACHE_CELL_ROW_BYTES: i16 = 10;
const DIALOGUE_ATLAS_DESCRIPTOR_ADDRESS: u32 = 0x800d_0174;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameDialogueRuntimeProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub nickname_hud_render_wrapper_bytes: Vec<u8>,
    pub nickname_hud_render_wrapper_instructions: Vec<Instruction>,
    pub mgame_reload_wrapper_bytes: Vec<u8>,
    pub mgame_reload_wrapper_instructions: Vec<Instruction>,
    pub mgame_runtime_repair: MgameRuntimeRepairProgram,
    pub instruction_offset: usize,
    pub ascii_resolver_address: u32,
    pub ascii_legacy_indices: BTreeSet<u16>,
    pub name_consumer_address: u32,
    pub nickname_hud_scaler_address: u32,
    pub nickname_hud_uploader_address: u32,
    pub nickname_hud_render_wrapper_address: u32,
    pub mgame_reload_wrapper_address: u32,
    pub report: NameDialogueRuntimeProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameDialogueRuntimeProgramReport {
    pub origin: String,
    pub byte_count: usize,
    pub byte_capacity: usize,
    pub sha256: String,
    pub typed_instruction_count: usize,
    pub instruction_origin: String,
    pub current_atlas_pointer_address: String,
    pub symbol_table_address: String,
    pub pack_byte_reader_address: String,
    pub cache_cell_clearer_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_cell_resolver_address: Option<String>,
    pub component_resolver_address: Option<String>,
    pub shared_outline_runtime_sha256: String,
    pub shared_outline_pixel_address: String,
    pub shared_outline_cleanup_address: String,
    pub nickname_hud_store_address: String,
    pub direct_name_hud_store_address: String,
    pub nickname_hud_persistent_cell_addresses: [String; 4],
    pub nickname_hud_reused_scene_cell_addresses: [String; 4],
    pub nickname_hud_writes_reused_scene_cells: bool,
    pub glyph_materializer_address: String,
    pub ascii_resolver_address: String,
    pub name_consumer_address: String,
    pub nickname_hud_scaler_address: String,
    pub nickname_hud_uploader_address: String,
    pub nickname_hud_render_wrapper_address: String,
    pub nickname_hud_render_wrapper_byte_count: usize,
    pub nickname_hud_render_wrapper_sha256: String,
    pub nickname_hud_upload_after_native_texture_load: bool,
    pub nickname_hud_atlas_descriptor_address: String,
    pub nickname_hud_name_record_address: String,
    pub nickname_hud_lazy_materialization: bool,
    pub relationship_name_buffer_address: String,
    pub relationship_name_field_category_register: String,
    pub relationship_name_cache_slot_offsets: [usize; 3],
    pub relationship_name_record_cell_capacity: usize,
    pub mgame_reload_wrapper_address: String,
    pub mgame_reload_wrapper_byte_count: usize,
    pub mgame_reload_wrapper_sha256: String,
    pub mgame_runtime_repair: MgameRuntimeRepairProgramReport,
    pub nickname_hud_vram_cell_rects: [[u16; 4]; 4],
    pub nickname_hud_glyph_layout: NicknameHudGlyphLayout,
    pub shared_cache_code_range: [String; 2],
    pub glyph_pack_coordinate_list_offset: Option<usize>,
    pub fits_execution_region: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub fn build_name_dialogue_runtime_program(
    layout: &NameGlyphConsumerLayout,
    runtime_pack: &NameInputRuntimePackLayout,
    outline: &SharedNameOutlineRuntimeProgram,
    nickname_hud_layout: &NicknameHudGlyphLayout,
) -> Result<NameDialogueRuntimeProgram> {
    build_dialogue_runtime(
        layout,
        DialogueGlyphPayload::Components(runtime_pack),
        outline,
        nickname_hud_layout,
    )
}

pub fn build_name_dialogue_band_runtime_program(
    layout: &NameGlyphConsumerLayout,
    pack: &super::NameGlyphBandPack,
    outline: &SharedNameOutlineRuntimeProgram,
    nickname_hud_layout: &NicknameHudGlyphLayout,
) -> Result<NameDialogueRuntimeProgram> {
    build_dialogue_runtime(
        layout,
        DialogueGlyphPayload::Bands(pack),
        outline,
        nickname_hud_layout,
    )
}

enum DialogueGlyphPayload<'a> {
    Components(&'a NameInputRuntimePackLayout),
    Bands(&'a super::NameGlyphBandPack),
}

fn build_dialogue_runtime(
    layout: &NameGlyphConsumerLayout,
    payload: DialogueGlyphPayload<'_>,
    outline: &SharedNameOutlineRuntimeProgram,
    nickname_hud_layout: &NicknameHudGlyphLayout,
) -> Result<NameDialogueRuntimeProgram> {
    let payload_bytes = match payload {
        DialogueGlyphPayload::Components(pack) => {
            pack.pack_bytes + pack.runtime_coordinate_list_byte_count
        }
        DialogueGlyphPayload::Bands(pack) => pack.bytes().len(),
    };
    ensure!(
        payload_bytes <= NAME_GLYPH_PACK_STORAGE_BYTES,
        "dialogue name runtime pack exceeds the shared cells"
    );
    ensure!(
        layout.pack_cells.len() * NAME_GLYPH_CELL_BYTES == NAME_GLYPH_PACK_STORAGE_BYTES,
        "dialogue name runtime layout has the wrong component-pack capacity"
    );
    for cell in &layout.pack_cells {
        let sequence_bias = match cell.pack_cell_index {
            0..=27 => 56,
            28..=57 => 108,
            58..=94 => 133,
            _ => anyhow::bail!("dialogue name runtime pack cell index is out of range"),
        };
        let expected_code = layout
            .code_start
            .checked_add(u16::try_from(cell.pack_cell_index + sequence_bias)?)
            .context("dialogue name runtime pack code overflow")?;
        ensure!(
            cell.code == expected_code,
            "dialogue name runtime pack-reader arithmetic differs from the shared layout"
        );
    }
    let all_cache_codes = [
        NameField::FamilyName,
        NameField::GivenName,
        NameField::Nickname,
    ]
    .into_iter()
    .map(|field| layout.cache_codes_for_field(field))
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    ensure!(
        all_cache_codes.len() == NAME_GLYPH_CACHE_SLOT_COUNT
            && all_cache_codes
                .windows(2)
                .all(|pair| pair[1] == pair[0] + 1),
        "dialogue name runtime requires one consecutive shared cache range"
    );

    let mut prefix = vec![0_u8; SYMBOL_TABLE_OFFSET];
    prefix.extend_from_slice(SYMBOL_KEYS.as_bytes());
    let instruction_offset = prefix.len().next_multiple_of(DATA_PREFIX_ALIGNMENT);
    prefix.resize(instruction_offset, 0);
    let instruction_origin = NAME_DIALOGUE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(instruction_offset)?)
        .context("dialogue name runtime instruction origin overflow")?;
    let current_atlas_pointer_address = NAME_DIALOGUE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(CURRENT_ATLAS_POINTER_OFFSET)?)
        .context("dialogue name atlas-pointer address overflow")?;
    let symbol_table_address = NAME_DIALOGUE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(SYMBOL_TABLE_OFFSET)?)
        .context("dialogue name symbol-table address overflow")?;

    let mut assembler = Assembler::new();
    let pack_byte_reader_address = instruction_origin;
    emit_dialogue_pack_byte_reader(&mut assembler, current_atlas_pointer_address);
    let cache_address = next_address(&assembler, instruction_origin)?;
    let (
        cache_cell_clearer_address,
        cache_cell_resolver_address,
        component_resolver_address,
        glyph_materializer_address,
    ) = match payload {
        DialogueGlyphPayload::Components(runtime_pack) => {
            emit_dialogue_cache_cell_clearer(
                &mut assembler,
                current_atlas_pointer_address,
                all_cache_codes[0],
            );
            let resolver = next_address(&assembler, instruction_origin)?;
            emit_component_resolver(&mut assembler, runtime_pack)?;
            let materializer = next_address(&assembler, instruction_origin)?;
            emit_glyph_fill_materializer(
                &mut assembler,
                runtime_pack,
                u16::try_from(runtime_pack.pack_bytes)?,
                DIALOGUE_CACHE_CELL_ROW_BYTES,
                outline.outline_pixel_address,
                outline.outline_cleanup_address,
                Some(NICKNAME_HUD_GLYPH_STORE_ORIGIN),
            )?;
            (Some(cache_address), None, Some(resolver), materializer)
        }
        DialogueGlyphPayload::Bands(pack) => {
            cache_cell::emit_dialogue_cache_cell_resolver(
                &mut assembler,
                current_atlas_pointer_address,
                all_cache_codes[0],
            );
            let materializer = next_address(&assembler, instruction_origin)?;
            assembler.label("materialize_name_glyph_fill");
            let bytes = pack.build_cache_program(
                materializer,
                pack_byte_reader_address,
                cache_address,
                DIALOGUE_CACHE_CELL_ROW_BYTES as usize,
                outline,
                Some(NICKNAME_HUD_GLYPH_STORE_ORIGIN),
            )?;
            assembler.emit_all(psx_r3000a::verify_placed_program(&bytes, materializer)?);
            (None, Some(cache_address), None, materializer)
        }
    };
    let ascii_resolver_address = next_address(&assembler, instruction_origin)?;
    let ascii_legacy_indices = ascii_name_legacy_indices(layout)?;
    emit_ascii_name_code_resolver(&mut assembler, layout, symbol_table_address)?;
    let name_consumer_address = next_address(&assembler, instruction_origin)?;
    emit_dialogue_name_consumer(
        &mut assembler,
        current_atlas_pointer_address,
        all_cache_codes[0],
    );
    let nickname_hud_scaler_address = next_address(&assembler, instruction_origin)?;
    emit_nickname_hud_glyph_scaler(&mut assembler, nickname_hud_layout);
    let nickname_hud_uploader_address = next_address(&assembler, instruction_origin)?;
    emit_nickname_hud_glyph_uploader(&mut assembler);
    let nickname_hud_render_wrapper_address = next_address(&assembler, instruction_origin)?;
    emit_nickname_hud_render_wrapper(
        &mut assembler,
        DIALOGUE_ATLAS_DESCRIPTOR_ADDRESS,
        NICKNAME_ADDRESS,
        name_consumer_address,
        nickname_hud_uploader_address,
    );
    let nickname_hud_render_wrapper_end = next_address(&assembler, instruction_origin)?;
    let placed = assembler
        .assemble(instruction_origin)
        .context("failed to assemble typed shared-name dialogue runtime")?;
    ensure!(
        instruction_offset + placed.bytes().len() <= NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY,
        "shared-name dialogue runtime needs {} bytes, exceeding its {}-byte execution region",
        instruction_offset + placed.bytes().len(),
        NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY
    );
    ensure!(
        placed.instruction_spans().len() == placed.instructions().len(),
        "shared-name dialogue runtime lost typed instruction placement evidence"
    );
    let render_wrapper_offset = usize::try_from(
        nickname_hud_render_wrapper_address
            .checked_sub(instruction_origin)
            .context("nickname HUD render wrapper precedes the dialogue runtime")?,
    )?;
    let render_wrapper_end = usize::try_from(
        nickname_hud_render_wrapper_end
            .checked_sub(instruction_origin)
            .context("nickname HUD render wrapper end precedes the dialogue runtime")?,
    )?;
    let nickname_hud_render_wrapper_bytes =
        placed.bytes()[render_wrapper_offset..render_wrapper_end].to_vec();
    let nickname_hud_render_wrapper_instructions =
        placed.instructions()[render_wrapper_offset / 4..render_wrapper_end / 4].to_vec();
    let instructions = placed.instructions().to_vec();
    let mut bytes = prefix;
    bytes.extend_from_slice(placed.bytes());
    let mgame_runtime_repair =
        build_mgame_runtime_repair_program(&bytes, nickname_hud_uploader_address)?;
    let mgame_reload_wrapper_address = MGAME_RELOAD_WRAPPER_ORIGIN;
    let mut mgame_reload_wrapper = Assembler::new();
    emit_mgame_reload_wrapper(&mut mgame_reload_wrapper, bytes.len())?;
    let mgame_reload_wrapper = mgame_reload_wrapper
        .assemble(mgame_reload_wrapper_address)
        .context("failed to assemble the persistent MGAME reload wrapper")?;
    let mgame_reload_wrapper_bytes = mgame_reload_wrapper.bytes().to_vec();
    let mgame_reload_wrapper_instructions = mgame_reload_wrapper.instructions().to_vec();
    ensure!(
        mgame_reload_wrapper_bytes.len() <= MGAME_RELOAD_WRAPPER_BYTE_CAPACITY,
        "MGAME reload wrapper exceeds its main-executable region"
    );

    Ok(NameDialogueRuntimeProgram {
        bytes: bytes.clone(),
        instructions: instructions.clone(),
        nickname_hud_render_wrapper_bytes: nickname_hud_render_wrapper_bytes.clone(),
        nickname_hud_render_wrapper_instructions: nickname_hud_render_wrapper_instructions.clone(),
        mgame_reload_wrapper_bytes: mgame_reload_wrapper_bytes.clone(),
        mgame_reload_wrapper_instructions: mgame_reload_wrapper_instructions.clone(),
        mgame_runtime_repair: mgame_runtime_repair.clone(),
        instruction_offset,
        ascii_resolver_address,
        ascii_legacy_indices,
        name_consumer_address,
        nickname_hud_scaler_address,
        nickname_hud_uploader_address,
        nickname_hud_render_wrapper_address,
        mgame_reload_wrapper_address,
        report: NameDialogueRuntimeProgramReport {
            origin: hex_address(NAME_DIALOGUE_RUNTIME_ORIGIN),
            byte_count: bytes.len(),
            byte_capacity: NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count: instructions.len(),
            instruction_origin: hex_address(instruction_origin),
            current_atlas_pointer_address: hex_address(current_atlas_pointer_address),
            symbol_table_address: hex_address(symbol_table_address),
            pack_byte_reader_address: hex_address(pack_byte_reader_address),
            cache_cell_clearer_address: cache_cell_clearer_address.map(hex_address),
            cache_cell_resolver_address: cache_cell_resolver_address.map(hex_address),
            component_resolver_address: component_resolver_address.map(hex_address),
            shared_outline_runtime_sha256: outline.report.sha256.clone(),
            shared_outline_pixel_address: hex_address(outline.outline_pixel_address),
            shared_outline_cleanup_address: hex_address(outline.outline_cleanup_address),
            nickname_hud_store_address: hex_address(NICKNAME_HUD_GLYPH_STORE_ORIGIN),
            direct_name_hud_store_address: hex_address(DIRECT_NAME_HUD_STORE_ORIGIN),
            nickname_hud_persistent_cell_addresses: [12, 13, 14, 15]
                .map(|slot| hex_address(nickname_hud_persistent_cell_address(slot).unwrap())),
            nickname_hud_reused_scene_cell_addresses: [12, 13, 14, 15]
                .map(|slot| hex_address(nickname_hud_reused_scene_cell_address(slot).unwrap())),
            nickname_hud_writes_reused_scene_cells: false,
            glyph_materializer_address: hex_address(glyph_materializer_address),
            ascii_resolver_address: hex_address(ascii_resolver_address),
            name_consumer_address: hex_address(name_consumer_address),
            nickname_hud_scaler_address: hex_address(nickname_hud_scaler_address),
            nickname_hud_uploader_address: hex_address(nickname_hud_uploader_address),
            nickname_hud_render_wrapper_address: hex_address(nickname_hud_render_wrapper_address),
            nickname_hud_render_wrapper_byte_count: nickname_hud_render_wrapper_bytes.len(),
            nickname_hud_render_wrapper_sha256: sha256_bytes(&nickname_hud_render_wrapper_bytes),
            nickname_hud_upload_after_native_texture_load: true,
            nickname_hud_atlas_descriptor_address: hex_address(DIALOGUE_ATLAS_DESCRIPTOR_ADDRESS),
            nickname_hud_name_record_address: hex_address(NICKNAME_ADDRESS),
            nickname_hud_lazy_materialization: true,
            relationship_name_buffer_address: hex_address(RELATIONSHIP_NAME_ADDRESS),
            relationship_name_field_category_register: RELATIONSHIP_NAME_FIELD_CATEGORY_REGISTER
                .to_string(),
            relationship_name_cache_slot_offsets: [0, 6, 12],
            relationship_name_record_cell_capacity: RELATIONSHIP_NAME_RECORD_CELL_CAPACITY,
            mgame_reload_wrapper_address: hex_address(mgame_reload_wrapper_address),
            mgame_reload_wrapper_byte_count: mgame_reload_wrapper_bytes.len(),
            mgame_reload_wrapper_sha256: sha256_bytes(&mgame_reload_wrapper_bytes),
            mgame_runtime_repair: mgame_runtime_repair.report.clone(),
            nickname_hud_vram_cell_rects: NICKNAME_HUD_VRAM_CELL_RECTS,
            nickname_hud_glyph_layout: *nickname_hud_layout,
            shared_cache_code_range: [
                format!("0x{:04x}", all_cache_codes[0]),
                format!("0x{:04x}", all_cache_codes[NAME_GLYPH_CACHE_SLOT_COUNT - 1]),
            ],
            glyph_pack_coordinate_list_offset: match payload {
                DialogueGlyphPayload::Components(pack) => Some(pack.pack_bytes),
                DialogueGlyphPayload::Bands(_) => None,
            },
            fits_execution_region: true,
            installed: false,
            runtime_execution_verified: false,
        },
    })
}

fn next_address(assembler: &Assembler, origin: u32) -> Result<u32> {
    let placed = assembler
        .assemble(origin)
        .context("failed to place partial shared-name dialogue runtime")?;
    origin
        .checked_add(u32::try_from(placed.bytes().len())?)
        .context("shared-name dialogue runtime address overflow")
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
