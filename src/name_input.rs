#[path = "name_input/glyph_bands.rs"]
mod glyph_bands;
pub use glyph_bands::{
    NameGlyphBandBuild, NameGlyphBandBuildReport, NameGlyphBandPack, build_name_glyph_band_pack,
};
#[path = "name_input/cache.rs"]
mod cache;
#[path = "name_input/composition.rs"]
mod composition;
#[path = "name_input/consumer_layout.rs"]
mod consumer_layout;
#[path = "name_input/dialogue_runtime.rs"]
mod dialogue_runtime;
#[path = "name_input/dialogue_runtime_bootstrap.rs"]
mod dialogue_runtime_bootstrap;
#[path = "name_input/glyph_pack.rs"]
mod glyph_pack;
#[path = "name_input/medial.rs"]
mod medial;
pub(crate) use glyph_pack::emit_name_glyph_pack_copy;
#[path = "name_input/kanri_storage.rs"]
mod kanri_storage;
#[path = "name_input/keyboard.rs"]
mod keyboard;
#[path = "name_input/model.rs"]
mod model;
#[path = "name_input/navigation.rs"]
mod navigation;
#[path = "name_input/nickname_hud_layout.rs"]
mod nickname_hud_layout;
#[path = "name_input/outline_runtime.rs"]
mod outline_runtime;
#[path = "name_input/record.rs"]
mod record;
#[path = "name_input/redisplay_runtime.rs"]
mod redisplay_runtime;
#[path = "name_input/repertoire.rs"]
mod repertoire;
#[path = "name_input/runtime.rs"]
mod runtime;
#[path = "name_input/runtime_atlas.rs"]
mod runtime_atlas;
#[path = "name_input/runtime_pack.rs"]
mod runtime_pack;
#[path = "name_input/slot_cursor.rs"]
mod slot_cursor;
#[path = "name_input/spec.rs"]
mod spec;

