use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::mode_select_character_select_routes::validate_mode_select_character_select_routes;
use super::mode_select_direct_entry_routes::{
    ModeSelectDirectEntrySources, validate_mode_select_direct_entry_routes,
};
use super::mode_select_dispatcher::validate_mode_select_dispatcher;
use super::mode_select_options_records::validate_mode_select_options_and_records;
use super::mode_select_practical_entry::validate_mode_select_practical_entry;
use super::model::{
    ModeSelectPanelSelection, SurfaceInventory, SurfaceInventoryBuildReport, SurfaceResolution,
    SurfaceTargetLayer,
};

const MODE_SELECT_DESCENDANT_FAMILY_ID: &str = "mode-select-descendants";
const REPORT_KIND: &str = "justice_gakuen2_adopted_surface_inventory";

pub(crate) struct ModeSelectInventorySources<'a> {
    pub(crate) source_bin_sha256: &'a str,
    pub(crate) mode_select_menu_path: &'a str,
    pub(crate) mode_select_overlay_path: &'a str,
    pub(crate) mode_select_overlay: &'a [u8],
    pub(crate) main_executable: &'a [u8],
    pub(crate) character_select_overlays: &'a [Vec<u8>],
    pub(crate) character_select_textures_stored: &'a [Vec<u8>],
    pub(crate) cooperative_menu_stored: &'a [u8],
    pub(crate) options_overlay: &'a [u8],
    pub(crate) options_information_stored: &'a [u8],
    pub(crate) direct_entries: ModeSelectDirectEntrySources<'a>,
}

pub(crate) fn validate_mode_select_descendant_inventory(
    inventory: &SurfaceInventory,
    selections: &[ModeSelectPanelSelection<'_>],
    sources: &ModeSelectInventorySources<'_>,
) -> Result<SurfaceInventoryBuildReport> {
    validate_mode_select_inventory_source_binding(inventory, sources.source_bin_sha256)?;
    validate_binding_population(inventory)?;
    let root = inventory
        .nodes
        .iter()
        .find(|node| node.id == inventory.root_surface_id)
        .context("surface inventory lost its root node")?;
    ensure!(
        root.resolution == SurfaceResolution::Resolved,
        "MODE SELECT root surface is not resolved"
    );
    let root_targets = root
        .target_objects
        .iter()
        .map(|target| (target.path.as_str(), target.layer))
        .collect::<BTreeSet<_>>();
    let expected_root_targets = BTreeSet::from([
        (
            sources.mode_select_menu_path,
            SurfaceTargetLayer::DecodedRecord,
        ),
        (
            sources.mode_select_overlay_path,
            SurfaceTargetLayer::IsoRecord,
        ),
    ]);
    ensure!(
        root_targets == expected_root_targets,
        "MODE SELECT root target objects differ from the built source records"
    );

    let mode_ids = selections
        .iter()
        .map(|selection| selection.asset_id)
        .collect::<Vec<_>>();
    let mode_select_root_selection_count =
        validate_mode_select_root_population(inventory, &mode_ids)?;
    let dispatcher = validate_mode_select_dispatcher(
        inventory,
        sources.mode_select_overlay,
        sources.main_executable,
        selections,
    )?;
    validate_mode_select_character_select_routes(
        inventory,
        sources.main_executable,
        sources.character_select_overlays,
        sources.character_select_textures_stored,
        sources.cooperative_menu_stored,
    )?;
    validate_mode_select_options_and_records(
        inventory,
        sources.main_executable,
        sources.options_overlay,
        sources.options_information_stored,
    )?;
    validate_mode_select_practical_entry(
        inventory,
        sources.main_executable,
        sources
            .character_select_overlays
            .first()
            .context("MODE SELECT practical entry is missing PLSEL1 source")?,
        sources
            .character_select_textures_stored
            .first()
            .context("MODE SELECT practical entry is missing SELP1 source")?,
    )?;
    validate_mode_select_direct_entry_routes(
        inventory,
        sources.main_executable,
        &sources.direct_entries,
    )?;

    Ok(SurfaceInventoryBuildReport {
        kind: REPORT_KIND.to_string(),
        family_id: inventory.family_id.clone(),
        source_bin_sha256: inventory.source_bin_sha256.clone(),
        inventory_sha256: inventory.identity_sha256.clone(),
        root_surface_id: inventory.root_surface_id.clone(),
        traversal_boundary: inventory.traversal_boundary.clone(),
        shard_count: inventory.shard_count,
        binding_file_count: inventory.binding_files.len(),
        surface_count: inventory.nodes.len(),
        edge_count: inventory.edges.len(),
        resolved_surface_count: count_resolution(inventory, SurfaceResolution::Resolved),
        excluded_surface_count: count_resolution(inventory, SurfaceResolution::Excluded),
        unresolved_surface_count: count_resolution(inventory, SurfaceResolution::Unresolved),
        mode_select_root_selection_count,
        all_surfaces_classified: true,
        root_selection_population_matches: true,
        mode_select_dispatcher: dispatcher,
    })
}

fn validate_binding_population(inventory: &SurfaceInventory) -> Result<()> {
    let binding_files = inventory
        .binding_files
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        super::mode_select_character_select_routes::BINDING_FILE,
        super::mode_select_direct_entry_routes::BINDING_FILE,
        super::mode_select_dispatcher::BINDING_FILE,
        super::mode_select_options_records::BINDING_FILE,
        super::mode_select_practical_entry::BINDING_FILE,
    ]);
    ensure!(
        binding_files == expected,
        "MODE SELECT surface inventory source-binding population changed"
    );
    Ok(())
}

pub(super) fn validate_mode_select_root_population(
    inventory: &SurfaceInventory,
    mode_ids: &[&str],
) -> Result<usize> {
    let expected_mode_ids = mode_ids.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        expected_mode_ids.len() == mode_ids.len(),
        "MODE SELECT build contains duplicate mode IDs"
    );
    let mut selected_mode_ids = BTreeSet::new();
    for edge in inventory
        .edges
        .iter()
        .filter(|edge| edge.from == inventory.root_surface_id)
    {
        let asset_id = edge.selection_asset_id.as_deref().with_context(|| {
            format!(
                "MODE SELECT root edge {} has no selected asset identity",
                edge.id
            )
        })?;
        ensure!(
            selected_mode_ids.insert(asset_id),
            "MODE SELECT root repeats selection asset {asset_id}"
        );
    }
    ensure!(
        selected_mode_ids == expected_mode_ids,
        "surface inventory MODE SELECT root population differs from the built asset population"
    );
    Ok(selected_mode_ids.len())
}

pub(crate) fn validate_mode_select_inventory_source_binding(
    inventory: &SurfaceInventory,
    source_bin_sha256: &str,
) -> Result<()> {
    ensure!(
        inventory.family_id == MODE_SELECT_DESCENDANT_FAMILY_ID,
        "development build selected the wrong surface inventory family"
    );
    ensure!(
        inventory.source_bin_sha256 == source_bin_sha256,
        "surface inventory source BIN identity changed"
    );
    Ok(())
}

fn count_resolution(inventory: &SurfaceInventory, resolution: SurfaceResolution) -> usize {
    inventory
        .nodes
        .iter()
        .filter(|node| node.resolution == resolution)
        .count()
}
