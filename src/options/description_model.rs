use serde::{Deserialize, Serialize};

use super::model::{OptionsDevelopmentStatus, OptionsReleaseStatus};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsDescriptionManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) optinfo_path: String,
    pub(super) optinfo_stored_sha256: String,
    pub(super) optinfo_decoded_sha256: String,
    pub(super) stream_arena_offset: String,
    pub(super) stream_arena_end: String,
    pub(super) item_table_offsets: Vec<String>,
    pub(super) root_table_offset: String,
    pub(super) units: Vec<OptionsDescriptionReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsDescriptionReference {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsDescriptionUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) item_index: usize,
    pub(super) source_slot_offset: String,
    pub(super) source_slot_sha256: String,
    pub(super) common: OptionsDescriptionVariant,
    pub(super) states: Vec<OptionsDescriptionVariant>,
    pub(super) development_status: OptionsDevelopmentStatus,
    pub(super) release_status: OptionsReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsDescriptionVariant {
    pub(super) source_stream_offset: String,
    pub(super) source_bytes: Vec<String>,
    pub(super) source_lines: Vec<String>,
    pub(super) korean_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DescriptionSpan {
    pub(super) start_cell: u8,
    pub(super) cell_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DescriptionStream {
    pub(super) lines: Vec<Vec<DescriptionSpan>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionsDescriptionAudit {
    pub item_count: usize,
    pub state_count: usize,
    pub authored_item_count: usize,
    pub release_approved_item_count: usize,
    pub source_stream_count: usize,
    pub optinfo_path: String,
    pub optinfo_stored_sha256: String,
    pub optinfo_decoded_sha256: String,
    pub manifest_sha256: String,
    pub source_bindings_match: bool,
    pub development_asset_input_available: bool,
    pub release_candidate_input_eligible: bool,
}

#[derive(Debug, Serialize)]
pub struct OptionsDescriptionBuild {
    pub id: String,
    pub item_index: usize,
    pub used_cell_count: usize,
    pub variants: Vec<OptionsDescriptionVariantBuild>,
}

#[derive(Debug, Serialize)]
pub struct OptionsDescriptionVariantBuild {
    pub role: String,
    pub korean_lines: Vec<String>,
    pub output_stream_offset: String,
    pub output_bytes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OptionsDescriptionGlyphBuild {
    pub item_index: usize,
    pub atlas_cell: usize,
    pub character: char,
    pub cell: crate::tim::Cell,
    pub ink_bounds: Option<[usize; 4]>,
    pub install: Option<crate::tim::GlyphInstallMetadata>,
}
