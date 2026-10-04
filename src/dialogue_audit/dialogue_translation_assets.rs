use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::dialogue_translation_assets_model::{
    DialogueTranslationAssetAuditConfig, DialogueTranslationAssetFamily,
    DialogueTranslationAssetManifest, DialogueTranslationAssetReport,
    DialogueTranslationAssetSyncConfig, TrackedPrimaryDialogueTranslationShard,
    TrackedSelectorDialogueTranslationShard,
};
use super::translation_workspace_model::DialogueTranslationDecisionStatus;

#[path = "dialogue_translation_asset_audit.rs"]
mod audit;
#[path = "dialogue_translation_asset_loading.rs"]
mod loading;
#[cfg(test)]
#[path = "dialogue_translation_assets_tests.rs"]
mod tests;
#[path = "dialogue_translation_asset_validation.rs"]
mod validation;
#[path = "dialogue_translation_asset_writer.rs"]
mod writer;

#[derive(Debug)]
pub(super) struct ValidatedDialogueTranslationAssets {
    pub(super) manifest: DialogueTranslationAssetManifest,
    pub(super) report: DialogueTranslationAssetReport,
    pub(super) primary_shards: Vec<ValidatedDialogueTranslationShardFile>,
    pub(super) selector_shards: Vec<ValidatedDialogueTranslationShardFile>,
}

#[derive(Debug)]
pub(super) struct ValidatedDialogueTranslationShardFile {
    pub(super) path: PathBuf,
    pub(super) content_sha256: String,
    pub(super) json_byte_count: usize,
}

pub fn sync_dialogue_translation_assets(
    config: &DialogueTranslationAssetSyncConfig,
) -> Result<DialogueTranslationAssetReport> {
    let mut primary = loading::load_primary_translation_assets(&config.primary_workspace)?;
    let mut selector = loading::load_selector_translation_assets(&config.selector_workspace)?;
    ensure!(
        primary.source_bin_sha256 == selector.source_bin_sha256
            && primary.codebook_sha256 == selector.codebook_sha256
            && primary.source_corpus_sha256 == selector.source_corpus_sha256,
        "primary and selector translation inputs do not share one source identity"
    );
    ensure_unique_semantic_decisions(&primary, &selector)?;
    if config.output.exists() {
        loading::preserve_tracked_translation_decisions(
            &config.output,
            &mut primary,
            &mut selector,
        )?;
    }

    let temporary_output = temporary_output_path(&config.output)?;
    if temporary_output.exists() {
        std::fs::remove_dir_all(&temporary_output).with_context(|| {
            format!(
                "failed to remove stale temporary output {}",
                temporary_output.display()
            )
        })?;
    }
    std::fs::create_dir_all(&temporary_output).with_context(|| {
        format!(
            "failed to create temporary output {}",
            temporary_output.display()
        )
    })?;

    let result = (|| {
        writer::write_translation_assets(&temporary_output, &primary, &selector)?;
        audit_dialogue_translation_assets(&DialogueTranslationAssetAuditConfig {
            input: temporary_output.clone(),
        })
    })();
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temporary_output);
            return Err(error);
        }
    };

    ensure!(
        config.force || !config.output.exists(),
        "refusing to replace tracked dialogue translation assets {}; pass --force after reviewing the current assets",
        config.output.display()
    );
    if config.output.exists() {
        if config.output.is_dir() {
            std::fs::remove_dir_all(&config.output)
                .with_context(|| format!("failed to remove {}", config.output.display()))?;
        } else {
            std::fs::remove_file(&config.output)
                .with_context(|| format!("failed to remove {}", config.output.display()))?;
        }
    }
    std::fs::rename(&temporary_output, &config.output).with_context(|| {
        format!(
            "failed to install tracked dialogue translation assets at {}",
            config.output.display()
        )
    })?;
    Ok(report)
}

pub fn audit_dialogue_translation_assets(
    config: &DialogueTranslationAssetAuditConfig,
) -> Result<DialogueTranslationAssetReport> {
    Ok(load_validated_dialogue_translation_assets(&config.input)?.report)
}

