use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::pipeline::sha256_bytes;

use super::codebook::validate_codebook;
use super::codebook_model::{
    DialogueCodebook, DialogueCodebookEntry, DialogueCodebookStatus, DialogueGlyphMeaning,
};

const REVIEW_KIND: &str = "Dialogue glyph source review decision shard";
const CONTEXT_KIND: &str = "Dialogue runtime-image glyph review context shard";

#[derive(Debug, Clone)]
pub struct DialogueCodebookReviewApplicationConfig {
    pub codebook: PathBuf,
    pub review: PathBuf,
    pub context_shard: PathBuf,
    pub review_png: PathBuf,
    pub output: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueGlyphReviewDecisionShard {
    kind: String,
    source_bin_sha256: String,
    codebook_before_sha256: String,
    context_shard_sha256: String,
    review_png_sha256: String,
    batch_index: usize,
    page_index: usize,
    evidence_id: String,
    evidence_description: String,
    reviewed_at: String,
    entries: Vec<DialogueGlyphReviewDecision>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueGlyphReviewDecision {
    slot_index: usize,
    pixel_sha256: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueCodebookReviewContextShard {
    kind: String,
    page_index: usize,
    slot_count: usize,
    slots: Vec<DialogueCodebookReviewContextSlot>,
}

#[derive(Debug, Deserialize)]
struct DialogueCodebookReviewContextSlot {
    slot_index: usize,
    pixel_sha256: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DialogueCodebookReviewApplicationReport {
    pub kind: String,
    pub batch_index: usize,
    pub page_index: usize,
    pub codebook_before_sha256: String,
    pub codebook_after_sha256: String,
    pub applied_entry_count: usize,
    pub codebook_entry_count: usize,
    pub output: String,
}

pub fn apply_dialogue_codebook_review(
    config: &DialogueCodebookReviewApplicationConfig,
) -> Result<DialogueCodebookReviewApplicationReport> {
    ensure!(
        config.force || !config.output.exists(),
        "refusing to replace {} without --force",
        config.output.display()
    );
    let codebook_bytes = std::fs::read(&config.codebook)
        .with_context(|| format!("failed to read {}", config.codebook.display()))?;
    let review_bytes = std::fs::read(&config.review)
        .with_context(|| format!("failed to read {}", config.review.display()))?;
    let context_bytes = std::fs::read(&config.context_shard)
        .with_context(|| format!("failed to read {}", config.context_shard.display()))?;
    let png_bytes = std::fs::read(&config.review_png)
        .with_context(|| format!("failed to read {}", config.review_png.display()))?;

    let (output_bytes, mut report) =
        apply_review_bytes(&codebook_bytes, &review_bytes, &context_bytes, &png_bytes)?;
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary_output = config.output.with_extension("json.tmp");
    ensure!(
        !temporary_output.exists(),
        "temporary output already exists: {}",
        temporary_output.display()
    );
    std::fs::write(&temporary_output, output_bytes)
        .with_context(|| format!("failed to write {}", temporary_output.display()))?;
    std::fs::rename(&temporary_output, &config.output).with_context(|| {
        format!(
            "failed to replace {} with {}",
            config.output.display(),
            temporary_output.display()
        )
    })?;
    report.output = config.output.display().to_string();
    Ok(report)
}

fn apply_review_bytes(
    codebook_bytes: &[u8],
    review_bytes: &[u8],
    context_bytes: &[u8],
    png_bytes: &[u8],
) -> Result<(Vec<u8>, DialogueCodebookReviewApplicationReport)> {
    let mut codebook: DialogueCodebook = serde_json::from_slice(codebook_bytes)?;
    let review: DialogueGlyphReviewDecisionShard = serde_json::from_slice(review_bytes)?;
    let context: DialogueCodebookReviewContextShard = serde_json::from_slice(context_bytes)?;

    validate_codebook(&codebook, &codebook.source_bin_sha256)?;

    ensure!(
        review.kind == REVIEW_KIND,
        "unexpected review decision kind"
    );
    ensure!(
        context.kind == CONTEXT_KIND,
        "unexpected review context kind"
    );
    ensure!(
        review.source_bin_sha256 == codebook.source_bin_sha256,
        "review source BIN does not match the codebook"
    );
    ensure!(
        review.codebook_before_sha256 == sha256_bytes(codebook_bytes),
        "review was not made against the selected codebook bytes"
    );
    ensure!(
        review.context_shard_sha256 == sha256_bytes(context_bytes),
        "review context shard SHA-256 mismatch"
    );
    ensure!(
        review.review_png_sha256 == sha256_bytes(png_bytes),
        "review PNG SHA-256 mismatch"
    );
    ensure!(
        review.page_index == context.page_index,
        "review page does not match its context shard"
    );
    ensure!(
        review.batch_index != 0,
        "review batch index must be nonzero"
    );
    ensure!(
        context.slot_count == context.slots.len(),
        "review context slot_count mismatch"
    );
    ensure!(
        !review.evidence_id.trim().is_empty()
            && !review.evidence_description.trim().is_empty()
            && valid_review_date(&review.reviewed_at),
        "review evidence metadata must be nonempty and use a YYYY-MM-DD date"
    );
    ensure!(!review.entries.is_empty(), "review decision shard is empty");
    ensure!(
        !codebook.evidence_sources.contains_key(&review.evidence_id),
        "review evidence ID already exists in the codebook"
    );

    let context_slots = context
        .slots
        .into_iter()
        .map(|slot| (slot.slot_index, slot.pixel_sha256))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        context_slots.len() == context.slot_count,
        "review context contains duplicate slot indices"
    );
    let existing_hashes = codebook
        .entries
        .iter()
        .map(|entry| entry.pixel_sha256.as_str())
        .collect::<BTreeSet<_>>();
    let mut previous_slot = None;
    let mut reviewed_hashes = BTreeSet::new();
    for decision in &review.entries {
        if let Some(previous) = previous_slot {
            ensure!(
                previous < decision.slot_index,
                "review decisions must have unique, ascending slot indices"
            );
        }
        previous_slot = Some(decision.slot_index);
        ensure!(
            context_slots.get(&decision.slot_index) == Some(&decision.pixel_sha256),
            "review decision does not match context slot {}",
            decision.slot_index
        );
        ensure!(
            !existing_hashes.contains(decision.pixel_sha256.as_str()),
            "reviewed pixel {} already exists in the codebook",
            decision.pixel_sha256
        );
        ensure!(
            reviewed_hashes.insert(decision.pixel_sha256.as_str()),
            "review decision repeats a pixel hash"
        );
        ensure!(
            decision.text.chars().count() == 1
                && !decision.text.chars().next().is_some_and(char::is_control),
            "reviewed text must contain exactly one non-control scalar"
        );
    }

    codebook.evidence_sources.insert(
        review.evidence_id.clone(),
        review.evidence_description.clone(),
    );
    codebook
        .entries
        .extend(review.entries.iter().map(|decision| DialogueCodebookEntry {
            pixel_sha256: decision.pixel_sha256.clone(),
            meaning: DialogueGlyphMeaning::Character {
                text: decision.text.clone(),
            },
            status: DialogueCodebookStatus::SourcePixelVerified,
            evidence: vec![review.evidence_id.clone()],
            reviewed_by: None,
            reviewed_at: None,
        }));
    codebook
        .entries
        .sort_by(|left, right| left.pixel_sha256.cmp(&right.pixel_sha256));
    validate_codebook(&codebook, &codebook.source_bin_sha256)?;

    let mut output_bytes = serde_json::to_vec_pretty(&codebook)?;
    output_bytes.push(b'\n');
    let report = DialogueCodebookReviewApplicationReport {
        kind: "Dialogue codebook review application report".to_string(),
        batch_index: review.batch_index,
        page_index: review.page_index,
        codebook_before_sha256: sha256_bytes(codebook_bytes),
        codebook_after_sha256: sha256_bytes(&output_bytes),
        applied_entry_count: review.entries.len(),
        codebook_entry_count: codebook.entries.len(),
        output: String::new(),
    };
    Ok((output_bytes, report))
}

fn valid_review_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

#[cfg(test)]
#[path = "codebook_review_application_tests.rs"]
mod tests;
