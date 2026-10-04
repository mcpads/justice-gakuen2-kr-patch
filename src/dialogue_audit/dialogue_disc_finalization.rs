use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::write_pretty_json_and_hash;

use crate::bonus_confirmation::BonusConfirmationBuild;
use crate::bonus_inventory::BonusInventoryBuild;
use crate::bonus_inventory_action_labels::BonusInventoryActionLabelBuild;
use crate::bonus_inventory_card_acquisition::BonusInventoryCardAcquisitionBuild;
use crate::bonus_inventory_memory_card_swap::BonusInventoryMemoryCardSwapBuild;
use crate::bonus_inventory_stock_label::BonusInventoryStockLabelBuild;
use crate::bonus_j_bank_return_label::BonusJBankReturnLabelBuild;
use crate::bonus_menu::BonusMenuBuild;
use crate::bonus_page_indicator::BonusPageIndicatorBuild;
use crate::bonus_shop_exit_confirmation::BonusShopExitConfirmationBuild;
use crate::bonus_shop_text_source::BonusShopTextBuild;
use crate::character_select_graphics::CharacterSelectAtlasBuild;
use crate::diary_header::DiaryHeaderBuild;
use crate::diary_scene::DiarySceneBuild;
use crate::disc::rebuild::{DiscRecordContribution, DiscRecordPlan};
use crate::edit_runtime_text::EditRuntimeTextBuild;
use crate::mode_descendant_graphics::ModeDescendantGraphicsBuild;
use crate::mode_select::ModeSelectRecordBuild;
use crate::options::OptionsRecordBuild;
use crate::practical_instruction_graphics::PracticalInstructionGraphicsBuild;
use crate::shop_ui::ShopUiBuild;
use crate::source_disc::VerifiedSourceDisc;
use crate::surface_inventory::SurfaceInventoryBuildReport;
use crate::title_menu::TitleMenuRecordBuild;
use crate::title_notice::TitleNoticeBuild;

use super::bonus_inventory_composition::BonusInventorySurfaceBuild;
use super::development_menu_build::DevelopmentMenuBuild;
use super::development_title_overlay_build::DevelopmentTitleOverlayBuild;
use super::dialogue_bundle::DialogueBundleBuild;
use super::dialogue_disc_build_model::{DialogueDiscBuildReport, DialogueDiscRecordWrite};
use super::dialogue_disc_outputs::DialogueDiscOutputs;
use super::dialogue_font_build_model::DialogueFontBuild;
use super::dialogue_name_runtime_build::DialogueNameRuntimeBuild;
use super::main_executable_record::StoredMainExecutableRecord;
use super::name_entry_composed_build::ComposedNameInputBuild;
use super::shop_surface_composition::ShopSurfaceBuild;

#[path = "dialogue_disc_finalization/readback.rs"]
mod readback;

use readback::{DialogueDiscReadbackInputs, DialogueDiscReadbacks, read_back_dialogue_disc};

#[derive(Clone, Copy)]
pub(super) struct DialogueDiscComponentBuilds<'a> {
    pub font: &'a DialogueFontBuild,
    pub name_entry: &'a ComposedNameInputBuild,
    pub diary_header: &'a DiaryHeaderBuild,
    pub diary_scene: &'a DiarySceneBuild,
    pub character_select: &'a CharacterSelectAtlasBuild,
    pub mode_descendants: &'a ModeDescendantGraphicsBuild,
    pub practical_instructions: &'a PracticalInstructionGraphicsBuild,
    pub mode_select: &'a ModeSelectRecordBuild,
    pub options: &'a OptionsRecordBuild,
    pub title_menu: &'a TitleMenuRecordBuild,
    pub title_notice: &'a TitleNoticeBuild,
    pub edit_runtime_text: &'a EditRuntimeTextBuild,
    pub bonus_menu: &'a BonusMenuBuild,
    pub bonus_inventory: &'a BonusInventoryBuild,
    pub bonus_inventory_action_labels: &'a BonusInventoryActionLabelBuild,
    pub bonus_confirmation: &'a BonusConfirmationBuild,
    pub bonus_inventory_stock_label: &'a BonusInventoryStockLabelBuild,
    pub bonus_inventory_card_acquisition: &'a BonusInventoryCardAcquisitionBuild,
    pub bonus_inventory_memory_card_swap: &'a BonusInventoryMemoryCardSwapBuild,
    pub bonus_j_bank_return_label: &'a BonusJBankReturnLabelBuild,
    pub bonus_page_indicator: &'a BonusPageIndicatorBuild,
    pub shop_ui: &'a ShopUiBuild,
    pub bonus_shop_exit_confirmation: &'a BonusShopExitConfirmationBuild,
    pub bonus_shop_text: &'a BonusShopTextBuild,
}

