//! Separates JP source cells, semantic references, and observed source usage.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use crate::tim::{Cell, cells_overlap};

use super::assets::parse_hex_offset;
use super::model::{
    PhysicalRegionId, PracticalResultEntry, PracticalResultPhysicalRegionCatalog,
    PracticalResultSourceGlyphCatalog, PracticalResultSourceUsageStatus, PracticalResultStrategy,
    SourceReferenceId,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PhysicalSourceCellKey {
    pub(super) source_path: String,
    pub(super) tim_offset: usize,
    pub(super) bpp: u8,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl PhysicalSourceCellKey {
    pub(super) fn cell(&self) -> Cell {
        Cell {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Debug)]
pub(super) struct PhysicalSourceCell {
    pub(super) region_id: PhysicalRegionId,
    pub(super) source_indexed_sha256: String,
    pub(super) expected_source_glyph: Option<char>,
    pub(super) reference_count: usize,
}

#[derive(Debug)]
pub(super) struct SemanticSourceReference {
    pub(super) entry_id: String,
    pub(super) reference_id: SourceReferenceId,
    pub(super) physical_region_id: PhysicalRegionId,
    pub(super) sequence_index: Option<usize>,
    pub(super) expected_source_glyph: Option<char>,
    pub(super) physical_cell: PhysicalSourceCellKey,
    pub(super) source_usage_group: ObservedSourceUsageKey,
}

#[derive(Debug)]
pub(super) struct ObservedSourceUsageGroup {
    pub(super) entry_id: String,
    pub(super) source_path: String,
    pub(super) tim_offset: usize,
    pub(super) bpp: u8,
    pub(super) reference_ids: Vec<SourceReferenceId>,
    pub(super) status: PracticalResultSourceUsageStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ObservedSourceUsageKey {
    pub(super) entry_id: String,
    pub(super) source_path: String,
    pub(super) tim_offset: usize,
    pub(super) bpp: u8,
}

pub(super) struct PracticalResultSourceOwnership {
    pub(super) physical_cells: BTreeMap<PhysicalSourceCellKey, PhysicalSourceCell>,
    pub(super) semantic_references: Vec<SemanticSourceReference>,
    pub(super) observed_source_usage_groups: Vec<ObservedSourceUsageGroup>,
}

impl PracticalResultSourceOwnership {
    pub(super) fn semantic_reference_count(&self) -> usize {
        self.semantic_references.len()
    }

    pub(super) fn observed_source_usage_group_count(&self) -> usize {
        self.observed_source_usage_groups.len()
    }

    pub(super) fn shared_physical_cell_count(&self) -> usize {
        self.physical_cells
            .values()
            .filter(|cell| cell.reference_count > 1)
            .count()
    }

    pub(super) fn overlapping_physical_cell_pair_count(&self) -> usize {
        let cells = self.physical_cells.keys().collect::<Vec<_>>();
        cells
            .iter()
            .enumerate()
            .flat_map(|(left_index, left)| {
                cells
                    .iter()
                    .skip(left_index + 1)
                    .map(move |right| (*left, *right))
            })
            .filter(|(left, right)| physical_cells_overlap(left, right))
            .count()
    }

    pub(super) fn overlapping_physical_cell_count(&self) -> usize {
        let cells = self.physical_cells.keys().collect::<Vec<_>>();
        let mut overlapping = BTreeSet::new();
        for (left_index, left) in cells.iter().enumerate() {
            for right in cells.iter().skip(left_index + 1) {
                if physical_cells_overlap(left, right) {
                    overlapping.insert(*left);
                    overlapping.insert(*right);
                }
            }
        }
        overlapping.len()
    }

    fn validate_internal_graph(&self) -> Result<()> {
        let mut reference_ids = BTreeSet::new();
        let groups_by_key = self
            .observed_source_usage_groups
            .iter()
            .map(|group| {
                (
                    ObservedSourceUsageKey {
                        entry_id: group.entry_id.clone(),
                        source_path: group.source_path.clone(),
                        tim_offset: group.tim_offset,
                        bpp: group.bpp,
                    },
                    group,
                )
            })
            .collect::<BTreeMap<_, _>>();
        for reference in &self.semantic_references {
            let physical = self.physical_cells.get(&reference.physical_cell);
            let source_usage_group = groups_by_key.get(&reference.source_usage_group);
            ensure!(
                !reference.entry_id.is_empty()
                    && !reference.reference_id.is_empty()
                    && !reference.physical_region_id.is_empty()
                    && physical.is_some_and(|physical| {
                        physical.region_id == reference.physical_region_id
                            && reference.expected_source_glyph.is_none_or(|expected| {
                                physical.expected_source_glyph == Some(expected)
                            })
                    })
                    && reference.source_usage_group.entry_id == reference.entry_id
                    && source_usage_group.is_some_and(|group| {
                        group.reference_ids.contains(&reference.reference_id)
                    })
                    && reference_ids.insert(reference.reference_id.as_str()),
                "practical-result semantic source-reference graph is inconsistent"
            );
        }
        let mut group_ids = BTreeSet::new();
        let mut grouped_reference_ids = BTreeSet::new();
        for group in &self.observed_source_usage_groups {
            ensure!(
                !group.entry_id.is_empty()
                    && !group.source_path.is_empty()
                    && !group.reference_ids.is_empty()
                    && matches!(group.bpp, 4 | 8)
                    && matches!(
                        group.status,
                        PracticalResultSourceUsageStatus::RuntimeObserved
                            | PracticalResultSourceUsageStatus::StaticObserved
                            | PracticalResultSourceUsageStatus::Unresolved
                    )
                    && group_ids.insert((
                        group.entry_id.as_str(),
                        group.source_path.as_str(),
                        group.tim_offset,
                        group.bpp,
                    ))
                    && group
                        .reference_ids
                        .iter()
                        .all(|reference_id| grouped_reference_ids.insert(reference_id.as_str())),
                "practical-result observed source-usage graph is inconsistent"
            );
        }
        Ok(())
    }
}

fn physical_cells_overlap(left: &PhysicalSourceCellKey, right: &PhysicalSourceCellKey) -> bool {
    left.source_path == right.source_path
        && left.tim_offset == right.tim_offset
        && left.bpp == right.bpp
        && cells_overlap(left.cell(), right.cell())
}

pub(super) fn derive_source_ownership(
    entries: &[PracticalResultEntry],
    physical_region_catalog: &PracticalResultPhysicalRegionCatalog,
    source_glyph_catalog: &PracticalResultSourceGlyphCatalog,
) -> Result<PracticalResultSourceOwnership> {
    let source_glyphs_by_cell = index_source_glyphs(source_glyph_catalog)?;
    let (mut physical_cells, key_by_region_id) =
        index_physical_regions(physical_region_catalog, &source_glyphs_by_cell)?;
    let mut semantic_references = Vec::new();
    let mut all_reference_ids = BTreeSet::new();
    let mut observed_source_usage_groups =
        BTreeMap::<ObservedSourceUsageKey, ObservedSourceUsageGroup>::new();

    for entry in entries {
        validate_sequence_denominator(entry)?;
        let source_glyphs = if entry.strategy == PracticalResultStrategy::GlyphSequence {
            let glyphs = entry.source_text.chars().collect::<Vec<_>>();
            ensure!(
                glyphs.len() == entry.source_references.len(),
                "practical-result glyph sequence {} has {} source glyphs but {} references",
                entry.id,
                glyphs.len(),
                entry.source_references.len()
            );
            Some(glyphs)
        } else {
            None
        };
        for source_reference in &entry.source_references {
            ensure!(
                all_reference_ids.insert(source_reference.reference_id.as_str()),
                "practical-result source reference {} is declared more than once",
                source_reference.reference_id
            );
            let expected_source_glyph = source_reference
                .sequence_index
                .and_then(|index| source_glyphs.as_ref().and_then(|glyphs| glyphs.get(index)))
                .copied();
            let key = key_by_region_id
                .get(&source_reference.physical_region_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "practical-result source reference {} names unknown physical region {}",
                        source_reference.reference_id,
                        source_reference.physical_region_id
                    )
                })?
                .clone();
            if let Some(expected_source_glyph) = expected_source_glyph {
                ensure!(
                    source_glyphs_by_cell.get(&key) == Some(&expected_source_glyph),
                    "practical-result glyph sequence {} maps {:?} to the wrong physical source cell",
                    entry.id,
                    expected_source_glyph
                );
            }
            let physical = physical_cells
                .get_mut(&key)
                .expect("physical-region key came from the indexed catalog");
            ensure!(
                physical.region_id == source_reference.physical_region_id
                    && expected_source_glyph.is_none_or(|expected| {
                        physical.expected_source_glyph == Some(expected)
                    }),
                "practical-result physical source region has conflicting identities"
            );
            physical.reference_count += 1;
            let source_usage_group = ObservedSourceUsageKey {
                entry_id: entry.id.clone(),
                source_path: key.source_path.clone(),
                tim_offset: key.tim_offset,
                bpp: key.bpp,
            };
            semantic_references.push(SemanticSourceReference {
                entry_id: entry.id.clone(),
                reference_id: source_reference.reference_id.clone(),
                physical_region_id: source_reference.physical_region_id.clone(),
                sequence_index: source_reference.sequence_index,
                expected_source_glyph,
                physical_cell: key,
                source_usage_group: source_usage_group.clone(),
            });
            add_observed_source_usage(
                &mut observed_source_usage_groups,
                source_usage_group,
                source_reference.reference_id.clone(),
                source_reference.source_usage_status,
            )?;
        }
        for source_reference in &entry.unresolved_source_references {
            ensure!(
                all_reference_ids.insert(source_reference.reference_id.as_str()),
                "practical-result source reference {} is declared more than once",
                source_reference.reference_id
            );
            add_observed_source_usage(
                &mut observed_source_usage_groups,
                ObservedSourceUsageKey {
                    entry_id: entry.id.clone(),
                    source_path: source_reference.source_path.clone(),
                    tim_offset: parse_hex_offset(&source_reference.tim_offset)?,
                    bpp: source_reference.bpp,
                },
                source_reference.reference_id.clone(),
                source_reference.source_usage_status,
            )?;
        }
    }

    ensure!(
        semantic_references.len()
            == physical_cells
                .values()
                .map(|cell| cell.reference_count)
                .sum::<usize>(),
        "practical-result source-reference denominator changed"
    );
    let ownership = PracticalResultSourceOwnership {
        physical_cells,
        semantic_references,
        observed_source_usage_groups: observed_source_usage_groups.into_values().collect(),
    };
    ownership.validate_internal_graph()?;
    Ok(ownership)
}

fn index_physical_regions(
    catalog: &PracticalResultPhysicalRegionCatalog,
    source_glyphs_by_cell: &BTreeMap<PhysicalSourceCellKey, char>,
) -> Result<(
    BTreeMap<PhysicalSourceCellKey, PhysicalSourceCell>,
    BTreeMap<PhysicalRegionId, PhysicalSourceCellKey>,
)> {
    let mut physical_cells = BTreeMap::new();
    let mut key_by_region_id = BTreeMap::new();
    for region in &catalog.regions {
        let key = PhysicalSourceCellKey {
            source_path: region.source_path.clone(),
            tim_offset: parse_hex_offset(&region.tim_offset)?,
            bpp: region.bpp,
            x: region.cell.x,
            y: region.cell.y,
            width: region.cell.width,
            height: region.cell.height,
        };
        ensure!(
            !region.region_id.is_empty()
                && !region.source_path.is_empty()
                && !region.source_indexed_sha256.is_empty()
                && matches!(region.bpp, 4 | 8)
                && region.cell.width > 0
                && region.cell.height > 0,
            "practical-result physical region {} is invalid",
            region.region_id
        );
        ensure!(
            key_by_region_id
                .insert(region.region_id.clone(), key.clone())
                .is_none(),
            "practical-result physical region id {} is declared more than once",
            region.region_id
        );
        let physical = PhysicalSourceCell {
            region_id: region.region_id.clone(),
            source_indexed_sha256: region.source_indexed_sha256.clone(),
            expected_source_glyph: source_glyphs_by_cell.get(&key).copied(),
            reference_count: 0,
        };
        ensure!(
            physical_cells.insert(key, physical).is_none(),
            "practical-result physical region catalog maps multiple ids to one source cell"
        );
    }
    Ok((physical_cells, key_by_region_id))
}

fn index_source_glyphs(
    catalog: &PracticalResultSourceGlyphCatalog,
) -> Result<BTreeMap<PhysicalSourceCellKey, char>> {
    let mut glyphs_by_cell = BTreeMap::new();
    for bank in &catalog.banks {
        let tim_offset = parse_hex_offset(&bank.tim_offset)?;
        for row in &bank.rows {
            for (index, glyph) in row.glyphs.chars().enumerate() {
                let x = row
                    .start_x
                    .checked_add(
                        index
                            .checked_mul(row.cell_width)
                            .ok_or_else(|| anyhow::anyhow!("source-glyph row width overflow"))?,
                    )
                    .ok_or_else(|| anyhow::anyhow!("source-glyph row x overflow"))?;
                let key = PhysicalSourceCellKey {
                    source_path: bank.source_path.clone(),
                    tim_offset,
                    bpp: bank.bpp,
                    x,
                    y: row.y,
                    width: row.cell_width,
                    height: row.cell_height,
                };
                ensure!(
                    glyphs_by_cell.insert(key, glyph).is_none(),
                    "practical-result source-glyph catalog has overlapping cells"
                );
            }
        }
    }
    Ok(glyphs_by_cell)
}

fn add_observed_source_usage(
    groups: &mut BTreeMap<ObservedSourceUsageKey, ObservedSourceUsageGroup>,
    key: ObservedSourceUsageKey,
    reference_id: SourceReferenceId,
    status: PracticalResultSourceUsageStatus,
) -> Result<()> {
    match groups.get_mut(&key) {
        Some(group) => {
            ensure!(
                group.status == status,
                "practical-result observed source usage {} has mixed status",
                key.entry_id
            );
            group.reference_ids.push(reference_id);
        }
        None => {
            groups.insert(
                key.clone(),
                ObservedSourceUsageGroup {
                    entry_id: key.entry_id,
                    source_path: key.source_path,
                    tim_offset: key.tim_offset,
                    bpp: key.bpp,
                    reference_ids: vec![reference_id],
                    status,
                },
            );
        }
    }
    Ok(())
}

fn validate_sequence_denominator(entry: &PracticalResultEntry) -> Result<()> {
    if entry.strategy != PracticalResultStrategy::GlyphSequence {
        return Ok(());
    }
    let indices = entry
        .source_references
        .iter()
        .map(|source_reference| source_reference.sequence_index)
        .collect::<Option<BTreeSet<_>>>();
    let expected = (0..entry.source_references.len()).collect::<BTreeSet<_>>();
    ensure!(
        indices.as_ref() == Some(&expected),
        "practical-result glyph sequence {} does not have one reference at every index 0..{}",
        entry.id,
        entry.source_references.len()
    );
    Ok(())
}
