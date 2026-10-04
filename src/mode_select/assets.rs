use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, cells_overlap, read_indexed_cell_in_prefix, read_indexed_cell_without_clut_in_prefix,
};

use super::model::{ModeSelectFixedTranslation, ModeSelectManifest, ModeSelectTranslation};
use super::source::{
    ATLAS_TIM_OFFSET, DESCRIPTION_TIM_FIRST_OFFSET, DESCRIPTION_TIM_STRIDE, MENU_PATH,
    MENU_STORED_SHA256, MODE_COUNT, MODE_SELECT_OVERLAY_PATH, MODE_SELECT_OVERLAY_SHA256,
    ModeSelectSource, PANEL_INDEX_BY_MODE_INDEX,
};

const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 mode-select translation manifest";
const TRANSLATION_KIND: &str = "Justice Gakuen 2 mode-select translation unit";
const FIXED_TRANSLATION_KIND: &str = "Justice Gakuen 2 mode-select fixed translation unit";
const CURRENT_POINTS_ID: &str = "current_points";
const CURRENT_POINTS_CELL: Cell = Cell {
    x: 544,
    y: 224,
    width: 80,
    height: 24,
};
const LIST_DESCRIPTOR_FIRST_OFFSET: usize = 0x01ac;
const LIST_DESCRIPTOR_STRIDE: usize = 0x18;
const DETAIL_COORDINATE_FIRST_OFFSET: usize = 0x0508;
const DETAIL_COORDINATE_STRIDE: usize = 6;
const DETAIL_TITLE_SLOT_WIDTH: usize = 112;
const DETAIL_TITLE_SLOT_HEIGHT: usize = 24;

pub(super) struct LoadedModeSelectAssets {
    pub(super) manifest_sha256: String,
    pub(super) fixed_labels: Vec<ModeSelectFixedTranslation>,
    pub(super) translations: Vec<ModeSelectTranslation>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &ModeSelectSource,
) -> Result<LoadedModeSelectAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ModeSelectManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown mode-select manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256,
        "mode-select manifest source BIN changed"
    );
    ensure!(
        manifest.menu_path == MENU_PATH
            && manifest.menu_stored_sha256 == MENU_STORED_SHA256
            && manifest.menu_decoded_sha256 == sha256_bytes(&source.menu_decoded),
        "mode-select manifest MENU.BIZ identity changed"
    );
    ensure!(
        manifest.overlay_path == MODE_SELECT_OVERLAY_PATH
            && manifest.overlay_sha256 == MODE_SELECT_OVERLAY_SHA256,
        "mode-select manifest overlay identity changed"
    );
    ensure!(
        manifest.atlas_tim_offset == ATLAS_TIM_OFFSET,
        "mode-select atlas TIM offset changed"
    );
    ensure!(
        manifest.modes.len() == MODE_COUNT,
        "mode-select manifest must contain {MODE_COUNT} modes"
    );
    ensure!(
        manifest.fixed_labels.len() == 1,
        "mode-select manifest must contain the current-points label"
    );

    let mut fixed_ids = BTreeSet::new();
    let mut fixed_labels = Vec::with_capacity(manifest.fixed_labels.len());
    for reference in manifest.fixed_labels {
        ensure!(
            reference.id == CURRENT_POINTS_ID && fixed_ids.insert(reference.id.clone()),
            "unknown or duplicate MODE SELECT fixed label"
        );
        validate_relative_file(&reference.file)?;
        let path = root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let translation: ModeSelectFixedTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            translation.kind == FIXED_TRANSLATION_KIND && translation.id == reference.id,
            "MODE SELECT fixed-label identity changed"
        );
        ensure!(
            !translation.source_text.trim().is_empty()
                && !translation.korean_text.trim().is_empty()
                && translation.font_px.is_finite()
                && translation.font_px > 0.0,
            "MODE SELECT fixed label contains invalid text or layout"
        );
        ensure!(
            translation.region.tim_offset == ATLAS_TIM_OFFSET
                && translation.region.cell == CURRENT_POINTS_CELL,
            "MODE SELECT current-points consumer binding changed"
        );
        validate_region_hash(
            &source.menu_decoded,
            &translation.region,
            false,
            &translation.id,
        )?;
        fixed_labels.push(translation);
    }

    let mut mode_indices = BTreeSet::new();
    let mut panel_indices = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut translations = Vec::with_capacity(MODE_COUNT);
    for (position, reference) in manifest.modes.into_iter().enumerate() {
        ensure!(
            reference.mode_index == position && reference.mode_index < MODE_COUNT,
            "mode-select manifest order changed"
        );
        ensure!(
            reference.panel_index == PANEL_INDEX_BY_MODE_INDEX[reference.mode_index],
            "mode-select panel mapping changed for mode {}",
            reference.mode_index
        );
        ensure!(
            mode_indices.insert(reference.mode_index),
            "duplicate mode index"
        );
        ensure!(
            panel_indices.insert(reference.panel_index),
            "duplicate panel index"
        );
        ensure!(
            !reference.id.trim().is_empty() && ids.insert(reference.id.clone()),
            "duplicate or empty mode-select id"
        );
        validate_relative_file(&reference.file)?;
        let path = root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let translation: ModeSelectTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            translation.kind == TRANSLATION_KIND,
            "unknown translation unit kind"
        );
        ensure!(
            translation.id == reference.id
                && translation.mode_index == reference.mode_index
                && translation.panel_index == reference.panel_index,
            "mode-select unit identity differs from its manifest reference"
        );
        validate_translation(&translation, source)?;
        translations.push(translation);
    }
    validate_regions_disjoint(&translations, &fixed_labels)?;
    Ok(LoadedModeSelectAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        fixed_labels,
        translations,
    })
}

