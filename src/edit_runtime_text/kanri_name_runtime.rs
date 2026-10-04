#[path = "kanri_fixed_glyphs.rs"]
mod fixed_glyphs;
#[path = "kanri_imported_names.rs"]
mod imported_names;
use fixed_glyphs::{emit_exact_static_glyph_unpacker, pack_static_glyphs};
use imported_names::{
    DIRECT_NAME_MAP_ADDRESS, DIRECT_NAME_MAP_OFFSET, ImportedDirectNameMap,
    emit_diary_name_transfer, emit_imported_name_normalizers,
};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{
    Assembler, Instruction, Register, decode, encode, load_address, verify_placed_program,
};
use serde::{Deserialize, Serialize};

use crate::compression::decompress;
use crate::contextual_texture_upload::{
    VRAM_UPLOAD_ROUTINE_ADDRESS, VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::dialogue_audit::{
    NAME_ENTRY_FONT_DECODED_SIZE, NAME_ENTRY_FONT_PATH, NAME_ENTRY_FONT_TIM_OFFSET,
};
use crate::disc::iso9660::FileRecord;
use crate::name_input::{
    ASCII_NAME_TAG, DIGIT_KEYS, HANGUL_NAME_TAG, KANRI_ASCII_CODE_TABLE_ENTRY_COUNT,
    KANRI_DISPLAY_CODE_CAPACITY, KANRI_GLYPH_PAYLOAD_BYTES, KANRI_INITIAL_GLYPH_CAPACITY,
    KANRI_NAME_CACHE_SLOT_COUNT, KANRI_NAME_SOURCE_SNAPSHOT_BYTES,
    KANRI_NAME_STORAGE_BYTE_CAPACITY, KANRI_NAME_STORAGE_ORIGIN, KANRI_PRIVATE_ASCII_GLYPH_COUNT,
    KANRI_PRIVATE_ASCII_GLYPHS, KANRI_RAW_ASCII_GLYPH_COUNT, KANRI_RAW_ASCII_GLYPHS,
    KANRI_UI_HANGUL_GLYPH_CAPACITY, KanriNameStorageLayout, LATIN_KEYS, NAME_GLYPH_CELL_BYTES,
    NAME_GLYPH_PACK_CELL_COUNT, NAME_GLYPH_PACK_STORAGE_BYTES, NameField, NameGlyphConsumerLayout,
    NameGlyphMaterializationBundle, NameInputRuntimeAtlasLayout, SYMBOL_KEYS,
    emit_component_resolver, emit_contiguous_pack_byte_reader, emit_glyph_fill_materializer,
    is_ks_x_1001_hangul,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::psx_machine_code_sources::verify_r3000a_load_delays;
use crate::text::{SKIP_GLYPH_CODE, common_menu_ascii_glyph_code};
use crate::tim::parse_4bpp_prefix;
use crate::write_scope::changed_ranges_are_within;

use super::source::OVERLAY_RUNTIME_BASE;

const EDIT_SHARED_UI_RUNTIME_BASE: u32 = 0x800d_4000;
const EDIT_SHARED_UI_SIZE: usize = 0x2b800;

const INITIALIZER_OFFSET: usize = 0x20c40;
const INITIALIZER_BYTE_CAPACITY: usize = 0x03c0;
const MATERIALIZER_STAGING_OFFSET: usize = 0x29400;
const MATERIALIZER_STAGING_BYTE_CAPACITY: usize = 0x0400;
const STATIC_REFRESHER_STAGING_OFFSET: usize = 0x20840;
const STATIC_REFRESHER_STAGING_BYTE_CAPACITY: usize = 0x0400;
const STATIC_HANGUL_PAYLOAD_STAGING_OFFSET: usize = 0x27240;
const STATIC_HANGUL_PAYLOAD_STAGING_BYTE_CAPACITY: usize =
    MATERIALIZER_STAGING_OFFSET - STATIC_HANGUL_PAYLOAD_STAGING_OFFSET;
const INITIALIZATION_DATA_STAGING_OFFSET: usize = 0x204d0;
const INITIALIZATION_DATA_STAGING_BYTE_CAPACITY: usize =
    STATIC_REFRESHER_STAGING_OFFSET - INITIALIZATION_DATA_STAGING_OFFSET;
const RESOLVER_STAGING_OFFSET: usize = 0x2b340;
const RESOLVER_STAGING_BYTE_CAPACITY: usize = 0x04c0;
const MATERIALIZER_PROGRAM_ORIGIN: u32 = 0x800a_b000;
const MATERIALIZER_PROGRAM_BYTE_CAPACITY: usize = 0x0400;
const RESOLVER_PROGRAM_ORIGIN: u32 = 0x800a_b400;
const RESOLVER_PROGRAM_BYTE_CAPACITY: usize = 0x04c0;
const STATIC_REFRESHER_PROGRAM_ORIGIN: u32 = 0x800a_b900;
const STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY: usize = STATIC_REFRESHER_STAGING_BYTE_CAPACITY;
// Keep the code table adjacent to the typed refresher/unpacker. Its staging
// copy shares the source-zero initialization band; glyph payloads stay contiguous.
const STATIC_REFRESHER_TABLE_OFFSET: usize = (STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY
    - (KANRI_INITIAL_GLYPH_CAPACITY - KANRI_RAW_ASCII_GLYPH_COUNT + 76) * 2)
    & !3;
const _: () = assert!(
    STATIC_REFRESHER_STAGING_OFFSET + STATIC_REFRESHER_STAGING_BYTE_CAPACITY == INITIALIZER_OFFSET
);
const _: () = assert!(MATERIALIZER_STAGING_OFFSET + MATERIALIZER_STAGING_BYTE_CAPACITY == 0x29800);
const STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN: u32 =
    STATIC_REFRESHER_PROGRAM_ORIGIN + STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY as u32;
const STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY: usize =
    KANRI_NAME_STORAGE_ORIGIN as usize - STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN as usize;
const KANRI_RUNTIME_OBSERVED_USED_END: u32 = 0x800a_af98;
const _: () = assert!(OVERLAY_RUNTIME_BASE + 0x83a8 <= KANRI_RUNTIME_OBSERVED_USED_END);
const _: () = assert!(KANRI_RUNTIME_OBSERVED_USED_END <= MATERIALIZER_PROGRAM_ORIGIN);

const LAST_IMPORT_PARSER_HOOK_OFFSET: usize = 0x1734;
const CARD_NAME_POINTER_OFFSET: usize = 0x2778;
const REGISTER_NAME_LOAD_OFFSET: usize = 0x41f4;

// Keep the registration-list adjustment local to its name draw call; card
// names and status-screen names use the same renderer at different positions.
const REGISTERED_LIST_NAME_X_OFFSET: usize = 0x2c18;
const REGISTERED_LIST_NAME_X: i16 = 62;

const NAME_RENDERER_ENTRY_OFFSET: usize = 0x1f74;
const NAME_RENDERER_ENTRY_HOOK_BYTE_COUNT: usize = 0x08;
const NAME_RENDERER_CONTINUATION_ADDRESS: u32 = OVERLAY_RUNTIME_BASE
    + NAME_RENDERER_ENTRY_OFFSET as u32
    + NAME_RENDERER_ENTRY_HOOK_BYTE_COUNT as u32;
const NAME_CODE_HOOK_OFFSET: usize = 0x1fc4;
const NAME_CODE_HOOK_BYTE_COUNT: usize = 0x20;
const STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_INSTRUCTION_OFFSET: usize = 0x5a74;
const STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_SOURCE_CODE: u16 = 0x00a9;
const STATIC_GLYPH_REFRESH_HOOK_OFFSETS: [usize; 2] = [0x5ea0, 0x62b0];
const STATIC_GLYPH_REFRESH_HOOK_BYTE_COUNT: usize = 0x04;
const STATUS_OVERVIEW_NATIVE_SETUP_ADDRESS: u32 = 0x800a_71b4;
const NATIVE_ENTRYPOINT: u32 = 0x800a_364c;
const LEGACY_NAME_MAPPING_TABLE_ADDRESS: u32 = 0x800a_2d74;
const LEGACY_NAME_CODE_COUNT: i16 = 0x061e;
const LEGACY_PADDING_CODE: u16 = 0x061e;
const TAG_MASK: u16 = 0xc000;
const HANGUL_PAYLOAD_MASK: u16 = 0x3fff;
const ASCII_PAYLOAD_MASK: u16 = 0x007f;
const FIXED_UI_NAME_TAG: u16 = 0xc000;
pub(super) const KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE: u16 = FIXED_UI_NAME_TAG | 1;
const SKIP_CODE: u16 = 0x0fff;
const MESSAGE_END_CODE: u16 = 0x3001;
const NAME_RENDERER_GLYPH_LIMIT: u16 = 5;
const PRIMARY_NICKNAME_ADDRESS: u32 = 0x801f_1896;

const NAME_ENTRY_FONT_CATALOG_INDEX: u16 = 718;
const NAME_ENTRY_FONT_CATALOG_ENTRY_OFFSET: usize = 0x0007_9090;
const NAME_ENTRY_FONT_CATALOG_ENTRY: [u8; 12] = [
    0x06, 0x18, 0x48, 0x00, 0xc2, 0xac, 0x01, 0x00, 0x00, 0x00, 0x10, 0x00,
];
const NATIVE_RECORD_LOADER_ADDRESS: u32 = 0x8001_5414;
// EDITMOJI ends at 800ff800. Keep the transient font above every staged
// initializer/table, including the appended roster supplier ignored by KANRI.
const NAME_ENTRY_TRANSIENT_LOAD_ORIGIN: u32 = 0x8010_0000;
const NAME_ENTRY_TRANSIENT_LOAD_END: u32 = 0x8014_0000;

const SCRATCH_GLYPH_ROW_BYTES: i16 = 10;
const SCRATCH_GLYPH_HEIGHT: u16 = 20;
pub(super) const KANRI_MENU_ATLAS_VRAM_WORD_X: i16 = 768;
// Derive private ASCII and imported-name addresses from the complete program reservation.
const FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY: usize = 8000;
const _: () = assert!(STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN == 0x800a_bd00);
const _: () = assert!(STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY == 0x4300);
const PRIVATE_ASCII_PERSISTENT_ADDRESS: u32 =
    STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN + FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY as u32;
const CACHE_UPLOAD_STACK_BYTES: i16 = 240;
const CACHE_UPLOAD_PIXELS_OFFSET: i16 = 8;
const CACHE_UPLOAD_S2_OFFSET: i16 = 224;
const CACHE_UPLOAD_S1_OFFSET: i16 = 228;
const CACHE_UPLOAD_S0_OFFSET: i16 = 232;
const CACHE_UPLOAD_RA_OFFSET: i16 = 236;

#[derive(Clone, Debug)]
pub(super) struct KanriStaticGlyph {
    pub(super) character: char,
    pub(super) code: u16,
    pub(super) payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct KanriFixedUiGlyphAlias {
    pub(super) source_instruction_offset: usize,
    pub(super) source_code: u16,
    pub(super) output_code: u16,
    pub(super) renderer_input_code: u16,
}

fn validate_spirit_gauge_unit_alias(
    source_overlay: &[u8],
    alias: KanriFixedUiGlyphAlias,
) -> Result<()> {
    ensure!(
        alias.source_instruction_offset == STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_INSTRUCTION_OFFSET
            && alias.source_code == STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_SOURCE_CODE
            && alias.renderer_input_code == KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE
            && alias.output_code < 0x0400,
        "KANRI spirit-gauge unit asset differs from its source-bound consumer"
    );
    let expected = [
        (
            0x5a5c,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::T0,
                offset: 0x0c,
            },
        ),
        (
            0x5a64,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x5a68,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x5a6c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x5a70,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x35e4,
            },
        ),
        (
            STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_INSTRUCTION_OFFSET,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_SOURCE_CODE as i16,
            },
        ),
        (
            0x5a78,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x2a,
            },
        ),
        (
            0x5a7c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: MESSAGE_END_CODE as i16,
            },
        ),
        (
            0x5a80,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x2c,
            },
        ),
        (
            0x5a88,
            Instruction::Sh {
                rt: Register::V1,
                base: Register::SP,
                offset: 0x28,
            },
        ),
        (
            0x5a90,
            Instruction::Jal {
                target: OVERLAY_RUNTIME_BASE + NAME_RENDERER_ENTRY_OFFSET as u32,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        ensure!(
            decode_instruction(source_overlay, offset)? == expected_instruction,
            "KANRI spirit-gauge unit consumer grammar changed at +0x{offset:04x}"
        );
    }
    ensure!(
        read_u32(source_overlay, 0x82dc)? == OVERLAY_RUNTIME_BASE + 0x5a38,
        "KANRI spirit-gauge row no longer selects its direct unit renderer"
    );
    Ok(())
}

