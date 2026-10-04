//! Classifies JP source-read footprints without creating KR placement authority.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::tim::Cell;

use super::super::model::{
    PracticalResultAssets, PracticalResultDynamicStreamStateHeaderBuild,
    PracticalResultProtectedSourceRegionIntersectionBuild,
    PracticalResultSourceConsumerReachability, PracticalResultSourceProtectionRole,
    PracticalResultSourceReadFootprintBuild, PracticalResultSourceReadFootprintDerivation,
    PracticalResultSourceReadPopulationBuild, PracticalResultSourceReadSetAssessment,
    PracticalResultSourceRegionIntersectionBuild, PracticalResultSourceTileGeometryBuild,
};
use super::super::projection_model::PracticalResultConsumerProjectionId;
use super::super::source_atlas_domain_model::SourceAtlasDomainId;
use super::footprint::{
    SourceAtlasRectangle, SourceAtlasTileGeometry, SourceConsumerReachability,
    SourceProjectionFootprint, SourceProjectionReadPopulation, SourceReadFootprintDerivation,
    SourceReadSetAssessment,
};

pub(super) fn build_source_read_footprint_reports(
    footprints: &[SourceProjectionFootprint<
        PracticalResultConsumerProjectionId,
        SourceAtlasDomainId,
    >],
    assets: &PracticalResultAssets,
) -> Result<Vec<PracticalResultSourceReadFootprintBuild>> {
    footprints
        .iter()
        .map(|footprint| build_source_read_footprint_report(footprint, assets))
        .collect()
}

