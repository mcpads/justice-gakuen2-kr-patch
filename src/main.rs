use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use justice_gakuen2_kr::bonus_shop_text_source::{
    BonusShopTextAssetAuditConfig, BonusShopTextAssetSyncConfig, BonusShopTextSourceConfig,
    audit_bonus_shop_text_assets, initialize_bonus_shop_text_source, sync_bonus_shop_text_assets,
};
use justice_gakuen2_kr::character_select_graphics::{
    CharacterSelectAtlasBuildConfig, CharacterSelectAtlasPlanConfig,
    CharacterSelectGraphicsAuditConfig, audit_character_select_graphics,
    build_character_select_atlas, write_character_select_atlas_plan,
};
use justice_gakuen2_kr::consumer_analysis::{
    DEFAULT_POINTER_RUN_VALUE_FLOW_STATE_BUDGET, DEFAULT_STATIC_CONSUMER_VALUE_FLOW_STATE_BUDGET,
    StaticConsumerAuditConfig, audit_static_consumers,
};
use justice_gakuen2_kr::development_build_spec::load_development_build_spec;
use justice_gakuen2_kr::dialogue_audit::{
    ComposedNameInputBuildConfig, DialogueAuditConfig, DialogueBuildCapacityAuditConfig,
    DialogueCodeAllocationConfig, DialogueCodebookAuditConfig,
    DialogueCodebookReviewApplicationConfig, DialogueCodebookReviewConfig,
    DialogueConsumerUnionAuditConfig, DialogueDevelopmentInputPolicy,
    DialogueDevelopmentRuntimeAuditConfig, DialogueDiscBuildConfig,
    DialogueFontAllocationAuditConfig, DialogueFontBuildConfig, DialogueFontConflictAuditConfig,
    DialogueFontPreviewConfig, DialogueGlossaryAuditConfig, DialogueLayoutAuditConfig,
    DialogueMessageBuildConfig, DialogueNameEntryAuditConfig, DialogueNameEntryBuildConfig,
    DialogueSceneAuditConfig, DialogueSelectorAddressFlowAuditConfig,
    DialogueSelectorTranslationAuditConfig, DialogueSelectorTranslationInitConfig,
    DialogueSelectorTranslationRefreshConfig, DialogueSelectorTranslationScope,
    DialogueSourceCorpusConfig, DialogueTranslationAssetAuditConfig,
    DialogueTranslationAssetSyncConfig, DialogueTranslationAuditConfig,
    DialogueTranslationDecisionHashConfig, DialogueTranslationInitConfig,
    DialogueTranslationRefreshConfig, DialogueTranslationScope, NameCompanionReferenceAuditConfig,
    apply_dialogue_codebook_review, audit_dialogue_assets, audit_dialogue_build_capacity,
    audit_dialogue_codebook, audit_dialogue_consumer_union, audit_dialogue_development_runtime,
    audit_dialogue_font_allocation, audit_dialogue_font_conflicts, audit_dialogue_glossary,
    audit_dialogue_layout, audit_dialogue_name_entry, audit_dialogue_scenes,
    audit_dialogue_selector_address_flow, audit_dialogue_selector_translation,
    audit_dialogue_translation, audit_dialogue_translation_assets, audit_name_companion_references,
    build_composed_name_input, build_dialogue_code_allocation, build_dialogue_codebook_review,
    build_dialogue_development_disc, build_dialogue_font_images, build_dialogue_font_previews,
    build_dialogue_name_entry_image, build_dialogue_source_corpus,
    hash_dialogue_translation_decisions, initialize_dialogue_selector_translation,
    initialize_dialogue_translation, prepare_dialogue_message_images,
    refresh_dialogue_selector_translation, refresh_dialogue_translation,
    sync_dialogue_translation_assets,
};
use justice_gakuen2_kr::diary_header::{DiaryHeaderDiscBuildConfig, build_diary_header_disc};
use justice_gakuen2_kr::diary_scene::{
    DiaryLocationGraphicsAuditConfig, audit_diary_location_graphics,
};
use justice_gakuen2_kr::disc_asset_comparison::{
    DiscAssetComparisonConfig, FixedPresentationGlyphSourceAuditConfig,
    audit_fixed_presentation_glyph_sources, compare_disc_assets,
};
use justice_gakuen2_kr::font::{MenuFontPreviewConfig, build_menu_font_preview};
use justice_gakuen2_kr::menu_atlas_labeling::{
    MenuAtlasLabelingConfig, prepare_menu_atlas_labeling,
};
use justice_gakuen2_kr::menu_atlas_migration::{
    MenuAtlasMigrationAuditConfig, audit_menu_atlas_migration,
};
use justice_gakuen2_kr::menu_audit::{
    DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET, MenuCodeAuditConfig, audit_menu_codes,
};
use justice_gakuen2_kr::menu_composition_probe::{
    MenuCompositionProbeBuildConfig, build_menu_composition_probe,
};
use justice_gakuen2_kr::menu_glyph_audit::{MenuGlyphAuditConfig, audit_menu_glyphs};
use justice_gakuen2_kr::menu_runtime_probe::{
    MenuRuntimeProbeBuildConfig, build_menu_runtime_probe,
};
use justice_gakuen2_kr::menu_source_workspace::{
    MenuSourceWorkspaceConfig, initialize_menu_source_workspace,
};
use justice_gakuen2_kr::menu_texture_variant::{
    MenuTextureVariantBuildConfig, build_menu_texture_variant,
};
use justice_gakuen2_kr::mode_descendant_graphics::{
    ModeDescendantGraphicsAuditConfig, ModeDescendantGraphicsBuildConfig,
    audit_mode_descendant_graphics, build_mode_descendant_graphics,
};
use justice_gakuen2_kr::mode_select::{ModeSelectBuildConfig, build_mode_select_assets};
use justice_gakuen2_kr::mode_select_graphics::{
    ModeSelectGraphicsAuditConfig, audit_mode_select_graphics,
};
use justice_gakuen2_kr::options::{
    OptionsAssetAuditConfig, OptionsBuildConfig, OptionsSourceAssetSyncConfig,
    audit_options_assets, build_options_assets, sync_options_source_assets,
};
use justice_gakuen2_kr::pipeline::{
    GlyphProbeConfig, HangulProbeConfig, TextCodeProbeConfig, build_glyph_probe,
    build_hangul_probe, build_text_code_probe,
};
use justice_gakuen2_kr::practical_instruction_graphics::{
    PracticalInstructionGraphicsAuditConfig, PracticalInstructionGraphicsBuildConfig,
    PracticalInstructionRuntimeLayoutAuditConfig, audit_practical_instruction_graphics,
    audit_practical_instruction_runtime_layout, build_practical_instruction_graphics,
};
use justice_gakuen2_kr::runtime_dialogue_glyph_audit::{
    RuntimeDialogueGlyphAuditConfig, audit_runtime_dialogue_glyphs,
};
use justice_gakuen2_kr::runtime_image_audit::{RuntimeImageAuditConfig, audit_runtime_images};
use justice_gakuen2_kr::title_graphics::{TitleGraphicsAuditConfig, audit_title_graphics};
use justice_gakuen2_kr::title_menu::{TitleMenuBuildConfig, build_title_menu_assets};