#[derive(Debug)]
pub(super) struct KanriTaggedNameRuntimeInstall {
    pub(super) report: KanriTaggedNameRuntimeReport,
    pub(super) overlay_write_claims: Vec<DecodedDataClaim>,
    pub(super) edit_shared_ui_write_claims: Vec<DecodedDataClaim>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KanriTaggedNameRuntimeReport {
    pub storage_layout: KanriNameStorageLayout,
    pub initial_glyph_count: usize,
    pub ui_hangul_glyph_count: usize,
    #[serde(default)]
    pub password_glyph_count: usize,
    pub private_ascii_glyph_count: usize,
    pub cache_slot_count: usize,
    pub common_ascii_glyph_count: usize,
    pub display_codes: Vec<String>,
    pub initialization_data_sha256: String,
    pub static_hangul_payload_staging_decoded_offset: String,
    pub static_hangul_payload_persistent_address: String,
    pub static_hangul_payload_persistent_byte_capacity: usize,
    pub static_hangul_payload_byte_count: usize,
    pub static_hangul_payload_sha256: String,
    pub private_ascii_payload_sha256: String,
    pub private_ascii_persistent_address: String,
    pub ascii_code_table_sha256: String,
    pub imported_direct_code_base: u16,
    pub imported_direct_map_address: String,
    pub imported_direct_map_sha256: String,
    pub imported_direct_map_byte_count: usize,
    pub name_entry_font_path: String,
    pub name_entry_font_catalog_index: u16,
    pub name_entry_font_stored_sha256: String,
    pub name_entry_font_decoded_sha256: String,
    pub name_entry_font_pixel_address: String,
    pub copied_pack_storage_sha256: String,
    pub copied_pack_matches_materialization_bundle: bool,
    pub initializer_program_decoded_offset: String,
    pub initializer_program_address: String,
    pub initializer_program_byte_count: usize,
    pub initializer_typed_instruction_count: usize,
    pub materializer_program_staging_decoded_offset: String,
    pub materializer_program_address: String,
    pub materializer_program_byte_count: usize,
    pub materializer_typed_instruction_count: usize,
    pub materializer_address: String,
    pub resolver_program_staging_decoded_offset: String,
    pub resolver_program_address: String,
    pub resolver_program_byte_count: usize,
    pub resolver_program_typed_instruction_count: usize,
    pub static_refresher_program_staging_decoded_offset: String,
    pub static_refresher_program_address: String,
    pub static_refresher_program_byte_count: usize,
    pub static_refresher_program_typed_instruction_count: usize,
    pub static_glyph_unpacker_address: String,
    pub static_glyph_refresh_hook_offsets: Vec<String>,
    pub all_native_ui_setup_calls_refresh: bool,
    pub status_overview_static_glyph_refresh_installed: bool,
    pub status_overview_spirit_gauge_unit_instruction_offset: String,
    pub status_overview_spirit_gauge_unit_source_code: String,
    pub status_overview_spirit_gauge_unit_output_code: String,
    pub status_overview_spirit_gauge_unit_renderer_input_code: String,
    pub status_overview_spirit_gauge_unit_consumer_verified: bool,
    pub status_overview_spirit_gauge_unit_uses_fixed_static_glyph: bool,
    #[serde(default)]
    pub fixed_glyphs_use_lossless_full_cell_payloads: bool,
    pub static_hangul_uses_dynamic_component_materializer: bool,
    pub static_hangul_persists_outside_reused_editmoji_buffer: bool,
    pub status_overview_name_cache_invalidated_before_rendering: bool,
    pub tagged_code_resolver_address: String,
    pub card_name_normalizer_address: String,
    pub registered_name_normalizer_address: String,
    pub cache_uploader_address: String,
    pub name_renderer_prewarm_wrapper_address: String,
    pub runtime_program_address_range: [String; 2],
    pub programs_copied_after_native_parsers: bool,
    pub import_parser_hook_offset: String,
    pub name_renderer_entry_hook_offset: String,
    pub name_code_hook_offset: String,
    pub name_renderer_source_pointer_forwarded: bool,
    pub current_name_prewarmed_before_rendering: bool,
    pub name_code_lookup_performs_no_vram_uploads: bool,
    pub renderer_cache_misses_are_side_effect_free: bool,
    pub cache_upload_copy_uses_preserved_pointers: bool,
    pub stack_backed_vram_uploads_synchronized: bool,
    pub name_renderer_prologue_preserved: bool,
    pub source_hooks_verified: bool,
    pub typed_programs_verified: bool,
    pub writes_confined_to_owned_ranges: bool,
    pub legacy_name_mapping_preserved: bool,
    pub nickname_companion_cache_code_range: [String; 2],
    pub nickname_companion_uses_live_primary_record: bool,
    pub tagged_ascii_uses_common_menu_codes: bool,
    pub name_entry_loaded_through_native_catalog: bool,
    pub glyph_pack_copied_to_mode_local_arena: bool,
    pub native_ui_parsed_before_transient_name_entry_load: bool,
    pub tagged_hangul_materialization_installed: bool,
    pub runtime_execution_verified: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn install_kanri_tagged_name_runtime(
    source_overlay: &[u8],
    output_overlay: &mut [u8],
    source_edit_shared_ui: &[u8],
    output_edit_shared_ui: &mut [u8],
    source_main_executable: &[u8],
    source_name_entry_font_record: &FileRecord,
    source_name_entry_font_stored: &[u8],
    patched_name_entry_font_stored: &[u8],
    patched_name_entry_font_decoded_sha256: &str,
    name_entry_runtime_atlas: &NameInputRuntimeAtlasLayout,
    name_glyph_materialization: &NameGlyphMaterializationBundle,
    name_glyph_consumer_layout: &NameGlyphConsumerLayout,
    runtime_coordinate_list_storage_byte_range: [usize; 2],
    storage_layout: &KanriNameStorageLayout,
    static_glyphs: &[KanriStaticGlyph],
    password_glyphs: &[KanriStaticGlyph],
    dynamic_display_codes: &[u16],
    spirit_gauge_unit_alias: KanriFixedUiGlyphAlias,
    outline_pixel_address: u32,
    outline_cleanup_address: u32,
) -> Result<KanriTaggedNameRuntimeInstall> {
    validate_source_layout(
        source_overlay,
        source_edit_shared_ui,
        source_main_executable,
        source_name_entry_font_record,
        source_name_entry_font_stored,
    )?;
    ensure!(
        output_overlay.len() == source_overlay.len()
            && output_edit_shared_ui.len() == source_edit_shared_ui.len(),
        "KANRI tagged-name outputs changed a source record extent"
    );
    let expected_storage_layout = KanriNameStorageLayout::plan(
        name_glyph_materialization,
        runtime_coordinate_list_storage_byte_range,
    )?;
    ensure!(
        storage_layout == &expected_storage_layout,
        "KANRI mode-local storage layout differs from the MA_ENT materialization source"
    );
    ensure!(
        static_glyphs.len() <= KANRI_INITIAL_GLYPH_CAPACITY
            && dynamic_display_codes.len() == KANRI_NAME_CACHE_SLOT_COUNT
            && static_glyphs.iter().all(|glyph| {
                glyph.payload.len() == KANRI_GLYPH_PAYLOAD_BYTES && glyph.code < 0x0400
            })
            && dynamic_display_codes.iter().all(|code| *code < 0x0400),
        "KANRI initial or dynamic glyph population differs from its fixed runtime capacity"
    );
    validate_spirit_gauge_unit_alias(source_overlay, spirit_gauge_unit_alias)?;
    ensure!(
        static_glyphs
            .iter()
            .any(|glyph| glyph.code == spirit_gauge_unit_alias.output_code),
        "KANRI spirit-gauge unit has no exact fixed glyph payload"
    );
    ensure!(
        static_glyphs
            .iter()
            .map(|glyph| glyph.character)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == static_glyphs.len(),
        "KANRI initial glyph population repeats a character"
    );
    let mut display_codes = static_glyphs
        .iter()
        .map(|glyph| glyph.code)
        .chain(dynamic_display_codes.iter().copied())
        .collect::<Vec<_>>();
    let unique_display_codes = display_codes
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        display_codes.len() <= KANRI_DISPLAY_CODE_CAPACITY
            && unique_display_codes.len() == display_codes.len(),
        "KANRI display-code allocation contains an overlap"
    );

    let name_entry_source = inspect_name_entry_pack_source(
        patched_name_entry_font_stored,
        patched_name_entry_font_decoded_sha256,
        name_entry_runtime_atlas,
        name_glyph_materialization,
        runtime_coordinate_list_storage_byte_range,
    )?;
    let initialization_data = build_initialization_data(
        static_glyphs,
        dynamic_display_codes,
        name_entry_runtime_atlas,
    )?;
    let exact_static_hangul = build_exact_static_hangul_data(static_glyphs, password_glyphs)?;
    let direct_name_map = ImportedDirectNameMap::build(name_glyph_consumer_layout)?;
    ensure!(
        exact_static_hangul.packed_payloads.len()
            + KANRI_RAW_ASCII_GLYPH_COUNT * KANRI_GLYPH_PAYLOAD_BYTES
            == DIRECT_NAME_MAP_OFFSET,
        "KANRI direct-name table overlaps fixed glyph payloads"
    );
    ensure!(
        initialization_data.bytes.len() <= INITIALIZATION_DATA_STAGING_BYTE_CAPACITY,
        "KANRI initialization data exceeds its source-zero EDITMOJI staging region"
    );

    let materializer_program_address = MATERIALIZER_PROGRAM_ORIGIN;
    let resolver_program_address = RESOLVER_PROGRAM_ORIGIN;
    let mut materializer = match &name_glyph_materialization.format {
        crate::name_input::NameGlyphMaterializationFormat::Components(report) => {
            let runtime_pack = crate::name_input::plan_name_input_runtime_pack(report)?;
            build_materializer_program(
                materializer_program_address,
                storage_layout,
                &runtime_pack,
                outline_pixel_address,
                outline_cleanup_address,
            )?
        }
        crate::name_input::NameGlyphMaterializationFormat::Bands => {
            let pack = crate::name_input::NameGlyphBandPack::parse(
                name_glyph_materialization.pack_bytes.clone(),
            )?;
            build_band_materializer_program(
                materializer_program_address,
                storage_layout,
                &pack,
                outline_pixel_address,
                outline_cleanup_address,
            )?
        }
    };
    ensure!(
        materializer.bytes.len() <= MATERIALIZER_PROGRAM_BYTE_CAPACITY
            && materializer.bytes.len() <= MATERIALIZER_STAGING_BYTE_CAPACITY,
        "KANRI materializer exceeds its staging or mode-local runtime region"
    );
    let nickname_companion_cache_code_range =
        nickname_companion_cache_code_range(name_glyph_consumer_layout)?;
    let resolver = build_resolver_program(
        resolver_program_address,
        storage_layout,
        materializer.materializer_address,
        nickname_companion_cache_code_range,
        spirit_gauge_unit_alias.output_code,
        &direct_name_map,
    )?;
    ensure!(
        resolver.bytes.len() <= RESOLVER_PROGRAM_BYTE_CAPACITY
            && resolver.bytes.len() <= RESOLVER_STAGING_BYTE_CAPACITY,
        "KANRI tagged-code resolver exceeds its staging or mode-local runtime region"
    );
    let diary_name_transfer_address =
        materializer_program_address + materializer.bytes.len() as u32;
    let mut transfer_assembler = Assembler::new();
    emit_diary_name_transfer(
        &mut transfer_assembler,
        diary_name_transfer_address,
        resolver.card_name_normalizer_address,
    )?;
    let transfer = place_program(
        &transfer_assembler,
        diary_name_transfer_address,
        "Diary name transfer",
    )?;
    materializer.bytes.extend_from_slice(&transfer.bytes);
    materializer.instructions.extend(transfer.instructions);
    ensure!(
        materializer.bytes.len() <= MATERIALIZER_PROGRAM_BYTE_CAPACITY
            && materializer.bytes.len() <= MATERIALIZER_STAGING_BYTE_CAPACITY,
        "KANRI materializer and Diary name transfer exceed owned storage"
    );
    let static_refresher = build_static_refresher_program(
        STATIC_REFRESHER_PROGRAM_ORIGIN,
        storage_layout,
        &exact_static_hangul,
    )?;
    ensure!(
        static_refresher.bytes.len() <= STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY
            && static_refresher.bytes.len() <= STATIC_REFRESHER_STAGING_BYTE_CAPACITY,
        "KANRI static-glyph refresher exceeds its staging or mode-local runtime region"
    );
    let initializer_address = edit_shared_ui_runtime_address(INITIALIZER_OFFSET)?;
    let runtime_program_copies = [
        RuntimeProgramCopy {
            source_address: edit_shared_ui_runtime_address(MATERIALIZER_STAGING_OFFSET)?,
            destination_address: materializer_program_address,
            byte_count: materializer.bytes.len(),
            loop_label: "copy_kanri_materializer_word",
        },
        RuntimeProgramCopy {
            source_address: edit_shared_ui_runtime_address(RESOLVER_STAGING_OFFSET)?,
            destination_address: resolver_program_address,
            byte_count: resolver.bytes.len(),
            loop_label: "copy_kanri_resolver_word",
        },
        RuntimeProgramCopy {
            source_address: edit_shared_ui_runtime_address(STATIC_REFRESHER_STAGING_OFFSET)?,
            destination_address: STATIC_REFRESHER_PROGRAM_ORIGIN,
            byte_count: static_refresher.bytes.len(),
            loop_label: "copy_kanri_static_refresher_word",
        },
        RuntimeProgramCopy {
            source_address: exact_static_hangul.staging_address()?,
            destination_address: exact_static_hangul.persistent_address(),
            byte_count: DIRECT_NAME_MAP_OFFSET + direct_name_map.bytes.len(),
            loop_label: "copy_kanri_static_payload_word",
        },
    ];
    let initializer = build_initializer_program(
        initializer_address,
        storage_layout,
        &runtime_program_copies,
        &initialization_data,
        name_entry_source.font_pixel_runtime_address,
        &exact_static_hangul,
        &static_refresher,
    )?;
    ensure!(
        initializer.bytes.len() <= INITIALIZER_BYTE_CAPACITY,
        "KANRI initializer exceeds its source-zero EDITMOJI region"
    );

    let before_overlay = output_overlay.to_vec();
    let before_edit_shared_ui = output_edit_shared_ui.to_vec();

    let card_name_hook = Instruction::Jal {
        target: resolver.card_name_normalizer_address,
    };
    write_instruction(
        output_overlay,
        REGISTERED_LIST_NAME_X_OFFSET,
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::A3,
            immediate: REGISTERED_LIST_NAME_X,
        },
    )?;
    let registered_name_hook = Instruction::J {
        target: resolver.registered_name_normalizer_address,
    };
    write_instruction(
        output_overlay,
        CARD_NAME_POINTER_OFFSET,
        card_name_hook.clone(),
    )?;
    write_instruction(
        output_overlay,
        REGISTER_NAME_LOAD_OFFSET,
        registered_name_hook.clone(),
    )?;
    let diary_name_hook = Instruction::J {
        target: diary_name_transfer_address,
    };
    write_instruction(output_overlay, 0x60f8, diary_name_hook.clone())?;
    write_instruction(output_overlay, 0x60fc, Instruction::nop())?;
    let parser_hook = Instruction::Jal {
        target: initializer_address,
    };
    write_instruction(
        output_overlay,
        LAST_IMPORT_PARSER_HOOK_OFFSET,
        parser_hook.clone(),
    )?;
    let name_renderer_entry_hook =
        name_renderer_entry_hook(resolver.name_renderer_prewarm_wrapper_address);
    for (index, instruction) in name_renderer_entry_hook.iter().cloned().enumerate() {
        write_instruction(
            output_overlay,
            NAME_RENDERER_ENTRY_OFFSET + index * 4,
            instruction,
        )?;
    }
    let name_hook = name_code_hook(resolver.name_draw_resolver_address);
    for (index, instruction) in name_hook.iter().cloned().enumerate() {
        write_instruction(
            output_overlay,
            NAME_CODE_HOOK_OFFSET + index * 4,
            instruction,
        )?;
    }
    let static_glyph_refresh_hook = Instruction::Jal {
        target: static_refresher.entry_address,
    };
    for offset in STATIC_GLYPH_REFRESH_HOOK_OFFSETS {
        write_instruction(output_overlay, offset, static_glyph_refresh_hook.clone())?;
    }
    let spirit_gauge_unit_instruction = Instruction::Addiu {
        rt: Register::V0,
        rs: Register::ZERO,
        immediate: spirit_gauge_unit_alias.renderer_input_code as i16,
    };
    write_instruction(
        output_overlay,
        spirit_gauge_unit_alias.source_instruction_offset,
        spirit_gauge_unit_instruction.clone(),
    )?;
    output_edit_shared_ui[INITIALIZER_OFFSET..INITIALIZER_OFFSET + initializer.bytes.len()]
        .copy_from_slice(&initializer.bytes);
    output_edit_shared_ui
        [MATERIALIZER_STAGING_OFFSET..MATERIALIZER_STAGING_OFFSET + materializer.bytes.len()]
        .copy_from_slice(&materializer.bytes);
    output_edit_shared_ui[RESOLVER_STAGING_OFFSET..RESOLVER_STAGING_OFFSET + resolver.bytes.len()]
        .copy_from_slice(&resolver.bytes);
    output_edit_shared_ui[STATIC_REFRESHER_STAGING_OFFSET
        ..STATIC_REFRESHER_STAGING_OFFSET + static_refresher.bytes.len()]
        .copy_from_slice(&static_refresher.bytes);
    output_edit_shared_ui[INITIALIZATION_DATA_STAGING_OFFSET
        ..INITIALIZATION_DATA_STAGING_OFFSET + initialization_data.bytes.len()]
        .copy_from_slice(&initialization_data.bytes);
    output_edit_shared_ui[STATIC_HANGUL_PAYLOAD_STAGING_OFFSET
        ..STATIC_HANGUL_PAYLOAD_STAGING_OFFSET + exact_static_hangul.packed_payloads.len()]
        .copy_from_slice(&exact_static_hangul.packed_payloads);

    let private_ascii_start =
        STATIC_HANGUL_PAYLOAD_STAGING_OFFSET + exact_static_hangul.packed_payloads.len();
    let private_ascii_payload = &initialization_data.bytes[initialization_data
        .private_ascii_payload_range[0]
        ..initialization_data.private_ascii_payload_range[1]];
    output_edit_shared_ui[private_ascii_start..private_ascii_start + private_ascii_payload.len()]
        .copy_from_slice(private_ascii_payload);
    let direct_name_map_start = STATIC_HANGUL_PAYLOAD_STAGING_OFFSET + DIRECT_NAME_MAP_OFFSET;
    output_edit_shared_ui
        [direct_name_map_start..direct_name_map_start + direct_name_map.bytes.len()]
        .copy_from_slice(&direct_name_map.bytes);

    let mut overlay_expected_ranges = vec![
        [0x60f8, 0x6100],
        [
            REGISTERED_LIST_NAME_X_OFFSET,
            REGISTERED_LIST_NAME_X_OFFSET + 4,
        ],
        [CARD_NAME_POINTER_OFFSET, CARD_NAME_POINTER_OFFSET + 4],
        [REGISTER_NAME_LOAD_OFFSET, REGISTER_NAME_LOAD_OFFSET + 4],
        [
            LAST_IMPORT_PARSER_HOOK_OFFSET,
            LAST_IMPORT_PARSER_HOOK_OFFSET + 4,
        ],
        [
            NAME_RENDERER_ENTRY_OFFSET,
            NAME_RENDERER_ENTRY_OFFSET + NAME_RENDERER_ENTRY_HOOK_BYTE_COUNT,
        ],
        [
            NAME_CODE_HOOK_OFFSET,
            NAME_CODE_HOOK_OFFSET + NAME_CODE_HOOK_BYTE_COUNT,
        ],
    ];
    overlay_expected_ranges.extend(
        STATIC_GLYPH_REFRESH_HOOK_OFFSETS
            .map(|offset| [offset, offset + STATIC_GLYPH_REFRESH_HOOK_BYTE_COUNT]),
    );
    overlay_expected_ranges.push([
        spirit_gauge_unit_alias.source_instruction_offset,
        spirit_gauge_unit_alias.source_instruction_offset + 4,
    ]);
    let edit_shared_ui_expected_ranges = vec![
        [
            private_ascii_start,
            private_ascii_start + private_ascii_payload.len(),
        ],
        [
            direct_name_map_start,
            direct_name_map_start + direct_name_map.bytes.len(),
        ],
        [
            INITIALIZER_OFFSET,
            INITIALIZER_OFFSET + initializer.bytes.len(),
        ],
        [
            MATERIALIZER_STAGING_OFFSET,
            MATERIALIZER_STAGING_OFFSET + materializer.bytes.len(),
        ],
        [
            RESOLVER_STAGING_OFFSET,
            RESOLVER_STAGING_OFFSET + resolver.bytes.len(),
        ],
        [
            STATIC_REFRESHER_STAGING_OFFSET,
            STATIC_REFRESHER_STAGING_OFFSET + static_refresher.bytes.len(),
        ],
        [
            INITIALIZATION_DATA_STAGING_OFFSET,
            INITIALIZATION_DATA_STAGING_OFFSET + initialization_data.bytes.len(),
        ],
        [
            STATIC_HANGUL_PAYLOAD_STAGING_OFFSET,
            STATIC_HANGUL_PAYLOAD_STAGING_OFFSET + exact_static_hangul.packed_payloads.len(),
        ],
    ];
    let writes_confined_to_owned_ranges = changed_ranges_are_within(
        &difference_ranges(&before_overlay, output_overlay),
        &overlay_expected_ranges,
    ) && changed_ranges_are_within(
        &difference_ranges(&before_edit_shared_ui, output_edit_shared_ui),
        &edit_shared_ui_expected_ranges,
    );
    ensure!(
        writes_confined_to_owned_ranges,
        "KANRI tagged-name installation escaped its Expected Writes"
    );
    ensure!(
        decode_instruction(output_overlay, 0x60f8)? == diary_name_hook
            && decode_instruction(output_overlay, 0x60fc)? == Instruction::nop()
            && decode_instruction(output_overlay, CARD_NAME_POINTER_OFFSET)? == card_name_hook
            && decode_instruction(output_overlay, REGISTER_NAME_LOAD_OFFSET)?
                == registered_name_hook
            && decode_instruction(output_overlay, LAST_IMPORT_PARSER_HOOK_OFFSET)? == parser_hook
            && name_renderer_entry_hook
                .iter()
                .cloned()
                .enumerate()
                .all(|(index, expected)| {
                    decode_instruction(output_overlay, NAME_RENDERER_ENTRY_OFFSET + index * 4)
                        .is_ok_and(|actual| actual == expected)
                })
            && name_hook
                .iter()
                .cloned()
                .enumerate()
                .all(|(index, expected)| {
                    decode_instruction(output_overlay, NAME_CODE_HOOK_OFFSET + index * 4)
                        .is_ok_and(|actual| actual == expected)
                })
            && STATIC_GLYPH_REFRESH_HOOK_OFFSETS.iter().all(|offset| {
                decode_instruction(output_overlay, *offset)
                    .is_ok_and(|actual| actual == static_glyph_refresh_hook)
            })
            && decode_instruction(
                output_overlay,
                spirit_gauge_unit_alias.source_instruction_offset,
            )? == spirit_gauge_unit_instruction,
        "KANRI tagged-name hooks failed typed readback"
    );

