use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::{ShiftedSizedFontSource, SizedFontSource};
use crate::embedded_tim::EmbeddedTimAudit;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct ModeDescendantGraphicsAuditConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantGraphicsAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub record_count: usize,
    pub tim_count: usize,
    pub unique_tim_count: usize,
    pub shared_tim_count: usize,
    pub edit_command_sheets: EditCommandSheetFamilyAudit,
    pub practical_exam_direct_texture_censuses: Vec<PracticalExamDirectTextureCensusAudit>,
    pub practical_exam_secondary_descriptor_censuses:
        Vec<PracticalExamSecondaryDescriptorCensusAudit>,
    pub practical_results: super::practical_results::PracticalResultBuildReport,
    pub texture_usage: Vec<ModeDescendantTextureUsage>,
    pub records: Vec<ModeDescendantGraphicsRecordAudit>,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetFamilyAudit {
    pub source_path: String,
    pub member_count: usize,
    pub all_primary_tims_are_unique: bool,
    pub all_secondary_tims_are_identical: bool,
    pub no_change_rebuild_is_byte_identical: bool,
    pub consumer: EditCommandSheetConsumerAudit,
    pub members: Vec<EditCommandSheetMemberAudit>,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetConsumerAudit {
    pub source_path: String,
    pub source_sha256: String,
    pub source_load_address: String,
    pub catalog_index: usize,
    pub decoded_destination_address: String,
    pub selector_bias: i32,
    pub custom_record_stride_bytes: usize,
    pub custom_record_selector_byte_offset: usize,
    pub load_site_count: usize,
    pub member_selector_value_range_guard_present: bool,
    pub source_bindings_verified: bool,
    pub load_sites: Vec<EditCommandSheetLoadSiteAudit>,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetLoadSiteAudit {
    pub id: String,
    pub catalog_index_instruction_offset: String,
    pub selector_load_instruction_offset: String,
    pub selector_bias_instruction_offset: String,
    pub selector_source: String,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetMemberAudit {
    pub id: String,
    pub index: usize,
    pub selector_value: usize,
    pub table_pair_offset: usize,
    pub stored_offset: usize,
    pub stored_size: usize,
    pub slot_size: usize,
    pub source_stored_sha256: String,
    pub decoded_offset: usize,
    pub decoded_size: usize,
    pub source_decoded_sha256: String,
    pub primary_tim: EmbeddedTimAudit,
    pub secondary_tim: EmbeddedTimAudit,
}

#[derive(Debug, Serialize)]
pub struct PracticalExamDirectTextureCensusAudit {
    pub consumer: String,
    pub source_path: String,
    pub source_sha256: String,
    pub full_census_complete: bool,
    pub census_region_count: usize,
    pub adopted_region_count: usize,
    pub canonical_cell_count: usize,
    pub canonical_cell_set_matches: bool,
    pub canonical_cells: Vec<Cell>,
}

#[derive(Debug, Serialize)]
pub struct PracticalExamSecondaryDescriptorCensusAudit {
    pub consumer: String,
    pub source_path: String,
    pub source_sha256: String,
    pub pointer_table_offset: usize,
    pub pointer_table_entry_count: usize,
    pub pointer_table_sha256: String,
    pub descriptor_arena_start: usize,
    pub descriptor_arena_end: usize,
    pub physical_descriptor_count: usize,
    pub alias_partition_complete: bool,
    pub descriptors: Vec<PracticalExamSecondaryDescriptorAudit>,
}

#[derive(Debug, Serialize)]
pub struct PracticalExamSecondaryDescriptorAudit {
    pub aliases: Vec<usize>,
    pub source_offset: usize,
    pub encoded_size: usize,
    pub source_sha256: String,
    pub fragments: Vec<PracticalExamSecondaryDescriptorFragmentAudit>,
}

#[derive(Debug, Serialize)]
pub struct PracticalExamSecondaryDescriptorFragmentAudit {
    pub texture_bank: PracticalExamSecondaryTextureBankAudit,
    pub texture_page: u8,
    pub clut_x_index: u8,
    pub clut_y_offset: u8,
    pub source_cell: Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalExamSecondaryTextureBankAudit {
    SharedProducer,
    External,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantTextureUsage {
    pub source_tim_sha256: String,
    pub bits_per_pixel: u8,
    pub total_size: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub palette_count: usize,
    pub consumer_count: usize,
    pub consumers: Vec<ModeDescendantTextureConsumer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Serialize)]
pub struct ModeDescendantTextureConsumer {
    pub role: String,
    pub source_path: String,
    pub tim_offset: usize,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantGraphicsRecordAudit {
    pub role: String,
    pub source_path: String,
    pub storage_kind: ModeDescendantStorageKind,
    pub source_extent_lba: u32,
    pub source_stored_size: usize,
    pub source_stored_sha256: String,
    pub source_decoded_size: usize,
    pub source_decoded_sha256: String,
    pub source_allows_trailing_bytes: bool,
    pub tim_count: usize,
    pub tims: Vec<EmbeddedTimAudit>,
}

#[derive(Debug, Clone)]
pub struct ModeDescendantFontSources {
    pub calendar_text: ShiftedSizedFontSource,
    pub gorin_heading: SizedFontSource,
    pub gorin_menu: SizedFontSource,
    pub gorin_speed_marker: Option<SizedFontSource>,
    pub gorin_small_label: Option<SizedFontSource>,
    pub edit_badge: Option<SizedFontSource>,
    pub edit_condition: Option<SizedFontSource>,
    pub edit_heading: SizedFontSource,
    pub edit_label: SizedFontSource,
    pub edit_compact_label: SizedFontSource,
    pub training_text: ShiftedSizedFontSource,
    pub battle_counter: ShiftedSizedFontSource,
    pub edit_team_up_name: SizedFontSource,
    pub edit_school_label: SizedFontSource,
    pub edit_runtime_text: ShiftedSizedFontSource,
    pub password: Option<ShiftedSizedFontSource>,
    pub practical_title: ShiftedSizedFontSource,
    /// Shares the instruction/title font selected by the product build.
    pub practical_gameplay: ShiftedSizedFontSource,
    pub practical_menu_label: ShiftedSizedFontSource,
    pub practical_prompt: ShiftedSizedFontSource,
    pub practical_hint: ShiftedSizedFontSource,
    pub practical_result_heading: ShiftedSizedFontSource,
    pub practical_result_label: ShiftedSizedFontSource,
    pub practical_result_hint: ShiftedSizedFontSource,
    pub practical_result_action: ShiftedSizedFontSource,
    pub practical_result_small_judgment: ShiftedSizedFontSource,
    pub practical_result_judgment_stamp: ShiftedSizedFontSource,
    pub practical_result_branding: ShiftedSizedFontSource,
}

#[derive(Debug, Clone)]
pub struct ModeDescendantGraphicsBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: ModeDescendantFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct ModeDescendantGraphicsBuild {
    pub stored_records: BTreeMap<ModeDescendantRecord, Vec<u8>>,
    pub(crate) decoded_write_claims: BTreeMap<ModeDescendantRecord, Vec<DecodedDataClaim>>,
    pub build_manifest_sha256: String,
    pub report: ModeDescendantGraphicsBuildReport,
}

impl ModeDescendantGraphicsBuild {
    pub fn stored_record(&self, record: ModeDescendantRecord) -> Option<&[u8]> {
        self.stored_records.get(&record).map(Vec::as_slice)
    }

    pub(crate) fn decoded_write_claims(
        &self,
        record: ModeDescendantRecord,
    ) -> Option<&[DecodedDataClaim]> {
        self.decoded_write_claims.get(&record).map(Vec::as_slice)
    }

    pub(crate) fn reported_records_match_stored_records(&self) -> bool {
        let reported = self
            .report
            .records
            .iter()
            .map(|record| record.record)
            .collect::<BTreeSet<_>>();

        !reported.is_empty()
            && reported.len() == self.report.records.len()
            && reported.len() == self.stored_records.len()
            && reported.len() == self.decoded_write_claims.len()
            && reported.iter().all(|record| {
                self.stored_records.contains_key(record)
                    && self.decoded_write_claims.contains_key(record)
            })
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeDescendantSurface {
    MainTitle,
    GorinGameplayHud,
    GorinMainMenu,
    EditSharedUi,
    PracticalExamSharedUi,
    PracticalExamResults,
    TrainingMenu,
    ContinueSlots,
    BattleAnnouncements,
}

impl ModeDescendantSurface {
    pub(super) const ALL: [Self; 9] = [
        Self::MainTitle,
        Self::GorinGameplayHud,
        Self::GorinMainMenu,
        Self::EditSharedUi,
        Self::PracticalExamSharedUi,
        Self::PracticalExamResults,
        Self::TrainingMenu,
        Self::ContinueSlots,
        Self::BattleAnnouncements,
    ];
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeDescendantRecord {
    MainTitle,
    VerticalAnnouncementEffects,
    AlternateVerticalAnnouncementEffects,
    GorinBallGameEffects,
    GorinSprintEffects,
    GorinDanceEffects,
    GorinMainMenu,
    GorinTitle1,
    GorinTitle2,
    GorinTitle3,
    GorinTitle4,
    GorinTitle5,
    GorinTitle6,
    GorinTitle7,
    GorinTitle8,
    GorinTitle9,
    GorinTitle10,
    GorinTitle11,
    GorinTitle12,
    GorinSprintSelector,
    GorinDanceSelector,
    EditSharedUi,
    EditCommandSheets,
    EditTechniqueNames,
    PracticalBasicsTextureProducer,
    Practical1999TextureProducer,
    PracticalBasicsDescriptorConsumer,
    Practical1999DescriptorConsumer,
    PracticalBasicsResultGraphics,
    Practical1999ResultGraphics,
    PracticalResultTermTitles,
    BattleEffects,
    TrainingMenuTexture,
    TrainingMenuOverlay,
    ContinueSchoolTexture,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeDescendantStorageKind {
    IndexedCompressedMembers,
    PagedCompressed,
    Raw,
    TzzCompressedMembers,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeDescendantFontRole {
    GorinHeading,
    GorinMenu,
    EditHeading,
    EditLabel,
    EditCompactLabel,
    EditSchoolLabel,
    PracticalTitle,
    PracticalMenuLabel,
    PracticalPrompt,
    PracticalHint,
    PracticalResultHeading,
    PracticalResultLabel,
    PracticalResultHint,
    PracticalResultAction,
    PracticalResultSmallJudgment,
    PracticalResultJudgmentStamp,
    PracticalResultBranding,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TextAlignment {
    Left,
    Center,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeDescendantManifest {
    pub(super) kind: String,
    pub(super) units: Vec<PathBuf>,
    pub(super) edit_command_sheets: PathBuf,
    pub(super) edit_technique_names: PathBuf,
    #[serde(default)]
    pub(super) training_menu: Option<PathBuf>,
    #[serde(default)]
    pub(super) battle_announcements: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_retry: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_announcements: Option<PathBuf>,
    #[serde(default)]
    pub(super) main_title: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_dance_intro: Option<PathBuf>,
    pub(super) gorin_gauge_labels: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_home_run: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_sprint_announcements: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_dance_announcements: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_dance_logo: Option<PathBuf>,
    #[serde(default)]
    pub(super) gorin_hud_labels: Option<PathBuf>,
    #[serde(default)]
    pub(super) continue_schools: Option<PathBuf>,
    pub(super) gorin_selector_names: Option<PathBuf>,
    #[serde(default)]
    pub(super) illustrated_panels: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeDescendantUnit {
    pub(super) kind: String,
    pub(super) entries: Vec<ModeDescendantEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeDescendantEntry {
    pub(super) id: String,
    pub(super) surface: ModeDescendantSurface,
    /// Optional physical subset for a fixed texture shared by only some records.
    pub(super) records: Option<Vec<ModeDescendantRecord>>,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) placement: ModeDescendantPlacement,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FixedPaletteRoles {
    pub clear_index: u8,
    pub outline_index: Option<u8>,
    pub fill_index: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ModeDescendantPlacement {
    Fixed {
        bits_per_pixel: u8,
        tim_offset: String,
        cell: Cell,
        alignment: TextAlignment,
        source_region_sha256: String,
        #[serde(default)]
        palette_roles: Option<FixedPaletteRoles>,
        #[serde(default)]
        vertical_shift_px: i32,
    },
    Fixed4bppWithoutClut {
        tim_offset: String,
        cell: Cell,
        alignment: TextAlignment,
        clear_index: u8,
        outline_index: Option<u8>,
        fill_index: u8,
        source_region_sha256: String,
    },
    Dynamic,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantGraphicsBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub illustrated_panels: Vec<super::illustrated_panels::IllustratedPanelReport>,
    pub gorin_selector_names: Option<super::gorin_selector_names::SelectorNamesReport>,
    pub record_owned_unit_count: usize,
    pub record_composited_unit_count: usize,
    pub record_owned_source_regions_match: bool,
    pub record_owned_cells_are_unique_and_non_overlapping: bool,
    pub record_owned_changes_confined_to_owned_cells: bool,
    pub edit_cpu_tactics_instruction_title_consumer_count: usize,
    pub edit_cpu_tactics_body_consumer_count: usize,
    pub edit_cpu_tactics_instruction_title_consumer_bindings_verified: bool,
    pub edit_command_sheets: EditCommandSheetBuildReport,
    pub edit_technique_names: super::edit_technique_names::TechniqueNameBuildReport,
    pub training_menu: Option<super::training_menu::TrainingMenuBuildReport>,
    pub battle_announcements: Option<super::battle_announcements::BattleAnnouncementReport>,
    pub gorin_retry: Option<super::gorin_retry::GorinRetryReport>,
    pub gorin_announcements: Option<super::gorin_announcements::AnnouncementReport>,
    pub main_title: Option<super::main_title::MainTitleReport>,
    pub gorin_dance_intro: Option<super::gorin_dance_intro::DanceIntroReport>,
    pub gorin_gauge_labels: Option<super::gorin_gauge_labels::GaugeLabelReport>,
    pub gorin_home_run: Option<super::gorin_home_run::HomeRunReport>,
    pub gorin_sprint_announcements:
        Option<super::gorin_sprint_announcements::SprintAnnouncementReport>,
    pub gorin_dance_announcements:
        Option<super::gorin_dance_announcements::DanceAnnouncementReport>,
    pub gorin_dance_logo: Option<super::gorin_dance_logo::DanceLogoReport>,
    pub gorin_hud_labels: Option<super::gorin_hud_labels::HudLabelReport>,
    pub continue_schools: Option<super::continue_schools::ContinueSchoolsBuildReport>,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub practical_results: Option<super::practical_results::PracticalResultBuildReport>,
    pub records: Vec<ModeDescendantRecordBuild>,
    pub units: Vec<ModeDescendantUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetBuildReport {
    pub team_up_names: super::edit_team_up_names::Report,
    pub air_allowed_badges: super::edit_badges::Report,
    pub plain_conditions: super::edit_conditions::Report,
    pub source_path: String,
    pub move_name_manifest_sha256: String,
    pub move_name_translation_count: usize,
    pub move_name_translation_status: String,
    pub move_names: Vec<EditCommandSheetMoveNameBuildReport>,
    pub manifest_sha256: String,
    pub member_count: usize,
    pub semantic_label_count: usize,
    pub physical_occurrence_count: usize,
    pub authored_common_label_occurrence_count: usize,
    pub authored_move_name_occurrence_count: usize,
    pub pending_move_name_occurrence_count: usize,
    pub distinct_source_primary_clut_count: usize,
    pub fill_palette_roles_verified: bool,
    pub full_label_backgrounds_reconstructed: bool,
    pub source_regions_match: bool,
    pub changes_confined_to_authored_occurrences: bool,
    pub primary_cluts_preserved: bool,
    pub secondary_cluts_preserved: bool,
    pub members: Vec<EditCommandSheetMemberBuildReport>,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetMoveNameBuildReport {
    pub id: String,
    pub source_text: String,
    pub korean_lines: Vec<String>,
    pub source_cell: crate::tim::Cell,
    pub render_cell: crate::tim::Cell,
    pub font_px: f32,
    pub line_advance_px: Vec<f32>,
    pub source_background_index: u8,
    pub fill_index: u8,
}

#[derive(Debug, Serialize)]
pub struct EditCommandSheetMemberBuildReport {
    pub member_index: usize,
    pub authored_common_label_occurrence_count: usize,
    pub authored_move_name_occurrence_count: usize,
    pub pending_move_name_occurrence_count: usize,
    pub changed_decoded_byte_count: usize,
    pub source_preview_file: String,
    pub patched_preview_file: String,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantRecordBuild {
    pub record: ModeDescendantRecord,
    pub surface: ModeDescendantSurface,
    pub source_path: String,
    pub storage_kind: ModeDescendantStorageKind,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub source_record_size: usize,
    pub unpadded_stored_size: Option<usize>,
    pub compression_roundtrip_verified: Option<bool>,
    pub catalog_prefix_preserved: Option<bool>,
    pub indexed_member_offsets_preserved: Option<bool>,
    pub indexed_member_sizes_match_rebuilt_streams: Option<bool>,
    pub indexed_members: Vec<ModeDescendantIndexedMemberBuild>,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantIndexedMemberBuild {
    pub id: String,
    pub table_pair_offset: usize,
    pub stored_offset: usize,
    pub slot_size: usize,
    pub source_stored_size: usize,
    pub source_stored_sha256: String,
    pub patched_stored_size: usize,
    pub patched_stored_sha256: String,
    pub decoded_size: usize,
    pub source_decoded_sha256: String,
    pub patched_decoded_sha256: String,
    pub changed_stored_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ModeDescendantUnitBuild {
    pub id: String,
    pub surface: ModeDescendantSurface,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: ModeDescendantFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub cells: Vec<Cell>,
    pub measured_advance_px: f32,
    pub changed_decoded_byte_count: usize,
    pub placement: ModeDescendantUnitPlacementBuild,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModeDescendantUnitPlacementBuild {
    FixedCell {
        record: ModeDescendantRecord,
        bits_per_pixel: u8,
        clut_in_tim: bool,
        tim_offset: String,
        source_region_sha256: String,
        source_index_histogram: Vec<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fill_gradient: Option<super::continue_schools::SchoolGradient>,
        clear_index: u8,
        outline_index: Option<u8>,
        fill_index: u8,
    },
    GorinHeading {
        record: ModeDescendantRecord,
        bits_per_pixel: u8,
        tim_offset: String,
        source_region_sha256: String,
        source_exclusive_palette_index_count: usize,
        reconstructed_background_pixel_count: usize,
        korean_ink_pixel_count: usize,
        source_pixels_preserved_outside_reconstruction_and_korean_ink: bool,
    },
    PracticalExamGlyphStream {
        vertical_shift_px: i32,
        glyph_count: usize,
        consumer_count: usize,
        atlas_advance_px: usize,
    },
}
