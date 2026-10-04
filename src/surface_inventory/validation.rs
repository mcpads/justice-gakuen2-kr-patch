use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use super::model::{SurfaceEdge, SurfaceNode, SurfaceResolution, SurfaceTraversalBoundary};

pub(super) fn validate_surface_graph(
    family_id: &str,
    source_bin_sha256: &str,
    root_surface_id: &str,
    boundary: &SurfaceTraversalBoundary,
    nodes: &[SurfaceNode],
    edges: &[SurfaceEdge],
) -> Result<()> {
    validate_local_id(family_id, "surface family")?;
    validate_sha256(source_bin_sha256, "surface inventory source BIN")?;
    validate_qualified_id(root_surface_id, "root surface")?;
    ensure_nonempty(&boundary.entry, "surface traversal entry")?;
    ensure_nonempty(&boundary.terminal_rule, "surface traversal terminal rule")?;
    ensure_nonempty(
        &boundary.beyond_boundary,
        "surface traversal beyond-boundary rule",
    )?;
    ensure!(!nodes.is_empty(), "surface inventory has no nodes");

    let mut node_indices = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        validate_node(node)?;
        ensure!(
            node_indices.insert(node.id.as_str(), index).is_none(),
            "duplicate surface node {}",
            node.id
        );
    }
    ensure!(
        node_indices.contains_key(root_surface_id),
        "surface inventory root {root_surface_id} is missing"
    );

    let mut edge_ids = BTreeSet::new();
    let mut adjacency = vec![Vec::new(); nodes.len()];
    let mut indegrees = vec![0usize; nodes.len()];
    for edge in edges {
        validate_qualified_id(&edge.id, "surface edge")?;
        ensure!(
            edge_ids.insert(edge.id.as_str()),
            "duplicate surface edge {}",
            edge.id
        );
        let from = *node_indices.get(edge.from.as_str()).with_context(|| {
            format!("surface edge {} has unknown source {}", edge.id, edge.from)
        })?;
        let to = *node_indices
            .get(edge.to.as_str())
            .with_context(|| format!("surface edge {} has unknown target {}", edge.id, edge.to))?;
        ensure!(from != to, "surface edge {} is a self-loop", edge.id);
        if let Some(asset_id) = &edge.selection_asset_id {
            validate_local_id(asset_id, "selection asset")?;
        }
        adjacency[from].push(to);
        indegrees[to] += 1;
    }

    let root_index = node_indices[root_surface_id];
    ensure!(
        indegrees[root_index] == 0,
        "surface inventory root has an incoming edge"
    );
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from([root_index]);
    while let Some(index) = queue.pop_front() {
        if !reachable.insert(index) {
            continue;
        }
        queue.extend(adjacency[index].iter().copied());
    }
    ensure!(
        reachable.len() == nodes.len(),
        "surface inventory has nodes outside the root traversal"
    );

    let mut acyclic_indegrees = indegrees;
    let mut acyclic_queue = acyclic_indegrees
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut ordered_count = 0usize;
    while let Some(index) = acyclic_queue.pop_front() {
        ordered_count += 1;
        for &target in &adjacency[index] {
            acyclic_indegrees[target] -= 1;
            if acyclic_indegrees[target] == 0 {
                acyclic_queue.push_back(target);
            }
        }
    }
    ensure!(
        ordered_count == nodes.len(),
        "surface inventory traversal contains a cycle"
    );
    Ok(())
}

fn validate_node(node: &SurfaceNode) -> Result<()> {
    validate_qualified_id(&node.id, "surface node")?;
    ensure_nonempty(&node.entry_route, "surface entry route")?;
    ensure_nonempty(&node.first_stable_stop, "surface first stable stop")?;
    if let Some(consumer_class) = &node.consumer_class {
        validate_qualified_id(consumer_class, "surface consumer class")?;
    }
    let mut target_objects = BTreeSet::new();
    for target in &node.target_objects {
        validate_disc_path(&target.path)?;
        ensure_nonempty(&target.role, "surface target-object role")?;
        ensure!(
            target_objects.insert((target.path.as_str(), target.layer, target.role.as_str())),
            "surface node {} repeats a target object",
            node.id
        );
    }
    match node.resolution {
        SurfaceResolution::Resolved => {
            ensure!(
                node.consumer_class.is_some() && !node.target_objects.is_empty(),
                "resolved surface {} lacks a consumer class or target object",
                node.id
            );
            ensure!(
                node.unresolved_reason.is_none(),
                "resolved surface {} has an unresolved reason",
                node.id
            );
        }
        SurfaceResolution::Excluded | SurfaceResolution::Unresolved => {
            ensure!(
                node.unresolved_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "non-resolved surface {} lacks a reason",
                node.id
            );
        }
    }
    Ok(())
}

fn validate_qualified_id(value: &str, role: &str) -> Result<()> {
    ensure!(
        value.split('/').count() >= 2 && value.split('/').all(valid_id_segment),
        "{role} ID is not fully qualified: {value:?}"
    );
    Ok(())
}

fn validate_local_id(value: &str, role: &str) -> Result<()> {
    ensure!(valid_id_segment(value), "invalid {role} ID {value:?}");
    Ok(())
}

fn valid_id_segment(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn validate_sha256(value: &str, role: &str) -> Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{role} is not a lowercase SHA-256"
    );
    Ok(())
}

fn validate_disc_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "surface target object has an invalid disc path {value:?}"
    );
    Ok(())
}

fn ensure_nonempty(value: &str, role: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{role} is empty");
    Ok(())
}