    let report = KanriTaggedNameRuntimeReport {
        storage_layout: storage_layout.clone(),
        initial_glyph_count: static_glyphs.len(),
        ui_hangul_glyph_count: static_glyphs
            .iter()
            .filter(|g| is_ks_x_1001_hangul(g.character))
            .count(),
        password_glyph_count: password_glyphs.len(),
        private_ascii_glyph_count: KANRI_PRIVATE_ASCII_GLYPH_COUNT,
        cache_slot_count: dynamic_display_codes.len(),
        common_ascii_glyph_count: initialization_data.common_ascii_glyph_count,
        display_codes: display_codes
            .drain(..)
            .map(|code| format!("0x{code:04x}"))
            .collect(),
        initialization_data_sha256: sha256_bytes(&initialization_data.bytes),
        static_hangul_payload_staging_decoded_offset: format!(
            "0x{STATIC_HANGUL_PAYLOAD_STAGING_OFFSET:05x}"
        ),
        static_hangul_payload_persistent_address: format!(
            "0x{STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN:08x}"
        ),
        static_hangul_payload_persistent_byte_capacity:
            STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY,
        static_hangul_payload_byte_count: exact_static_hangul.packed_payloads.len(),
        static_hangul_payload_sha256: sha256_bytes(&exact_static_hangul.packed_payloads),
        private_ascii_persistent_address: format!("0x{PRIVATE_ASCII_PERSISTENT_ADDRESS:08x}"),
        private_ascii_payload_sha256: sha256_bytes(
            &initialization_data.bytes[initialization_data.private_ascii_payload_range[0]
                ..initialization_data.private_ascii_payload_range[1]],
        ),
        imported_direct_code_base: direct_name_map.base,
        imported_direct_map_address: format!("0x{DIRECT_NAME_MAP_ADDRESS:08x}"),
        imported_direct_map_sha256: sha256_bytes(&direct_name_map.bytes),
        imported_direct_map_byte_count: direct_name_map.bytes.len(),
        ascii_code_table_sha256: sha256_bytes(
            &initialization_data.bytes[initialization_data.ascii_code_table_range[0]
                ..initialization_data.ascii_code_table_range[1]],
        ),
        name_entry_font_path: NAME_ENTRY_FONT_PATH.to_string(),
        name_entry_font_catalog_index: NAME_ENTRY_FONT_CATALOG_INDEX,
        name_entry_font_stored_sha256: sha256_bytes(patched_name_entry_font_stored),
        name_entry_font_decoded_sha256: name_entry_source.decoded_sha256,
        name_entry_font_pixel_address: format!(
            "0x{:08x}",
            name_entry_source.font_pixel_runtime_address
        ),
        copied_pack_storage_sha256: sha256_bytes(&name_entry_source.copied_pack_storage),
        copied_pack_matches_materialization_bundle: true,
        initializer_program_decoded_offset: format!("0x{INITIALIZER_OFFSET:05x}"),
        initializer_program_address: format!("0x{initializer_address:08x}"),
        initializer_program_byte_count: initializer.bytes.len(),
        initializer_typed_instruction_count: initializer.instructions.len(),
        materializer_program_staging_decoded_offset: format!("0x{MATERIALIZER_STAGING_OFFSET:05x}"),
        materializer_program_address: format!("0x{materializer_program_address:08x}"),
        materializer_program_byte_count: materializer.bytes.len(),
        materializer_typed_instruction_count: materializer.instructions.len(),
        materializer_address: format!("0x{:08x}", materializer.materializer_address),
        resolver_program_staging_decoded_offset: format!("0x{RESOLVER_STAGING_OFFSET:05x}"),
        resolver_program_address: format!("0x{resolver_program_address:08x}"),
        resolver_program_byte_count: resolver.bytes.len(),
        resolver_program_typed_instruction_count: resolver.instructions.len(),
        static_refresher_program_staging_decoded_offset: format!(
            "0x{STATIC_REFRESHER_STAGING_OFFSET:05x}"
        ),
        static_refresher_program_address: format!("0x{STATIC_REFRESHER_PROGRAM_ORIGIN:08x}"),
        static_refresher_program_byte_count: static_refresher.bytes.len(),
        static_refresher_program_typed_instruction_count: static_refresher.instructions.len(),
        static_glyph_unpacker_address: format!("0x{:08x}", static_refresher.unpacker_address),
        static_glyph_refresh_hook_offsets: STATIC_GLYPH_REFRESH_HOOK_OFFSETS
            .iter()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        all_native_ui_setup_calls_refresh: true,
        status_overview_static_glyph_refresh_installed: true,
        status_overview_spirit_gauge_unit_instruction_offset: format!(
            "0x{:04x}",
            spirit_gauge_unit_alias.source_instruction_offset
        ),
        status_overview_spirit_gauge_unit_source_code: format!(
            "0x{:04x}",
            spirit_gauge_unit_alias.source_code
        ),
        status_overview_spirit_gauge_unit_output_code: format!(
            "0x{:04x}",
            spirit_gauge_unit_alias.output_code
        ),
        status_overview_spirit_gauge_unit_renderer_input_code: format!(
            "0x{:04x}",
            spirit_gauge_unit_alias.renderer_input_code
        ),
        status_overview_spirit_gauge_unit_consumer_verified: true,
        status_overview_spirit_gauge_unit_uses_fixed_static_glyph: true,
        fixed_glyphs_use_lossless_full_cell_payloads: true,
        static_hangul_uses_dynamic_component_materializer: false,
        static_hangul_persists_outside_reused_editmoji_buffer: true,
        status_overview_name_cache_invalidated_before_rendering: true,
        tagged_code_resolver_address: format!("0x{:08x}", resolver.tagged_code_resolver_address),
        card_name_normalizer_address: format!("0x{:08x}", resolver.card_name_normalizer_address),
        registered_name_normalizer_address: format!(
            "0x{:08x}",
            resolver.registered_name_normalizer_address
        ),
        cache_uploader_address: format!("0x{:08x}", resolver.cache_uploader_address),
        name_renderer_prewarm_wrapper_address: format!(
            "0x{:08x}",
            resolver.name_renderer_prewarm_wrapper_address
        ),
        runtime_program_address_range: [
            format!("0x{MATERIALIZER_PROGRAM_ORIGIN:08x}"),
            format!(
                "0x{:08x}",
                STATIC_REFRESHER_PROGRAM_ORIGIN + u32::try_from(static_refresher.bytes.len())?
            ),
        ],
        programs_copied_after_native_parsers: true,
        import_parser_hook_offset: format!("0x{LAST_IMPORT_PARSER_HOOK_OFFSET:04x}"),
        name_renderer_entry_hook_offset: format!("0x{NAME_RENDERER_ENTRY_OFFSET:04x}"),
        name_code_hook_offset: format!("0x{NAME_CODE_HOOK_OFFSET:04x}"),
        name_renderer_source_pointer_forwarded: true,
        current_name_prewarmed_before_rendering: true,
        name_code_lookup_performs_no_vram_uploads: true,
        renderer_cache_misses_are_side_effect_free: true,
        cache_upload_copy_uses_preserved_pointers: true,
        stack_backed_vram_uploads_synchronized: true,
        name_renderer_prologue_preserved: true,
        source_hooks_verified: true,
        typed_programs_verified: true,
        writes_confined_to_owned_ranges,
        legacy_name_mapping_preserved: true,
        nickname_companion_cache_code_range: [
            format!("0x{:04x}", nickname_companion_cache_code_range[0]),
            format!("0x{:04x}", nickname_companion_cache_code_range[1] - 1),
        ],
        nickname_companion_uses_live_primary_record: true,
        tagged_ascii_uses_common_menu_codes: true,
        name_entry_loaded_through_native_catalog: true,
        glyph_pack_copied_to_mode_local_arena: true,
        native_ui_parsed_before_transient_name_entry_load: true,
        tagged_hangul_materialization_installed: true,
        runtime_execution_verified: false,
    };
    Ok(KanriTaggedNameRuntimeInstall {
        report,
        overlay_write_claims: DecodedDataClaim::from_ranges(
            "kanri:tagged-name-hooks",
            "route KANRI static initialization and dynamic names through the tagged-name runtime",
            overlay_expected_ranges,
        ),
        edit_shared_ui_write_claims: DecodedDataClaim::from_ranges(
            "edit-shared-ui:tagged-name-runtime",
            "stage the KANRI runtime and its native MA_ENT copy initializer",
            edit_shared_ui_expected_ranges,
        ),
    })
}

#[derive(Debug)]
struct InitializationData {
    bytes: Vec<u8>,
    pack_lookup_range: [usize; 2],
    ascii_code_table_range: [usize; 2],
    dynamic_display_code_table_range: [usize; 2],
    private_ascii_payload_range: [usize; 2],
    common_ascii_glyph_count: usize,
}

#[derive(Debug)]
struct ExactStaticHangulData {
    codes: Vec<u16>,
    packed_payloads: Vec<u8>,
}

impl ExactStaticHangulData {
    fn staging_address(&self) -> Result<u32> {
        edit_shared_ui_runtime_address(STATIC_HANGUL_PAYLOAD_STAGING_OFFSET)
    }

    fn persistent_address(&self) -> u32 {
        STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN
    }
}

impl InitializationData {
    fn runtime_address(&self, relative_offset: usize) -> Result<u32> {
        ensure!(
            relative_offset <= self.bytes.len(),
            "KANRI initialization-data address leaves its staged bytes"
        );
        edit_shared_ui_runtime_address(INITIALIZATION_DATA_STAGING_OFFSET + relative_offset)
    }
}

#[derive(Debug)]
struct NameEntryPackSource {
    decoded_sha256: String,
    font_pixel_runtime_address: u32,
    copied_pack_storage: Vec<u8>,
}

