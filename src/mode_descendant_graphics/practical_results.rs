//! Source-bound result and score surfaces for the practical-exam subtree.

#[path = "practical_results/action_cells.rs"]
mod action_cells;
#[path = "practical_results/action_relocation.rs"]
mod action_relocation;
#[path = "practical_results/assets.rs"]
mod assets;
#[path = "practical_results/decorative_background.rs"]
mod decorative_background;
#[path = "practical_results/fixed_text.rs"]
mod fixed_text;
#[path = "practical_results/glyph_plan.rs"]
mod glyph_plan;
#[path = "practical_results/indexed_result_mirror.rs"]
mod indexed_result_mirror;
#[path = "practical_results/judgment_stamp.rs"]
mod judgment_stamp;
#[path = "practical_results/model.rs"]
mod model;
#[path = "practical_results/opaque_pointer_run_model.rs"]
mod opaque_pointer_run_model;
#[path = "practical_results/projection_model.rs"]
mod projection_model;
#[path = "practical_results/projection_validation.rs"]
mod projection_validation;
#[path = "practical_results/resident_page_mirror.rs"]
mod resident_page_mirror;
#[path = "practical_results/result_term_titles.rs"]
mod result_term_titles;
#[path = "practical_results/source_atlas_domain_model.rs"]
mod source_atlas_domain_model;
#[path = "practical_results/source_ownership.rs"]
mod source_ownership;

pub(in crate::mode_descendant_graphics) use indexed_result_mirror::mirror_canonical_result_cells_into_indexed_members;
pub use model::{
    PracticalResultBuildReport, PracticalResultDevelopmentStatus, PracticalResultReleaseStatus,
    PracticalResultSourceUsageStatus, PracticalResultStrategy, PracticalResultUnitBuild,
};
pub(in crate::mode_descendant_graphics) use result_term_titles::build_result_term_titles;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;

use super::catalog::{
    PRACTICAL_1999_DESCRIPTOR_PATH, PRACTICAL_BASICS_DESCRIPTOR_PATH,
    PRACTICAL_BASICS_RESULTS_PATH, PracticalResultRecordSpecs,
};
use super::model::ModeDescendantFontSources;
use super::practical_exam::{
    PracticalExamActiveTitleCachePlan, PracticalExamConsumer,
    external_secondary_descriptors_avoid_source_cell,
};
use super::source::ModeDescendantSourceRecord;

// The main practical-result overlay reads these finite cells from the rightmost
// page of the wide SIKEN1/SIKEN10 texture producer. Keep them unavailable to the
// practical-exam glyph allocator so the result compositor can update the resident
// provider without destroying dynamically allocated Hangul glyphs.
pub(super) const RESIDENT_RESULT_PROVIDER_CELLS: [crate::tim::Cell; 4] = [
    crate::tim::Cell {
        x: 512,
        y: 0,
        width: 240,
        height: 20,
    },
    crate::tim::Cell {
        x: 512,
        y: 20,
        width: 40,
        height: 20,
    },
    crate::tim::Cell {
        x: 616,
        y: 64,
        width: 56,
        height: 32,
    },
    crate::tim::Cell {
        x: 672,
        y: 64,
        width: 56,
        height: 32,
    },
];

pub(super) struct PracticalResultFixedTextBuild {
    pub(super) basics_texture_decoded: Vec<u8>,
    pub(super) basics_texture_decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) basics_results_decoded: Vec<u8>,
    pub(super) basics_results_decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) exam_1999_results_decoded: Vec<u8>,
    pub(super) exam_1999_results_decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) exam_1999_texture_decoded: Vec<u8>,
    pub(super) exam_1999_texture_decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) basics_consumer_overlay: Vec<u8>,
    pub(super) exam_1999_consumer_overlay: Vec<u8>,
    pub(super) report: PracticalResultBuildReport,
}

pub(super) struct PracticalResultFixedTextBases<'a> {
    pub(super) basics_texture: &'a [u8],
    pub(super) exam_1999_texture: &'a [u8],
    pub(super) basics_consumer: &'a [u8],
    pub(super) exam_1999_consumer: &'a [u8],
}

pub(super) struct PracticalResultPlan {
    assets: model::PracticalResultAssets,
    source_ownership: source_ownership::PracticalResultSourceOwnership,
    glyph_demand: glyph_plan::PracticalResultGlyphDemandPlan,
}

impl PracticalResultPlan {
    pub(super) fn load(mode_descendant_assets: &Path) -> Result<Self> {
        let assets = assets::load_practical_result_assets(
            &mode_descendant_assets.join("practical/results"),
        )?;
        let source_ownership = source_ownership::derive_source_ownership(
            &assets.entries,
            &assets.physical_region_catalog,
            &assets.source_glyph_catalog,
        )?;
        let glyph_demand = glyph_plan::derive_glyph_demand(&assets.entries)?;
        Ok(Self {
            assets,
            source_ownership,
            glyph_demand,
        })
    }

    pub(super) fn source_paths(&self) -> Vec<&str> {
        self.assets
            .sources
            .iter()
            .map(|source| source.source_path.as_str())
            .collect()
    }

    pub(super) fn validate_sources(
        &self,
        sources: &[ModeDescendantSourceRecord],
    ) -> Result<projection_validation::PracticalResultProjectionValidation> {
        assets::validate_practical_result_sources(&self.assets, &self.source_ownership, sources)?;
        projection_validation::validate_practical_result_projections(
            &self.assets,
            &self.source_ownership,
            sources,
        )
    }