#[derive(Debug, Parser)]
#[command(name = "justice-gakuen2-kr")]
#[command(about = "Justice Gakuen 2 Korean patch production pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compose selected source-bound MENU contributors from a hash-pinned JSON spec.
    BuildMenuCompositionProbe {
        /// JSON spec binding the source CUE, decoded contributors, and owned output.
        #[arg(long)]
        spec: PathBuf,
        /// Replace only the output files owned by this composition probe.
        #[arg(long)]
        force: bool,
    },
    /// Build a one-record non-release disc probe from a hash-pinned JSON spec.
    BuildMenuRuntimeProbe {
        /// JSON spec binding the base CUE, replacement MENU.BIZ, and owned outputs.
        #[arg(long)]
        spec: PathBuf,
        /// Replace only the output files owned by this probe spec.
        #[arg(long)]
        force: bool,
    },
    /// Restore selected source texture regions in a hash-pinned MENU.BIZ variant.
    BuildMenuTextureVariant {
        /// JSON spec binding the source, input MENU, restored regions, and output.
        #[arg(long)]
        spec: PathBuf,
        /// Replace only the output files owned by this variant spec.
        #[arg(long)]
        force: bool,
    },
    /// Inventory hash-pinned character-select graphics records and embedded TIM images.
    AuditCharacterSelectGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON binding a paired emulator RAM dump and frame to this audit.
        #[arg(long)]
        runtime_evidence_spec: Option<PathBuf>,
        /// Directory for the audit JSON and TIM previews.
        #[arg(long, default_value = "build/character-select-graphics-audit")]
        output_dir: PathBuf,
        /// Replace an existing owned audit directory.
        #[arg(long)]
        force: bool,
    },
    /// Plan source-blank SELP atlas allocations from all sharded character-select assets.
    PlanCharacterSelectAtlas {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Owned JSON report; this command does not modify disc records.
        #[arg(long, default_value = "build/character-select-atlas-plan.json")]
        output: PathBuf,
        /// Replace the existing owned plan report.
        #[arg(long)]
        force: bool,
    },
    /// Build dynamic SELP glyph atlases and their guarded PLSEL consumers.
    BuildCharacterSelectAtlas {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Owned non-release SELP records, PLSEL overlays, previews, and build report.
        #[arg(long, default_value = "build/character-select-atlas")]
        output_dir: PathBuf,
        /// Replace the existing owned output directory.
        #[arg(long)]
        force: bool,
    },
    /// Inventory hash-pinned Gorin, EDIT, and practical-exam descendant graphics records.
    AuditModeDescendantGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Directory for the audit JSON and TIM previews.
        #[arg(long, default_value = "build/mode-descendant-graphics-audit")]
        output_dir: PathBuf,
        /// Replace an existing owned audit directory.
        #[arg(long)]
        force: bool,
    },
    /// Inventory source-bound MODE SELECT graphics without running analysis during a build.
    AuditModeSelectGraphics {
        /// Hash-pinned JSON spec binding the original CUE and owned audit output.
        #[arg(long, default_value = "specs/mode-select-graphics-audit.json")]
        spec: PathBuf,
        /// Replace the audit directory owned by the spec.
        #[arg(long)]
        force: bool,
    },
    /// Build the independently implemented MENU.BIZ glyph proof.
    BuildGlyphProbe {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Directory for rebuilt BIN/CUE and the Rust manifest.
        #[arg(long, default_value = "build/rust")]
        output_dir: PathBuf,
        /// Replace an existing Rust probe output.
        #[arg(long)]
        force: bool,
    },
    /// Build a controlled NEWOPT.BIN string-to-atlas code proof.
    BuildTextCodeProbe {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Directory for rebuilt BIN/CUE and the Rust manifest.
        #[arg(long, default_value = "build/rust-text-code-probe")]
        output_dir: PathBuf,
        /// Replace an existing Rust probe output.
        #[arg(long)]
        force: bool,
    },
    /// Preview a supported hash-pinned font in the confirmed 20x20 menu glyph cell.
    PreviewMenuFont {
        /// Exact Maplestory Light or Maplestory Bold TTF input.
        #[arg(long)]
        font: PathBuf,
        /// Directory for the PNG preview and JSON fit report.
        #[arg(long, default_value = "build/font-preview")]
        output_dir: PathBuf,
        /// Comma-separated font sizes to compare.
        #[arg(long, value_delimiter = ',', default_value = "13,14,15")]
        sizes: Vec<f32>,
        /// Representative glyphs to render in each size.
        #[arg(
            long,
            default_value = "가나다라마바사아자차카타파하옵션공격력난이도진동"
        )]
        characters: String,
    },
    /// Audit whole-disc loaded-image reachability before semantic sink classification.
    AuditStaticConsumers {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Ignored investigation JSON; never a product-build input.
        #[arg(long, default_value = "work/consumer-analysis/static-consumers.json")]
        output: PathBuf,
        /// Maximum path-sensitive value-flow states admitted for each reachable LUI seed.
        #[arg(
            long,
            default_value_t = DEFAULT_STATIC_CONSUMER_VALUE_FLOW_STATE_BUDGET
        )]
        value_flow_state_budget: usize,
        /// Stronger per-seed budget used only for declared pointer-run images.
        #[arg(
            long,
            default_value_t = DEFAULT_POINTER_RUN_VALUE_FLOW_STATE_BUDGET
        )]
        pointer_run_value_flow_state_budget: usize,
    },
    /// Audit candidate menu-code strings across DAT1 overlays.
    AuditMenuCodes {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "work/consumer-analysis/menu-code-audit.json")]
        output: PathBuf,
        /// Maximum path-sensitive flow states admitted for each LUI seed.
        #[arg(long, default_value_t = DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)]
        address_flow_state_budget: usize,
    },
    /// Match MENU.BIZ physical cells against the verified dialogue pixel codebook.
    AuditMenuGlyphs {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/menu-glyph-audit.json")]
        output: PathBuf,
    },
    /// Prepare a source-bound free-rectangle labeling editor for the shared MENU atlas.
    PrepareMenuAtlasLabeling {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Optional previously exported label JSON to validate and reopen.
        #[arg(long)]
        labels: Option<PathBuf>,
        /// Optional source-bound migration-risk JSON to embed as a review queue.
        #[arg(long)]
        migration_risk: Option<PathBuf>,
        /// Ignored local editor, source atlas PNG, code map, and evidence report.
        #[arg(long, default_value = "work/menu-atlas-labeling")]
        output_dir: PathBuf,
        /// Replace only an existing owned labeling workspace.
        #[arg(long)]
        force: bool,
        /// Maximum path-sensitive flow states admitted for each LUI seed.
        #[arg(long, default_value_t = DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)]
        address_flow_state_budget: usize,
    },
    /// Cross the last candidate's global MENU writes against source consumers and labels.
    AuditMenuAtlasMigration {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Last candidate build whose three shared-atlas writers will be audited.
        #[arg(long)]
        candidate_build_dir: PathBuf,
        /// Source-bound free-rectangle labels exported by the labeling workspace.
        #[arg(long)]
        labels: PathBuf,
        /// Ignored local migration-obligation JSON; a sibling overlay PNG is also written.
        #[arg(long, default_value = "work/menu-atlas-labeling/migration-risk.json")]
        output: PathBuf,
        /// Replace existing migration JSON and overlay PNG.
        #[arg(long)]
        force: bool,
        /// Maximum path-sensitive flow states admitted for each LUI seed.
        #[arg(long, default_value_t = DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)]
        address_flow_state_budget: usize,
    },
    /// Build sharded menu source candidates without generating translations.
    InitMenuSourceWorkspace {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Local sharded source workspace.
        #[arg(long, default_value = "work/menu-source")]
        output: PathBuf,
        /// Replace an existing workspace after explicitly preserving any work.
        #[arg(long)]
        force: bool,
        /// Maximum register-flow states processed from each LUI seed.
        #[arg(long, default_value_t = DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)]
        address_flow_state_budget: usize,
    },
    /// Extract every pointer-backed KOUBAI source record without generating translations.
    InitBonusShopTextSource {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Local sharded source workspace and contact sheets.
        #[arg(long, default_value = "work/bonus-shop-text-source")]
        output: PathBuf,
        /// Replace an existing source-only workspace.
        #[arg(long)]
        force: bool,
    },
    /// Add every unique pointer-backed KOUBAI record to tracked translation assets.
    SyncBonusShopTextSourceAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Tracked source-bound bonus-shop translation assets.
        #[arg(long, default_value = "assets/menu/shop-text")]
        assets: PathBuf,
    },
    /// Audit tracked KOUBAI translation assets without requiring translation completion.
    AuditBonusShopTextAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        dialogue_codebook: PathBuf,
        /// Tracked source-bound bonus-shop translation assets.
        #[arg(long, default_value = "assets/menu/shop-text")]
        assets: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/bonus-shop-text-asset-audit.json")]
        output: PathBuf,
    },
    /// Validate the tracked options-screen translations against original media.
    AuditOptionsAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-bound options-screen translations.
        #[arg(long, default_value = "assets/menu/options")]
        assets: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/options-asset-audit.json")]
        output: PathBuf,
    },
    /// Add every unique pointer-backed NEWOPT source record to the tracked assets.
    SyncOptionsSourceAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-bound NEWOPT assets.
        #[arg(long, default_value = "assets/menu/options")]
        assets: PathBuf,
    },
    /// Build a non-release Korean options-screen development disc.
    BuildOptionsAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the rebuilt development BIN/CUE and manifest.
        #[arg(long, default_value = "build/options-development")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Build a non-release diary-header-only development disc.
    BuildDiaryHeaderDisc {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the rebuilt development BIN/CUE and manifest.
        #[arg(long, default_value = "build/diary-header-development")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Catalog and preview every diary scene graphic and its runtime bundle consumers.
    AuditDiaryLocationGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Directory for source previews, catalogues, and runtime-binding shards.
        #[arg(long, default_value = "work/diary-location-graphics")]
        output_dir: PathBuf,
    },
    /// Catalog and preview every practical-exam instruction graphic in TESTMJ.TIZ.
    AuditPracticalInstructionGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Owned directory for member identities and TIM previews.
        #[arg(long, default_value = "work/practical-instruction-graphics")]
        output_dir: PathBuf,
        /// Replace a prior output carrying this command's ownership marker.
        #[arg(long)]
        force: bool,
    },
    /// Derive one TESTMJ member's native tile presentation from a frozen runtime frame.
    AuditPracticalInstructionRuntimeLayout {
        /// Exact original single-track MODE2/2352 CUE used to identify the source member.
        #[arg(long)]
        source_cue: PathBuf,
        /// TESTMJ member selected in the paired frozen runtime frame.
        #[arg(long)]
        member_index: usize,
        /// Exact full 2 MiB PS1 RAM dump paired with the frame.
        #[arg(long)]
        ram_dump: PathBuf,
        /// Exact full 1 MiB PS1 GPU dump paired with the frame.
        #[arg(long)]
        gpu_dump: PathBuf,
        /// PNG captured from the same frozen runtime frame.
        #[arg(long)]
        runtime_frame: PathBuf,
        /// JSON evidence output.
        #[arg(long, default_value = "work/practical-instruction-runtime-layout.json")]
        output: PathBuf,
        /// Replace an existing output file.
        #[arg(long)]
        force: bool,
    },
    /// Build source-bound Korean practical-exam instruction panels in TESTMJ.TIZ.
    BuildPracticalInstructionGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Source-bound 33-member instruction manifest and authored translations.
        #[arg(long, default_value = "assets/menu/practical-instructions")]
        assets: PathBuf,
        /// Directory for the rebuilt TESTMJ.TIZ, previews, and build report.
        #[arg(long, default_value = "build/practical-instruction-graphics")]
        output_dir: PathBuf,
        /// Replace a prior output carrying this command's ownership marker.
        #[arg(long)]
        force: bool,
    },
    /// Bind untranslated title-logo source cells to the exact frozen emucap VRAM state.
    AuditTitleGraphics {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked source-bound title graphic assets.
        #[arg(long, default_value = "assets/menu/title-graphics")]
        assets: PathBuf,
        /// Frozen emucap 1024x512 16-bit GPU dump paired with the frame.
        #[arg(long)]
        gpu_dump: PathBuf,
        /// PNG frame captured without guest advance from the same frozen state.
        #[arg(long)]
        runtime_frame: PathBuf,
        /// Directory for source previews and the runtime audit report.
        #[arg(long, default_value = "work/title-graphics-audit")]
        output_dir: PathBuf,
        /// Replace only the audit's owned outputs.
        #[arg(long)]
        force: bool,
    },
    /// Enumerate every pointer-defined dialogue runtime image on the supported disc.
    AuditDialogueAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/dialogue-asset-audit.json")]
        output: PathBuf,
    },
    /// Audit the disjoint union of primary and selector dialogue consumers.
    AuditDialogueConsumerUnion {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked full-population glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// JSON consumer-union audit output.
        #[arg(long, default_value = "build/dialogue-consumer-union-audit.json")]
        output: PathBuf,
    },
    /// Trace typed MGAME construction of the dialogue selector-table base.
    AuditDialogueSelectorAddressFlow {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON address-flow audit output.
        #[arg(
            long,
            default_value = "build/dialogue-selector-address-flow-audit.json"
        )]
        output: PathBuf,
    },
    /// Catalog compressed runtime images and their selector-table signatures.
    AuditRuntimeImages {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/runtime-image-audit.json")]
        output: PathBuf,
    },
    /// Cross one PS1 RAM/GPU snapshot against original dialogue cells and live draw packets.
    AuditRuntimeDialogueGlyphs {
        /// Exact original decoded dialogue runtime image containing the source atlas.
        #[arg(long)]
        source_decoded: PathBuf,
        /// Exact patched decoded dialogue image expected at runtime address 0x800d0000.
        #[arg(long)]
        active_decoded: Option<PathBuf>,
        /// Current dialogue code-allocation JSON that records fixed-cell reuse.
        #[arg(long)]
        allocation: PathBuf,
        /// Tracked source-pixel-verified dialogue glyph codebook.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Exact full 1 MiB PS1 GPU memory dump for one frozen frame.
        #[arg(long)]
        gpu_dump: PathBuf,
        /// Exact full 2 MiB PS1 RAM dump paired with the GPU dump.
        #[arg(long)]
        ram_dump: PathBuf,
        /// Source dialogue runtime image path inside the disc.
        #[arg(long, default_value = "DAT2/MGK04.BIZ")]
        source_path: String,
        /// JSON audit output.
        #[arg(long, default_value = "work/runtime-dialogue-glyph-audit.json")]
        output: PathBuf,
    },
    /// Locate exact runtime Japanese/symbol glyph pixels across every supported embedded TIM.
    AuditFixedPresentationGlyphSources {
        /// Exact supported original single-track MODE2/2352 CUE.
        #[arg(long)]
        source_cue: PathBuf,
        /// Exact patched single-track MODE2/2352 CUE to compare.
        #[arg(long)]
        patched_cue: PathBuf,
        /// Original decoded dialogue image used to resolve runtime-report source codes.
        #[arg(long)]
        source_decoded: PathBuf,
        /// Source-pixel codebook used to extend the scan beyond the observed frame.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Current dialogue allocation whose asset set classifies dialogue font matches.
        #[arg(long)]
        allocation: PathBuf,
        /// Output from audit-runtime-dialogue-glyphs for one exact RAM/GPU frame.
        #[arg(long)]
        runtime_report: PathBuf,
        /// Optional exact 1 MiB GPU dump used to bind whole TIM image residency.
        #[arg(long)]
        runtime_gpu_dump: Option<PathBuf>,
        /// Optional exact 2 MiB RAM dump paired with the GPU dump for draw-packet binding.
        #[arg(long)]
        runtime_ram_dump: Option<PathBuf>,
        /// Search stored 4-bpp TIMs for exact runtime sprite textures at or above this area.
        #[arg(long)]
        runtime_texture_source_min_pixel_count: Option<usize>,
        /// Owned report directory with JSON and highlighted candidate TIM previews.
        #[arg(long, default_value = "work/fixed-presentation-glyph-source-audit")]
        output_dir: PathBuf,
        /// Replace an existing directory owned by this audit command.
        #[arg(long)]
        force: bool,
    },
    /// Compare every ISO record and preview changed embedded TIM assets side by side.
    CompareDiscAssets {
        /// Exact supported original single-track MODE2/2352 CUE.
        #[arg(long)]
        source_cue: PathBuf,
        /// Exact patched single-track MODE2/2352 CUE to compare.
        #[arg(long)]
        patched_cue: PathBuf,
        /// Local directory for report.json, index.html, and PNG comparisons.
        #[arg(long, default_value = "work/disc-asset-comparison")]
        output_dir: PathBuf,
        /// Replace an existing directory owned by this comparison command.
        #[arg(long)]
        force: bool,
    },
    /// Render every runtime-image-scoped dialogue glyph as a contact sheet.
    PreviewDialogueFonts {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Directory for PNG contact sheets and their manifest.
        #[arg(long, default_value = "build/dialogue-font-previews")]
        output_dir: PathBuf,
    },
    /// Verify reviewed dialogue glyph mappings against exact source pixels.
    AuditDialogueCodebook {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// JSON coverage audit output.
        #[arg(long, default_value = "build/dialogue-codebook-audit.json")]
        output: PathBuf,
    },
    /// Apply a source-pixel review shard to the tracked dialogue codebook.
    ApplyDialogueCodebookReview {
        /// Tracked glyph codebook JSON reviewed at the bound SHA-256.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked page-scoped manual review decisions.
        #[arg(long)]
        review: PathBuf,
        /// Generated page-scoped runtime dialogue context evidence.
        #[arg(long)]
        context_shard: PathBuf,
        /// Generated source-pixel review PNG paired with the context evidence.
        #[arg(long)]
        review_png: PathBuf,
        /// Updated codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        output: PathBuf,
        /// Replace an existing output after every bound input has been checked.
        #[arg(long)]
        force: bool,
    },
    /// Render unresolved dialogue glyphs in deterministic review order.
    PreviewDialogueCodebookReview {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Directory for review PNG pages and their manifest.
        #[arg(long, default_value = "build/dialogue-codebook-review")]
        output_dir: PathBuf,
    },
    /// Build a reversible Japanese source corpus for every dialogue runtime image.
    BuildDialogueSourceCorpus {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Generated local sharded source corpus directory.
        #[arg(long, default_value = "build/dialogue-source-corpus")]
        output_dir: PathBuf,
    },
    /// Initialize a local sharded Korean translation workspace from executable references.
    InitDialogueTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Local sharded workspace containing copyrighted source text.
        #[arg(long, default_value = "work/dialogue-translation")]
        output: PathBuf,
        /// Replace an existing workspace after authored work is preserved.
        #[arg(long)]
        force: bool,
        /// Executable-reference population to materialize.
        #[arg(long, value_enum, default_value_t = DialogueTranslationScope::MgkDevelopment)]
        scope: DialogueTranslationScope,
    },
    /// Initialize selector-bank source, context, translation, and review shards.
    InitDialogueSelectorTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Local sharded workspace containing copyrighted selector-bank text.
        #[arg(long, default_value = "work/dialogue-selector-translation")]
        output: PathBuf,
        /// Source population used to acquire selector banks 2 through 6.
        #[arg(long, value_enum, default_value_t = DialogueSelectorTranslationScope::MgkDevelopment)]
        scope: DialogueSelectorTranslationScope,
        /// Replace an existing workspace after authored work is preserved.
        #[arg(long)]
        force: bool,
    },
    /// Rebuild selector source evidence while preserving compatible authored decisions.
    RefreshDialogueSelectorTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Existing internally consistent selector translation workspace.
        #[arg(long)]
        previous: PathBuf,
        /// Separate destination for the refreshed workspace.
        #[arg(long, default_value = "work/dialogue-selector-translation-refreshed")]
        output: PathBuf,
        /// Source population used to rebuild selector evidence.
        #[arg(long, value_enum, default_value_t = DialogueSelectorTranslationScope::MgkDevelopment)]
        scope: DialogueSelectorTranslationScope,
        /// Replace only the separate destination, never the previous workspace.
        #[arg(long)]
        force: bool,
    },
    /// Validate selector-bank Korean decisions against source and consumer evidence.
    AuditDialogueSelectorTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Authored local sharded selector translation workspace.
        #[arg(long)]
        translation: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/dialogue-selector-translation-audit.json")]
        output: PathBuf,
    },
    /// Rebuild immutable source/context shards and preserve compatible authored decisions.
    RefreshDialogueTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Updated tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Existing internally consistent translation workspace.
        #[arg(long)]
        previous: PathBuf,
        /// Separate destination for the refreshed workspace.
        #[arg(long, default_value = "work/dialogue-translation-refreshed")]
        output: PathBuf,
        /// Replace only the separate destination, never the previous workspace.
        #[arg(long)]
        force: bool,
        /// Executable-reference population to materialize in the destination.
        #[arg(long, value_enum, default_value_t = DialogueTranslationScope::MgkDevelopment)]
        scope: DialogueTranslationScope,
    },
    /// Validate Korean translation decisions against original media and codebook.
    AuditDialogueTranslation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Authored local sharded Korean translation workspace.
        #[arg(long)]
        translation: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        output: PathBuf,
    },
    /// Materialize unique source-plus-Korean decisions as tracked development assets.
    SyncDialogueTranslationAssets {
        /// Resolved-primary source/context/decision workspace.
        #[arg(long)]
        primary_workspace: PathBuf,
        /// All-runtime selector source/context/decision workspace.
        #[arg(long)]
        selector_workspace: PathBuf,
        /// Tracked source-plus-Korean decision asset root.
        #[arg(long, default_value = "assets/dialogue/translations")]
        output: PathBuf,
        /// Replace existing generated assets after the new tree passes its audit.
        #[arg(long)]
        force: bool,
    },
    /// Audit tracked dialogue translation assets without requiring original media.
    AuditDialogueTranslationAssets {
        /// Tracked source-plus-Korean decision asset root.
        #[arg(long, default_value = "assets/dialogue/translations")]
        input: PathBuf,
    },
    /// Print the per-decision hashes used by independent translation reviews.
    HashDialogueTranslationDecisions {
        /// One Korean dialogue translation shard.
        #[arg(long)]
        input: PathBuf,
    },
    /// Measure every authored Korean message against the source-bound dialogue window.
    AuditDialogueLayout {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy local workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Translation integrity audit emitted before layout measurement.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        translation_audit_output: PathBuf,
        /// Reject translations that lack current independent approval.
        #[arg(long)]
        require_approved: bool,
        /// JSON layout audit output.
        #[arg(long, default_value = "build/dialogue-layout-audit.json")]
        output: PathBuf,
    },
    /// Classify per-asset development glyph demand against source and extension cells.
    AuditDialogueFontAllocation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy local workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Translation integrity audit emitted before allocation analysis.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        translation_audit_output: PathBuf,
        /// JSON allocation audit output.
        #[arg(long, default_value = "build/dialogue-font-allocation-audit.json")]
        output: PathBuf,
    },
    /// Measure authored Korean message storage against each fixed MGK bank region.
    AuditDialogueBuildCapacity {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy local workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Translation integrity audit emitted before build-capacity analysis.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        translation_audit_output: PathBuf,
        /// JSON build-capacity audit output.
        #[arg(long, default_value = "build/dialogue-build-capacity-audit.json")]
        output: PathBuf,
    },
    /// Allocate deterministic asset-local dialogue codes and reserve global name codes.
    BuildDialogueCodeAllocation {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy local workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy selector workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        selector_translation: PathBuf,
        /// Tracked Korean name-input keyboard and shared glyph-cell layout.
        #[arg(long, default_value = "assets/dialogue/name-entry/keyboard.json")]
        name_input_keyboard: PathBuf,
        /// Translation integrity audit emitted before allocation.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        translation_audit_output: PathBuf,
        /// Selector translation audit emitted before allocation.
        #[arg(long, default_value = "build/dialogue-selector-translation-audit.json")]
        selector_translation_audit_output: PathBuf,
        /// JSON code allocation output.
        #[arg(long, default_value = "build/dialogue-code-allocation.json")]
        output: PathBuf,
        /// Whether development requires every scoped unit or preserves blank units.
        #[arg(long, value_enum, default_value_t = DialogueDevelopmentInputPolicy::CompleteScope)]
        input_policy: DialogueDevelopmentInputPolicy,
    },
    /// Attribute protected source-glyph conflicts to exact untranslated dialogue groups.
    AuditDialogueFontConflicts {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked full-population glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a resolved-primary workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Tracked source-bound Korean translation assets or an all-runtime selector workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        selector_translation: PathBuf,
        /// Tracked Korean name-input keyboard and shared glyph-cell layout.
        #[arg(long, default_value = "assets/dialogue/name-entry/keyboard.json")]
        name_input_keyboard: PathBuf,
        /// Translation integrity audit emitted before attribution.
        #[arg(
            long,
            default_value = "build/dialogue-font-conflicts-translation-audit.json"
        )]
        translation_audit_output: PathBuf,
        /// Selector translation audit emitted before attribution.
        #[arg(
            long,
            default_value = "build/dialogue-font-conflicts-selector-audit.json"
        )]
        selector_translation_audit_output: PathBuf,
        /// Development code allocation used as the conflict baseline.
        #[arg(
            long,
            default_value = "build/dialogue-font-conflicts-code-allocation.json"
        )]
        code_allocation_output: PathBuf,
        /// Sharded conflict attribution output.
        #[arg(long, default_value = "build/dialogue-font-conflicts")]
        output_dir: PathBuf,
        /// Replace an existing attribution output directory.
        #[arg(long)]
        force: bool,
    },
    /// Analyze authored text and export parse-verified message images for diagnostics.
    PrepareDialogueMessages {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy local workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        translation: PathBuf,
        /// Tracked source-bound Korean translation assets or a legacy selector workspace.
        #[arg(long, default_value = "assets/dialogue/translations")]
        selector_translation: PathBuf,
        /// Tracked Korean name-input keyboard and shared glyph-cell layout.
        #[arg(long, default_value = "assets/dialogue/name-entry/keyboard.json")]
        name_input_keyboard: PathBuf,
        /// Translation integrity audit emitted before building.
        #[arg(long, default_value = "build/dialogue-translation-audit.json")]
        translation_audit_output: PathBuf,
        /// Selector translation audit emitted before building.
        #[arg(long, default_value = "build/dialogue-selector-translation-audit.json")]
        selector_translation_audit_output: PathBuf,
        /// Deterministic development code allocation emitted before building.
        #[arg(long, default_value = "build/dialogue-code-allocation.json")]
        code_allocation_output: PathBuf,
        /// Directory for decoded images, compressed records, and the build manifest.
        #[arg(long, default_value = "build/dialogue-messages")]
        output_dir: PathBuf,
        /// Whether development requires every scoped unit or preserves blank units.
        #[arg(long, value_enum, default_value_t = DialogueDevelopmentInputPolicy::CompleteScope)]
        input_policy: DialogueDevelopmentInputPolicy,
        /// Replace an existing output directory.
        #[arg(long)]
        force: bool,
    },
    /// Analyze translations and fix message/code placement for subsequent builds.
    PrepareDialogueInputs {
        #[arg(long)]
        cue: PathBuf,
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        #[arg(long, value_enum, default_value_t = DialogueDevelopmentInputPolicy::CompleteScope)]
        input_policy: DialogueDevelopmentInputPolicy,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Build font-installed non-release assets from the selected development input policy.
    BuildDialogueFonts {
        /// Explicit output of prepare-dialogue-inputs; never generated by this build.
        #[arg(long)]
        prepared_dialogue: PathBuf,
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Whether development requires every scoped unit or preserves blank units.
        #[arg(long, value_enum, default_value_t = DialogueDevelopmentInputPolicy::CompleteScope)]
        input_policy: DialogueDevelopmentInputPolicy,
        /// Directory for font-installed decoded images, compressed records, and manifests.
        #[arg(long, default_value = "build/dialogue-fonts")]
        output_dir: PathBuf,
        /// Replace owned outputs in an existing directory.
        #[arg(long)]
        force: bool,
    },
    /// Render and rebuild the selected loading portraits and message variants.
    BuildLoadingArt {
        #[arg(long)]
        cue: PathBuf,
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    /// Render native staff-roll text and geometry without a disc build.
    BuildStaffRoll {
        #[arg(long)]
        cue: PathBuf,
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    /// Render all selected card artwork into CARD.BIZ and previews without a ROM build.
    BuildCardArt {
        #[arg(long)]
        cue: PathBuf,
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    /// Build a readback-verified non-release Korean development BIN/CUE.
    BuildDialogueDisc {
        /// Explicit output of prepare-dialogue-inputs; never generated by this build.
        #[arg(long)]
        prepared_dialogue: PathBuf,
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Whether development requires every scoped unit or preserves blank units.
        #[arg(long, value_enum, default_value_t = DialogueDevelopmentInputPolicy::CompleteScope)]
        input_policy: DialogueDevelopmentInputPolicy,
        /// Directory for the rebuilt development BIN/CUE and manifests.
        #[arg(long, default_value = "build/dialogue-disc")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Build source-bound Korean MODE SELECT graphics as a standalone MENU.BIZ record.
    BuildModeSelectAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the rebuilt MENU.BIZ and build manifest.
        #[arg(long, default_value = "build/mode-select")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Build fixed and practical mode-descendant graphics from the shared JSON spec.
    BuildModeDescendantAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for compressed producers, raw consumers, previews, and the build manifest.
        #[arg(long, default_value = "build/mode-descendants")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Build the title-adjacent new-enrollment menu and its MENU.BIZ glyphs.
    BuildTitleMenuAssets {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the rebuilt MENU.BIZ, MGTIT.BIZ, and build manifest.
        #[arg(long, default_value = "build/title-menu")]
        output_dir: PathBuf,
        /// Replace owned outputs in the destination.
        #[arg(long)]
        force: bool,
    },
    /// Bind a development disc readback to its loaded BZZ member and emucap RAM image.
    AuditDialogueDevelopmentRuntime {
        /// Readback manifest emitted by build-dialogue-disc.
        #[arg(long)]
        disc_report: PathBuf,
        /// Exact development BIN launched by emucap.
        #[arg(long)]
        disc_bin: PathBuf,
        /// Full 2 MiB RAM dump captured with the verified dialogue frame.
        #[arg(long)]
        runtime_ram: PathBuf,
        /// PNG frame paired with the RAM dump.
        #[arg(long)]
        runtime_frame: PathBuf,
        /// BIZ record whose individual and bundled copies must agree.
        #[arg(long, default_value = "DAT2/MGG04T.BIZ")]
        source_path: String,
        /// Exact emucap launch that produced the paired evidence.
        #[arg(long)]
        launch_id: String,
        /// Exact emulator implementation/build reported by emucap.
        #[arg(long)]
        emulator_build: String,
        /// Exact emucap capability revision in sha256:<digest> form.
        #[arg(long)]
        capability_revision: String,
        /// JSON runtime audit output.
        #[arg(long, default_value = "build/dialogue-development-runtime-audit.json")]
        output: PathBuf,
    },
    /// Audit the original enrollment name pages and stored-buffer capacities.
    AuditDialogueNameEntry {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON name-entry audit output.
        #[arg(long, default_value = "build/dialogue-name-entry-audit.json")]
        output: PathBuf,
    },
    /// Offline source-reference census for runtime memory ownership review.
    AuditMemoryReferences {
        #[arg(long)]
        cue: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Half-open hexadecimal start:end; repeat for multiple regions.
        #[arg(long = "range", required = true)]
        ranges: Vec<String>,
        #[arg(long, default_value_t = 8192)]
        state_budget: usize,
    },
    /// Find source-bound static accesses to the nickname companion buffer.
    AuditNameCompanionReferences {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// JSON static-reference audit output.
        #[arg(
            long,
            default_value = "work/consumer-analysis/name-companion-reference-audit.json"
        )]
        output: PathBuf,
        /// Maximum path-sensitive flow states admitted for each LUI seed.
        #[arg(long, default_value_t = DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)]
        address_flow_state_budget: usize,
    },
    /// Build a non-release Korean name-entry overlay from the tracked candidate pages.
    BuildDialogueNameEntry {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets, hash-pinned fonts, and surface-specific roles.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the patched overlay and build manifest.
        #[arg(long, default_value = "build/dialogue-name-entry")]
        output_dir: PathBuf,
        /// Replace owned outputs in an existing directory.
        #[arg(long)]
        force: bool,
    },
    /// Build the three-page composed Korean name-input assets.
    BuildComposedNameInput {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Shared development assets and role-specific font settings.
        #[arg(long, default_value = "assets/build/development.json")]
        spec: PathBuf,
        /// Directory for the patched overlay, font image, and manifest.
        #[arg(long, default_value = "build/composed-name-input")]
        output_dir: PathBuf,
        /// Replace owned outputs in an existing directory.
        #[arg(long)]
        force: bool,
    },
    /// Validate tracked Korean terminology against the exact dialogue source.
    AuditDialogueGlossary {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Tracked, source-bound candidate and approved terminology.
        #[arg(long, default_value = "assets/dialogue/glossary.json")]
        glossary: PathBuf,
        /// JSON audit output.
        #[arg(long, default_value = "build/dialogue-glossary-audit.json")]
        output: PathBuf,
    },
    /// Audit all primary dialogue graphs and join the emucap-observed MGK scene.
    AuditDialogueScenes {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Tracked glyph codebook JSON.
        #[arg(long, default_value = "assets/dialogue/codebook.json")]
        codebook: PathBuf,
        /// Exact emucap RAM dump captured while the first message is visible.
        #[arg(long)]
        runtime_ram: PathBuf,
        /// Exact emucap frame paired with the RAM dump.
        #[arg(long)]
        runtime_frame: PathBuf,
        /// JSON scene audit output.
        #[arg(long, default_value = "build/dialogue-scene-audit.json")]
        output: PathBuf,
    },
    /// Build a Rust-only Maplestory Light 공격력 proof in the options menu.
    BuildHangulProbe {
        /// Exact original single-track MODE2/2352 CUE.
        #[arg(long)]
        cue: PathBuf,
        /// Exact Maplestory Light TTF input.
        #[arg(long)]
        font: PathBuf,
        /// Directory for rebuilt BIN/CUE and the Rust manifest.
        #[arg(long, default_value = "build/rust-hangul-probe")]
        output_dir: PathBuf,
        /// Replace an existing Rust Hangul probe output.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::BuildMenuCompositionProbe { spec, force } => {
            let report =
                build_menu_composition_probe(&MenuCompositionProbeBuildConfig { spec, force })?;
            println!(
                "built {} ({} contributors, compression roundtrip: {})",
                report.output_menu_path,
                report.contributors.len(),
                report.compression_roundtrip_verified
            );
        }
        Command::BuildMenuRuntimeProbe { spec, force } => {
            let report = build_menu_runtime_probe(&MenuRuntimeProbeBuildConfig { spec, force })?;
            println!(
                "built {} ({} changed MENU sectors, readback: {})",
                report.output_cue,
                report.changed_lbas.len(),
                report.readback_matches_replacement
            );
        }
        Command::BuildMenuTextureVariant { spec, force } => {
            let report =
                build_menu_texture_variant(&MenuTextureVariantBuildConfig { spec, force })?;
            println!(
                "built {} ({} source regions restored, compression roundtrip: {})",
                report.output_menu_path,
                report.restored_region_count,
                report.compression_roundtrip_verified
            );
        }
        Command::AuditCharacterSelectGraphics {
            cue,
            runtime_evidence_spec,
            output_dir,
            force,
        } => {
            let report = audit_character_select_graphics(&CharacterSelectGraphicsAuditConfig {
                cue,
                runtime_evidence_spec,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} select records / {} TIMs, {} auxiliary records / {} TIMs)",
                output_dir.display(),
                report.record_count,
                report.tim_count,
                report.auxiliary_record_count,
                report.auxiliary_tim_count
            );
        }
        Command::PlanCharacterSelectAtlas {
            cue,
            spec,
            output,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = write_character_select_atlas_plan(&CharacterSelectAtlasPlanConfig {
                cue,
                assets: loaded_spec.assets.character_select,
                fonts: loaded_spec.fonts.character_select,
                build_spec_sha256: loaded_spec.sha256,
                output: output.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} glyphs, {} runtime-composed entries, {} unresolved route occurrences, {} bound strips, {} select-heading cells on one shared texture page, {} shared SELP records)",
                output.display(),
                report.atlas.required_glyph_count,
                report.atlas.runtime_composed_texture_entry_count,
                report.atlas.route_census.unresolved_route_occurrence_count,
                report.atlas.bound_fixed_texture_strip_count,
                report
                    .atlas
                    .available_select_heading_cell_count_on_one_texture_page,
                report.source_record_count,
            );
        }
        Command::BuildCharacterSelectAtlas {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let build = build_character_select_atlas(&CharacterSelectAtlasBuildConfig {
                native_text_font: loaded_spec.fonts.shared_menu_numerals.clone(),
                cue,
                assets: loaded_spec.assets.character_select,
                fonts: loaded_spec.fonts.character_select,
                build_spec_sha256: loaded_spec.sha256,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} dynamic glyphs in {} SELP and {} auxiliary records with {} guarded PLSEL consumers)",
                output_dir.display(),
                build.report.glyph_count,
                build.report.record_count,
                build.report.auxiliary_record_count,
                build.report.overlays.len(),
            );
        }
        Command::AuditModeDescendantGraphics {
            cue,
            output_dir,
            force,
        } => {
            let report = audit_mode_descendant_graphics(&ModeDescendantGraphicsAuditConfig {
                cue,
                assets: PathBuf::from("assets/menu/mode-descendants"),
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} source records, {} embedded TIM images)",
                output_dir.display(),
                report.record_count,
                report.tim_count
            );
        }
        Command::AuditModeSelectGraphics { spec, force } => {
            let report =
                audit_mode_select_graphics(&ModeSelectGraphicsAuditConfig { spec, force })?;
            println!(
                "inventoried {} embedded MODE SELECT TIM images ({} unclassified)",
                report.embedded_tim_count, report.unclassified_tim_count
            );
        }
        Command::BuildGlyphProbe {
            cue,
            output_dir,
            force,
        } => {
            let manifest = build_glyph_probe(&GlyphProbeConfig {
                cue,
                output_dir,
                force,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::BuildTextCodeProbe {
            cue,
            output_dir,
            force,
        } => {
            let manifest = build_text_code_probe(&TextCodeProbeConfig {
                cue,
                output_dir,
                force,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::PreviewMenuFont {
            font,
            output_dir,
            sizes,
            characters,
        } => {
            let manifest = build_menu_font_preview(&MenuFontPreviewConfig {
                font,
                output_dir,
                sizes,
                characters,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::AuditStaticConsumers {
            cue,
            output,
            value_flow_state_budget,
            pointer_run_value_flow_state_budget,
        } => {
            let report = audit_static_consumers(&StaticConsumerAuditConfig {
                cue,
                output,
                value_flow_state_budget,
                pointer_run_value_flow_state_budget,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditMenuCodes {
            cue,
            output,
            address_flow_state_budget,
        } => {
            let manifest = audit_menu_codes(&MenuCodeAuditConfig {
                cue,
                output,
                address_flow_state_budget,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::AuditMenuGlyphs {
            cue,
            dialogue_codebook,
            output,
        } => {
            let manifest = audit_menu_glyphs(&MenuGlyphAuditConfig {
                cue,
                dialogue_codebook,
                output,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::PrepareMenuAtlasLabeling {
            cue,
            dialogue_codebook,
            labels,
            migration_risk,
            output_dir,
            force,
            address_flow_state_budget,
        } => {
            let report = prepare_menu_atlas_labeling(&MenuAtlasLabelingConfig {
                cue,
                dialogue_codebook,
                labels,
                migration_risk,
                output_dir: output_dir.clone(),
                force,
                address_flow_state_budget,
            })?;
            println!(
                "prepared {} labels with {} statically used logical codes under {}",
                report.label_count,
                report.statically_used_code_count,
                output_dir.display()
            );
        }
        Command::AuditMenuAtlasMigration {
            cue,
            dialogue_codebook,
            candidate_build_dir,
            labels,
            output,
            force,
            address_flow_state_budget,
        } => {
            let report = audit_menu_atlas_migration(&MenuAtlasMigrationAuditConfig {
                cue,
                dialogue_codebook,
                candidate_build_dir,
                labels,
                output: output.clone(),
                force,
                address_flow_state_budget,
            })?;
            println!(
                "recorded {} candidate writes and {} source-code migration obligations in {}",
                report.candidate_write_count,
                report.source_code_migration_obligation_count,
                output.display()
            );
        }
        Command::InitMenuSourceWorkspace {
            cue,
            dialogue_codebook,
            output,
            force,
            address_flow_state_budget,
        } => {
            let manifest = initialize_menu_source_workspace(&MenuSourceWorkspaceConfig {
                cue,
                dialogue_codebook,
                output,
                force,
                address_flow_state_budget,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::InitBonusShopTextSource {
            cue,
            dialogue_codebook,
            output,
            force,
        } => {
            let manifest = initialize_bonus_shop_text_source(&BonusShopTextSourceConfig {
                cue,
                dialogue_codebook,
                output,
                force,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Command::SyncBonusShopTextSourceAssets {
            cue,
            dialogue_codebook,
            assets,
        } => {
            let report = sync_bonus_shop_text_assets(&BonusShopTextAssetSyncConfig {
                cue,
                dialogue_codebook,
                assets,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditBonusShopTextAssets {
            cue,
            dialogue_codebook,
            assets,
            output,
        } => {
            let report = audit_bonus_shop_text_assets(&BonusShopTextAssetAuditConfig {
                cue,
                dialogue_codebook,
                assets,
                output,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditOptionsAssets {
            cue,
            assets,
            output,
        } => {
            let report = audit_options_assets(&OptionsAssetAuditConfig {
                cue,
                assets,
                output,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::SyncOptionsSourceAssets { cue, assets } => {
            let report = sync_options_source_assets(&OptionsSourceAssetSyncConfig { cue, assets })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::BuildOptionsAssets {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = build_options_assets(&OptionsBuildConfig {
                cue,
                assets: loaded_spec.assets.options,
                fonts: loaded_spec.fonts.options,
                build_spec_sha256: loaded_spec.sha256,
                output_dir,
                force,
            })?;
            println!(
                "built {} options units with {} development-only physical cells (development input: {}, release candidate: {})",
                report.records.unit_count,
                report.records.provisional_code_count,
                report.records.development_input_available,
                report.records.release_candidate_input_eligible
            );
        }
        Command::BuildDiaryHeaderDisc {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = build_diary_header_disc(&DiaryHeaderDiscBuildConfig {
                cue,
                assets: loaded_spec.assets.diary_header,
                fonts: loaded_spec.fonts.diary_header,
                build_spec_sha256: loaded_spec.sha256,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} changed sectors, readback: {}, EDC/ECC: {})",
                output_dir.display(),
                report.changed_lbas.len(),
                report.readback_verified,
                report.edc_ecc_verified
            );
        }
        Command::AuditDiaryLocationGraphics { cue, output_dir } => {
            let report = audit_diary_location_graphics(&DiaryLocationGraphicsAuditConfig {
                cue,
                output_dir: output_dir.clone(),
            })?;
            println!(
                "wrote {} files under {} ({} catalogue members, {} location blocks, {} runtime bundles, {} bound runtime members)",
                report.output_files.len(),
                output_dir.display(),
                report.catalogue_member_count,
                report.location_block_member_count,
                report.runtime_bundle_count,
                report.runtime_member_binding_count
            );
        }
        Command::AuditPracticalInstructionGraphics {
            cue,
            output_dir,
            force,
        } => {
            let report =
                audit_practical_instruction_graphics(&PracticalInstructionGraphicsAuditConfig {
                    cue,
                    output_dir: output_dir.clone(),
                    force,
                })?;
            println!(
                "wrote {} ({} TESTMJ members, {} embedded TIMs)",
                output_dir.display(),
                report.member_count,
                report.embedded_tim_count
            );
        }
        Command::AuditPracticalInstructionRuntimeLayout {
            source_cue,
            member_index,
            ram_dump,
            gpu_dump,
            runtime_frame,
            output,
            force,
        } => {
            let report = audit_practical_instruction_runtime_layout(
                &PracticalInstructionRuntimeLayoutAuditConfig {
                    source_cue,
                    member_index,
                    ram_dump,
                    gpu_dump,
                    runtime_frame,
                    output: output.clone(),
                    force,
                },
            )?;
            println!(
                "wrote {} (member {}, {} unique packets, {} rows)",
                output.display(),
                report.member_index,
                report.unique_packet_count,
                report.destination_rows.len()
            );
        }
        Command::BuildPracticalInstructionGraphics {
            cue,
            spec,
            assets,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let build =
                build_practical_instruction_graphics(&PracticalInstructionGraphicsBuildConfig {
                    cue,
                    assets,
                    body_font: loaded_spec.fonts.practical_instruction,
                    build_spec_sha256: loaded_spec.sha256,
                    output_dir: output_dir.clone(),
                    force,
                })?;
            let report = build.report;
            println!(
                "wrote {} (authored: {}, pending: {}, source members: {}, preserved unowned members: {})",
                output_dir.display(),
                report.authored_member_count,
                report.pending_member_count,
                report.member_count,
                report.all_unowned_members_preserved
            );
        }
        Command::AuditTitleGraphics {
            cue,
            assets,
            gpu_dump,
            runtime_frame,
            output_dir,
            force,
        } => {
            let report = audit_title_graphics(&TitleGraphicsAuditConfig {
                cue,
                assets,
                gpu_dump,
                runtime_frame,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} inputs without Korean artwork, {}/{} source pixels resident, exact draw consumer: {}; Korean runtime not checked)",
                output_dir.display(),
                report.untranslated_unit_count,
                report.matching_vram_pixel_count,
                report.source_texture_pixel_count,
                report.exact_draw_consumer_verified,
            );
        }
        Command::AuditDialogueAssets { cue, output } => {
            let manifest = audit_dialogue_assets(&DialogueAuditConfig {
                cue,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} assets, {} banks, {} entries, {} unique entries)",
                output.display(),
                manifest.asset_count,
                manifest.bank_count,
                manifest.entry_count,
                manifest.unique_entry_count
            );
        }
        Command::AuditDialogueConsumerUnion {
            cue,
            codebook,
            output,
        } => {
            let report = audit_dialogue_consumer_union(&DialogueConsumerUnionAuditConfig {
                cue,
                codebook,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} primary groups + {} selector groups = {} consumer groups across {} coordinates; complete-game inventory: {})",
                output.display(),
                report.primary_semantic_group_count,
                report.selector_semantic_group_count,
                report.consumer_union_semantic_group_count,
                report.consumer_union_coordinate_count,
                report.complete_game_text_consumer_inventory,
            );
        }
        Command::AuditDialogueSelectorAddressFlow { cue, output } => {
            let report =
                audit_dialogue_selector_address_flow(&DialogueSelectorAddressFlowAuditConfig {
                    cue,
                    output: output.clone(),
                })?;
            println!(
                "wrote {} ({} typed selector-table base materializations)",
                output.display(),
                report.table_base_materialization_count,
            );
        }
        Command::AuditRuntimeImages { cue, output } => {
            let report = audit_runtime_images(&RuntimeImageAuditConfig {
                cue,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} stored BIZ records, {} decoded, {} selector signatures, {} dialogue runtime images)",
                output.display(),
                report.stored_biz_record_count,
                report.decoded_biz_record_count,
                report.selector_signature_record_count,
                report.dialogue_runtime_image_record_count
            );
        }
        Command::AuditRuntimeDialogueGlyphs {
            source_decoded,
            active_decoded,
            allocation,
            codebook,
            gpu_dump,
            ram_dump,
            source_path,
            output,
        } => {
            let report = audit_runtime_dialogue_glyphs(&RuntimeDialogueGlyphAuditConfig {
                source_decoded,
                active_decoded,
                allocation,
                codebook,
                gpu_dump,
                ram_dump,
                source_path,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} resident source-pixel locations, {} draw-packet pixel matches, {} whose same source codes changed in the allocation)",
                output.display(),
                report.resident_location_count,
                report.draw_consumer_count,
                report.allocation_changed_code_draw_pixel_match_count,
            );
        }
        Command::AuditFixedPresentationGlyphSources {
            source_cue,
            patched_cue,
            source_decoded,
            codebook,
            allocation,
            runtime_report,
            runtime_gpu_dump,
            runtime_ram_dump,
            runtime_texture_source_min_pixel_count,
            output_dir,
            force,
        } => {
            let report =
                audit_fixed_presentation_glyph_sources(&FixedPresentationGlyphSourceAuditConfig {
                    source_cue,
                    patched_cue,
                    source_decoded,
                    codebook,
                    allocation,
                    runtime_report,
                    runtime_gpu_dump,
                    runtime_ram_dump,
                    runtime_texture_source_min_pixel_count,
                    output_dir: output_dir.clone(),
                    force,
                })?;
            println!(
                "wrote {} ({} Japanese-script targets, {} observed at runtime; {} source / {} patched glyph matches; {} / {} exact runtime texture-source matches from {} deduplicated texture targets; {} / {} candidate surfaces in {} exact-source-TIM families, {} duplicated)",
                output_dir.display(),
                report.japanese_script_target_count,
                report.runtime_observed_target_count,
                report.source_match_count,
                report.patched_match_count,
                report.source_runtime_texture_source_match_count,
                report.patched_runtime_texture_source_match_count,
                report.runtime_texture_target_count,
                report.source_presentation_candidate_surface_count,
                report.patched_presentation_candidate_surface_count,
                report.source_presentation_candidate_atlas_family_count,
                report.duplicated_source_presentation_candidate_atlas_family_count,
            );
        }
        Command::CompareDiscAssets {
            source_cue,
            patched_cue,
            output_dir,
            force,
        } => {
            let report = compare_disc_assets(&DiscAssetComparisonConfig {
                source_cue,
                patched_cue,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} ISO records: {} changed / {} unchanged; {} TIM comparisons / {} visually changed; {} decode issues)",
                output_dir.display(),
                report.compared_record_count,
                report.changed_record_count,
                report.unchanged_record_count,
                report.embedded_tim_comparison_count,
                report.visually_changed_tim_count,
                report.decode_issue_count,
            );
        }
        Command::PreviewDialogueFonts { cue, output_dir } => {
            let manifest = build_dialogue_font_previews(&DialogueFontPreviewConfig {
                cue,
                output_dir: output_dir.clone(),
            })?;
            println!(
                "wrote {} ({} asset contact sheets)",
                output_dir.display(),
                manifest.assets.len()
            );
        }
        Command::AuditDialogueCodebook {
            cue,
            codebook,
            output,
        } => {
            let manifest = audit_dialogue_codebook(&DialogueCodebookAuditConfig {
                cue,
                codebook,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} verified, {} candidate, {} unresolved used glyph hashes)",
                output.display(),
                manifest.verified_entry_count,
                manifest.candidate_entry_count,
                manifest.unresolved_used_source_pixel_hash_count
            );
        }
        Command::ApplyDialogueCodebookReview {
            codebook,
            review,
            context_shard,
            review_png,
            output,
            force,
        } => {
            let report =
                apply_dialogue_codebook_review(&DialogueCodebookReviewApplicationConfig {
                    codebook,
                    review,
                    context_shard,
                    review_png,
                    output,
                    force,
                })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::PreviewDialogueCodebookReview {
            cue,
            codebook,
            output_dir,
        } => {
            let manifest = build_dialogue_codebook_review(&DialogueCodebookReviewConfig {
                cue,
                codebook,
                output_dir: output_dir.clone(),
            })?;
            println!(
                "wrote {} ({} pages, {} unresolved used glyph hashes)",
                output_dir.display(),
                manifest.pages.len(),
                manifest.unresolved_used_source_pixel_hash_count
            );
        }
        Command::BuildDialogueSourceCorpus {
            cue,
            codebook,
            output_dir,
        } => {
            let report = build_dialogue_source_corpus(&DialogueSourceCorpusConfig {
                cue,
                codebook,
                output_dir: output_dir.clone(),
            })?;
            println!(
                "wrote {} ({} shards, {} coordinates, {} semantic groups, {} roundtrip verified)",
                output_dir.display(),
                report.shard_count,
                report.coordinate_count,
                report.semantic_shared_group_count,
                report.roundtrip_verified_coordinate_count
            );
        }
        Command::InitDialogueTranslation {
            cue,
            codebook,
            output,
            scope,
            force,
        } => {
            let translation = initialize_dialogue_translation(&DialogueTranslationInitConfig {
                cue,
                codebook,
                output: output.clone(),
                force,
                scope,
            })?;
            println!(
                "wrote {} ({} semantic groups across {} assets, status {:?})",
                output.display(),
                translation.semantic_group_count,
                translation.asset_count,
                translation.project_status
            );
        }
        Command::InitDialogueSelectorTranslation {
            cue,
            codebook,
            output,
            scope,
            force,
        } => {
            let translation =
                initialize_dialogue_selector_translation(&DialogueSelectorTranslationInitConfig {
                    cue,
                    codebook,
                    output: output.clone(),
                    force,
                    scope,
                })?;
            println!(
                "wrote {} ({} selector groups, {} authored targets, {} source-bound runtime rewrites, {} consumer-pending groups)",
                output.display(),
                translation.semantic_group_count,
                translation.authored_translation_target_group_count,
                translation.runtime_insertion_rewrite_group_count,
                translation.consumer_pending_group_count
            );
        }
        Command::RefreshDialogueSelectorTranslation {
            cue,
            codebook,
            previous,
            output,
            scope,
            force,
        } => {
            let translation =
                refresh_dialogue_selector_translation(&DialogueSelectorTranslationRefreshConfig {
                    cue,
                    codebook,
                    previous,
                    output: output.clone(),
                    force,
                    scope,
                })?;
            println!(
                "wrote {} ({} compatible selector groups, {} coordinates)",
                output.display(),
                translation.semantic_group_count,
                translation.coordinate_count
            );
        }
        Command::AuditDialogueSelectorTranslation {
            cue,
            codebook,
            translation,
            output,
        } => {
            let report =
                audit_dialogue_selector_translation(&DialogueSelectorTranslationAuditConfig {
                    cue,
                    codebook,
                    translation,
                    output: output.clone(),
                })?;
            println!(
                "wrote {} ({} groups, {} development-authored, full selector input: {}, release-candidate selector input: {})",
                output.display(),
                report.semantic_group_count,
                report.development_authored_group_count,
                report.development_full_selector_input_available,
                report.release_candidate_selector_input_eligible
            );
        }
        Command::RefreshDialogueTranslation {
            cue,
            codebook,
            previous,
            output,
            force,
            scope,
        } => {
            let translation = refresh_dialogue_translation(&DialogueTranslationRefreshConfig {
                cue,
                codebook,
                previous,
                output: output.clone(),
                force,
                scope,
            })?;
            println!(
                "wrote {} ({} compatible semantic groups across {} assets, status {:?})",
                output.display(),
                translation.semantic_group_count,
                translation.asset_count,
                translation.project_status
            );
        }
        Command::AuditDialogueTranslation {
            cue,
            codebook,
            translation,
            output,
        } => {
            let report = audit_dialogue_translation(&DialogueTranslationAuditConfig {
                cue,
                codebook,
                translation,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} groups, {} untranslated, development input available: {}, release-candidate input eligible: {}, translation project complete: {})",
                output.display(),
                report.semantic_group_count,
                report.untranslated_group_count,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible,
                report.translation_project_complete
            );
        }
        Command::SyncDialogueTranslationAssets {
            primary_workspace,
            selector_workspace,
            output,
            force,
        } => {
            let report = sync_dialogue_translation_assets(&DialogueTranslationAssetSyncConfig {
                primary_workspace,
                selector_workspace,
                output: output.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} unique decisions: {} primary, {} selector; {} untranslated, {} draft, {} ready for review)",
                output.display(),
                report.semantic_decision_count,
                report.primary_decision_count,
                report.selector_decision_count,
                report.untranslated_decision_count,
                report.draft_decision_count,
                report.ready_for_review_decision_count
            );
        }
        Command::AuditDialogueTranslationAssets { input } => {
            let report =
                audit_dialogue_translation_assets(&DialogueTranslationAssetAuditConfig { input })?;
            println!(
                "audited {} unique decisions across {} primary and {} selector owners (largest JSON: {} bytes)",
                report.semantic_decision_count,
                report.primary_owner_count,
                report.selector_owner_count,
                report.largest_json_byte_count
            );
        }
        Command::HashDialogueTranslationDecisions { input } => {
            let report =
                hash_dialogue_translation_decisions(&DialogueTranslationDecisionHashConfig {
                    input,
                })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditDialogueLayout {
            cue,
            codebook,
            translation,
            translation_audit_output,
            require_approved,
            output,
        } => {
            let report = audit_dialogue_layout(&DialogueLayoutAuditConfig {
                cue,
                codebook,
                translation,
                translation_audit_output,
                require_approved,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} groups, {} layout overflows, within renderer capacity: {}, independent review complete: {})",
                output.display(),
                report.semantic_group_count,
                report.overflow_group_count,
                report.layout_within_renderer_capacity,
                report.independent_review_complete
            );
        }
        Command::AuditDialogueFontAllocation {
            cue,
            codebook,
            translation,
            translation_audit_output,
            output,
        } => {
            let report = audit_dialogue_font_allocation(&DialogueFontAllocationAuditConfig {
                cue,
                codebook,
                translation,
                translation_audit_output,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} assets, development input available: {}, release-candidate input eligible: {})",
                output.display(),
                report.assets.len(),
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::AuditDialogueBuildCapacity {
            cue,
            codebook,
            translation,
            translation_audit_output,
            output,
        } => {
            let report = audit_dialogue_build_capacity(&DialogueBuildCapacityAuditConfig {
                cue,
                codebook,
                translation,
                translation_audit_output,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} coordinates, fixed bank regions fit: {}, relocated bank regions fit: {}, development input available: {}, release-candidate input eligible: {})",
                output.display(),
                report.coordinate_count,
                report.fixed_bank_regions_fit,
                report.relocated_bank_regions_fit,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::BuildDialogueCodeAllocation {
            cue,
            codebook,
            translation,
            selector_translation,
            name_input_keyboard,
            translation_audit_output,
            selector_translation_audit_output,
            output,
            input_policy,
        } => {
            let report = build_dialogue_code_allocation(&DialogueCodeAllocationConfig {
                cue,
                codebook,
                translation,
                selector_translation,
                name_input_keyboard,
                translation_audit_output,
                selector_translation_audit_output,
                output: output.clone(),
                input_policy,
            })?;
            println!(
                "wrote {} (input policy: {}, {} primary and {} selector groups authored, {} asset-local codes, {} global name codes reserved, static allocation complete: {}, development build input available: {}, complete translation scope available: {}, release-candidate input eligible: {})",
                output.display(),
                report.input_policy,
                report.development_authored_group_count,
                report.selector_development_authored_group_count,
                report.asset_local_code_count,
                report.global_name_code_count,
                report.static_allocation_complete,
                report.development_build_input_available,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::AuditDialogueFontConflicts {
            cue,
            codebook,
            translation,
            selector_translation,
            name_input_keyboard,
            translation_audit_output,
            selector_translation_audit_output,
            code_allocation_output,
            output_dir,
            force,
        } => {
            let report = audit_dialogue_font_conflicts(&DialogueFontConflictAuditConfig {
                cue,
                codebook,
                translation,
                selector_translation,
                name_input_keyboard,
                translation_audit_output,
                selector_translation_audit_output,
                code_allocation_output,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} conflicts in {} assets, {} exact untranslated groups required, {} codes have one preserved group owner, all attributed: {})",
                output_dir.display(),
                report.conflicting_code_count,
                report.conflict_asset_count,
                report.required_untranslated_group_count,
                report.single_group_owned_code_count,
                report.all_conflicts_attributed
            );
        }
        Command::PrepareDialogueMessages {
            cue,
            codebook,
            translation,
            selector_translation,
            name_input_keyboard,
            translation_audit_output,
            selector_translation_audit_output,
            code_allocation_output,
            output_dir,
            input_policy,
            force,
        } => {
            let report = prepare_dialogue_message_images(&DialogueMessageBuildConfig {
                cue,
                codebook,
                translation,
                selector_translation,
                name_input_keyboard,
                translation_audit_output,
                selector_translation_audit_output,
                code_allocation_output,
                output_dir: output_dir.clone(),
                force,
                input_policy,
            })?;
            println!(
                "wrote {} (input policy: {}, {} assets, {} translated coordinates, {} preserved untranslated coordinates, {} runtime insertion coordinates, {} selector groups currently authored, parse-back: {}, compressed within original extents: {}, development build input available: {}, complete translation scope available: {}, release-candidate input eligible: {})",
                output_dir.display(),
                report.input_policy,
                report.asset_count,
                report.translated_coordinate_count,
                report.preserved_untranslated_coordinate_count,
                report.rewritten_runtime_insertion_coordinate_count,
                report.selector_development_authored_group_count,
                report.all_assets_parse_back,
                report.all_assets_compress_within_original_extents,
                report.development_build_input_available,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::PrepareDialogueInputs {
            cue,
            spec,
            input_policy,
            output_dir,
            force,
        } => {
            justice_gakuen2_kr::dialogue_audit::prepare_dialogue_inputs(
                &cue,
                &spec,
                input_policy,
                &output_dir,
                force,
            )?;
            println!("prepared dialogue inputs in {}", output_dir.display());
        }
        Command::BuildDialogueFonts {
            prepared_dialogue,
            cue,
            spec,
            input_policy,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let build = build_dialogue_font_images(&DialogueFontBuildConfig {
                prepared_dialogue,
                cue,
                codebook: loaded_spec.assets.dialogue_codebook,
                translation: loaded_spec.assets.dialogue_translations,
                selector_translation: loaded_spec.assets.dialogue_selector_translations,
                name_input_keyboard: loaded_spec
                    .assets
                    .name_entry_candidates
                    .join("keyboard.json"),
                name_glyph_font: loaded_spec.fonts.name_entry.composed_glyphs.path,
                name_glyph_font_px: loaded_spec.fonts.name_entry.composed_glyphs.font_px,
                font: loaded_spec.fonts.dialogue_body.path,
                font_px: loaded_spec.fonts.dialogue_body.font_px,
                input_policy,
                output_dir: output_dir.clone(),
                force,
            })?;
            let report = &build.report;
            println!(
                "wrote {} (input policy: {}, {} of {} assets installed, {} source-conflicting assets preserved, {} rasterized characters, {} installed glyph cells, message regions preserved: {}, compressed within original extents: {}, development build input available: {}, complete translation scope available: {}, release-candidate input eligible: {})",
                output_dir.display(),
                report.input_policy,
                report.asset_count,
                report.source_asset_count,
                report.skipped_asset_count,
                report.rasterized_character_count,
                report.installed_local_glyph_count + report.installed_shared_name_cell_count,
                report.all_message_regions_preserved,
                report.all_assets_compress_within_original_extents,
                report.development_build_input_available,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::BuildLoadingArt {
            cue,
            spec,
            output_dir,
        } => {
            justice_gakuen2_kr::loading_art::build_loading_art(&cue, &spec, &output_dir)?;
            println!("wrote {}", output_dir.display());
        }
        Command::BuildStaffRoll {
            cue,
            spec,
            output_dir,
        } => {
            justice_gakuen2_kr::staff_roll::build_staff_roll(&cue, &spec, &output_dir)?;
            println!("wrote {}", output_dir.display());
        }
        Command::BuildCardArt {
            cue,
            spec,
            output_dir,
        } => {
            justice_gakuen2_kr::build_card_art(&cue, &spec, &output_dir)?;
            println!("wrote {}", output_dir.display());
        }
        Command::BuildDialogueDisc {
            prepared_dialogue,
            cue,
            spec,
            input_policy,
            output_dir,
            force,
        } => {
            let report = build_dialogue_development_disc(&DialogueDiscBuildConfig {
                prepared_dialogue,
                cue,
                build_spec: spec,
                input_policy,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} (input policy: {}, {} records, {} changed sectors, readback: {}, EDC/ECC: {}, development build input available: {}, complete translation scope available: {}, release-candidate input eligible: {})",
                output_dir.display(),
                report.input_policy,
                report.replacement_record_count,
                report.changed_sector_count,
                report.all_records_read_back,
                report.edc_ecc_verified,
                report.development_build_input_available,
                report.development_translation_input_available,
                report.release_candidate_translation_input_eligible
            );
        }
        Command::BuildModeSelectAssets {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = build_mode_select_assets(&ModeSelectBuildConfig {
                cue,
                assets: loaded_spec.assets.mode_select,
                fonts: loaded_spec.fonts.mode_select,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} modes, source regions: {}, development input available: {}, release-candidate input eligible: {})",
                output_dir.display(),
                report.mode_count,
                report.source_regions_match,
                report.development_input_available,
                report.release_candidate_input_eligible,
            );
        }
        Command::BuildModeDescendantAssets {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let build = build_mode_descendant_graphics(&ModeDescendantGraphicsBuildConfig {
                cue,
                assets: loaded_spec.assets.mode_descendants,
                fonts: loaded_spec.fonts.mode_descendants,
                build_spec_sha256: loaded_spec.sha256,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} (record-owned units: {}/{} composited; practical-result units: {}, static insertion complete: {}; record-owned source regions: {}; development input available: {}; release-candidate input eligible: {})",
                output_dir.display(),
                build.report.record_composited_unit_count,
                build.report.record_owned_unit_count,
                build
                    .report
                    .practical_results
                    .as_ref()
                    .map_or(0, |report| report.source_unit_count),
                build
                    .report
                    .practical_results
                    .as_ref()
                    .is_some_and(|report| report.static_patch_insertion_complete),
                build.report.record_owned_source_regions_match,
                build.report.development_input_available,
                build.report.release_candidate_input_eligible,
            );
        }
        Command::BuildTitleMenuAssets {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = build_title_menu_assets(&TitleMenuBuildConfig {
                cue,
                assets: loaded_spec.assets.title_menu,
                font: loaded_spec.fonts.title_menu,
                build_spec_sha256: loaded_spec.sha256,
                output_dir: output_dir.clone(),
                force,
            })?
            .report;
            println!(
                "wrote {} ({} entries, source records: {}, development input available: {}, release-candidate input eligible: {})",
                output_dir.display(),
                report.entry_count,
                report.source_records_match,
                report.development_input_available,
                report.release_candidate_input_eligible,
            );
        }
        Command::AuditDialogueDevelopmentRuntime {
            disc_report,
            disc_bin,
            runtime_ram,
            runtime_frame,
            source_path,
            launch_id,
            emulator_build,
            capability_revision,
            output,
        } => {
            let report =
                audit_dialogue_development_runtime(&DialogueDevelopmentRuntimeAuditConfig {
                    disc_report,
                    disc_bin,
                    runtime_ram,
                    runtime_frame,
                    source_path,
                    launch_id,
                    emulator_build,
                    capability_revision,
                    output: output.clone(),
                })?;
            println!(
                "wrote {} ({} from {} at {}, disc/BZZ readback: {}, runtime residency: {})",
                output.display(),
                report.source_path,
                report.bundle_path,
                report.resident_runtime_start,
                report.disc_readback_chain_verified,
                report.runtime_residency_verified
            );
        }
        Command::AuditDialogueNameEntry { cue, output } => {
            let report = audit_dialogue_name_entry(&DialogueNameEntryAuditConfig {
                cue,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} visible candidate cells, {} original unique codes, name capacities: 6/6/4)",
                output.display(),
                report.source_selectable_cell_count,
                report.source_unique_selectable_code_count
            );
        }
        Command::AuditMemoryReferences {
            cue,
            output,
            ranges,
            state_budget,
        } => {
            let ranges = ranges
                .iter()
                .map(|s| {
                    let (a, b) = s.split_once(':').context("range must be start:end")?;
                    Ok([
                        u32::from_str_radix(a.trim_start_matches("0x"), 16)?,
                        u32::from_str_radix(b.trim_start_matches("0x"), 16)?,
                    ])
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            justice_gakuen2_kr::consumer_analysis::audit_memory_references(
                &justice_gakuen2_kr::consumer_analysis::MemoryReferenceAuditConfig {
                    cue,
                    output,
                    ranges,
                    state_budget,
                },
            )?;
        }
        Command::AuditNameCompanionReferences {
            cue,
            output,
            address_flow_state_budget,
        } => {
            let report = audit_name_companion_references(&NameCompanionReferenceAuditConfig {
                cue,
                output: output.clone(),
                address_flow_state_budget,
            })?;
            println!(
                "wrote {} ({} static access sites: {} reads, {} writes; runtime confirmation required: {})",
                output.display(),
                report.static_memory_access_site_count,
                report.static_read_site_count,
                report.static_write_site_count,
                report.runtime_confirmation_required,
            );
        }
        Command::BuildDialogueNameEntry {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let report = build_dialogue_name_entry_image(&DialogueNameEntryBuildConfig {
                cue,
                candidates: loaded_spec.assets.name_entry_candidates,
                graphics: loaded_spec.assets.name_entry_graphics,
                candidate_font: loaded_spec.fonts.name_entry.candidates.path,
                candidate_font_px: loaded_spec.fonts.name_entry.candidates.font_px,
                fixed_graphics_font: loaded_spec.fonts.name_entry.fixed_graphics,
                output_dir: output_dir.clone(),
                force,
            })?;
            println!(
                "wrote {} ({} Korean candidates, {} source digits preserved, shared reservation {}..{}, {} visible glyphs installed, development mapping complete: {}, release-candidate mapping eligible: {})",
                output_dir.display(),
                report.localized_candidate_count,
                report.preserved_source_candidate_count,
                report.global_name_code_start,
                report.global_name_code_end,
                report.font.installed_glyph_count,
                report.development_mapping_complete,
                report.release_candidate_mapping_eligible
            );
        }
        Command::BuildComposedNameInput {
            cue,
            spec,
            output_dir,
            force,
        } => {
            let loaded_spec = load_development_build_spec(&spec)?;
            let build = build_composed_name_input(&ComposedNameInputBuildConfig {
                cue,
                keyboard: loaded_spec
                    .assets
                    .name_entry_candidates
                    .join("keyboard.json"),
                graphics: loaded_spec.assets.name_entry_graphics,
                keyboard_font: loaded_spec.fonts.name_entry.keyboard_keys.path,
                keyboard_font_px: loaded_spec.fonts.name_entry.keyboard_keys.font_px,
                composed_glyph_font: loaded_spec.fonts.name_entry.composed_glyphs.path,
                composed_glyph_font_px: loaded_spec.fonts.name_entry.composed_glyphs.font_px,
                roster_font: loaded_spec.fonts.name_entry.roster_glyphs.path.clone(),
                roster_font_px: loaded_spec.fonts.name_entry.roster_glyphs.font_px,
                fixed_graphics_font: loaded_spec.fonts.name_entry.fixed_graphics,
                output_dir: output_dir.clone(),
                force,
            })?;
            let report = &build.report;
            println!(
                "wrote {} ({} keys, {} glyph-pack bytes, runtime verified: {}, release eligible: {})",
                output_dir.display(),
                report.keyboard_overlay.active_key_count,
                report.font.glyph_pack.pack_bytes,
                report.runtime_consumer_verified,
                report.release_candidate_eligible
            );
        }
        Command::AuditDialogueGlossary {
            cue,
            codebook,
            glossary,
            output,
        } => {
            let report = audit_dialogue_glossary(&DialogueGlossaryAuditConfig {
                cue,
                codebook,
                glossary,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} terms, {} candidate, {} source occurrences)",
                output.display(),
                report.entry_count,
                report.candidate_entry_count,
                report.matched_term_occurrence_count
            );
        }
        Command::AuditDialogueScenes {
            cue,
            codebook,
            runtime_ram,
            runtime_frame,
            output,
        } => {
            let report = audit_dialogue_scenes(&DialogueSceneAuditConfig {
                cue,
                codebook,
                runtime_ram,
                runtime_frame,
                output: output.clone(),
            })?;
            println!(
                "wrote {} ({} source assets, {} resolved primary scripts, {} unresolved primary scripts, {} contextual message references, {} admitted runtime scene references)",
                output.display(),
                report.source_asset_count,
                report.resolved_primary_script_asset_count,
                report.unresolved_primary_script_asset_count,
                report.contextual_message_reference_count,
                report.admitted_message_reference_count
            );
        }
        Command::BuildHangulProbe {
            cue,
            font,
            output_dir,
            force,
        } => {
            let manifest = build_hangul_probe(&HangulProbeConfig {
                cue,
                font,
                output_dir,
                force,
            })?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