fn build_initialization_data(
    initial_glyphs: &[KanriStaticGlyph],
    dynamic_display_codes: &[u16],
    runtime_atlas: &NameInputRuntimeAtlasLayout,
) -> Result<InitializationData> {
    ensure!(
        runtime_atlas.lookup_table_bytes.len() == NAME_GLYPH_PACK_CELL_COUNT * 2
            && dynamic_display_codes.len() == KANRI_NAME_CACHE_SLOT_COUNT,
        "KANRI initialization tables differ from their fixed populations"
    );
    let hangul_glyphs = initial_glyphs
        .iter()
        .filter(|glyph| is_ks_x_1001_hangul(glyph.character))
        .collect::<Vec<_>>();
    ensure!(
        !hangul_glyphs.is_empty() && hangul_glyphs.len() <= KANRI_UI_HANGUL_GLYPH_CAPACITY,
        "KANRI fixed UI exceeds its reserved Hangul capacity"
    );
    let private_ascii_glyphs = KANRI_PRIVATE_ASCII_GLYPHS
        .chars()
        .map(|character| {
            initial_glyphs
                .iter()
                .find(|glyph| glyph.character == character)
                .with_context(|| format!("KANRI private ASCII glyph {character:?} is missing"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        private_ascii_glyphs.len() == KANRI_PRIVATE_ASCII_GLYPH_COUNT
            && initial_glyphs.len() <= KANRI_INITIAL_GLYPH_CAPACITY
            && initial_glyphs.iter().all(|glyph| {
                is_ks_x_1001_hangul(glyph.character)
                    || KANRI_PRIVATE_ASCII_GLYPHS.contains(glyph.character)
                    || (glyph.character != ' '
                        && (common_menu_ascii_glyph_code(glyph.character).is_some()
                            || crate::text::fixed_menu_glyph_code(glyph.character).is_some()))
            }),
        "KANRI initial glyph population contains an unowned character"
    );

    let private_ascii_codes = private_ascii_glyphs
        .iter()
        .map(|glyph| (glyph.character, glyph.code))
        .collect::<std::collections::BTreeMap<_, _>>();
    let admitted_ascii = LATIN_KEYS
        .chars()
        .chain(DIGIT_KEYS.chars())
        .chain(SYMBOL_KEYS.chars())
        .collect::<std::collections::BTreeSet<_>>();
    let mut ascii_codes = vec![u16::MAX; KANRI_ASCII_CODE_TABLE_ENTRY_COUNT];
    let mut common_ascii_glyph_count = 0;
    for character in admitted_ascii {
        let code = if let Some(code) = common_menu_ascii_glyph_code(character) {
            common_ascii_glyph_count += 1;
            code
        } else {
            *private_ascii_codes.get(&character).with_context(|| {
                format!("KANRI admitted ASCII glyph {character:?} has no verified consumer code")
            })?
        };
        ascii_codes[usize::try_from(u32::from(character))?] = code;
    }
    ensure!(
        ascii_codes[usize::from(b' ')] == SKIP_GLYPH_CODE
            && KANRI_PRIVATE_ASCII_GLYPHS.chars().all(|character| {
                ascii_codes[usize::try_from(u32::from(character)).unwrap()] != u16::MAX
            }),
        "KANRI ASCII table lost its skip or private glyph mapping"
    );

    let mut bytes = runtime_atlas.lookup_table_bytes.clone();
    let pack_lookup_range = [0, bytes.len()];
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    let ascii_code_table_start = bytes.len();
    for code in &ascii_codes {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    let ascii_code_table_range = [ascii_code_table_start, bytes.len()];
    let dynamic_display_code_table_start = bytes.len();
    for code in dynamic_display_codes {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    let dynamic_display_code_table_range = [dynamic_display_code_table_start, bytes.len()];
    let private_ascii_payload_start = bytes.len();
    for glyph in private_ascii_glyphs
        .into_iter()
        .filter(|g| KANRI_RAW_ASCII_GLYPHS.contains(g.character))
    {
        bytes.extend_from_slice(&glyph.payload);
    }
    let private_ascii_payload_range = [private_ascii_payload_start, bytes.len()];
    ensure!(
        ascii_code_table_range[1] - ascii_code_table_range[0]
            == KANRI_ASCII_CODE_TABLE_ENTRY_COUNT * 2
            && dynamic_display_code_table_range[1] - dynamic_display_code_table_range[0]
                == KANRI_NAME_CACHE_SLOT_COUNT * 2
            && private_ascii_payload_range[1] - private_ascii_payload_range[0]
                == KANRI_RAW_ASCII_GLYPH_COUNT * KANRI_GLYPH_PAYLOAD_BYTES,
        "KANRI initialization-data ranges differ from their runtime consumers"
    );
    Ok(InitializationData {
        bytes,
        pack_lookup_range,
        ascii_code_table_range,
        dynamic_display_code_table_range,
        private_ascii_payload_range,
        common_ascii_glyph_count,
    })
}

fn build_exact_static_hangul_data(
    initial_glyphs: &[KanriStaticGlyph],
    password_glyphs: &[KanriStaticGlyph],
) -> Result<ExactStaticHangulData> {
    let mut hangul_glyphs = initial_glyphs
        .iter()
        .filter(|glyph| is_ks_x_1001_hangul(glyph.character))
        .collect::<Vec<_>>();
    hangul_glyphs.sort_by_key(|glyph| glyph.character);
    ensure!(
        !hangul_glyphs.is_empty() && hangul_glyphs.len() <= KANRI_UI_HANGUL_GLYPH_CAPACITY,
        "KANRI exact static payload exceeds its reserved Hangul capacity"
    );

    ensure!(
        password_glyphs.is_empty() || password_glyphs.len() == 76,
        "password font must cover all 76 native values"
    );
    for glyph in password_glyphs {
        ensure!(
            super::password_alphabet::PasswordAlphabet::glyph_code(glyph.character)? == glyph.code,
            "password font changed its native display mapping"
        );
    }
    hangul_glyphs.extend(initial_glyphs.iter().filter(|g| {
        !is_ks_x_1001_hangul(g.character) && !KANRI_RAW_ASCII_GLYPHS.contains(g.character)
    }));
    hangul_glyphs.extend(password_glyphs.iter());
    let mut codes = Vec::with_capacity(hangul_glyphs.len());
    let mut payloads = Vec::new();
    for glyph in hangul_glyphs {
        codes.push(glyph.code);
        payloads.push(glyph.payload.as_slice());
    }
    let mut packed_payloads = pack_static_glyphs(&payloads)?;
    ensure!(
        packed_payloads.len() <= fixed_glyphs::TABLE_OFFSET,
        "KANRI fixed glyph stream needs {} bytes before its history",
        packed_payloads.len()
    );
    ensure!(
        packed_payloads.len() <= STATIC_HANGUL_PAYLOAD_STAGING_BYTE_CAPACITY
            && packed_payloads.len() <= STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY,
        "KANRI exact static payload exceeds its staging or persistent runtime region"
    );
    ensure!(
        packed_payloads.len() <= FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY,
        "KANRI packed fixed glyphs exceed their 8000-byte reservation"
    );
    ensure!(
        codes
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == codes.len(),
        "KANRI fixed UI and password glyph destinations overlap"
    );
    packed_payloads.resize(FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY, 0);
    packed_payloads[fixed_glyphs::TABLE_OFFSET..fixed_glyphs::TABLE_OFFSET + 512]
        .copy_from_slice(&fixed_glyphs::decode_table());
    Ok(ExactStaticHangulData {
        codes,
        packed_payloads,
    })
}

fn inspect_name_entry_pack_source(
    patched_stored: &[u8],
    expected_decoded_sha256: &str,
    runtime_atlas: &NameInputRuntimeAtlasLayout,
    materialization: &NameGlyphMaterializationBundle,
    runtime_coordinate_list_storage_byte_range: [usize; 2],
) -> Result<NameEntryPackSource> {
    let decoded = decompress(patched_stored, true)
        .context("failed to decode the patched MA_ENT glyph-pack source")?;
    let decoded_sha256 = sha256_bytes(&decoded);
    ensure!(
        decoded.len() >= NAME_ENTRY_FONT_DECODED_SIZE
            && u64::from(NAME_ENTRY_TRANSIENT_LOAD_ORIGIN) + decoded.len() as u64
                <= u64::from(NAME_ENTRY_TRANSIENT_LOAD_END)
            && decoded_sha256 == expected_decoded_sha256,
        "patched MA_ENT identity differs from the composed name-input build"
    );
    let tim = parse_4bpp_prefix(
        decoded
            .get(NAME_ENTRY_FONT_TIM_OFFSET..)
            .context("MA_ENT font TIM offset leaves the decoded record")?,
    )?;
    ensure!(
        runtime_atlas.font_atlas_row_bytes == tim.row_bytes()
            && runtime_atlas.glyph_cell_width == 20
            && runtime_atlas.glyph_cell_height == 20
            && runtime_atlas.pack_storage_cell_base_byte_offsets.len()
                == NAME_GLYPH_PACK_CELL_COUNT,
        "MA_ENT runtime atlas geometry differs from its copied-cell consumer"
    );
    let pixel_data = decoded
        .get(
            NAME_ENTRY_FONT_TIM_OFFSET + tim.pixel_offset
                ..NAME_ENTRY_FONT_TIM_OFFSET + tim.total_size,
        )
        .context("MA_ENT font pixel data is truncated")?;
    let mut copied_pack_storage = Vec::with_capacity(NAME_GLYPH_PACK_STORAGE_BYTES);
    for &cell_base in &runtime_atlas.pack_storage_cell_base_byte_offsets {
        let cell_base = usize::from(cell_base);
        for row in 0..runtime_atlas.glyph_cell_height {
            let start = cell_base + row * runtime_atlas.font_atlas_row_bytes;
            let end = start + NAME_GLYPH_CELL_BYTES / runtime_atlas.glyph_cell_height;
            copied_pack_storage.extend_from_slice(
                pixel_data
                    .get(start..end)
                    .context("MA_ENT glyph-pack cell leaves the font pixels")?,
            );
        }
    }
    ensure!(
        copied_pack_storage.len() == NAME_GLYPH_PACK_STORAGE_BYTES
            && copied_pack_storage.get(..materialization.pack_bytes.len())
                == Some(materialization.pack_bytes.as_slice())
            && copied_pack_storage.get(
                runtime_coordinate_list_storage_byte_range[0]
                    ..runtime_coordinate_list_storage_byte_range[1]
            ) == Some(materialization.runtime_coordinate_list.as_slice()),
        "MA_ENT copied cells differ from the authoritative Hangul materialization bundle"
    );
    let lookup_start = runtime_coordinate_list_storage_byte_range[0]
        .checked_sub(runtime_atlas.lookup_table_bytes.len())
        .context("MA_ENT runtime lookup precedes the copied pack storage")?;
    ensure!(
        copied_pack_storage.get(lookup_start..runtime_coordinate_list_storage_byte_range[0])
            == Some(runtime_atlas.lookup_table_bytes.as_slice()),
        "MA_ENT embedded physical-cell lookup differs from the composed runtime atlas"
    );
    let font_pixel_runtime_address = NAME_ENTRY_TRANSIENT_LOAD_ORIGIN
        .checked_add(u32::try_from(
            NAME_ENTRY_FONT_TIM_OFFSET + tim.pixel_offset,
        )?)
        .context("MA_ENT transient font-pixel address overflow")?;
    Ok(NameEntryPackSource {
        decoded_sha256,
        font_pixel_runtime_address,
        copied_pack_storage,
    })
}

#[derive(Debug)]
struct PlacedProgram {
    bytes: Vec<u8>,
    instructions: Vec<Instruction>,
}

#[derive(Debug)]
struct MaterializerProgram {
    bytes: Vec<u8>,
    instructions: Vec<Instruction>,
    materializer_address: u32,
}

#[derive(Debug)]
struct ResolverProgram {
    bytes: Vec<u8>,
    instructions: Vec<Instruction>,
    cache_uploader_address: u32,
    tagged_code_resolver_address: u32,
    name_draw_resolver_address: u32,
    card_name_normalizer_address: u32,
    registered_name_normalizer_address: u32,
    name_renderer_prewarm_wrapper_address: u32,
}

#[derive(Debug)]
struct StaticGlyphRefresherProgram {
    bytes: Vec<u8>,
    instructions: Vec<Instruction>,
    entry_address: u32,
    unpacker_address: u32,
    code_table_address: u32,
}

#[derive(Clone, Copy, Debug)]
struct RuntimeProgramCopy {
    source_address: u32,
    destination_address: u32,
    byte_count: usize,
    loop_label: &'static str,
}

fn build_initializer_program(
    origin: u32,
    layout: &KanriNameStorageLayout,
    runtime_program_copies: &[RuntimeProgramCopy],
    initialization_data: &InitializationData,
    name_entry_font_pixel_address: u32,
    exact_static_hangul: &ExactStaticHangulData,
    static_refresher: &StaticGlyphRefresherProgram,
) -> Result<PlacedProgram> {
    let cache_map_address = layout.runtime_address(layout.cache_map_byte_range[0])?;
    let name_source_snapshot_address =
        layout.runtime_address(layout.name_source_snapshot_byte_range[0])?;
    let scratch_glyph_payload_address =
        layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let pack_lookup_address =
        initialization_data.runtime_address(initialization_data.pack_lookup_range[0])?;
    let static_hangul_payload_address = exact_static_hangul.persistent_address();
    let static_hangul_code_table_address = static_refresher.code_table_address;
    let staged_ascii_code_table_address =
        initialization_data.runtime_address(initialization_data.ascii_code_table_range[0])?;
    let staged_private_ascii_payload_address =
        initialization_data.runtime_address(initialization_data.private_ascii_payload_range[0])?;
    let persistent_ascii_code_table_address =
        layout.runtime_address(layout.ascii_code_table_byte_range[0])?;
    let persistent_table_byte_count = initialization_data.dynamic_display_code_table_range[1]
        - initialization_data.ascii_code_table_range[0];
    ensure!(
        initialization_data.ascii_code_table_range[1]
            == initialization_data.dynamic_display_code_table_range[0]
            && layout.ascii_code_table_byte_range[1]
                == layout.dynamic_display_code_table_byte_range[0]
            && persistent_table_byte_count
                == layout.dynamic_display_code_table_byte_range[1]
                    - layout.ascii_code_table_byte_range[0],
        "KANRI persistent lookup tables are not one contiguous copy"
    );
    let mut assembler = Assembler::new();
    assembler
        .label("initialize_kanri_name_runtime")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -64,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 60,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 56,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 52,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 48,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: 44,
        })
        .emit(Instruction::Sw {
            rt: Register::S4,
            base: Register::SP,
            offset: 40,
        })
        .emit(Instruction::Addu {
            rd: Register::S4,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jalr {
            rd: Register::RA,
            rs: Register::S4,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Sw {
            rt: Register::V0,
            base: Register::SP,
            offset: 36,
        });
    for runtime_program_copy in runtime_program_copies {
        emit_runtime_program_copy(&mut assembler, *runtime_program_copy)?;
    }
    assembler
        .emit_all(load_address(Register::A0, NAME_ENTRY_TRANSIENT_LOAD_ORIGIN))
        .emit(Instruction::Jal {
            target: NATIVE_RECORD_LOADER_ADDRESS,
        })
        .emit(Instruction::Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: NAME_ENTRY_FONT_CATALOG_INDEX,
        });
    crate::name_input::emit_name_glyph_pack_copy(
        &mut assembler,
        pack_lookup_address,
        name_entry_font_pixel_address,
        KANRI_NAME_STORAGE_ORIGIN,
    );
    emit_runtime_halfword_copy(
        &mut assembler,
        staged_ascii_code_table_address,
        persistent_ascii_code_table_address,
        persistent_table_byte_count,
        "copy_kanri_persistent_table_halfword",
    )?;
    assembler
        .emit_all(load_address(Register::S0, cache_map_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: KANRI_NAME_CACHE_SLOT_COUNT as u16,
        })
        .label("clear_kanri_name_cache_map")
        .emit(Instruction::Sh {
            rt: Register::ZERO,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bgtz(Register::S1, "clear_kanri_name_cache_map")
        .emit(Instruction::nop())
        .emit_all(load_address(Register::S0, name_source_snapshot_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: (KANRI_NAME_SOURCE_SNAPSHOT_BYTES / 2) as u16,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: u16::MAX,
        })
        .label("invalidate_kanri_name_source_snapshot")
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bgtz(Register::S1, "invalidate_kanri_name_source_snapshot")
        .emit(Instruction::nop())
        .emit_all(load_address(Register::S0, static_hangul_code_table_address))
        .emit_all(load_address(Register::S1, static_hangul_payload_address))
        .emit_all(load_address(Register::S4, scratch_glyph_payload_address))
        .emit(Instruction::Ori {
            rt: Register::S2,
            rs: Register::ZERO,
            immediate: exact_static_hangul.codes.len() as u16,
        })
        .label("upload_exact_kanri_static_hangul")
        .emit(Instruction::Lhu {
            rt: Register::S3,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: static_refresher.unpacker_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S4,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::S1,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::SP,
            rt: Register::ZERO,
        });
    emit_menu_code_rect(&mut assembler, Register::S3);
    assembler
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S4,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: -1,
        })
        .bgtz(Register::S2, "upload_exact_kanri_static_hangul")
        .emit(Instruction::nop())
        .emit_all(load_address(
            Register::S0,
            staged_private_ascii_payload_address,
        ))
        .emit_all(load_address(
            Register::S1,
            persistent_ascii_code_table_address,
        ));
    for character in KANRI_RAW_ASCII_GLYPHS.chars() {
        assembler.emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S1,
            offset: i16::try_from(u32::from(character) * 2)?,
        });
        assembler.emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::SP,
            rt: Register::ZERO,
        });
        emit_menu_code_rect(&mut assembler, Register::T0);
        assembler
            .emit(Instruction::Jal {
                target: VRAM_UPLOAD_ROUTINE_ADDRESS,
            })
            .emit(Instruction::Addu {
                rd: Register::A1,
                rs: Register::S0,
                rt: Register::ZERO,
            })
            .emit(Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: KANRI_GLYPH_PAYLOAD_BYTES as i16,
            });
    }
    assembler
        .emit(Instruction::Lw {
            rt: Register::V0,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::SP,
            offset: 40,
        })
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: 44,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 48,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 52,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 56,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 60,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 64,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    place_program(&assembler, origin, "KANRI initializer")
}

fn build_static_refresher_program(
    origin: u32,
    layout: &KanriNameStorageLayout,
    exact_static_hangul: &ExactStaticHangulData,
) -> Result<StaticGlyphRefresherProgram> {
    ensure!(
        !exact_static_hangul.codes.is_empty()
            && exact_static_hangul.codes.len()
                <= KANRI_INITIAL_GLYPH_CAPACITY - KANRI_RAW_ASCII_GLYPH_COUNT + 76
            && exact_static_hangul.packed_payloads.len() == FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY,
        "KANRI static refresher lost its fixed Hangul population"
    );

    let code_table_address = origin
        .checked_add(u32::try_from(STATIC_REFRESHER_TABLE_OFFSET)?)
        .context("KANRI static refresher table address overflow")?;
    let static_hangul_payload_address = exact_static_hangul.persistent_address();
    let cache_map_address = layout.runtime_address(layout.cache_map_byte_range[0])?;
    let scratch_glyph_payload_address =
        layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let persistent_ascii_code_table_address =
        layout.runtime_address(layout.ascii_code_table_byte_range[0])?;
    let mut assembler = Assembler::new();
    let unpacker_address = origin;
    emit_exact_static_glyph_unpacker(&mut assembler);
    let unpacker = place_program(&assembler, origin, "KANRI exact static glyph unpacker")?;
    let entry_address = origin
        .checked_add(u32::try_from(unpacker.bytes.len())?)
        .context("KANRI static refresher entry address overflow")?;
    assembler
        .label("refresh_kanri_status_overview_glyphs")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -40,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::S4,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Jal {
            target: STATUS_OVERVIEW_NATIVE_SETUP_ADDRESS,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Sw {
            rt: Register::V0,
            base: Register::SP,
            offset: 8,
        })
        .emit_all(load_address(Register::S0, code_table_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: exact_static_hangul.codes.len() as u16,
        })
        .emit_all(load_address(Register::S2, scratch_glyph_payload_address))
        .emit_all(load_address(Register::S3, static_hangul_payload_address))
        .label("refresh_kanri_static_hangul")
        .emit(Instruction::Lhu {
            rt: Register::S4,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: unpacker_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::SP,
            rt: Register::ZERO,
        });
    emit_menu_code_rect(&mut assembler, Register::S4);
    assembler
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bgtz(Register::S1, "refresh_kanri_static_hangul")
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit_all(load_address(Register::S0, PRIVATE_ASCII_PERSISTENT_ADDRESS))
        .emit_all(load_address(
            Register::S1,
            persistent_ascii_code_table_address,
        ));
    for character in KANRI_RAW_ASCII_GLYPHS.chars() {
        assembler.emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S1,
            offset: i16::try_from(u32::from(character) * 2)?,
        });
        assembler.emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::SP,
            rt: Register::ZERO,
        });
        emit_menu_code_rect(&mut assembler, Register::T0);
        assembler
            .emit(Instruction::Jal {
                target: VRAM_UPLOAD_ROUTINE_ADDRESS,
            })
            .emit(Instruction::Addu {
                rd: Register::A1,
                rs: Register::S0,
                rt: Register::ZERO,
            })
            .emit(Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: KANRI_GLYPH_PAYLOAD_BYTES as i16,
            });
    }
    assembler
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit_all(load_address(Register::S0, cache_map_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: KANRI_NAME_CACHE_SLOT_COUNT as u16,
        })
        .label("invalidate_refreshed_kanri_cache_map")
        .emit(Instruction::Sh {
            rt: Register::ZERO,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bgtz(Register::S1, "invalidate_refreshed_kanri_cache_map")
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Lw {
            rt: Register::V0,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 40,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());

    let placed = place_program(&assembler, origin, "KANRI status-overview glyph refresher")?;
    ensure!(
        placed.bytes.len() <= STATIC_REFRESHER_TABLE_OFFSET,
        "KANRI static refresher code ({} bytes) overlaps its embedded table",
        placed.bytes.len()
    );
    let mut bytes = placed.bytes;
    bytes.resize(STATIC_REFRESHER_TABLE_OFFSET, 0);
    for code in &exact_static_hangul.codes {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    Ok(StaticGlyphRefresherProgram {
        bytes,
        instructions: placed.instructions,
        entry_address,
        unpacker_address,
        code_table_address,
    })
}

fn emit_runtime_halfword_copy(
    assembler: &mut Assembler,
    source_address: u32,
    destination_address: u32,
    byte_count: usize,
    loop_label: &'static str,
) -> Result<()> {
    ensure!(
        byte_count > 0 && byte_count.is_multiple_of(2),
        "KANRI persistent table copy is not a non-empty halfword sequence"
    );
    assembler
        .emit_all(load_address(Register::S0, source_address))
        .emit_all(load_address(Register::S1, destination_address))
        .emit(Instruction::Ori {
            rt: Register::S2,
            rs: Register::ZERO,
            immediate: u16::try_from(byte_count / 2)?,
        })
        .label(loop_label)
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: -1,
        })
        .bgtz(Register::S2, loop_label)
        .emit(Instruction::nop());
    Ok(())
}