fn validate_translation(
    translation: &ModeSelectTranslation,
    source: &ModeSelectSource,
) -> Result<()> {
    ensure!(
        !translation.source_title.trim().is_empty()
            && !translation.korean_title.trim().is_empty()
            && !translation.source_description_lines.is_empty()
            && !translation.korean_description_lines.is_empty(),
        "mode-select {} contains empty text",
        translation.id
    );
    ensure!(
        translation
            .source_description_lines
            .iter()
            .all(|line| !line.trim().is_empty())
            && translation
                .korean_description_lines
                .iter()
                .all(|line| !line.trim().is_empty()),
        "mode-select {} contains a blank description line",
        translation.id
    );
    ensure!(
        translation.title_font_px.is_finite() && translation.title_font_px > 0.0,
        "mode-select {} has an invalid title font size",
        translation.id
    );
    ensure!(
        translation.description_font_px.is_finite()
            && translation.description_font_px > 0.0
            && translation.description_line_height > 0,
        "mode-select {} has an invalid description layout",
        translation.id
    );
    let expected_list = list_label_region(&source.overlay, translation.panel_index)?;
    let expected_detail = detail_title_region(&source.overlay, translation.panel_index)?;
    let expected_description = Cell {
        x: 0,
        y: 0,
        width: 176,
        height: 112,
    };
    let expected_description_offset =
        DESCRIPTION_TIM_FIRST_OFFSET + translation.panel_index * DESCRIPTION_TIM_STRIDE;
    ensure!(
        translation.list_label.tim_offset == ATLAS_TIM_OFFSET
            && translation.list_label.cell == expected_list,
        "mode-select {} list-label consumer binding changed",
        translation.id
    );
    ensure!(
        translation.detail_title.tim_offset == ATLAS_TIM_OFFSET
            && translation.detail_title.cell == expected_detail,
        "mode-select {} detail-title consumer binding changed",
        translation.id
    );
    ensure!(
        translation.description.tim_offset == expected_description_offset
            && translation.description.cell == expected_description,
        "mode-select {} description consumer binding changed",
        translation.id
    );
    validate_region_hash(
        &source.menu_decoded,
        &translation.list_label,
        false,
        &translation.id,
    )?;
    validate_region_hash(
        &source.menu_decoded,
        &translation.detail_title,
        false,
        &translation.id,
    )?;
    validate_region_hash(
        &source.menu_decoded,
        &translation.description,
        true,
        &translation.id,
    )?;
    Ok(())
}

