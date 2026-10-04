use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DiscAssetComparisonConfig {
    pub source_cue: PathBuf,
    pub patched_cue: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct FixedPresentationGlyphSourceAuditConfig {
    pub source_cue: PathBuf,
    pub patched_cue: PathBuf,
    pub source_decoded: PathBuf,
    pub codebook: PathBuf,
    pub allocation: PathBuf,
    pub runtime_report: PathBuf,
    pub runtime_gpu_dump: Option<PathBuf>,
    pub runtime_ram_dump: Option<PathBuf>,
    pub runtime_texture_source_min_pixel_count: Option<usize>,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct FixedPresentationGlyphSourceAuditReport {
    pub kind: String,
    pub source_cue: String,
    pub source_bin_sha256: String,
    pub patched_cue: String,
    pub patched_bin_sha256: String,
    pub source_decoded_sha256: String,
    pub codebook_sha256: String,
    pub allocation_sha256: String,
    pub runtime_report_sha256: String,
    pub runtime_gpu_dump_sha256: Option<String>,
    pub runtime_ram_dump_sha256: Option<String>,
    pub runtime_texture_source_min_pixel_count: Option<usize>,
    pub runtime_texture_target_count: usize,
    pub runtime_sprite_cell_count: usize,
    pub runtime_sprite_cells: Vec<FixedPresentationRuntimeSpriteCellAudit>,
    pub target_count: usize,
    pub japanese_script_target_count: usize,
    pub runtime_observed_target_count: usize,
    pub source_iso_file_count: usize,
    pub patched_iso_file_count: usize,
    pub source_decoded_layer_count: usize,
    pub patched_decoded_layer_count: usize,
    pub source_embedded_tim_count: usize,
    pub patched_embedded_tim_count: usize,
    pub source_decode_issue_count: usize,
    pub patched_decode_issue_count: usize,
    pub source_decode_issues: Vec<DecodeIssue>,
    pub patched_decode_issues: Vec<DecodeIssue>,
    pub source_match_count: usize,
    pub patched_match_count: usize,
    pub source_presentation_candidate_match_count: usize,
    pub patched_presentation_candidate_match_count: usize,
    pub source_presentation_candidate_surface_count: usize,
    pub patched_presentation_candidate_surface_count: usize,
    pub source_presentation_candidate_atlas_family_count: usize,
    pub duplicated_source_presentation_candidate_atlas_family_count: usize,
    pub source_runtime_resident_surface_count: usize,
    pub patched_runtime_resident_surface_count: usize,
    pub source_runtime_resident_surfaces: Vec<FixedPresentationRuntimeResidentSurface>,
    pub patched_runtime_resident_surfaces: Vec<FixedPresentationRuntimeResidentSurface>,
    pub source_runtime_draw_source_match_count: usize,
    pub patched_runtime_draw_source_match_count: usize,
    pub source_runtime_draw_source_matches: Vec<FixedPresentationRuntimeDrawSourceMatch>,
    pub patched_runtime_draw_source_matches: Vec<FixedPresentationRuntimeDrawSourceMatch>,
    pub source_runtime_texture_source_match_count: usize,
    pub patched_runtime_texture_source_match_count: usize,
    pub source_runtime_texture_source_matches: Vec<FixedPresentationRuntimeTextureSourceMatch>,
    pub patched_runtime_texture_source_matches: Vec<FixedPresentationRuntimeTextureSourceMatch>,
    pub presentation_candidate_atlas_families: Vec<FixedPresentationGlyphAtlasFamilyAudit>,
    pub targets: Vec<FixedPresentationGlyphTargetAudit>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationRuntimeSpriteCellAudit {
    pub packet_address: String,
    pub screen_x: i32,
    pub screen_y: i32,
    pub sprite_width_pixels: usize,
    pub sprite_height_pixels: usize,
    pub cell_offset_x_pixels: usize,
    pub cell_offset_y_pixels: usize,
    pub texture_vram_word_x: usize,
    pub texture_vram_y: usize,
    pub pixel_sha256: String,
    pub nonzero_pixel_count: usize,
    pub clut: String,
    pub draw_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationRuntimeDrawSourceMatch {
    pub record_path: String,
    pub decoded_layer_id: String,
    pub decoded_layer_sha256: String,
    pub tim_offset: usize,
    pub tim_sha256: String,
    pub tim_bits_per_pixel: u8,
    pub tim_pixel_width: usize,
    pub tim_pixel_height: usize,
    pub tim_image_vram_word_x: u16,
    pub tim_image_vram_y: u16,
    pub source_class: String,
    pub source_pixel_x: usize,
    pub source_pixel_y: usize,
    pub packet_address: String,
    pub screen_x: i32,
    pub screen_y: i32,
    pub sprite_width_pixels: usize,
    pub sprite_height_pixels: usize,
    pub texture_vram_word_x: usize,
    pub texture_vram_y: usize,
    pub clut: String,
    pub draw_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationRuntimeTextureSourceMatch {
    pub runtime_texture_pixel_sha256: String,
    pub record_path: String,
    pub decoded_layer_id: String,
    pub decoded_layer_sha256: String,
    pub tim_offset: usize,
    pub tim_sha256: String,
    pub tim_bits_per_pixel: u8,
    pub tim_pixel_width: usize,
    pub tim_pixel_height: usize,
    pub tim_image_vram_word_x: u16,
    pub tim_image_vram_y: u16,
    pub source_class: String,
    pub source_pixel_x: usize,
    pub source_pixel_y: usize,
    pub packet_address: String,
    pub screen_x: i32,
    pub screen_y: i32,
    pub sprite_width_pixels: usize,
    pub sprite_height_pixels: usize,
    pub texture_vram_word_x: usize,
    pub texture_vram_y: usize,
    pub clut: String,
    pub draw_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationRuntimeResidentSurface {
    pub record_path: String,
    pub decoded_layer_id: String,
    pub decoded_layer_sha256: String,
    pub tim_offset: usize,
    pub tim_sha256: String,
    pub tim_bits_per_pixel: u8,
    pub tim_pixel_width: usize,
    pub tim_pixel_height: usize,
    pub tim_image_vram_word_x: u16,
    pub tim_image_vram_y: u16,
    pub source_class: String,
}

#[derive(Debug, Serialize)]
pub struct FixedPresentationGlyphAtlasFamilyAudit {
    pub family_id: String,
    pub source_tim_sha256: Option<String>,
    pub patched_tim_sha256s: Vec<String>,
    pub surface_count: usize,
    pub source_surface_count: usize,
    pub patched_surface_count: usize,
    pub unchanged_surface_count: usize,
    pub changed_surface_count: usize,
    pub source_only_surface_count: usize,
    pub patched_only_surface_count: usize,
    pub source_match_count: usize,
    pub patched_match_count: usize,
    pub source_target_count: usize,
    pub patched_target_count: usize,
    pub source_runtime_observed_target_count: usize,
    pub patched_runtime_observed_target_count: usize,
    pub source_texts: Vec<String>,
    pub patched_texts: Vec<String>,
    pub surfaces: Vec<FixedPresentationGlyphAtlasSurfaceAudit>,
}

#[derive(Debug, Serialize)]
pub struct FixedPresentationGlyphAtlasSurfaceAudit {
    pub record_path: String,
    pub decoded_layer_id: String,
    pub tim_offset: usize,
    pub tim_status: ComparisonStatus,
    pub source: Option<FixedPresentationGlyphAtlasSideAudit>,
    pub patched: Option<FixedPresentationGlyphAtlasSideAudit>,
}

#[derive(Debug, Serialize)]
pub struct FixedPresentationGlyphAtlasSideAudit {
    pub decoded_layer_sha256: String,
    pub tim_sha256: String,
    pub tim_bits_per_pixel: u8,
    pub tim_pixel_width: usize,
    pub tim_pixel_height: usize,
    pub tim_image_vram_word_x: u16,
    pub tim_image_vram_y: u16,
    pub match_count: usize,
    pub target_count: usize,
    pub runtime_observed_target_count: usize,
    pub target_pixel_sha256s: Vec<String>,
    pub runtime_observed_target_pixel_sha256s: Vec<String>,
    pub source_texts: Vec<String>,
    pub preview_file: Option<String>,
    pub preview_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FixedPresentationGlyphTargetAudit {
    pub pixel_sha256: String,
    pub source_codes: Vec<String>,
    pub source_texts: Vec<String>,
    pub source_semantic_ids: Vec<String>,
    pub runtime_uses: Vec<FixedPresentationGlyphRuntimeUse>,
    pub source_matches: Vec<FixedPresentationGlyphSourceMatch>,
    pub patched_matches: Vec<FixedPresentationGlyphSourceMatch>,
    pub source_presentation_candidate_match_count: usize,
    pub patched_presentation_candidate_match_count: usize,
    pub unchanged_presentation_candidate_locations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationGlyphRuntimeUse {
    pub screen_x: i32,
    pub screen_y: i32,
    pub sprite_width_pixels: usize,
    pub sprite_height_pixels: usize,
    pub texture_vram_word_x: usize,
    pub texture_vram_y: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedPresentationGlyphSourceMatch {
    pub record_path: String,
    pub decoded_layer_id: String,
    pub decoded_layer_sha256: String,
    pub tim_offset: usize,
    pub tim_sha256: String,
    pub tim_bits_per_pixel: u8,
    pub tim_pixel_width: usize,
    pub tim_pixel_height: usize,
    pub tim_image_vram_word_x: u16,
    pub tim_image_vram_y: u16,
    pub pixel_x: usize,
    pub pixel_y: usize,
    pub source_class: String,
    pub preview_file: Option<String>,
    pub preview_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Unchanged,
    Changed,
    SourceOnly,
    PatchedOnly,
}

#[derive(Debug, Serialize)]
pub struct DiscAssetComparisonReport {
    pub kind: String,
    pub visual_scan_scope: String,
    pub output_directory: String,
    pub source_cue: String,
    pub source_bin: String,
    pub source_bin_sha256: String,
    pub source_bin_byte_count: u64,
    pub patched_cue: String,
    pub patched_bin: String,
    pub patched_bin_sha256: String,
    pub patched_bin_byte_count: u64,
    pub source_iso_file_count: usize,
    pub patched_iso_file_count: usize,
    pub file_inventory_matches: bool,
    pub compared_record_count: usize,
    pub unchanged_record_count: usize,
    pub changed_record_count: usize,
    pub source_only_record_count: usize,
    pub patched_only_record_count: usize,
    pub changed_record_without_embedded_tim_count: usize,
    pub decoded_layer_comparison_count: usize,
    pub changed_decoded_layer_count: usize,
    pub embedded_tim_comparison_count: usize,
    pub changed_embedded_tim_count: usize,
    pub visually_changed_tim_count: usize,
    pub changed_clut_tim_count: usize,
    pub decode_issue_count: usize,
    pub report_file: String,
    pub html_file: String,
    pub records: Vec<DiscRecordComparison>,
}

#[derive(Debug, Serialize)]
pub struct DiscRecordComparison {
    pub path: String,
    pub status: ComparisonStatus,
    pub source: Option<DiscRecordIdentity>,
    pub patched: Option<DiscRecordIdentity>,
    pub extent_metadata_matches: bool,
    pub stored_difference: Option<ByteDifferenceSummary>,
    pub decoded_layers: Vec<DecodedLayerComparison>,
    pub decode_issues: Vec<DecodeIssue>,
}

#[derive(Debug, Serialize)]
pub struct DiscRecordIdentity {
    pub extent_lba: u32,
    pub extended_attribute_blocks: u8,
    pub byte_count: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ByteDifferenceSummary {
    pub changed_byte_count: usize,
    pub changed_range_count: usize,
    pub changed_byte_ranges_preview: Vec<[usize; 2]>,
    pub ranges_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonSide {
    Source,
    Patched,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecodeIssue {
    pub side: ComparisonSide,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DecodedLayerComparison {
    pub id: String,
    pub kind: String,
    pub status: ComparisonStatus,
    pub source: Option<DecodedLayerIdentity>,
    pub patched: Option<DecodedLayerIdentity>,
    pub decoded_difference: Option<ByteDifferenceSummary>,
    pub embedded_tim_scan_performed: bool,
    pub source_embedded_tim_count: usize,
    pub patched_embedded_tim_count: usize,
    pub embedded_tim_comparison_count: usize,
    pub unchanged_embedded_tim_count: usize,
    pub embedded_tims: Vec<EmbeddedTimComparison>,
}

#[derive(Debug, Serialize)]
pub struct DecodedLayerIdentity {
    pub stored_offset: usize,
    pub stored_byte_count: usize,
    pub decoded_byte_count: usize,
    pub decoded_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct EmbeddedTimComparison {
    pub offset: usize,
    pub status: ComparisonStatus,
    pub source: Option<EmbeddedTimIdentity>,
    pub patched: Option<EmbeddedTimIdentity>,
    pub tim_byte_difference: Option<ByteDifferenceSummary>,
    pub indexed_pixel_difference_count: Option<usize>,
    pub clut_word_difference_count: Option<usize>,
    pub preview_palette_index: Option<usize>,
    pub palette_zero_rgba_difference_count: Option<usize>,
    pub source_preview_file: Option<String>,
    pub source_preview_sha256: Option<String>,
    pub patched_preview_file: Option<String>,
    pub patched_preview_sha256: Option<String>,
    pub difference_preview_file: Option<String>,
    pub difference_preview_sha256: Option<String>,
    pub preview_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddedTimIdentity {
    pub bits_per_pixel: u8,
    pub total_size: usize,
    pub tim_sha256: String,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub palette_count: usize,
}
