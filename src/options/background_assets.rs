use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, parse_4bpp_prefix, read_indexed_cell_in_prefix};

use super::background_model::{
    LoadedOptionsBackgroundAssets, OptionsBackgroundFontRole, OptionsBackgroundLayout,
    OptionsBackgroundManifest, OptionsBackgroundTranslationUnit,
};
use super::menu_textures::EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET;
use super::model::{OptionsDevelopmentStatus, OptionsReleaseStatus};
use super::source::MENU_PATH;

const GRAPHICS_DIRECTORY: &str = "graphics";
const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 shared options-background translation manifest";
const UNIT_KIND: &str = "Justice Gakuen 2 shared options-background translation unit";

pub(super) fn load_options_background_assets(
    root: &Path,
    source_bin_sha256: &str,
    menu_stored: &[u8],
    menu_decoded: &[u8],
) -> Result<LoadedOptionsBackgroundAssets> {
    let graphics_root = root.join(GRAPHICS_DIRECTORY);
    let manifest_path = graphics_root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: OptionsBackgroundManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown options-background manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source_bin_sha256,
        "options-background source BIN changed"
    );
    ensure!(
        manifest.menu_path == MENU_PATH
            && manifest.menu_stored_sha256 == sha256_bytes(menu_stored)
            && manifest.menu_decoded_sha256 == sha256_bytes(menu_decoded),
        "options-background MENU.BIZ identity changed"
    );
    ensure!(
        manifest.tim_offset == EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET,
        "options-background TIM offset changed"
    );
    let tim = parse_4bpp_prefix(
        menu_decoded
            .get(manifest.tim_offset..)
            .context("options-background TIM offset is outside MENU.BIZ")?,
    )?;
    ensure!(
        tim.pixel_width() == manifest.pixel_width && tim.image_height == manifest.pixel_height,
        "options-background TIM geometry changed"
    );
    ensure!(
        manifest.runtime_consumers == ["options", "records"],
        "options-background runtime consumer binding changed"
    );
    ensure!(
        manifest.units.len() == 2,
        "options-background unit population changed"
    );

    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut occurrence_ids = BTreeSet::new();
    let mut cells = Vec::new();
    let mut units = Vec::with_capacity(manifest.units.len());
    for reference in &manifest.units {
        validate_relative_file(&reference.file)?;
        ensure!(
            ids.insert(reference.id.clone()),
            "duplicate options-background id"
        );
        ensure!(
            files.insert(reference.file.clone()),
            "duplicate options-background file"
        );
        let path = graphics_root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: OptionsBackgroundTranslationUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND && unit.id == reference.id,
            "options-background unit identity differs from its manifest reference"
        );
        validate_unit_state(&unit)?;
        for occurrence in &unit.occurrences {
            ensure!(
                occurrence_ids.insert(occurrence.id.clone()),
                "duplicate options-background occurrence id"
            );
            ensure!(
                cells
                    .iter()
                    .all(|cell| !cells_overlap(*cell, occurrence.cell)),
                "options-background editable regions overlap"
            );
            let source_pixels =
                read_indexed_cell_in_prefix(menu_decoded, manifest.tim_offset, occurrence.cell)?;
            ensure!(
                sha256_bytes(&source_pixels) == occurrence.source_indexed_pixel_sha256,
                "options-background {} source region changed",
                occurrence.id
            );
            cells.push(occurrence.cell);
        }
        units.push(unit);
    }
    ensure!(
        ids == ["crest_mark", "school_name"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        "options-background semantic unit set changed"
    );
    Ok(LoadedOptionsBackgroundAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        tim_offset: manifest.tim_offset,
        units,
    })
}

fn validate_unit_state(unit: &OptionsBackgroundTranslationUnit) -> Result<()> {
    ensure!(
        unit.development_status == OptionsDevelopmentStatus::Authored
            && unit.release_status != OptionsReleaseStatus::Untranslated,
        "options-background {} is not authored for development",
        unit.id
    );
    ensure!(
        !unit.source_text.trim().is_empty()
            && !unit.korean_text.trim().is_empty()
            && !unit.occurrences.is_empty(),
        "options-background {} has incomplete translation fields",
        unit.id
    );
    ensure!(
        unit.clear_index < 16
            && unit.outline_index < 16
            && unit.fill_index < 16
            && unit.clear_index != unit.outline_index
            && unit.clear_index != unit.fill_index
            && unit.outline_index != unit.fill_index,
        "options-background {} has invalid palette roles",
        unit.id
    );
    match unit.id.as_str() {
        "school_name" => ensure!(
            unit.source_text == "MINAMI"
                && unit.korean_text == unit.source_text
                && unit.font_role == OptionsBackgroundFontRole::SchoolName
                && unit.layout == OptionsBackgroundLayout::VerticalGlyphs
                && unit.occurrences.len() == 2,
            "school-name background semantics changed"
        ),
        "crest_mark" => ensure!(
            unit.source_text == "南"
                && unit.korean_text == "남"
                && unit.font_role == OptionsBackgroundFontRole::CrestMark
                && unit.layout == OptionsBackgroundLayout::Centered
                && unit.occurrences.len() == 1,
            "crest-mark background semantics changed"
        ),
        _ => anyhow::bail!("unknown options-background unit {}", unit.id),
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
        "options-background manifest contains an unsafe file path"
    );
    Ok(())
}
