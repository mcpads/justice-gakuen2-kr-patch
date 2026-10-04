use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SurfaceInventoryManifestDocument {
    pub(super) kind: String,
    pub(super) family_id: String,
    pub(super) source_bin_sha256: String,
    pub(super) root_surface_id: String,
    pub(super) traversal_boundary: SurfaceTraversalBoundary,
    pub(super) shards: Vec<PathBuf>,
    pub(super) bindings: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SurfaceInventoryShardDocument {
    pub(super) kind: String,
    pub(super) family_id: String,
    pub(super) nodes: Vec<SurfaceNode>,
    pub(super) edges: Vec<SurfaceEdge>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceTraversalBoundary {
    pub entry: String,
    pub terminal_rule: String,
    pub beyond_boundary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SurfaceResolution {
    Resolved,
    Excluded,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SurfaceNode {
    pub(super) id: String,
    pub(super) resolution: SurfaceResolution,
    pub(super) entry_route: String,
    pub(super) first_stable_stop: String,
    pub(super) consumer_class: Option<String>,
    pub(super) target_objects: Vec<SurfaceTargetObject>,
    pub(super) unresolved_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct SurfaceTargetObject {
    pub(super) path: String,
    pub(super) layer: SurfaceTargetLayer,
    pub(super) role: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(super) enum SurfaceTargetLayer {
    DecodedMember,
    DecodedRecord,
    StoredRecord,
    IsoRecord,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SurfaceEdge {
    pub(super) id: String,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) selection_asset_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SurfaceInventory {
    pub(super) family_id: String,
    pub(super) source_bin_sha256: String,
    pub(super) root_surface_id: String,
    pub(super) traversal_boundary: SurfaceTraversalBoundary,
    pub(super) nodes: Vec<SurfaceNode>,
    pub(super) edges: Vec<SurfaceEdge>,
    pub(super) shard_count: usize,
    pub(super) binding_files: BTreeMap<String, Vec<u8>>,
    pub(super) identity_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModeSelectPanelSelection<'a> {
    pub(crate) panel_index: usize,
    pub(crate) asset_id: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModeSelectDispatcherBuildReport {
    pub kind: String,
    pub writer_record_path: String,
    pub dispatcher_record_path: String,
    pub panel_index_state_address: String,
    pub jump_table_address: String,
    pub entry_count: usize,
    pub source_records_match: bool,
    pub writer_sequence_matches: bool,
    pub dispatcher_sequence_matches: bool,
    pub panel_population_matches: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SurfaceInventoryBuildReport {
    pub kind: String,
    pub family_id: String,
    pub source_bin_sha256: String,
    pub inventory_sha256: String,
    pub root_surface_id: String,
    pub traversal_boundary: SurfaceTraversalBoundary,
    pub shard_count: usize,
    pub binding_file_count: usize,
    pub surface_count: usize,
    pub edge_count: usize,
    pub resolved_surface_count: usize,
    pub excluded_surface_count: usize,
    pub unresolved_surface_count: usize,
    pub mode_select_root_selection_count: usize,
    pub all_surfaces_classified: bool,
    pub root_selection_population_matches: bool,
    pub mode_select_dispatcher: ModeSelectDispatcherBuildReport,
}
