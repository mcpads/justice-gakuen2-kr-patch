use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::dialogue_consumer_union_model::{
    DialogueConsumerAssetCoverage, DialogueConsumerUnionAuditConfig,
    DialogueConsumerUnionAuditReport,
};
use super::selector_translation_model::DialogueSelectorTranslationScope;
use super::selector_translation_source::extract_selector_translation_source;
use super::translation_model::DialogueTranslationScope;
use super::translation_workspace_io::json_bytes;
use super::translation_workspace_source::extract_translation_workspace_source;

const SOURCE_ASSET_COUNT: usize = 78;
const STORED_COORDINATE_COUNT: usize = 62_639;

pub fn audit_dialogue_consumer_union(
    config: &DialogueConsumerUnionAuditConfig,
) -> Result<DialogueConsumerUnionAuditReport> {
    let primary = extract_translation_workspace_source(
        &config.cue,
        &config.codebook,
        DialogueTranslationScope::ResolvedPrimary,
    )?;
    let selector = extract_selector_translation_source(
        &config.cue,
        &config.codebook,
        DialogueSelectorTranslationScope::AllRuntimeImages,
    )?;
    ensure!(
        primary.source_bin_sha256 == selector.source_bin_sha256
            && primary.codebook_sha256 == selector.codebook_sha256
            && primary.source_asset_count == selector.source_asset_count,
        "primary and selector source identities differ"
    );

    let primary_semantic_groups = primary
        .groups_by_owner
        .values()
        .flatten()
        .map(|group| group.semantic_source_sha256.clone())
        .collect::<BTreeSet<_>>();
    let selector_semantic_groups = selector
        .groups_by_selector
        .values()
        .flatten()
        .map(|group| group.semantic_source_sha256.clone())
        .collect::<BTreeSet<_>>();
    let selector_coordinates = selector
        .contexts_by_selector
        .values()
        .flatten()
        .map(|context| context.coordinate_id.clone())
        .collect::<BTreeSet<_>>();
    let counts = consumer_union_counts(
        &primary_semantic_groups,
        &selector_semantic_groups,
        &primary.referenced_coordinates,
        &selector_coordinates,
    );
    let stored_coordinates_by_asset = primary
        .corpus
        .assets
        .iter()
        .map(|asset| {
            (
                asset.source_path.clone(),
                asset
                    .banks
                    .iter()
                    .flat_map(|bank| bank.entries.iter())
                    .map(|entry| entry.coordinate_id.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let assets = summarize_consumer_asset_coverage(
        &stored_coordinates_by_asset,
        &primary.referenced_coordinates,
        &selector_coordinates,
    )?;
    ensure!(
        selector.source_asset_count == SOURCE_ASSET_COUNT
            && counts.primary_semantic_group_count == 18_221
            && counts.selector_semantic_group_count == 550
            && counts.overlapping_semantic_group_count == 0
            && counts.consumer_union_semantic_group_count == 18_771
            && counts.primary_coordinate_count == 40_026
            && counts.selector_coordinate_count == 22_500
            && counts.overlapping_coordinate_count == 0
            && counts.consumer_union_coordinate_count == 62_526,
        "primary and selector consumer union denominator changed: {counts:?}"
    );
    let report = DialogueConsumerUnionAuditReport {
        kind: "Justice Gakuen 2 dialogue consumer union audit".to_string(),
        source_bin_sha256: primary.source_bin_sha256,
        codebook_sha256: primary.codebook_sha256,
        source_asset_count: selector.source_asset_count,
        stored_coordinate_count: STORED_COORDINATE_COUNT,
        primary_semantic_group_count: counts.primary_semantic_group_count,
        selector_semantic_group_count: counts.selector_semantic_group_count,
        overlapping_semantic_group_count: counts.overlapping_semantic_group_count,
        consumer_union_semantic_group_count: counts.consumer_union_semantic_group_count,
        primary_coordinate_count: counts.primary_coordinate_count,
        selector_coordinate_count: counts.selector_coordinate_count,
        overlapping_coordinate_count: counts.overlapping_coordinate_count,
        consumer_union_coordinate_count: counts.consumer_union_coordinate_count,
        stored_coordinate_outside_consumer_union_count: STORED_COORDINATE_COUNT
            - counts.consumer_union_coordinate_count,
        assets,
        primary_and_selector_population_complete: true,
        complete_game_text_consumer_inventory: false,
    };
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&config.output, json_bytes(&report)?)
        .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

pub(super) fn summarize_consumer_asset_coverage(
    stored_coordinates_by_asset: &BTreeMap<String, BTreeSet<String>>,
    primary_coordinates: &BTreeSet<String>,
    selector_coordinates: &BTreeSet<String>,
) -> Result<Vec<DialogueConsumerAssetCoverage>> {
    let stored_coordinates = stored_coordinates_by_asset
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        primary_coordinates.is_subset(&stored_coordinates),
        "primary consumer contains a coordinate outside the source corpus"
    );
    ensure!(
        selector_coordinates.is_subset(&stored_coordinates),
        "selector consumer contains a coordinate outside the source corpus"
    );

    Ok(stored_coordinates_by_asset
        .iter()
        .map(|(source_path, stored)| {
            let primary_coordinate_count = stored.intersection(primary_coordinates).count();
            let selector_coordinate_count = stored.intersection(selector_coordinates).count();
            let consumer_union_coordinate_count = stored
                .iter()
                .filter(|coordinate| {
                    primary_coordinates.contains(*coordinate)
                        || selector_coordinates.contains(*coordinate)
                })
                .count();
            DialogueConsumerAssetCoverage {
                source_path: source_path.clone(),
                stored_coordinate_count: stored.len(),
                primary_coordinate_count,
                selector_coordinate_count,
                consumer_union_coordinate_count,
                stored_coordinate_outside_consumer_union_count: stored.len()
                    - consumer_union_coordinate_count,
            }
        })
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DialogueConsumerUnionCounts {
    pub(super) primary_semantic_group_count: usize,
    pub(super) selector_semantic_group_count: usize,
    pub(super) overlapping_semantic_group_count: usize,
    pub(super) consumer_union_semantic_group_count: usize,
    pub(super) primary_coordinate_count: usize,
    pub(super) selector_coordinate_count: usize,
    pub(super) overlapping_coordinate_count: usize,
    pub(super) consumer_union_coordinate_count: usize,
}

pub(super) fn consumer_union_counts(
    primary_semantic_groups: &BTreeSet<String>,
    selector_semantic_groups: &BTreeSet<String>,
    primary_coordinates: &BTreeSet<String>,
    selector_coordinates: &BTreeSet<String>,
) -> DialogueConsumerUnionCounts {
    let overlapping_semantic_group_count = primary_semantic_groups
        .intersection(selector_semantic_groups)
        .count();
    let overlapping_coordinate_count = primary_coordinates
        .intersection(selector_coordinates)
        .count();
    DialogueConsumerUnionCounts {
        primary_semantic_group_count: primary_semantic_groups.len(),
        selector_semantic_group_count: selector_semantic_groups.len(),
        overlapping_semantic_group_count,
        consumer_union_semantic_group_count: primary_semantic_groups.len()
            + selector_semantic_groups.len()
            - overlapping_semantic_group_count,
        primary_coordinate_count: primary_coordinates.len(),
        selector_coordinate_count: selector_coordinates.len(),
        overlapping_coordinate_count,
        consumer_union_coordinate_count: primary_coordinates.len() + selector_coordinates.len()
            - overlapping_coordinate_count,
    }
}
