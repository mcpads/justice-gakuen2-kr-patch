use std::path::PathBuf;

use serde::Serialize;

use crate::menu_audit::{
    AddressFlowReferenceAudit, MenuConsumerEvidence, MenuNonTextEvidence, SharedStringReferences,
};

#[derive(Debug, Clone)]
pub struct MenuSourceWorkspaceConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub address_flow_state_budget: usize,
}

#[derive(Debug, Serialize)]
pub struct MenuSourceWorkspaceManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub menu_decoded_sha256: String,
    pub menu_code_analysis_sha256: String,
    pub menu_glyph_analysis_sha256: String,
    pub address_flow_state_budget: usize,
    pub scope: String,
    pub overlay_count: usize,
    pub candidate_string_count: usize,
    pub fully_exact_decoded_string_count: usize,
    pub partially_exact_decoded_string_count: usize,
    pub unresolved_string_count: usize,
    pub exact_decoded_glyph_occurrence_count: usize,
    pub unresolved_glyph_occurrence_count: usize,
    pub entrypoint_reachable_static_reference_string_count: usize,
    pub entrypoint_reachable_address_materialization_reference_count: usize,
    pub entrypoint_reachable_memory_access_reference_count: usize,
    pub entrypoint_reachable_loaded_word_reference_count: usize,
    pub entrypoint_reachable_direct_pointer_load_string_count: usize,
    pub entrypoint_reachable_direct_pointer_load_reference_count: usize,
    pub consumer_confirmed_string_count: usize,
    pub consumer_confirmed_reference_count: usize,
    pub confirmed_non_text_candidate_count: usize,
    pub unclassified_candidate_count: usize,
    pub unclassified_entrypoint_reachable_direct_pointer_load_candidate_count: usize,
    pub translation_target_count: Option<usize>,
    pub acquisition_complete: bool,
    pub max_entries_per_shard: usize,
    pub max_bytes_per_shard: usize,
    pub shard_count: usize,
    pub shards: Vec<MenuSourceShardRef>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuSourceShardRef {
    pub overlay_path: String,
    pub shard_id: String,
    pub path: String,
    pub entry_count: usize,
    pub content_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct MenuSourceShard {
    pub kind: String,
    pub shard_id: String,
    pub source_bin_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub menu_code_analysis_sha256: String,
    pub menu_glyph_analysis_sha256: String,
    pub overlay_path: String,
    pub entries: Vec<MenuSourceCandidate>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuSourceAdmission {
    FormatConfirmedRendererConsumer,
    ConfirmedNonTextStructure,
    StaticReferenceCandidate,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuSourceConsumerEvidence {
    KanriMenuLabelTable,
    KanriPlacementRecordTable,
    KanriStatusRatingTable,
    NewoptPlacementPointerTable,
    MgtitPlacementRecordTable,
    NewoptDirectStringWriterCall,
}

impl From<MenuConsumerEvidence> for MenuSourceConsumerEvidence {
    fn from(evidence: MenuConsumerEvidence) -> Self {
        match evidence {
            MenuConsumerEvidence::KanriMenuLabelTable => Self::KanriMenuLabelTable,
            MenuConsumerEvidence::KanriPlacementRecordTable => Self::KanriPlacementRecordTable,
            MenuConsumerEvidence::KanriStatusRatingTable => Self::KanriStatusRatingTable,
            MenuConsumerEvidence::MgtitPlacementRecordTable => Self::MgtitPlacementRecordTable,
            MenuConsumerEvidence::NewoptDirectStringWriterCall => {
                Self::NewoptDirectStringWriterCall
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuSourceNonTextEvidence {
    KanriCountedObjectRecordTable,
    Koubai2CommandByteSequence,
    MiniselPrimitiveRecordTable,
    Plsel5CountedParameterPointerTable,
}

impl From<MenuNonTextEvidence> for MenuSourceNonTextEvidence {
    fn from(evidence: MenuNonTextEvidence) -> Self {
        match evidence {
            MenuNonTextEvidence::KanriCountedObjectRecordTable => {
                Self::KanriCountedObjectRecordTable
            }
            MenuNonTextEvidence::Koubai2CommandByteSequence => Self::Koubai2CommandByteSequence,
            MenuNonTextEvidence::MiniselPrimitiveRecordTable => Self::MiniselPrimitiveRecordTable,
            MenuNonTextEvidence::Plsel5CountedParameterPointerTable => {
                Self::Plsel5CountedParameterPointerTable
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MenuSourceCandidate {
    pub candidate_id: String,
    pub target_offset: String,
    pub admission: MenuSourceAdmission,
    pub consumer_confirmed: bool,
    pub consumer_evidence: Vec<MenuSourceConsumerEvidence>,
    pub consumer_reference_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub non_text_evidence: Vec<MenuSourceNonTextEvidence>,
    pub fully_exact_text: Option<String>,
    pub exact_decoded_glyph_count: usize,
    pub unresolved_glyph_count: usize,
    pub tokens: Vec<MenuSourceToken>,
    pub pointer_offsets: Vec<String>,
    pub address_materialization_offsets: Vec<String>,
    pub memory_access_offsets: Vec<String>,
    pub loaded_word_references: Vec<MenuSourceLoadedWordReference>,
    pub entrypoint_reachable_address_materialization_references: Vec<AddressFlowReferenceAudit>,
    pub entrypoint_reachable_memory_access_references: Vec<AddressFlowReferenceAudit>,
    pub shared_references: Vec<SharedStringReferences>,
}

#[derive(Debug, Serialize)]
pub struct MenuSourceLoadedWordReference {
    pub instruction_offset: String,
    pub load_instruction_offset: String,
    pub storage_address: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entrypoint_reachable_seed_offsets: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entrypoint_reachable_direct_pointer_load_seed_offsets: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuSourceTokenKind {
    Glyph,
    Skip,
}

#[derive(Debug, Serialize)]
pub struct MenuSourceToken {
    pub raw_code: String,
    pub normalized_code: String,
    pub control_nibble: u8,
    pub kind: MenuSourceTokenKind,
    pub pixel_sha256: Option<String>,
    pub exact_dialogue_pixel_text: Option<String>,
}