fn build_source_read_footprint_report(
    footprint: &SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>,
    assets: &PracticalResultAssets,
) -> Result<PracticalResultSourceReadFootprintBuild> {
    let physical_source_region_intersections = assets
        .physical_region_catalog
        .regions
        .iter()
        .filter(|region| {
            region.source_atlas_domain_id.as_ref() == Some(&footprint.source_atlas_domain_id)
        })
        .filter_map(|region| {
            build_source_region_intersection(region.region_id.to_string(), region.cell, footprint)
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    let protected_source_region_intersections = assets
        .protected_content
        .regions
        .iter()
        .filter(|region| region.source_atlas_domain_id == footprint.source_atlas_domain_id)
        .filter_map(|region| {
            build_protected_source_region_intersection(
                region.id.to_string(),
                region.protection_role,
                region.cell,
                footprint,
            )
            .transpose()
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(PracticalResultSourceReadFootprintBuild {
        projection_id: footprint.projection_id.to_string(),
        source_atlas_domain_id: footprint.source_atlas_domain_id.to_string(),
        derivation: match footprint.derivation {
            SourceReadFootprintDerivation::CatalogDescriptorFragments => {
                PracticalResultSourceReadFootprintDerivation::CatalogDescriptorFragments
            }
            SourceReadFootprintDerivation::CatalogDigitDescriptorTable => {
                PracticalResultSourceReadFootprintDerivation::CatalogDigitDescriptorTable
            }
            SourceReadFootprintDerivation::DirectNumericSpriteSites => {
                PracticalResultSourceReadFootprintDerivation::DirectNumericSpriteSites
            }
            SourceReadFootprintDerivation::DirectSpriteSelectorTable => {
                PracticalResultSourceReadFootprintDerivation::DirectSpriteSelectorTable
            }
            SourceReadFootprintDerivation::CatalogDescriptorSelectorTable => {
                PracticalResultSourceReadFootprintDerivation::CatalogDescriptorSelectorTable
            }
            SourceReadFootprintDerivation::NumericLookupTable => {
                PracticalResultSourceReadFootprintDerivation::NumericLookupTable
            }
            SourceReadFootprintDerivation::StaticTileStreams => {
                PracticalResultSourceReadFootprintDerivation::StaticTileStreams
            }
            SourceReadFootprintDerivation::DynamicConfigGraph => {
                PracticalResultSourceReadFootprintDerivation::DynamicConfigGraph
            }
        },
        read_population: match footprint.read_population {
            SourceProjectionReadPopulation::DeclaredRectangles {
                declared_rectangle_count,
                unique_rectangle_count,
            } => PracticalResultSourceReadPopulationBuild::DeclaredRectangles {
                declared_rectangle_count,
                unique_rectangle_count,
            },
            SourceProjectionReadPopulation::StaticDecodedStreams {
                decoded_stream_count,
            } => PracticalResultSourceReadPopulationBuild::StaticDecodedStreams {
                decoded_stream_count,
            },
            SourceProjectionReadPopulation::DynamicRootArrayTargets {
                pointer_target_count,
                unique_pointer_target_count,
                selector_referenced_unique_pointer_target_count,
            } => PracticalResultSourceReadPopulationBuild::DynamicRootArrayTargets {
                pointer_target_count,
                unique_pointer_target_count,
                selector_referenced_unique_pointer_target_count,
            },
        },
        tile_geometry: footprint.tile_geometry.map(|geometry| {
            PracticalResultSourceTileGeometryBuild {
                atlas_width: geometry.atlas_width,
                atlas_height: geometry.atlas_height,
                tile_width: geometry.tile_width,
                tile_height: geometry.tile_height,
            }
        }),
        decoded_tile_ids: footprint
            .decoded_tile_ids
            .as_ref()
            .map(|ids| ids.iter().copied().collect()),
        selector_referenced_tile_ids: footprint
            .selector_referenced_tile_ids
            .as_ref()
            .map(|ids| ids.iter().copied().collect()),
        structural_stream_source_offsets: footprint
            .structural_stream_source_offsets
            .as_ref()
            .map(|offsets| offsets.iter().copied().collect()),
        selector_referenced_stream_source_offsets: footprint
            .selector_referenced_stream_source_offsets
            .as_ref()
            .map(|offsets| offsets.iter().copied().collect()),
        dynamic_stream_state_headers: footprint.dynamic_stream_state_headers.as_ref().map(
            |headers| {
                headers
                    .iter()
                    .map(|header| PracticalResultDynamicStreamStateHeaderBuild {
                        source_offset: header.source_offset,
                        value: header.value,
                        selector_referenced: header.selector_referenced,
                    })
                    .collect()
            },
        ),
        source_read_rectangles: footprint
            .source_read_rectangles
            .iter()
            .copied()
            .map(source_atlas_rectangle_cell)
            .collect(),
        selector_referenced_rectangles: footprint.selector_referenced_rectangles.as_ref().map(
            |rectangles| {
                rectangles
                    .iter()
                    .copied()
                    .map(source_atlas_rectangle_cell)
                    .collect()
            },
        ),
        source_read_set_assessment: match footprint.source_read_set_assessment {
            SourceReadSetAssessment::InputSetUnassessed => {
                PracticalResultSourceReadSetAssessment::InputSetUnassessed
            }
            SourceReadSetAssessment::DeclaredReadSetComplete => {
                PracticalResultSourceReadSetAssessment::DeclaredReadSetComplete
            }
        },
        consumer_reachability: match footprint.consumer_reachability {
            SourceConsumerReachability::Unassessed => {
                PracticalResultSourceConsumerReachability::Unassessed
            }
            SourceConsumerReachability::RuntimeRootSelectionUnresolved => {
                PracticalResultSourceConsumerReachability::RuntimeRootSelectionUnresolved
            }
            SourceConsumerReachability::DormantOutsideDeclaredEntrypoints => {
                PracticalResultSourceConsumerReachability::DormantOutsideDeclaredEntrypoints
            }
            SourceConsumerReachability::Closed => PracticalResultSourceConsumerReachability::Closed,
        },
        physical_source_region_intersections,
        protected_source_region_intersections,
    })
}

fn build_source_region_intersection(
    region_id: String,
    region_cell: Cell,
    footprint: &SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>,
) -> Result<Option<PracticalResultSourceRegionIntersectionBuild>> {
    let source_read_intersection_cells =
        intersection_cells(&footprint.source_read_rectangles, region_cell)?;
    if source_read_intersection_cells.is_empty() {
        return Ok(None);
    }
    let decoded_overlapping_tile_ids = decoded_overlapping_tile_ids(footprint, region_cell)?;
    Ok(Some(PracticalResultSourceRegionIntersectionBuild {
        region_id,
        region_cell,
        decoded_overlapping_tile_ids,
        selector_referenced_overlapping_tile_ids: selector_referenced_overlapping_tile_ids(
            footprint,
            region_cell,
        )?,
        source_read_intersection_cells,
        selector_referenced_intersection_cells: footprint
            .selector_referenced_rectangles
            .as_ref()
            .map(|rectangles| intersection_cells(rectangles, region_cell))
            .transpose()?,
    }))
}

fn build_protected_source_region_intersection(
    region_id: String,
    protection_role: PracticalResultSourceProtectionRole,
    region_cell: Cell,
    footprint: &SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>,
) -> Result<Option<PracticalResultProtectedSourceRegionIntersectionBuild>> {
    let source_read_intersection_cells =
        intersection_cells(&footprint.source_read_rectangles, region_cell)?;
    if source_read_intersection_cells.is_empty() {
        return Ok(None);
    }
    let decoded_overlapping_tile_ids = decoded_overlapping_tile_ids(footprint, region_cell)?;
    Ok(Some(
        PracticalResultProtectedSourceRegionIntersectionBuild {
            region_id,
            protection_role,
            region_cell,
            decoded_overlapping_tile_ids,
            selector_referenced_overlapping_tile_ids: selector_referenced_overlapping_tile_ids(
                footprint,
                region_cell,
            )?,
            source_read_intersection_cells,
            selector_referenced_intersection_cells: footprint
                .selector_referenced_rectangles
                .as_ref()
                .map(|rectangles| intersection_cells(rectangles, region_cell))
                .transpose()?,
        },
    ))
}

fn decoded_overlapping_tile_ids(
    footprint: &SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>,
    region: Cell,
) -> Result<Option<Vec<usize>>> {
    footprint
        .decoded_tile_ids
        .as_ref()
        .map(|tile_ids| {
            overlapping_tile_ids(
                tile_ids,
                footprint
                    .tile_geometry
                    .context("tile source-read footprint has no tile geometry")?,
                region,
            )
        })
        .transpose()
}

fn selector_referenced_overlapping_tile_ids(
    footprint: &SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>,
    region: Cell,
) -> Result<Option<Vec<usize>>> {
    footprint
        .selector_referenced_tile_ids
        .as_ref()
        .map(|tile_ids| {
            overlapping_tile_ids(
                tile_ids,
                footprint
                    .tile_geometry
                    .context("selector-referenced tile footprint has no tile geometry")?,
                region,
            )
        })
        .transpose()
}

fn overlapping_tile_ids(
    tile_ids: &BTreeSet<usize>,
    geometry: SourceAtlasTileGeometry,
    region: Cell,
) -> Result<Vec<usize>> {
    ensure!(
        geometry.tile_width > 0 && geometry.atlas_width.is_multiple_of(geometry.tile_width),
        "source footprint lost its validated tile geometry"
    );
    let column_count = geometry.atlas_width / geometry.tile_width;
    let mut overlapping = Vec::new();
    for &tile_id in tile_ids {
        if cells_intersect(tile_cell(tile_id, column_count, geometry)?, region)? {
            overlapping.push(tile_id);
        }
    }
    Ok(overlapping)
}

fn tile_cell(
    tile_id: usize,
    column_count: usize,
    geometry: SourceAtlasTileGeometry,
) -> Result<Cell> {
    let x = (tile_id % column_count)
        .checked_mul(geometry.tile_width)
        .context("source tile x overflow")?;
    let y = (tile_id / column_count)
        .checked_mul(geometry.tile_height)
        .context("source tile y overflow")?;
    let cell = Cell {
        x,
        y,
        width: geometry.tile_width,
        height: geometry.tile_height,
    };
    ensure!(
        x.checked_add(cell.width)
            .is_some_and(|end| end <= geometry.atlas_width)
            && y.checked_add(cell.height)
                .is_some_and(|end| end <= geometry.atlas_height),
        "source footprint tile escapes its atlas"
    );
    Ok(cell)
}

fn intersection_cells(rectangles: &[SourceAtlasRectangle], region: Cell) -> Result<Vec<Cell>> {
    Ok(rectangles
        .iter()
        .copied()
        .map(|rectangle| intersect_cells(source_atlas_rectangle_cell(rectangle), region))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn intersect_cells(left: Cell, right: Cell) -> Result<Option<Cell>> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = left
        .x
        .checked_add(left.width)
        .context("left source-region x extent overflow")?
        .min(
            right
                .x
                .checked_add(right.width)
                .context("right source-region x extent overflow")?,
        );
    let bottom_edge = left
        .y
        .checked_add(left.height)
        .context("left source-region y extent overflow")?
        .min(
            right
                .y
                .checked_add(right.height)
                .context("right source-region y extent overflow")?,
        );
    if x >= right_edge || y >= bottom_edge {
        return Ok(None);
    }
    Ok(Some(Cell {
        x,
        y,
        width: right_edge - x,
        height: bottom_edge - y,
    }))
}

fn cells_intersect(left: Cell, right: Cell) -> Result<bool> {
    Ok(intersect_cells(left, right)?.is_some())
}

fn source_atlas_rectangle_cell(rectangle: SourceAtlasRectangle) -> Cell {
    Cell {
        x: rectangle.x,
        y: rectangle.y,
        width: rectangle.width,
        height: rectangle.height,
    }
}

#[cfg(test)]
#[path = "source_footprint_report_tests.rs"]
mod tests;
