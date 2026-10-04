use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::selector_translation_model::{
    DialogueSelectorConsumerEvidence, DialogueSelectorDevelopmentResolution,
    DialogueSelectorTranslationInitConfig, DialogueSelectorTranslationSelectorManifest,
    DialogueSelectorTranslationSelectorRef, DialogueSelectorTranslationWorkspaceManifest,
};
use super::selector_translation_source::extract_selector_translation_source;
use super::selector_translation_writer::write_selector_units;
use super::translation_workspace::prepare_workspace_root;
use super::translation_workspace_io::{relative_path, write_bounded_json};
use super::translation_workspace_validation::{MAX_SHARD_BYTES, MAX_SHARD_ENTRIES};
use super::translation_workspace_writer::path_string;

pub fn initialize_dialogue_selector_translation(
    config: &DialogueSelectorTranslationInitConfig,
) -> Result<DialogueSelectorTranslationWorkspaceManifest> {
    prepare_workspace_root(&config.output, config.force)?;
    let source = extract_selector_translation_source(&config.cue, &config.codebook, config.scope)?;
    let mut selectors = Vec::new();
    for (&canonical_selector, groups) in &source.groups_by_selector {
        let contexts = source
            .contexts_by_selector
            .get(&canonical_selector)
            .context("selector translation context owner disappeared")?;
        let refs = write_selector_units(
            &config.output,
            &source,
            canonical_selector,
            groups,
            contexts,
        )?;
        let selector_manifest = DialogueSelectorTranslationSelectorManifest {
            kind: "Justice Gakuen 2 selector translation manifest".to_string(),
            canonical_selector,
            semantic_group_count: groups.len(),
            coordinate_count: contexts.len(),
            roles: vec![refs.source, refs.context, refs.translation, refs.review],
        };
        let path = relative_path(format!("manifests/selector-{canonical_selector}.json"))?;
        let (manifest_sha256, _) = write_bounded_json(&config.output, &path, &selector_manifest)?;
        selectors.push(DialogueSelectorTranslationSelectorRef {
            canonical_selector,
            manifest_path: path_string(&path),
            manifest_sha256,
            semantic_group_count: groups.len(),
            coordinate_count: contexts.len(),
        });
    }

    let groups = source
        .groups_by_selector
        .values()
        .flatten()
        .collect::<Vec<_>>();
    let count_evidence = |evidence| {
        groups
            .iter()
            .filter(|group| group.consumer_evidence == evidence)
            .count()
    };
    let runtime_insertion_rewrite_group_count = groups
        .iter()
        .filter(|group| {
            group.development_resolution
                == DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
        })
        .count();
    let manifest = DialogueSelectorTranslationWorkspaceManifest {
        kind: "Justice Gakuen 2 sharded Korean selector translation workspace".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        codebook_sha256: source.codebook_sha256,
        source_corpus_sha256: source.source_corpus_sha256,
        selector_consumer_audit_sha256: source.selector_consumer_audit_sha256,
        target_scope: source.scope.target_scope().to_string(),
        source_asset_count: source.source_asset_count,
        selector_count: selectors.len(),
        semantic_group_count: groups.len(),
        coordinate_count: source.contexts_by_selector.values().map(Vec::len).sum(),
        authored_translation_target_group_count: groups.len()
            - runtime_insertion_rewrite_group_count,
        runtime_insertion_rewrite_group_count,
        direct_entry_load_group_count: count_evidence(
            DialogueSelectorConsumerEvidence::DirectEntryLoad,
        ),
        direct_selector_load_group_count: count_evidence(
            DialogueSelectorConsumerEvidence::DirectSelectorLoad,
        ),
        consumer_pending_group_count: count_evidence(
            DialogueSelectorConsumerEvidence::ConsumerPending,
        ),
        max_entries_per_shard: MAX_SHARD_ENTRIES,
        max_bytes_per_shard: MAX_SHARD_BYTES,
        selectors,
    };
    let expected = match config.scope {
        super::selector_translation_model::DialogueSelectorTranslationScope::MgkDevelopment => {
            (10, 496, 3_070, 492, 431)
        }
        super::selector_translation_model::DialogueSelectorTranslationScope::AllRuntimeImages => {
            (78, 550, 22_500, 546, 485)
        }
    };
    ensure!(
        (
            manifest.source_asset_count,
            manifest.semantic_group_count,
            manifest.coordinate_count,
            manifest.authored_translation_target_group_count,
            manifest.direct_selector_load_group_count,
        ) == expected
            && manifest.selector_count == 5
            && manifest.runtime_insertion_rewrite_group_count == 4
            && manifest.direct_entry_load_group_count == 1
            && manifest.consumer_pending_group_count == 60,
        "selector translation workspace denominator changed for {}",
        config.scope.target_scope(),
    );
    write_bounded_json(&config.output, Path::new("manifest.json"), &manifest)?;
    Ok(manifest)
}