pub use consumer_layout::{
    NameGlyphActiveCell, NameGlyphConsumerLayout, NameGlyphPackCodeCell,
    plan_name_glyph_consumer_layout,
};
pub use dialogue_runtime::{
    MGAME_RELOAD_WRAPPER_BYTE_CAPACITY, MGAME_RELOAD_WRAPPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
    MgameRuntimeRepairProgram, MgameRuntimeRepairProgramReport,
    NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_ORIGIN,
    NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN,
    NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
    NameDialogueRuntimeProgram, NameDialogueRuntimeProgramReport,
    build_name_dialogue_band_runtime_program, build_name_dialogue_runtime_program,
};
pub use dialogue_runtime_bootstrap::{
    NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
    NICKNAME_HUD_GLYPH_STORE_ORIGIN, NICKNAME_HUD_MAGIC_ADDRESS,
    NICKNAME_HUD_PERSISTENT_CELL_ORIGIN, NICKNAME_HUD_STORAGE_OFFSET,
    NameDialogueRuntimeBootstrapProgram, NameDialogueRuntimeBootstrapProgramReport,
    build_name_dialogue_runtime_bootstrap_program,
};
pub use glyph_pack::{
    NameGlyphFillCorrections, NameGlyphMaterializationBundle, NameGlyphPack,
    NameGlyphPackCellPayload, NameGlyphPackDecoder, NameGlyphPackReport,
    NameGlyphPackRuntimeLookupInstallReport, NameGlyphPackRuntimeLookupPayload,
    NameGlyphPackStorageImage, NameGlyphPackTimInstallReport, build_default_name_glyph_pack,
    build_name_fill_correction_program, build_name_glyph_pack, encode_name_glyph_pack_cells,
    install_name_glyph_pack_auxiliary, install_name_glyph_pack_cells_in_tim,
    install_name_glyph_pack_runtime_lookup, read_name_glyph_pack_cells,
};
pub use kanri_storage::{
    KANRI_ASCII_CODE_TABLE_ENTRY_COUNT, KANRI_DISPLAY_CODE_CAPACITY, KANRI_GLYPH_PAYLOAD_BYTES,
    KANRI_INITIAL_GLYPH_CAPACITY, KANRI_NAME_CACHE_SLOT_COUNT, KANRI_NAME_SOURCE_SNAPSHOT_BYTES,
    KANRI_NAME_SOURCE_SNAPSHOT_SLOT_BYTES, KANRI_NAME_STORAGE_BYTE_CAPACITY,
    KANRI_NAME_STORAGE_ORIGIN, KANRI_PRIVATE_ASCII_GLYPH_COUNT, KANRI_PRIVATE_ASCII_GLYPHS,
    KANRI_RAW_ASCII_GLYPH_COUNT, KANRI_RAW_ASCII_GLYPHS, KANRI_RENDERED_NAME_WORD_COUNT,
    KANRI_SAVED_NAME_RECORD_COUNT, KANRI_UI_HANGUL_GLYPH_CAPACITY, KanriNameStorageLayout,
};
pub use keyboard::{
    DIGIT_KEYS, HANGUL_CONSONANT_KEYS, HANGUL_VOWEL_KEYS, LATIN_KEYS, SYMBOL_KEYS, keyboard_keys,
};
pub use model::{
    HangulCompositionState, NameField, NameGlyphCachePlan, NameGlyphCacheSlot, NameGlyphPackCell,
    NameGlyphPackStoragePlan, NameInputEditor, NameInputKey, NameInputKeyAssignment,
    NameInputKeyboardPlan, NameInputPage, NameInputPagePlan,
};
pub use navigation::{
    NAME_CANDIDATE_POSITION_COUNT, NAME_NAVIGATION_MAP_BYTES, NAME_NAVIGATION_POSITION_COUNT,
    NameNavigationMap, build_sparse_name_navigation,
};
pub use nickname_hud_layout::{
    NicknameHudGlyphLayout, NicknameHudGlyphStyle, plan_nickname_hud_glyph_layout,
    plan_nickname_hud_glyph_layout_for_crop,
};
pub use outline_runtime::{
    SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
    SharedNameOutlineRuntimeProgram, SharedNameOutlineRuntimeProgramReport,
    build_shared_name_outline_runtime_program,
};
pub use record::{
    ASCII_NAME_TAG, CanonicalNameRecord, EMPTY_NAME_SLOT, HANGUL_NAME_TAG,
    NICKNAME_COMPANION_TO_NICKNAME_BYTE_DISPLACEMENT, NameSlotCode,
};
pub use redisplay_runtime::{
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    NameInputRedisplayRuntimeProgram, NameInputRedisplayRuntimeProgramReport,
    build_name_input_band_redisplay_runtime_program, build_name_input_redisplay_runtime_program,
};
pub use repertoire::{KS_X_1001_HANGUL_COUNT, is_ks_x_1001_hangul, load_ks_x_1001_hangul};
pub use runtime::{
    NAME_INPUT_RUNTIME_BYTE_CAPACITY, NAME_INPUT_RUNTIME_ORIGIN, NameInputRuntimeProgram,
    NameInputRuntimeProgramReport, build_name_input_band_runtime_program,
    build_name_input_runtime_program, build_name_input_runtime_program_with_atlas,
    build_name_input_runtime_program_with_stored_atlas_and_outline,
};
pub(crate) use runtime::{emit_component_resolver, emit_glyph_fill_materializer};
pub use runtime_atlas::{
    NAME_FONT_ATLAS_ROW_BYTES, NameInputRuntimeAtlasLayout, plan_name_input_runtime_atlas,
};
pub use runtime_pack::{NameInputRuntimePackLayout, plan_name_input_runtime_pack};
pub use slot_cursor::{NameRecordSlotCursor, NameRecordSlotSelection};
pub use spec::load_name_input_keyboard;

