use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::{Deserialize, Serialize};

use crate::compression::decompress;
use crate::container_plan::{ContainerMemberSpec, ContainerPlan, MemberContribution};
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::development_build_spec::ShiftedSizedFontSource;
use crate::embedded_tim::{decode_embedded_tim_preview, detect_embedded_tim_images};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers};
use crate::paged_compression::{
    compress_page_safe_image, profile_paged_compression, source_paged_compression_profile,
};
use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    Cell, RgbaImage, parse_4bpp_prefix, read_4bpp_palette_words_in_prefix,
    read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};
use crate::tim_preview::write_tim_preview;
use crate::tzz::{TzzMember, parse_tzz};

use super::PRACTICAL_INSTRUCTION_PATH;

#[path = "build/battle_subject_titles.rs"]
mod battle_subject_titles;

#[path = "build/attempt_counter.rs"]
mod attempt_counter;

use battle_subject_titles::{
    BattleSubjectTitleMemberPatch, build_battle_subject_title_family,
    validate_battle_subject_title_ownership,
};

const MANIFEST_KIND: &str = "justice_gakuen2_practical_instruction_manifest";
const TRANSLATION_KIND: &str = "justice_gakuen2_practical_instruction_translation";
const OUTPUT_MARKER_FILE: &str = ".practical-instruction-graphics-build-output";
const OUTPUT_MARKER_TEXT: &str = "justice_gakuen2_practical_instruction_graphics_build\n";
const OUTPUT_RECORD_FILE: &str = "TESTMJ.TIZ";
const OUTPUT_REPORT_FILE: &str = "practical-instruction-build.json";
const PROMPT_TRANSLATIONS_FILE: &str = "gameplay-prompts.json";
const DYNAMIC_OVERLAYS_FILE: &str = "dynamic-overlays.json";
const TILE_WIDTH: usize = 32;
const TILE_HEIGHT: usize = 16;
const SOURCE_TILE_COLUMNS: usize = 8;
const PROMPT_MEMBER_COUNT: usize = 30;
const PROMPT_MAPPING_BYTE_COUNT: usize = 32;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PresentationLayout {
    source_tile_indices: Vec<usize>,
    rows: Vec<PresentationRow>,
    initial_row_tile_counts: Option<Vec<usize>>,
    text_palette_indices: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PresentationRow {
    x: usize,
    y: usize,
    tile_count: usize,
}

#[derive(Debug, Clone)]
pub struct PracticalInstructionGraphicsBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub body_font: ShiftedSizedFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionGraphicsBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub source_record_path: String,
    pub source_record_size: usize,
    pub source_record_sha256: String,
    pub patched_record_sha256: String,
    pub manifest_sha256: String,
    pub member_count: usize,
    pub authored_member_count: usize,
    pub pending_member_count: usize,
    pub all_source_members_bound: bool,
    pub all_unowned_members_preserved: bool,
    pub member_table_preserved: bool,
    pub compression_roundtrip_verified: bool,
    pub primary_clut_member_count: usize,
    pub primary_clut_vram_x: usize,
    pub primary_clut_vram_y: usize,
    pub source_primary_clut_palette_family_sha256: String,
    pub patched_primary_clut_palette_family_sha256: String,
    pub all_primary_clut_palettes_preserved: bool,
    pub complete_scope: bool,
    pub release_candidate_input_eligible: bool,
    pub output_record_file: String,
    pub gameplay_prompts: PracticalGameplayPromptBuildReport,
    pub dynamic_overlays: PracticalDynamicOverlayBuildReport,
    pub battle_subject_titles: PracticalBattleSubjectTitleBuildReport,
    pub members: Vec<PracticalInstructionMemberBuildReport>,
}

pub struct PracticalInstructionGraphicsBuild {
    pub record: Vec<u8>,
    pub prompt_consumers: Vec<PracticalInstructionPromptConsumerBuild>,
    pub battle_subject_title_consumer: PracticalBattleSubjectTitleConsumerBuild,
    pub build_manifest_sha256: String,
    pub report: PracticalInstructionGraphicsBuildReport,
}

pub struct PracticalBattleSubjectTitleConsumerBuild {
    pub path: String,
    pub source_record: Vec<u8>,
    pub record: Vec<u8>,
    pub source_record_size: usize,
    pub source_record_sha256: String,
    pub patched_record_sha256: String,
}