pub(super) fn load_validated_dialogue_translation_assets(
    root: &Path,
) -> Result<ValidatedDialogueTranslationAssets> {
    audit::load_validated_translation_assets(root)
}

fn ensure_unique_semantic_decisions(primary: &LoadedFamily, selector: &LoadedFamily) -> Result<()> {
    let mut semantic_ids = BTreeSet::new();
    let mut shard_ids = BTreeSet::new();
    for family in [primary, selector] {
        for owner in &family.owners {
            for shard in &owner.shards {
                ensure!(
                    shard_ids.insert(shard.shard_id().to_string()),
                    "tracked translation shard ID is duplicated: {}",
                    shard.shard_id()
                );
                for semantic_id in shard.semantic_ids() {
                    ensure!(
                        semantic_ids.insert(semantic_id.to_string()),
                        "semantic translation decision is duplicated across tracked assets: {semantic_id}"
                    );
                }
            }
        }
    }
    Ok(())
}

fn temporary_output_path(output: &Path) -> Result<PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .context("tracked dialogue translation asset output has no file name")?;
    Ok(parent.join(format!(".{name}.sync-{}", std::process::id())))
}

#[derive(Debug)]
struct LoadedFamily {
    family: DialogueTranslationAssetFamily,
    source_bin_sha256: String,
    codebook_sha256: String,
    source_corpus_sha256: String,
    source_binding_sha256: String,
    target_scope: String,
    source_asset_count: usize,
    owners: Vec<LoadedOwner>,
    counts: DecisionCounts,
}

#[derive(Debug)]
struct LoadedOwner {
    owner_id: String,
    source_path: Option<String>,
    canonical_selector: Option<usize>,
    shards: Vec<LoadedShard>,
}

#[derive(Debug)]
enum LoadedShard {
    Primary {
        path: PathBuf,
        shard: TrackedPrimaryDialogueTranslationShard,
        counts: DecisionCounts,
    },
    Selector {
        path: PathBuf,
        shard: TrackedSelectorDialogueTranslationShard,
        counts: DecisionCounts,
    },
}

impl LoadedShard {
    fn path(&self) -> &Path {
        match self {
            Self::Primary { path, .. } | Self::Selector { path, .. } => path,
        }
    }

    fn shard_id(&self) -> &str {
        match self {
            Self::Primary { shard, .. } => &shard.shard_id,
            Self::Selector { shard, .. } => &shard.shard_id,
        }
    }

    fn source_shard_sha256(&self) -> &str {
        match self {
            Self::Primary { shard, .. } => &shard.source_shard_sha256,
            Self::Selector { shard, .. } => &shard.source_shard_sha256,
        }
    }

    fn counts(&self) -> DecisionCounts {
        match self {
            Self::Primary { counts, .. } | Self::Selector { counts, .. } => *counts,
        }
    }

    fn semantic_ids(&self) -> impl Iterator<Item = &str> {
        let values: Vec<_> = match self {
            Self::Primary { shard, .. } => shard
                .entries
                .iter()
                .map(|entry| entry.semantic_source_sha256.as_str())
                .collect(),
            Self::Selector { shard, .. } => shard
                .entries
                .iter()
                .map(|entry| entry.semantic_source_sha256.as_str())
                .collect(),
        };
        values.into_iter()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct DecisionCounts {
    untranslated: usize,
    draft: usize,
    ready_for_review: usize,
}

impl DecisionCounts {
    fn observe(&mut self, status: DialogueTranslationDecisionStatus) {
        match status {
            DialogueTranslationDecisionStatus::Untranslated => self.untranslated += 1,
            DialogueTranslationDecisionStatus::Draft => self.draft += 1,
            DialogueTranslationDecisionStatus::ReadyForReview => self.ready_for_review += 1,
        }
    }

    fn decision_count(self) -> usize {
        self.untranslated + self.draft + self.ready_for_review
    }

    fn merge(&mut self, other: Self) {
        self.untranslated += other.untranslated;
        self.draft += other.draft;
        self.ready_for_review += other.ready_for_review;
    }
}
