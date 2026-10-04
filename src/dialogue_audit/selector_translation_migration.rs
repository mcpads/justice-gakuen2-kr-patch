use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::selector_translation_model::{
    DialogueSelectorTranslationDecision, DialogueSelectorTranslationRefreshConfig,
    DialogueSelectorTranslationReviewShard, DialogueSelectorTranslationRoleManifest,
    DialogueSelectorTranslationRoleRef, DialogueSelectorTranslationSelectorManifest,
    DialogueSelectorTranslationShard, DialogueSelectorTranslationShardRef,
    DialogueSelectorTranslationSourceGroup, DialogueSelectorTranslationSourceShard,
    DialogueSelectorTranslationWorkspaceManifest,
};
use super::selector_translation_validation::{
    SelectorTranslationAuditCounts, validate_selector_review_entries,
    validate_selector_translation_entries,
};
use super::selector_translation_workspace::initialize_dialogue_selector_translation;
use super::translation_workspace_io::{read_bounded_json, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationReviewDecision, DialogueTranslationWorkspaceRole,
};

#[derive(Clone)]
pub(super) struct PreviousSelectorGroup {
    pub(super) source_segments: Vec<String>,
    pub(super) controls: Vec<super::translation_model::DialogueTranslationControl>,
    pub(super) development_resolution:
        super::selector_translation_model::DialogueSelectorDevelopmentResolution,
    pub(super) translation: DialogueSelectorTranslationDecision,
    pub(super) review: DialogueTranslationReviewDecision,
}

#[derive(Debug)]
struct GroupUnitRefs {
    source: DialogueSelectorTranslationShardRef,
    translation: DialogueSelectorTranslationShardRef,
    review: DialogueSelectorTranslationShardRef,
}

pub fn refresh_dialogue_selector_translation(
    config: &DialogueSelectorTranslationRefreshConfig,
) -> Result<DialogueSelectorTranslationWorkspaceManifest> {
    ensure!(
        config.previous != config.output,
        "selector refresh output must differ from the previous workspace"
    );
    let previous = load_previous_groups(&config.previous)?;
    let manifest = initialize_dialogue_selector_translation(
        &super::selector_translation_model::DialogueSelectorTranslationInitConfig {
            cue: config.cue.clone(),
            codebook: config.codebook.clone(),
            output: config.output.clone(),
            force: config.force,
            scope: config.scope,
        },
    )?;
    migrate_compatible_groups(&config.output, &previous)?;
    Ok(manifest)
}

fn load_previous_groups(root: &Path) -> Result<BTreeMap<String, PreviousSelectorGroup>> {
    let (manifest, units) = registered_group_units(root)?;
    let mut previous = BTreeMap::new();
    for unit in units {
        let (source, source_bytes): (DialogueSelectorTranslationSourceShard, _) =
            read_bounded_json(root, Path::new(&unit.source.path))?;
        let source_sha256 = sha256_bytes(&source_bytes);
        ensure!(
            unit.source.content_sha256.as_deref() == Some(source_sha256.as_str()),
            "previous selector protected source hash changed at {}",
            unit.source.path
        );
        let (translation, _): (DialogueSelectorTranslationShard, _) =
            read_bounded_json(root, Path::new(&unit.translation.path))?;
        ensure!(
            unit.translation.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && translation.source_shard_sha256 == source_sha256,
            "previous selector translation source binding changed at {}",
            unit.translation.path
        );
        let (review, _): (DialogueSelectorTranslationReviewShard, _) =
            read_bounded_json(root, Path::new(&unit.review.path))?;
        ensure!(
            unit.review.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && review.source_shard_sha256 == source_sha256,
            "previous selector review source binding changed at {}",
            unit.review.path
        );
        let mut counts = SelectorTranslationAuditCounts::default();
        validate_selector_translation_entries(&translation.entries, &source.entries, &mut counts)?;
        validate_selector_review_entries(
            &review,
            &unit.translation.path,
            &source.entries,
            &translation.entries,
            &mut counts,
        )?;
        ensure!(
            source.entries.len() == translation.entries.len()
                && source.entries.len() == review.entries.len(),
            "previous selector group unit shape changed"
        );
        for ((group, decision), review_decision) in source
            .entries
            .iter()
            .zip(translation.entries)
            .zip(review.entries)
        {
            ensure!(
                group.semantic_source_sha256 == decision.semantic_source_sha256
                    && group.semantic_source_sha256 == review_decision.semantic_source_sha256,
                "previous selector semantic identity changed"
            );
            let item = PreviousSelectorGroup {
                source_segments: group.source_segments.clone(),
                controls: group.controls.clone(),
                development_resolution: group.development_resolution,
                translation: decision,
                review: review_decision,
            };
            ensure!(
                previous
                    .insert(group.semantic_source_sha256.clone(), item)
                    .is_none(),
                "previous selector semantic group occurs more than once"
            );
        }
    }
    ensure!(
        previous.len() == manifest.semantic_group_count,
        "previous selector semantic group coverage changed"
    );
    Ok(previous)
}

