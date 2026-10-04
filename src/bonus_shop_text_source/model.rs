use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::bonus_inventory::BonusInventoryTextStyleSource;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct BonusShopTextSourceConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub output: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShopTextRole {
    ProductDescription,
    ProductLabel,
    ClerkDialogue,
}

impl ShopTextRole {
    pub(super) const ALL: [Self; 3] = [
        Self::ProductDescription,
        Self::ProductLabel,
        Self::ClerkDialogue,
    ];

    pub(super) const fn slug(self) -> &'static str {
        match self {
            Self::ProductDescription => "product-descriptions",
            Self::ProductLabel => "product-labels",
            Self::ClerkDialogue => "clerk-dialogue",
        }
    }

    pub(super) const fn unit_stem(self) -> &'static str {
        match self {
            Self::ProductDescription => "product-description",
            Self::ProductLabel => "product-label",
            Self::ClerkDialogue => "clerk-dialogue",
        }
    }

    pub(super) const fn expected_record_count(self) -> usize {
        match self {
            Self::ProductDescription | Self::ProductLabel => 83,
            Self::ClerkDialogue => 20,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextSourceManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub source_shop_ui_path: String,
    pub source_shop_ui_decoded_sha256: String,
    pub source_glyph_tim_offset: String,
    pub source_glyph_tim_sha256: String,
    pub overlay_runtime_base: String,
    pub table_count: usize,
    pub total_record_count: usize,
    pub unique_record_target_count: usize,
    pub total_pointer_interval_byte_count: usize,
    pub total_trailing_interval_byte_count: usize,
    pub all_trailing_interval_bytes_zero: bool,
    pub total_glyph_count: usize,
    pub resolved_glyph_count: usize,
    pub unresolved_glyph_count: usize,
    pub fully_decoded_record_count: usize,
    pub unresolved_record_count: usize,
    pub acquisition_complete: bool,
    pub tables: Vec<BonusShopTextSourceTableReport>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextSourceTableReport {
    pub role: ShopTextRole,
    pub pointer_table_range: [String; 2],
    pub record_region: [String; 2],
    pub record_count: usize,
    pub unique_record_target_count: usize,
    pub pointer_interval_byte_count: usize,
    pub trailing_interval_byte_count: usize,
    pub all_trailing_interval_bytes_zero: bool,
    pub index_path: String,
    pub index_sha256: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BonusShopTextSourceTableIndex {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_overlay_sha256: String,
    pub source_glyph_tim_sha256: String,
    pub role: ShopTextRole,
    pub pointer_table_range: [String; 2],
    pub record_region: [String; 2],
    pub record_count: usize,
    pub fully_decoded_record_count: usize,
    pub pointer_interval_byte_count: usize,
    pub trailing_interval_byte_count: usize,
    pub all_trailing_interval_bytes_zero: bool,
    pub total_glyph_count: usize,
    pub resolved_glyph_count: usize,
    pub unresolved_glyph_count: usize,
    pub units: Vec<BonusShopTextSourceUnitRef>,
    pub contact_sheets: Vec<BonusShopTextContactSheetRef>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextSourceUnit {
    pub kind: String,
    pub unit_id: String,
    pub role: ShopTextRole,
    pub record_index: usize,
    pub pointer_storage_offset: String,
    pub source_offset: String,
    pub source_runtime_address: String,
    pub source_record_end_offset: String,
    pub source_record_byte_count: usize,
    pub source_record_sha256: String,
    pub source_record_hex: String,
    #[serde(default)]
    pub source_pointer_interval_end_offset: String,
    #[serde(default)]
    pub source_pointer_interval_byte_count: usize,
    #[serde(default)]
    pub trailing_interval_byte_count: usize,
    #[serde(default)]
    pub trailing_interval_sha256: String,
    #[serde(default)]
    pub trailing_interval_all_zero: bool,
    pub line_count: usize,
    pub glyph_count: usize,
    pub resolved_glyph_count: usize,
    pub unresolved_glyph_count: usize,
    pub exact_source_text: Option<String>,
    pub tokens: Vec<BonusShopTextSourceToken>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextSourceToken {
    pub source_offset: String,
    pub raw_hex: String,
    pub kind: BonusShopTextSourceTokenKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_pixel_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_source_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BonusShopTextSourceTokenKind {
    Glyph,
    LineBreak,
    Blank,
    Terminator,
}

#[derive(Debug, Serialize)]
pub(super) struct BonusShopTextSourceUnitRef {
    pub unit_id: String,
    pub path: String,
    pub source_offset: String,
    pub content_sha256: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BonusShopTextContactSheetRef {
    pub sheet_id: String,
    pub png_path: String,
    pub png_sha256: String,
    pub index_path: String,
    pub index_sha256: String,
    pub record_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct BonusShopTextContactSheetIndex {
    pub kind: String,
    pub sheet_id: String,
    pub role: ShopTextRole,
    pub scale: usize,
    pub width: usize,
    pub height: usize,
    pub records: Vec<BonusShopTextContactSheetPlacement>,
}

#[derive(Debug, Serialize)]
pub(super) struct BonusShopTextContactSheetPlacement {
    pub unit_id: String,
    pub record_index: usize,
    pub source_offset: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone)]
pub struct BonusShopTextAssetSyncConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub assets: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextAssetSyncReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_overlay_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub source_record_count: usize,
    pub existing_unit_count: usize,
    pub existing_delegated_record_count: usize,
    pub created_untranslated_unit_count: usize,
    pub created_delegated_record_count: usize,
    pub migrated_delegated_record_count: usize,
    pub refreshed_source_unit_count: usize,
    pub final_unit_count: usize,
    pub final_delegated_record_count: usize,
    pub complete_source_population: bool,
}

#[derive(Debug, Clone)]
pub struct BonusShopTextAssetAuditConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub assets: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextAssetAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_overlay_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub manifest_sha256: String,
    pub source_record_count: usize,
    pub tracked_unit_count: usize,
    pub delegated_record_count: usize,
    pub untranslated_unit_count: usize,
    pub authored_unit_count: usize,
    pub release_approved_unit_count: usize,
    pub role_counts: Vec<BonusShopTextRoleAudit>,
    pub source_records_match: bool,
    pub complete_source_population: bool,
    pub development_can_continue: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub required_korean_characters: String,
    pub required_korean_character_count: usize,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextRoleAudit {
    pub role: ShopTextRole,
    pub source_record_count: usize,
    pub delegated_record_count: usize,
    pub untranslated_unit_count: usize,
    pub authored_unit_count: usize,
    pub release_approved_unit_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextAssetManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) source_overlay_path: String,
    pub(super) source_overlay_sha256: String,
    pub(super) source_shop_ui_path: String,
    pub(super) source_shop_ui_decoded_sha256: String,
    pub(super) source_glyph_tim_sha256: String,
    pub(super) dialogue_codebook_sha256: String,
    pub(super) source_record_count: usize,
    pub(super) units: Vec<BonusShopTextAssetReference>,
    #[serde(default)]
    pub(super) delegated_records: Vec<BonusShopTextAssetReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextAssetReference {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextDelegatedRecord {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source: BonusShopTextSourceUnit,
    pub(super) writer: BonusShopTextDelegatedWriter,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextDelegatedWriter {
    pub(super) id: String,
    pub(super) unit_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTranslationUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source: BonusShopTextSourceUnit,
    pub(super) korean_text: Option<String>,
    pub(super) font_role: Option<BonusShopFontRole>,
    pub(super) development_status: BonusShopDevelopmentStatus,
    pub(super) release_status: BonusShopReleaseStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BonusShopFontRole {
    ProductDescription,
    ProductLabel,
    ClerkDialogue,
}

impl BonusShopFontRole {
    pub(super) const fn for_source_role(role: ShopTextRole) -> Self {
        match role {
            ShopTextRole::ProductDescription => Self::ProductDescription,
            ShopTextRole::ProductLabel => Self::ProductLabel,
            ShopTextRole::ClerkDialogue => Self::ClerkDialogue,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BonusShopDevelopmentStatus {
    Untranslated,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BonusShopReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
}

pub type BonusShopTextFontSource = BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusShopTextFontSources {
    pub product_description: BonusShopTextFontSource,
    pub product_label: BonusShopTextFontSource,
    pub clerk_dialogue: BonusShopTextFontSource,
}

impl BonusShopTextFontSources {
    pub(super) fn for_role(&self, role: BonusShopFontRole) -> &BonusShopTextFontSource {
        match role {
            BonusShopFontRole::ProductDescription => &self.product_description,
            BonusShopFontRole::ProductLabel => &self.product_label,
            BonusShopFontRole::ClerkDialogue => &self.clerk_dialogue,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BonusShopTextBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: BonusShopTextFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusShopTextBuild {
    pub(super) glyphs: Vec<super::glyph_atlas::BonusShopGlyphPixels>,
    pub(super) glyph_codes:
        std::collections::BTreeMap<BonusShopFontRole, std::collections::BTreeMap<char, u16>>,
    pub(super) units: std::collections::BTreeMap<String, BonusShopTranslationUnit>,
    pub(super) card_results: super::card_results::BonusShopCardResultText,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusShopTextBuildReport,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub translation_unit_sha256s: Vec<String>,
    pub delegated_record_sha256s: Vec<String>,
    pub card_result_text_sha256: String,
    pub card_result_record_count: usize,
    pub source_shop_ui_path: String,
    pub source_shop_ui_decoded_sha256: String,
    pub output_shop_ui_decoded_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub output_overlay_sha256: String,
    pub source_record_count: usize,
    pub tracked_unit_count: usize,
    pub delegated_record_count: usize,
    pub untranslated_unit_count: usize,
    pub authored_unit_count: usize,
    pub release_approved_unit_count: usize,
    pub delegated_records_match_existing_writer: bool,
    pub protected_source_code_count: usize,
    pub direct_selector_code_count: usize,
    pub runtime_glyph_code_count: usize,
    pub available_glyph_cell_count: usize,
    pub required_glyph_count: usize,
    pub fonts: Vec<BonusShopTextFontBuildReport>,
    pub glyphs: Vec<BonusShopTextGlyphBuildReport>,
    pub units: Vec<BonusShopTextUnitBuildReport>,
    pub shop_ui_expected_write_ranges: Vec<[usize; 2]>,
    pub shop_ui_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_records_match: bool,
    pub complete_source_population: bool,
    pub shop_ui_changes_confined_to_allocated_glyph_cells: bool,
    pub overlay_changes_confined_to_authored_records: bool,
    pub development_can_continue: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub runtime_verification_required: bool,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextFontBuildReport {
    pub role: BonusShopFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextGlyphBuildReport {
    pub role: BonusShopFontRole,
    pub text: String,
    pub code: String,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub changed_decoded_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct BonusShopTextUnitBuildReport {
    pub id: String,
    pub role: ShopTextRole,
    pub source_offset: String,
    pub source_record_byte_count: usize,
    pub output_record_byte_count: usize,
    pub changed: bool,
    pub development_status: BonusShopDevelopmentStatus,
    pub release_status: BonusShopReleaseStatus,
}
