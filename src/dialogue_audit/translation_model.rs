use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DialogueTranslationScope {
    MgkDevelopment,
    ResolvedPrimary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DialogueDevelopmentInputPolicy {
    CompleteScope,
    AuthoredSubset,
}

impl DialogueDevelopmentInputPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::CompleteScope => "complete_scope",
            Self::AuthoredSubset => "authored_subset",
        }
    }

    pub fn requires_complete_scope(self) -> bool {
        self == Self::CompleteScope
    }
}

impl DialogueTranslationScope {
    pub fn target_scope(self) -> &'static str {
        match self {
            Self::MgkDevelopment => "mgk_primary_execution_referenced_dialogue",
            Self::ResolvedPrimary => "resolved_primary_execution_referenced_dialogue",
        }
    }

    pub fn from_target_scope(value: &str) -> Option<Self> {
        match value {
            "primary_execution_referenced_dialogue"
            | "mgk_primary_execution_referenced_dialogue" => Some(Self::MgkDevelopment),
            "resolved_primary_execution_referenced_dialogue" => Some(Self::ResolvedPrimary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DialogueTranslationInitConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub scope: DialogueTranslationScope,
}

#[derive(Debug, Clone)]
pub struct DialogueTranslationRefreshConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub previous: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub scope: DialogueTranslationScope,
}

#[derive(Debug, Clone)]
pub struct DialogueTranslationAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationInput {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub translation_status: DialogueTranslationProjectStatus,
    pub translation_review: Option<DialogueTranslationReview>,
    pub groups: Vec<DialogueTranslationGroup>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTranslationProjectStatus {
    InProgress,
    Complete,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationReview {
    pub decision: String,
    pub approved_by: String,
    pub approved_on: String,
    pub scope: String,
    pub basis: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationGroup {
    pub semantic_source_sha256: String,
    pub source_segments: Vec<String>,
    pub controls: Vec<DialogueTranslationControl>,
    pub coordinate_ids: Vec<String>,
    pub korean_segments: Vec<Option<String>>,
    pub status: DialogueTranslationStatus,
    pub translator: Option<String>,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationControl {
    pub semantic_name: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTranslationStatus {
    ProtectedOnly,
    Untranslated,
    Draft,
    NeedsReview,
    Approved,
}

#[derive(Debug, Serialize)]
pub struct DialogueTranslationAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub group_count: usize,
    pub coordinate_count: usize,
    pub protected_only_group_count: usize,
    pub untranslated_group_count: usize,
    pub draft_group_count: usize,
    pub needs_review_group_count: usize,
    pub approved_group_count: usize,
    pub translated_linguistic_group_count: usize,
    pub linguistic_group_count: usize,
    pub korean_character_count: usize,
    pub korean_repertoire: String,
    pub ready_for_font_repertoire: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub translation_project_complete: bool,
}