fn migrate_compatible_groups(
    root: &Path,
    previous: &BTreeMap<String, PreviousSelectorGroup>,
) -> Result<()> {
    let (manifest, units) = registered_group_units(root)?;
    let mut migrated = BTreeSet::new();
    for unit in units {
        let (source, source_bytes): (DialogueSelectorTranslationSourceShard, _) =
            read_bounded_json(root, Path::new(&unit.source.path))?;
        let source_sha256 = sha256_bytes(&source_bytes);
        ensure!(
            unit.source.content_sha256.as_deref() == Some(source_sha256.as_str()),
            "refreshed selector protected source hash changed at {}",
            unit.source.path
        );
        let translation_path = PathBuf::from(&unit.translation.path);
        let review_path = PathBuf::from(&unit.review.path);
        let (mut translation, _): (DialogueSelectorTranslationShard, _) =
            read_bounded_json(root, &translation_path)?;
        let (mut review, _): (DialogueSelectorTranslationReviewShard, _) =
            read_bounded_json(root, &review_path)?;
        preserve_compatible_selector_groups(
            &source.entries,
            &mut translation.entries,
            &mut review.entries,
            previous,
            &mut migrated,
        )?;
        write_bounded_json(root, &translation_path, &translation)?;
        write_bounded_json(root, &review_path, &review)?;
    }
    ensure!(
        migrated.len() == previous.len() && migrated.len() <= manifest.semantic_group_count,
        "selector refresh semantic group population changed"
    );
    Ok(())
}

pub(super) fn preserve_compatible_selector_groups(
    source: &[DialogueSelectorTranslationSourceGroup],
    translations: &mut [DialogueSelectorTranslationDecision],
    reviews: &mut [DialogueTranslationReviewDecision],
    previous: &BTreeMap<String, PreviousSelectorGroup>,
    migrated: &mut BTreeSet<String>,
) -> Result<()> {
    ensure!(
        source.len() == translations.len() && source.len() == reviews.len(),
        "refreshed selector group unit shape changed"
    );
    for ((group, output_translation), output_review) in source.iter().zip(translations).zip(reviews)
    {
        let Some(old) = previous.get(&group.semantic_source_sha256) else {
            continue;
        };
        ensure!(
            old.source_segments == group.source_segments
                && old.controls == group.controls
                && old.development_resolution == group.development_resolution
                && old.translation.semantic_source_sha256 == group.semantic_source_sha256
                && old.review.semantic_source_sha256 == group.semantic_source_sha256,
            "refreshed selector source meaning changed for {}",
            group.semantic_source_sha256
        );
        *output_translation = old.translation.clone();
        *output_review = old.review.clone();
        ensure!(
            migrated.insert(group.semantic_source_sha256.clone()),
            "refreshed selector semantic group occurs more than once"
        );
    }
    Ok(())
}

fn registered_group_units(
    root: &Path,
) -> Result<(
    DialogueSelectorTranslationWorkspaceManifest,
    Vec<GroupUnitRefs>,
)> {
    let (manifest, _): (DialogueSelectorTranslationWorkspaceManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    ensure!(
        manifest.kind == "Justice Gakuen 2 sharded Korean selector translation workspace"
            && manifest.selector_count == manifest.selectors.len(),
        "selector workspace manifest shape changed"
    );
    let mut units = Vec::new();
    let mut registered_paths = BTreeSet::from([PathBuf::from("manifest.json")]);
    for selector_ref in &manifest.selectors {
        let selector_path = PathBuf::from(&selector_ref.manifest_path);
        ensure!(
            registered_paths.insert(selector_path.clone()),
            "selector manifest path is duplicated"
        );
        let (selector, bytes): (DialogueSelectorTranslationSelectorManifest, _) =
            read_bounded_json(root, &selector_path)?;
        ensure!(
            sha256_bytes(&bytes) == selector_ref.manifest_sha256
                && selector.canonical_selector == selector_ref.canonical_selector,
            "selector manifest binding changed at {}",
            selector_path.display()
        );
        let source = load_role_shards(
            root,
            selector.canonical_selector,
            &selector.roles,
            DialogueTranslationWorkspaceRole::Source,
            &mut registered_paths,
        )?;
        let translation = load_role_shards(
            root,
            selector.canonical_selector,
            &selector.roles,
            DialogueTranslationWorkspaceRole::Translation,
            &mut registered_paths,
        )?;
        let review = load_role_shards(
            root,
            selector.canonical_selector,
            &selector.roles,
            DialogueTranslationWorkspaceRole::Review,
            &mut registered_paths,
        )?;
        ensure!(
            source.len() == translation.len() && source.len() == review.len(),
            "selector group role shard counts changed"
        );
        units.extend(source.into_iter().zip(translation).zip(review).map(
            |((source, translation), review)| GroupUnitRefs {
                source,
                translation,
                review,
            },
        ));
    }
    Ok((manifest, units))
}

fn load_role_shards(
    root: &Path,
    canonical_selector: usize,
    roles: &[DialogueSelectorTranslationRoleRef],
    target_role: DialogueTranslationWorkspaceRole,
    registered_paths: &mut BTreeSet<PathBuf>,
) -> Result<Vec<DialogueSelectorTranslationShardRef>> {
    let role = roles
        .iter()
        .find(|role| role.role == target_role)
        .with_context(|| format!("selector {canonical_selector} lacks {target_role:?} role"))?;
    let mut shards = Vec::new();
    for manifest_ref in &role.manifests {
        let path = PathBuf::from(&manifest_ref.manifest_path);
        ensure!(
            registered_paths.insert(path.clone()),
            "selector role manifest path is duplicated"
        );
        let (role_manifest, bytes): (DialogueSelectorTranslationRoleManifest, _) =
            read_bounded_json(root, &path)?;
        ensure!(
            sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                && role_manifest.canonical_selector == canonical_selector
                && role_manifest.role == target_role
                && role_manifest.shard_count == role_manifest.shards.len()
                && role_manifest.shard_count == manifest_ref.shard_count
                && role_manifest.entry_count == manifest_ref.entry_count,
            "selector role manifest binding changed at {}",
            path.display()
        );
        shards.extend(role_manifest.shards);
    }
    ensure!(
        role.manifest_count == role.manifests.len()
            && role.shard_count == shards.len()
            && role.entry_count == shards.iter().map(|shard| shard.entry_count).sum::<usize>(),
        "selector role aggregate changed"
    );
    Ok(shards)
}
