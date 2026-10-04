use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{NAME_GLYPH_PACK_STORAGE_BYTES, NameGlyphMaterializationBundle};

/// KANRI owns this RAM only after its overlay and UI assets have finished loading.
/// Nothing in this arena is expected to survive a mode transition.
pub const KANRI_NAME_STORAGE_ORIGIN: u32 = 0x800b_0000;
pub const KANRI_NAME_STORAGE_BYTE_CAPACITY: usize = 0x5000;
// The 0x400-byte refresher reservation holds 158 instruction words and
// 195 code entries: up to 119 packed UI glyphs plus 76 password glyphs.
// Hangul and role-specific ASCII share the UI budget; two private ASCII
// glyphs remain in their separate raw staging slots.
// Actual populations come from the authored text and can be smaller.
pub const KANRI_UI_HANGUL_GLYPH_CAPACITY: usize = 116;
pub const KANRI_PRIVATE_ASCII_GLYPHS: &str = "_:.\"'";
// These two older raw payloads keep their fixed staging/import offsets.
// Further private symbols use the shared packed fixed-glyph stream.
pub const KANRI_RAW_ASCII_GLYPHS: &str = "_:";
pub const KANRI_RAW_ASCII_GLYPH_COUNT: usize = KANRI_RAW_ASCII_GLYPHS.len();
pub const KANRI_PRIVATE_ASCII_GLYPH_COUNT: usize = KANRI_PRIVATE_ASCII_GLYPHS.len();
pub const KANRI_INITIAL_GLYPH_CAPACITY: usize =
    KANRI_UI_HANGUL_GLYPH_CAPACITY + KANRI_PRIVATE_ASCII_GLYPH_COUNT;
pub const KANRI_NAME_CACHE_SLOT_COUNT: usize = 14;
pub const KANRI_DISPLAY_CODE_CAPACITY: usize =
    KANRI_INITIAL_GLYPH_CAPACITY + KANRI_NAME_CACHE_SLOT_COUNT;
pub const KANRI_GLYPH_PAYLOAD_BYTES: usize = 20 * 20 / 2;
pub const KANRI_ASCII_CODE_TABLE_ENTRY_COUNT: usize = 128;
pub const KANRI_SAVED_NAME_RECORD_COUNT: usize = 5;
pub const KANRI_RENDERED_NAME_WORD_COUNT: usize = 5;
pub const KANRI_NAME_SOURCE_SNAPSHOT_SLOT_BYTES: usize = 2 + KANRI_RENDERED_NAME_WORD_COUNT * 2;
pub const KANRI_NAME_SOURCE_SNAPSHOT_BYTES: usize =
    KANRI_SAVED_NAME_RECORD_COUNT * KANRI_NAME_SOURCE_SNAPSHOT_SLOT_BYTES;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KanriNameStorageLayout {
    pub origin: String,
    pub byte_capacity: usize,
    pub copied_pack_storage_byte_range: [usize; 2],
    pub pack_payload_byte_range: [usize; 2],
    pub runtime_coordinate_list_byte_range: [usize; 2],
    pub ascii_code_table_byte_range: [usize; 2],
    pub dynamic_display_code_table_byte_range: [usize; 2],
    pub cache_map_byte_range: [usize; 2],
    pub name_source_snapshot_byte_range: [usize; 2],
    pub scratch_glyph_payload_byte_range: [usize; 2],
    pub initial_glyph_capacity: usize,
    pub cache_slot_count: usize,
    pub fits_mode_local_arena: bool,
}

impl KanriNameStorageLayout {
    pub(crate) fn plan(
        bundle: &NameGlyphMaterializationBundle,
        runtime_coordinate_list_storage_byte_range: [usize; 2],
    ) -> Result<Self> {
        bundle.validate()?;
        let copied_pack_storage_byte_range = [0, NAME_GLYPH_PACK_STORAGE_BYTES];
        let pack_payload_byte_range = [0, bundle.pack_bytes.len()];
        ensure!(
            pack_payload_byte_range[1] <= runtime_coordinate_list_storage_byte_range[0]
                && runtime_coordinate_list_storage_byte_range[1]
                    <= copied_pack_storage_byte_range[1]
                && runtime_coordinate_list_storage_byte_range[1]
                    - runtime_coordinate_list_storage_byte_range[0]
                    == bundle.runtime_coordinate_list.len(),
            "KANRI MA_ENT coordinate list is outside the copied glyph-pack cells"
        );
        let ascii_code_table_byte_range = [
            copied_pack_storage_byte_range[1],
            copied_pack_storage_byte_range[1] + KANRI_ASCII_CODE_TABLE_ENTRY_COUNT * 2,
        ];
        let dynamic_display_code_table_byte_range = [
            ascii_code_table_byte_range[1],
            ascii_code_table_byte_range[1] + KANRI_NAME_CACHE_SLOT_COUNT * 2,
        ];
        let cache_map_byte_range = [
            dynamic_display_code_table_byte_range[1],
            dynamic_display_code_table_byte_range[1] + KANRI_NAME_CACHE_SLOT_COUNT * 2,
        ];
        let name_source_snapshot_byte_range = [
            cache_map_byte_range[1],
            cache_map_byte_range[1] + KANRI_NAME_SOURCE_SNAPSHOT_BYTES,
        ];
        let scratch_glyph_payload_start = name_source_snapshot_byte_range[1].next_multiple_of(4);
        let scratch_glyph_payload_byte_range = [
            scratch_glyph_payload_start,
            scratch_glyph_payload_start + KANRI_GLYPH_PAYLOAD_BYTES,
        ];
        ensure!(
            scratch_glyph_payload_byte_range[1] <= KANRI_NAME_STORAGE_BYTE_CAPACITY,
            "KANRI name materialization storage exceeds its mode-local arena"
        );
        Ok(Self {
            origin: format!("0x{KANRI_NAME_STORAGE_ORIGIN:08x}"),
            byte_capacity: KANRI_NAME_STORAGE_BYTE_CAPACITY,
            copied_pack_storage_byte_range,
            pack_payload_byte_range,
            runtime_coordinate_list_byte_range: runtime_coordinate_list_storage_byte_range,
            ascii_code_table_byte_range,
            dynamic_display_code_table_byte_range,
            cache_map_byte_range,
            name_source_snapshot_byte_range,
            scratch_glyph_payload_byte_range,
            initial_glyph_capacity: KANRI_INITIAL_GLYPH_CAPACITY,
            cache_slot_count: KANRI_NAME_CACHE_SLOT_COUNT,
            fits_mode_local_arena: true,
        })
    }

    pub(crate) fn runtime_address(&self, offset: usize) -> Result<u32> {
        ensure!(
            offset <= KANRI_NAME_STORAGE_BYTE_CAPACITY,
            "KANRI name storage address leaves its capacity"
        );
        KANRI_NAME_STORAGE_ORIGIN
            .checked_add(u32::try_from(offset)?)
            .context("KANRI name storage address overflow")
    }
}
