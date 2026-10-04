use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::translation_workspace_model::{DialogueTranslationDecision, DialogueTranslationShard};
use super::translation_workspace_validation::MAX_SHARD_BYTES;

#[derive(Debug, Clone)]
pub struct DialogueTranslationDecisionHashConfig {
    pub input: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueTranslationDecisionHashReport {
    pub kind: String,
    pub translation_path: String,
    pub entries: Vec<DialogueTranslationDecisionHash>,
}

#[derive(Debug, Serialize)]
pub struct DialogueTranslationDecisionHash {
    pub semantic_source_sha256: String,
    pub reviewed_translation_sha256: String,
}

#[derive(Serialize)]
struct ReviewableTranslation<'a> {
    semantic_source_sha256: &'a str,
    korean_segments: &'a [Option<String>],
}

pub fn hash_dialogue_translation_decisions(
    config: &DialogueTranslationDecisionHashConfig,
) -> Result<DialogueTranslationDecisionHashReport> {
    let bytes = std::fs::read(&config.input)
        .with_context(|| format!("failed to read {}", config.input.display()))?;
    ensure!(
        bytes.len() <= MAX_SHARD_BYTES,
        "translation shard exceeds {MAX_SHARD_BYTES} bytes"
    );
    let shard: DialogueTranslationShard = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", config.input.display()))?;
    ensure!(
        shard.kind == "Justice Gakuen 2 Korean dialogue translation shard",
        "input is not a Korean dialogue translation shard"
    );

    let entries = shard
        .entries
        .iter()
        .map(|decision| {
            Ok(DialogueTranslationDecisionHash {
                semantic_source_sha256: decision.semantic_source_sha256.clone(),
                reviewed_translation_sha256: translation_decision_sha256(decision)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(DialogueTranslationDecisionHashReport {
        kind: "Justice Gakuen 2 reviewable translation decision hashes".to_string(),
        translation_path: config.input.display().to_string(),
        entries,
    })
}

pub(super) fn translation_decision_sha256(
    decision: &DialogueTranslationDecision,
) -> Result<String> {
    let payload = ReviewableTranslation {
        semantic_source_sha256: &decision.semantic_source_sha256,
        korean_segments: &decision.korean_segments,
    };
    Ok(sha256_bytes(&serde_json::to_vec(&payload)?))
}
