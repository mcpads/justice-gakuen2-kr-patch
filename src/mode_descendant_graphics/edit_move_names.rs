use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, IndexedImage};

#[derive(Debug)]
pub(super) struct MoveNameTranslations {
    pub(super) sha256: String,
    pub(super) entries: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source_text: String,
    korean_lines: Vec<String>,
    status: DraftStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum DraftStatus {
    Draft,
}

pub(super) fn load_move_names(path: &Path) -> Result<MoveNameTranslations> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "justice_gakuen2_edit_move_name_translations",
        "unsupported EDIT move-name manifest"
    );
    let mut entries = BTreeMap::new();
    for entry in manifest.entries {
        let DraftStatus::Draft = entry.status;
        ensure!(
            !entry.source_text.trim().is_empty()
                && !entry.korean_lines.is_empty()
                && entry
                    .korean_lines
                    .iter()
                    .all(|line| !line.trim().is_empty()),
            "incomplete EDIT move-name translation"
        );
        ensure!(
            entries
                .insert(entry.source_text, entry.korean_lines)
                .is_none(),
            "duplicate EDIT move-name source text"
        );
    }
    Ok(MoveNameTranslations {
        sha256: sha256_bytes(&bytes),
        entries,
    })
}

pub(super) fn validate_move_cell(source_cell: Cell, render_cell: Cell) -> Result<()> {
    ensure!(
        render_cell.width > 0
            && render_cell.height > 0
            && render_cell.x <= source_cell.x
            && render_cell.y <= source_cell.y
            && render_cell
                .x
                .checked_add(render_cell.width)
                .is_some_and(|end| source_cell
                    .x
                    .checked_add(source_cell.width)
                    .is_some_and(|source_end| source_end <= end))
            && render_cell
                .y
                .checked_add(render_cell.height)
                .is_some_and(|end| source_cell
                    .y
                    .checked_add(source_cell.height)
                    .is_some_and(|source_end| source_end <= end)),
        "EDIT move-name render cell does not contain the complete source label"
    );
    Ok(())
}

pub(super) fn move_name_background(
    source: &IndexedImage,
    source_cell: Cell,
    render_cell: Cell,
    background_index: u8,
    palette: &[u16; 16],
) -> Result<Vec<u8>> {
    validate_move_cell(source_cell, render_cell)?;
    ensure!(
        source.pixels.len() == source.width * source.height
            && render_cell.x + render_cell.width <= source.width
            && render_cell.y + render_cell.height <= source.height,
        "EDIT move-name render cell leaves the source image"
    );
    ensure!(
        background_index < 16 && palette[usize::from(background_index)] & 0x7fff == 0x7fff,
        "EDIT move-name background is not source white"
    );
    for y in render_cell.y..render_cell.y + render_cell.height {
        for x in render_cell.x..render_cell.x + render_cell.width {
            let in_source = (source_cell.x..source_cell.x + source_cell.width).contains(&x)
                && (source_cell.y..source_cell.y + source_cell.height).contains(&y);
            ensure!(
                in_source || source.pixels[y * source.width + x] == background_index,
                "EDIT move-name extension would erase source pixels at ({x}, {y})"
            );
        }
    }
    Ok(vec![
        background_index;
        render_cell.width * render_cell.height
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_extension_requires_blank_source_and_preserves_the_input() {
        let cell = Cell {
            x: 2,
            y: 1,
            width: 2,
            height: 1,
        };
        let render = Cell {
            x: 1,
            y: 0,
            width: 4,
            height: 3,
        };
        let mut source = IndexedImage {
            width: 6,
            height: 3,
            pixels: vec![15; 18],
        };
        source.pixels[8] = 4;
        let mut palette = [0; 16];
        palette[15] = 0x7fff;
        assert_eq!(
            move_name_background(&source, cell, render, 15, &palette).unwrap(),
            vec![15; 12]
        );
        palette[15] = 0xffff;
        assert_eq!(
            move_name_background(&source, cell, render, 15, &palette).unwrap(),
            vec![15; 12]
        );
        assert_eq!(source.pixels[8], 4);
        source.pixels[10] = 1;
        assert!(
            move_name_background(&source, cell, render, 15, &palette)
                .unwrap_err()
                .to_string()
                .contains("erase source pixels")
        );
    }

    #[test]
    fn move_extension_rejects_cropped_source_and_wrong_palette() {
        let cell = Cell {
            x: 1,
            y: 1,
            width: 3,
            height: 1,
        };
        let source = IndexedImage {
            width: 6,
            height: 3,
            pixels: vec![15; 18],
        };
        let mut palette = [0; 16];
        palette[15] = 0x7fff;
        let cropped = Cell { x: 2, ..cell };
        assert!(move_name_background(&source, cell, cropped, 15, &palette).is_err());
        palette[15] = 0x0421;
        assert!(
            move_name_background(&source, cell, cell, 15, &palette)
                .unwrap_err()
                .to_string()
                .contains("source white")
        );
    }
}
