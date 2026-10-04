use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, IndexedImage, read_4bpp_indexed_image_in_prefix};

use super::model::{TitleGraphicUnit, TitleGraphicsManifest};
use super::source::{
    PALETTE_INDEX, SOURCE_DECODED_SHA256, SOURCE_PATH, SOURCE_PIXEL_SHA256, SOURCE_STORED_SHA256,
    TIM_DECODED_SIZE, TIM_OFFSET, TitleGraphicsSource,
};

const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "justice_gakuen2_title_graphics_manifest";
const UNIT_KIND: &str = "justice_gakuen2_title_graphic_unit";
const EXPECTED_UNITS: [(&str, &str, Cell, &str); 2] = [
    (
        "franchise_logo_large",
        "title_adjacent_large_logo",
        Cell {
            x: 513,
            y: 1,
            width: 250,
            height: 126,
        },
        "57f26190969f96366bdf06780db23863414513ab4084ea31395eeb80a1018083",
    ),
    (
        "franchise_logo_small",
        "title_adjacent_small_logo",
        Cell {
            x: 517,
            y: 137,
            width: 85,
            height: 62,
        },
        "f22799d0b4c9209b5350faab4354f887bc665e16f511a4ea5908b73520b0eca0",
    ),
];

pub(super) struct LoadedTitleGraphicsAssets {
    pub(super) manifest_sha256: String,
    pub(super) asset_set_sha256: String,
    pub(super) units: Vec<TitleGraphicUnit>,
}

pub(super) fn load_title_graphics_assets(
    root: &Path,
    source: &TitleGraphicsSource,
) -> Result<LoadedTitleGraphicsAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: TitleGraphicsManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;
    let mut asset_set_bytes = Vec::new();
    append_bound_file(&mut asset_set_bytes, MANIFEST_FILE, &manifest_bytes)?;

    let indexed = read_4bpp_indexed_image_in_prefix(&source.decoded, TIM_OFFSET)?;
    let mut files = BTreeSet::new();
    let mut units = Vec::with_capacity(EXPECTED_UNITS.len());
    for (entry, (expected_id, runtime_role, expected_cell, expected_hash)) in
        manifest.units.iter().zip(EXPECTED_UNITS)
    {
        ensure!(
            entry.id == expected_id,
            "title graphic unit order or identity changed"
        );
        validate_relative_file(&entry.file)?;
        ensure!(
            files.insert(entry.file.clone()),
            "duplicate title graphic asset file"
        );
        let path = root.join(&entry.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        append_bound_file(&mut asset_set_bytes, &entry.file, &bytes)?;
        let unit: TitleGraphicUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND
                && unit.id == expected_id
                && unit.runtime_role == runtime_role
                && unit.cell == expected_cell
                && unit.source_indexed_pixel_sha256 == expected_hash,
            "title graphic {expected_id} source binding changed"
        );
        validate_unit(&unit, &indexed)?;
        units.push(unit);
    }
    ensure!(
        units.len() == EXPECTED_UNITS.len(),
        "title graphics manifest must contain both franchise logo assets"
    );
    ensure!(
        cells_are_disjoint(units.iter().map(|unit| unit.cell)),
        "title graphic source cells overlap"
    );
    Ok(LoadedTitleGraphicsAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        asset_set_sha256: sha256_bytes(&asset_set_bytes),
        units,
    })
}