    pub(super) fn validate_unread_active_title_cache(
        &self,
        sources: &[ModeDescendantSourceRecord],
        cache: &PracticalExamActiveTitleCachePlan,
    ) -> Result<()> {
        ensure!(
            cache.covers_all_term_titles(),
            "active-title cache has incomplete semantic coverage"
        );
        let projection_validation = self.validate_sources(sources)?;
        let cells = cache.result_write_cells()?;
        for cell in &cells {
            action_relocation::validate_target_is_unread(
                PRACTICAL_BASICS_RESULTS_PATH,
                0,
                4,
                *cell,
                &self.assets.physical_region_catalog,
                &self.assets.protected_content,
                &projection_validation.source_read_footprints,
            )
            .with_context(|| format!("active-title cache cell {cell:?} has another reader"))?;
        }
        for (path, consumer) in [
            (
                PRACTICAL_BASICS_DESCRIPTOR_PATH,
                PracticalExamConsumer::BasicsReview,
            ),
            (
                PRACTICAL_1999_DESCRIPTOR_PATH,
                PracticalExamConsumer::Exam1999,
            ),
        ] {
            let overlay = sources
                .iter()
                .find(|source| source.path == path)
                .with_context(|| format!("active-title cache consumer {path} was not loaded"))?;
            for cell in &cells {
                ensure!(
                    external_secondary_descriptors_avoid_source_cell(
                        &overlay.decoded,
                        consumer,
                        *cell,
                    )?,
                    "active-title cache overlaps an existing secondary-descriptor reader in {path}"
                );
            }
        }
        Ok(())
    }

    pub(super) fn manifest_sha256(&self) -> &str {
        &self.assets.manifest_sha256
    }

    pub(super) fn assess_sources(
        &self,
        result_specs: PracticalResultRecordSpecs,
        sources: &[ModeDescendantSourceRecord],
    ) -> Result<PracticalResultBuildReport> {
        self.report_sources(result_specs, sources, None, None, None, None)
    }

