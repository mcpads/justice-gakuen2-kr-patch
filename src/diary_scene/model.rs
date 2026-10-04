use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::development_build_spec::SizedFontSource;
use crate::font::HorizontalTextAlignment;
use crate::tim::Cell;

pub(super) const DIARY_SCENE_SOURCE_CATALOGUE_KIND: &str =
    "justice_gakuen2_diary_scene_source_catalogue";
pub(super) const DIARY_SCENE_RUNTIME_CATALOGUE_KIND: &str =
    "justice_gakuen2_diary_scene_runtime_catalogue";

#[derive(Debug, Clone)]
pub struct DiarySceneFontSources {
    pub location_label: SizedFontSource,
    pub movement_map_label: SizedFontSource,
    pub exam_heading: SizedFontSource,
    pub exam_stamp: SizedFontSource,
    pub exam_evaluation: SizedFontSource,
    pub exam_finished: SizedFontSource,
    pub training_list_label: SizedFontSource,
    pub training_status_axis: SizedFontSource,
    pub cooperative_selection: SizedFontSource,
    pub calendar_title: Option<SizedFontSource>,
}

#[derive(Debug, Clone)]
pub struct DiarySceneBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: DiarySceneFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct DiarySceneBuild {
    pub archive: Vec<u8>,
    pub artwork_archives: Vec<super::artwork::SceneArtworkArchiveBuild>,
    pub runtime_bundles: Vec<DiarySceneRuntimeBundleBuild>,
    pub manifest_sha256: String,
    pub report: DiarySceneBuildReport,
}