fn emit_runtime_program_copy(
    assembler: &mut Assembler,
    runtime_program_copy: RuntimeProgramCopy,
) -> Result<()> {
    ensure!(
        runtime_program_copy.byte_count > 0 && runtime_program_copy.byte_count.is_multiple_of(4),
        "KANRI staged runtime program is not a non-empty word sequence"
    );
    let word_count = u16::try_from(runtime_program_copy.byte_count / 4)
        .context("KANRI staged runtime program word count exceeds an immediate")?;
    assembler
        .emit_all(load_address(
            Register::S0,
            runtime_program_copy.source_address,
        ))
        .emit_all(load_address(
            Register::S1,
            runtime_program_copy.destination_address,
        ))
        .emit(Instruction::Ori {
            rt: Register::S2,
            rs: Register::ZERO,
            immediate: word_count,
        })
        .label(runtime_program_copy.loop_label)
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 4,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::S1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: -1,
        })
        .bgtz(Register::S2, runtime_program_copy.loop_label)
        .emit(Instruction::nop());
    Ok(())
}

fn build_band_materializer_program(
    origin: u32,
    layout: &KanriNameStorageLayout,
    pack: &crate::name_input::NameGlyphBandPack,
    outline_pixel_address: u32,
    outline_cleanup_address: u32,
) -> Result<MaterializerProgram> {
    ensure!(
        layout.pack_payload_byte_range == [0, pack.bytes().len()],
        "KANRI band pack differs from its storage layout"
    );
    let scratch = layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let mut assembler = Assembler::new();
    emit_contiguous_pack_byte_reader(
        &mut assembler,
        KANRI_NAME_STORAGE_ORIGIN,
        pack.bytes().len(),
    )?;
    let materializer_address = origin + u32::try_from(assembler.assemble(origin)?.bytes().len())?;
    // KANRI supplies an untagged syllable; its sole scratch cell is mode-owned.
    assembler.emit_all(load_address(Register::A1, scratch));
    let render_origin = origin + u32::try_from(assembler.assemble(origin)?.bytes().len())?;
    let renderer = pack.build_render_program_with_outline_addresses(
        render_origin,
        origin,
        SCRATCH_GLYPH_ROW_BYTES as usize,
        outline_pixel_address,
        outline_cleanup_address,
    )?;
    assembler.emit_all(verify_placed_program(&renderer, render_origin)?);
    let placed = place_program(&assembler, origin, "KANRI band materializer")?;
    ensure!(
        placed.bytes.len() <= MATERIALIZER_PROGRAM_BYTE_CAPACITY
            && placed.bytes.len() <= MATERIALIZER_STAGING_BYTE_CAPACITY,
        "KANRI band materializer exceeds its staging or runtime region"
    );
    Ok(MaterializerProgram {
        bytes: placed.bytes,
        instructions: placed.instructions,
        materializer_address,
    })
}

fn build_materializer_program(
    origin: u32,
    layout: &KanriNameStorageLayout,
    runtime_pack: &crate::name_input::NameInputRuntimePackLayout,
    outline_pixel_address: u32,
    outline_cleanup_address: u32,
) -> Result<MaterializerProgram> {
    let scratch_glyph_address =
        layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let mut assembler = Assembler::new();
    emit_contiguous_pack_byte_reader(
        &mut assembler,
        KANRI_NAME_STORAGE_ORIGIN,
        layout.runtime_coordinate_list_byte_range[1],
    )?;
    emit_kanri_scratch_glyph_clearer(&mut assembler, scratch_glyph_address);
    emit_component_resolver(&mut assembler, runtime_pack)?;
    let placed_prefix = assembler
        .assemble(origin)
        .context("failed to place KANRI materializer prefix")?;
    let materializer_address = origin
        .checked_add(u32::try_from(placed_prefix.bytes().len())?)
        .context("KANRI materializer address overflow")?;
    emit_glyph_fill_materializer(
        &mut assembler,
        runtime_pack,
        u16::try_from(layout.runtime_coordinate_list_byte_range[0])?,
        SCRATCH_GLYPH_ROW_BYTES,
        outline_pixel_address,
        outline_cleanup_address,
        None,
    )?;
    let placed = place_program(&assembler, origin, "KANRI materializer")?;
    Ok(MaterializerProgram {
        bytes: placed.bytes,
        instructions: placed.instructions,
        materializer_address,
    })
}

fn build_resolver_program(
    origin: u32,
    layout: &KanriNameStorageLayout,
    materializer_address: u32,
    nickname_companion_cache_code_range: [u16; 2],
    fixed_spirit_gauge_unit_code: u16,
    direct_name_map: &ImportedDirectNameMap,
) -> Result<ResolverProgram> {
    let mut assembler = Assembler::new();
    let scratch_glyph_address =
        layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let cache_uploader_address = origin;
    emit_kanri_cache_uploader(&mut assembler, layout)?;
    let uploader = assembler
        .assemble(origin)
        .context("failed to place KANRI cache uploader")?;
    let tagged_code_resolver_address = origin
        .checked_add(u32::try_from(uploader.bytes().len())?)
        .context("KANRI tagged-code resolver address overflow")?;
    emit_kanri_tagged_code_resolver(
        &mut assembler,
        layout,
        materializer_address,
        cache_uploader_address,
        nickname_companion_cache_code_range,
        fixed_spirit_gauge_unit_code,
    )?;
    let resolver = assembler
        .assemble(origin)
        .context("failed to place KANRI tagged-code resolver")?;
    let name_renderer_prewarm_wrapper_address = origin
        .checked_add(u32::try_from(resolver.bytes().len())?)
        .context("KANRI name-renderer prewarm wrapper address overflow")?;
    emit_kanri_name_renderer_prewarm_wrapper(&mut assembler, tagged_code_resolver_address);
    let (card_name_normalizer_address, registered_name_normalizer_address) =
        emit_imported_name_normalizers(&mut assembler, origin, direct_name_map)?;
    let name_draw_resolver_address =
        origin + u32::try_from(assembler.assemble(origin)?.bytes().len())?;
    super::registration_list_layout::emit_list_name_resolver(
        &mut assembler,
        tagged_code_resolver_address,
    );
    let placed = place_program(&assembler, origin, "KANRI tagged-code resolver")?;
    verify_cache_upload_copy_contract(
        &placed.instructions,
        origin,
        tagged_code_resolver_address,
        scratch_glyph_address,
    )?;
    Ok(ResolverProgram {
        bytes: placed.bytes,
        instructions: placed.instructions,
        cache_uploader_address,
        tagged_code_resolver_address,
        name_draw_resolver_address,
        name_renderer_prewarm_wrapper_address,
        card_name_normalizer_address,
        registered_name_normalizer_address,
    })
}

fn nickname_companion_cache_code_range(layout: &NameGlyphConsumerLayout) -> Result<[u16; 2]> {
    let codes = layout.cache_codes_for_field(NameField::Nickname)?;
    let start = *codes
        .first()
        .context("nickname companion cache has no first code")?;
    let end = codes
        .last()
        .context("nickname companion cache has no last code")?
        .checked_add(1)
        .context("nickname companion cache code range overflow")?;
    ensure!(
        codes.len() == NameField::Nickname.visible_glyph_capacity()
            && codes.iter().copied().eq(start..end),
        "nickname companion cache codes are not one complete consecutive field"
    );
    Ok([start, end])
}

fn emit_kanri_name_renderer_prewarm_wrapper(assembler: &mut Assembler, resolver_address: u32) {
    assembler
        .label("prewarm_current_name_before_rendering")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -48,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 44,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 40,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Sw {
            rt: Register::A0,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::A1,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Sw {
            rt: Register::A2,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Sw {
            rt: Register::A3,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: NAME_RENDERER_GLYPH_LIMIT,
        })
        .label("prewarm_current_name_word")
        .emit(Instruction::Lhu {
            rt: Register::A0,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: MESSAGE_END_CODE,
        })
        .beq(Register::A0, Register::T0, "current_name_prewarm_complete")
        .emit(Instruction::Ori {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Jal {
            target: resolver_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bgtz(Register::S1, "prewarm_current_name_word")
        .emit(Instruction::nop())
        .label("current_name_prewarm_complete")
        .emit(Instruction::Lw {
            rt: Register::A3,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::A2,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Lw {
            rt: Register::A1,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Lw {
            rt: Register::A0,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 40,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 44,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 48,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -56,
        })
        .emit(Instruction::Sw {
            rt: Register::S5,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::J {
            target: NAME_RENDERER_CONTINUATION_ADDRESS,
        })
        .emit(Instruction::nop());
}

fn emit_kanri_scratch_glyph_clearer(assembler: &mut Assembler, scratch_glyph_address: u32) {
    assembler
        .label("clear_name_glyph_cache_cell")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: KANRI_NAME_CACHE_SLOT_COUNT as i16,
        })
        .bne(Register::T0, Register::ZERO, "kanri_cache_slot_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("kanri_cache_slot_in_range")
        .emit_all(load_address(Register::V0, scratch_glyph_address))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: KANRI_GLYPH_PAYLOAD_BYTES as u16,
        })
        .label("clear_kanri_cache_byte")
        .emit(Instruction::Sb {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .bgtz(Register::T1, "clear_kanri_cache_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

fn emit_kanri_cache_uploader(
    assembler: &mut Assembler,
    layout: &KanriNameStorageLayout,
) -> Result<()> {
    let scratch_glyph_address =
        layout.runtime_address(layout.scratch_glyph_payload_byte_range[0])?;
    let dynamic_code_table_address =
        layout.runtime_address(layout.dynamic_display_code_table_byte_range[0])?;
    assembler
        .label("upload_kanri_name_cache_cell")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: KANRI_NAME_CACHE_SLOT_COUNT as i16,
        })
        .bne(
            Register::T0,
            Register::ZERO,
            "upload_kanri_cache_slot_in_range",
        )
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("upload_kanri_cache_slot_in_range")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -CACHE_UPLOAD_STACK_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: CACHE_UPLOAD_RA_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: CACHE_UPLOAD_S0_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: CACHE_UPLOAD_S1_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: CACHE_UPLOAD_S2_OFFSET,
        })
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::S0,
            shift: 1,
        })
        .emit_all(load_address(Register::T2, dynamic_code_table_address))
        .emit(Instruction::Addu {
            rd: Register::T2,
            rs: Register::T2,
            rt: Register::T0,
        })
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::T2,
            offset: 0,
        })
        // This also separates the following RECT calculation from the LHU's
        // architectural load-delay slot.
        .emit_all(load_address(Register::S1, scratch_glyph_address));
    emit_menu_code_rect(assembler, Register::T0);
    assembler
        // The RECT builder owns T1-T3. Keep both copy pointers in preserved
        // registers so later helper edits cannot redirect the payload copy.
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::SP,
            immediate: CACHE_UPLOAD_PIXELS_OFFSET,
        })
        .emit(Instruction::Ori {
            rt: Register::T3,
            rs: Register::ZERO,
            immediate: SCRATCH_GLYPH_HEIGHT,
        })
        .label("copy_kanri_cache_row")
        .emit(Instruction::Ori {
            rt: Register::T4,
            rs: Register::ZERO,
            immediate: SCRATCH_GLYPH_ROW_BYTES as u16,
        })
        .label("copy_kanri_cache_byte")
        .emit(Instruction::Lbu {
            rt: Register::T5,
            base: Register::S1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 1,
        })
        .emit(Instruction::Sb {
            rt: Register::T5,
            base: Register::S2,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T4,
            immediate: -1,
        })
        .bgtz(Register::T4, "copy_kanri_cache_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T3,
            rs: Register::T3,
            immediate: -1,
        })
        .bgtz(Register::T3, "copy_kanri_cache_row")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::SP,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::SP,
            immediate: CACHE_UPLOAD_PIXELS_OFFSET,
        })
        // The native TIM loader waits here too. The stack payload must remain
        // live until the GPU transfer has completed.
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: CACHE_UPLOAD_S2_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: CACHE_UPLOAD_S1_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: CACHE_UPLOAD_S0_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: CACHE_UPLOAD_RA_OFFSET,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: CACHE_UPLOAD_STACK_BYTES,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    Ok(())
}

fn verify_cache_upload_copy_contract(
    instructions: &[Instruction],
    program_origin: u32,
    resolver_address: u32,
    scratch_glyph_address: u32,
) -> Result<()> {
    let uploader_instruction_count = usize::try_from(
        resolver_address
            .checked_sub(program_origin)
            .context("KANRI resolver precedes its cache uploader")?
            / 4,
    )?;
    let uploader = instructions
        .get(..uploader_instruction_count)
        .context("KANRI cache uploader leaves its placed program")?;
    let scratch_pointer = load_address(Register::S1, scratch_glyph_address);
    let has_scratch_pointer = uploader
        .windows(scratch_pointer.len())
        .any(|window| window == scratch_pointer.as_slice());
    let upload_index = uploader.iter().position(|instruction| {
        *instruction
            == Instruction::Jal {
                target: VRAM_UPLOAD_ROUTINE_ADDRESS,
            }
    });
    let sync_index = uploader.iter().position(|instruction| {
        *instruction
            == Instruction::Jal {
                target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
            }
    });
    let stack_release_index = uploader.iter().position(|instruction| {
        *instruction
            == Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: CACHE_UPLOAD_STACK_BYTES,
            }
    });
    ensure!(
        has_scratch_pointer
            && uploader.contains(&Instruction::Addiu {
                rt: Register::S2,
                rs: Register::SP,
                immediate: CACHE_UPLOAD_PIXELS_OFFSET,
            })
            && uploader.contains(&Instruction::Lbu {
                rt: Register::T5,
                base: Register::S1,
                offset: 0,
            })
            && uploader.contains(&Instruction::Sb {
                rt: Register::T5,
                base: Register::S2,
                offset: 0,
            })
            && uploader.contains(&Instruction::Sw {
                rt: Register::S1,
                base: Register::SP,
                offset: CACHE_UPLOAD_S1_OFFSET,
            })
            && uploader.contains(&Instruction::Sw {
                rt: Register::S2,
                base: Register::SP,
                offset: CACHE_UPLOAD_S2_OFFSET,
            })
            && uploader.contains(&Instruction::Lw {
                rt: Register::S1,
                base: Register::SP,
                offset: CACHE_UPLOAD_S1_OFFSET,
            })
            && uploader.contains(&Instruction::Lw {
                rt: Register::S2,
                base: Register::SP,
                offset: CACHE_UPLOAD_S2_OFFSET,
            })
            && matches!(
                (upload_index, sync_index, stack_release_index),
                (Some(upload), Some(sync), Some(release)) if upload < sync && sync < release
            ),
        "KANRI cache uploader no longer copies scratch-to-stack with preserved pointers and synchronous lifetime"
    );
    Ok(())
}

