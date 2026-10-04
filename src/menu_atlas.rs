use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::tim::Cell;

#[path = "menu_atlas/native_text.rs"]
pub(crate) mod native_text;
#[path = "menu_atlas/presentation.rs"]
pub(crate) mod presentation;
#[path = "menu_atlas/source.rs"]
mod source;

pub(crate) use source::{MENU_ATLAS_RECORD_PATH, load_source_menu_atlas};

pub const MENU_ATLAS_PAGE_WIDTH: usize = 256;
pub const MENU_ATLAS_HEIGHT: usize = 256;
pub const MENU_ATLAS_PAGE_COUNT: usize = 4;
pub const MENU_ATLAS_WIDTH: usize = MENU_ATLAS_PAGE_WIDTH * MENU_ATLAS_PAGE_COUNT;
pub const MENU_GLYPH_CELL_WIDTH: usize = 20;
pub const MENU_GLYPH_CELL_HEIGHT: usize = 20;
pub const MENU_GLYPH_CODE_COUNT: usize = 0x0400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MenuAtlasPosition {
    pub page: u8,
    pub column: u8,
    pub row: u8,
    pub x: usize,
    pub y: usize,
}

pub fn logical_code_position(code: u16) -> Result<MenuAtlasPosition> {
    ensure!(
        usize::from(code) < MENU_GLYPH_CODE_COUNT,
        "unsupported menu glyph code: 0x{code:04x}"
    );
    let page = (code >> 8) as u8;
    let column = (code & 0x000f) as u8;
    let row = ((code >> 4) & 0x000f) as u8;
    let page_x = (usize::from(column) * MENU_GLYPH_CELL_WIDTH) & 0xff;
    let y = (usize::from(row) * MENU_GLYPH_CELL_HEIGHT) & 0xff;
    Ok(MenuAtlasPosition {
        page,
        column,
        row,
        x: usize::from(page) * MENU_ATLAS_PAGE_WIDTH + page_x,
        y,
    })
}

pub(crate) fn parse_menu_code(code: &str) -> Result<u16> {
    ensure!(
        code.len() == 6 && code.starts_with("0x"),
        "menu code must use 0x0000 form: {code}"
    );
    let value = u16::from_str_radix(&code[2..], 16)
        .with_context(|| format!("invalid menu code: {code}"))?;
    ensure!(
        usize::from(value) < MENU_GLYPH_CODE_COUNT,
        "menu code is outside 0x0000..0x03ff: {code}"
    );
    Ok(value)
}

pub fn logical_code_fragments(code: u16, width: usize, height: usize) -> Result<Vec<Cell>> {
    ensure!(
        (1..=MENU_ATLAS_PAGE_WIDTH).contains(&width),
        "menu atlas footprint width must be within 1..=256"
    );
    ensure!(
        (1..=MENU_ATLAS_HEIGHT).contains(&height),
        "menu atlas footprint height must be within 1..=256"
    );
    let position = logical_code_position(code)?;
    let page_x = position.x % MENU_ATLAS_PAGE_WIDTH;
    let page_base_x = usize::from(position.page) * MENU_ATLAS_PAGE_WIDTH;
    let x_segments = wrapped_axis_segments(page_x, width);
    let y_segments = wrapped_axis_segments(position.y, height);
    let mut fragments = Vec::with_capacity(4);
    for (y, fragment_height) in y_segments {
        if fragment_height == 0 {
            continue;
        }
        for (x, fragment_width) in x_segments {
            if fragment_width == 0 {
                continue;
            }
            fragments.push(Cell {
                x: page_base_x + x,
                y,
                width: fragment_width,
                height: fragment_height,
            });
        }
    }
    Ok(fragments)
}

pub fn require_proven_shared_atlas_writes(
    owner: &str,
    global_write_count: usize,
    allocation_proven_reclaimable: bool,
) -> Result<()> {
    if global_write_count > 0 && !allocation_proven_reclaimable {
        bail!(
            "refusing to compose {global_write_count} globally resident {owner} writes into shared MENU.BIZ: reclaimability is not proven; finish whole-consumer migration or use a proven context-scoped upload"
        );
    }
    Ok(())
}

pub(crate) fn logical_regions_overlap(
    left_code: u16,
    left_width: usize,
    left_height: usize,
    right_code: u16,
    right_width: usize,
    right_height: usize,
) -> bool {
    let left = logical_code_fragments(left_code, left_width, left_height)
        .expect("internal menu-atlas footprint is valid");
    let right = logical_code_fragments(right_code, right_width, right_height)
        .expect("internal menu-atlas footprint is valid");
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.x < right.x + right.width
                && right.x < left.x + left.width
                && left.y < right.y + right.height
                && right.y < left.y + left.height
        })
    })
}

fn wrapped_axis_segments(start: usize, size: usize) -> [(usize, usize); 2] {
    let end = start + size;
    if end <= MENU_ATLAS_PAGE_WIDTH {
        [(start, size), (0, 0)]
    } else {
        [
            (start, MENU_ATLAS_PAGE_WIDTH - start),
            (0, end - MENU_ATLAS_PAGE_WIDTH),
        ]
    }
}

#[cfg(test)]
#[path = "menu_atlas_tests.rs"]
mod tests;