#[cfg(test)]
#[path = "name_input/cache_tests.rs"]
mod cache_tests;
#[cfg(test)]
#[path = "name_input/composition_tests.rs"]
mod composition_tests;
#[cfg(test)]
#[path = "name_input/consumer_layout_tests.rs"]
mod consumer_layout_tests;
#[cfg(test)]
#[path = "name_input/dialogue_runtime_bootstrap_tests.rs"]
mod dialogue_runtime_bootstrap_tests;
#[cfg(test)]
#[path = "name_input/dialogue_runtime_tests.rs"]
mod dialogue_runtime_tests;
#[cfg(test)]
#[path = "name_input/glyph_fill_materializer_tests.rs"]
mod glyph_fill_materializer_tests;
#[cfg(test)]
#[path = "name_input/glyph_pack_storage_tests.rs"]
mod glyph_pack_storage_tests;
#[cfg(test)]
#[path = "name_input/glyph_pack_tests.rs"]
mod glyph_pack_tests;
#[cfg(test)]
#[path = "name_input/keyboard_tests.rs"]
mod keyboard_tests;
#[cfg(test)]
#[path = "name_input/navigation_tests.rs"]
mod navigation_tests;
#[cfg(test)]
#[path = "name_input/nickname_hud_layout_tests.rs"]
mod nickname_hud_layout_tests;
#[cfg(test)]
#[path = "name_input/outline_runtime_tests.rs"]
mod outline_runtime_tests;
#[cfg(test)]
#[path = "name_input/record_tests.rs"]
mod record_tests;
#[cfg(test)]
#[path = "name_input/redisplay_runtime_tests.rs"]
mod redisplay_runtime_tests;
#[cfg(test)]
#[path = "name_input/runtime_atlas_tests.rs"]
mod runtime_atlas_tests;
#[cfg(test)]
#[path = "name_input/runtime_pack_tests.rs"]
mod runtime_pack_tests;
#[cfg(test)]
#[path = "name_input/runtime_tests.rs"]
mod runtime_tests;
#[cfg(test)]
#[path = "name_input/slot_cursor_tests.rs"]
mod slot_cursor_tests;
#[cfg(test)]
#[path = "name_input/spec_tests.rs"]
mod spec_tests;
pub use cache::{
    NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_CELL_BYTES, NAME_GLYPH_CODE_COUNT,
    NAME_GLYPH_CODE_START, NAME_GLYPH_PACK_CELL_COUNT, NAME_GLYPH_PACK_STORAGE_BYTES,
};

#[cfg(test)]
#[path = "name_input/runtime_test_machine.rs"]
pub(crate) mod runtime_test_machine;

pub(crate) use glyph_pack::NameGlyphMaterializationFormat;

#[cfg(test)]
#[path = "name_input/editing_runtime_tests.rs"]
pub(crate) mod editing_runtime_tests;

pub use repertoire::{is_name_input_hangul, load_name_input_hangul};

pub(crate) use runtime_atlas::NameInputDirectGlyphSource;

#[path = "name_input/selector_loader.rs"]
mod selector_loader;
pub use selector_loader::{
    NameAssetLoad, SelectorNameLoaderLayout, SelectorNameLoaderProgram, build_selector_name_loader,
};

#[path = "name_input/selector_bootstrap.rs"]
mod selector_bootstrap;
pub use selector_bootstrap::{
    SELECTOR_BOOTSTRAP_CALL_SITE, SELECTOR_BOOTSTRAP_RETURN, SelectorBootstrapLayout,
    SelectorDataCopy, SelectorPayloadSplit, build_selector_bootstrap,
    build_selector_bootstrap_entry, build_selector_bootstrap_with_data,
};

#[path = "name_input/contiguous_pack_reader.rs"]
mod contiguous_pack_reader;
pub(crate) use contiguous_pack_reader::emit_contiguous_pack_byte_reader;

#[path = "name_input/selector_materializer.rs"]
mod selector_materializer;
pub use selector_materializer::{
    SelectorMaterializerLayout, SelectorMaterializerProgram, build_selector_materializer,
};