fn emit_kanri_tagged_code_resolver(
    assembler: &mut Assembler,
    layout: &KanriNameStorageLayout,
    materializer_address: u32,
    cache_uploader_address: u32,
    nickname_companion_cache_code_range: [u16; 2],
    fixed_spirit_gauge_unit_code: u16,
) -> Result<()> {
    let cache_map_address = layout.runtime_address(layout.cache_map_byte_range[0])?;
    let dynamic_code_table_address =
        layout.runtime_address(layout.dynamic_display_code_table_byte_range[0])?;
    let ascii_code_table_address = layout.runtime_address(layout.ascii_code_table_byte_range[0])?;
    assembler
        .label("resolve_kanri_tagged_name_code")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -32,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Sw {
            rt: Register::A2,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::S0,
            immediate: TAG_MASK,
        })
        .beq(Register::T0, Register::ZERO, "resolve_kanri_legacy_code")
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: ASCII_NAME_TAG,
        })
        .beq(Register::T0, Register::T1, "resolve_kanri_ascii_code")
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .beq(Register::T0, Register::T1, "resolve_kanri_hangul_code")
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: FIXED_UI_NAME_TAG,
        })
        .beq(Register::T0, Register::T1, "resolve_kanri_fixed_ui_code")
        .emit(Instruction::nop())
        .jump("kanri_name_code_failed")
        .emit(Instruction::nop())
        .label("resolve_kanri_fixed_ui_code")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE,
        })
        .bne(Register::S0, Register::T0, "kanri_name_code_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: fixed_spirit_gauge_unit_code,
        })
        .jump("kanri_name_code_complete")
        .emit(Instruction::nop())
        .label("resolve_kanri_ascii_code")
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::S0,
            immediate: ASCII_PAYLOAD_MASK,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 1,
        })
        .emit_all(load_address(Register::T1, ascii_code_table_address))
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Lhu {
            rt: Register::V0,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: u16::MAX,
        })
        .beq(Register::V0, Register::T0, "kanri_name_code_failed")
        .emit(Instruction::nop())
        .jump("kanri_name_code_complete")
        .emit(Instruction::nop())
        .label("resolve_kanri_legacy_code")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: nickname_companion_cache_code_range[0],
        })
        .emit(Instruction::Sltu {
            rd: Register::T1,
            rs: Register::S0,
            rt: Register::T0,
        })
        .bne(Register::T1, Register::ZERO, "resolve_kanri_legacy_mapping")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: nickname_companion_cache_code_range[1],
        })
        .emit(Instruction::Sltu {
            rd: Register::T1,
            rs: Register::S0,
            rt: Register::T0,
        })
        .beq(Register::T1, Register::ZERO, "resolve_kanri_legacy_mapping")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::S0,
            immediate: -(nickname_companion_cache_code_range[0] as i16),
        })
        .emit(Instruction::Sll {
            rd: Register::T2,
            rt: Register::T2,
            shift: 1,
        })
        .emit_all(load_address(Register::T0, PRIMARY_NICKNAME_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T2,
        })
        .emit(Instruction::Lhu {
            rt: Register::T2,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: ASCII_NAME_TAG,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T2,
            immediate: TAG_MASK,
        })
        .beq(
            Register::T0,
            Register::T1,
            "resolve_kanri_tagged_companion_ascii",
        )
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .bne(Register::T0, Register::T1, "resolve_kanri_legacy_mapping")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::T2,
            rt: Register::ZERO,
        })
        .jump("resolve_kanri_hangul_code")
        .emit(Instruction::nop())
        .label("resolve_kanri_tagged_companion_ascii")
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::T2,
            rt: Register::ZERO,
        })
        .jump("resolve_kanri_ascii_code")
        .emit(Instruction::nop())
        .label("resolve_kanri_legacy_mapping")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: LEGACY_PADDING_CODE,
        })
        .bne(Register::S0, Register::T0, "kanri_legacy_index_ready")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("kanri_legacy_index_ready")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::S0,
            immediate: LEGACY_NAME_CODE_COUNT,
        })
        .beq(Register::T0, Register::ZERO, "kanri_name_code_failed")
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::S0,
            shift: 1,
        })
        .emit_all(load_address(
            Register::T1,
            LEGACY_NAME_MAPPING_TABLE_ADDRESS,
        ))
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Lhu {
            rt: Register::V0,
            base: Register::T1,
            offset: 0,
        })
        .jump("kanri_name_code_complete")
        .emit(Instruction::Andi {
            rt: Register::V0,
            rs: Register::V0,
            immediate: SKIP_CODE,
        })
        .label("resolve_kanri_hangul_code")
        .emit(Instruction::Andi {
            rt: Register::S0,
            rs: Register::S0,
            immediate: HANGUL_PAYLOAD_MASK,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S0,
            immediate: 1,
        })
        .emit_all(load_address(Register::S2, cache_map_address))
        .emit(Instruction::Ori {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: u16::MAX,
        })
        .emit(Instruction::Addu {
            rd: Register::T2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("scan_kanri_name_cache")
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S2,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: u16::MAX,
        })
        .beq(Register::T0, Register::S1, "kanri_name_cache_hit")
        .emit(Instruction::nop())
        .bne(Register::T0, Register::ZERO, "next_kanri_name_cache_slot")
        .emit(Instruction::nop())
        .bne(Register::S3, Register::T1, "next_kanri_name_cache_slot")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::T2,
            rt: Register::ZERO,
        })
        .label("next_kanri_name_cache_slot")
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: 1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T2,
            immediate: KANRI_NAME_CACHE_SLOT_COUNT as i16,
        })
        .bne(Register::T0, Register::ZERO, "scan_kanri_name_cache")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: u16::MAX,
        })
        .beq(Register::S3, Register::T0, "kanri_name_code_failed")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .beq(Register::T0, Register::ZERO, "kanri_name_code_failed")
        .emit(Instruction::nop())
        .emit(Instruction::Jal {
            target: materializer_address,
        })
        .emit(Instruction::nop())
        .beq(Register::V0, Register::ZERO, "kanri_name_code_failed")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: cache_uploader_address,
        })
        .emit(Instruction::nop())
        .beq(Register::V0, Register::ZERO, "kanri_name_code_failed")
        .emit_all(load_address(Register::T0, cache_map_address))
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::S3,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Sh {
            rt: Register::S1,
            base: Register::T0,
            offset: 0,
        })
        .jump("kanri_name_cache_code")
        .emit(Instruction::nop())
        .label("kanri_name_cache_hit")
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::T2,
            rt: Register::ZERO,
        })
        .label("kanri_name_cache_code")
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::S3,
            shift: 1,
        })
        .emit_all(load_address(Register::T1, dynamic_code_table_address))
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Lhu {
            rt: Register::V0,
            base: Register::T1,
            offset: 0,
        })
        .jump("kanri_name_code_complete")
        .emit(Instruction::nop())
        .label("kanri_name_code_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: SKIP_CODE,
        })
        .label("kanri_name_code_complete")
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 32,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    Ok(())
}

fn emit_menu_code_rect(assembler: &mut Assembler, code: Register) {
    assembler
        .emit(Instruction::Srl {
            rd: Register::T1,
            rt: code,
            shift: 8,
        })
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::T1,
            shift: 6,
        })
        .emit(Instruction::Andi {
            rt: Register::T2,
            rs: code,
            immediate: 0x000f,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T2,
            shift: 2,
        })
        .emit(Instruction::Addu {
            rd: Register::T2,
            rs: Register::T2,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: KANRI_MENU_ATLAS_VRAM_WORD_X,
        })
        .emit(Instruction::Sh {
            rt: Register::T1,
            base: Register::SP,
            offset: 0,
        })
        .emit(Instruction::Srl {
            rd: Register::T1,
            rt: code,
            shift: 4,
        })
        .emit(Instruction::Andi {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 0x000f,
        })
        .emit(Instruction::Sll {
            rd: Register::T2,
            rt: Register::T1,
            shift: 2,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::T1,
            shift: 2,
        })
        .emit(Instruction::Sh {
            rt: Register::T1,
            base: Register::SP,
            offset: 2,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 5,
        })
        .emit(Instruction::Sh {
            rt: Register::T1,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 20,
        })
        .emit(Instruction::Sh {
            rt: Register::T1,
            base: Register::SP,
            offset: 6,
        });
}

fn validate_source_layout(
    source_overlay: &[u8],
    source_edit_shared_ui: &[u8],
    source_main_executable: &[u8],
    source_name_entry_font_record: &FileRecord,
    source_name_entry_font_stored: &[u8],
) -> Result<()> {
    super::registration_list_layout::validate_list_name_consumer(source_overlay)?;
    for (i, register) in [
        Register::V1,
        Register::A0,
        Register::A1,
        Register::A2,
        Register::A3,
    ]
    .into_iter()
    .enumerate()
    {
        ensure!(
            decode_instruction(source_overlay, 0x60f8 + i * 8)?
                == Instruction::Lui {
                    rt: register,
                    immediate: 0x801f
                }
                && decode_instruction(source_overlay, 0x60fc + i * 8)?
                    == Instruction::Lhu {
                        rt: register,
                        base: register,
                        offset: 0x1886 + i as i16 * 2
                    },
            "KANRI Diary temporary-name copy changed"
        );
    }

    ensure!(
        decode_instruction(source_overlay, REGISTERED_LIST_NAME_X_OFFSET)?
            == Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 54,
            }
            && decode_instruction(source_overlay, 0x2c3c)?
                == Instruction::Jal {
                    target: OVERLAY_RUNTIME_BASE + NAME_RENDERER_ENTRY_OFFSET as u32,
                },
        "KANRI registered-list name position consumer changed"
    );
    ensure!(
        source_overlay.len() == 0x83a8
            && read_u32(source_overlay, 0)? == NATIVE_ENTRYPOINT
            && decode_instruction(source_overlay, LAST_IMPORT_PARSER_HOOK_OFFSET)?
                == Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                }
            && decode_instruction(source_overlay, LAST_IMPORT_PARSER_HOOK_OFFSET + 4)?
                == Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: 0xd800,
                },
        "KANRI final imported-image parser hook source changed"
    );
    ensure!(
        decode_instruction(source_overlay, CARD_NAME_POINTER_OFFSET)?
            == Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S5,
                immediate: 0x86,
            }
            && decode_instruction(source_overlay, CARD_NAME_POINTER_OFFSET + 4)?
                == Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::ZERO,
                    immediate: 0x041f,
                }
            && decode_instruction(source_overlay, REGISTER_NAME_LOAD_OFFSET)?
                == Instruction::Lhu {
                    rt: Register::V1,
                    base: Register::A0,
                    offset: 0x86,
                }
            && decode_instruction(source_overlay, REGISTER_NAME_LOAD_OFFSET + 4)?
                == Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 3,
                },
        "KANRI saved-name consumer or registration copy changed"
    );
    let expected_name_renderer_entry = [
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -56,
        },
        Instruction::Sw {
            rt: Register::S5,
            base: Register::SP,
            offset: 36,
        },
    ];
    ensure!(
        expected_name_renderer_entry
            .iter()
            .cloned()
            .enumerate()
            .all(|(index, expected)| {
                decode_instruction(source_overlay, NAME_RENDERER_ENTRY_OFFSET + index * 4)
                    .is_ok_and(|actual| actual == expected)
            }),
        "KANRI name-renderer entry prologue changed"
    );
    let native_ui_setup_call_offsets = (0..source_overlay.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            decode_instruction(source_overlay, *offset).is_ok_and(|instruction| {
                instruction
                    == Instruction::Jal {
                        target: STATUS_OVERVIEW_NATIVE_SETUP_ADDRESS,
                    }
            })
        })
        .collect::<Vec<_>>();
    ensure!(
        native_ui_setup_call_offsets == STATIC_GLYPH_REFRESH_HOOK_OFFSETS
            && decode_instruction(source_overlay, STATIC_GLYPH_REFRESH_HOOK_OFFSETS[0] + 4)?
                == Instruction::nop()
            && decode_instruction(source_overlay, STATIC_GLYPH_REFRESH_HOOK_OFFSETS[1] + 4)?
                == Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: 0x10,
                },
        "KANRI native UI setup call domain changed"
    );
    let expected_name_block = [
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: LEGACY_PADDING_CODE as i16,
        },
        Instruction::Bne {
            rs: Register::V1,
            rt: Register::V0,
            target: OVERLAY_RUNTIME_BASE + NAME_CODE_HOOK_OFFSET as u32 + 0x14,
        },
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::V1,
            shift: 1,
        },
        Instruction::Addu {
            rd: Register::V1,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::V1,
            shift: 1,
        },
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x800a,
        },
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: Register::V0,
        },
        Instruction::Lh {
            rt: Register::V1,
            base: Register::AT,
            offset: 0x2d74,
        },
    ];
    ensure!(
        expected_name_block
            .iter()
            .cloned()
            .enumerate()
            .all(|(index, expected)| {
                decode_instruction(source_overlay, NAME_CODE_HOOK_OFFSET + index * 4)
                    .is_ok_and(|actual| actual == expected)
            }),
        "KANRI dynamic-name legacy mapping block changed"
    );
    ensure!(
        source_edit_shared_ui.len() == EDIT_SHARED_UI_SIZE,
        "EDITMOJI tagged-name source extent changed"
    );
    for (region, [start, end]) in [
        (
            "initializer",
            [
                INITIALIZER_OFFSET,
                INITIALIZER_OFFSET + INITIALIZER_BYTE_CAPACITY,
            ],
        ),
        (
            "materializer",
            [
                MATERIALIZER_STAGING_OFFSET,
                MATERIALIZER_STAGING_OFFSET + MATERIALIZER_STAGING_BYTE_CAPACITY,
            ],
        ),
        (
            "static refresher",
            [
                STATIC_REFRESHER_STAGING_OFFSET,
                STATIC_REFRESHER_STAGING_OFFSET + STATIC_REFRESHER_STAGING_BYTE_CAPACITY,
            ],
        ),
        (
            "exact static Hangul payload",
            [
                STATIC_HANGUL_PAYLOAD_STAGING_OFFSET,
                STATIC_HANGUL_PAYLOAD_STAGING_OFFSET + STATIC_HANGUL_PAYLOAD_STAGING_BYTE_CAPACITY,
            ],
        ),
        (
            "initialization data",
            [
                INITIALIZATION_DATA_STAGING_OFFSET,
                INITIALIZATION_DATA_STAGING_OFFSET + INITIALIZATION_DATA_STAGING_BYTE_CAPACITY,
            ],
        ),
        (
            "resolver",
            [
                RESOLVER_STAGING_OFFSET,
                RESOLVER_STAGING_OFFSET + RESOLVER_STAGING_BYTE_CAPACITY,
            ],
        ),
    ] {
        if let Some(relative_offset) = source_edit_shared_ui[start..end]
            .iter()
            .position(|byte| *byte != 0)
        {
            anyhow::bail!(
                "KANRI {region} staging storage is not source-zero EDITMOJI padding at decoded offset 0x{:05x}",
                start + relative_offset
            );
        }
    }
    let catalog_entry = source_main_executable
        .get(
            NAME_ENTRY_FONT_CATALOG_ENTRY_OFFSET
                ..NAME_ENTRY_FONT_CATALOG_ENTRY_OFFSET + NAME_ENTRY_FONT_CATALOG_ENTRY.len(),
        )
        .context("MA_ENT native catalog entry leaves the main executable")?;
    ensure!(
        catalog_entry == NAME_ENTRY_FONT_CATALOG_ENTRY
            && source_catalog_key(source_name_entry_font_record)?
                == NAME_ENTRY_FONT_CATALOG_ENTRY[..8]
            && source_name_entry_font_record.name == "MA_ENT.BIZ"
            && source_name_entry_font_record.extended_attribute_blocks == 0
            && source_name_entry_font_record.size as usize == source_name_entry_font_stored.len(),
        "KANRI native MA_ENT catalog binding changed"
    );
    let overlay_loaded_end = OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(source_overlay.len())?)
        .context("KANRI overlay loaded-end address overflow")?;
    ensure!(
        overlay_loaded_end <= KANRI_RUNTIME_OBSERVED_USED_END
            && KANRI_RUNTIME_OBSERVED_USED_END <= MATERIALIZER_PROGRAM_ORIGIN
            && MATERIALIZER_PROGRAM_ORIGIN + u32::try_from(MATERIALIZER_PROGRAM_BYTE_CAPACITY)?
                == RESOLVER_PROGRAM_ORIGIN
            && RESOLVER_PROGRAM_ORIGIN + u32::try_from(RESOLVER_PROGRAM_BYTE_CAPACITY)?
                <= STATIC_REFRESHER_PROGRAM_ORIGIN
            && STATIC_REFRESHER_PROGRAM_ORIGIN
                + u32::try_from(STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY)?
                == STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN
            && STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN
                + u32::try_from(STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY)?
                == KANRI_NAME_STORAGE_ORIGIN
            && KANRI_NAME_STORAGE_ORIGIN + u32::try_from(KANRI_NAME_STORAGE_BYTE_CAPACITY)?
                <= NAME_ENTRY_TRANSIENT_LOAD_ORIGIN
            && edit_shared_ui_runtime_address(EDIT_SHARED_UI_SIZE)?
                <= NAME_ENTRY_TRANSIENT_LOAD_ORIGIN,
        "KANRI runtime programs or persistent exact glyphs no longer fit their bounded RAM windows"
    );
    Ok(())
}

fn source_catalog_key(record: &FileRecord) -> Result<[u8; 8]> {
    let absolute_sector = record
        .extent_lba
        .checked_add(150)
        .context("MA_ENT absolute sector overflow")?;
    let minute = absolute_sector / (75 * 60);
    let remainder = absolute_sector % (75 * 60);
    let second = remainder / 75;
    let frame = remainder % 75;
    let mut key = [0_u8; 8];
    key[0] = binary_coded_decimal(minute)?;
    key[1] = binary_coded_decimal(second)?;
    key[2] = binary_coded_decimal(frame)?;
    key[4..].copy_from_slice(&record.size.to_le_bytes());
    Ok(key)
}

fn binary_coded_decimal(value: u32) -> Result<u8> {
    ensure!(
        value <= 99,
        "catalog minute or second exceeds two BCD digits"
    );
    Ok(u8::try_from(((value / 10) << 4) | (value % 10))?)
}

fn place_program(assembler: &Assembler, origin: u32, label: &str) -> Result<PlacedProgram> {
    let placed = assembler
        .assemble(origin)
        .with_context(|| format!("failed to assemble {label}"))?;
    ensure!(
        verify_placed_program(placed.bytes(), origin)? == placed.instructions(),
        "{label} failed typed placement verification"
    );
    verify_r3000a_load_delays(placed.instructions(), origin, label)?;
    Ok(PlacedProgram {
        bytes: placed.bytes().to_vec(),
        instructions: placed.instructions().to_vec(),
    })
}

fn edit_shared_ui_runtime_address(offset: usize) -> Result<u32> {
    EDIT_SHARED_UI_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("EDITMOJI runtime address overflow")
}

