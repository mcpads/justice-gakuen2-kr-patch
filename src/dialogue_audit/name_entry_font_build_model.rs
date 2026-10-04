use serde::Serialize;

use crate::tim::Cell;

use super::name_entry_fixed_graphics_model::DialogueNameEntryFixedGraphicBuildReport;
use super::name_entry_runtime_code::NameEntryRuntimeCodeRegionAudit;
use crate::name_input::NameInputRuntimeProgramReport;

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryFontBuildReport {
    pub kind: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub source_record_size: usize,
    pub runtime_code_region: NameEntryRuntimeCodeRegionAudit,
    pub runtime_program: NameInputRuntimeProgramReport,
    pub embedded_font_tim_offset: String,
    pub embedded_font_tim_size: usize,
    pub embedded_font_image_vram_x: u16,
    pub embedded_font_image_vram_y: u16,
    pub embedded_font_image_width: usize,
    pub embedded_font_image_height: usize,
    pub source_background_tim_overlap_byte_count: usize,
    pub glyph_layout_table_offset: String,
    pub glyph_layout_table_sha256: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub outline_palette_index: u8,
    pub fill_palette_index: u8,
    pub installed_glyph_count: usize,
    pub changed_decoded_byte_count: usize,
    pub patched_decoded_sha256: String,
    pub compressed_stream_size: usize,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub source_compression_stream_byte_count: usize,
    pub source_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub patched_stored_sha256: String,
    pub stored_output_file: String,
    pub all_glyph_cells_unique: bool,
    pub compressed_within_original_extent: bool,
    pub compression_roundtrip_verified: bool,
    pub fixed_graphics: Vec<DialogueNameEntryFixedGraphicBuildReport>,
    pub all_fixed_graphic_surfaces_disjoint: bool,
    pub installs: Vec<DialogueNameEntryGlyphInstall>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryGlyphInstall {
    pub source_page: String,
    pub page_position: usize,
    pub character: String,
    pub code: String,
    pub texture_page: u8,
    pub column: u8,
    pub row: u8,
    pub cell: Cell,
    pub indexed_pixels_sha256: String,
    pub changed_decoded_byte_count: usize,
}
