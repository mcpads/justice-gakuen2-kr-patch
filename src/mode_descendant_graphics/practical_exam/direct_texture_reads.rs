//! Adopted shared-texture read footprints for the supported practical overlays.
//!
//! Production validates the complete overlay identity and reads the fixed UV
//! domain established by the offline census. It does not rediscover renderer
//! calls, packet flows, or selector provenance.

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::Cell;

use super::{PracticalExamConsumer, PracticalExamTextureRegion};

const TEXTURE_WIDTH: usize = 768;
const TEXTURE_HEIGHT: usize = 256;
const SELECTOR_ENTRY_COUNT: usize = 255;
const SELECTOR_ENTRY_STRIDE: usize = 3;
const SHARED_PAGE_14_X: usize = 512;
const SELECTOR_SPRITE_WIDTH: usize = 56;
const SELECTOR_SPRITE_HEIGHT: usize = 32;

const BASICS_SELECTOR_TABLE_OFFSET: usize = 0x0084;
const EXAM_1999_SELECTOR_TABLE_OFFSET: usize = 0x0138;

pub(super) struct PracticalExamDirectTextureReads {
    pub(super) regions: Vec<PracticalExamTextureRegion>,
}

pub(super) fn load_direct_texture_reads(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    expected_source_sha256: &str,
) -> Result<PracticalExamDirectTextureReads> {
    ensure!(
        sha256_bytes(overlay) == expected_source_sha256,
        "{consumer:?} practical-exam direct-texture source changed"
    );

    let (fixed_region, selector_table_offset, selector_role) = match consumer {
        PracticalExamConsumer::BasicsReview => (
            PracticalExamTextureRegion {
                id: "siken-p12-direct-numeric-byte-domain",
                cell: Cell {
                    x: 0,
                    y: 0,
                    width: 256,
                    height: 20,
                },
            },
            BASICS_SELECTOR_TABLE_OFFSET,
            "siken-p14-nonzero-byte-selector-domain",
        ),
        PracticalExamConsumer::Exam1999 => (
            PracticalExamTextureRegion {
                id: "siken2-p12-external-digit-selector-page-domain",
                cell: Cell {
                    x: 0,
                    y: 0,
                    width: 256,
                    height: 256,
                },
            },
            EXAM_1999_SELECTOR_TABLE_OFFSET,
            "siken2-p14-nonzero-byte-selector-domain",
        ),
    };

    let mut regions = vec![fixed_region];
    regions.extend(
        selector_cells(overlay, selector_table_offset)?
            .into_iter()
            .map(|cell| PracticalExamTextureRegion {
                id: selector_role,
                cell,
            }),
    );
    ensure!(
        regions.iter().all(|region| {
            region.cell.width > 0
                && region.cell.height > 0
                && region.cell.x + region.cell.width <= TEXTURE_WIDTH
                && region.cell.y + region.cell.height <= TEXTURE_HEIGHT
        }),
        "{consumer:?} practical-exam direct-texture reads escaped the shared texture"
    );
    Ok(PracticalExamDirectTextureReads { regions })
}

fn selector_cells(overlay: &[u8], table_offset: usize) -> Result<Vec<Cell>> {
    let table_size = SELECTOR_ENTRY_COUNT
        .checked_mul(SELECTOR_ENTRY_STRIDE)
        .context("practical-exam selector-table size overflow")?;
    let table_end = table_offset
        .checked_add(table_size)
        .context("practical-exam selector-table range overflow")?;
    let table = overlay
        .get(table_offset..table_end)
        .context("practical-exam selector-table is truncated")?;

    let mut cells = Vec::new();
    for entry in table.as_chunks::<SELECTOR_ENTRY_STRIDE>().0 {
        for cell in wrapped_texture_cells(usize::from(entry[0]), usize::from(entry[1])) {
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }
    ensure!(
        !cells.is_empty(),
        "practical-exam selector table produced no protected cells"
    );
    Ok(cells)
}

fn wrapped_texture_cells(u: usize, v: usize) -> Vec<Cell> {
    let horizontal = if u + SELECTOR_SPRITE_WIDTH <= 256 {
        vec![(u, SELECTOR_SPRITE_WIDTH)]
    } else {
        vec![(u, 256 - u), (0, u + SELECTOR_SPRITE_WIDTH - 256)]
    };
    let vertical = if v + SELECTOR_SPRITE_HEIGHT <= 256 {
        vec![(v, SELECTOR_SPRITE_HEIGHT)]
    } else {
        vec![(v, 256 - v), (0, v + SELECTOR_SPRITE_HEIGHT - 256)]
    };
    horizontal
        .into_iter()
        .flat_map(|(x, width)| {
            vertical.iter().copied().map(move |(y, height)| Cell {
                x: SHARED_PAGE_14_X + x,
                y,
                width,
                height,
            })
        })
        .collect()
}