fn name_renderer_entry_hook(prewarm_wrapper_address: u32) -> [Instruction; 2] {
    [
        Instruction::J {
            target: prewarm_wrapper_address,
        },
        Instruction::nop(),
    ]
}

fn name_code_hook(resolver_address: u32) -> [Instruction; 8] {
    [
        Instruction::Addu {
            rd: Register::A0,
            rs: Register::V1,
            rt: Register::ZERO,
        },
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Jal {
            target: resolver_address,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::S6,
            immediate: -2,
        },
        Instruction::Addu {
            rd: Register::V1,
            rs: Register::V0,
            rt: Register::ZERO,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
    ]
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("KANRI word is truncated")?
            .try_into()?,
    ))
}

pub(super) fn decode_instruction(bytes: &[u8], offset: usize) -> Result<Instruction> {
    decode(
        read_u32(bytes, offset)?,
        OVERLAY_RUNTIME_BASE + u32::try_from(offset)?,
    )
    .context("failed to decode KANRI instruction")
}

fn write_instruction(bytes: &mut [u8], offset: usize, instruction: Instruction) -> Result<()> {
    let address = OVERLAY_RUNTIME_BASE + u32::try_from(offset)?;
    bytes[offset..offset + 4].copy_from_slice(&encode(&instruction, address)?.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::name_input::{
        NAME_FONT_ATLAS_ROW_BYTES, NameGlyphMaterializationBundle, build_default_name_glyph_pack,
        load_ks_x_1001_hangul, load_name_input_keyboard, plan_name_glyph_consumer_layout,
    };

    use super::*;

    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn band_materializer_reads_copied_pack_and_reuses_only_kanri_scratch() {
        use crate::name_input::runtime_test_machine::execute;
        use crate::name_input::{
            NameGlyphBandPack, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
            build_shared_name_outline_runtime_program,
        };
        let font = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
        let repertoire: String = load_ks_x_1001_hangul().unwrap().into_iter().collect();
        let reference =
            crate::font::rasterize_menu_glyphs(&font, &repertoire, 12.0, 3, 13).unwrap();
        let pack = NameGlyphBandPack::build(&reference, NAME_GLYPH_PACK_STORAGE_BYTES).unwrap();
        let bundle = NameGlyphMaterializationBundle::from_bands(&pack);
        let layout = KanriNameStorageLayout::plan(&bundle, [18990, 18990]).unwrap();
        let outline = build_shared_name_outline_runtime_program().unwrap();
        let materializer = build_band_materializer_program(
            MATERIALIZER_PROGRAM_ORIGIN,
            &layout,
            &pack,
            outline.outline_pixel_address,
            outline.outline_cleanup_address,
        )
        .unwrap();
        let origin = SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - 8;
        let mut a = Assembler::new();
        a.emit(Instruction::J {
            target: materializer.materializer_address,
        })
        .emit(Instruction::nop());
        let mut code = a.assemble(origin).unwrap().bytes().to_vec();
        code.extend_from_slice(&outline.bytes);
        code.resize((MATERIALIZER_PROGRAM_ORIGIN - origin) as usize, 0);
        code.extend_from_slice(&materializer.bytes);
        let pack_base = (KANRI_NAME_STORAGE_ORIGIN & 0x1fff_ffff) as usize;
        let scratch = pack_base + layout.scratch_glyph_payload_byte_range[0];
        let mut memory = vec![0x55; 0x200000];
        memory[pack_base..pack_base + pack.bytes().len()].copy_from_slice(pack.bytes());
        for c in ['가', '힣', '각', '쐐', '힝', '힣'] {
            let mut expected = memory.clone();
            let supported = pack.supports(c);
            if supported {
                let glyph = reference.glyphs.iter().find(|g| g.character == c).unwrap();
                for pixel in 0..200 {
                    expected[scratch + pixel] =
                        glyph.pixels[pixel * 2] | glyph.pixels[pixel * 2 + 1] << 4;
                }
            }
            let mut r = std::array::from_fn(|i| i as u32 * 13);
            r[0] = 0;
            r[4] = c as u32 - 0xac00;
            r[29] = 0x801fe000;
            r[31] = 0x800f0000;
            let before = r;
            execute(&code, origin, &mut r, &mut memory);
            assert_eq!(
                r[2],
                if supported {
                    0x80000000 + scratch as u32
                } else {
                    0
                }
            );
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], before[reg]);
            }
            memory[0x1fdfc0..0x1fe000].copy_from_slice(&expected[0x1fdfc0..0x1fe000]);
            assert!(
                memory == expected,
                "KANRI scratch or surrounding memory mismatch for {c}"
            );
        }
    }

    #[test]
    #[ignore = "requires assets/ and fonts in ../fonts/"]
    fn generated_kanri_programs_fit_owned_regions_and_observe_r3000a_load_delays() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let font = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf");
        let pack = build_default_name_glyph_pack(&font).unwrap();
        let materialization = NameGlyphMaterializationBundle::from_pack(&pack);
        let coordinate_range = [18_790, 18_930];
        let layout = KanriNameStorageLayout::plan(&materialization, coordinate_range).unwrap();
        let atlas = test_runtime_atlas();
        let initial_glyphs = synthetic_static_glyphs();
        let dynamic_display_codes = (KANRI_INITIAL_GLYPH_CAPACITY as u16
            ..KANRI_DISPLAY_CODE_CAPACITY as u16)
            .collect::<Vec<_>>();
        let initialization_data =
            build_initialization_data(&initial_glyphs, &dynamic_display_codes, &atlas).unwrap();
        let exact_static_hangul = build_exact_static_hangul_data(&initial_glyphs, &[]).unwrap();
        let materializer = build_materializer_program(
            MATERIALIZER_PROGRAM_ORIGIN,
            &layout,
            &crate::name_input::plan_name_input_runtime_pack(&pack.report).unwrap(),
            0x8008_c22c,
            0x8008_c294,
        )
        .unwrap();
        let keyboard =
            load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json"))
                .unwrap();
        let consumer_layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
        let resolver = build_resolver_program(
            RESOLVER_PROGRAM_ORIGIN,
            &layout,
            materializer.materializer_address,
            nickname_companion_cache_code_range(&consumer_layout).unwrap(),
            0x0053,
            &ImportedDirectNameMap::build(&consumer_layout).unwrap(),
        )
        .unwrap();
        let static_refresher = build_static_refresher_program(
            STATIC_REFRESHER_PROGRAM_ORIGIN,
            &layout,
            &exact_static_hangul,
        )
        .unwrap();
        let runtime_program_copies = [
            RuntimeProgramCopy {
                source_address: edit_shared_ui_runtime_address(MATERIALIZER_STAGING_OFFSET)
                    .unwrap(),
                destination_address: MATERIALIZER_PROGRAM_ORIGIN,
                byte_count: materializer.bytes.len(),
                loop_label: "copy_test_materializer_word",
            },
            RuntimeProgramCopy {
                source_address: edit_shared_ui_runtime_address(RESOLVER_STAGING_OFFSET).unwrap(),
                destination_address: RESOLVER_PROGRAM_ORIGIN,
                byte_count: resolver.bytes.len(),
                loop_label: "copy_test_resolver_word",
            },
            RuntimeProgramCopy {
                source_address: edit_shared_ui_runtime_address(STATIC_REFRESHER_STAGING_OFFSET)
                    .unwrap(),
                destination_address: STATIC_REFRESHER_PROGRAM_ORIGIN,
                byte_count: static_refresher.bytes.len(),
                loop_label: "copy_test_static_refresher_word",
            },
            RuntimeProgramCopy {
                source_address: exact_static_hangul.staging_address().unwrap(),
                destination_address: exact_static_hangul.persistent_address(),
                byte_count: exact_static_hangul.packed_payloads.len(),
                loop_label: "copy_test_static_payload_word",
            },
        ];
        let initializer = build_initializer_program(
            edit_shared_ui_runtime_address(INITIALIZER_OFFSET).unwrap(),
            &layout,
            &runtime_program_copies,
            &initialization_data,
            NAME_ENTRY_TRANSIENT_LOAD_ORIGIN + 0x192e0,
            &exact_static_hangul,
            &static_refresher,
        )
        .unwrap();

        assert_eq!(layout.cache_slot_count, 14);
        assert_eq!(layout.copied_pack_storage_byte_range, [0, 19_000]);
        assert_eq!(
            layout.name_source_snapshot_byte_range[1] - layout.name_source_snapshot_byte_range[0],
            KANRI_NAME_SOURCE_SNAPSHOT_BYTES
        );
        assert!(
            layout.cache_map_byte_range[1] <= layout.name_source_snapshot_byte_range[0]
                && layout.name_source_snapshot_byte_range[1]
                    <= layout.scratch_glyph_payload_byte_range[0]
        );
        assert!(layout.fits_mode_local_arena);
        assert_eq!(
            layout.scratch_glyph_payload_byte_range[1] - layout.scratch_glyph_payload_byte_range[0],
            KANRI_GLYPH_PAYLOAD_BYTES
        );
        assert!(
            initializer.bytes.len() <= INITIALIZER_BYTE_CAPACITY,
            "initializer uses {} of {} bytes",
            initializer.bytes.len(),
            INITIALIZER_BYTE_CAPACITY
        );
        assert!(
            materializer.bytes.len() <= MATERIALIZER_STAGING_BYTE_CAPACITY,
            "materializer uses {} of {} bytes",
            materializer.bytes.len(),
            MATERIALIZER_STAGING_BYTE_CAPACITY
        );
        assert!(
            resolver.bytes.len() <= RESOLVER_STAGING_BYTE_CAPACITY,
            "resolver uses {} of {} bytes",
            resolver.bytes.len(),
            RESOLVER_STAGING_BYTE_CAPACITY
        );
        assert!(materializer.bytes.len() <= MATERIALIZER_PROGRAM_BYTE_CAPACITY);
        assert!(materializer.instructions.contains(&Instruction::Addiu {
            rt: Register::V0,
            rs: Register::A0,
            immediate: -(KANRI_GLYPH_PAYLOAD_BYTES as i16),
        }));
        assert!(resolver.bytes.len() <= RESOLVER_PROGRAM_BYTE_CAPACITY);
        assert!(static_refresher.bytes.len() <= STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY);
        assert!(
            static_refresher.bytes.len()
                >= STATIC_REFRESHER_TABLE_OFFSET + exact_static_hangul.codes.len() * 2
        );
        assert_eq!(
            exact_static_hangul.packed_payloads.len(),
            FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY
        );
        assert!(
            exact_static_hangul.packed_payloads.len()
                <= STATIC_HANGUL_PAYLOAD_STAGING_BYTE_CAPACITY
        );
        assert!(
            exact_static_hangul.packed_payloads.len()
                <= STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY
        );
        assert_eq!(
            exact_static_hangul.persistent_address()
                + u32::try_from(STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY).unwrap(),
            KANRI_NAME_STORAGE_ORIGIN
        );
        assert!(static_refresher.instructions.contains(&Instruction::Jal {
            target: static_refresher.unpacker_address,
        }));
        assert!(static_refresher.instructions.contains(&Instruction::Jal {
            target: STATUS_OVERVIEW_NATIVE_SETUP_ADDRESS,
        }));
        assert_eq!(
            static_refresher
                .instructions
                .iter()
                .filter(|instruction| {
                    **instruction
                        == Instruction::Jal {
                            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
                        }
                })
                .count(),
            1 + KANRI_RAW_ASCII_GLYPH_COUNT
        );
        assert!(static_refresher.instructions.contains(&Instruction::Jal {
            target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
        }));
        assert!(initialization_data.bytes.len() <= INITIALIZATION_DATA_STAGING_BYTE_CAPACITY);
        assert_eq!(
            MATERIALIZER_PROGRAM_ORIGIN + MATERIALIZER_PROGRAM_BYTE_CAPACITY as u32,
            RESOLVER_PROGRAM_ORIGIN
        );
        assert!(
            RESOLVER_PROGRAM_ORIGIN + RESOLVER_PROGRAM_BYTE_CAPACITY as u32
                <= STATIC_REFRESHER_PROGRAM_ORIGIN
                && STATIC_REFRESHER_PROGRAM_ORIGIN + STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY as u32
                    <= KANRI_NAME_STORAGE_ORIGIN
        );
        assert!(
            materializer.materializer_address >= MATERIALIZER_PROGRAM_ORIGIN
                && materializer.materializer_address
                    < MATERIALIZER_PROGRAM_ORIGIN + MATERIALIZER_PROGRAM_BYTE_CAPACITY as u32
        );
        assert!(
            resolver.tagged_code_resolver_address >= RESOLVER_PROGRAM_ORIGIN
                && resolver.tagged_code_resolver_address
                    < RESOLVER_PROGRAM_ORIGIN + RESOLVER_PROGRAM_BYTE_CAPACITY as u32
        );
        assert!(
            resolver.name_renderer_prewarm_wrapper_address >= RESOLVER_PROGRAM_ORIGIN
                && resolver.name_renderer_prewarm_wrapper_address
                    < RESOLVER_PROGRAM_ORIGIN + RESOLVER_PROGRAM_BYTE_CAPACITY as u32
        );
        assert!(resolver.instructions.contains(&Instruction::Jal {
            target: resolver.tagged_code_resolver_address,
        }));
        assert_eq!(
            &resolver.instructions[(resolver.card_name_normalizer_address - RESOLVER_PROGRAM_ORIGIN)
                as usize
                / 4
                - 2
                ..(resolver.card_name_normalizer_address - RESOLVER_PROGRAM_ORIGIN) as usize / 4],
            &[
                Instruction::J {
                    target: NAME_RENDERER_CONTINUATION_ADDRESS,
                },
                Instruction::nop(),
            ]
        );
        let native_parser_index = initializer
            .instructions
            .iter()
            .position(|instruction| {
                *instruction
                    == Instruction::Jalr {
                        rd: Register::RA,
                        rs: Register::S4,
                    }
            })
            .unwrap();
        let name_entry_load_index = initializer
            .instructions
            .iter()
            .position(|instruction| {
                *instruction
                    == Instruction::Jal {
                        target: NATIVE_RECORD_LOADER_ADDRESS,
                    }
            })
            .unwrap();
        assert!(native_parser_index < name_entry_load_index);
        // Execute the initializer while the native font load overwrites its full
        // expanded extent. EDITMOJI contains the active code and lookup tables.
        let off = |address: u32| (address & 0x1fff_ffff) as usize;
        let origin = edit_shared_ui_runtime_address(INITIALIZER_OFFSET).unwrap();
        let mut memory = vec![0x55; 0x200000];
        let ui = off(EDIT_SHARED_UI_RUNTIME_BASE);
        let init_data = ui + INITIALIZATION_DATA_STAGING_OFFSET;
        memory[init_data..init_data + initialization_data.bytes.len()]
            .copy_from_slice(&initialization_data.bytes);
        memory[off(origin)..off(origin) + initializer.bytes.len()]
            .copy_from_slice(&initializer.bytes);
        let before_ui = memory[ui..ui + EDIT_SHARED_UI_SIZE].to_vec();
        let mut registers = [0u32; 32];
        registers[2] = 0x80012340; // intercepted original parser callback
        registers[29] = 0x801fe000;
        registers[31] = 0x80012344;
        let mut loaded = false;
        crate::name_input::runtime_test_machine::execute_with_callbacks_limit(
            &initializer.bytes,
            origin,
            &mut registers,
            &mut memory,
            None,
            1_000_000,
            &mut |pc, r, m| {
                if pc == 0x80012340 {
                    r[2] = 0x1234;
                    return true;
                }
                if pc == NATIVE_RECORD_LOADER_ADDRESS {
                    assert_eq!((r[4], r[5]), (NAME_ENTRY_TRANSIENT_LOAD_ORIGIN, 718));
                    let base = off(r[4]);
                    m[base..base + 229168].fill(0x37);
                    loaded = true;
                    return true;
                }
                if pc == static_refresher.unpacker_address {
                    assert!(loaded);
                    r[4] += 1; // pixel decoding is verified by its separate renderer tests
                    return true;
                }
                pc == VRAM_UPLOAD_ROUTINE_ADDRESS
            },
        );
        assert!(loaded);
        assert_eq!(registers[2], 0x1234);
        assert_eq!(registers[29], 0x801fe000);
        assert_eq!(&memory[ui..ui + EDIT_SHARED_UI_SIZE], &before_ui);
        let copied = off(KANRI_NAME_STORAGE_ORIGIN);
        assert!(
            memory[copied..copied + NAME_GLYPH_PACK_STORAGE_BYTES]
                .iter()
                .all(|&b| b == 0x37)
        );
    }

    // Returning from a model/status screen must restore punctuation from owned
    // persistent pixels even after the transient EDITMOJI payload is overwritten.
    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn status_refresh_keeps_private_punctuation_after_staging_is_reused() {
        use crate::name_input::runtime_test_machine::execute_with_callbacks;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let pack = build_default_name_glyph_pack(
            &root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf"),
        )
        .unwrap();
        let bundle = NameGlyphMaterializationBundle::from_pack(&pack);
        let layout = KanriNameStorageLayout::plan(&bundle, [18_790, 18_930]).unwrap();
        let glyphs = synthetic_static_glyphs();
        let fixed = build_exact_static_hangul_data(&glyphs, &[]).unwrap();
        let init = build_initialization_data(
            &glyphs,
            &(KANRI_INITIAL_GLYPH_CAPACITY as u16..KANRI_DISPLAY_CODE_CAPACITY as u16)
                .collect::<Vec<_>>(),
            &test_runtime_atlas(),
        )
        .unwrap();
        let program =
            build_static_refresher_program(STATIC_REFRESHER_PROGRAM_ORIGIN, &layout, &fixed)
                .unwrap();
        let origin = STATIC_REFRESHER_PROGRAM_ORIGIN - 8;
        let mut entry = Assembler::new();
        entry
            .emit(Instruction::J {
                target: program.entry_address,
            })
            .emit(Instruction::nop());
        let mut code = entry.assemble(origin).unwrap().bytes().to_vec();
        code.extend_from_slice(&program.bytes);
        let mut memory = vec![0x55; 0x200000];
        let runtime = (STATIC_REFRESHER_PROGRAM_ORIGIN & 0x1fffffff) as usize;
        memory[runtime..runtime + program.bytes.len()].copy_from_slice(&program.bytes);
        let packed_base = (STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN & 0x1fffffff) as usize;
        memory[packed_base..packed_base + fixed.packed_payloads.len()]
            .copy_from_slice(&fixed.packed_payloads);
        let persistent = (PRIVATE_ASCII_PERSISTENT_ADDRESS & 0x1fffffff) as usize;
        let expected =
            &init.bytes[init.private_ascii_payload_range[0]..init.private_ascii_payload_range[1]];
        memory[persistent..persistent + expected.len()].copy_from_slice(expected);
        let ascii = (layout
            .runtime_address(layout.ascii_code_table_byte_range[0])
            .unwrap()
            & 0x1fffffff) as usize;
        memory[ascii..ascii + 256].copy_from_slice(
            &init.bytes[init.ascii_code_table_range[0]..init.ascii_code_table_range[1]],
        );
        let stale = (init
            .runtime_address(init.private_ascii_payload_range[0])
            .unwrap()
            & 0x1fffffff) as usize;
        memory[stale..stale + expected.len()].copy_from_slice(expected);
        let mut r = [0; 32];
        r[29] = 0x801fe000;
        r[31] = 0x80010000;
        let mut uploads = Vec::new();
        execute_with_callbacks(&code, origin, &mut r, &mut memory, None, &mut |pc, r, m| {
            if pc == STATUS_OVERVIEW_NATIVE_SETUP_ADDRESS {
                m[stale..stale + expected.len()].fill(0xa5);
                r[2] = 1;
            } else if pc == program.unpacker_address {
                let dst = (r[5] & 0x1fffffff) as usize;
                let glyph = glyphs
                    .iter()
                    .find(|g| g.code == fixed.codes[uploads.len()])
                    .unwrap();
                m[dst..dst + 200].copy_from_slice(&glyph.payload);
                r[4] += 1;
            } else if pc == VRAM_UPLOAD_ROUTINE_ADDRESS {
                let src = (r[5] & 0x1fffffff) as usize;
                uploads.push(m[src..src + KANRI_GLYPH_PAYLOAD_BYTES].to_vec());
            } else if pc != VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS {
                return false;
            }
            true
        });
        assert_eq!(
            uploads.len(),
            KANRI_UI_HANGUL_GLYPH_CAPACITY + KANRI_PRIVATE_ASCII_GLYPH_COUNT
        );
        for (upload, glyph) in uploads
            .iter()
            .zip(glyphs.iter().filter(|g| is_ks_x_1001_hangul(g.character)))
        {
            assert_eq!(upload, &glyph.payload);
        }
        let expected_glyphs = fixed
            .codes
            .iter()
            .map(|code| glyphs.iter().find(|g| g.code == *code).unwrap())
            .chain(
                KANRI_RAW_ASCII_GLYPHS
                    .chars()
                    .map(|ch| glyphs.iter().find(|g| g.character == ch).unwrap()),
            );
        for (upload, glyph) in uploads.iter().zip(expected_glyphs) {
            assert_eq!(
                upload, &glyph.payload,
                "{} was not restored",
                glyph.character
            );
        }
        assert_eq!(uploads[fixed.codes.len()..].concat(), expected);
        assert_eq!(&memory[persistent..persistent + expected.len()], expected);
        assert_eq!(r[29], 0x801fe000);
    }

    #[test]
    #[ignore = "requires assets/ and fonts in ../fonts/"]
    fn renderer_prewarm_fills_cache_before_side_effect_free_lookup() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let font = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf");
        let pack = build_default_name_glyph_pack(&font).unwrap();
        let materialization = NameGlyphMaterializationBundle::from_pack(&pack);
        let layout = KanriNameStorageLayout::plan(&materialization, [18_790, 18_930]).unwrap();
        let materializer = build_materializer_program(
            MATERIALIZER_PROGRAM_ORIGIN,
            &layout,
            &crate::name_input::plan_name_input_runtime_pack(&pack.report).unwrap(),
            0x8008_c22c,
            0x8008_c294,
        )
        .unwrap();
        let keyboard =
            load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json"))
                .unwrap();
        let consumer_layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
        let resolver = build_resolver_program(
            RESOLVER_PROGRAM_ORIGIN,
            &layout,
            materializer.materializer_address,
            nickname_companion_cache_code_range(&consumer_layout).unwrap(),
            0x0053,
            &ImportedDirectNameMap::build(&consumer_layout).unwrap(),
        )
        .unwrap();

        let renderer_hook = name_code_hook(resolver.name_draw_resolver_address);
        assert_eq!(
            renderer_hook[1],
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            }
        );
        assert_eq!(
            renderer_hook[2],
            Instruction::Jal {
                target: resolver.name_draw_resolver_address,
            }
        );

        let prewarm_wrapper_index = usize::try_from(
            (resolver.name_renderer_prewarm_wrapper_address - RESOLVER_PROGRAM_ORIGIN) / 4,
        )
        .unwrap();
        let prewarm_wrapper = &resolver.instructions[prewarm_wrapper_index..];
        assert!(prewarm_wrapper.contains(&Instruction::Ori {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 1,
        }));
        assert!(prewarm_wrapper.contains(&Instruction::Jal {
            target: resolver.tagged_code_resolver_address,
        }));
        assert_eq!(
            name_renderer_entry_hook(resolver.name_renderer_prewarm_wrapper_address)[0],
            Instruction::J {
                target: resolver.name_renderer_prewarm_wrapper_address,
            }
        );
    }

    #[test]
    #[ignore = "requires assets/ and fonts in ../fonts/"]
    fn cache_uploader_preserves_copy_pointers_and_stack_payload_until_sync() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let font = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf");
        let pack = build_default_name_glyph_pack(&font).unwrap();
        let materialization = NameGlyphMaterializationBundle::from_pack(&pack);
        let layout = KanriNameStorageLayout::plan(&materialization, [18_790, 18_930]).unwrap();
        let materializer = build_materializer_program(
            MATERIALIZER_PROGRAM_ORIGIN,
            &layout,
            &crate::name_input::plan_name_input_runtime_pack(&pack.report).unwrap(),
            0x8008_c22c,
            0x8008_c294,
        )
        .unwrap();
        let keyboard =
            load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json"))
                .unwrap();
        let consumer_layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
        let resolver = build_resolver_program(
            RESOLVER_PROGRAM_ORIGIN,
            &layout,
            materializer.materializer_address,
            nickname_companion_cache_code_range(&consumer_layout).unwrap(),
            0x0053,
            &ImportedDirectNameMap::build(&consumer_layout).unwrap(),
        )
        .unwrap();
        let uploader_instruction_count = usize::try_from(
            (resolver.tagged_code_resolver_address - resolver.cache_uploader_address) / 4,
        )
        .unwrap();
        let uploader = &resolver.instructions[..uploader_instruction_count];

        assert!(uploader.contains(&Instruction::Lbu {
            rt: Register::T5,
            base: Register::S1,
            offset: 0,
        }));
        assert!(uploader.contains(&Instruction::Sb {
            rt: Register::T5,
            base: Register::S2,
            offset: 0,
        }));
        assert!(!uploader.contains(&Instruction::Lbu {
            rt: Register::T5,
            base: Register::T1,
            offset: 0,
        }));
        assert!(!uploader.contains(&Instruction::Sb {
            rt: Register::T5,
            base: Register::T2,
            offset: 0,
        }));

        let upload_index = uploader
            .iter()
            .position(|instruction| {
                *instruction
                    == Instruction::Jal {
                        target: VRAM_UPLOAD_ROUTINE_ADDRESS,
                    }
            })
            .unwrap();
        let sync_index = uploader
            .iter()
            .position(|instruction| {
                *instruction
                    == Instruction::Jal {
                        target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
                    }
            })
            .unwrap();
        let stack_release_index = uploader
            .iter()
            .position(|instruction| {
                *instruction
                    == Instruction::Addiu {
                        rt: Register::SP,
                        rs: Register::SP,
                        immediate: CACHE_UPLOAD_STACK_BYTES,
                    }
            })
            .unwrap();
        assert!(upload_index < sync_index && sync_index < stack_release_index);
    }

    #[test]
    #[ignore = "requires assets/ and fonts in ../fonts/"]
    fn full_password_and_private_punctuation_fit_the_owned_refresher() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let pack = build_default_name_glyph_pack(
            &root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf"),
        )
        .unwrap();
        let bundle = NameGlyphMaterializationBundle::from_pack(&pack);
        let layout = KanriNameStorageLayout::plan(&bundle, [18_790, 18_930]).unwrap();
        let alphabet = crate::edit_runtime_text::password_alphabet::PasswordAlphabet::load(
            &root.join("assets/menu/mode-descendants"),
        )
        .unwrap();
        let password = alphabet
            .characters()
            .map(|character| KanriStaticGlyph {
                character,
                code: crate::edit_runtime_text::password_alphabet::PasswordAlphabet::glyph_code(
                    character,
                )
                .unwrap(),
                payload: vec![0x03; KANRI_GLYPH_PAYLOAD_BYTES],
            })
            .collect::<Vec<_>>();
        // Synthetic UI codes must not alias the real password destinations.
        let mut glyphs = synthetic_static_glyphs();
        for glyph in &mut glyphs {
            glyph.code += 0x200;
        }
        let fixed = build_exact_static_hangul_data(&glyphs, &password).unwrap();
        let program =
            build_static_refresher_program(STATIC_REFRESHER_PROGRAM_ORIGIN, &layout, &fixed)
                .unwrap();
        assert!(program.bytes.len() <= STATIC_REFRESHER_PROGRAM_BYTE_CAPACITY);
        assert!(program.instructions.len() * 4 <= STATIC_REFRESHER_TABLE_OFFSET);

        for character in KANRI_PRIVATE_ASCII_GLYPHS
            .chars()
            .filter(|ch| !KANRI_RAW_ASCII_GLYPHS.contains(*ch))
        {
            assert!(
                fixed.codes.contains(
                    &glyphs
                        .iter()
                        .find(|g| g.character == character)
                        .unwrap()
                        .code
                )
            );
        }
    }

    #[test]
    fn tagged_ascii_table_uses_verified_common_codes_and_private_punctuation() {
        let data = build_initialization_data(
            &synthetic_static_glyphs(),
            &(KANRI_INITIAL_GLYPH_CAPACITY as u16..KANRI_DISPLAY_CODE_CAPACITY as u16)
                .collect::<Vec<_>>(),
            &test_runtime_atlas(),
        )
        .unwrap();
        let table = &data.bytes[data.ascii_code_table_range[0]..data.ascii_code_table_range[1]];
        let code = |character: char| {
            let offset = usize::try_from(u32::from(character)).unwrap() * 2;
            u16::from_le_bytes(table[offset..offset + 2].try_into().unwrap())
        };

        assert_eq!(code('A'), 0x000a);
        assert_eq!(code('z'), 0x0051);
        assert_eq!(code('!'), 0x0063);
        assert_eq!(code(' '), SKIP_GLYPH_CODE);
        let private_codes = KANRI_PRIVATE_ASCII_GLYPHS
            .chars()
            .map(code)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(private_codes.len(), KANRI_PRIVATE_ASCII_GLYPH_COUNT);
        assert!(
            private_codes
                .iter()
                .all(|code| usize::from(*code) < KANRI_INITIAL_GLYPH_CAPACITY)
        );
        assert_eq!(code('['), u16::MAX);
        assert_eq!(
            data.private_ascii_payload_range[1] - data.private_ascii_payload_range[0],
            2 * KANRI_GLYPH_PAYLOAD_BYTES
        );
    }

    #[test]
    fn authored_glyph_population_can_change_within_the_reserved_capacity() {
        let all = synthetic_static_glyphs();
        let dynamic = (KANRI_INITIAL_GLYPH_CAPACITY as u16..KANRI_DISPLAY_CODE_CAPACITY as u16)
            .collect::<Vec<_>>();
        for count in [17, 100, 103, KANRI_UI_HANGUL_GLYPH_CAPACITY] {
            let glyphs = all
                .iter()
                .filter(|g| !is_ks_x_1001_hangul(g.character))
                .chain(
                    all.iter()
                        .filter(|g| is_ks_x_1001_hangul(g.character))
                        .take(count),
                )
                .cloned()
                .collect::<Vec<_>>();
            let data = build_initialization_data(&glyphs, &dynamic, &test_runtime_atlas()).unwrap();
            assert_eq!(
                data.dynamic_display_code_table_range[1] - data.dynamic_display_code_table_range[0],
                KANRI_NAME_CACHE_SLOT_COUNT * 2
            );
            let fixed = build_exact_static_hangul_data(&glyphs, &[]).unwrap();
            assert_eq!(
                fixed.codes.len(),
                count + KANRI_PRIVATE_ASCII_GLYPH_COUNT - KANRI_RAW_ASCII_GLYPH_COUNT
            );
        }
    }

    #[test]
    fn menu_ascii_payload_is_uploaded_without_rebinding_name_ascii() {
        let mut glyphs = synthetic_static_glyphs();
        glyphs.retain(|g| !is_ks_x_1001_hangul(g.character) || g.character == '가');
        let mut menu_ascii = glyphs[0].clone();
        menu_ascii.character = 'A';
        menu_ascii.code = 0x0330;
        glyphs.push(menu_ascii);
        let dynamic = (KANRI_INITIAL_GLYPH_CAPACITY as u16..KANRI_DISPLAY_CODE_CAPACITY as u16)
            .collect::<Vec<_>>();
        let init = build_initialization_data(&glyphs, &dynamic, &test_runtime_atlas()).unwrap();
        let offset = init.ascii_code_table_range[0] + usize::from(b'A') * 2;
        assert_eq!(
            u16::from_le_bytes(init.bytes[offset..offset + 2].try_into().unwrap()),
            0x000a
        );
        let fixed = build_exact_static_hangul_data(&glyphs, &[]).unwrap();
        assert!(fixed.codes.contains(&0x0330));
        assert!(!fixed.codes.contains(&0x000a));
        glyphs.last_mut().unwrap().character = '🦀';
        assert!(build_initialization_data(&glyphs, &dynamic, &test_runtime_atlas()).is_err());
    }

    #[test]
    fn ma_ent_catalog_key_matches_the_native_loader_entry() {
        let record = FileRecord {
            extent_lba: 28_248,
            extended_attribute_blocks: 0,
            size: 109_762,
            flags: 0,
            name: "MA_ENT.BIZ".to_string(),
        };
        assert_eq!(
            source_catalog_key(&record).unwrap(),
            NAME_ENTRY_FONT_CATALOG_ENTRY[..8]
        );
    }

    fn test_runtime_atlas() -> NameInputRuntimeAtlasLayout {
        NameInputRuntimeAtlasLayout {
            font_atlas_row_bytes: NAME_FONT_ATLAS_ROW_BYTES,
            glyph_cell_width: 20,
            glyph_cell_height: 20,
            pack_storage_cell_base_byte_offsets: (0..NAME_GLYPH_PACK_CELL_COUNT)
                .map(|index| u16::try_from(index * NAME_GLYPH_CELL_BYTES).unwrap())
                .collect(),
            cache_cell_base_byte_offsets: Vec::new(),
            lookup_table_bytes: (0..NAME_GLYPH_PACK_CELL_COUNT)
                .flat_map(|index| {
                    u16::try_from(index * NAME_GLYPH_CELL_BYTES)
                        .unwrap()
                        .to_le_bytes()
                })
                .collect(),
        }
    }

    fn synthetic_static_glyphs() -> Vec<KanriStaticGlyph> {
        load_ks_x_1001_hangul()
            .unwrap()
            .into_iter()
            .take(KANRI_UI_HANGUL_GLYPH_CAPACITY)
            .chain(KANRI_PRIVATE_ASCII_GLYPHS.chars())
            .enumerate()
            .map(|(index, character)| {
                let mut payload = vec![0; KANRI_GLYPH_PAYLOAD_BYTES];
                payload[40..160].fill(if index.is_multiple_of(2) { 0xe3 } else { 0x30 });
                KanriStaticGlyph {
                    character,
                    code: u16::try_from(index).unwrap(),
                    payload,
                }
            })
            .collect()
    }
}
