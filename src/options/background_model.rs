use serde::{Deserialize, Serialize};

use crate::tim::Cell;

use super::model::{OptionsDevelopmentStatus, OptionsFontBuild, OptionsReleaseStatus};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsBackgroundManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) menu_path: String,
    pub(super) menu_stored_sha256: String,
    pub(super) menu_decoded_sha256: String,
    pub(super) tim_offset: usize,
    pub(super) pixel_width: usize,
    pub(super) pixel_height: usize,
    pub(super) runtime_consumers: Vec<String>,
    pub(super) units: Vec<OptionsBackgroundAssetReference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsBackgroundAssetReference {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsBackgroundTranslationUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: OptionsBackgroundFontRole,
    pub(super) layout: OptionsBackgroundLayout,
    pub(super) clear_index: u8,
    pub(super) outline_index: u8,
    pub(super) fill_index: u8,
    pub(super) occurrences: Vec<OptionsBackgroundOccurrence>,
    pub(super) development_status: OptionsDevelopmentStatus,
    pub(super) release_status: OptionsReleaseStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum OptionsBackgroundFontRole {
    SchoolName,
    CrestMark,
}

impl OptionsBackgroundFontRole {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::SchoolName => "options_background_school_name",
            Self::CrestMark => "options_background_crest_mark",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum OptionsBackgroundLayout {
    VerticalGlyphs,
    Centered,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsBackgroundOccurrence {
    pub(super) id: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

pub(super) struct LoadedOptionsBackgroundAssets {
    pub(super) manifest_sha256: String,
    pub(super) tim_offset: usize,
    pub(super) units: Vec<OptionsBackgroundTranslationUnit>,
}

#[derive(Debug, Serialize)]
pub struct OptionsBackgroundBuildReport {
    pub manifest_sha256: String,
    pub tim_offset: String,
    pub unit_count: usize,
    pub release_approved_unit_count: usize,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    pub changes_confined_to_owned_regions: bool,
    pub fonts: Vec<OptionsFontBuild>,
    pub regions: Vec<OptionsBackgroundRegionBuild>,
}

#[derive(Debug, Serialize)]
pub struct OptionsBackgroundRegionBuild {
    pub unit_id: String,
    pub occurrence_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: String,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    pub ink_bounds: Vec<[usize; 4]>,
    pub allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub changed_decoded_byte_count: usize,
}
