use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::tim::{Cell, GlyphInstallMetadata};

#[derive(Debug, Clone)]
pub(crate) struct TitleNoticeFontStyle {
    pub(crate) font: PathBuf,
    pub(crate) font_px: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct TitleNoticeBuildConfig {
    pub(crate) assets: PathBuf,
    pub(crate) font: TitleNoticeFontStyle,
    pub(crate) build_spec_sha256: String,
    pub(crate) output_dir: PathBuf,
    pub(crate) force: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleNoticeManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) menu_path: String,
    pub(super) menu_stored_sha256: String,
    pub(super) menu_decoded_sha256: String,
    pub(super) overlay_path: String,
    pub(super) overlay_stored_sha256: String,
    pub(super) overlay_decoded_sha256: String,
    pub(super) entries: Vec<TitleNoticeAssetReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleNoticeAssetReference {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleNoticeTranslation {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_offset: String,
    pub(super) source_record_size: usize,
    pub(super) source_record_sha256: String,
    pub(super) source_codes: Vec<String>,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) consumer: TitleNoticeConsumer,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum TitleNoticeConsumer {
    Placement {
        record_offset: String,
        source_x: i16,
        source_y: i16,
        source_record_sha256: String,
    },
    ContinueEmptySlots,
    ContinueClearSlots,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DevelopmentStatus {
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReleaseStatus {
    NeedsHumanReview,
    Approved,
}

#[derive(Debug, Serialize)]
pub(crate) struct TitleNoticeBuildReport {
    pub(crate) kind: String,
    pub(crate) build_spec_sha256: String,
    pub(crate) source_bin_sha256: String,
    pub(crate) source_menu_decoded_sha256: String,
    pub(crate) output_menu_decoded_sha256: String,
    pub(crate) source_overlay_decoded_sha256: String,
    pub(crate) output_overlay_decoded_sha256: String,
    pub(crate) translation_manifest_sha256: String,
    pub(crate) font_name: String,
    pub(crate) font_sha256: String,
    pub(crate) font_px: f32,
    pub(crate) entry_count: usize,
    pub(crate) release_approved_entry_count: usize,
    pub(crate) development_input_available: bool,
    pub(crate) release_candidate_input_eligible: bool,
    pub(crate) source_records_match: bool,
    pub(crate) source_placements_match: bool,
    pub(crate) changed_bytes_confined_to_owned_ranges: bool,
    pub(crate) reused_glyph_count: usize,
    pub(crate) installed_glyph_count: usize,
    pub(crate) global_menu_write_count: usize,
    pub(crate) source_menu_graphics_preserved: bool,
    pub(crate) menu_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) overlay_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) glyphs: Vec<TitleNoticeGlyphBuild>,
    pub(crate) entries: Vec<TitleNoticeEntryBuild>,
    pub(crate) runtime_consumer_verified: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TitleNoticeGlyphBuild {
    pub(crate) character: char,
    pub(crate) code: String,
    pub(crate) cell: Cell,
    pub(crate) reused: bool,
    pub(crate) global_menu_resident: bool,
    pub(crate) runtime_context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ink_bounds: Option<[usize; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) install: Option<GlyphInstallMetadata>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TitleNoticeEntryBuild {
    pub(crate) id: String,
    pub(crate) source_offset: String,
    pub(crate) source_text: String,
    pub(crate) korean_text: String,
    pub(crate) output_codes: Vec<String>,
    pub(crate) consumer: TitleNoticeConsumer,
    pub(crate) development_status: DevelopmentStatus,
    pub(crate) release_status: ReleaseStatus,
}

pub(crate) struct TitleNoticeBuild {
    pub(crate) source_overlay_decoded: Vec<u8>,
    pub(crate) overlay_decoded: Vec<u8>,
    pub(crate) menu_decoded: Vec<u8>,
    pub(crate) menu_write_claims: Vec<DecodedDataClaim>,
    pub(crate) overlay_write_claims: Vec<DecodedDataClaim>,
    pub(crate) contextual_glyphs: Vec<crate::contextual_texture_upload::ContextualMenuGlyph>,
    pub(crate) build_manifest_sha256: String,
    pub(crate) report: TitleNoticeBuildReport,
}
