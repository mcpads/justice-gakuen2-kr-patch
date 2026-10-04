use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub(crate) use crate::embedded_tim::EmbeddedTimAudit as CharacterSelectTimAudit;

#[derive(Debug, Clone)]
pub struct CharacterSelectGraphicsAuditConfig {
    pub cue: PathBuf,
    pub runtime_evidence_spec: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectRuntimeEvidenceSpec {
    pub(super) ram_dump: PathBuf,
    pub(super) frame: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectGraphicsAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub record_count: usize,
    pub tim_count: usize,
    pub auxiliary_record_count: usize,
    pub auxiliary_tim_count: usize,
    pub shared_atlas_offset: usize,
    pub shared_atlas_sha256: String,
    pub shared_atlas_identical_across_records: bool,
    pub runtime_evidence: Option<CharacterSelectRuntimeEvidenceAudit>,
    pub records: Vec<CharacterSelectRecordAudit>,
    pub auxiliary_records: Vec<CharacterSelectAuxiliaryRecordAudit>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAuxiliaryRecordAudit {
    pub role: String,
    pub source_path: String,
    pub source_extent_lba: u32,
    pub source_stored_size: usize,
    pub source_stored_sha256: String,
    pub source_decoded_size: usize,
    pub source_decoded_sha256: String,
    pub tim_count: usize,
    pub tims: Vec<CharacterSelectTimAudit>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectRuntimeEvidenceAudit {
    pub spec_path: String,
    pub spec_sha256: String,
    pub ram_dump_path: String,
    pub ram_dump_sha256: String,
    pub frame_path: String,
    pub frame_sha256: String,
    pub overlay_ram_offset: usize,
    pub exact_source_overlay_match_count: usize,
    pub matching_overlay_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectRecordAudit {
    pub source_path: String,
    pub source_extent_lba: u32,
    pub source_stored_size: usize,
    pub source_stored_sha256: String,
    pub source_decoded_size: usize,
    pub source_decoded_sha256: String,
    pub overlay_path: String,
    pub overlay_extent_lba: u32,
    pub overlay_size: usize,
    pub overlay_sha256: String,
    pub runtime_overlay: Option<CharacterSelectRuntimeOverlayAudit>,
    pub tim_count: usize,
    pub tims: Vec<CharacterSelectTimAudit>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectRuntimeOverlayAudit {
    pub resident_sha256: String,
    pub matching_byte_count: usize,
    pub matching_prefix_length: usize,
    pub source_matches_resident: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectFontRole {
    SelectHeading,
    ModeMenuHeading,
    ModeMenuLabel,
    CooperativeEmblemCharacter,
    TournamentBracketLabel,
    TournamentCertificateTitle,
    TournamentCertificateLabel,
    TournamentCertificateBody,
    Label,
    RosterName,
    FixedPrompt,
    CompactPrompt,
    SoloStatePrompt,
    CommonPauseMenu,
    SoloStoryIntro,
    SoloEpisodeCard,
    PracticalSelectionLabel,
    StageLabel,
    LeagueStandingLabel,
    SelectionHelp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectTextureSurface {
    SharedSelectAtlas,
    SharedFixedStripAtlas,
    BattleReadyAtlas,
    LeagueTournamentPromptAtlas,
    TournamentPromptAtlas,
    CooperativeModeMenuAtlas,
    CooperativeBackgroundEmblem,
    TournamentBracketLabelAtlas,
    TournamentCertificate,
    VersusLabelAtlas,
    VersusHandicapAtlas,
    ParticipantLabelAtlas,
    PracticalSelectionAtlas,
    StageLabelAtlas,
    LeagueStandingsAtlas,
    SelectionHelpAtlas,
    SoloStatePromptAtlas,
    CommonPauseMenuAtlas,
    SoloStoryIntroAtlas,
    SoloEpisodeCardAtlas,
}

impl CharacterSelectFontRole {
    pub(super) const fn cell_size(self) -> [usize; 2] {
        match self {
            Self::SelectHeading => [32, 32],
            Self::ModeMenuHeading => [40, 40],
            Self::ModeMenuLabel => [240, 32],
            Self::CooperativeEmblemCharacter => [34, 96],
            Self::TournamentBracketLabel => [32, 32],
            Self::TournamentCertificateTitle => [40, 160],
            Self::TournamentCertificateLabel => [40, 80],
            Self::TournamentCertificateBody => [30, 275],
            Self::Label => [20, 20],
            Self::RosterName => [48, 16],
            Self::FixedPrompt => [240, 20],
            Self::CompactPrompt => [240, 16],
            Self::SoloStatePrompt => [112, 12],
            Self::CommonPauseMenu => [120, 12],
            Self::SoloStoryIntro => [256, 24],
            Self::SoloEpisodeCard => [320, 40],
            Self::PracticalSelectionLabel => [140, 20],
            Self::StageLabel => [20, 20],
            Self::LeagueStandingLabel => [16, 16],
            Self::SelectionHelp => [12, 16],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CharacterSelectFontStyle {
    pub path: PathBuf,
    pub font_px: f32,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Clone)]
pub struct CharacterSelectFontSources {
    pub system_settings: CharacterSelectFontStyle,
    pub cooperative_diagnosis: CharacterSelectFontStyle,
    pub select_heading: CharacterSelectFontStyle,
    pub mode_menu_heading: CharacterSelectFontStyle,
    pub mode_menu_label: CharacterSelectFontStyle,
    pub cooperative_emblem_character: CharacterSelectFontStyle,
    pub tournament_bracket_label: CharacterSelectFontStyle,
    pub tournament_certificate_title: CharacterSelectFontStyle,
    pub tournament_certificate_label: CharacterSelectFontStyle,
    pub tournament_certificate_body: CharacterSelectFontStyle,
    pub label: CharacterSelectFontStyle,
    pub roster_name: CharacterSelectFontStyle,
    pub fixed_prompt: CharacterSelectFontStyle,
    pub compact_prompt: CharacterSelectFontStyle,
    pub solo_state_prompt: CharacterSelectFontStyle,
    pub common_pause_menu: CharacterSelectFontStyle,
    pub solo_story_intro: CharacterSelectFontStyle,
    pub solo_episode_card: CharacterSelectFontStyle,
    pub practical_selection_label: CharacterSelectFontStyle,
    pub stage_label: CharacterSelectFontStyle,
    pub league_standing_label: CharacterSelectFontStyle,
    pub selection_help: CharacterSelectFontStyle,
}

impl CharacterSelectFontSources {
    pub(super) fn style(&self, role: CharacterSelectFontRole) -> &CharacterSelectFontStyle {
        match role {
            CharacterSelectFontRole::SelectHeading => &self.select_heading,
            CharacterSelectFontRole::ModeMenuHeading => &self.mode_menu_heading,
            CharacterSelectFontRole::ModeMenuLabel => &self.mode_menu_label,
            CharacterSelectFontRole::CooperativeEmblemCharacter => {
                &self.cooperative_emblem_character
            }
            CharacterSelectFontRole::TournamentBracketLabel => &self.tournament_bracket_label,
            CharacterSelectFontRole::TournamentCertificateTitle => {
                &self.tournament_certificate_title
            }
            CharacterSelectFontRole::TournamentCertificateLabel => {
                &self.tournament_certificate_label
            }
            CharacterSelectFontRole::TournamentCertificateBody => &self.tournament_certificate_body,
            CharacterSelectFontRole::Label => &self.label,
            CharacterSelectFontRole::RosterName => &self.roster_name,
            CharacterSelectFontRole::FixedPrompt => &self.fixed_prompt,
            CharacterSelectFontRole::CompactPrompt => &self.compact_prompt,
            CharacterSelectFontRole::SoloStatePrompt => &self.solo_state_prompt,
            CharacterSelectFontRole::CommonPauseMenu => &self.common_pause_menu,
            CharacterSelectFontRole::SoloStoryIntro => &self.solo_story_intro,
            CharacterSelectFontRole::SoloEpisodeCard => &self.solo_episode_card,
            CharacterSelectFontRole::PracticalSelectionLabel => &self.practical_selection_label,
            CharacterSelectFontRole::StageLabel => &self.stage_label,
            CharacterSelectFontRole::LeagueStandingLabel => &self.league_standing_label,
            CharacterSelectFontRole::SelectionHelp => &self.selection_help,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CharacterSelectAtlasPlanConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: CharacterSelectFontSources,
    pub build_spec_sha256: String,
    pub output: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectTranslationManifest {
    pub(super) kind: String,
    pub(super) source_inventory: PathBuf,
    #[serde(default)]
    pub(super) required_fixed_strip_sets: Vec<CharacterSelectFixedStripSet>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectSourceInventoryManifest {
    pub(super) kind: String,
    pub(super) complete: bool,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectSourceInventoryUnit {
    pub(super) kind: String,
    pub(super) entries: Vec<CharacterSelectSourceInventoryEntry>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum CharacterSelectSourceTreatment {
    Translate,
    Preserve,
    Exclude,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectSourceInventoryEntry {
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) treatment: CharacterSelectSourceTreatment,
    #[serde(default)]
    pub(super) translation_id: Option<String>,
    #[serde(default)]
    pub(super) reason: Option<String>,
}

impl CharacterSelectSourceInventoryEntry {
    pub(super) fn translation_id(&self) -> &str {
        self.translation_id.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CharacterSelectFixedStripSet {
    RosterNames,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectTranslationUnit {
    pub(super) kind: String,
    pub(super) entries: Vec<CharacterSelectTranslationEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectTranslationEntry {
    pub(super) id: String,
    pub(super) korean_text: String,
}

#[derive(Debug, Clone)]
pub(super) struct CharacterSelectLocalizedSource {
    pub(super) source_ui_id: String,
    pub(super) translation_id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectSourceCellOccupancy {
    Blank,
    Inked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectGlyphAllocation {
    pub font_role: CharacterSelectFontRole,
    pub character: char,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub texture_page_index: u8,
    pub texture_uv: [u8; 2],
    pub cell: Cell,
    pub source_cell_occupancy: CharacterSelectSourceCellOccupancy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectSourceGlyphCell {
    pub physical_cell_id: String,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub texture_page_index: u8,
    pub texture_uv: [u8; 2],
    pub cell: Cell,
    pub source_indexed_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectFixedStripAllocation {
    pub physical_text_region_id: String,
    pub source_ui_ids: Vec<String>,
    pub translation_id: String,
    pub text_selection: CharacterSelectTextSelection,
    pub text_flow: CharacterSelectTextFlow,
    pub write_mode: CharacterSelectFixedStripWriteMode,
    pub font_role: CharacterSelectFontRole,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub cell: Cell,
    pub text_cell: Cell,
    pub clear_index: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterSelectSegmentedFixedStripAllocation {
    pub logical_text_region_id: String,
    pub source_ui_ids: Vec<String>,
    pub translation_id: String,
    pub text_selection: CharacterSelectTextSelection,
    pub text_flow: CharacterSelectTextFlow,
    pub write_mode: CharacterSelectFixedStripWriteMode,
    pub font_role: CharacterSelectFontRole,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub logical_width: usize,
    pub logical_height: usize,
    pub tracking_px: f32,
    pub horizontal_scale: usize,
    pub segments: Vec<CharacterSelectFixedStripSegment>,
    pub clear_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CharacterSelectFixedStripSegment {
    pub physical_text_region_id: &'static str,
    pub cell: Cell,
    pub logical_origin: [usize; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectTextSelection {
    Entire,
    Character { index: usize },
    Line { index: usize },
    Suffix { skip_characters: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectTextFlow {
    Horizontal,
    HorizontalLeft,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectFixedStripWriteMode {
    ReplaceRegion,
    OverlayNonClearPixels,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectSourceInkCleanupAllocation {
    pub physical_region_id: String,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub cell: Cell,
    pub first_ink_index: u8,
    pub last_ink_index: u8,
    pub replacement_index: u8,
    pub expected_source_ink_count: usize,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAtlasPlan {
    pub kind: String,
    pub source_shared_atlas_sha256: String,
    pub translation_assets_sha256: String,
    pub source_inventory_complete: bool,
    pub source_inventory_unit_count: usize,
    pub source_inventory_entry_count: usize,
    pub translated_source_ui_count: usize,
    pub untranslated_source_ui_count: usize,
    pub untranslated_source_ui_ids: Vec<String>,
    pub preserved_source_ui_ids: Vec<String>,
    pub excluded_source_ui_ids: Vec<String>,
    pub translation_unit_count: usize,
    pub translation_entry_count: usize,
    pub dynamic_atlas_entry_count: usize,
    pub fixed_texture_strip_entry_count: usize,
    pub source_ink_cleanup_count: usize,
    pub bound_fixed_texture_strip_count: usize,
    pub runtime_composed_texture_entry_count: usize,
    pub unresolved_route_occurrence_count: usize,
    pub fully_routed_source_ui_ids: Vec<String>,
    pub unresolved_route_source_ui_ids: Vec<String>,
    pub unresolved_route_translation_ids: Vec<String>,
    pub route_census: CharacterSelectRouteCensus,
    pub consumer: CharacterSelectConsumerPlan,
    pub required_glyph_count: usize,
    pub available_select_heading_cell_count_on_one_texture_page: usize,
    pub source_ink_cells_protected: bool,
    pub source_glyph_cell_count: usize,
    pub physical_cells_are_unique_and_non_overlapping: bool,
    pub source_glyph_cells: Vec<CharacterSelectSourceGlyphCell>,
    pub allocations: Vec<CharacterSelectGlyphAllocation>,
    pub fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub segmented_fixed_strips: Vec<CharacterSelectSegmentedFixedStripAllocation>,
    pub source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupAllocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectRouteReadiness {
    ProducerAndConsumerKnown,
    ProducerKnownConsumerPending,
    ProducerCandidateConsumerPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectRouteOwner {
    CharacterSelectGraphics,
    ModeDescendantGraphics,
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectProducerBindingStatus {
    Bound,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectConsumerLocationStatus {
    ExactByteOffsets,
    RecordOnly,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSelectConsumerReferenceKind {
    DescriptorAndPointer,
    ResourceLoad,
    PrimitiveLayout,
    PrimitiveSetup,
    RecordIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectRouteProducerTarget {
    pub source_record: String,
    pub tim_offset: Option<usize>,
    pub surface: Option<CharacterSelectTextureSurface>,
    pub physical_region_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectRouteConsumerTarget {
    pub source_record: String,
    pub consumer_record: String,
    pub reference_kind: CharacterSelectConsumerReferenceKind,
    pub byte_offsets: Vec<usize>,
    pub runtime_states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectRouteOccurrence {
    pub occurrence_id: String,
    pub producer_family: String,
    pub producer_binding_status: CharacterSelectProducerBindingStatus,
    pub consumer_location_status: CharacterSelectConsumerLocationStatus,
    pub readiness: CharacterSelectRouteReadiness,
    pub owner: CharacterSelectRouteOwner,
    pub source_ui_ids: Vec<String>,
    pub producer_targets: Vec<CharacterSelectRouteProducerTarget>,
    pub consumer_targets: Vec<CharacterSelectRouteConsumerTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterSelectRouteCensus {
    pub translated_source_ui_count: usize,
    pub route_occurrence_count: usize,
    pub producer_bound_occurrence_count: usize,
    pub producer_pending_occurrence_count: usize,
    pub exact_consumer_location_occurrence_count: usize,
    pub record_only_consumer_location_occurrence_count: usize,
    pub unresolved_consumer_location_occurrence_count: usize,
    pub fully_routed_occurrence_count: usize,
    pub unresolved_route_occurrence_count: usize,
    pub fully_routed_source_ui_count: usize,
    pub partially_routed_source_ui_ids: Vec<String>,
    pub unresolved_only_source_ui_ids: Vec<String>,
    pub source_ui_ids_with_multiple_occurrences: Vec<String>,
    pub producer_and_consumer_known_pending_occurrence_count: usize,
    pub producer_known_consumer_pending_occurrence_count: usize,
    pub producer_candidate_consumer_pending_occurrence_count: usize,
    pub unclassified_pending_occurrence_ids: Vec<String>,
    pub occurrences: Vec<CharacterSelectRouteOccurrence>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CharacterSelectConsumerPlan {
    pub shared_atlas_vram_word_x: u16,
    pub select_heading_texture_page_index: u8,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAtlasPlanReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub source_record_count: usize,
    pub shared_atlas_identical_across_records: bool,
    pub select_heading_font: PathBuf,
    pub select_heading_font_px: f32,
    pub select_heading_vertical_shift_px: i32,
    pub mode_menu_heading_font: PathBuf,
    pub mode_menu_heading_font_px: f32,
    pub mode_menu_heading_vertical_shift_px: i32,
    pub mode_menu_label_font: PathBuf,
    pub mode_menu_label_font_px: f32,
    pub mode_menu_label_vertical_shift_px: i32,
    pub cooperative_emblem_character_font: PathBuf,
    pub cooperative_emblem_character_font_px: f32,
    pub cooperative_emblem_character_vertical_shift_px: i32,
    pub tournament_bracket_label_font: PathBuf,
    pub tournament_bracket_label_font_px: f32,
    pub tournament_bracket_label_vertical_shift_px: i32,
    pub tournament_certificate_title_font: PathBuf,
    pub tournament_certificate_title_font_px: f32,
    pub tournament_certificate_title_vertical_shift_px: i32,
    pub tournament_certificate_label_font: PathBuf,
    pub tournament_certificate_label_font_px: f32,
    pub tournament_certificate_label_vertical_shift_px: i32,
    pub tournament_certificate_body_font: PathBuf,
    pub tournament_certificate_body_font_px: f32,
    pub tournament_certificate_body_vertical_shift_px: i32,
    pub label_font: PathBuf,
    pub label_font_px: f32,
    pub label_vertical_shift_px: i32,
    pub roster_name_font: PathBuf,
    pub roster_name_font_px: f32,
    pub roster_name_vertical_shift_px: i32,
    pub fixed_prompt_font: PathBuf,
    pub fixed_prompt_font_px: f32,
    pub fixed_prompt_vertical_shift_px: i32,
    pub compact_prompt_font: PathBuf,
    pub compact_prompt_font_px: f32,
    pub compact_prompt_vertical_shift_px: i32,
    pub solo_state_prompt_font: PathBuf,
    pub solo_state_prompt_font_px: f32,
    pub solo_state_prompt_vertical_shift_px: i32,
    pub common_pause_menu_font: PathBuf,
    pub common_pause_menu_font_px: f32,
    pub common_pause_menu_vertical_shift_px: i32,
    pub solo_story_intro_font: PathBuf,
    pub solo_story_intro_font_px: f32,
    pub solo_story_intro_vertical_shift_px: i32,
    pub solo_episode_card_font: PathBuf,
    pub solo_episode_card_font_px: f32,
    pub solo_episode_card_vertical_shift_px: i32,
    pub practical_selection_label_font: PathBuf,
    pub practical_selection_label_font_px: f32,
    pub practical_selection_label_vertical_shift_px: i32,
    pub stage_label_font: PathBuf,
    pub stage_label_font_px: f32,
    pub stage_label_vertical_shift_px: i32,
    pub league_standing_label_font: PathBuf,
    pub league_standing_label_font_px: f32,
    pub league_standing_label_vertical_shift_px: i32,
    pub selection_help_font: PathBuf,
    pub selection_help_font_px: f32,
    pub selection_help_vertical_shift_px: i32,
    pub atlas: CharacterSelectAtlasPlan,
}

#[derive(Debug, Clone)]
pub struct CharacterSelectAtlasBuildConfig {
    pub native_text_font: crate::development_build_spec::SizedFontSource,
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: CharacterSelectFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug)]
pub struct CharacterSelectAtlasBuild {
    pub records: Vec<Vec<u8>>,
    pub auxiliary_records: Vec<Vec<u8>>,
    pub overlays: Vec<Vec<u8>>,
    pub manifest_sha256: String,
    pub report: CharacterSelectAtlasBuildReport,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAtlasBuildReport {
    pub selector_names: Option<crate::name_input::SelectorRuntimeInstallReport>,
    pub small_logo: serde_json::Value,
    pub system_settings: crate::system_settings_text::SystemSettingsBuildReport,
    pub cooperative_diagnosis: crate::cooperative_diagnosis::DiagnosisBuildReport,
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_assets_sha256: String,
    pub source_shared_atlas_sha256: String,
    pub source_inventory_complete: bool,
    pub source_inventory_unit_count: usize,
    pub source_inventory_entry_count: usize,
    pub translated_source_ui_count: usize,
    pub untranslated_source_ui_count: usize,
    pub untranslated_source_ui_ids: Vec<String>,
    pub preserved_source_ui_ids: Vec<String>,
    pub excluded_source_ui_ids: Vec<String>,
    pub runtime_composed_texture_entry_count: usize,
    pub unresolved_route_occurrence_count: usize,
    pub consumer: CharacterSelectConsumerPlan,
    pub fully_routed_translation_ids: Vec<String>,
    pub fully_routed_source_ui_ids: Vec<String>,
    pub unresolved_route_source_ui_ids: Vec<String>,
    pub unresolved_route_translation_ids: Vec<String>,
    pub route_census: CharacterSelectRouteCensus,
    pub record_count: usize,
    pub auxiliary_record_count: usize,
    pub glyph_count: usize,
    pub fixed_texture_strip_count: usize,
    pub source_ink_cleanup_count: usize,
    pub source_glyph_cell_count: usize,
    pub all_records_share_select_heading_allocations: bool,
    pub changed_bytes_confined_to_allocated_cells: bool,
    pub compression_roundtrip_verified: bool,
    pub catalog_prefixes_preserved: bool,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub records: Vec<CharacterSelectAtlasRecordBuild>,
    pub auxiliary_records: Vec<CharacterSelectAuxiliaryRecordBuild>,
    pub overlays: Vec<CharacterSelectOverlayBuild>,
    pub source_glyph_cells: Vec<CharacterSelectSourceGlyphCell>,
    pub glyphs: Vec<CharacterSelectGlyphBuild>,
    pub fixed_strips: Vec<CharacterSelectFixedStripBuild>,
    pub source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupBuild>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAuxiliaryRecordBuild {
    pub source_path: String,
    pub output_file: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub source_record_size: usize,
    pub changed_stored_byte_ranges: Vec<[usize; 2]>,
    pub compressed_streams: Vec<CharacterSelectCompressedStreamBuild>,
    pub target_tims: Vec<CharacterSelectTargetTimBuild>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectCompressedStreamBuild {
    pub stream_index: usize,
    pub decoded_range: [usize; 2],
    pub source_decoded_sha256: String,
    pub patched_decoded_sha256: String,
    pub changed: bool,
    pub unpadded_stored_size: usize,
    pub encoded_stream_offset: usize,
    pub encoded_stream_capacity: usize,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectTargetTimBuild {
    pub target_tim_offset: usize,
    pub source_target_tim_sha256: String,
    pub patched_target_tim_sha256: String,
    pub atlas_preview_file: String,
    pub atlas_preview_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectAtlasRecordBuild {
    pub source_path: String,
    pub output_file: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub patched_shared_atlas_sha256: String,
    pub native_text: serde_json::Value,
    pub source_record_size: usize,
    pub changed_stored_byte_ranges: Vec<[usize; 2]>,
    pub compressed_streams: Vec<CharacterSelectCompressedStreamBuild>,
    pub target_tims: Vec<CharacterSelectTargetTimBuild>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectOverlayBuild {
    pub source_path: String,
    pub output_file: String,
    pub source_sha256: String,
    pub patched_sha256: String,
    pub source_size: usize,
    pub expected_write_ranges: Vec<[usize; 2]>,
    pub changed_byte_ranges: Vec<[usize; 2]>,
    pub source_guards: Vec<CharacterSelectSourceGuardBuild>,
    pub descriptors: Vec<CharacterSelectDescriptorBuild>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectSourceGuardBuild {
    pub role: String,
    pub byte_range: [usize; 2],
    pub source_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectDescriptorBuild {
    pub route_occurrence_id: String,
    pub source_ui_id: String,
    pub translation_id: String,
    pub descriptor_offset: usize,
    pub logical_character_count: usize,
    pub encoded_glyph_count: usize,
    pub omitted_separator_count: usize,
    pub texture_page_indices: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectGlyphBuild {
    pub font_role: CharacterSelectFontRole,
    pub character: char,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub texture_page_index: u8,
    pub texture_uv: [u8; 2],
    pub cell: Cell,
    pub source_cell_occupancy: CharacterSelectSourceCellOccupancy,
    pub clear_index: u8,
    pub outline_index: u8,
    pub fill_index: u8,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub indexed_sha256: String,
    pub measured_advance_px: f32,
    pub changed_decoded_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectFixedStripBuild {
    pub physical_text_region_id: String,
    pub source_ui_ids: Vec<String>,
    pub translation_id: String,
    pub text_selection: CharacterSelectTextSelection,
    pub text_flow: CharacterSelectTextFlow,
    pub write_mode: CharacterSelectFixedStripWriteMode,
    pub korean_text: String,
    pub font_role: CharacterSelectFontRole,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub cell: Cell,
    pub text_cell: Cell,
    pub clear_index: u8,
    pub outline_index: u8,
    pub fill_index: u8,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub indexed_sha256: String,
    pub measured_advance_px: f32,
    pub changed_decoded_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct CharacterSelectSourceInkCleanupBuild {
    pub physical_region_id: String,
    pub surface: CharacterSelectTextureSurface,
    pub tim_offset: usize,
    pub cell: Cell,
    pub first_ink_index: u8,
    pub last_ink_index: u8,
    pub replacement_index: u8,
    pub source_ink_count: usize,
    pub changed_decoded_byte_count: usize,
}
