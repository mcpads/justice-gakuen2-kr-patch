use super::component_workers::join_component_builds;

use std::path::Path;

use anyhow::{Result, ensure};

use crate::bonus_confirmation::{
    BonusConfirmationBuild, BonusConfirmationBuildConfig,
    build_bonus_confirmation_from_inventory_source,
};
use crate::bonus_inventory::{
    BONUS_INVENTORY_PATH, BonusInventoryBuild, BonusInventoryBuildConfig,
    build_bonus_inventory_from_inventory_source,
};
use crate::bonus_inventory_action_labels::{
    BonusInventoryActionLabelBuild, BonusInventoryActionLabelBuildConfig,
    build_bonus_inventory_action_labels_from_inventory_source,
};
use crate::bonus_inventory_card_acquisition::{
    BonusInventoryCardAcquisitionBuild, BonusInventoryCardAcquisitionBuildConfig,
    build_bonus_inventory_card_acquisition_from_inventory_source,
};
use crate::bonus_inventory_memory_card_swap::{
    BonusInventoryMemoryCardSwapBuild, BonusInventoryMemoryCardSwapBuildConfig,
    build_bonus_inventory_memory_card_swap_from_inventory_source,
};
use crate::bonus_inventory_source::load_bonus_inventory_source;
use crate::bonus_inventory_stock_label::{
    BonusInventoryStockLabelBuild, BonusInventoryStockLabelBuildConfig,
    build_bonus_inventory_stock_label_from_inventory_source,
};
use crate::bonus_j_bank_return_label::{
    BonusJBankReturnLabelBuild, BonusJBankReturnLabelBuildConfig,
    build_bonus_j_bank_return_label_from_inventory_source,
};
use crate::bonus_page_indicator::{
    BonusPageIndicatorBuild, BonusPageIndicatorBuildConfig,
    build_bonus_page_indicator_from_inventory_source,
};
use crate::development_build_spec::LoadedDevelopmentBuildSpec;
use crate::source_disc::VerifiedSourceDisc;

use super::bonus_inventory_composition::{
    BonusInventorySurfaceBuild, load_bonus_inventory_surface_composition,
};

const FIXED_UI_OUTPUT_DIRECTORY: &str = "bonus-inventory";
const ACTION_LABELS_OUTPUT_DIRECTORY: &str = "bonus-inventory-action-labels";
const CARD_ACQUISITION_OUTPUT_DIRECTORY: &str = "bonus-inventory-card-acquisition";
const CONFIRMATION_OUTPUT_DIRECTORY: &str = "bonus-confirmation";
const J_BANK_RETURN_LABEL_OUTPUT_DIRECTORY: &str = "bonus-j-bank-return-label";
const MEMORY_CARD_SWAP_OUTPUT_DIRECTORY: &str = "bonus-inventory-memory-card-swap";
const PAGE_INDICATOR_OUTPUT_DIRECTORY: &str = "bonus-page-indicator";
const STOCK_LABEL_OUTPUT_DIRECTORY: &str = "bonus-inventory-stock-label";

pub(super) struct BonusInventoryDevelopmentBuild {
    pub fixed_ui: BonusInventoryBuild,
    pub action_labels: BonusInventoryActionLabelBuild,
    pub card_acquisition: BonusInventoryCardAcquisitionBuild,
    pub confirmation: BonusConfirmationBuild,
    pub j_bank_return_label: BonusJBankReturnLabelBuild,
    pub memory_card_swap: BonusInventoryMemoryCardSwapBuild,
    pub page_indicator: BonusPageIndicatorBuild,
    pub stock_label: BonusInventoryStockLabelBuild,
    pub surfaces: BonusInventorySurfaceBuild,
}

