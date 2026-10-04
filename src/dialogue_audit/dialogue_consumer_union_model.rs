use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueConsumerUnionAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueConsumerUnionAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_asset_count: usize,
    pub stored_coordinate_count: usize,
    pub primary_semantic_group_count: usize,
    pub selector_semantic_group_count: usize,
    pub overlapping_semantic_group_count: usize,
    pub consumer_union_semantic_group_count: usize,
    pub primary_coordinate_count: usize,
    pub selector_coordinate_count: usize,
    pub overlapping_coordinate_count: usize,
    pub consumer_union_coordinate_count: usize,
    pub stored_coordinate_outside_consumer_union_count: usize,
    pub assets: Vec<DialogueConsumerAssetCoverage>,
    pub primary_and_selector_population_complete: bool,
    pub complete_game_text_consumer_inventory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueConsumerAssetCoverage {
    pub source_path: String,
    pub stored_coordinate_count: usize,
    pub primary_coordinate_count: usize,
    pub selector_coordinate_count: usize,
    pub consumer_union_coordinate_count: usize,
    pub stored_coordinate_outside_consumer_union_count: usize,
}