    pub(super) fn build_fixed_text(
        &self,
        result_specs: PracticalResultRecordSpecs,
        sources: &[ModeDescendantSourceRecord],
        fonts: &ModeDescendantFontSources,
        bases: PracticalResultFixedTextBases<'_>,
    ) -> Result<PracticalResultFixedTextBuild> {
        let projection_validation = self.validate_sources(sources)?;
        let basics_texture_source = sources
            .iter()
            .find(|source| source.path == result_specs.basics_texture_results.source_path)
            .context("practical-result SIKEN1 source was not loaded")?;
        let exam_1999_texture_source = sources
            .iter()
            .find(|source| source.path == result_specs.exam_1999_texture_results.source_path)
            .context("practical-result SIKEN10 source was not loaded")?;
        let basics_results_source = sources
            .iter()
            .find(|source| source.path == result_specs.basics_results.source_path)
            .context("practical-result SIKEN2 source was not loaded")?;
        let exam_1999_results_source = sources
            .iter()
            .find(|source| source.path == result_specs.exam_1999_results.source_path)
            .context("practical-result SIKEN20 source was not loaded")?;
        let basics_texture_rendered = fixed_text::render_fixed_text_entries(
            &self.assets.entries,
            &self.assets.fixed_text_target_catalog,
            fonts,
            basics_texture_source,
            bases.basics_texture,
        )?;
        let exam_1999_texture_rendered = fixed_text::render_fixed_text_entries(
            &self.assets.entries,
            &self.assets.fixed_text_target_catalog,
            fonts,
            exam_1999_texture_source,
            bases.exam_1999_texture,
        )?;
        let basics_fixed_text_rendered = fixed_text::render_fixed_text_entries(
            &self.assets.entries,
            &self.assets.fixed_text_target_catalog,
            fonts,
            basics_results_source,
            &basics_results_source.decoded,
        )?;
        let exam_1999_mixed_background = decorative_background::render_decorative_backgrounds(
            &self.assets.entries,
            &self.assets.decorative_background_target_catalog,
            fonts,
            exam_1999_results_source,
            &exam_1999_results_source.decoded,
            model::PracticalResultDecorativeCompositionStage::BeforeFixedText,
        )?;
        let exam_1999_results_rendered = fixed_text::render_fixed_text_entries(
            &self.assets.entries,
            &self.assets.fixed_text_target_catalog,
            fonts,
            exam_1999_results_source,
            &exam_1999_mixed_background.decoded,
        )?;
        let basics_judgment_rendered = judgment_stamp::render_judgment_stamp_entries(
            &self.assets.entries,
            &self.assets.judgment_stamp_target_catalog,
            fonts,
            basics_results_source,
            &basics_fixed_text_rendered.decoded,
        )?;
        let exam_1999_judgment_rendered = judgment_stamp::render_judgment_stamp_entries(
            &self.assets.entries,
            &self.assets.judgment_stamp_target_catalog,
            fonts,
            exam_1999_results_source,
            &exam_1999_results_rendered.decoded,
        )?;
        let action_entries = self.in_place_action_entries()?;
        let basics_action_rendered = action_cells::render_action_cell_entries(
            &action_entries,
            &self.assets.physical_region_catalog,
            fonts,
            basics_results_source,
            &basics_judgment_rendered.decoded,
        )?;
        let relocated = action_relocation::relocate_action_entries(
            &self.assets.entries,
            &self.assets.action_cell_target_catalog,
            &self.assets.action_consumer_occurrence_catalog,
            &self.assets.physical_region_catalog,
            &self.assets.protected_content,
            &projection_validation.source_read_footprints,
            fonts,
            sources,
            basics_results_source,
            &basics_action_rendered.decoded,
            bases.basics_consumer,
            bases.exam_1999_consumer,
        )?;
        let basics_consumer_overlay = relocated.basics_consumer_overlay;
        let exam_1999_consumer_overlay = relocated.exam_1999_consumer_overlay;
        let basics_action_rendered =
            action_cells::merge_rendered_action_cells(basics_action_rendered, relocated.rendered)?;
        let observed_label_entries = self.in_place_observed_label_entries()?;
        let observed_labels = action_cells::render_independent_label_sequences(
            &observed_label_entries,
            &self.assets.physical_region_catalog,
            fonts,
            basics_results_source,
            &basics_action_rendered.decoded,
        )?;
        let basics_action_rendered =
            action_cells::merge_rendered_action_cells(basics_action_rendered, observed_labels)?;
        let completion = fixed_text::merge_rendered_fixed_text(&[
            &basics_texture_rendered,
            &exam_1999_texture_rendered,
            &basics_fixed_text_rendered,
            &exam_1999_results_rendered,
        ])?;
        let judgment_completion = judgment_stamp::merge_rendered_judgment_stamps(&[
            &basics_judgment_rendered,
            &exam_1999_judgment_rendered,
        ])?;
        let basics_resident_page = resident_page_mirror::mirror_bound_result_cells(
            "basics",
            &basics_results_source.decoded,
            &basics_action_rendered.decoded,
            &basics_texture_source.decoded,
            &basics_texture_rendered.decoded,
        )?;
        let exam_1999_resident_page = resident_page_mirror::mirror_bound_result_cells(
            "exam-1999",
            &basics_results_source.decoded,
            &basics_action_rendered.decoded,
            &exam_1999_texture_source.decoded,
            &exam_1999_texture_rendered.decoded,
        )?;
        ensure!(
            basics_resident_page.mirrored_cell_count == 4
                && exam_1999_resident_page.mirrored_cell_count == 4,
            "practical-result resident provider mirror omitted a finite cell"
        );
        let basics_texture_decorated = decorative_background::render_decorative_backgrounds(
            &self.assets.entries,
            &self.assets.decorative_background_target_catalog,
            fonts,
            basics_texture_source,
            &basics_resident_page.decoded,
            model::PracticalResultDecorativeCompositionStage::AfterFixedText,
        )?;
        let exam_1999_texture_decorated = decorative_background::render_decorative_backgrounds(
            &self.assets.entries,
            &self.assets.decorative_background_target_catalog,
            fonts,
            exam_1999_texture_source,
            &exam_1999_resident_page.decoded,
            model::PracticalResultDecorativeCompositionStage::AfterFixedText,
        )?;
        let basics_results_decorated = decorative_background::render_decorative_backgrounds(
            &self.assets.entries,
            &self.assets.decorative_background_target_catalog,
            fonts,
            basics_results_source,
            &basics_action_rendered.decoded,
            model::PracticalResultDecorativeCompositionStage::AfterFixedText,
        )?;
        let exam_1999_results_decorated = decorative_background::render_decorative_backgrounds(
            &self.assets.entries,
            &self.assets.decorative_background_target_catalog,
            fonts,
            exam_1999_results_source,
            &exam_1999_judgment_rendered.decoded,
            model::PracticalResultDecorativeCompositionStage::AfterFixedText,
        )?;
        let decorative_completion =
            decorative_background::merge_rendered_decorative_backgrounds(&[
                &basics_texture_decorated,
                &exam_1999_texture_decorated,
                &basics_results_decorated,
                &exam_1999_mixed_background,
                &exam_1999_results_decorated,
            ])?;
        let mut report = self.report_sources(
            result_specs,
            sources,
            Some(&completion),
            Some(&judgment_completion),
            Some(&basics_action_rendered),
            Some(&decorative_completion),
        )?;
        report.resident_result_provider_cell_count =
            basics_resident_page.mirrored_cell_count + exam_1999_resident_page.mirrored_cell_count;
        report.resident_result_provider_expected_write_count =
            basics_resident_page.claims.len() + exam_1999_resident_page.claims.len();
        report.resident_result_provider_write_contract_complete =
            report.resident_result_provider_cell_count == RESIDENT_RESULT_PROVIDER_CELLS.len() * 2
                && report.resident_result_provider_expected_write_count > 0;
        let mut basics_results_decoded_write_claims =
            basics_fixed_text_rendered.decoded_write_claims;
        basics_results_decoded_write_claims.extend(basics_judgment_rendered.decoded_write_claims);
        basics_results_decoded_write_claims.extend(basics_action_rendered.decoded_write_claims);
        basics_results_decoded_write_claims.extend(basics_results_decorated.decoded_write_claims);
        let mut exam_1999_results_decoded_write_claims =
            exam_1999_mixed_background.decoded_write_claims;
        exam_1999_results_decoded_write_claims
            .extend(exam_1999_results_rendered.decoded_write_claims);
        exam_1999_results_decoded_write_claims =
            compose_result_backdrop_claims(exam_1999_results_decoded_write_claims);
        exam_1999_results_decoded_write_claims
            .extend(exam_1999_judgment_rendered.decoded_write_claims);
        exam_1999_results_decoded_write_claims
            .extend(exam_1999_results_decorated.decoded_write_claims);
        Ok(PracticalResultFixedTextBuild {
            basics_texture_decoded: basics_texture_decorated.decoded,
            basics_texture_decoded_write_claims: basics_texture_rendered
                .decoded_write_claims
                .into_iter()
                .chain(basics_resident_page.claims)
                .chain(basics_texture_decorated.decoded_write_claims)
                .collect(),
            basics_results_decoded: basics_results_decorated.decoded,
            basics_results_decoded_write_claims,
            exam_1999_results_decoded: exam_1999_results_decorated.decoded,
            exam_1999_results_decoded_write_claims,
            exam_1999_texture_decoded: exam_1999_texture_decorated.decoded,
            exam_1999_texture_decoded_write_claims: exam_1999_texture_rendered
                .decoded_write_claims
                .into_iter()
                .chain(exam_1999_resident_page.claims)
                .chain(exam_1999_texture_decorated.decoded_write_claims)
                .collect(),
            basics_consumer_overlay,
            exam_1999_consumer_overlay,
            report,
        })
    }

