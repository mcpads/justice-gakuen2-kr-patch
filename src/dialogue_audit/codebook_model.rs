use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct DialogueCodebookAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueCodebook {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) evidence_sources: BTreeMap<String, String>,
    pub(super) entries: Vec<DialogueCodebookEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueCodebookEntry {
    pub(super) pixel_sha256: String,
    pub(super) meaning: DialogueGlyphMeaning,
    pub(super) status: DialogueCodebookStatus,
    pub(super) evidence: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reviewed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DialogueGlyphMeaning {
    Character { text: String },
    Symbol { id: String, display: String },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum DialogueCodebookStatus {
    Candidate,
    KnownTextVerified,
    SourcePixelVerified,
    HumanApproved,
}

impl DialogueCodebookStatus {
    pub(super) fn resolves_source(self) -> bool {
        matches!(
            self,
            Self::KnownTextVerified | Self::SourcePixelVerified | Self::HumanApproved
        )
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::KnownTextVerified => "known_text_verified",
            Self::SourcePixelVerified => "source_pixel_verified",
            Self::HumanApproved => "human_approved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedDialogueGlyph {
    pub(super) text: String,
    pub(super) semantic_id: Option<String>,
    pub(super) codebook_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueGlyphSourceReference {
    pub source_asset: String,
    pub code: String,
    pub occurrence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueUnresolvedGlyph {
    pub pixel_sha256: String,
    pub occurrence_count: usize,
    pub source_references: Vec<DialogueGlyphSourceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueCodebookAuditManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub codebook_entry_count: usize,
    pub candidate_entry_count: usize,
    pub verified_entry_count: usize,
    pub source_pixel_hash_count: usize,
    pub used_source_pixel_hash_count: usize,
    pub resolved_used_source_pixel_hash_count: usize,
    pub unresolved_used_source_pixel_hash_count: usize,
    pub unresolved_glyph_occurrence_count: usize,
    pub ready_for_glyph_decode: bool,
    pub unresolved: Vec<DialogueUnresolvedGlyph>,
    pub limitations: Vec<String>,
}