pub(super) fn build_bonus_inventory_development_surfaces(
    source_disc: &VerifiedSourceDisc,
    build_spec: &LoadedDevelopmentBuildSpec,
    output_directory: &Path,
    force: bool,
) -> Result<BonusInventoryDevelopmentBuild> {
    let source_cue = source_disc.snapshot_cue_path().to_path_buf();
    let source = load_bonus_inventory_source(source_disc.source())?;
    let fixed_ui_output_directory = output_directory.join(FIXED_UI_OUTPUT_DIRECTORY);
    let page_indicator_output_directory = output_directory.join(PAGE_INDICATOR_OUTPUT_DIRECTORY);
    let action_labels_output_directory = output_directory.join(ACTION_LABELS_OUTPUT_DIRECTORY);
    let confirmation_output_directory = output_directory.join(CONFIRMATION_OUTPUT_DIRECTORY);
    let stock_label_output_directory = output_directory.join(STOCK_LABEL_OUTPUT_DIRECTORY);
    let card_acquisition_output_directory =
        output_directory.join(CARD_ACQUISITION_OUTPUT_DIRECTORY);
    let memory_card_swap_output_directory =
        output_directory.join(MEMORY_CARD_SWAP_OUTPUT_DIRECTORY);
    let j_bank_return_label_output_directory =
        output_directory.join(J_BANK_RETURN_LABEL_OUTPUT_DIRECTORY);
    let (
        fixed_ui,
        page_indicator,
        action_labels,
        confirmation,
        stock_label,
        card_acquisition,
        memory_card_swap,
        j_bank_return_label,
    ) = std::thread::scope(|scope| -> Result<_> {
        let fixed_ui = scope.spawn(|| {
            build_bonus_inventory_from_inventory_source(
                &BonusInventoryBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_inventory.clone(),
                    fonts: build_spec.fonts.bonus_inventory.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: fixed_ui_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let page_indicator = scope.spawn(|| {
            build_bonus_page_indicator_from_inventory_source(
                &BonusPageIndicatorBuildConfig {
                    cue: source_cue.clone(),
                    asset: build_spec.assets.bonus_page_indicator.clone(),
                    font: build_spec.fonts.bonus_page_indicator.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: page_indicator_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let action_labels = scope.spawn(|| {
            build_bonus_inventory_action_labels_from_inventory_source(
                &BonusInventoryActionLabelBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_inventory_action_labels.clone(),
                    font: build_spec.fonts.bonus_inventory_action_labels.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: action_labels_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let confirmation = scope.spawn(|| {
            build_bonus_confirmation_from_inventory_source(
                &BonusConfirmationBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_confirmation.clone(),
                    font: build_spec.fonts.bonus_confirmation.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: confirmation_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let stock_label = scope.spawn(|| {
            build_bonus_inventory_stock_label_from_inventory_source(
                &BonusInventoryStockLabelBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_inventory_stock_label.clone(),
                    font: build_spec.fonts.bonus_inventory_stock_label.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: stock_label_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let card_acquisition = scope.spawn(|| {
            build_bonus_inventory_card_acquisition_from_inventory_source(
                &BonusInventoryCardAcquisitionBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_inventory_card_acquisition.clone(),
                    font: build_spec.fonts.bonus_inventory_card_acquisition.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: card_acquisition_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let memory_card_swap = scope.spawn(|| {
            build_bonus_inventory_memory_card_swap_from_inventory_source(
                &BonusInventoryMemoryCardSwapBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_inventory_memory_card_swap.clone(),
                    font: build_spec.fonts.bonus_inventory_memory_card_swap.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: memory_card_swap_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let j_bank_return_label = scope.spawn(|| {
            build_bonus_j_bank_return_label_from_inventory_source(
                &BonusJBankReturnLabelBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_j_bank_return_label.clone(),
                    font: build_spec.fonts.bonus_j_bank_return_label.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: j_bank_return_label_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        join_component_builds!(
            fixed_ui => "bonus inventory fixed UI",
            page_indicator => "bonus page indicator",
            action_labels => "bonus inventory action labels",
            confirmation => "bonus confirmation",
            stock_label => "bonus inventory stock label",
            card_acquisition => "bonus inventory card acquisition",
            memory_card_swap => "bonus inventory memory card swap",
            j_bank_return_label => "bonus J-BANK return label",
        )
    })?;
    ensure!(
        fixed_ui.report.development_input_available
            && fixed_ui.report.source_regions_match
            && fixed_ui
                .report
                .occurrence_cells_are_unique_and_non_overlapping
            && fixed_ui.report.untranslated_regions_unchanged
            && fixed_ui.report.changes_confined_to_owned_cells,
        "development disc build requires verified bonus-inventory fixed UI"
    );

    ensure!(
        page_indicator.report.development_input_available
            && page_indicator.report.source_regions_match
            && page_indicator.report.changes_confined_to_owned_regions,
        "development disc build requires a verified bonus page indicator"
    );

    ensure!(
        action_labels.report.development_input_available
            && action_labels.report.source_regions_match
            && action_labels
                .report
                .allocated_glyphs_are_unreferenced_by_source_consumers
            && action_labels.report.changes_confined_to_owned_regions,
        "development disc build requires verified bonus inventory action labels"
    );

    ensure!(
        confirmation.report.development_input_available
            && confirmation.report.source_regions_match
            && confirmation
                .report
                .allocated_glyphs_are_unreferenced_by_known_source_consumers
            && confirmation.report.changes_confined_to_owned_regions
            && confirmation.report.selected_card_prompt_translated
            && confirmation.report.memory_card_destination_translated
            && confirmation.report.memory_card_copy_prompt_translated
            && confirmation.report.fixed_record_space.remains_blank
            && !confirmation
                .report
                .fixed_record_space
                .included_in_expected_write_ranges,
        "development disc build requires a verified bonus confirmation"
    );

    ensure!(
        stock_label.report.development_input_available
            && stock_label.report.source_regions_match
            && stock_label
                .report
                .allocated_glyphs_are_unreferenced_by_known_source_consumers
            && stock_label
                .report
                .existing_bonus_glyph_allocations_are_disjoint
            && stock_label.report.changes_confined_to_owned_regions
            && stock_label.report.exit_state_skips_renderer,
        "development disc build requires a verified bonus inventory stock label"
    );

    let card_glyph_ownership = &card_acquisition.report.glyph_ownership_evidence;
    ensure!(
        card_acquisition.report.development_input_available
            && card_acquisition.report.source_command_records_match
            && card_glyph_ownership.declared_physical_alias_set_matches
            && card_glyph_ownership.existing_bonus_component_allocations_disjoint
            && card_glyph_ownership.pointer_command_table_parsed_glyphs_disjoint
            && card_glyph_ownership.declared_direct_selector_byte_region_scan_disjoint
            && card_acquisition
                .report
                .inventory_changes_confined_to_allocated_glyph_cells
            && card_acquisition
                .report
                .overlay_changes_confined_to_fixed_command_records
            && card_acquisition
                .report
                .reused_source_glyphs
                .iter()
                .all(|glyph| glyph.source_indexed_pixels_match_declared_hash),
        "development disc build requires a verified bonus inventory card-acquisition surface"
    );

    let memory_card_swap_glyph_ownership = &memory_card_swap.report.glyph_ownership_evidence;
    ensure!(
        memory_card_swap.report.development_input_available
            && memory_card_swap.report.source_command_records_match
            && memory_card_swap_glyph_ownership.declared_physical_alias_set_matches
            && memory_card_swap_glyph_ownership.existing_bonus_component_allocations_disjoint
            && memory_card_swap_glyph_ownership.pointer_command_table_parsed_glyphs_disjoint
            && memory_card_swap_glyph_ownership.declared_direct_selector_byte_region_scan_disjoint
            && memory_card_swap
                .report
                .inventory_changes_confined_to_allocated_glyph_cells
            && memory_card_swap
                .report
                .overlay_changes_confined_to_fixed_command_records
            && memory_card_swap
                .report
                .reused_source_glyphs
                .iter()
                .all(|glyph| glyph.source_indexed_pixels_match_declared_hash)
            && memory_card_swap.report.units.iter().all(|unit| {
                unit.output_payload_byte_length <= unit.source_storage_length
                    && unit.output_line_command_counts.iter().all(|command_count| {
                        *command_count <= memory_card_swap.report.renderer_line_command_limit
                            && *command_count * memory_card_swap.report.renderer_command_advance_px
                                <= memory_card_swap.report.renderer_line_width_px
                    })
            }),
        "development disc build requires a verified bonus inventory memory-card-swap surface"
    );

    let j_bank_glyph_ownership = &j_bank_return_label.report.glyph_ownership_evidence;
    ensure!(
        j_bank_return_label.report.development_input_available
            && j_bank_return_label.report.source_command_record_matches
            && j_bank_glyph_ownership.declared_physical_alias_set_matches
            && j_bank_glyph_ownership.existing_bonus_component_allocations_disjoint
            && j_bank_glyph_ownership.pointer_command_table_parsed_glyphs_disjoint
            && j_bank_glyph_ownership.declared_direct_selector_byte_region_scan_disjoint
            && j_bank_return_label
                .report
                .inventory_changes_confined_to_allocated_glyph_cells
            && j_bank_return_label
                .report
                .overlay_changes_confined_to_fixed_command_record
            && j_bank_return_label.report.unit.output_payload_byte_length
                <= j_bank_return_label.report.unit.source_storage_length
            && j_bank_return_label.report.unit.output_command_count > 0,
        "development disc build requires a verified bonus J-BANK return-label surface"
    );

    for (stage, reported_sha256) in [
        (
            "bonus inventory",
            fixed_ui.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus page indicator",
            page_indicator.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus inventory action labels",
            action_labels.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus confirmation",
            confirmation.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus inventory stock label",
            stock_label.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus inventory card acquisition",
            card_acquisition.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus inventory memory card swap",
            memory_card_swap.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus J-BANK return label",
            j_bank_return_label.report.source_bin_sha256.as_str(),
        ),
    ] {
        ensure!(
            reported_sha256 == source_disc.source_bin_sha256(),
            "{stage} build did not consume the verified source snapshot"
        );
    }
    ensure!(
        page_indicator.report.source_inventory_path == BONUS_INVENTORY_PATH
            && page_indicator.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && page_indicator.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && action_labels.report.source_inventory_path == BONUS_INVENTORY_PATH
            && action_labels.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && action_labels.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && action_labels.report.source_overlay_path
                == page_indicator.report.source_overlay_path
            && action_labels.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256
            && confirmation.report.source_inventory_path == BONUS_INVENTORY_PATH
            && confirmation.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && confirmation.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && confirmation.report.source_overlay_path == page_indicator.report.source_overlay_path
            && confirmation.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256
            && stock_label.report.source_inventory_path == BONUS_INVENTORY_PATH
            && stock_label.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && stock_label.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && stock_label.report.source_overlay_path == page_indicator.report.source_overlay_path
            && stock_label.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256
            && card_acquisition.report.source_inventory_path == BONUS_INVENTORY_PATH
            && card_acquisition.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && card_acquisition.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && card_acquisition.report.source_overlay_path
                == page_indicator.report.source_overlay_path
            && card_acquisition.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256
            && memory_card_swap.report.source_inventory_path == BONUS_INVENTORY_PATH
            && memory_card_swap.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && memory_card_swap.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && memory_card_swap.report.source_overlay_path
                == page_indicator.report.source_overlay_path
            && memory_card_swap.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256
            && j_bank_return_label.report.source_inventory_path == BONUS_INVENTORY_PATH
            && j_bank_return_label.report.source_inventory_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && j_bank_return_label.report.source_inventory_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && j_bank_return_label.report.source_overlay_path
                == page_indicator.report.source_overlay_path
            && j_bank_return_label.report.source_overlay_sha256
                == page_indicator.report.source_overlay_sha256,
        "bonus-inventory surfaces do not share one verified source"
    );

    let mut surfaces = load_bonus_inventory_surface_composition(
        &source,
        &fixed_ui,
        &page_indicator.report.source_overlay_path,
        &page_indicator.report.source_overlay_sha256,
    )?;
    surfaces.apply_surface_writer(
        "bonus page indicator",
        |inventory| page_indicator.apply_to_inventory_decoded(inventory),
        |overlay| page_indicator.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus inventory action labels",
        |inventory| action_labels.apply_to_inventory_decoded(inventory),
        |overlay| action_labels.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus confirmation",
        |inventory| confirmation.apply_to_inventory_decoded(inventory),
        |overlay| confirmation.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus inventory stock label",
        |inventory| stock_label.apply_to_inventory_decoded(inventory),
        |overlay| stock_label.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus inventory card acquisition",
        |inventory| card_acquisition.apply_to_inventory_decoded(inventory),
        |overlay| card_acquisition.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus inventory memory card swap",
        |inventory| memory_card_swap.apply_to_inventory_decoded(inventory),
        |overlay| memory_card_swap.apply_to_overlay(overlay),
    )?;
    surfaces.apply_surface_writer(
        "bonus J-BANK return label",
        |inventory| j_bank_return_label.apply_to_inventory_decoded(inventory),
        |overlay| j_bank_return_label.apply_to_overlay(overlay),
    )?;
    let surfaces = surfaces.finish()?;
    fixed_ui
        .device_text
        .validate_composed(&surfaces.inventory_decoded, &surfaces.overlay)?;
    // Per-writer checks use immutable source bytes; protect the blank against
    // the final union too, including writers that never touch its own report.
    crate::bonus_confirmation::validate_fixed_record_space_is_blank(&surfaces.inventory_decoded)?;

    Ok(BonusInventoryDevelopmentBuild {
        fixed_ui,
        action_labels,
        card_acquisition,
        confirmation,
        j_bank_return_label,
        memory_card_swap,
        page_indicator,
        stock_label,
        surfaces,
    })
}