    fn report_sources(
        &self,
        result_specs: PracticalResultRecordSpecs,
        sources: &[ModeDescendantSourceRecord],
        rendered: Option<&fixed_text::FixedTextCompletion>,
        rendered_judgments: Option<&judgment_stamp::JudgmentStampCompletion>,
        rendered_actions: Option<&action_cells::RenderedActionCells>,
        rendered_decorative: Option<&decorative_background::DecorativeBackgroundCompletion>,
    ) -> Result<PracticalResultBuildReport> {
        ensure!(
            self.source_paths()
                .contains(&result_specs.basics_texture_results.source_path)
                && self
                    .source_paths()
                    .contains(&result_specs.basics_results.source_path)
                && self
                    .source_paths()
                    .contains(&result_specs.exam_1999_results.source_path),
            "practical-result record roles do not cover both result consumers"
        );
        let projection_validation = self.validate_sources(sources)?;
        let no_completed_targets = BTreeSet::new();
        let fixed = fixed_text::plan_fixed_text_entries(
            &self.assets.entries,
            &self.assets.fixed_text_target_catalog,
            rendered
                .map(|rendered| &rendered.completed_target_ids)
                .unwrap_or(&no_completed_targets),
        )?;
        let no_completed_judgment_targets = BTreeSet::new();
        let judgments = judgment_stamp::plan_judgment_stamp_entries(
            &self.assets.entries,
            &self.assets.judgment_stamp_target_catalog,
            rendered_judgments
                .map(|rendered| &rendered.completed_target_ids)
                .unwrap_or(&no_completed_judgment_targets),
        )?;

        let mut authored_unit_count = 0usize;
        let mut unresolved_unit_count = 0usize;
        let mut fixed_text_unit_count = 0usize;
        let mut deferred_fixed_text_unit_count = 0usize;
        let mut deferred_glyph_sequence_unit_count = 0usize;
        let mut authored_glyph_sequence_unit_count = 0usize;
        let rendered_glyph_sequence_unit_count = rendered_actions
            .map(|rendered| rendered.completed_entry_ids.len())
            .unwrap_or(0);
        let deferred_judgment_unit_count = judgments.deferred_entry_ids.len();
        let mut deferred_decorative_mask_unit_count = 0usize;
        let rendered_fixed_text_unit_count = rendered
            .map(|rendered| rendered.changed_bytes_by_entry.len())
            .unwrap_or(0);
        let rendered_judgment_unit_count = rendered_judgments
            .map(|rendered| rendered.changed_bytes_by_entry.len())
            .unwrap_or(0);
        let mut rendered_source_reference_count = 0usize;
        let mut source_reference_count = 0usize;
        let mut located_source_reference_count = 0usize;
        let mut unresolved_source_reference_count = 0usize;
        let mut indexed_region_hash_matched_reference_count = 0usize;
        let mut runtime_observed_source_reference_count = 0usize;
        let mut static_observed_source_reference_count = 0usize;
        let mut unresolved_source_usage_reference_count = 0usize;
        let mut units = Vec::with_capacity(self.assets.entries.len());
        for entry in &self.assets.entries {
            match entry.development_status {
                model::PracticalResultDevelopmentStatus::Authored => authored_unit_count += 1,
                model::PracticalResultDevelopmentStatus::Unresolved => unresolved_unit_count += 1,
            }
            match entry.strategy {
                model::PracticalResultStrategy::FixedText => {
                    fixed_text_unit_count += 1;
                    if fixed.deferred_entry_ids.contains(&entry.id) {
                        deferred_fixed_text_unit_count += 1;
                    }
                }
                model::PracticalResultStrategy::GlyphSequence => {
                    if !rendered_actions.is_some_and(|rendered| {
                        rendered.completed_entry_ids.contains(entry.id.as_str())
                    }) {
                        deferred_glyph_sequence_unit_count += 1;
                    }
                    if entry.development_status == model::PracticalResultDevelopmentStatus::Authored
                    {
                        authored_glyph_sequence_unit_count += 1;
                    }
                }
                model::PracticalResultStrategy::JudgmentStamp => {}
                model::PracticalResultStrategy::DecorativeMask => {
                    if !rendered_decorative.is_some_and(|rendered| {
                        rendered
                            .rendered_references_by_entry
                            .get(&entry.id)
                            .is_some_and(|count| *count == entry.source_references.len())
                    }) {
                        deferred_decorative_mask_unit_count += 1;
                    }
                }
            }
            let rendered_fixed_source_references = rendered
                .and_then(|rendered| rendered.rendered_references_by_entry.get(&entry.id))
                .copied()
                .unwrap_or(0);
            let rendered_judgment_source_references = rendered_judgments
                .and_then(|rendered| rendered.rendered_references_by_entry.get(&entry.id))
                .copied()
                .unwrap_or(0);
            let rendered_source_references = rendered_fixed_source_references
                + rendered_judgment_source_references
                + rendered_actions
                    .and_then(|rendered| rendered.rendered_references_by_entry.get(&entry.id))
                    .copied()
                    .unwrap_or(0)
                + rendered_decorative
                    .and_then(|rendered| rendered.rendered_references_by_entry.get(&entry.id))
                    .copied()
                    .unwrap_or(0);
            rendered_source_reference_count += rendered_source_references;
            let located_source_references = entry.source_references.len();
            let unresolved_source_references = entry.unresolved_source_references.len();
            let indexed_region_hash_matched_references = located_source_references;
            let statuses = entry
                .source_references
                .iter()
                .map(|reference| reference.source_usage_status)
                .chain(
                    entry
                        .unresolved_source_references
                        .iter()
                        .map(|reference| reference.source_usage_status),
                )
                .collect::<Vec<_>>();
            let runtime_observed_source_references = statuses
                .iter()
                .filter(|status| {
                    **status == model::PracticalResultSourceUsageStatus::RuntimeObserved
                })
                .count();
            let static_observed_source_references = statuses
                .iter()
                .filter(|status| {
                    **status == model::PracticalResultSourceUsageStatus::StaticObserved
                })
                .count();
            let unresolved_source_usage_references = statuses
                .iter()
                .filter(|status| **status == model::PracticalResultSourceUsageStatus::Unresolved)
                .count();
            source_reference_count += located_source_references + unresolved_source_references;
            located_source_reference_count += located_source_references;
            unresolved_source_reference_count += unresolved_source_references;
            indexed_region_hash_matched_reference_count += indexed_region_hash_matched_references;
            runtime_observed_source_reference_count += runtime_observed_source_references;
            static_observed_source_reference_count += static_observed_source_references;
            unresolved_source_usage_reference_count += unresolved_source_usage_references;
            units.push(model::PracticalResultUnitBuild {
                id: entry.id.clone(),
                strategy: entry.strategy,
                source_text: entry.source_text.clone(),
                korean_text: entry.korean_text.clone(),
                font_role: entry.font_role,
                development_status: entry.development_status,
                release_status: entry.release_status,
                source_reference_count: located_source_references + unresolved_source_references,
                located_source_reference_count: located_source_references,
                unresolved_source_reference_count: unresolved_source_references,
                indexed_region_hash_matched_reference_count: indexed_region_hash_matched_references,
                runtime_observed_source_reference_count: runtime_observed_source_references,
                static_observed_source_reference_count: static_observed_source_references,
                unresolved_source_usage_reference_count: unresolved_source_usage_references,
                rendered_source_reference_count: rendered_source_references,
                changed_decoded_byte_count: rendered
                    .and_then(|rendered| rendered.changed_bytes_by_entry.get(&entry.id))
                    .copied()
                    .unwrap_or(0)
                    + rendered_judgments
                        .and_then(|rendered| rendered.changed_bytes_by_entry.get(&entry.id))
                        .copied()
                        .unwrap_or(0)
                    + rendered_actions
                        .and_then(|rendered| rendered.changed_bytes_by_entry.get(&entry.id))
                        .copied()
                        .unwrap_or(0)
                    + rendered_decorative
                        .and_then(|rendered| rendered.changed_bytes_by_entry.get(&entry.id))
                        .copied()
                        .unwrap_or(0),
            });
        }
        ensure!(
            deferred_fixed_text_unit_count == fixed.deferred_entry_ids.len(),
            "practical-result fixed-text mask denominator changed"
        );
        ensure!(
            located_source_reference_count == self.source_ownership.semantic_reference_count(),
            "practical-result semantic source-reference denominator changed"
        );
        ensure!(
            authored_glyph_sequence_unit_count == self.glyph_demand.sequences.len(),
            "practical-result Korean glyph-demand denominator changed"
        );
        let completed_fixed_text_expected_write_count = fixed.expected_write_count;
        let completed_judgment_stamp_expected_write_count = judgments.expected_write_count;
        let completed_action_cell_expected_write_count = rendered_actions
            .map(|rendered| rendered.completed_cell_write_count)
            .unwrap_or(0);
        let rendered_decorative_background_target_count = rendered_decorative
            .map(|rendered| rendered.completed_target_count)
            .unwrap_or(0);
        let completed_decorative_background_expected_write_count = rendered_decorative
            .map(|rendered| rendered.expected_write_count)
            .unwrap_or(0);
        let rendered_decorative_background_text_placement_count = rendered_decorative
            .map(|rendered| rendered.rendered_text_placement_count)
            .unwrap_or(0);
        let decorative_background_palettes_preserved = rendered_decorative
            .map(|rendered| rendered.palettes_preserved)
            .unwrap_or(false);
        let decorative_background_changes_confined_to_owned_cells = rendered_decorative
            .map(|rendered| rendered.changes_confined_to_owned_cells)
            .unwrap_or(false);
        let shared_action_suffix_cell_count = rendered_actions
            .map(|rendered| rendered.shared_suffix_cell_count)
            .unwrap_or(0);
        let korean_atlas_slot_assignment_count = 0usize;
        let consumer_projection_rewrite_count = rendered_actions
            .map(|rendered| rendered.consumer_projection_rewrite_count)
            .unwrap_or(0);
        let dynamic_atlas_plan_complete = false;
        let rendered_fixed_cells_are_unique_and_non_overlapping = rendered
            .map(|rendered| rendered.cells_are_unique_and_non_overlapping)
            .unwrap_or(false);
        let rendered_fixed_changes_confined_to_owned_cells = rendered
            .map(|rendered| rendered.changes_confined_to_owned_cells)
            .unwrap_or(false);
        let rendered_judgment_cells_are_unique_and_non_overlapping = rendered_judgments
            .map(|rendered| rendered.cells_are_unique_and_non_overlapping)
            .unwrap_or(false);
        let rendered_judgment_changes_confined_to_owned_cells = rendered_judgments
            .map(|rendered| rendered.changes_confined_to_owned_cells)
            .unwrap_or(false);
        let rendered_action_cell_writes_are_unique_and_non_overlapping = rendered_actions
            .map(|rendered| rendered.cell_writes_are_unique_and_non_overlapping)
            .unwrap_or(false);
        let rendered_action_changes_confined_to_owned_cells = rendered_actions
            .map(|rendered| rendered.changes_confined_to_owned_cells)
            .unwrap_or(false);
        let source_reference_without_indexed_region_count = source_reference_count
            .checked_sub(indexed_region_hash_matched_reference_count)
            .context("practical-result indexed-region denominator underflow")?;
        let all_source_references_have_hash_matched_indexed_regions =
            source_reference_without_indexed_region_count == 0;
        let source_records_match = true;
        let fixed_text_expected_writes_complete = fixed.write_contract_complete;
        let judgment_stamp_expected_writes_complete = judgments.write_contract_complete;
        let derived_source_read_footprint_count =
            projection_validation.source_read_footprints.len();
        let fixed_text_insertion_complete = fixed_text_expected_writes_complete
            && rendered_fixed_text_unit_count == fixed_text_unit_count
            && rendered_fixed_cells_are_unique_and_non_overlapping
            && rendered_fixed_changes_confined_to_owned_cells;
        let judgment_stamp_insertion_complete = judgment_stamp_expected_writes_complete
            && rendered_judgment_cells_are_unique_and_non_overlapping
            && rendered_judgment_changes_confined_to_owned_cells;
        let glyph_projection_insertion_complete = projection_validation
            .authored_glyph_sequence_projection_coverage_complete
            && projection_validation.action_consumer_occurrence_coverage_complete
            && projection_validation.complete
            && dynamic_atlas_plan_complete
            && korean_atlas_slot_assignment_count == self.glyph_demand.glyphs.len()
            && consumer_projection_rewrite_count
                == projection_validation.semantic_bound_consumer_projection_count;
        let static_patch_insertion_complete = unresolved_unit_count == 0
            && deferred_fixed_text_unit_count == 0
            && deferred_glyph_sequence_unit_count == 0
            && deferred_judgment_unit_count == 0
            && deferred_decorative_mask_unit_count == 0
            && all_source_references_have_hash_matched_indexed_regions
            && source_records_match
            && unresolved_source_usage_reference_count == 0
            && fixed_text_insertion_complete
            && judgment_stamp_insertion_complete
            && glyph_projection_insertion_complete;
        Ok(PracticalResultBuildReport {
            kind: "Justice Gakuen 2 practical-result development build".to_string(),
            translation_manifest_sha256: self.manifest_sha256().to_string(),
            physical_region_catalog_sha256: self.assets.physical_region_catalog_sha256.clone(),
            source_atlas_domain_catalog_sha256: self
                .assets
                .source_atlas_domain_catalog_sha256
                .clone(),
            declared_source_atlas_domain_count: projection_validation.source_atlas_domain_count,
            domain_bound_physical_region_count: projection_validation
                .domain_bound_physical_region_count,
            physical_region_without_declared_source_atlas_domain_count: projection_validation
                .physical_region_without_declared_source_atlas_domain_count,
            domain_bound_protected_region_count: projection_validation
                .domain_bound_protected_region_count,
            domain_bound_source_glyph_bank_count: projection_validation
                .domain_bound_source_glyph_bank_count,
            domain_bound_vram_residency_count: projection_validation
                .domain_bound_vram_residency_count,
            domain_bound_clut_binding_count: projection_validation.domain_bound_clut_binding_count,
            external_clut_producer_family_count: projection_validation
                .external_clut_producer_family_count,
            external_clut_palette_family_sha256s: projection_validation
                .external_clut_palette_family_sha256s,
            unresolved_external_clut_binding_count: projection_validation
                .unresolved_external_clut_binding_count,
            domain_bound_consumer_descriptor_count: projection_validation
                .domain_bound_consumer_descriptor_count,
            domain_bound_consumer_projection_count: projection_validation
                .domain_bound_consumer_projection_count,
            alternative_consumer_source_binding_count: projection_validation
                .alternative_consumer_source_binding_count,
            assessed_source_projection_read_count: projection_validation
                .assessed_source_projection_read_count,
            unassessed_source_projection_read_count: projection_validation
                .unassessed_source_projection_read_count,
            declared_read_set_complete_source_projection_count: projection_validation
                .declared_read_set_complete_source_projection_count,
            consumer_reachability_closed_source_projection_count: projection_validation
                .consumer_reachability_closed_source_projection_count,
            consumer_reachability_unassessed_source_projection_count: projection_validation
                .consumer_reachability_unassessed_source_projection_count,
            consumer_reachability_unresolved_source_projection_count: projection_validation
                .consumer_reachability_unresolved_source_projection_count,
            consumer_reachability_dormant_source_projection_count: projection_validation
                .consumer_reachability_dormant_source_projection_count,
            source_projection_read_assessment_coverage_complete: projection_validation
                .source_projection_read_assessment_coverage_complete,
            source_atlas_in_place_rewrite_gate_open_domain_count: projection_validation
                .source_atlas_in_place_rewrite_gate_open_domain_count,
            source_atlas_in_place_rewrite_gate_blocked_domain_count: projection_validation
                .source_atlas_in_place_rewrite_gate_blocked_domain_count,
            source_atlas_in_place_rewrite_gates: projection_validation
                .source_atlas_in_place_rewrite_gates,
            derived_source_read_footprint_count,
            source_read_footprints: projection_validation.source_read_footprints,
            source_atlas_evidence_joins_complete: projection_validation
                .source_atlas_evidence_joins_complete,
            indexed_result_selected_member_count: projection_validation
                .indexed_result_selected_member_count,
            indexed_result_known_post_upload_descriptor_aliases: projection_validation
                .indexed_result_known_post_upload_descriptor_aliases,
            indexed_result_direct_descriptor_render_call_offsets: projection_validation
                .indexed_result_direct_descriptor_render_call_offsets,
            indexed_result_known_post_upload_draw_path_validated: projection_validation
                .indexed_result_known_post_upload_draw_path_validated,
            indexed_result_delegated_overlay_callback_index: projection_validation
                .indexed_result_delegated_overlay_callback_index,
            indexed_result_delegated_overlay_dispatch_offset: projection_validation
                .indexed_result_delegated_overlay_dispatch_offset,
            indexed_result_tim_upload_call_offsets: projection_validation
                .indexed_result_tim_upload_call_offsets,
            indexed_result_validated_result_title_member_count: projection_validation
                .indexed_result_validated_result_title_member_count,
            indexed_result_texture_lifetime_validated: projection_validation
                .indexed_result_texture_lifetime_validated,
            opaque_pointer_run_catalog_sha256: self
                .assets
                .opaque_pointer_run_catalog_sha256
                .clone(),
            declared_opaque_pointer_run_count: projection_validation
                .declared_opaque_pointer_run_count,
            source_atlas_excluded_opaque_pointer_run_count: projection_validation
                .source_atlas_excluded_opaque_pointer_run_count,
            unresolved_opaque_pointer_run_count: projection_validation
                .unresolved_opaque_pointer_run_count,
            source_glyph_catalog_sha256: self.assets.source_glyph_catalog_sha256.clone(),
            cataloged_source_glyph_cell_count: self
                .assets
                .source_glyph_catalog
                .banks
                .iter()
                .flat_map(|bank| &bank.rows)
                .map(|row| row.glyphs.chars().count())
                .sum(),
            fixed_text_target_catalog_sha256: self.assets.fixed_text_target_catalog_sha256.clone(),
            fixed_text_consumer_occurrence_catalog_sha256: self
                .assets
                .fixed_text_consumer_occurrence_catalog_sha256
                .clone(),
            judgment_stamp_target_catalog_sha256: self
                .assets
                .judgment_stamp_target_catalog_sha256
                .clone(),
            judgment_stamp_consumer_occurrence_catalog_sha256: self
                .assets
                .judgment_stamp_consumer_occurrence_catalog_sha256
                .clone(),
            action_cell_target_catalog_sha256: self
                .assets
                .action_cell_target_catalog_sha256
                .clone(),
            validated_action_cell_target_count: self
                .assets
                .action_cell_target_catalog
                .targets
                .len(),
            decorative_background_target_catalog_sha256: self
                .assets
                .decorative_background_target_catalog_sha256
                .clone(),
            validated_decorative_background_target_count: self
                .assets
                .decorative_background_target_catalog
                .targets
                .len(),
            rendered_decorative_background_target_count,
            completed_decorative_background_expected_write_count,
            rendered_decorative_background_text_placement_count,
            decorative_background_palettes_preserved,
            decorative_background_changes_confined_to_owned_cells,
            validated_judgment_stamp_consumer_occurrence_count: self
                .assets
                .judgment_stamp_consumer_occurrence_catalog
                .occurrences
                .len(),
            validated_judgment_stamp_target_count: judgments.validated_target_count,
            judgment_stamp_layout_candidate_count: judgments.layout_candidate_count,
            completed_judgment_stamp_expected_write_count,
            validated_fixed_text_consumer_occurrence_count: self
                .assets
                .fixed_text_consumer_occurrence_catalog
                .occurrences
                .len(),
            validated_fixed_text_target_count: fixed.validated_target_count,
            fixed_text_layout_candidate_count: fixed.layout_candidate_count,
            completed_fixed_text_expected_write_count,
            vram_residency_catalog_sha256: self.assets.vram_residency_catalog_sha256.clone(),
            clut_binding_catalog_sha256: self.assets.clut_binding_catalog_sha256.clone(),
            action_consumer_occurrence_catalog_sha256: self
                .assets
                .action_consumer_occurrence_catalog_sha256
                .clone(),
            consumer_projection_catalog_sha256: self
                .assets
                .consumer_projection_catalog_sha256
                .clone(),
            declared_vram_residency_count: projection_validation.vram_residency_count,
            declared_clut_binding_count: self.assets.clut_binding_catalog.bindings.len(),
            resolved_clut_binding_count: projection_validation.resolved_clut_binding_count,
            declared_consumer_descriptor_count: projection_validation.consumer_descriptor_count,
            declared_consumer_projection_count: projection_validation.consumer_projection_count,
            declared_action_consumer_occurrence_count: projection_validation
                .declared_action_consumer_occurrence_count,
            validated_action_consumer_occurrence_count: projection_validation
                .validated_action_consumer_occurrence_count,
            action_consumer_occurrence_coverage_complete: projection_validation
                .action_consumer_occurrence_coverage_complete,
            source_unit_count: self.assets.entries.len(),
            authored_unit_count,
            unresolved_unit_count,
            fixed_text_unit_count,
            rendered_fixed_text_unit_count,
            deferred_fixed_text_unit_count,
            deferred_glyph_sequence_unit_count,
            authored_glyph_sequence_unit_count,
            rendered_glyph_sequence_unit_count,
            completed_action_cell_expected_write_count,
            shared_action_suffix_cell_count,
            resident_result_provider_cell_count: 0,
            resident_result_provider_expected_write_count: 0,
            resident_result_provider_write_contract_complete: false,
            indexed_result_member_count: 0,
            indexed_result_mirrored_cell_count: 0,
            indexed_result_expected_write_count: 0,
            indexed_result_write_contract_complete: false,
            result_term_titles: None,
            korean_sequence_token_count: self.glyph_demand.sequence_token_count(),
            unique_korean_glyph_demand_count: self.glyph_demand.glyphs.len(),
            korean_atlas_slot_assignment_count,
            consumer_projection_rewrite_count,
            dynamic_atlas_plan_complete,
            deferred_judgment_unit_count,
            rendered_judgment_unit_count,
            deferred_decorative_mask_unit_count,
            source_reference_count,
            located_source_reference_count,
            unresolved_source_reference_count,
            physical_region_count: self.source_ownership.physical_cells.len(),
            shared_physical_region_count: self.source_ownership.shared_physical_cell_count(),
            overlapping_physical_region_count: self
                .source_ownership
                .overlapping_physical_cell_count(),
            overlapping_physical_region_pair_count: self
                .source_ownership
                .overlapping_physical_cell_pair_count(),
            observed_source_usage_group_count: self
                .source_ownership
                .observed_source_usage_group_count(),
            semantic_bound_consumer_projection_count: projection_validation
                .semantic_bound_consumer_projection_count,
            matched_authored_glyph_sequence_unit_count: projection_validation
                .matched_authored_glyph_sequence_unit_count,
            authored_glyph_sequence_projection_coverage_complete: projection_validation
                .authored_glyph_sequence_projection_coverage_complete,
            residency_bound_consumer_projection_count: projection_validation
                .residency_bound_consumer_projection_count,
            resolved_clut_bound_consumer_projection_count: projection_validation
                .resolved_clut_bound_consumer_projection_count,
            unresolved_external_clut_projection_count: projection_validation
                .unresolved_external_clut_projection_count,
            validated_projection_hashed_span_count: projection_validation.hashed_span_count,
            validated_projection_runtime_address_count: projection_validation.runtime_address_count,
            validated_projection_pointer_alias_count: projection_validation.pointer_alias_count,
            consumer_projection_binding_complete: projection_validation.complete,
            indexed_region_hash_matched_reference_count,
            source_reference_without_indexed_region_count,
            runtime_observed_source_reference_count,
            static_observed_source_reference_count,
            unresolved_source_usage_reference_count,
            rendered_source_reference_count,
            all_source_references_have_hash_matched_indexed_regions,
            source_records_match,
            rendered_fixed_cells_are_unique_and_non_overlapping,
            rendered_fixed_changes_confined_to_owned_cells,
            rendered_judgment_cells_are_unique_and_non_overlapping,
            rendered_judgment_changes_confined_to_owned_cells,
            rendered_action_cell_writes_are_unique_and_non_overlapping,
            rendered_action_changes_confined_to_owned_cells,
            fixed_text_expected_writes_complete,
            judgment_stamp_expected_writes_complete,
            development_input_available: authored_unit_count > 0,
            static_patch_insertion_complete,
            release_candidate_input_eligible: false,
            units,
        })
    }