pub struct PracticalInstructionPromptConsumerBuild {
    pub path: String,
    pub record: Vec<u8>,
    pub source_record_size: usize,
    pub source_record_sha256: String,
    pub patched_record_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PracticalGameplayPromptBuildReport {
    pub translation_manifest_sha256: String,
    pub consumer_count: usize,
    pub member_count: usize,
    pub pointer_bound_record_count: usize,
    pub reachable_record_count: usize,
    pub drawable_reachable_record_count: usize,
    pub non_drawing_reachable_record_count: usize,
    pub localized_member_count: usize,
    pub pending_member_count: usize,
    pub localized_record_count: usize,
    pub human_approved_record_count: usize,
    pub localized_action_ids: Vec<u8>,
    pub all_consumer_tables_structurally_identical: bool,
    pub all_changes_confined_to_prompt_owned_tiles_and_records: bool,
    pub consumers: Vec<PracticalGameplayPromptConsumerReport>,
    pub members: Vec<PracticalGameplayPromptMemberReport>,
}

#[derive(Debug, Serialize)]
pub struct PracticalGameplayPromptConsumerReport {
    pub path: String,
    pub runtime_base: String,
    pub source_record_size: usize,
    pub source_record_sha256: String,
    pub patched_record_sha256: String,
    pub changed_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PracticalGameplayPromptMemberReport {
    pub member_index: usize,
    pub pointer_bound_record_count: usize,
    pub reachable_record_count: usize,
    pub drawable_reachable_record_count: usize,
    pub non_drawing_reachable_record_count: usize,
    pub source_prompt_tile_count: usize,
    pub allocated_source_transparent_scratch_tile_count: usize,
    pub allocated_source_transparent_scratch_tiles: Vec<usize>,
    pub allocated_prompt_tile_count: usize,
    pub source_records: Vec<PracticalGameplayPromptRecordReport>,
    pub localized: bool,
    pub action_ids: Vec<u8>,
    pub korean_labels: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PracticalGameplayPromptRecordReport {
    pub record_ordinal: usize,
    pub action_id: u8,
    pub reachable: bool,
    pub fragments: Vec<PracticalGameplayPromptFragmentReport>,
    pub source_preview_file: Option<String>,
    pub source_preview_sha256: Option<String>,
    pub allocated_start_tile: Option<usize>,
    pub allocated_tile_count: Option<usize>,
    pub uses_source_transparent_scratch: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PracticalGameplayPromptFragmentReport {
    pub new_line: bool,
    pub start_tile: usize,
    pub tile_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PracticalDynamicOverlayBuildReport {
    pub attempt_counter: Option<attempt_counter::AttemptCounterReport>,
    pub translation_manifest_sha256: String,
    pub localized_overlay_count: usize,
    pub all_changes_confined_to_consumer_bound_tiles: bool,
    pub overlays: Vec<PracticalDynamicOverlayReport>,
}

#[derive(Debug, Serialize)]
pub struct PracticalDynamicOverlayReport {
    pub id: String,
    pub producer_member_index: usize,
    pub consumer_path: String,
    pub consumer_sha256: String,
    pub packet_builder_offset: String,
    pub counter_formula: String,
    pub source_text: String,
    pub korean_text: String,
    pub human_approval: HumanApproval,
    pub prefix_tiles: Vec<usize>,
    pub digit_tiles: Vec<usize>,
    pub suffix_tiles: Vec<usize>,
    pub dynamic_digits_regenerated: bool,
    pub changed_decoded_byte_count: usize,
    pub preview_file: String,
    pub preview_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PracticalBattleSubjectTitleBuildReport {
    pub term_translation_catalog_path: String,
    pub term_translation_catalog_sha256: String,
    pub native_term_title_member_count: usize,
    pub translation_catalog_path: String,
    pub translation_catalog_sha256: String,
    pub consumer_path: String,
    pub consumer_runtime_base: String,
    pub consumer_function_offset: String,
    pub source_consumer_sha256: String,
    pub patched_consumer_sha256: String,
    pub member_count: usize,
    pub localized_member_count: usize,
    pub old_prefix_packets_moved_offscreen: bool,
    pub title_strip_screen_x: i16,
    pub all_changes_confined_to_title_strips_and_consumer_bindings: bool,
    pub members: Vec<PracticalBattleSubjectTitleMemberReport>,
}

#[derive(Debug, Serialize)]
pub struct PracticalBattleSubjectTitleMemberReport {
    pub member_index: usize,
    pub title_semantic_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_packet_width: u8,
    pub patched_packet_width: u8,
    pub allocated_strip_width: usize,
    pub owned_title_tiles: Vec<usize>,
    pub changed_decoded_byte_count: usize,
    pub preview_file: String,
    pub preview_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionMemberBuildReport {
    pub asset_unit_id: String,
    pub title_semantic_id: String,
    pub member_index: usize,
    pub translation_state: TranslationState,
    pub human_approval: Option<HumanApproval>,
    pub source_decoded_sha256: String,
    pub patched_decoded_sha256: String,
    pub primary_clut_vram_x: usize,
    pub primary_clut_vram_y: usize,
    pub source_primary_clut_palette_sha256: String,
    pub patched_primary_clut_palette_sha256: String,
    pub primary_clut_palette_preserved: bool,
    pub source_compressed_sha256: String,
    pub patched_compressed_sha256: String,
    pub source_compressed_size: usize,
    pub translation_asset_sha256: Option<String>,
    pub reencoded_size: Option<usize>,
    pub compression_headroom: Option<usize>,
    pub source_compression_maximum_match_words: Option<usize>,
    pub source_compression_maximum_control_block_output_words: Option<usize>,
    pub source_compression_control_blocks_crossing_input_pages: Option<usize>,
    pub rebuilt_compression_maximum_match_words: Option<usize>,
    pub rebuilt_compression_maximum_control_block_output_words: Option<usize>,
    pub rebuilt_compression_control_blocks_crossing_input_pages: Option<usize>,
    pub changed_decoded_byte_count: usize,
    pub korean_lines: Vec<String>,
    pub source_transparent_palette_index: Option<u8>,
    pub source_transparent_palette_indices: Vec<u8>,
    pub text_palette_indices: Vec<u8>,
    pub preserved_graphics: Vec<PracticalInstructionPreservedGraphicReport>,
    pub gameplay_prompt_localized: bool,
    pub gameplay_prompt_action_ids: Vec<u8>,
    pub gameplay_prompt_korean_labels: Vec<String>,
    pub gameplay_prompt_preview_file: Option<String>,
    pub gameplay_prompt_preview_sha256: Option<String>,
    pub dynamic_overlay_ids: Vec<String>,
    pub source_presentation_preview_file: Option<String>,
    pub source_presentation_preview_sha256: Option<String>,
    pub preview_file: Option<String>,
    pub preview_sha256: Option<String>,
    pub atlas_preview_file: Option<String>,
    pub atlas_preview_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PracticalInstructionPreservedGraphicReport {
    pub semantic_id: String,
    pub source_tile_indices: Vec<usize>,
    pub presentation_row: usize,
    pub presentation_column: usize,
    pub preview_file: String,
    pub preview_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PracticalInstructionManifest {
    kind: String,
    source_record_path: String,
    source_record_sha256: String,
    continuity_group_id: String,
    members: Vec<PracticalInstructionMemberBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PracticalInstructionMemberBinding {
    asset_unit_id: String,
    title_semantic_id: String,
    member_index: usize,
    source_decoded_sha256: String,
    translation_state: TranslationState,
    presentation_layout: Option<PresentationLayout>,
    translation: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationState {
    Pending,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanApproval {
    Pending,
    Approved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PracticalInstructionTranslation {
    kind: String,
    asset_unit_id: String,
    source_text: String,
    korean_lines: Vec<String>,
    #[serde(default)]
    text_placements: Vec<PresentationTextPlacement>,
    #[serde(default)]
    preserved_graphics: Vec<PreservedPresentationGraphic>,
    human_approval: HumanApproval,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PresentationTextPlacement {
    row: usize,
    column: usize,
    text: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PreservedPresentationGraphic {
    semantic_id: String,
    source_tile_indices: Vec<usize>,
    row: usize,
    column: usize,
}

struct AuthoredMember {
    compressed: Vec<u8>,
    decoded: Vec<u8>,
    reencoded_size: usize,
    source_compression_maximum_match_words: usize,
    source_compression_maximum_control_block_output_words: usize,
    source_compression_control_blocks_crossing_input_pages: usize,
    rebuilt_compression_maximum_match_words: usize,
    rebuilt_compression_maximum_control_block_output_words: usize,
    rebuilt_compression_control_blocks_crossing_input_pages: usize,
    changed_decoded_byte_count: usize,
    korean_lines: Vec<String>,
    preserved_graphics: Vec<PracticalInstructionPreservedGraphicReport>,
    human_approval: HumanApproval,
    translation_asset_sha256: String,
    preview_file: String,
    preview_sha256: String,
    atlas_preview_file: String,
    atlas_preview_sha256: String,
    gameplay_prompt_action_ids: Vec<u8>,
    gameplay_prompt_korean_labels: Vec<String>,
    gameplay_prompt_preview_file: Option<String>,
    gameplay_prompt_preview_sha256: Option<String>,
    dynamic_overlay_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameplayPromptTranslationManifest {
    kind: String,
    pointer_table_offset: usize,
    member_count: usize,
    consumers: Vec<GameplayPromptConsumerBinding>,
    actions: Vec<GameplayPromptTranslation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameplayPromptConsumerBinding {
    path: String,
    runtime_base: u32,
    source_record_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameplayPromptTranslation {
    action_id: u8,
    semantic_id: String,
    source_text: String,
    korean_text: String,
    human_approval: HumanApproval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameplayPromptFragment {
    new_line: bool,
    start_tile: usize,
    tile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameplayPromptRecord {
    offset: usize,
    encoded_size: usize,
    action_id: u8,
    fragments: Vec<GameplayPromptFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameplayPromptMemberLayout {
    member_index: usize,
    mapping: Vec<u8>,
    records: Vec<GameplayPromptRecord>,
    reachable_record_ordinals: BTreeSet<usize>,
    source_prompt_tiles: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameplayPromptConsumerLayout {
    members: Vec<GameplayPromptMemberLayout>,
}

struct GameplayPromptMemberPatch {
    candidate: Vec<u8>,
    claims: Vec<DecodedDataClaim>,
    record_ordinals: Vec<usize>,
    action_ids: Vec<u8>,
    korean_labels: Vec<String>,
    tile_allocations: Vec<(usize, usize)>,
    owned_tile_indices: BTreeSet<usize>,
    allocated_source_transparent_scratch_tiles: BTreeSet<usize>,
}

struct GameplayPromptFamilyBuild {
    member_patches: BTreeMap<usize, GameplayPromptMemberPatch>,
    consumers: Vec<PracticalInstructionPromptConsumerBuild>,
    report: PracticalGameplayPromptBuildReport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DynamicOverlayManifest {
    kind: String,
    entries: Vec<DynamicOverlayTranslation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DynamicOverlayTranslation {
    id: String,
    producer_member_index: usize,
    consumer_path: String,
    consumer_runtime_base: u32,
    source_consumer_sha256: String,
    source_text: String,
    korean_text: String,
    sample_count: u8,
    human_approval: HumanApproval,
}

struct DynamicOverlayMemberPatch {
    candidate: Vec<u8>,
    claims: Vec<DecodedDataClaim>,
    overlay_ids: Vec<String>,
    owned_tiles: BTreeSet<usize>,
}

struct DynamicOverlayFamilyBuild {
    member_patches: BTreeMap<usize, DynamicOverlayMemberPatch>,
    report: PracticalDynamicOverlayBuildReport,
}

struct MemberSupplementalPatches<'a> {
    gameplay_prompt: Option<&'a GameplayPromptMemberPatch>,
    dynamic_overlay: Option<&'a DynamicOverlayMemberPatch>,
    battle_subject_title: Option<&'a BattleSubjectTitleMemberPatch>,
}

#[derive(Clone)]
struct RemainingHitsCounterLayout {
    prefix_tiles: Vec<usize>,
    digit_tiles: Vec<usize>,
    suffix_tiles: Vec<usize>,
}

struct SourcePresentationPreview {
    file: String,
    sha256: String,
}

struct PrimaryClutPalette {
    vram_x: usize,
    vram_y: usize,
    bytes: Vec<u8>,
}

pub fn build_practical_instruction_graphics(
    config: &PracticalInstructionGraphicsBuildConfig,
) -> Result<PracticalInstructionGraphicsBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_practical_instruction_graphics_from_source(config, &source)
}

pub(crate) fn build_practical_instruction_graphics_from_source(
    config: &PracticalInstructionGraphicsBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<PracticalInstructionGraphicsBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let (manifest, manifest_sha256) = load_manifest(&config.assets)?;
    ensure!(
        manifest.source_record_path == PRACTICAL_INSTRUCTION_PATH,
        "practical-instruction manifest targets an unexpected source record"
    );
    ensure!(
        !manifest.continuity_group_id.trim().is_empty(),
        "practical-instruction continuity group is empty"
    );

    let (_, archive) = source.read_record(PRACTICAL_INSTRUCTION_PATH)?;
    ensure!(
        sha256_bytes(&archive) == manifest.source_record_sha256,
        "TESTMJ.TIZ source identity changed"
    );
    let source_members = parse_tzz(&archive)?;
    validate_member_bindings(&manifest.members, source_members.len())?;
    let gameplay_prompt_build =
        build_gameplay_prompt_family(config, source, &manifest.members, &archive, &source_members)?;
    let mut dynamic_overlay_build =
        build_dynamic_overlay_family(config, source, &manifest.members, &archive, &source_members)?;
    let (attempt_patch, attempt_report) =
        attempt_counter::build(config, source, &manifest.members, &archive, &source_members)?;
    ensure!(
        dynamic_overlay_build
            .member_patches
            .insert(super::counter_layout::MEMBER, attempt_patch,)
            .is_none(),
        "attempt counter shares a member with another dynamic overlay"
    );
    dynamic_overlay_build.report.localized_overlay_count += 1;
    dynamic_overlay_build.report.attempt_counter = Some(attempt_report);
    let battle_subject_title_build = build_battle_subject_title_family(
        config,
        source,
        &manifest.members,
        &archive,
        &source_members,
    )?;
    for (member_index, dynamic_patch) in &dynamic_overlay_build.member_patches {
        if let Some(prompt_patch) = gameplay_prompt_build.member_patches.get(member_index) {
            let prompt_tiles = prompt_patch
                .tile_allocations
                .iter()
                .flat_map(|(start, count)| *start..*start + *count)
                .collect::<BTreeSet<_>>();
            ensure!(
                dynamic_patch.owned_tiles.is_disjoint(&prompt_tiles),
                "practical member {member_index} reuses a tile for a gameplay prompt and dynamic overlay"
            );
        }
    }
    validate_battle_subject_title_ownership(
        &battle_subject_title_build,
        &gameplay_prompt_build,
        &dynamic_overlay_build,
        &manifest.members,
    )?;

    let mut container = ContainerPlan::new(
        PRACTICAL_INSTRUCTION_PATH,
        "practical-instruction-graphics-builder",
        "source-bound Korean practical instruction panels",
        &archive,
        &manifest.source_record_sha256,
        source_members
            .iter()
            .map(|member| ContainerMemberSpec {
                id: member_id(member.index),
                writable_range: member.compressed_range(),
                slot_range: member.slot_range(),
                slot_start_alignment: 0x800,
                expected_source_sha256: sha256_bytes(&archive[member.compressed_range()]),
            })
            .collect(),
    )?;

    let mut authored = BTreeMap::new();
    let mut source_presentation_previews = BTreeMap::new();
    for binding in &manifest.members {
        let source_member = source_members
            .get(binding.member_index)
            .context("practical-instruction source member disappeared")?;
        let compressed_source = &archive[source_member.compressed_range()];
        let decoded_source = decompress(compressed_source, true)?;
        ensure!(
            sha256_bytes(&decoded_source) == binding.source_decoded_sha256,
            "practical-instruction source member {} identity changed",
            binding.member_index
        );
        if let Some(layout) = binding.presentation_layout.as_ref() {
            validate_presentation_layout(layout, &decoded_source)?;
            let preview = write_source_presentation_preview(
                config,
                binding.member_index,
                layout,
                &decoded_source,
            )?;
            ensure!(
                source_presentation_previews
                    .insert(binding.member_index, preview)
                    .is_none(),
                "practical-instruction member has two source presentation previews"
            );
        }
        if binding.translation_state == TranslationState::Pending {
            continue;
        }
        let translation_path = binding.translation.as_ref().with_context(|| {
            format!(
                "authored practical-instruction member {} has no translation asset",
                binding.member_index
            )
        })?;
        let (translation, translation_asset_sha256) =
            load_translation(&config.assets, translation_path, binding)?;
        let authored_member = build_authored_member(
            config,
            binding,
            &translation,
            &translation_asset_sha256,
            &decoded_source,
            compressed_source,
            MemberSupplementalPatches {
                gameplay_prompt: gameplay_prompt_build
                    .member_patches
                    .get(&binding.member_index),
                dynamic_overlay: dynamic_overlay_build
                    .member_patches
                    .get(&binding.member_index),
                battle_subject_title: battle_subject_title_build
                    .member_patches
                    .get(&binding.member_index),
            },
        )?;
        container.register_member(MemberContribution {
            member_id: &member_id(binding.member_index),
            owner: &binding.asset_unit_id,
            purpose: "replace one source-bound practical instruction panel",
            consumed_source_sha256: &sha256_bytes(compressed_source),
            candidate: &authored_member.compressed,
            lineage_id: &binding.asset_unit_id,
        })?;
        ensure!(
            authored
                .insert(binding.member_index, authored_member)
                .is_none(),
            "practical-instruction member has two authored outputs"
        );
    }
    ensure!(
        !authored.is_empty(),
        "practical-instruction build has no authored member"
    );

    let output = container
        .seal()?
        .context("practical-instruction container produced no change")?;
    ensure!(
        output.path == PRACTICAL_INSTRUCTION_PATH
            && output.owner == "practical-instruction-graphics-builder",
        "practical-instruction container ownership changed"
    );
    let patched_members = parse_tzz(&output.data)?;
    ensure!(
        patched_members == source_members,
        "TESTMJ.TIZ member table changed"
    );

    let mut reports = Vec::with_capacity(source_members.len());
    let mut all_unowned_members_preserved = true;
    let mut primary_clut_vram_coordinates = None;
    let mut source_primary_clut_palette_family = Vec::with_capacity(source_members.len() * 32);
    let mut patched_primary_clut_palette_family = Vec::with_capacity(source_members.len() * 32);
    for binding in &manifest.members {
        let source_member = source_members[binding.member_index];
        let patched_member = patched_members[binding.member_index];
        let source_compressed = &archive[source_member.compressed_range()];
        let patched_compressed = &output.data[patched_member.compressed_range()];
        let source_decoded = decompress(source_compressed, true)?;
        let patched_decoded = decompress(patched_compressed, true)?;
        let source_primary_clut = primary_clut_palette(&source_decoded)?;
        let patched_primary_clut = primary_clut_palette(&patched_decoded)?;
        let coordinates = (source_primary_clut.vram_x, source_primary_clut.vram_y);
        if let Some(expected) = primary_clut_vram_coordinates {
            ensure!(
                coordinates == expected,
                "practical-instruction primary CLUT coordinates differ between members"
            );
        } else {
            primary_clut_vram_coordinates = Some(coordinates);
        }
        ensure!(
            (patched_primary_clut.vram_x, patched_primary_clut.vram_y) == coordinates
                && patched_primary_clut.bytes == source_primary_clut.bytes,
            "practical-instruction member {} changed its primary CLUT",
            binding.member_index
        );
        let source_primary_clut_palette_sha256 = sha256_bytes(&source_primary_clut.bytes);
        let patched_primary_clut_palette_sha256 = sha256_bytes(&patched_primary_clut.bytes);
        source_primary_clut_palette_family.extend_from_slice(&source_primary_clut.bytes);
        patched_primary_clut_palette_family.extend_from_slice(&patched_primary_clut.bytes);
        let source_palette_roles = binding
            .presentation_layout
            .as_ref()
            .map(|_| source_presentation_palette_roles(&source_decoded))
            .transpose()?;
        let text_palette_indices = binding
            .presentation_layout
            .as_ref()
            .zip(source_palette_roles.as_ref())
            .map(|(layout, roles)| resolved_presentation_text_palette_indices(layout, roles))
            .transpose()?
            .unwrap_or_default();
        let authored_member = authored.get(&binding.member_index);
        let source_presentation_preview = source_presentation_previews.get(&binding.member_index);
        if let Some(authored_member) = authored_member {
            ensure!(
                patched_decoded == authored_member.decoded,
                "authored practical-instruction member {} failed readback",
                binding.member_index
            );
        } else {
            all_unowned_members_preserved &=
                source_compressed == patched_compressed && source_decoded == patched_decoded;
        }
        reports.push(PracticalInstructionMemberBuildReport {
            asset_unit_id: binding.asset_unit_id.clone(),
            title_semantic_id: binding.title_semantic_id.clone(),
            member_index: binding.member_index,
            translation_state: binding.translation_state,
            human_approval: authored_member.map(|member| member.human_approval),
            source_decoded_sha256: sha256_bytes(&source_decoded),
            patched_decoded_sha256: sha256_bytes(&patched_decoded),
            primary_clut_vram_x: coordinates.0,
            primary_clut_vram_y: coordinates.1,
            source_primary_clut_palette_sha256,
            patched_primary_clut_palette_sha256,
            primary_clut_palette_preserved: true,
            source_compressed_sha256: sha256_bytes(source_compressed),
            patched_compressed_sha256: sha256_bytes(patched_compressed),
            source_compressed_size: source_member.compressed_size,
            translation_asset_sha256: authored_member
                .map(|member| member.translation_asset_sha256.clone()),
            reencoded_size: authored_member.map(|member| member.reencoded_size),
            compression_headroom: authored_member
                .map(|member| source_member.compressed_size - member.reencoded_size),
            source_compression_maximum_match_words: authored_member
                .map(|member| member.source_compression_maximum_match_words),
            source_compression_maximum_control_block_output_words: authored_member
                .map(|member| member.source_compression_maximum_control_block_output_words),
            source_compression_control_blocks_crossing_input_pages: authored_member
                .map(|member| member.source_compression_control_blocks_crossing_input_pages),
            rebuilt_compression_maximum_match_words: authored_member
                .map(|member| member.rebuilt_compression_maximum_match_words),
            rebuilt_compression_maximum_control_block_output_words: authored_member
                .map(|member| member.rebuilt_compression_maximum_control_block_output_words),
            rebuilt_compression_control_blocks_crossing_input_pages: authored_member
                .map(|member| member.rebuilt_compression_control_blocks_crossing_input_pages),
            changed_decoded_byte_count: authored_member
                .map_or(0, |member| member.changed_decoded_byte_count),
            korean_lines: authored_member
                .map_or_else(Vec::new, |member| member.korean_lines.clone()),
            source_transparent_palette_index: source_palette_roles
                .as_ref()
                .map(|roles| roles.background_index),
            source_transparent_palette_indices: source_palette_roles
                .map(|roles| roles.transparent_indices.into_iter().collect())
                .unwrap_or_default(),
            text_palette_indices,
            preserved_graphics: authored_member
                .map_or_else(Vec::new, |member| member.preserved_graphics.clone()),
            gameplay_prompt_localized: authored_member
                .is_some_and(|member| !member.gameplay_prompt_action_ids.is_empty()),
            gameplay_prompt_action_ids: authored_member
                .map_or_else(Vec::new, |member| member.gameplay_prompt_action_ids.clone()),
            gameplay_prompt_korean_labels: authored_member.map_or_else(Vec::new, |member| {
                member.gameplay_prompt_korean_labels.clone()
            }),
            gameplay_prompt_preview_file: authored_member
                .and_then(|member| member.gameplay_prompt_preview_file.clone()),
            gameplay_prompt_preview_sha256: authored_member
                .and_then(|member| member.gameplay_prompt_preview_sha256.clone()),
            dynamic_overlay_ids: authored_member
                .map_or_else(Vec::new, |member| member.dynamic_overlay_ids.clone()),
            source_presentation_preview_file: source_presentation_preview
                .map(|preview| preview.file.clone()),
            source_presentation_preview_sha256: source_presentation_preview
                .map(|preview| preview.sha256.clone()),
            preview_file: authored_member.map(|member| member.preview_file.clone()),
            preview_sha256: authored_member.map(|member| member.preview_sha256.clone()),
            atlas_preview_file: authored_member.map(|member| member.atlas_preview_file.clone()),
            atlas_preview_sha256: authored_member.map(|member| member.atlas_preview_sha256.clone()),
        });
    }
    ensure!(
        all_unowned_members_preserved,
        "unowned practical-instruction member changed"
    );
    ensure!(
        output.report.members.len() == source_members.len() && output.report.changed_byte_count > 0,
        "practical-instruction container report denominator changed"
    );

    let output_record = config.output_dir.join(OUTPUT_RECORD_FILE);
    std::fs::write(&output_record, &output.data)
        .with_context(|| format!("failed to write {}", output_record.display()))?;
    for consumer in &gameplay_prompt_build.consumers {
        let file_name = Path::new(&consumer.path)
            .file_name()
            .context("gameplay-prompt consumer path has no file name")?;
        let path = config.output_dir.join(file_name);
        std::fs::write(&path, &consumer.record)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    let battle_subject_consumer_path = config.output_dir.join(
        Path::new(&battle_subject_title_build.consumer.path)
            .file_name()
            .context("battle subject-title consumer path has no file name")?,
    );
    std::fs::write(
        &battle_subject_consumer_path,
        &battle_subject_title_build.consumer.record,
    )
    .with_context(|| format!("failed to write {}", battle_subject_consumer_path.display()))?;
    let authored_member_count = authored.len();
    let pending_member_count = source_members.len() - authored_member_count;
    let (primary_clut_vram_x, primary_clut_vram_y) =
        primary_clut_vram_coordinates.context("practical-instruction build has no primary CLUT")?;
    let source_primary_clut_palette_family_sha256 =
        sha256_bytes(&source_primary_clut_palette_family);
    let patched_primary_clut_palette_family_sha256 =
        sha256_bytes(&patched_primary_clut_palette_family);
    ensure!(
        source_primary_clut_palette_family_sha256 == patched_primary_clut_palette_family_sha256,
        "practical-instruction primary CLUT family changed"
    );
    let report = PracticalInstructionGraphicsBuildReport {
        kind: "justice_gakuen2_practical_instruction_graphics_build".to_string(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_record_path: PRACTICAL_INSTRUCTION_PATH.to_string(),
        source_record_size: archive.len(),
        source_record_sha256: sha256_bytes(&archive),
        patched_record_sha256: sha256_bytes(&output.data),
        manifest_sha256,
        member_count: source_members.len(),
        authored_member_count,
        pending_member_count,
        all_source_members_bound: true,
        all_unowned_members_preserved,
        member_table_preserved: true,
        compression_roundtrip_verified: true,
        primary_clut_member_count: reports.len(),
        primary_clut_vram_x,
        primary_clut_vram_y,
        source_primary_clut_palette_family_sha256,
        patched_primary_clut_palette_family_sha256,
        all_primary_clut_palettes_preserved: true,
        complete_scope: pending_member_count == 0,
        release_candidate_input_eligible: false,
        output_record_file: OUTPUT_RECORD_FILE.to_string(),
        gameplay_prompts: gameplay_prompt_build.report,
        dynamic_overlays: dynamic_overlay_build.report,
        battle_subject_titles: battle_subject_title_build.report,
        members: reports,
    };
    let build_manifest_sha256 =
        write_pretty_json_and_hash(&config.output_dir.join(OUTPUT_REPORT_FILE), &report, true)?;
    Ok(PracticalInstructionGraphicsBuild {
        record: output.data,
        prompt_consumers: gameplay_prompt_build.consumers,
        battle_subject_title_consumer: battle_subject_title_build.consumer,
        build_manifest_sha256,
        report,
    })
}

fn primary_clut_palette(decoded: &[u8]) -> Result<PrimaryClutPalette> {
    let tim =
        parse_4bpp_prefix(decoded).context("practical-instruction member has no primary TIM")?;
    ensure!(
        tim.clut_width * tim.clut_height == 16,
        "practical-instruction primary TIM no longer has one 16-color palette"
    );
    let bytes = read_4bpp_palette_words_in_prefix(decoded, 0, 0)?
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect();
    Ok(PrimaryClutPalette {
        vram_x: usize::from(tim.clut_x),
        vram_y: usize::from(tim.clut_y),
        bytes,
    })
}

fn build_authored_member(
    config: &PracticalInstructionGraphicsBuildConfig,
    binding: &PracticalInstructionMemberBinding,
    translation: &PracticalInstructionTranslation,
    translation_asset_sha256: &str,
    source_decoded: &[u8],
    compressed_source: &[u8],
    supplemental: MemberSupplementalPatches<'_>,
) -> Result<AuthoredMember> {
    let layout = binding.presentation_layout.as_ref().with_context(|| {
        format!(
            "authored practical-instruction member {} has no presentation layout",
            binding.member_index
        )
    })?;
    validate_translation_layout(translation, layout)?;
    let mut candidate = source_decoded.to_vec();
    let mut allowed_ranges = Vec::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let body_rasterizer = rasterizers.for_font(&config.body_font.path)?;
    let presentation = render_presentation(
        body_rasterizer,
        &config.body_font,
        layout,
        translation,
        source_decoded,
    )?;
    for binding in presentation_tile_bindings(layout) {
        let pixels = read_indexed_cell(&presentation, binding.presentation_cell)?;
        allowed_ranges.extend(
            write_indexed_cell_in_prefix_with_report(
                &mut candidate,
                0,
                binding.source_cell,
                &pixels,
            )?
            .allowed_ranges,
        );
    }

    let source_sha256 = sha256_bytes(source_decoded);
    let claims = DecodedDataClaim::from_effective_ranges(
        &format!("{}:text", binding.asset_unit_id),
        "clear the owned Japanese text region and render Korean title and instructions",
        source_decoded,
        &candidate,
        allowed_ranges,
    )?;
    ensure!(
        !claims.is_empty(),
        "practical-instruction authored member changes no decoded bytes"
    );
    let mut plan =
        DecodedRecordWritePlan::new(PRACTICAL_INSTRUCTION_PATH, source_decoded, &source_sha256)?;
    plan.register_data_candidate(&binding.asset_unit_id, &source_sha256, &candidate, &claims)?;
    if let Some(prompt_patch) = supplemental.gameplay_prompt {
        plan.register_data_candidate(
            &format!("{}:gameplay-prompts", binding.asset_unit_id),
            &source_sha256,
            &prompt_patch.candidate,
            &prompt_patch.claims,
        )?;
    }
    if let Some(overlay_patch) = supplemental.dynamic_overlay {
        plan.register_data_candidate(
            &format!("{}:dynamic-overlays", binding.asset_unit_id),
            &source_sha256,
            &overlay_patch.candidate,
            &overlay_patch.claims,
        )?;
    }
    if let Some(title_patch) = supplemental.battle_subject_title {
        plan.register_data_candidate(
            &format!("{}:battle-subject-title", binding.asset_unit_id),
            &source_sha256,
            &title_patch.candidate,
            &title_patch.claims,
        )?;
    }
    let decoded = plan.apply(None)?;
    ensure!(
        claims
            .iter()
            .all(|claim| decoded[claim.range.clone()] == candidate[claim.range.clone()]),
        "practical-instruction decoded write plan omitted a text contribution"
    );
    if let Some(prompt_patch) = supplemental.gameplay_prompt {
        ensure!(
            prompt_patch.claims.iter().all(|claim| {
                decoded[claim.range.clone()] == prompt_patch.candidate[claim.range.clone()]
            }),
            "practical-instruction decoded write plan omitted a gameplay-prompt contribution"
        );
    }
    if let Some(overlay_patch) = supplemental.dynamic_overlay {
        ensure!(
            overlay_patch.claims.iter().all(|claim| {
                decoded[claim.range.clone()] == overlay_patch.candidate[claim.range.clone()]
            }),
            "practical-instruction decoded write plan omitted a dynamic-overlay contribution"
        );
    }
    if let Some(title_patch) = supplemental.battle_subject_title {
        ensure!(
            title_patch.claims.iter().all(|claim| {
                decoded[claim.range.clone()] == title_patch.candidate[claim.range.clone()]
            }),
            "practical-instruction decoded write plan omitted a battle subject-title contribution"
        );
    }
    let changed_decoded_byte_count = difference_ranges(source_decoded, &decoded)
        .iter()
        .map(|[start, end]| end - start)
        .sum();

    let source_compression = source_paged_compression_profile(compressed_source)?;
    let reencoded = compress_page_safe_image(&decoded, source_compression)?;
    ensure!(
        reencoded.len() <= compressed_source.len(),
        "practical-instruction member {} exceeds its source compressed extent",
        binding.member_index
    );
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "practical-instruction member {} failed unpadded compression roundtrip",
        binding.member_index
    );
    let rebuilt_compression = profile_paged_compression(&reencoded)?;
    let reencoded_size = reencoded.len();
    let mut compressed = reencoded;
    compressed.resize(compressed_source.len(), 0);
    ensure!(
        decompress(&compressed, true)? == decoded,
        "practical-instruction member {} failed padded compression roundtrip",
        binding.member_index
    );

    let preview_dir = config.output_dir.join("previews");
    std::fs::create_dir_all(&preview_dir)?;
    let palette = read_4bpp_palette_words_in_prefix(&decoded, 0, 0)?;
    let presentation_rgba = indexed_to_rgba(&presentation, &palette);
    let tim = detect_embedded_tim_images(&decoded)
        .into_iter()
        .next()
        .context("authored practical-instruction member lost its TIM")?;
    let preview_file = format!(
        "previews/member-{:03}-presentation.png",
        binding.member_index
    );
    let preview_path = config.output_dir.join(&preview_file);
    write_tim_preview(&preview_path, &presentation_rgba)?;
    let preview_sha256 = sha256_file(&preview_path)?;
    let atlas_rgba = decode_embedded_tim_preview(&decoded, &tim)?;
    let atlas_preview_file = format!("previews/member-{:03}-atlas.png", binding.member_index);
    let atlas_preview_path = config.output_dir.join(&atlas_preview_file);
    write_tim_preview(&atlas_preview_path, &atlas_rgba)?;
    let atlas_preview_sha256 = sha256_file(&atlas_preview_path)?;
    let preserved_graphics = write_preserved_graphic_previews(
        config,
        binding.member_index,
        &translation.preserved_graphics,
        source_decoded,
    )?;
    let (gameplay_prompt_preview_file, gameplay_prompt_preview_sha256) =
        if let Some(prompt_patch) = supplemental.gameplay_prompt {
            let preview = unpack_gameplay_prompt_preview(&decoded, &prompt_patch.tile_allocations)?;
            let rgba = indexed_to_rgba(&preview, &palette);
            let file = format!(
                "previews/member-{:03}-gameplay-prompts.png",
                binding.member_index
            );
            let path = config.output_dir.join(&file);
            write_tim_preview(&path, &rgba)?;
            (Some(file), Some(sha256_file(&path)?))
        } else {
            (None, None)
        };

    Ok(AuthoredMember {
        compressed,
        decoded,
        reencoded_size,
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        source_compression_control_blocks_crossing_input_pages: source_compression
            .control_blocks_crossing_input_pages,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression
            .control_blocks_crossing_input_pages,
        changed_decoded_byte_count,
        korean_lines: translation.korean_lines.clone(),
        preserved_graphics,
        human_approval: translation.human_approval,
        translation_asset_sha256: translation_asset_sha256.to_string(),
        preview_file,
        preview_sha256,
        atlas_preview_file,
        atlas_preview_sha256,
        gameplay_prompt_action_ids: supplemental
            .gameplay_prompt
            .map_or_else(Vec::new, |patch| patch.action_ids.clone()),
        gameplay_prompt_korean_labels: supplemental
            .gameplay_prompt
            .map_or_else(Vec::new, |patch| patch.korean_labels.clone()),
        gameplay_prompt_preview_file,
        gameplay_prompt_preview_sha256,
        dynamic_overlay_ids: supplemental
            .dynamic_overlay
            .map_or_else(Vec::new, |patch| patch.overlay_ids.clone()),
    })
}

fn unpack_gameplay_prompt_preview(
    decoded: &[u8],
    allocations: &[(usize, usize)],
) -> Result<IndexedPresentation> {
    ensure!(
        !allocations.is_empty(),
        "gameplay-prompt preview has no allocated records"
    );
    let width = allocations
        .iter()
        .map(|(_, tile_count)| tile_count * TILE_WIDTH)
        .max()
        .context("gameplay-prompt preview has no width")?;
    let transparent_index = source_presentation_palette_roles(decoded)?.background_index;
    let mut preview = IndexedPresentation {
        width,
        height: allocations.len() * TILE_HEIGHT,
        pixels: vec![transparent_index; width * allocations.len() * TILE_HEIGHT],
    };
    for (row, &(start_tile, tile_count)) in allocations.iter().enumerate() {
        for column in 0..tile_count {
            let pixels = read_indexed_cell_in_prefix(decoded, 0, tile_cell(start_tile + column))?;
            write_indexed_cell(
                &mut preview,
                Cell {
                    x: column * TILE_WIDTH,
                    y: row * TILE_HEIGHT,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
                &pixels,
            )?;
        }
    }
    Ok(preview)
}

fn write_source_gameplay_prompt_preview(
    config: &PracticalInstructionGraphicsBuildConfig,
    member_index: usize,
    record_ordinal: usize,
    record: &GameplayPromptRecord,
    source_decoded: &[u8],
) -> Result<SourcePresentationPreview> {
    let mut lines = vec![Vec::<usize>::new()];
    for fragment in &record.fragments {
        if fragment.new_line && !lines.last().is_some_and(Vec::is_empty) {
            lines.push(Vec::new());
        }
        lines
            .last_mut()
            .context("source gameplay-prompt preview lost its current line")?
            .extend(fragment.start_tile..fragment.start_tile + fragment.tile_count);
    }
    ensure!(
        lines.iter().all(|line| !line.is_empty()),
        "source gameplay-prompt preview contains an empty line"
    );
    let width = lines
        .iter()
        .map(|line| line.len() * TILE_WIDTH)
        .max()
        .context("source gameplay-prompt preview has no width")?;
    let transparent_index = source_presentation_palette_roles(source_decoded)?.background_index;
    let mut preview = IndexedPresentation {
        width,
        height: lines.len() * TILE_HEIGHT,
        pixels: vec![transparent_index; width * lines.len() * TILE_HEIGHT],
    };
    for (row, line) in lines.iter().enumerate() {
        for (column, &source_tile) in line.iter().enumerate() {
            let pixels = read_indexed_cell_in_prefix(source_decoded, 0, tile_cell(source_tile))?;
            write_indexed_cell(
                &mut preview,
                Cell {
                    x: column * TILE_WIDTH,
                    y: row * TILE_HEIGHT,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
                &pixels,
            )?;
        }
    }
    let palette = read_4bpp_palette_words_in_prefix(source_decoded, 0, 0)?;
    let rgba = indexed_to_rgba(&preview, &palette);
    let file = format!(
        "previews/member-{member_index:03}-record-{record_ordinal:03}-action-{:03}-source-gameplay-prompt.png",
        record.action_id
    );
    let path = config.output_dir.join(&file);
    std::fs::create_dir_all(
        path.parent()
            .context("source gameplay-prompt preview path has no parent")?,
    )?;
    write_tim_preview(&path, &rgba)?;
    Ok(SourcePresentationPreview {
        file,
        sha256: sha256_file(&path)?,
    })
}

fn build_dynamic_overlay_family(
    config: &PracticalInstructionGraphicsBuildConfig,
    source: &SupportedSourceDisc,
    bindings: &[PracticalInstructionMemberBinding],
    archive: &[u8],
    source_members: &[TzzMember],
) -> Result<DynamicOverlayFamilyBuild> {
    const MANIFEST_KIND: &str = "justice_gakuen2_practical_dynamic_overlay_translations";
    let manifest_path = config.assets.join(DYNAMIC_OVERLAYS_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: DynamicOverlayManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND && !manifest.entries.is_empty(),
        "practical dynamic-overlay manifest has an unexpected or empty scope"
    );

    let mut ids = BTreeSet::new();
    let mut producer_members = BTreeSet::new();
    let mut member_patches = BTreeMap::new();
    let mut reports = Vec::with_capacity(manifest.entries.len());
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&config.body_font.path)?;
    for entry in &manifest.entries {
        ensure!(
            !entry.id.is_empty()
                && entry
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
                && ids.insert(entry.id.as_str()),
            "practical dynamic-overlay id is invalid or duplicated: {}",
            entry.id
        );
        ensure!(
            producer_members.insert(entry.producer_member_index),
            "practical dynamic-overlay entries currently require one owner per producer member"
        );
        ensure!(
            entry.sample_count <= 30
                && is_sha256(&entry.source_consumer_sha256)
                && bindings
                    .get(entry.producer_member_index)
                    .is_some_and(|binding| {
                        binding.member_index == entry.producer_member_index
                            && binding.translation_state == TranslationState::Authored
                    }),
            "practical dynamic-overlay {} has an invalid source or producer binding",
            entry.id
        );
        let (source_prefix, source_suffix) = split_counter_pattern(&entry.source_text)?;
        let (korean_prefix, korean_suffix) = split_counter_pattern(&entry.korean_text)?;
        ensure!(
            !source_prefix.trim().is_empty()
                && !source_suffix.trim().is_empty()
                && !korean_prefix.trim().is_empty()
                && !korean_suffix.trim().is_empty(),
            "practical dynamic-overlay {} must retain text on both sides of {{count}}",
            entry.id
        );

        let (_, consumer) = source.read_record(&entry.consumer_path)?;
        ensure!(
            sha256_bytes(&consumer) == entry.source_consumer_sha256,
            "practical dynamic-overlay consumer {} source identity changed",
            entry.consumer_path
        );
        let layout = audit_remaining_hits_counter_consumer(&consumer, entry.consumer_runtime_base)?;
        let member = source_members
            .get(entry.producer_member_index)
            .context("practical dynamic-overlay producer member disappeared")?;
        let compressed = archive
            .get(member.compressed_range())
            .context("practical dynamic-overlay producer extent disappeared")?;
        let source_decoded = decompress(compressed, true)?;
        let tim = detect_embedded_tim_images(&source_decoded)
            .into_iter()
            .next()
            .context("practical dynamic-overlay producer has no TIM")?;
        ensure!(
            tim.offset == 0
                && tim.bits_per_pixel == 4
                && tim.pixel_width == 256
                && tim.image_vram_word_x == 896
                && tim.image_vram_y == 256
                && tim.clut_vram_y == 503,
            "practical dynamic-overlay producer no longer matches the counter texture page"
        );
        let owned_tiles = layout
            .prefix_tiles
            .iter()
            .chain(&layout.suffix_tiles)
            .chain(&layout.digit_tiles)
            .copied()
            .collect::<BTreeSet<_>>();
        let panel_tiles = bindings[entry.producer_member_index]
            .presentation_layout
            .as_ref()
            .context("practical dynamic-overlay producer has no panel layout")?
            .source_tile_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        ensure!(
            owned_tiles.is_disjoint(&panel_tiles),
            "practical dynamic-overlay {} overlaps its instruction panel",
            entry.id
        );

        let panel_layout = bindings[entry.producer_member_index]
            .presentation_layout
            .as_ref()
            .expect("checked panel layout");
        let palette = read_4bpp_palette_words_in_prefix(&source_decoded, 0, 0)?;
        let palette_roles = source_presentation_palette_roles(&source_decoded)?;
        let transparent_index = palette_roles.background_index;
        let text_palette_indices =
            white_panel_text_palette_indices(panel_layout, &palette_roles, &source_decoded)?;
        let prefix_pixels = render_dynamic_overlay_segment(
            rasterizer,
            &config.body_font,
            korean_prefix,
            layout.prefix_tiles.len() * TILE_WIDTH,
            &text_palette_indices,
            transparent_index,
            HorizontalTextAlignment::Right,
        )?;
        let suffix_pixels = render_dynamic_overlay_segment(
            rasterizer,
            &config.body_font,
            korean_suffix,
            layout.suffix_tiles.len() * TILE_WIDTH,
            &text_palette_indices,
            transparent_index,
            HorizontalTextAlignment::Left,
        )?;

        let mut candidate = source_decoded.clone();
        let mut allowed_ranges = Vec::new();
        write_dynamic_overlay_tiles(
            &mut candidate,
            &layout.prefix_tiles,
            &prefix_pixels,
            &mut allowed_ranges,
        )?;
        write_dynamic_overlay_tiles(
            &mut candidate,
            &layout.suffix_tiles,
            &suffix_pixels,
            &mut allowed_ranges,
        )?;
        // The counter owns a complete decimal alphabet. Render it with the
        // same font, size and palette as its surrounding words, preserving
        // every UV, source counter value and numeric-selection instruction.
        for (digit, &tile) in layout.digit_tiles.iter().enumerate() {
            let pixels = render_dynamic_overlay_segment(
                rasterizer,
                &config.body_font,
                &digit.to_string(),
                TILE_WIDTH,
                &text_palette_indices,
                transparent_index,
                HorizontalTextAlignment::Center,
            )?;
            write_dynamic_overlay_tiles(&mut candidate, &[tile], &pixels, &mut allowed_ranges)?;
            ensure!(
                read_indexed_cell_in_prefix(&candidate, 0, tile_cell(tile))? == pixels,
                "remaining-hits decimal glyph readback differs"
            );
        }
        let claims = DecodedDataClaim::from_effective_ranges(
            &format!("practical-dynamic-overlay:{}", entry.id),
            "regenerate the complete counter alphabet while preserving native values, UVs and selection",
            &source_decoded,
            &candidate,
            allowed_ranges,
        )?;
        ensure!(
            !claims.is_empty(),
            "practical dynamic-overlay {} changed no producer bytes",
            entry.id
        );
        let changed_decoded_byte_count = claims.iter().map(|claim| claim.range.len()).sum();
        let preview = unpack_dynamic_counter_preview(&candidate, &layout, entry.sample_count)?;
        let preview_file = format!(
            "previews/member-{:03}-dynamic-overlay-{}.png",
            entry.producer_member_index, entry.id
        );
        let preview_path = config.output_dir.join(&preview_file);
        write_tim_preview(&preview_path, &indexed_to_rgba(&preview, &palette))?;
        let preview_sha256 = sha256_file(&preview_path)?;

        ensure!(
            member_patches
                .insert(
                    entry.producer_member_index,
                    DynamicOverlayMemberPatch {
                        candidate,
                        claims,
                        overlay_ids: vec![entry.id.clone()],
                        owned_tiles,
                    },
                )
                .is_none(),
            "practical dynamic-overlay producer patch was duplicated"
        );
        reports.push(PracticalDynamicOverlayReport {
            id: entry.id.clone(),
            producer_member_index: entry.producer_member_index,
            consumer_path: entry.consumer_path.clone(),
            consumer_sha256: entry.source_consumer_sha256.clone(),
            packet_builder_offset: "0x6500".to_string(),
            counter_formula: "30 - state[0x4d]".to_string(),
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            human_approval: entry.human_approval,
            prefix_tiles: layout.prefix_tiles,
            digit_tiles: layout.digit_tiles,
            suffix_tiles: layout.suffix_tiles,
            dynamic_digits_regenerated: true,
            changed_decoded_byte_count,
            preview_file,
            preview_sha256,
        });
    }

    Ok(DynamicOverlayFamilyBuild {
        member_patches,
        report: PracticalDynamicOverlayBuildReport {
            attempt_counter: None,
            translation_manifest_sha256: sha256_bytes(&manifest_bytes),
            localized_overlay_count: reports.len(),
            all_changes_confined_to_consumer_bound_tiles: true,
            overlays: reports,
        },
    })
}

fn audit_remaining_hits_counter_consumer(
    consumer: &[u8],
    runtime_base: u32,
) -> Result<RemainingHitsCounterLayout> {
    const TABLE_START: usize = 0x0ce0;
    const TABLE_END: usize = 0x0d03;
    const TABLE_SHA256: &str = "fcab49bb482df01af58d90b226070cef28d2eca6223bb1289548fd3c14c8dec0";
    ensure!(
        runtime_base == 0x800a_2000
            && consumer
                .get(TABLE_START..TABLE_END)
                .is_some_and(|table| sha256_bytes(table) == TABLE_SHA256),
        "remaining-hits counter source table changed"
    );
    let expected = [
        (
            0x6510,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x800a,
            },
        ),
        (
            0x6514,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 0x2cf4,
            },
        ),
        (
            0x654c,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0380,
            },
        ),
        (
            0x6550,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::ZERO,
                immediate: 0x0100,
            },
        ),
        (
            0x66b4,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x004d,
            },
        ),
        (
            0x66b8,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 30,
            },
        ),
        (
            0x66bc,
            Instruction::Subu {
                rd: Register::A0,
                rs: Register::A0,
                rt: Register::V0,
            },
        ),
        (
            0x675c,
            Instruction::Lui {
                rt: Register::T0,
                immediate: 0x800a,
            },
        ),
        (
            0x6760,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 0x2ce0,
            },
        ),
        (
            0x687c,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x800a,
            },
        ),
        (
            0x6880,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 0x2cfe,
            },
        ),
    ];
    for (offset, instruction) in expected {
        let bytes = consumer
            .get(offset..offset + 4)
            .context("remaining-hits counter instruction span is truncated")?;
        let actual = decode(
            u32::from_le_bytes(bytes.try_into()?),
            runtime_base + u32::try_from(offset)?,
        )?;
        ensure!(
            actual == instruction,
            "remaining-hits counter instruction +0x{offset:04x} changed"
        );
    }

    let (digit_uvs, digit_remainder) = consumer[TABLE_START..TABLE_START + 20].as_chunks::<2>();
    ensure!(
        digit_remainder.is_empty(),
        "remaining-hits digit UV table is not pair aligned"
    );
    let digit_tiles = digit_uvs
        .iter()
        .map(|uv| counter_tile_from_uv(uv[0], uv[1]))
        .collect::<Result<Vec<_>>>()?;
    let mut prefix_tiles = Vec::new();
    for offset in [0x0cf4, 0x0cf9] {
        prefix_tiles.extend(counter_sprite_record_tiles(consumer, offset)?);
    }
    let suffix_tiles = counter_sprite_record_tiles(consumer, 0x0cfe)?;
    ensure!(
        prefix_tiles == [63, 64, 65]
            && digit_tiles == (66..=75).collect::<Vec<_>>()
            && suffix_tiles == [76],
        "remaining-hits counter tile ownership changed"
    );
    Ok(RemainingHitsCounterLayout {
        prefix_tiles,
        digit_tiles,
        suffix_tiles,
    })
}

fn counter_sprite_record_tiles(consumer: &[u8], offset: usize) -> Result<Vec<usize>> {
    let record = consumer
        .get(offset..offset + 5)
        .context("remaining-hits counter sprite record is truncated")?;
    let width = usize::from(record[2]);
    ensure!(
        width > 0 && width.is_multiple_of(TILE_WIDTH) && record[3] == TILE_HEIGHT as u8,
        "remaining-hits counter sprite geometry changed"
    );
    (0..width / TILE_WIDTH)
        .map(|column| {
            let u = usize::from(record[0]) + column * TILE_WIDTH;
            ensure!(
                u <= usize::from(u8::MAX),
                "counter sprite crosses its texture page"
            );
            counter_tile_from_uv(u as u8, record[1])
        })
        .collect()
}

fn counter_tile_from_uv(u: u8, v: u8) -> Result<usize> {
    ensure!(
        usize::from(u).is_multiple_of(TILE_WIDTH) && usize::from(v).is_multiple_of(TILE_HEIGHT),
        "remaining-hits counter UV is not tile aligned"
    );
    Ok(usize::from(v) / TILE_HEIGHT * SOURCE_TILE_COLUMNS + usize::from(u) / TILE_WIDTH)
}

fn split_counter_pattern(pattern: &str) -> Result<(&str, &str)> {
    let (prefix, suffix) = pattern
        .split_once("{count}")
        .context("dynamic counter text has no {count} placeholder")?;
    ensure!(
        !suffix.contains("{count}"),
        "dynamic counter text has more than one {{count}} placeholder"
    );
    Ok((prefix, suffix))
}

fn render_dynamic_overlay_segment(
    rasterizer: &IndexedTextRasterizer,
    font: &ShiftedSizedFontSource,
    text: &str,
    width: usize,
    text_palette_indices: &[u8],
    transparent_index: u8,
    alignment: HorizontalTextAlignment,
) -> Result<Vec<u8>> {
    let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
        text,
        width,
        TILE_HEIGHT,
        font.font_px,
        0.0,
        font.vertical_shift_px,
        0,
        1,
        u8::try_from(text_palette_indices.len())?,
        alignment,
    )?;
    ensure!(
        raster.measured_advance_px <= width as f32,
        "dynamic counter segment does not fit its consumer-bound cells: {text}"
    );
    raster
        .pixels
        .into_iter()
        .map(|pixel| {
            if pixel == 0 {
                Ok(transparent_index)
            } else {
                text_palette_indices
                    .get(usize::from(pixel - 1))
                    .copied()
                    .context("dynamic counter coverage index left its text palette")
            }
        })
        .collect()
}

fn write_dynamic_overlay_tiles(
    candidate: &mut [u8],
    tiles: &[usize],
    pixels: &[u8],
    allowed_ranges: &mut Vec<[usize; 2]>,
) -> Result<()> {
    ensure!(
        !tiles.is_empty() && pixels.len() == tiles.len() * TILE_WIDTH * TILE_HEIGHT,
        "dynamic overlay tile canvas changed"
    );
    let canvas_width = tiles.len() * TILE_WIDTH;
    for (column, &tile) in tiles.iter().enumerate() {
        let mut cell_pixels = Vec::with_capacity(TILE_WIDTH * TILE_HEIGHT);
        for y in 0..TILE_HEIGHT {
            let start = y * canvas_width + column * TILE_WIDTH;
            cell_pixels.extend_from_slice(&pixels[start..start + TILE_WIDTH]);
        }
        allowed_ranges.extend(
            write_indexed_cell_in_prefix_with_report(candidate, 0, tile_cell(tile), &cell_pixels)?
                .allowed_ranges,
        );
    }
    Ok(())
}

fn unpack_dynamic_counter_preview(
    decoded: &[u8],
    layout: &RemainingHitsCounterLayout,
    count: u8,
) -> Result<IndexedPresentation> {
    let mut tiles = layout.prefix_tiles.clone();
    if count >= 10 {
        tiles.push(layout.digit_tiles[usize::from(count / 10)]);
    }
    tiles.push(layout.digit_tiles[usize::from(count % 10)]);
    tiles.extend(&layout.suffix_tiles);
    let background_index = source_presentation_palette_roles(decoded)?.background_index;
    let mut preview = IndexedPresentation {
        width: tiles.len() * TILE_WIDTH,
        height: TILE_HEIGHT,
        pixels: vec![background_index; tiles.len() * TILE_WIDTH * TILE_HEIGHT],
    };
    for (column, tile) in tiles.into_iter().enumerate() {
        let pixels = read_indexed_cell_in_prefix(decoded, 0, tile_cell(tile))?;
        write_indexed_cell(
            &mut preview,
            Cell {
                x: column * TILE_WIDTH,
                y: 0,
                width: TILE_WIDTH,
                height: TILE_HEIGHT,
            },
            &pixels,
        )?;
    }
    Ok(preview)
}

fn build_gameplay_prompt_family(
    config: &PracticalInstructionGraphicsBuildConfig,
    source: &SupportedSourceDisc,
    bindings: &[PracticalInstructionMemberBinding],
    archive: &[u8],
    source_members: &[TzzMember],
) -> Result<GameplayPromptFamilyBuild> {
    let manifest_path = config.assets.join(PROMPT_TRANSLATIONS_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: GameplayPromptTranslationManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == "justice_gakuen2_practical_gameplay_prompt_translations"
            && manifest.member_count == PROMPT_MEMBER_COUNT
            && manifest.consumers.len() == 2,
        "practical gameplay-prompt manifest has an unexpected scope"
    );
    ensure!(
        manifest.pointer_table_offset.is_multiple_of(4),
        "practical gameplay-prompt pointer table is not aligned"
    );

    let mut translations = BTreeMap::new();
    for translation in &manifest.actions {
        ensure!(
            !translation.semantic_id.trim().is_empty()
                && !translation.source_text.trim().is_empty()
                && !translation.korean_text.trim().is_empty(),
            "practical gameplay-prompt translation is incomplete"
        );
        ensure!(
            translations
                .insert(translation.action_id, translation)
                .is_none(),
            "duplicate practical gameplay-prompt action {}",
            translation.action_id
        );
    }

    struct LoadedConsumer<'a> {
        binding: &'a GameplayPromptConsumerBinding,
        source: Vec<u8>,
        layout: GameplayPromptConsumerLayout,
    }

    let mut loaded_consumers = Vec::with_capacity(manifest.consumers.len());
    let mut consumer_paths = BTreeSet::new();
    for binding in &manifest.consumers {
        ensure!(
            consumer_paths.insert(&binding.path) && is_sha256(&binding.source_record_sha256),
            "practical gameplay-prompt consumer binding is invalid"
        );
        let (_, bytes) = source.read_record(&binding.path)?;
        ensure!(
            sha256_bytes(&bytes) == binding.source_record_sha256,
            "practical gameplay-prompt consumer {} source identity changed",
            binding.path
        );
        let layout = parse_gameplay_prompt_consumer(
            &bytes,
            binding.runtime_base,
            manifest.pointer_table_offset,
            manifest.member_count,
        )?;
        loaded_consumers.push(LoadedConsumer {
            binding,
            source: bytes,
            layout,
        });
    }
    let primary_layout = &loaded_consumers[0].layout;
    ensure!(
        loaded_consumers
            .iter()
            .all(|consumer| consumer.layout == *primary_layout),
        "paired practical gameplay-prompt tables are not structurally identical"
    );
    ensure!(
        primary_layout.members.len() == PROMPT_MEMBER_COUNT
            && source_members.len() >= PROMPT_MEMBER_COUNT,
        "practical gameplay-prompt member denominator changed"
    );

    let mut consumer_candidates = loaded_consumers
        .iter()
        .map(|consumer| consumer.source.clone())
        .collect::<Vec<_>>();
    let mut consumer_allowed_ranges = vec![Vec::<[usize; 2]>::new(); loaded_consumers.len()];
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&config.body_font.path)?;
    let mut member_patches = BTreeMap::new();
    let mut member_reports = Vec::with_capacity(PROMPT_MEMBER_COUNT);
    let mut localized_action_ids = BTreeSet::new();
    let mut localized_record_count = 0usize;
    let mut human_approved_record_count = 0usize;
    let mut pointer_bound_record_count = 0usize;
    let mut reachable_record_count = 0usize;
    let mut drawable_reachable_record_count = 0usize;
    let mut localized_member_count = 0usize;

    for (member_index, layout) in primary_layout.members.iter().enumerate() {
        let binding = bindings
            .get(member_index)
            .context("practical gameplay-prompt member lost its panel binding")?;
        let member = source_members
            .get(member_index)
            .context("practical gameplay-prompt producer member disappeared")?;
        let compressed = archive
            .get(member.compressed_range())
            .context("practical gameplay-prompt producer extent disappeared")?;
        let decoded = decompress(compressed, true)?;
        let mut source_records = Vec::with_capacity(layout.records.len());
        for (record_ordinal, record) in layout.records.iter().enumerate() {
            let preview = if record.fragments.is_empty() {
                None
            } else {
                Some(write_source_gameplay_prompt_preview(
                    config,
                    member_index,
                    record_ordinal,
                    record,
                    &decoded,
                )?)
            };
            source_records.push(PracticalGameplayPromptRecordReport {
                record_ordinal,
                action_id: record.action_id,
                reachable: layout.reachable_record_ordinals.contains(&record_ordinal),
                fragments: record
                    .fragments
                    .iter()
                    .map(|fragment| PracticalGameplayPromptFragmentReport {
                        new_line: fragment.new_line,
                        start_tile: fragment.start_tile,
                        tile_count: fragment.tile_count,
                    })
                    .collect(),
                source_preview_file: preview.as_ref().map(|preview| preview.file.clone()),
                source_preview_sha256: preview.map(|preview| preview.sha256),
                allocated_start_tile: None,
                allocated_tile_count: None,
                uses_source_transparent_scratch: None,
            });
        }
        pointer_bound_record_count += layout.records.len();
        reachable_record_count += layout.reachable_record_ordinals.len();
        let drawable_record_ordinals = drawable_reachable_record_ordinals(layout);
        drawable_reachable_record_count += drawable_record_ordinals.len();
        let action_ids = layout
            .records
            .iter()
            .map(|record| record.action_id)
            .collect::<Vec<_>>();
        let localized = binding.translation_state == TranslationState::Authored;
        localized_member_count += usize::from(localized);
        let mut korean_labels = Vec::new();
        let mut allocated_source_transparent_scratch_tile_count = 0usize;
        let mut allocated_source_transparent_scratch_tiles = Vec::new();
        let mut allocated_prompt_tile_count = 0usize;
        if localized && !drawable_record_ordinals.is_empty() {
            let panel_layout = binding
                .presentation_layout
                .as_ref()
                .context("authored practical gameplay-prompt member lost its panel layout")?;
            let patch = build_gameplay_prompt_member_patch(
                member_index,
                layout,
                panel_layout,
                &translations,
                rasterizer,
                &config.body_font,
                &decoded,
            )?;
            for (consumer_index, candidate) in consumer_candidates.iter_mut().enumerate() {
                rewrite_gameplay_prompt_records(
                    candidate,
                    layout,
                    &patch,
                    &translations,
                    &mut consumer_allowed_ranges[consumer_index],
                )?;
            }
            localized_action_ids.extend(&patch.action_ids);
            localized_record_count += patch.record_ordinals.len();
            human_approved_record_count += patch
                .action_ids
                .iter()
                .filter(|action_id| {
                    translations.get(action_id).is_some_and(|translation| {
                        translation.human_approval == HumanApproval::Approved
                    })
                })
                .count();
            korean_labels = patch.korean_labels.clone();
            allocated_source_transparent_scratch_tile_count =
                patch.allocated_source_transparent_scratch_tiles.len();
            allocated_source_transparent_scratch_tiles = patch
                .allocated_source_transparent_scratch_tiles
                .iter()
                .copied()
                .collect();
            allocated_prompt_tile_count =
                patch.tile_allocations.iter().map(|(_, count)| count).sum();
            for (&record_ordinal, &(start_tile, tile_count)) in
                patch.record_ordinals.iter().zip(&patch.tile_allocations)
            {
                let record_report = source_records
                    .get_mut(record_ordinal)
                    .context("localized gameplay-prompt record ordinal disappeared")?;
                record_report.allocated_start_tile = Some(start_tile);
                record_report.allocated_tile_count = Some(tile_count);
                record_report.uses_source_transparent_scratch =
                    Some((start_tile..start_tile + tile_count).any(|tile| {
                        patch
                            .allocated_source_transparent_scratch_tiles
                            .contains(&tile)
                    }));
            }
            ensure!(
                member_patches.insert(member_index, patch).is_none(),
                "duplicate practical gameplay-prompt producer patch"
            );
        }
        member_reports.push(PracticalGameplayPromptMemberReport {
            member_index,
            pointer_bound_record_count: layout.records.len(),
            reachable_record_count: layout.reachable_record_ordinals.len(),
            drawable_reachable_record_count: drawable_record_ordinals.len(),
            non_drawing_reachable_record_count: layout.reachable_record_ordinals.len()
                - drawable_record_ordinals.len(),
            source_prompt_tile_count: layout.source_prompt_tiles.len(),
            allocated_source_transparent_scratch_tile_count,
            allocated_source_transparent_scratch_tiles,
            allocated_prompt_tile_count,
            source_records,
            localized,
            action_ids,
            korean_labels,
        });
    }

    let mut consumers = Vec::with_capacity(loaded_consumers.len());
    let mut consumer_reports = Vec::with_capacity(loaded_consumers.len());
    for (index, loaded) in loaded_consumers.iter().enumerate() {
        let source_sha256 = sha256_bytes(&loaded.source);
        let claims = DecodedDataClaim::from_effective_ranges(
            &format!("practical-gameplay-prompts:{}", loaded.binding.path),
            "rewrite only gameplay-prompt records for newly allocated Korean prompt tiles",
            &loaded.source,
            &consumer_candidates[index],
            consumer_allowed_ranges[index].clone(),
        )?;
        ensure!(
            !claims.is_empty(),
            "practical gameplay-prompt consumer {} changed no records",
            loaded.binding.path
        );
        let mut plan =
            DecodedRecordWritePlan::new(&loaded.binding.path, &loaded.source, &source_sha256)?;
        plan.register_data_candidate(
            "practical-gameplay-prompt-table-builder",
            &source_sha256,
            &consumer_candidates[index],
            &claims,
        )?;
        let record = plan.apply(None)?;
        let patched_sha256 = sha256_bytes(&record);
        let changed_byte_count = difference_ranges(&loaded.source, &record)
            .iter()
            .map(|[start, end]| end - start)
            .sum();
        consumers.push(PracticalInstructionPromptConsumerBuild {
            path: loaded.binding.path.clone(),
            record,
            source_record_size: loaded.source.len(),
            source_record_sha256: source_sha256.clone(),
            patched_record_sha256: patched_sha256.clone(),
        });
        consumer_reports.push(PracticalGameplayPromptConsumerReport {
            path: loaded.binding.path.clone(),
            runtime_base: format!("0x{:08x}", loaded.binding.runtime_base),
            source_record_size: loaded.source.len(),
            source_record_sha256: source_sha256,
            patched_record_sha256: patched_sha256,
            changed_byte_count,
        });
    }
    Ok(GameplayPromptFamilyBuild {
        member_patches,
        consumers,
        report: PracticalGameplayPromptBuildReport {
            translation_manifest_sha256: sha256_bytes(&manifest_bytes),
            consumer_count: loaded_consumers.len(),
            member_count: PROMPT_MEMBER_COUNT,
            pointer_bound_record_count,
            reachable_record_count,
            drawable_reachable_record_count,
            non_drawing_reachable_record_count: reachable_record_count
                - drawable_reachable_record_count,
            localized_member_count,
            pending_member_count: PROMPT_MEMBER_COUNT - localized_member_count,
            localized_record_count,
            human_approved_record_count,
            localized_action_ids: localized_action_ids.into_iter().collect(),
            all_consumer_tables_structurally_identical: true,
            all_changes_confined_to_prompt_owned_tiles_and_records: true,
            consumers: consumer_reports,
            members: member_reports,
        },
    })
}

fn parse_gameplay_prompt_consumer(
    bytes: &[u8],
    runtime_base: u32,
    pointer_table_offset: usize,
    member_count: usize,
) -> Result<GameplayPromptConsumerLayout> {
    ensure!(
        pointer_table_offset + member_count * 4 <= bytes.len(),
        "practical gameplay-prompt pointer table is truncated"
    );
    let structure_offsets = (0..member_count)
        .map(|index| {
            read_runtime_pointer_offset(bytes, pointer_table_offset + index * 4, runtime_base)
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        structure_offsets.windows(2).all(|pair| pair[0] < pair[1])
            && structure_offsets
                .last()
                .is_some_and(|offset| *offset < pointer_table_offset),
        "practical gameplay-prompt structures are not ordered before their table"
    );
    let mut members = Vec::with_capacity(member_count);
    for (member_index, &structure_offset) in structure_offsets.iter().enumerate() {
        let mapping_offset = read_runtime_pointer_offset(bytes, structure_offset, runtime_base)?;
        let mapping = bytes
            .get(mapping_offset..mapping_offset + PROMPT_MAPPING_BYTE_COUNT)
            .context("practical gameplay-prompt mapping is truncated")?
            .to_vec();
        let record_count = mapping
            .iter()
            .copied()
            .max()
            .map_or(0, |ordinal| usize::from(ordinal) + 1);
        let record_offsets = (0..record_count)
            .map(|record| {
                read_runtime_pointer_offset(bytes, structure_offset + 4 + record * 4, runtime_base)
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            !record_offsets.is_empty()
                && record_offsets.windows(2).all(|pair| pair[0] < pair[1])
                && record_offsets
                    .last()
                    .is_some_and(|offset| *offset < structure_offset)
                && mapping
                    .iter()
                    .all(|ordinal| usize::from(*ordinal) < record_count),
            "practical gameplay-prompt member {member_index} record catalog is invalid"
        );
        let reachable_record_ordinals = mapping
            .iter()
            .map(|ordinal| usize::from(*ordinal))
            .collect::<BTreeSet<_>>();
        let mut records = Vec::with_capacity(record_count);
        let mut source_prompt_tiles = BTreeSet::new();
        for (record_ordinal, &offset) in record_offsets.iter().enumerate() {
            let end = record_offsets
                .get(record_ordinal + 1)
                .copied()
                .unwrap_or(structure_offset);
            ensure!(
                end > offset + 1,
                "practical gameplay-prompt record is truncated"
            );
            let action_id = bytes[offset];
            let fragment_count = usize::from(bytes[offset + 1]);
            let mut cursor = offset + 2;
            let mut fragments = Vec::with_capacity(fragment_count);
            for _ in 0..fragment_count {
                let new_line = bytes
                    .get(cursor)
                    .copied()
                    .context("practical gameplay-prompt fragment is truncated")?
                    == 0xff;
                if new_line {
                    cursor += 1;
                }
                let start_tile = usize::from(
                    *bytes
                        .get(cursor)
                        .context("practical gameplay-prompt tile start is truncated")?,
                );
                let tile_count = usize::from(
                    *bytes
                        .get(cursor + 1)
                        .context("practical gameplay-prompt tile count is truncated")?,
                );
                ensure!(
                    tile_count > 0,
                    "practical gameplay-prompt fragment is empty"
                );
                cursor += 2;
                source_prompt_tiles.extend(start_tile..start_tile + tile_count);
                fragments.push(GameplayPromptFragment {
                    new_line,
                    start_tile,
                    tile_count,
                });
            }
            ensure!(
                cursor <= end,
                "practical gameplay-prompt member {member_index} record {record_ordinal} action {action_id} exceeds its next pointer boundary"
            );
            records.push(GameplayPromptRecord {
                offset,
                encoded_size: cursor - offset,
                action_id,
                fragments,
            });
        }
        members.push(GameplayPromptMemberLayout {
            member_index,
            mapping,
            records,
            reachable_record_ordinals,
            source_prompt_tiles,
        });
    }
    Ok(GameplayPromptConsumerLayout { members })
}

fn build_gameplay_prompt_member_patch(
    member_index: usize,
    layout: &GameplayPromptMemberLayout,
    panel_layout: &PresentationLayout,
    translations: &BTreeMap<u8, &GameplayPromptTranslation>,
    rasterizer: &IndexedTextRasterizer,
    font: &ShiftedSizedFontSource,
    source_decoded: &[u8],
) -> Result<GameplayPromptMemberPatch> {
    ensure!(
        layout.member_index == member_index
            && !drawable_reachable_record_ordinals(layout).is_empty()
            && !layout.source_prompt_tiles.is_empty(),
        "practical gameplay-prompt member layout changed"
    );
    let panel_tiles = panel_layout
        .source_tile_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let tim = detect_embedded_tim_images(source_decoded)
        .into_iter()
        .next()
        .context("practical gameplay-prompt producer has no TIM")?;
    ensure!(
        tim.offset == 0 && tim.bits_per_pixel == 4 && tim.pixel_width == 256,
        "practical gameplay-prompt producer TIM geometry changed"
    );
    let palette_roles = source_presentation_palette_roles(source_decoded)?;
    let source_transparent_index = palette_roles.background_index;
    let text_palette_indices =
        white_panel_text_palette_indices(panel_layout, &palette_roles, source_decoded)?;
    ensure!(
        layout.source_prompt_tiles.iter().all(|tile| {
            tile % SOURCE_TILE_COLUMNS * TILE_WIDTH + TILE_WIDTH <= tim.pixel_width
                && tile / SOURCE_TILE_COLUMNS * TILE_HEIGHT + TILE_HEIGHT <= tim.pixel_height
        }),
        "practical gameplay-prompt tile leaves its producer TIM"
    );

    struct RenderedPrompt {
        record_ordinal: usize,
        action_id: u8,
        korean_text: String,
        tile_count: usize,
        canvas_width: usize,
        pixels: Vec<u8>,
    }
    let source_transparent_scratch_tiles = source_transparent_prompt_scratch_tiles(
        source_decoded,
        &tim,
        &layout.source_prompt_tiles,
        &panel_tiles,
    )?;
    let owned_tile_indices = layout
        .source_prompt_tiles
        .difference(&panel_tiles)
        .copied()
        .chain(source_transparent_scratch_tiles.iter().copied())
        .collect::<BTreeSet<_>>();
    ensure!(
        !owned_tile_indices.is_empty(),
        "practical gameplay-prompt member {member_index} has no disjoint producer cells"
    );
    let maximum_width = contiguous_tile_runs(&owned_tile_indices)
        .into_iter()
        .map(|(_, count)| count)
        .max()
        .context("practical gameplay-prompt member has no contiguous producer cells")?
        * TILE_WIDTH;
    let mut rendered = Vec::with_capacity(layout.reachable_record_ordinals.len());
    for (record_ordinal, record) in layout
        .records
        .iter()
        .enumerate()
        .filter(|(ordinal, record)| {
            layout.reachable_record_ordinals.contains(ordinal) && !record.fragments.is_empty()
        })
    {
        let translation = translations.get(&record.action_id).with_context(|| {
            format!(
                "authored practical member {member_index} has no Korean gameplay prompt for action {}",
                record.action_id
            )
        })?;
        let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
            &translation.korean_text,
            maximum_width,
            TILE_HEIGHT,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            1,
            u8::try_from(text_palette_indices.len())?,
            HorizontalTextAlignment::Left,
        )?;
        let tile_count = ((raster.measured_advance_px.ceil() as usize).max(1)).div_ceil(TILE_WIDTH);
        ensure!(
            tile_count <= owned_tile_indices.len(),
            "Korean gameplay prompt does not fit member {member_index}: {}",
            translation.korean_text
        );
        let mut pixels = raster.pixels;
        for pixel in &mut pixels {
            *pixel = if *pixel == 0 {
                source_transparent_index
            } else {
                *text_palette_indices
                    .get(usize::from(*pixel - 1))
                    .context("practical gameplay-prompt coverage index left its text palette")?
            };
        }
        rendered.push(RenderedPrompt {
            record_ordinal,
            action_id: record.action_id,
            korean_text: translation.korean_text.clone(),
            tile_count,
            canvas_width: maximum_width,
            pixels,
        });
    }

    let mut free_runs = contiguous_tile_runs(&owned_tile_indices);
    let mut allocations = Vec::with_capacity(rendered.len());
    for prompt in &rendered {
        let (run_index, &(run_start, run_count)) = free_runs
            .iter()
            .enumerate()
            .filter(|(_, (_, count))| *count >= prompt.tile_count)
            .min_by_key(|(_, (_, count))| *count)
            .with_context(|| {
                format!(
                    "Korean gameplay prompt '{}' needs {} contiguous tiles in member {member_index}; remaining owned runs: {free_runs:?}",
                    prompt.korean_text, prompt.tile_count
                )
            })?;
        allocations.push((run_start, prompt.tile_count));
        if run_count == prompt.tile_count {
            free_runs.remove(run_index);
        } else {
            free_runs[run_index] = (run_start + prompt.tile_count, run_count - prompt.tile_count);
        }
    }

    let mut candidate = source_decoded.to_vec();
    let mut allowed_ranges = Vec::new();
    let allocated_tiles = allocations
        .iter()
        .flat_map(|&(start, count)| start..start + count)
        .collect::<BTreeSet<_>>();
    let allocated_source_transparent_scratch_tiles = allocated_tiles
        .intersection(&source_transparent_scratch_tiles)
        .copied()
        .collect::<BTreeSet<_>>();
    let clear_tile = vec![source_transparent_index; TILE_WIDTH * TILE_HEIGHT];
    for &tile in &allocated_tiles {
        allowed_ranges.extend(
            write_indexed_cell_in_prefix_with_report(
                &mut candidate,
                0,
                tile_cell(tile),
                &clear_tile,
            )?
            .allowed_ranges,
        );
    }
    for (prompt, &(start_tile, tile_count)) in rendered.iter().zip(&allocations) {
        for tile_column in 0..tile_count {
            let mut pixels = Vec::with_capacity(TILE_WIDTH * TILE_HEIGHT);
            for y in 0..TILE_HEIGHT {
                let start = y * prompt.canvas_width + tile_column * TILE_WIDTH;
                pixels.extend_from_slice(&prompt.pixels[start..start + TILE_WIDTH]);
            }
            allowed_ranges.extend(
                write_indexed_cell_in_prefix_with_report(
                    &mut candidate,
                    0,
                    tile_cell(start_tile + tile_column),
                    &pixels,
                )?
                .allowed_ranges,
            );
        }
    }
    let claims = DecodedDataClaim::from_effective_ranges(
        &format!("practical-gameplay-prompts:member-{member_index:03}"),
        "replace only consumer-bound or source-transparent scratch gameplay-prompt tiles with Korean labels",
        source_decoded,
        &candidate,
        allowed_ranges,
    )?;
    ensure!(
        !claims.is_empty(),
        "gameplay-prompt producer changed no bytes"
    );
    Ok(GameplayPromptMemberPatch {
        candidate,
        claims,
        record_ordinals: rendered
            .iter()
            .map(|prompt| prompt.record_ordinal)
            .collect(),
        action_ids: rendered.iter().map(|prompt| prompt.action_id).collect(),
        korean_labels: rendered
            .iter()
            .map(|prompt| prompt.korean_text.clone())
            .collect(),
        tile_allocations: allocations,
        owned_tile_indices,
        allocated_source_transparent_scratch_tiles,
    })
}

fn source_transparent_prompt_scratch_tiles(
    source_decoded: &[u8],
    tim: &crate::embedded_tim::EmbeddedTimAudit,
    source_prompt_tiles: &BTreeSet<usize>,
    panel_tiles: &BTreeSet<usize>,
) -> Result<BTreeSet<usize>> {
    ensure!(
        tim.pixel_width.is_multiple_of(TILE_WIDTH) && tim.pixel_height.is_multiple_of(TILE_HEIGHT),
        "practical gameplay-prompt producer TIM is not tile aligned"
    );
    let transparent_index =
        usize::from(source_presentation_palette_roles(source_decoded)?.background_index);
    let first_tail_tile = source_prompt_tiles
        .iter()
        .chain(panel_tiles)
        .copied()
        .max()
        .map_or(0, |tile| tile + 1);
    let tile_count = tim.pixel_width / TILE_WIDTH * (tim.pixel_height / TILE_HEIGHT);
    let mut scratch = BTreeSet::new();
    for tile in first_tail_tile..tile_count {
        if tile > usize::from(u8::MAX) || panel_tiles.contains(&tile) {
            continue;
        }
        let pixels = read_indexed_cell_in_prefix(source_decoded, 0, tile_cell(tile))?;
        if pixels
            .iter()
            .all(|&pixel| usize::from(pixel) == transparent_index)
        {
            scratch.insert(tile);
        }
    }
    Ok(scratch)
}

fn rewrite_gameplay_prompt_records(
    consumer: &mut [u8],
    layout: &GameplayPromptMemberLayout,
    patch: &GameplayPromptMemberPatch,
    translations: &BTreeMap<u8, &GameplayPromptTranslation>,
    allowed_ranges: &mut Vec<[usize; 2]>,
) -> Result<()> {
    ensure!(
        patch.record_ordinals.len() == patch.action_ids.len()
            && patch.action_ids.len() == patch.korean_labels.len()
            && patch.korean_labels.len() == patch.tile_allocations.len(),
        "practical gameplay-prompt rewrite population changed"
    );
    ensure!(
        patch
            .record_ordinals
            .iter()
            .copied()
            .eq(drawable_reachable_record_ordinals(layout)),
        "practical gameplay-prompt rewrite population left the drawable reachable records"
    );
    let mut allocated_tiles = BTreeSet::new();
    ensure!(
        patch.tile_allocations.iter().all(|(start, count)| {
            *count > 0
                && (*start..*start + *count).all(|tile| {
                    patch.owned_tile_indices.contains(&tile) && allocated_tiles.insert(tile)
                })
        }),
        "practical gameplay-prompt allocation leaves or reuses its owned tiles"
    );
    for (((&record_ordinal, &action_id), korean_label), &(start_tile, tile_count)) in patch
        .record_ordinals
        .iter()
        .zip(&patch.action_ids)
        .zip(&patch.korean_labels)
        .zip(&patch.tile_allocations)
    {
        let record = layout
            .records
            .get(record_ordinal)
            .context("reachable practical gameplay-prompt record disappeared")?;
        ensure!(
            action_id == record.action_id
                && translations
                    .get(&action_id)
                    .is_some_and(|translation| translation.korean_text == *korean_label)
                && record.encoded_size >= 4,
            "practical gameplay-prompt record identity changed"
        );
        let target = consumer
            .get_mut(record.offset..record.offset + record.encoded_size)
            .context("practical gameplay-prompt consumer record disappeared")?;
        target.fill(0);
        target[..4].copy_from_slice(&[
            action_id,
            1,
            u8::try_from(start_tile)?,
            u8::try_from(tile_count)?,
        ]);
        allowed_ranges.push([record.offset, record.offset + record.encoded_size]);
    }
    Ok(())
}

fn drawable_reachable_record_ordinals(layout: &GameplayPromptMemberLayout) -> BTreeSet<usize> {
    layout
        .records
        .iter()
        .enumerate()
        .filter(|(ordinal, record)| {
            layout.reachable_record_ordinals.contains(ordinal) && !record.fragments.is_empty()
        })
        .map(|(ordinal, _)| ordinal)
        .collect()
}

fn contiguous_tile_runs(tiles: &BTreeSet<usize>) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    for &tile in tiles {
        match runs.last_mut() {
            Some((start, count)) if *start + *count == tile => *count += 1,
            _ => runs.push((tile, 1)),
        }
    }
    runs
}

fn tile_cell(tile: usize) -> Cell {
    Cell {
        x: tile % SOURCE_TILE_COLUMNS * TILE_WIDTH,
        y: tile / SOURCE_TILE_COLUMNS * TILE_HEIGHT,
        width: TILE_WIDTH,
        height: TILE_HEIGHT,
    }
}

fn read_runtime_pointer_offset(bytes: &[u8], offset: usize, runtime_base: u32) -> Result<usize> {
    let pointer = u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("practical gameplay-prompt pointer is truncated")?
            .try_into()?,
    );
    ensure!(
        pointer >= runtime_base,
        "practical gameplay-prompt pointer {pointer:#010x} at source offset {offset:#06x} precedes runtime base {runtime_base:#010x}"
    );
    let source_offset = usize::try_from(pointer - runtime_base)?;
    ensure!(
        source_offset < bytes.len(),
        "practical gameplay-prompt pointer leaves its overlay"
    );
    Ok(source_offset)
}

fn load_manifest(directory: &Path) -> Result<(PracticalInstructionManifest, String)> {
    let path = directory.join("manifest.json");
    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: PracticalInstructionManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported practical-instruction manifest kind"
    );
    ensure!(
        is_sha256(&manifest.source_record_sha256),
        "practical-instruction source record hash is invalid"
    );
    Ok((manifest, sha256_bytes(&bytes)))
}

fn validate_member_bindings(
    bindings: &[PracticalInstructionMemberBinding],
    source_member_count: usize,
) -> Result<()> {
    ensure!(
        !bindings.is_empty(),
        "practical-instruction manifest has no members"
    );
    let mut asset_ids = BTreeSet::new();
    let mut title_ids = BTreeSet::new();
    let mut indices = BTreeSet::new();
    for binding in bindings {
        ensure!(
            !binding.asset_unit_id.trim().is_empty()
                && !binding.title_semantic_id.trim().is_empty()
                && is_sha256(&binding.source_decoded_sha256),
            "practical-instruction member binding is incomplete"
        );
        ensure!(
            asset_ids.insert(&binding.asset_unit_id),
            "duplicate practical-instruction asset-unit ID {}",
            binding.asset_unit_id
        );
        ensure!(
            title_ids.insert(&binding.title_semantic_id),
            "duplicate practical-instruction title semantic ID {}",
            binding.title_semantic_id
        );
        ensure!(
            indices.insert(binding.member_index),
            "duplicate practical-instruction member index {}",
            binding.member_index
        );
        match binding.translation_state {
            TranslationState::Pending => ensure!(
                binding.translation.is_none(),
                "pending practical-instruction member {} has a translation asset",
                binding.member_index
            ),
            TranslationState::Authored => ensure!(
                binding.translation.is_some() && binding.presentation_layout.is_some(),
                "authored practical-instruction member {} has incomplete presentation input",
                binding.member_index
            ),
        }
    }
    ensure!(
        indices == (0..source_member_count).collect(),
        "practical-instruction manifest does not bind every source member exactly once"
    );
    Ok(())
}

fn load_translation(
    assets: &Path,
    relative: &Path,
    binding: &PracticalInstructionMemberBinding,
) -> Result<(PracticalInstructionTranslation, String)> {
    ensure_normalized_relative_path(relative)?;
    let path = assets.join(relative);
    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let translation: PracticalInstructionTranslation = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    ensure!(
        translation.kind == TRANSLATION_KIND && translation.asset_unit_id == binding.asset_unit_id,
        "practical-instruction translation does not match its manifest binding"
    );
    ensure!(
        !translation.source_text.trim().is_empty()
            && !translation.korean_lines.is_empty()
            && translation
                .korean_lines
                .iter()
                .all(|line| !line.trim().is_empty())
            && translation
                .text_placements
                .iter()
                .all(|placement| !placement.text.trim().is_empty()),
        "practical-instruction translation text is empty"
    );
    Ok((translation, sha256_bytes(&bytes)))
}

fn validate_translation_layout(
    translation: &PracticalInstructionTranslation,
    layout: &PresentationLayout,
) -> Result<()> {
    ensure!(
        !translation.korean_lines.is_empty() && translation.korean_lines.len() <= layout.rows.len(),
        "practical-instruction Korean lines exceed the consumer presentation rows"
    );
    let text_placements = effective_text_placements(translation);
    ensure!(
        !text_placements.is_empty()
            && text_placements.iter().all(|placement| {
                layout
                    .rows
                    .get(placement.row)
                    .is_some_and(|row| placement.column < row.tile_count)
            }),
        "practical-instruction text placement leaves its consumer rows"
    );
    let source_tiles = layout
        .source_tile_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut graphic_ids = BTreeSet::new();
    let mut graphic_source_tiles = BTreeSet::new();
    let mut graphic_target_cells = BTreeSet::new();
    for graphic in &translation.preserved_graphics {
        ensure!(
            !graphic.semantic_id.is_empty()
                && graphic
                    .semantic_id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
                && graphic_ids.insert(&graphic.semantic_id),
            "practical-instruction preserved graphic semantic ID is invalid or duplicated"
        );
        let row = layout
            .rows
            .get(graphic.row)
            .context("practical-instruction preserved graphic leaves its consumer rows")?;
        ensure!(
            !graphic.source_tile_indices.is_empty()
                && graphic.column + graphic.source_tile_indices.len() <= row.tile_count
                && graphic.source_tile_indices.iter().all(|tile| {
                    source_tiles.contains(tile) && graphic_source_tiles.insert(*tile)
                })
                && (graphic.column..graphic.column + graphic.source_tile_indices.len())
                    .all(|column| graphic_target_cells.insert((graphic.row, column))),
            "practical-instruction preserved graphic ownership overlaps or leaves its panel"
        );
    }
    Ok(())
}

fn effective_text_placements(
    translation: &PracticalInstructionTranslation,
) -> Vec<PresentationTextPlacement> {
    if translation.text_placements.is_empty() {
        translation
            .korean_lines
            .iter()
            .enumerate()
            .map(|(row, text)| PresentationTextPlacement {
                row,
                column: 0,
                text: text.clone(),
            })
            .collect()
    } else {
        translation.text_placements.clone()
    }
}

fn validate_presentation_layout(layout: &PresentationLayout, source_decoded: &[u8]) -> Result<()> {
    validate_presentation_shape(layout)?;
    let tim = detect_embedded_tim_images(source_decoded)
        .into_iter()
        .next()
        .context("practical-instruction source member has no TIM")?;
    ensure!(
        tim.offset == 0 && tim.bits_per_pixel == 4,
        "practical-instruction source member is no longer a leading 4-bpp TIM"
    );
    let source_image = Cell {
        x: 0,
        y: 0,
        width: tim.pixel_width,
        height: tim.pixel_height,
    };
    let bindings = presentation_tile_bindings(layout);
    ensure!(
        !bindings.is_empty()
            && bindings
                .iter()
                .all(|binding| contains_cell(source_image, binding.source_cell)),
        "practical-instruction consumer layout leaves the source TIM"
    );
    for (index, binding) in bindings.iter().enumerate() {
        ensure!(
            binding.source_cell.width == TILE_WIDTH
                && binding.source_cell.height == TILE_HEIGHT
                && binding.presentation_cell.width == TILE_WIDTH
                && binding.presentation_cell.height == TILE_HEIGHT,
            "practical-instruction consumer tile geometry changed"
        );
        for previous in bindings.iter().take(index) {
            ensure!(
                !cells_overlap_locally(binding.source_cell, previous.source_cell)
                    && !cells_overlap_locally(
                        binding.presentation_cell,
                        previous.presentation_cell
                    ),
                "practical-instruction consumer tile bindings overlap"
            );
        }
    }
    Ok(())
}

fn validate_presentation_shape(layout: &PresentationLayout) -> Result<()> {
    ensure!(
        !layout.rows.is_empty()
            && layout.source_tile_indices.len()
                == layout.rows.iter().map(|row| row.tile_count).sum::<usize>(),
        "practical-instruction source-tile stream does not cover its presentation rows"
    );
    if let Some(text_palette_indices) = layout.text_palette_indices.as_ref() {
        ensure!(
            !text_palette_indices.is_empty()
                && text_palette_indices.len() <= 15
                && text_palette_indices.iter().all(|&index| index < 16)
                && text_palette_indices
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == text_palette_indices.len(),
            "practical-instruction text palette indices are invalid or duplicated"
        );
    }
    if let Some(initial_counts) = layout.initial_row_tile_counts.as_ref() {
        ensure!(
            !initial_counts.is_empty()
                && initial_counts.len() <= layout.rows.len()
                && initial_counts
                    .iter()
                    .zip(&layout.rows)
                    .all(|(initial, final_row)| *initial <= final_row.tile_count),
            "practical-instruction initial row capacities exceed the final presentation"
        );
    }
    Ok(())
}

fn resolved_presentation_text_palette_indices(
    layout: &PresentationLayout,
    palette_roles: &SourcePresentationPaletteRoles,
) -> Result<Vec<u8>> {
    let indices = layout.text_palette_indices.clone().unwrap_or_else(|| {
        (0..16)
            .filter(|index| !palette_roles.transparent_indices.contains(index))
            .collect()
    });
    ensure!(
        !indices.is_empty()
            && indices
                .iter()
                .all(|index| !palette_roles.transparent_indices.contains(index)),
        "practical-instruction text palette includes a transparent index"
    );
    Ok(indices)
}

#[derive(Debug)]
struct SourcePresentationPaletteRoles {
    background_index: u8,
    transparent_indices: BTreeSet<u8>,
}

fn source_presentation_palette_roles(
    source_decoded: &[u8],
) -> Result<SourcePresentationPaletteRoles> {
    let tim = parse_4bpp_prefix(source_decoded)?;
    let palette = read_4bpp_palette_words_in_prefix(source_decoded, 0, 0)?;
    let pixels = read_indexed_cell_in_prefix(
        source_decoded,
        0,
        Cell {
            x: 0,
            y: 0,
            width: tim.pixel_width(),
            height: tim.image_height,
        },
    )?;
    let mut histogram = [0usize; 16];
    for pixel in pixels {
        histogram[usize::from(pixel)] += 1;
    }
    let transparent_indices = palette
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == 0)
        .map(|(index, _)| u8::try_from(index))
        .collect::<Result<BTreeSet<_>, _>>()?;
    ensure!(
        !transparent_indices.is_empty(),
        "practical-instruction source palette has no transparent color"
    );
    let background_index = transparent_indices
        .iter()
        .copied()
        .max_by_key(|index| (histogram[usize::from(*index)], std::cmp::Reverse(*index)))
        .expect("checked transparent candidate population");
    ensure!(
        histogram[usize::from(background_index)] > 0,
        "practical-instruction source does not use a transparent palette index"
    );
    Ok(SourcePresentationPaletteRoles {
        background_index,
        transparent_indices,
    })
}

#[derive(Debug)]
struct IndexedPresentation {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct PresentationTileBinding {
    source_cell: Cell,
    presentation_cell: Cell,
}

fn presentation_tile_bindings(layout: &PresentationLayout) -> Vec<PresentationTileBinding> {
    let mut source_tiles = layout.source_tile_indices.iter().copied();
    let mut bindings = Vec::new();
    for row in &layout.rows {
        for column in 0..row.tile_count {
            let source_tile = source_tiles
                .next()
                .expect("validated presentation source-tile stream");
            bindings.push(PresentationTileBinding {
                source_cell: Cell {
                    x: source_tile % SOURCE_TILE_COLUMNS * TILE_WIDTH,
                    y: source_tile / SOURCE_TILE_COLUMNS * TILE_HEIGHT,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
                presentation_cell: Cell {
                    x: row.x + column * TILE_WIDTH,
                    y: row.y,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
            });
        }
    }
    debug_assert!(source_tiles.next().is_none());
    bindings
}

fn presentation_canvas(
    layout: &PresentationLayout,
    background_index: u8,
) -> Result<IndexedPresentation> {
    let width = layout
        .rows
        .iter()
        .map(|row| row.x + row.tile_count * TILE_WIDTH)
        .max()
        .context("practical-instruction presentation has no rows")?;
    let height = layout
        .rows
        .iter()
        .map(|row| row.y + TILE_HEIGHT)
        .max()
        .context("practical-instruction presentation has no rows")?;
    Ok(IndexedPresentation {
        width,
        height,
        pixels: vec![background_index; width * height],
    })
}

fn unpack_source_presentation(
    source_decoded: &[u8],
    layout: &PresentationLayout,
) -> Result<IndexedPresentation> {
    let background_index = source_presentation_palette_roles(source_decoded)?.background_index;
    let mut presentation = presentation_canvas(layout, background_index)?;
    for binding in presentation_tile_bindings(layout) {
        let pixels = read_indexed_cell_in_prefix(source_decoded, 0, binding.source_cell)?;
        write_indexed_cell(&mut presentation, binding.presentation_cell, &pixels)?;
    }
    Ok(presentation)
}

fn write_source_presentation_preview(
    config: &PracticalInstructionGraphicsBuildConfig,
    member_index: usize,
    layout: &PresentationLayout,
    source_decoded: &[u8],
) -> Result<SourcePresentationPreview> {
    let presentation = unpack_source_presentation(source_decoded, layout)?;
    let palette = read_4bpp_palette_words_in_prefix(source_decoded, 0, 0)?;
    let rgba = indexed_to_rgba(&presentation, &palette);
    let file = format!("previews/member-{member_index:03}-source-presentation.png");
    let path = config.output_dir.join(&file);
    std::fs::create_dir_all(path.parent().context("source preview path has no parent")?)?;
    write_tim_preview(&path, &rgba)?;
    Ok(SourcePresentationPreview {
        file,
        sha256: sha256_file(&path)?,
    })
}

fn write_preserved_graphic_previews(
    config: &PracticalInstructionGraphicsBuildConfig,
    member_index: usize,
    graphics: &[PreservedPresentationGraphic],
    source_decoded: &[u8],
) -> Result<Vec<PracticalInstructionPreservedGraphicReport>> {
    let palette = read_4bpp_palette_words_in_prefix(source_decoded, 0, 0)?;
    let transparent_index = source_presentation_palette_roles(source_decoded)?.background_index;
    let mut reports = Vec::with_capacity(graphics.len());
    for graphic in graphics {
        let mut preview = IndexedPresentation {
            width: graphic.source_tile_indices.len() * TILE_WIDTH,
            height: TILE_HEIGHT,
            pixels: vec![
                transparent_index;
                graphic.source_tile_indices.len() * TILE_WIDTH * TILE_HEIGHT
            ],
        };
        let mut has_ink = false;
        for (column, &source_tile) in graphic.source_tile_indices.iter().enumerate() {
            let pixels = read_indexed_cell_in_prefix(source_decoded, 0, tile_cell(source_tile))?;
            has_ink |= pixels.iter().any(|&pixel| pixel != transparent_index);
            write_indexed_cell(
                &mut preview,
                Cell {
                    x: column * TILE_WIDTH,
                    y: 0,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
                &pixels,
            )?;
        }
        ensure!(
            has_ink,
            "practical-instruction preserved graphic {} has no source ink",
            graphic.semantic_id
        );
        let file = format!(
            "previews/member-{member_index:03}-source-graphic-{}.png",
            graphic.semantic_id
        );
        let path = config.output_dir.join(&file);
        let rgba = indexed_to_rgba(&preview, &palette);
        write_tim_preview(&path, &rgba)?;
        reports.push(PracticalInstructionPreservedGraphicReport {
            semantic_id: graphic.semantic_id.clone(),
            source_tile_indices: graphic.source_tile_indices.clone(),
            presentation_row: graphic.row,
            presentation_column: graphic.column,
            preview_file: file,
            preview_sha256: sha256_file(&path)?,
        });
    }
    Ok(reports)
}

fn render_presentation(
    rasterizer: &IndexedTextRasterizer,
    font: &ShiftedSizedFontSource,
    layout: &PresentationLayout,
    translation: &PracticalInstructionTranslation,
    source_decoded: &[u8],
) -> Result<IndexedPresentation> {
    let palette_roles = source_presentation_palette_roles(source_decoded)?;
    let source_transparent_index = palette_roles.background_index;
    let mut presentation = presentation_canvas(layout, source_transparent_index)?;
    let text_palette_indices =
        white_panel_text_palette_indices(layout, &palette_roles, source_decoded)?;
    for placement in effective_text_placements(translation) {
        let row = layout
            .rows
            .get(placement.row)
            .context("practical-instruction text placement row disappeared")?;
        let initial_row_width = layout
            .initial_row_tile_counts
            .as_ref()
            .and_then(|counts| counts.get(placement.row))
            .copied()
            .unwrap_or(row.tile_count)
            * TILE_WIDTH;
        let placement_x = placement.column * TILE_WIDTH;
        ensure!(
            placement_x < initial_row_width,
            "practical-instruction text placement begins outside its initial row"
        );
        let available_width = initial_row_width - placement_x;
        let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
            &placement.text,
            available_width,
            TILE_HEIGHT,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            1,
            u8::try_from(text_palette_indices.len())?,
            HorizontalTextAlignment::Left,
        )?;
        ensure!(
            raster.measured_advance_px <= available_width as f32,
            "practical-instruction text does not fit its consumer row: {}",
            placement.text
        );
        for y in 0..TILE_HEIGHT {
            for x in 0..available_width {
                let coverage_index = raster.pixels[y * available_width + x];
                if coverage_index == 0 {
                    continue;
                }
                let pixel = *text_palette_indices
                    .get(usize::from(coverage_index - 1))
                    .context("practical-instruction text coverage index left its palette")?;
                let target = (row.y + y) * presentation.width + row.x + placement_x + x;
                ensure!(
                    presentation.pixels[target] == source_transparent_index,
                    "practical-instruction text placements overlap"
                );
                presentation.pixels[target] = pixel;
            }
        }
    }
    for graphic in &translation.preserved_graphics {
        let row = layout
            .rows
            .get(graphic.row)
            .context("practical-instruction preserved graphic row disappeared")?;
        for (column_offset, &source_tile) in graphic.source_tile_indices.iter().enumerate() {
            let pixels = read_indexed_cell_in_prefix(source_decoded, 0, tile_cell(source_tile))?;
            composite_indexed_cell(
                &mut presentation,
                Cell {
                    x: row.x + (graphic.column + column_offset) * TILE_WIDTH,
                    y: row.y,
                    width: TILE_WIDTH,
                    height: TILE_HEIGHT,
                },
                &pixels,
                source_transparent_index,
                source_transparent_index,
            )?;
        }
    }
    Ok(presentation)
}

// Introductory panels, in-fight callouts and the remaining-hit counter draw on
// native white bubbles. Active titles and attempt counters draw on the arena.
fn white_panel_text_palette_indices(
    layout: &PresentationLayout,
    palette_roles: &SourcePresentationPaletteRoles,
    source_decoded: &[u8],
) -> Result<Vec<u8>> {
    let mut indices = resolved_presentation_text_palette_indices(layout, palette_roles)?;
    let palette = read_4bpp_palette_words_in_prefix(source_decoded, 0, 0)?;
    order_dark_panel_ink(&mut indices, &palette);
    Ok(indices)
}

fn order_dark_panel_ink(indices: &mut [u8], palette: &[u16; 16]) {
    indices.sort_by_key(|&index| {
        let word = palette[usize::from(index)];
        let luminance = u32::from(word & 31) * 299
            + u32::from((word >> 5) & 31) * 587
            + u32::from((word >> 10) & 31) * 114;
        (std::cmp::Reverse(luminance), index)
    });
}

fn composite_indexed_cell(
    presentation: &mut IndexedPresentation,
    cell: Cell,
    pixels: &[u8],
    source_transparent_index: u8,
    target_background_index: u8,
) -> Result<()> {
    ensure!(
        pixels.len() == cell.width * cell.height
            && contains_cell(
                Cell {
                    x: 0,
                    y: 0,
                    width: presentation.width,
                    height: presentation.height,
                },
                cell
            ),
        "practical-instruction preserved graphic leaves its presentation"
    );
    for y in 0..cell.height {
        for x in 0..cell.width {
            let pixel = pixels[y * cell.width + x];
            if pixel == source_transparent_index {
                continue;
            }
            let target = (cell.y + y) * presentation.width + cell.x + x;
            ensure!(
                presentation.pixels[target] == target_background_index,
                "practical-instruction preserved graphic overlaps Korean text or another graphic"
            );
            presentation.pixels[target] = pixel;
        }
    }
    Ok(())
}

fn write_indexed_cell(
    presentation: &mut IndexedPresentation,
    cell: Cell,
    pixels: &[u8],
) -> Result<()> {
    ensure!(
        pixels.len() == cell.width * cell.height,
        "practical-instruction presentation tile pixel count changed"
    );
    ensure!(
        contains_cell(
            Cell {
                x: 0,
                y: 0,
                width: presentation.width,
                height: presentation.height,
            },
            cell
        ),
        "practical-instruction presentation write leaves its canvas"
    );
    for y in 0..cell.height {
        let source_start = y * cell.width;
        let target_start = (cell.y + y) * presentation.width + cell.x;
        presentation.pixels[target_start..target_start + cell.width]
            .copy_from_slice(&pixels[source_start..source_start + cell.width]);
    }
    Ok(())
}

fn read_indexed_cell(presentation: &IndexedPresentation, cell: Cell) -> Result<Vec<u8>> {
    ensure!(
        contains_cell(
            Cell {
                x: 0,
                y: 0,
                width: presentation.width,
                height: presentation.height,
            },
            cell
        ),
        "practical-instruction presentation tile leaves its canvas"
    );
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        let start = y * presentation.width + cell.x;
        pixels.extend_from_slice(&presentation.pixels[start..start + cell.width]);
    }
    Ok(pixels)
}

fn indexed_to_rgba(presentation: &IndexedPresentation, palette: &[u16; 16]) -> RgbaImage {
    let rgba_palette = palette.map(|value| {
        [
            expand_five_bit_color((value & 0x1f) as u8),
            expand_five_bit_color(((value >> 5) & 0x1f) as u8),
            expand_five_bit_color(((value >> 10) & 0x1f) as u8),
            if value == 0 { 0 } else { 255 },
        ]
    });
    let mut pixels = Vec::with_capacity(presentation.pixels.len() * 4);
    for &index in &presentation.pixels {
        pixels.extend_from_slice(&rgba_palette[usize::from(index)]);
    }
    RgbaImage {
        width: presentation.width,
        height: presentation.height,
        pixels,
    }
}

fn expand_five_bit_color(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

fn cells_overlap_locally(left: Cell, right: Cell) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

fn contains_cell(outer: Cell, inner: Cell) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn ensure_normalized_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| { matches!(component, Component::Normal(_)) }),
        "practical-instruction asset path must be normalized and relative: {}",
        path.display()
    );
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn member_id(index: usize) -> String {
    format!("member-{index:03}")
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    ensure_safe_output_path(output_dir)?;
    let marker = output_dir.join(OUTPUT_MARKER_FILE);
    if output_dir.exists() {
        if !force {
            bail!(
                "practical-instruction graphics build output exists; pass --force to replace it: {}",
                output_dir.display()
            );
        }
        let marker_text = std::fs::read_to_string(&marker).with_context(|| {
            format!(
                "refusing to replace an unowned output directory without {OUTPUT_MARKER_FILE}: {}",
                output_dir.display()
            )
        })?;
        ensure!(
            marker_text == OUTPUT_MARKER_TEXT,
            "refusing to replace practical-instruction output with an unknown marker"
        );
        std::fs::remove_dir_all(output_dir)?;
    }
    std::fs::create_dir_all(output_dir)?;
    std::fs::write(output_dir.join(OUTPUT_MARKER_FILE), OUTPUT_MARKER_TEXT)?;
    Ok(())
}

fn ensure_safe_output_path(output_dir: &Path) -> Result<()> {
    ensure!(
        !output_dir.as_os_str().is_empty()
            && output_dir != Path::new(".")
            && output_dir != Path::new("..")
            && output_dir != Path::new("/"),
        "refusing unsafe practical-instruction output directory: {}",
        output_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        GameplayPromptMemberPatch, GameplayPromptRecord, GameplayPromptTranslation, HumanApproval,
        IndexedPresentation, PracticalInstructionMemberBinding, PresentationLayout,
        PresentationRow, SOURCE_TILE_COLUMNS, SourcePresentationPaletteRoles, TILE_HEIGHT,
        TILE_WIDTH, TranslationState, composite_indexed_cell, contiguous_tile_runs,
        parse_gameplay_prompt_consumer, presentation_canvas, presentation_tile_bindings,
        primary_clut_palette, resolved_presentation_text_palette_indices,
        rewrite_gameplay_prompt_records, validate_member_bindings, validate_presentation_shape,
    };

    #[test]
    fn white_panel_uses_dark_solid_ink_even_when_palette_indices_are_shuffled() {
        let mut palette = [0; 16];
        palette[3] = 0xffff;
        palette[7] = 0x98c6;
        palette[11] = 0xc210;
        for mut indices in [vec![3, 7, 11], vec![11, 3, 7], vec![7, 11, 3]] {
            super::order_dark_panel_ink(&mut indices, &palette);
            assert_eq!(indices, [3, 11, 7]);
            assert_ne!(palette[usize::from(*indices.last().unwrap())], 0);
        }
    }

    fn binding(member_index: usize) -> PracticalInstructionMemberBinding {
        PracticalInstructionMemberBinding {
            asset_unit_id: format!("instruction-{member_index}"),
            title_semantic_id: format!("title-{member_index}"),
            member_index,
            source_decoded_sha256: "0".repeat(64),
            translation_state: TranslationState::Pending,
            presentation_layout: None,
            translation: None,
        }
    }

    #[test]
    fn member_bindings_must_cover_the_source_denominator() {
        let mut bindings = vec![binding(0), binding(2)];
        let error = validate_member_bindings(&bindings, 3).unwrap_err();
        assert!(error.to_string().contains("every source member"));

        bindings.insert(1, binding(1));
        validate_member_bindings(&bindings, 3).unwrap();
    }

    #[test]
    fn primary_clut_palette_reads_the_member_owned_palette_and_coordinates() {
        const CLUT_SIZE: usize = 12 + 16 * 2;
        const IMAGE_SIZE: usize = 12 + 2;
        let mut tim = vec![0u8; 8 + CLUT_SIZE + IMAGE_SIZE];
        tim[0..4].copy_from_slice(&0x10u32.to_le_bytes());
        tim[4..8].copy_from_slice(&0x08u32.to_le_bytes());
        tim[8..12].copy_from_slice(&(CLUT_SIZE as u32).to_le_bytes());
        tim[12..14].copy_from_slice(&0u16.to_le_bytes());
        tim[14..16].copy_from_slice(&503u16.to_le_bytes());
        tim[16..18].copy_from_slice(&16u16.to_le_bytes());
        tim[18..20].copy_from_slice(&1u16.to_le_bytes());
        for color in 0..16u16 {
            let offset = 20 + usize::from(color) * 2;
            tim[offset..offset + 2].copy_from_slice(&(color * 3).to_le_bytes());
        }
        let image_offset = 8 + CLUT_SIZE;
        tim[image_offset..image_offset + 4].copy_from_slice(&(IMAGE_SIZE as u32).to_le_bytes());
        tim[image_offset + 8..image_offset + 10].copy_from_slice(&1u16.to_le_bytes());
        tim[image_offset + 10..image_offset + 12].copy_from_slice(&1u16.to_le_bytes());

        let palette = primary_clut_palette(&tim).unwrap();
        assert_eq!((palette.vram_x, palette.vram_y), (0, 503));
        assert_eq!(palette.bytes.len(), 32);
        assert_eq!(&palette.bytes[0..4], &[0, 0, 3, 0]);
    }

    #[test]
    fn presentation_bindings_follow_the_declared_runtime_tile_stream() {
        let layout = PresentationLayout {
            source_tile_indices: vec![2, 3, 9],
            rows: vec![
                PresentationRow {
                    x: 32,
                    y: 28,
                    tile_count: 2,
                },
                PresentationRow {
                    x: 32,
                    y: 44,
                    tile_count: 1,
                },
            ],
            initial_row_tile_counts: None,
            text_palette_indices: None,
        };

        validate_presentation_shape(&layout).unwrap();
        let bindings = presentation_tile_bindings(&layout);
        assert_eq!(bindings.len(), 3);
        for (binding, source_tile) in bindings.iter().zip([2, 3, 9]) {
            assert_eq!(
                (binding.source_cell.x, binding.source_cell.y),
                (
                    source_tile % SOURCE_TILE_COLUMNS * TILE_WIDTH,
                    source_tile / SOURCE_TILE_COLUMNS * TILE_HEIGHT,
                )
            );
        }
        assert_eq!(
            (
                bindings[2].presentation_cell.x,
                bindings[2].presentation_cell.y
            ),
            (32, 44)
        );
    }

    #[test]
    fn presentation_palette_resolution_excludes_all_transparent_indices() {
        let mut layout = PresentationLayout {
            source_tile_indices: vec![2],
            rows: vec![PresentationRow {
                x: 32,
                y: 28,
                tile_count: 1,
            }],
            initial_row_tile_counts: None,
            text_palette_indices: None,
        };

        let roles = SourcePresentationPaletteRoles {
            background_index: 14,
            transparent_indices: [14, 15].into_iter().collect(),
        };
        let indices = resolved_presentation_text_palette_indices(&layout, &roles).unwrap();
        assert_eq!(indices, (0..14).collect::<Vec<_>>());
        let presentation = presentation_canvas(&layout, 15).unwrap();
        assert!(presentation.pixels.iter().all(|&pixel| pixel == 15));

        layout.text_palette_indices = Some(vec![1, 15]);
        let error = resolved_presentation_text_palette_indices(&layout, &roles).unwrap_err();
        assert!(error.to_string().contains("includes a transparent index"));
    }

    #[test]
    fn staged_presentations_reject_an_initial_row_wider_than_its_final_row() {
        let layout = PresentationLayout {
            source_tile_indices: vec![4, 5],
            rows: vec![PresentationRow {
                x: 32,
                y: 28,
                tile_count: 2,
            }],
            initial_row_tile_counts: Some(vec![3]),
            text_palette_indices: None,
        };

        let error = validate_presentation_shape(&layout).unwrap_err();
        assert!(error.to_string().contains("initial row capacities"));
    }

    #[test]
    fn preserved_graphic_composition_keeps_transparency_and_rejects_text_overlap() {
        let mut presentation = IndexedPresentation {
            width: TILE_WIDTH * 2,
            height: TILE_HEIGHT,
            pixels: vec![0; TILE_WIDTH * 2 * TILE_HEIGHT],
        };
        presentation.pixels[0] = 7;
        let mut graphic = vec![0; TILE_WIDTH * TILE_HEIGHT];
        graphic[0] = 5;
        graphic[3] = 6;
        composite_indexed_cell(
            &mut presentation,
            crate::tim::Cell {
                x: TILE_WIDTH,
                y: 0,
                width: TILE_WIDTH,
                height: TILE_HEIGHT,
            },
            &graphic,
            0,
            0,
        )
        .unwrap();
        assert_eq!(presentation.pixels[0], 7);
        assert_eq!(presentation.pixels[TILE_WIDTH], 5);
        assert_eq!(presentation.pixels[TILE_WIDTH + 1], 0);
        assert_eq!(presentation.pixels[TILE_WIDTH + 3], 6);

        let error = composite_indexed_cell(
            &mut presentation,
            crate::tim::Cell {
                x: TILE_WIDTH,
                y: 0,
                width: TILE_WIDTH,
                height: TILE_HEIGHT,
            },
            &graphic,
            0,
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("overlaps Korean text"));
    }

    #[test]
    fn gameplay_prompt_parser_follows_pointer_bound_members_and_reachable_records() {
        let runtime_base = 0x8001_0000u32;
        let mut overlay = vec![0u8; 0x120];
        overlay[0x20..0x40].fill(0);
        overlay[0x40..0x44].copy_from_slice(&[8, 1, 4, 2]);
        overlay[0x50..0x54].copy_from_slice(&(runtime_base + 0x20).to_le_bytes());
        overlay[0x54..0x58].copy_from_slice(&(runtime_base + 0x40).to_le_bytes());

        overlay[0x60..0x80].fill(0);
        overlay[0x80..0x87].copy_from_slice(&[37, 2, 10, 3, 0xff, 20, 2]);
        overlay[0x90..0x94].copy_from_slice(&(runtime_base + 0x60).to_le_bytes());
        overlay[0x94..0x98].copy_from_slice(&(runtime_base + 0x80).to_le_bytes());

        overlay[0x100..0x104].copy_from_slice(&(runtime_base + 0x50).to_le_bytes());
        overlay[0x104..0x108].copy_from_slice(&(runtime_base + 0x90).to_le_bytes());

        let parsed = parse_gameplay_prompt_consumer(&overlay, runtime_base, 0x100, 2).unwrap();
        assert_eq!(parsed.members.len(), 2);
        assert_eq!(parsed.members[0].records[0].action_id, 8);
        assert_eq!(
            parsed.members[0].source_prompt_tiles,
            BTreeSet::from([4, 5])
        );
        assert_eq!(
            parsed.members[1].reachable_record_ordinals,
            BTreeSet::from([0])
        );
        assert!(parsed.members[1].records[0].fragments[1].new_line);
    }

    #[test]
    fn gameplay_prompt_record_rewrite_changes_only_drawable_reachable_records() {
        let runtime_base = 0x8001_0000u32;
        let mut overlay = vec![0x5au8; 0x120];
        overlay[0x20..0x40].fill(0);
        overlay[0x21] = 1;
        overlay[0x40..0x46].copy_from_slice(&[8, 2, 4, 1, 6, 1]);
        overlay[0x48..0x4a].copy_from_slice(&[39, 0]);
        overlay[0x50..0x54].copy_from_slice(&(runtime_base + 0x20).to_le_bytes());
        overlay[0x54..0x58].copy_from_slice(&(runtime_base + 0x40).to_le_bytes());
        overlay[0x58..0x5c].copy_from_slice(&(runtime_base + 0x48).to_le_bytes());
        overlay[0x100..0x104].copy_from_slice(&(runtime_base + 0x50).to_le_bytes());
        let mut layout = parse_gameplay_prompt_consumer(&overlay, runtime_base, 0x100, 1)
            .unwrap()
            .members
            .remove(0);
        overlay[0x4c..0x50].copy_from_slice(&[36, 1, 20, 1]);
        layout.records.push(GameplayPromptRecord {
            offset: 0x4c,
            encoded_size: 4,
            action_id: 36,
            fragments: Vec::new(),
        });
        let translation = GameplayPromptTranslation {
            action_id: 8,
            semantic_id: "forward_step".to_string(),
            source_text: "source".to_string(),
            korean_text: "전진 스텝".to_string(),
            human_approval: HumanApproval::Pending,
        };
        let translations = BTreeMap::from([(8, &translation)]);
        let patch = GameplayPromptMemberPatch {
            candidate: Vec::new(),
            claims: Vec::new(),
            record_ordinals: vec![0],
            action_ids: vec![8],
            korean_labels: vec!["전진 스텝".to_string()],
            tile_allocations: vec![(12, 1)],
            owned_tile_indices: BTreeSet::from([12]),
            allocated_source_transparent_scratch_tiles: BTreeSet::from([12]),
        };
        let before = overlay.clone();
        let mut allowed = Vec::new();
        rewrite_gameplay_prompt_records(&mut overlay, &layout, &patch, &translations, &mut allowed)
            .unwrap();

        assert_eq!(&overlay[0x40..0x46], &[8, 1, 12, 1, 0, 0]);
        assert_eq!(&overlay[0x48..0x4a], &[39, 0]);
        assert_eq!(&overlay[0x4c..0x50], &[36, 1, 20, 1]);
        assert_eq!(&overlay[..0x40], &before[..0x40]);
        assert_eq!(&overlay[0x46..], &before[0x46..]);
        assert_eq!(allowed, vec![[0x40, 0x46]]);
    }

    #[test]
    fn prompt_tile_runs_keep_only_physically_contiguous_capacity() {
        let tiles = BTreeSet::from([4, 5, 7, 8, 9, 12]);
        assert_eq!(contiguous_tile_runs(&tiles), vec![(4, 2), (7, 3), (12, 1)]);
    }
}