#[derive(Clone, Copy)]
pub(super) struct DialogueDiscComposition<'a> {
    pub dialogue_bundles: &'a [DialogueBundleBuild],
    pub development_menu: &'a DevelopmentMenuBuild,
    pub development_title_overlay: &'a DevelopmentTitleOverlayBuild,
    pub main_executable: &'a StoredMainExecutableRecord,
    pub dialogue_name_runtime: &'a DialogueNameRuntimeBuild,
    pub bonus_inventory_surfaces: &'a BonusInventorySurfaceBuild,
    pub shop_surfaces: &'a ShopSurfaceBuild,
}

pub(super) struct DialogueDiscFinalization<'a> {
    pub source_disc: &'a VerifiedSourceDisc,
    pub component_builds: DialogueDiscComponentBuilds<'a>,
    pub composition: DialogueDiscComposition<'a>,
    pub dialogue_name_runtime_build_manifest_sha256: &'a str,
    pub shared_name_runtime_build_manifest_sha256: &'a str,
    pub development_build_spec_sha256: &'a str,
    pub surface_inventory: &'a SurfaceInventoryBuildReport,
    pub replacements: &'a [DiscRecordContribution<'a>],
    pub outputs: &'a DialogueDiscOutputs,
}

pub(super) fn finalize_dialogue_disc(
    finalization: DialogueDiscFinalization<'_>,
) -> Result<DialogueDiscBuildReport> {
    let DialogueDiscFinalization {
        source_disc,
        component_builds,
        composition,
        dialogue_name_runtime_build_manifest_sha256,
        shared_name_runtime_build_manifest_sha256,
        development_build_spec_sha256,
        surface_inventory,
        replacements,
        outputs,
    } = finalization;
    let temporary_bin = outputs.staged_bin();
    let output_bin = outputs.bin();
    let output_cue = outputs.cue();
    let mut disc_plan = DiscRecordPlan::new();
    for replacement in replacements {
        disc_plan.register(replacement.clone())?;
    }
    let finalized = disc_plan.finalize(source_disc.snapshot_image_path(), temporary_bin)?;
    ensure!(
        finalized.source_sector_count == finalized.output_sector_count
            && finalized.metadata_lbas.is_empty(),
        "development disc must preserve track length and ISO locations for memory-card identity"
    );
    let metadata_lbas = finalized.metadata_lbas;
    let source_sector_count = finalized.source_sector_count;
    let output_sector_count = finalized.output_sector_count;
    let rebuilt = finalized.records;
    let changed_lbas = finalized.changed_lbas;
    let output_bin_sha256 = finalized.output_sha256;
    ensure!(
        rebuilt.len() == replacements.len(),
        "dialogue disc replacement count changed"
    );
    let allowed_lbas = rebuilt
        .iter()
        .flat_map(|record| record.target_lbas.iter().copied())
        .collect::<BTreeSet<_>>();
    let planned_lbas = rebuilt
        .iter()
        .flat_map(|record| record.changed_lbas.iter().copied())
        .chain(metadata_lbas.iter().copied())
        .collect::<BTreeSet<_>>();
    let observed_lbas = changed_lbas.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        observed_lbas == planned_lbas,
        "dialogue disc image diff differs from its planned changed sectors"
    );
    ensure!(
        !changed_lbas.is_empty(),
        "dialogue disc build changed no sectors"
    );
    let changes_confined_to_replaced_records =
        changed_lbas.iter().all(|lba| allowed_lbas.contains(lba));
    let changes_confined_to_declared_records_and_metadata = changed_lbas
        .iter()
        .all(|lba| allowed_lbas.contains(lba) || metadata_lbas.contains(lba));
    ensure!(
        changes_confined_to_declared_records_and_metadata,
        "dialogue disc build changed a sector outside declared records and metadata"
    );
    ensure!(
        rebuilt.iter().all(|record| {
            record
                .target_lbas
                .iter()
                .any(|lba| observed_lbas.contains(lba))
        }),
        "dialogue disc build left one replacement record unchanged"
    );
    let record_writes = rebuilt
        .iter()
        .map(|record| DialogueDiscRecordWrite {
            owner: record.owner.clone(),
            path: record.path.clone(),
            logical_byte_count: record.record.size,
            source_sha256: record.source_sha256.clone(),
            replacement_sha256: record.replacement_sha256.clone(),
            extent_lba: record.record.extent_lba,
            sector_count: record.target_lbas.len(),
            changed_lba_ranges: contiguous_lba_ranges(&record.changed_lbas),
        })
        .collect::<Vec<_>>();

    let readbacks = read_back_dialogue_disc(DialogueDiscReadbackInputs {
        temporary_bin,
        rebuilt: &rebuilt,
        replacements,
        component_builds,
        composition,
    })?;
    let DialogueDiscReadbacks {
        battle_name,
        dialogue_assets,
        dialogue_bundles: dialogue_bundle_readbacks,
        name_entry_font,
        name_entry,
        diary_header,
        diary_scene,
        diary_scene_runtime_bundles,
        diary_artwork_archives,
        character_select_records,
        character_select_overlays,
        mode_descendant_records,
        practical_instruction,
        practical_instruction_prompt_consumers,
        menu,
        options_overlay,
        options_info,
        edit_runtime_text_overlay,
        edit_runtime_text_pass,
        title_menu_overlay,
        main_executable: main_executable_readback,
        dialogue_name_runtime: dialogue_name_runtime_readback,
        bonus_menu,
        bonus_inventory,
        bonus_inventory_overlay,
        shop_ui,
        shop_overlay,
    } = readbacks;

    let DialogueDiscComponentBuilds {
        font: font_build,
        name_entry: name_entry_build,
        diary_header: diary_header_build,
        diary_scene: diary_scene_build,
        character_select: character_select_build,
        mode_descendants: mode_descendant_build,
        practical_instructions: practical_instruction_build,
        mode_select: mode_select_record_build,
        options: options_build,
        title_menu: title_menu_build,
        title_notice: title_notice_build,
        edit_runtime_text: edit_runtime_text_build,
        bonus_menu: bonus_menu_build,
        bonus_inventory: bonus_inventory_build,
        bonus_inventory_action_labels: bonus_inventory_action_labels_build,
        bonus_confirmation: bonus_confirmation_build,
        bonus_inventory_stock_label: bonus_inventory_stock_label_build,
        bonus_inventory_card_acquisition: bonus_inventory_card_acquisition_build,
        bonus_inventory_memory_card_swap: bonus_inventory_memory_card_swap_build,
        bonus_j_bank_return_label: bonus_j_bank_return_label_build,
        bonus_page_indicator: bonus_page_indicator_build,
        shop_ui: shop_ui_build,
        bonus_shop_exit_confirmation: bonus_shop_exit_confirmation_build,
        bonus_shop_text: bonus_shop_text_build,
    } = component_builds;
    let mode_select_build = &mode_select_record_build.report;
    let font_report = &font_build.report;
    let name_entry_report = &name_entry_build.report;
    let DialogueDiscComposition {
        dialogue_bundles: _,
        development_menu,
        development_title_overlay,
        main_executable: _,
        dialogue_name_runtime: _,
        bonus_inventory_surfaces: _,
        shop_surfaces: _,
    } = composition;
    let font_build_manifest_sha256 = font_build.manifest_sha256.clone();
    let name_entry_build_manifest_sha256 = name_entry_build.manifest_sha256.clone();
    let diary_header_build_manifest_sha256 = diary_header_build.build_manifest_sha256.clone();
    let diary_scene_build_manifest_sha256 = diary_scene_build.manifest_sha256.clone();
    let character_select_build_manifest_sha256 = character_select_build.manifest_sha256.clone();
    let mode_descendant_build_manifest_sha256 = mode_descendant_build.build_manifest_sha256.clone();
    let practical_instruction_build_manifest_sha256 =
        practical_instruction_build.build_manifest_sha256.clone();
    let mode_select_build_manifest_sha256 = mode_select_record_build.build_manifest_sha256.clone();
    let options_build_manifest_sha256 = options_build.build_manifest_sha256.clone();
    let title_menu_build_manifest_sha256 = title_menu_build.build_manifest_sha256.clone();
    let title_notice_build_manifest_sha256 = title_notice_build.build_manifest_sha256.clone();
    let edit_runtime_text_build_manifest_sha256 =
        edit_runtime_text_build.build_manifest_sha256.clone();
    let bonus_menu_build_manifest_sha256 = bonus_menu_build.build_manifest_sha256.clone();
    let bonus_inventory_build_manifest_sha256 = bonus_inventory_build.build_manifest_sha256.clone();
    let bonus_inventory_action_labels_build_manifest_sha256 = bonus_inventory_action_labels_build
        .build_manifest_sha256
        .clone();
    let bonus_confirmation_build_manifest_sha256 =
        bonus_confirmation_build.build_manifest_sha256.clone();
    let bonus_inventory_stock_label_build_manifest_sha256 = bonus_inventory_stock_label_build
        .build_manifest_sha256
        .clone();
    let bonus_inventory_card_acquisition_build_manifest_sha256 =
        bonus_inventory_card_acquisition_build
            .build_manifest_sha256
            .clone();
    let bonus_inventory_memory_card_swap_build_manifest_sha256 =
        bonus_inventory_memory_card_swap_build
            .build_manifest_sha256
            .clone();
    let bonus_j_bank_return_label_build_manifest_sha256 = bonus_j_bank_return_label_build
        .build_manifest_sha256
        .clone();
    let bonus_page_indicator_build_manifest_sha256 =
        bonus_page_indicator_build.build_manifest_sha256.clone();
    let shop_ui_build_manifest_sha256 = shop_ui_build.build_manifest_sha256.clone();
    let bonus_shop_exit_confirmation_build_manifest_sha256 = bonus_shop_exit_confirmation_build
        .build_manifest_sha256
        .clone();
    let bonus_shop_text_build_manifest_sha256 = bonus_shop_text_build.build_manifest_sha256.clone();
    let dialogue_name_runtime_build_manifest_sha256 =
        dialogue_name_runtime_build_manifest_sha256.to_string();
    let shared_name_runtime_build_manifest_sha256 =
        shared_name_runtime_build_manifest_sha256.to_string();
    let report = DialogueDiscBuildReport {
        kind: "Justice Gakuen 2 non-release Korean development disc build".to_string(),
        source_bin_sha256: font_report.source_bin_sha256.clone(),
        input_policy: font_report.input_policy.clone(),
        output_bin_sha256,
        output_bin: output_bin
            .file_name()
            .context("dialogue output BIN has no file name")?
            .to_string_lossy()
            .into_owned(),
        output_cue: output_cue
            .file_name()
            .context("dialogue output CUE has no file name")?
            .to_string_lossy()
            .into_owned(),
        development_build_spec_sha256: development_build_spec_sha256.to_string(),
        mode_select_descendant_surface_inventory: Some(surface_inventory.clone()),
        font_build_manifest_sha256,
        name_entry_build_manifest_sha256,
        bonus_menu_build_manifest_sha256: Some(bonus_menu_build_manifest_sha256),
        bonus_inventory_build_manifest_sha256: Some(bonus_inventory_build_manifest_sha256),
        bonus_inventory_action_labels_build_manifest_sha256: Some(
            bonus_inventory_action_labels_build_manifest_sha256,
        ),
        bonus_confirmation_build_manifest_sha256: Some(bonus_confirmation_build_manifest_sha256),
        bonus_inventory_stock_label_build_manifest_sha256: Some(
            bonus_inventory_stock_label_build_manifest_sha256,
        ),
        bonus_inventory_card_acquisition_build_manifest_sha256: Some(
            bonus_inventory_card_acquisition_build_manifest_sha256,
        ),
        bonus_inventory_memory_card_swap_build_manifest_sha256: Some(
            bonus_inventory_memory_card_swap_build_manifest_sha256,
        ),
        bonus_j_bank_return_label_build_manifest_sha256: Some(
            bonus_j_bank_return_label_build_manifest_sha256,
        ),
        bonus_page_indicator_build_manifest_sha256: Some(
            bonus_page_indicator_build_manifest_sha256,
        ),
        shop_ui_build_manifest_sha256: Some(shop_ui_build_manifest_sha256),
        bonus_shop_exit_confirmation_build_manifest_sha256: Some(
            bonus_shop_exit_confirmation_build_manifest_sha256,
        ),
        bonus_shop_text_build_manifest_sha256: Some(bonus_shop_text_build_manifest_sha256),
        diary_header_build_manifest_sha256: Some(diary_header_build_manifest_sha256),
        diary_scene_build_manifest_sha256: Some(diary_scene_build_manifest_sha256),
        character_select_build_manifest_sha256: Some(character_select_build_manifest_sha256),
        mode_descendant_build_manifest_sha256: Some(mode_descendant_build_manifest_sha256),
        practical_instruction_build_manifest_sha256: Some(
            practical_instruction_build_manifest_sha256,
        ),
        mode_select_build_manifest_sha256: Some(mode_select_build_manifest_sha256),
        options_build_manifest_sha256: Some(options_build_manifest_sha256),
        title_menu_build_manifest_sha256: Some(title_menu_build_manifest_sha256),
        title_notice_build_manifest_sha256: Some(title_notice_build_manifest_sha256),
        edit_runtime_text_build_manifest_sha256: Some(edit_runtime_text_build_manifest_sha256),
        dialogue_name_runtime_build_manifest_sha256: Some(
            dialogue_name_runtime_build_manifest_sha256,
        ),
        shared_name_runtime_build_manifest_sha256: Some(shared_name_runtime_build_manifest_sha256),
        options_build_spec_sha256: Some(options_build.report.build_spec_sha256.clone()),
        dialogue_font_name: font_report.font_name.clone(),
        dialogue_font_sha256: font_report.font_sha256.clone(),
        dialogue_font_px: font_report.font_px,
        name_entry_candidate_font_name: name_entry_report.font.keyboard_font_name.clone(),
        name_entry_candidate_font_sha256: name_entry_report.font.keyboard_font_sha256.clone(),
        name_entry_candidate_font_px: name_entry_report.font.keyboard_font_px,
        mode_select_fonts: mode_select_build.fonts.clone(),
        bonus_menu_development_input_available: Some(
            bonus_menu_build.report.development_input_available,
        ),
        bonus_menu_release_candidate_input_eligible: Some(
            bonus_menu_build.report.release_candidate_input_eligible,
        ),
        bonus_inventory_development_input_available: Some(
            bonus_inventory_build.report.development_input_available,
        ),
        bonus_inventory_release_candidate_input_eligible: Some(
            bonus_inventory_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_inventory_action_labels_development_input_available: Some(
            bonus_inventory_action_labels_build
                .report
                .development_input_available,
        ),
        bonus_inventory_action_labels_release_candidate_input_eligible: Some(
            bonus_inventory_action_labels_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_confirmation_development_input_available: Some(
            bonus_confirmation_build.report.development_input_available,
        ),
        bonus_confirmation_release_candidate_input_eligible: Some(
            bonus_confirmation_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_inventory_stock_label_development_input_available: Some(
            bonus_inventory_stock_label_build
                .report
                .development_input_available,
        ),
        bonus_inventory_stock_label_release_candidate_input_eligible: Some(
            bonus_inventory_stock_label_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_inventory_card_acquisition_development_input_available: Some(
            bonus_inventory_card_acquisition_build
                .report
                .development_input_available,
        ),
        bonus_inventory_card_acquisition_release_candidate_input_eligible: Some(
            bonus_inventory_card_acquisition_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_inventory_memory_card_swap_development_input_available: Some(
            bonus_inventory_memory_card_swap_build
                .report
                .development_input_available,
        ),
        bonus_inventory_memory_card_swap_release_candidate_input_eligible: Some(
            bonus_inventory_memory_card_swap_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_j_bank_return_label_development_input_available: Some(
            bonus_j_bank_return_label_build
                .report
                .development_input_available,
        ),
        bonus_j_bank_return_label_release_candidate_input_eligible: Some(
            bonus_j_bank_return_label_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_page_indicator_development_input_available: Some(
            bonus_page_indicator_build
                .report
                .development_input_available,
        ),
        bonus_page_indicator_release_candidate_input_eligible: Some(
            bonus_page_indicator_build
                .report
                .release_candidate_input_eligible,
        ),
        shop_ui_development_input_available: Some(shop_ui_build.report.development_input_available),
        shop_ui_release_candidate_input_eligible: Some(
            shop_ui_build.report.release_candidate_input_eligible,
        ),
        bonus_shop_exit_confirmation_development_input_available: Some(
            bonus_shop_exit_confirmation_build
                .report
                .development_input_available,
        ),
        bonus_shop_exit_confirmation_release_candidate_input_eligible: Some(
            bonus_shop_exit_confirmation_build
                .report
                .release_candidate_input_eligible,
        ),
        bonus_shop_text_development_input_available: Some(
            bonus_shop_text_build.report.development_can_continue,
        ),
        bonus_shop_text_release_candidate_input_eligible: Some(
            bonus_shop_text_build
                .report
                .release_candidate_input_eligible,
        ),
        mode_select_development_input_available: Some(
            mode_select_build.development_input_available,
        ),
        mode_descendant_development_input_available: Some(
            mode_descendant_build.report.development_input_available,
        ),
        mode_descendant_release_candidate_input_eligible: Some(
            mode_descendant_build
                .report
                .release_candidate_input_eligible,
        ),
        practical_instruction_development_input_available: Some(
            practical_instruction_build.report.authored_member_count > 0
                && practical_instruction_build.report.all_source_members_bound,
        ),
        practical_instruction_release_candidate_input_eligible: Some(
            practical_instruction_build
                .report
                .release_candidate_input_eligible,
        ),
        mode_select_release_candidate_input_eligible: Some(
            mode_select_build.release_candidate_input_eligible,
        ),
        options_development_input_available: Some(options_build.report.development_input_available),
        options_release_candidate_input_eligible: Some(
            options_build.report.release_candidate_input_eligible,
        ),
        title_menu_development_input_available: Some(
            title_menu_build.report.development_input_available,
        ),
        title_menu_release_candidate_input_eligible: Some(
            title_menu_build.report.release_candidate_input_eligible,
        ),
        title_notice_development_input_available: Some(
            title_notice_build.report.development_input_available,
        ),
        title_notice_release_candidate_input_eligible: Some(
            title_notice_build.report.release_candidate_input_eligible,
        ),
        edit_runtime_text_development_input_available: Some(
            edit_runtime_text_build.report.development_input_available,
        ),
        edit_runtime_text_release_candidate_input_eligible: Some(
            edit_runtime_text_build
                .report
                .release_candidate_input_eligible,
        ),
        development_build_input_available: font_report.development_build_input_available
            && bonus_menu_build.report.development_input_available
            && bonus_inventory_build.report.development_input_available
            && bonus_inventory_action_labels_build
                .report
                .development_input_available
            && bonus_confirmation_build.report.development_input_available
            && bonus_inventory_stock_label_build
                .report
                .development_input_available
            && bonus_inventory_card_acquisition_build
                .report
                .development_input_available
            && bonus_inventory_memory_card_swap_build
                .report
                .development_input_available
            && bonus_j_bank_return_label_build
                .report
                .development_input_available
            && bonus_page_indicator_build
                .report
                .development_input_available
            && shop_ui_build.report.development_input_available
            && bonus_shop_exit_confirmation_build
                .report
                .development_input_available
            && bonus_shop_text_build.report.development_can_continue
            && diary_header_build.report.development_input_available
            && diary_scene_build.report.development_input_available
            && character_select_build.report.development_input_available
            && mode_descendant_build.report.development_input_available
            && practical_instruction_build.report.authored_member_count > 0
            && practical_instruction_build.report.all_source_members_bound
            && mode_select_build.development_input_available
            && options_build.report.development_input_available
            && title_menu_build.report.development_input_available
            && title_notice_build.report.development_input_available
            && edit_runtime_text_build.report.development_input_available,
        development_translation_input_available: font_report
            .development_translation_input_available
            && bonus_menu_build.report.development_input_available
            && bonus_inventory_build.report.development_input_available
            && bonus_inventory_action_labels_build
                .report
                .development_input_available
            && bonus_confirmation_build.report.development_input_available
            && bonus_inventory_stock_label_build
                .report
                .development_input_available
            && bonus_inventory_card_acquisition_build
                .report
                .development_input_available
            && bonus_inventory_memory_card_swap_build
                .report
                .development_input_available
            && bonus_j_bank_return_label_build
                .report
                .development_input_available
            && bonus_page_indicator_build
                .report
                .development_input_available
            && shop_ui_build.report.development_input_available
            && bonus_shop_exit_confirmation_build
                .report
                .development_input_available
            && bonus_shop_text_build
                .report
                .development_translation_input_available
            && diary_header_build.report.development_input_available
            && diary_scene_build.report.development_input_available
            && character_select_build.report.development_input_available
            && mode_descendant_build.report.development_input_available
            && practical_instruction_build.report.complete_scope
            && mode_select_build.development_input_available
            && options_build.report.development_input_available
            && title_menu_build.report.development_input_available
            && title_notice_build.report.development_input_available
            && edit_runtime_text_build.report.development_input_available,
        release_candidate_translation_input_eligible: font_report
            .release_candidate_translation_input_eligible
            && bonus_menu_build.report.release_candidate_input_eligible
            && bonus_inventory_build
                .report
                .release_candidate_input_eligible
            && bonus_inventory_action_labels_build
                .report
                .release_candidate_input_eligible
            && bonus_confirmation_build
                .report
                .release_candidate_input_eligible
            && bonus_inventory_stock_label_build
                .report
                .release_candidate_input_eligible
            && bonus_inventory_card_acquisition_build
                .report
                .release_candidate_input_eligible
            && bonus_inventory_memory_card_swap_build
                .report
                .release_candidate_input_eligible
            && bonus_j_bank_return_label_build
                .report
                .release_candidate_input_eligible
            && bonus_page_indicator_build
                .report
                .release_candidate_input_eligible
            && shop_ui_build.report.release_candidate_input_eligible
            && bonus_shop_exit_confirmation_build
                .report
                .release_candidate_input_eligible
            && bonus_shop_text_build
                .report
                .release_candidate_input_eligible
            && diary_header_build.report.release_candidate_input_eligible
            && diary_scene_build.report.release_candidate_input_eligible
            && character_select_build
                .report
                .release_candidate_input_eligible
            && mode_descendant_build
                .report
                .release_candidate_input_eligible
            && practical_instruction_build
                .report
                .release_candidate_input_eligible
            && mode_select_build.release_candidate_input_eligible
            && options_build.report.release_candidate_input_eligible
            && title_menu_build.report.release_candidate_input_eligible
            && title_notice_build.report.release_candidate_input_eligible
            && edit_runtime_text_build
                .report
                .release_candidate_input_eligible
            // Loading artwork still requires native and human review.
            && !replacements.iter().any(|r| r.path == crate::loading_art::PATH),
        compression_maximum_match_words: font_report.compression_maximum_match_words,
        compression_maximum_control_block_output_words: font_report
            .compression_maximum_control_block_output_words,
        fixed_code_consumer_ownership_complete: font_report.fixed_code_consumer_ownership_complete,
        replacement_record_count: rebuilt.len(),
        record_writes,
        changed_sector_count: changed_lbas.len(),
        changed_lba_ranges: contiguous_lba_ranges(&changed_lbas),
        changes_confined_to_replaced_records,
        changes_confined_to_declared_records_and_metadata,
        metadata_lbas,
        source_sector_count,
        output_sector_count,
        all_records_read_back: true,
        edc_ecc_verified: true,
        menu_surface_writes_disjoint: true,
        menu_decoded_changed_byte_ranges: development_menu.decoded_changed_byte_ranges.clone(),
        menu_source_compression_maximum_match_words: development_menu
            .source_compression
            .maximum_match_words,
        menu_source_compression_maximum_control_block_output_words: development_menu
            .source_compression
            .maximum_control_block_output_words,
        menu_source_compression_control_blocks_crossing_input_pages: development_menu
            .source_compression
            .control_blocks_crossing_input_pages,
        menu_source_compression_stream_byte_count: development_menu
            .source_compression
            .stream_byte_count,
        menu_rebuilt_compression_maximum_match_words: development_menu
            .rebuilt_compression
            .maximum_match_words,
        menu_rebuilt_compression_maximum_control_block_output_words: development_menu
            .rebuilt_compression
            .maximum_control_block_output_words,
        menu_rebuilt_compression_control_blocks_crossing_input_pages: development_menu
            .rebuilt_compression
            .control_blocks_crossing_input_pages,
        menu_rebuilt_compression_stream_byte_count: development_menu
            .rebuilt_compression
            .stream_byte_count,
        menu_unpadded_stored_size: development_menu.unpadded_stored_size,
        title_overlay_writes_disjoint: true,
        title_overlay_decoded_changed_byte_ranges: development_title_overlay
            .decoded_changed_byte_ranges
            .clone(),
        continue_name_runtime: Some(development_title_overlay.continue_names.clone()),
        title_contextual_glyph_upload: Some(development_title_overlay.runtime_glyph_upload.clone()),
        edit_tagged_name_runtime: Some(edit_runtime_text_build.report.tagged_name_runtime.clone()),
        dialogue_assets,
        dialogue_bundles: dialogue_bundle_readbacks,
        name_entry_font,
        name_entry,
        bonus_menu: Some(bonus_menu),
        bonus_inventory: Some(bonus_inventory),
        bonus_inventory_overlay: Some(bonus_inventory_overlay),
        shop_ui: Some(shop_ui),
        shop_overlay: Some(shop_overlay),
        diary_header: Some(diary_header),
        diary_scene: Some(diary_scene),
        diary_scene_runtime_bundles,
        diary_artwork_archives,
        character_select_records,
        character_select_overlays,
        mode_descendant_records,
        practical_instruction: Some(practical_instruction),
        practical_instruction_prompt_consumers,
        menu: Some(menu),
        options_overlay: Some(options_overlay),
        options_info: Some(options_info),
        edit_runtime_text_overlay: Some(edit_runtime_text_overlay),
        edit_runtime_text_pass: Some(edit_runtime_text_pass),
        title_menu_overlay: Some(title_menu_overlay),
        main_executable: Some(main_executable_readback),
        battle_name: Some(battle_name),
        dialogue_name_runtime: Some(dialogue_name_runtime_readback),
    };
    std::fs::write(
        outputs.staged_cue(),
        source_disc.rewritten_output_cue(output_cue, output_bin)?,
    )?;
    write_pretty_json_and_hash(outputs.staged_manifest(), &report, true)?;
    outputs.publish()?;
    Ok(report)
}

pub(super) fn contiguous_lba_ranges(lbas: &[u32]) -> Vec<[u32; 2]> {
    let Some(&first) = lbas.first() else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut start = first;
    let mut previous = first;
    for &lba in &lbas[1..] {
        if lba != previous + 1 {
            ranges.push([start, previous + 1]);
            start = lba;
        }
        previous = lba;
    }
    ranges.push([start, previous + 1]);
    ranges
}
