use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueSourceCorpusConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueSourceCorpusBuildReport {
    pub output_directory: String,
    pub manifest_sha256: String,
    pub asset_count: usize,
    pub bank_count: usize,
    pub shard_count: usize,
    pub coordinate_count: usize,
    pub semantic_shared_group_count: usize,
    pub roundtrip_verified_coordinate_count: usize,
    pub maximum_shard_bytes: usize,
    pub coordinate_partition_complete: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueSourceCorpus {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub coordinate_count: usize,
    pub raw_unique_entry_count: usize,
    pub semantic_shared_group_count: usize,
    pub glyph_occurrence_count: usize,
    pub control_occurrence_count: usize,
    pub alignment_padding_word_count: usize,
    pub roundtrip_verified_coordinate_count: usize,
    pub ready_for_translation: bool,
    pub markup_contract: String,
    pub assets: Vec<DialogueCorpusAsset>,
}

#[derive(Debug, Serialize)]
pub struct DialogueCorpusAsset {
    pub source_path: String,
    pub stored_sha256: String,
    pub decoded_sha256: String,
    pub fixed_cell_count: usize,
    pub banks: Vec<DialogueCorpusBank>,
}

#[derive(Debug, Serialize)]
pub struct DialogueCorpusBank {
    pub selector_index: usize,
    pub entries: Vec<DialogueCorpusEntry>,
}

#[derive(Debug, Serialize)]
pub struct DialogueCorpusEntry {
    pub coordinate_id: String,
    pub entry_index: usize,
    pub decoded_offset: String,
    pub runtime_address: String,
    pub raw_entry_sha256: String,
    pub semantic_source_sha256: String,
    pub semantic_shared_coordinate_count: usize,
    pub raw_words: Vec<String>,
    pub alignment_padding_word_count: usize,
    pub source_markup: String,
    pub tokens: Vec<DialogueCorpusToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialogueCorpusToken {
    Glyph {
        code: String,
        pixel_sha256: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        semantic_id: Option<String>,
        codebook_status: String,
    },
    Control {
        code: String,
        semantic_name: String,
        arguments: Vec<String>,
    },
}