fn validate_region_hash(
    decoded: &[u8],
    region: &super::model::BoundIndexedRegion,
    without_clut: bool,
    id: &str,
) -> Result<()> {
    let pixels = if without_clut {
        read_indexed_cell_without_clut_in_prefix(decoded, region.tim_offset, region.cell)?
    } else {
        read_indexed_cell_in_prefix(decoded, region.tim_offset, region.cell)?
    };
    let actual = sha256_bytes(&pixels);
    ensure!(
        actual == region.source_region_sha256,
        "mode-select {id} source pixel region changed: expected {}, got {actual}",
        region.source_region_sha256
    );
    Ok(())
}

fn list_label_region(overlay: &[u8], panel_index: usize) -> Result<Cell> {
    let offset = LIST_DESCRIPTOR_FIRST_OFFSET + panel_index * LIST_DESCRIPTOR_STRIDE;
    let values = read_u16_array::<12>(overlay, offset)?;
    ensure!(
        values[0] == 1
            && values[1] == 0
            && values[3] == 0x0100
            && values[4] == 0x0070
            && values[5] == 0x01e2
            && values[9] == 0x0020,
        "mode-select list descriptor {panel_index} changed"
    );
    let page_x = match values[2] {
        0x0240 => 256,
        0x0280 => 512,
        value => anyhow::bail!("unknown mode-select list texture page 0x{value:04x}"),
    };
    Ok(Cell {
        x: page_x + usize::from(values[6]),
        y: usize::from(values[7]),
        width: usize::from(values[8]),
        height: usize::from(values[9]),
    })
}

pub(super) fn selected_title_origin_x(overlay: &[u8], panel_index: usize) -> Result<i16> {
    let values = read_u16_array::<12>(
        overlay,
        LIST_DESCRIPTOR_FIRST_OFFSET + panel_index * LIST_DESCRIPTOR_STRIDE,
    )?;
    Ok(values[10] as i16)
}

pub(super) fn detail_title_region(overlay: &[u8], panel_index: usize) -> Result<Cell> {
    let values = read_u16_array::<3>(
        overlay,
        DETAIL_COORDINATE_FIRST_OFFSET + panel_index * DETAIL_COORDINATE_STRIDE,
    )?;
    let page_x = match values[0] {
        0x02c0 => 768,
        0x0300 => 1024,
        value => anyhow::bail!("unknown mode-select detail texture page 0x{value:04x}"),
    };
    Ok(Cell {
        x: page_x + usize::from(values[1]),
        y: usize::from(values[2]),
        width: DETAIL_TITLE_SLOT_WIDTH,
        height: DETAIL_TITLE_SLOT_HEIGHT,
    })
}

fn validate_regions_disjoint(
    translations: &[ModeSelectTranslation],
    fixed_labels: &[ModeSelectFixedTranslation],
) -> Result<()> {
    for (index, translation) in translations.iter().enumerate() {
        for previous in &translations[..index] {
            ensure!(
                !cells_overlap(translation.list_label.cell, previous.list_label.cell),
                "mode-select list-label regions overlap"
            );
            ensure!(
                !cells_overlap(translation.detail_title.cell, previous.detail_title.cell),
                "mode-select detail-title regions overlap"
            );
        }
    }
    for fixed in fixed_labels {
        for translation in translations {
            ensure!(
                !cells_overlap(fixed.region.cell, translation.list_label.cell)
                    && !cells_overlap(fixed.region.cell, translation.detail_title.cell),
                "MODE SELECT fixed and mode-specific regions overlap"
            );
        }
    }
    Ok(())
}

fn validate_relative_file(file: &str) -> Result<()> {
    let path = Path::new(file);
    ensure!(
        !file.is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "mode-select manifest contains an unsafe file path"
    );
    Ok(())
}

fn read_u16_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u16; N]> {
    ensure!(offset + N * 2 <= data.len(), "truncated mode-select table");
    Ok(std::array::from_fn(|index| {
        u16::from_le_bytes([data[offset + index * 2], data[offset + index * 2 + 1]])
    }))
}