pub struct DiarySceneRuntimeBundleBuild {
    pub path: String,
    pub data: Vec<u8>,
    pub report: DiarySceneRuntimeBundleReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiarySceneFontRole {
    LocationLabel,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneManifest {
    pub(super) kind: String,
    pub(super) source_catalogue: PathBuf,
    pub(super) runtime_catalogues: Vec<PathBuf>,
    pub(super) fixed_presentations: Vec<PathBuf>,
    pub(super) calendar_backgrounds: Option<PathBuf>,
    pub(super) month_overview: Option<PathBuf>,
    #[serde(default)]
    pub(super) scene_backgrounds: Option<PathBuf>,
    pub(super) portraits: Option<PathBuf>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneSourceCatalogue {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) path: String,
    pub(super) archive_sha256: String,
    pub(super) location_block: Cell,
    pub(super) members: Vec<DiarySceneCatalogueMemberBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneCatalogueMemberBinding {
    pub(super) index: usize,
    pub(super) offset: usize,
    pub(super) compressed_size: usize,
    pub(super) compressed_sha256: String,
    pub(super) decoded_size: usize,
    pub(super) decoded_sha256: String,
    pub(super) tim_vram: [u16; 2],
    pub(super) tim_pixel_size: [usize; 2],
    pub(super) clut_vram: [u16; 2],
    pub(super) clut_size: [usize; 2],
    pub(super) location_block_sha256: Option<String>,
    pub(super) location_palette_roles: Option<DiarySceneLocationPaletteRoles>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneLocationPaletteRoles {
    pub(super) clear_index: u8,
    pub(super) outline_index: u8,
    pub(super) fill_index: u8,
    pub(super) clear_bgr555: u16,
    pub(super) outline_bgr555: u16,
    pub(super) fill_bgr555: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneRuntimeCatalogue {
    pub(super) kind: String,
    pub(super) family: String,
    pub(super) bundles: Vec<DiarySceneRuntimeBundleBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneRuntimeBundleBinding {
    pub(super) path: String,
    pub(super) archive_sha256: String,
    pub(super) member_count: usize,
    pub(super) members: Vec<DiarySceneRuntimeMemberBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneRuntimeMemberBinding {
    pub(super) index: usize,
    pub(super) offset: usize,
    pub(super) compressed_size: usize,
    pub(super) compressed_sha256: String,
    pub(super) decoded_size: usize,
    pub(super) decoded_sha256: String,
    pub(super) catalogue_member_indices: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationFamily {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_decoded_sha256: String,
    pub(super) tim_offset: usize,
    pub(super) source_tim_sha256: String,
    #[serde(default)]
    pub(super) team_up_names_path: Option<PathBuf>,
    #[serde(skip)]
    pub(super) team_up_names_sha256: Option<String>,
    pub(super) transparent_index: u8,
    pub(super) consumers: Vec<DiarySceneFixedPresentationConsumer>,
    pub(super) translated_regions: Vec<DiarySceneFixedPresentationTextRegion>,
    pub(super) preserved_regions: Vec<DiarySceneFixedPresentationPreservedRegion>,
    #[serde(default)]
    pub(super) preserved_remainder: Option<DiarySceneFixedPresentationPreservedRemainder>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationFamilySet {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) tim_offset: usize,
    pub(super) transparent_index: u8,
    pub(super) shared_translated_regions: Vec<DiarySceneFixedPresentationTextRegion>,
    pub(super) preserved_remainder: DiarySceneFixedPresentationPreservedRemainder,
    pub(super) variants: Vec<DiarySceneFixedPresentationVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationVariant {
    pub(super) id: String,
    pub(super) source_decoded_sha256: String,
    pub(super) source_tim_sha256: String,
    pub(super) consumers: Vec<DiarySceneFixedPresentationConsumer>,
    pub(super) translated_regions: Vec<DiarySceneFixedPresentationTextRegion>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationConsumer {
    pub(super) path: String,
    pub(super) member_index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationTextRegion {
    pub(super) id: String,
    pub(super) source_text: String,
    #[serde(default)]
    pub(super) korean_text: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_sha256: String,
    pub(super) font_role: DiarySceneFixedPresentationFontRole,
    pub(super) rendering: DiarySceneFixedPresentationRendering,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationPreservedRegion {
    pub(super) id: String,
    pub(super) purpose: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFixedPresentationPreservedRemainder {
    pub(super) id: String,
    pub(super) purpose: String,
    pub(super) source_indexed_sha256: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiarySceneFixedPresentationFontRole {
    MovementMapLabel,
    Heading,
    Stamp,
    Evaluation,
    Completion,
    TrainingListLabel,
    TrainingStatusAxis,
    CooperativeSelection,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DiarySceneFixedPresentationRendering {
    SplitLine {
        clear_index: u8,
        outline_index: u8,
        fill_index: u8,
        second_width: usize,
    },
    Outlined {
        clear_index: u8,
        outline_index: u8,
        fill_index: u8,
        horizontal_alignment: HorizontalTextAlignment,
    },
    CoverageRamp {
        clear_index: u8,
        first_ink_index: u8,
        last_ink_index: u8,
        #[serde(default)]
        horizontal_alignment: Option<HorizontalTextAlignment>,
    },
    VerticalStamp {
        clear_index: u8,
        fill_index: u8,
        border_inset: usize,
        border_thickness: usize,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneUnit {
    pub(super) kind: String,
    pub(super) entries: Vec<DiarySceneEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneEntry {
    pub(super) id: String,
    pub(super) member_index: usize,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: DiarySceneFontRole,
    pub(super) cell: Cell,
}

#[derive(Debug, Serialize)]
pub struct DiarySceneBuildReport {
    pub kind: String,
    pub source_path: String,
    pub source_archive_size: usize,
    pub source_archive_sha256: String,
    pub patched_archive_sha256: String,
    pub build_spec_sha256: String,
    pub source_regions_match: bool,
    pub changed_bytes_confined_to_owned_cells: bool,
    pub catalogue_background_outside_text_cells_preserved: bool,
    pub archive_changes_confined_to_owned_members: bool,
    pub source_runtime_bundle_count: usize,
    pub source_runtime_member_count: usize,
    pub patched_runtime_bundle_count: usize,
    pub patched_runtime_member_count: usize,
    pub fixed_presentation_family_count: usize,
    pub fixed_presentation_translation_catalogues: std::collections::BTreeMap<String, String>,
    pub fixed_presentation_consumer_count: usize,
    pub fixed_presentation_source_regions_match: bool,
    pub fixed_presentation_preserved_regions_unchanged: bool,
    pub runtime_unclaimed_decoded_bytes_preserved: bool,
    pub runtime_compression_requirements_satisfied: bool,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub catalogue_members: Vec<DiarySceneCatalogueMemberReport>,
    pub runtime_bundles: Vec<DiarySceneRuntimeBundleReport>,
    pub fonts: Vec<DiarySceneFontBuild>,
    pub entries: Vec<DiarySceneEntryBuild>,
    pub calendar_panels: Vec<super::calendar::CalendarPanelReport>,
    pub artwork_archives: Vec<super::artwork::SceneArtworkArchiveReport>,
}

#[derive(Debug, Serialize)]
pub struct DiarySceneCatalogueMemberReport {
    pub index: usize,
    pub source_compressed_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_compressed_sha256: String,
    pub patched_decoded_sha256: String,
    pub source_compressed_size: usize,
    pub rebuilt_compressed_size: usize,
    pub tim_vram: [u16; 2],
    pub tim_pixel_size: [usize; 2],
    pub clut_vram: [u16; 2],
    pub clut_size: [usize; 2],
    pub clear_palette_index: Option<u8>,
    pub outline_palette_index: Option<u8>,
    pub fill_palette_index: Option<u8>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiarySceneRuntimeBundleReport {
    pub path: String,
    pub source_size: usize,
    pub source_sha256: String,
    pub patched_sha256: String,
    pub changed: bool,
    pub members: Vec<DiarySceneRuntimeMemberReport>,
    pub source_members_start_with_catalogue_member: bool,
    pub changes_confined_to_owned_members: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiarySceneRuntimeMemberReport {
    pub index: usize,
    pub catalogue_member_indices: Vec<usize>,
    pub embedded_catalogue_offsets: Vec<usize>,
    pub offset: usize,
    pub slot_byte_count: usize,
    pub source_compressed_size: usize,
    pub rebuilt_compressed_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub source_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub source_compression_final_input_page: usize,
    pub rebuilt_compression_final_input_page: usize,
    pub source_compression_prefix_preservation_required: bool,
    pub compression_requirement_satisfied: bool,
    pub source_compressed_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_compressed_sha256: String,
    pub patched_decoded_sha256: String,
    pub catalogue_prefix_byte_count: usize,
    pub preserved_source_compressed_prefix_byte_count: usize,
    pub preserved_source_decoded_prefix_byte_count: usize,
    pub changed: bool,
    pub trailing_decoded_bytes_preserved: bool,
    pub unclaimed_decoded_bytes_preserved: bool,
    pub fixed_presentation_family_ids: Vec<String>,
    pub translated_fixed_presentation_region_ids: Vec<String>,
    pub preserved_fixed_presentation_region_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DiarySceneFontBuild {
    pub role: DiarySceneFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
}

#[derive(Debug, Serialize)]
pub struct DiarySceneEntryBuild {
    pub id: String,
    pub member_index: usize,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: DiarySceneFontRole,
    pub cell: Cell,
    pub cleared_cell: Cell,
    pub source_region_sha256: String,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub changed_decoded_byte_count: usize,
    pub preserved_location_background_pixel_count: usize,
}
