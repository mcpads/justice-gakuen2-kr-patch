use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueGlossary {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_corpus_sha256: String,
    pub evidence_sources: BTreeMap<String, String>,
    pub entries: Vec<DialogueGlossaryEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueGlossaryEntry {
    pub source: String,
    pub korean: String,
    pub category: String,
    pub status: DialogueGlossaryStatus,
    pub evidence_ids: Vec<String>,
    pub usage_note: String,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialogueGlossaryStatus {
    Candidate,
    Approved,
}

#[derive(Debug, Clone)]
pub struct DialogueGlossaryAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub glossary: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueGlossaryAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_corpus_sha256: String,
    pub glossary_sha256: String,
    pub entry_count: usize,
    pub candidate_entry_count: usize,
    pub approved_entry_count: usize,
    pub matched_group_count: usize,
    pub matched_term_occurrence_count: usize,
    pub all_terms_project_owner_approved: bool,
}