    fn in_place_action_entries(&self) -> Result<Vec<&model::PracticalResultEntry>> {
        let projected_ids = self
            .assets
            .action_consumer_occurrence_catalog
            .occurrences
            .iter()
            .filter_map(|occurrence| occurrence.semantic_entry_id.as_ref())
            .map(|id| id.as_str())
            .collect::<BTreeSet<_>>();
        let relocated_ids = self
            .assets
            .action_cell_target_catalog
            .targets
            .iter()
            .map(|target| target.semantic_entry_id.as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            relocated_ids.is_subset(&projected_ids),
            "relocated practical-result action is not projection-bound"
        );
        let in_place_ids = projected_ids
            .difference(&relocated_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        let entries = self
            .assets
            .entries
            .iter()
            .filter(|entry| in_place_ids.contains(entry.id.as_str()))
            .collect::<Vec<_>>();
        ensure!(
            !entries.is_empty()
                && entries.iter().all(|entry| {
                    entry.strategy == model::PracticalResultStrategy::GlyphSequence
                        && entry.font_role
                            == super::model::ModeDescendantFontRole::PracticalResultAction
                        && entry.development_status
                            == model::PracticalResultDevelopmentStatus::Authored
                        && entry.unresolved_source_references.is_empty()
                        && entry.source_references.iter().all(|reference| {
                            reference.source_usage_status
                                == model::PracticalResultSourceUsageStatus::RuntimeObserved
                        })
                })
                && entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<BTreeSet<_>>()
                    == in_place_ids,
            "runtime-observed practical-result action set is incomplete"
        );
        Ok(entries)
    }

    fn in_place_observed_label_entries(&self) -> Result<Vec<&model::PracticalResultEntry>> {
        let entries = self
            .assets
            .entries
            .iter()
            .filter(|entry| {
                entry.strategy == model::PracticalResultStrategy::GlyphSequence
                    && entry.font_role == super::model::ModeDescendantFontRole::PracticalResultLabel
                    && entry.development_status == model::PracticalResultDevelopmentStatus::Authored
            })
            .collect::<Vec<_>>();
        ensure!(
            !entries.is_empty()
                && entries.iter().all(|entry| {
                    entry.unresolved_source_references.is_empty()
                        && entry.source_references.iter().all(|reference| {
                            reference.source_usage_status
                                == model::PracticalResultSourceUsageStatus::RuntimeObserved
                        })
                }),
            "runtime-observed practical-result label set is incomplete"
        );
        Ok(entries)
    }
}

// The backdrop is rendered first, followed by these two foreground labels.
// They form one composed asset; all other independently owned claims still
// reach the strict overlap validator unchanged.
fn compose_result_backdrop_claims(claims: Vec<DecodedDataClaim>) -> Vec<DecodedDataClaim> {
    let targets = [
        "siken20_member01_mixed_result_backdrop",
        "target__exam_result_announcement__siken20_member01_75800",
        "target__exam_records_back_hint_text__siken20_member01_75800",
    ];
    let mut independent = Vec::new();
    let mut layered = Vec::new();
    for claim in claims {
        if targets.iter().any(|target| {
            claim
                .id
                .starts_with(&format!("mode-descendant:practical-result:{target}:part-"))
        }) {
            layered.push([claim.range.start, claim.range.end]);
        } else {
            independent.push(claim);
        }
    }
    independent.extend(DecodedDataClaim::from_ranges(
        "mode-descendant:practical-result:composed-result-backdrop",
        "compose result backdrop with announcement and back hint",
        crate::write_scope::merge_byte_ranges(layered),
    ));
    independent
}

#[cfg(test)]
mod backdrop_claim_tests {
    use super::*;

    #[test]
    fn layered_claims_merge_without_absorbing_independent_owners() {
        let make = |id: &str, range| DecodedDataClaim {
            id: format!("mode-descendant:practical-result:{id}:part-0"),
            purpose: id.to_string(),
            range,
        };
        let independent = make("paper-label", 15..25);
        let result = compose_result_backdrop_claims(vec![
            make("siken20_member01_mixed_result_backdrop", 10..20),
            make(
                "target__exam_result_announcement__siken20_member01_75800",
                12..18,
            ),
            make(
                "target__exam_records_back_hint_text__siken20_member01_75800",
                40..50,
            ),
            independent.clone(),
        ]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], independent);
        assert_eq!(result[1].range, 10..20);
        assert_eq!(result[2].range, 40..50);
    }
}