fn append_bound_file(output: &mut Vec<u8>, path: &str, bytes: &[u8]) -> Result<()> {
    output.extend_from_slice(&u64::try_from(path.len())?.to_le_bytes());
    output.extend_from_slice(path.as_bytes());
    output.extend_from_slice(&u64::try_from(bytes.len())?.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn validate_manifest(manifest: &TitleGraphicsManifest, source: &TitleGraphicsSource) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown title graphics manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256
            && manifest.source_path == SOURCE_PATH
            && manifest.source_stored_sha256 == SOURCE_STORED_SHA256
            && manifest.source_stored_sha256 == sha256_bytes(&source.stored)
            && manifest.source_decoded_sha256 == SOURCE_DECODED_SHA256
            && manifest.source_decoded_sha256 == sha256_bytes(&source.decoded),
        "title graphics source identity changed"
    );
    ensure!(
        manifest.tim_offset == TIM_OFFSET
            && manifest.tim_decoded_size == TIM_DECODED_SIZE
            && manifest.source_pixel_sha256 == SOURCE_PIXEL_SHA256
            && manifest.palette_index == PALETTE_INDEX
            && manifest.pixel_width == source.tim.pixel_width()
            && manifest.pixel_height == source.tim.image_height
            && manifest.image_vram_word_x == source.tim.image_x
            && manifest.image_vram_y == source.tim.image_y
            && manifest.clut_vram_x == source.tim.clut_x
            && manifest.clut_vram_y == source.tim.clut_y,
        "title graphics TIM binding changed"
    );
    ensure!(
        manifest.units.len() == EXPECTED_UNITS.len(),
        "title graphics manifest unit count changed"
    );
    Ok(())
}

fn validate_unit(unit: &TitleGraphicUnit, indexed: &IndexedImage) -> Result<()> {
    ensure!(
        !unit.source_label.trim().is_empty(),
        "title graphic {} has no source label",
        unit.id
    );
    ensure!(
        (unit.korean_text.is_none()
            && unit.artwork.is_none()
            && unit.development_status == "untranslated"
            && unit.release_status == "untranslated")
            || (unit
                .korean_text
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty())
                && unit.artwork.is_some()
                && unit.development_status == "authored"
                && unit.release_status == "pending_review"),
        "title graphic {} has inconsistent authoring status",
        unit.id
    );
    ensure!(
        unit.clear_index < 16,
        "title graphic clear index is not 4-bpp"
    );
    let pixels = read_cell(indexed, unit.cell)?;
    ensure!(
        sha256_bytes(&pixels) == unit.source_indexed_pixel_sha256,
        "title graphic {} source pixels changed",
        unit.id
    );
    ensure_tight_non_clear_bounds(indexed, unit.cell, unit.clear_index)
}

fn read_cell(indexed: &IndexedImage, cell: Cell) -> Result<Vec<u8>> {
    ensure!(
        cell.width > 0
            && cell.height > 0
            && cell.x + cell.width <= indexed.width
            && cell.y + cell.height <= indexed.height,
        "title graphic cell is outside its source texture"
    );
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        let start = y * indexed.width + cell.x;
        pixels.extend_from_slice(&indexed.pixels[start..start + cell.width]);
    }
    Ok(pixels)
}

pub(super) fn ensure_tight_non_clear_bounds(
    indexed: &IndexedImage,
    cell: Cell,
    clear_index: u8,
) -> Result<()> {
    let pixels = read_cell(indexed, cell)?;
    let top_has_ink = pixels[..cell.width]
        .iter()
        .any(|pixel| *pixel != clear_index);
    let bottom = (cell.height - 1) * cell.width;
    let bottom_has_ink = pixels[bottom..].iter().any(|pixel| *pixel != clear_index);
    let left_has_ink = (0..cell.height).any(|y| pixels[y * cell.width] != clear_index);
    let right_has_ink =
        (0..cell.height).any(|y| pixels[y * cell.width + cell.width - 1] != clear_index);
    ensure!(
        top_has_ink && bottom_has_ink && left_has_ink && right_has_ink,
        "title graphic cell is not the tight source-pixel boundary"
    );
    Ok(())
}

fn cells_are_disjoint(cells: impl Iterator<Item = Cell>) -> bool {
    let cells = cells.collect::<Vec<_>>();
    cells.iter().enumerate().all(|(index, left)| {
        cells[index + 1..].iter().all(|right| {
            left.x + left.width <= right.x
                || right.x + right.width <= left.x
                || left.y + left.height <= right.y
                || right.y + right.height <= left.y
        })
    })
}

fn validate_relative_file(path: &str) -> Result<()> {
    let path = Path::new(path);
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "title graphic asset path must be a plain relative path"
    );
    Ok(())
}