#[path = "name_input/selector_runtime.rs"]
mod selector_runtime;
pub use selector_runtime::{
    SelectorRuntimeLayout, SelectorRuntimeProgram, build_roster_runtime, build_selector_runtime,
};

#[path = "name_input/selector_upload.rs"]
mod selector_upload;
pub use selector_upload::{SelectorUploadLayout, build_selector_upload};

#[path = "name_input/selector_sprite.rs"]
mod selector_sprite;
pub use selector_sprite::{SelectorSpriteLayout, build_selector_sprite};

#[path = "name_input/selector_dispatch.rs"]
mod selector_dispatch;
pub use selector_dispatch::{SELECTOR_NAME_ADVANCE, build_selector_name_dispatch};

#[path = "name_input/roster_glyph.rs"]
mod roster_glyph;
pub use roster_glyph::{RosterGlyphLayout, build_roster_glyph};

#[path = "name_input/roster_adapter.rs"]
mod roster_adapter;
pub use roster_adapter::{RosterAdapterLayout, RosterValue, build_roster_adapter};

#[path = "name_input/roster_source_hooks.rs"]
mod roster_source_hooks;
pub use roster_source_hooks::{RosterSourceHooks, build_roster_source_hooks};

#[path = "name_input/selector_ascii.rs"]
mod selector_ascii;
pub use selector_ascii::SelectorAsciiGlyphs;
#[path = "name_input/selector_ascii_runtime.rs"]
mod selector_ascii_runtime;
#[path = "name_input/selector_source_hooks.rs"]
mod selector_source_hooks;
pub use selector_source_hooks::{SelectorSourceHooks, build_selector_source_hooks};
#[path = "name_input/selector_installation.rs"]
mod selector_installation;
pub use selector_installation::SelectorRuntimeInstallReport;
pub(crate) use selector_installation::SelectorRuntimeInstallation;
#[path = "name_input/roster_font.rs"]
mod roster_font;
pub(crate) use roster_font::RosterNameFont;
pub use roster_font::RosterNameFontReport;
#[path = "name_input/roster_installation.rs"]
mod roster_installation;
pub(crate) use roster_installation::RosterRuntimeInstallation;

// Native selection bridge owns the reverse-edit helper inside MGENT.
pub(crate) const NAME_INPUT_MEDIAL_BACKSPACE_ADDRESS: u32 = 0x8018_1248;
pub(crate) use runtime::MEDIAL_TRANSITION_ADDRESS as NAME_INPUT_MEDIAL_TRANSITION_ADDRESS;

#[path = "name_input/battle_loader.rs"]
mod battle_loader;
pub use battle_loader::{
    BATTLE_NAME_FONT_PIXELS, BATTLE_NAME_GENERATOR_CAPACITY, BATTLE_NAME_GENERATOR_ORIGIN,
    BATTLE_NAME_LEGACY_STORAGE_BYTES, BATTLE_NAME_LOAD_DESTINATION,
    BATTLE_NAME_NATIVE_SUPPLIER_BYTES, build_battle_name_loader,
};
#[path = "name_input/battle_sprite.rs"]
mod battle_sprite;
pub use battle_sprite::{BATTLE_NAME_SPRITE_CONTINUATION, build_battle_name_sprite};
#[path = "name_input/battle_glyph_dispatch.rs"]
mod battle_glyph_dispatch;
pub use battle_glyph_dispatch::build_battle_glyph_dispatch;
#[path = "name_input/battle_legacy_glyph.rs"]
mod battle_legacy_glyph;
pub use battle_legacy_glyph::build_battle_legacy_glyph;
#[path = "name_input/battle_runtime.rs"]
mod battle_runtime;
pub use battle_runtime::{
    BATTLE_NAME_BUFFER_ADDRESS, BATTLE_NAME_PACK_ADDRESS, BATTLE_NAME_SCRATCH_ADDRESS,
    BattleNameRuntimeProgram, build_battle_name_runtime,
};
