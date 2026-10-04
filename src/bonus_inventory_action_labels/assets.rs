use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::model::{ActionLabelManifest, ActionLabelUnit, DevelopmentStatus, ReleaseStatus};
use super::overlay::{ACTION_SEQUENCES, POINTER_TABLE_OFFSET, SECONDARY_PARSER_RUNTIME_ADDRESS};
use super::source::{
    ACTION_GLYPHS, BONUS_INVENTORY_ACTION_LABEL_INVENTORY_PATH,
    BONUS_INVENTORY_ACTION_LABEL_OVERLAY_PATH, BonusInventoryActionLabelSource, GLYPH_TIM_OFFSET,
    INVENTORY_SOURCE_DECODED_SHA256, INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256,
    SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_inventory_action_label_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_inventory_action_label_unit";
const RUNTIME_CONSUMER: &str = "bonus_inventory_category_action_labels";

pub(super) struct LoadedActionLabelAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256s: Vec<String>,
    pub(super) runtime_consumer: String,
    pub(super) units: Vec<ActionLabelUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusInventoryActionLabelSource,
) -> Result<LoadedActionLabelAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ActionLabelManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(ACTION_SEQUENCES.len());
    let mut unit_sha256s = Vec::with_capacity(ACTION_SEQUENCES.len());
    for (relative, spec) in manifest.units.iter().zip(ACTION_SEQUENCES) {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate action-label unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: ActionLabelUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            ids.insert(unit.id.clone()),
            "duplicate action-label unit id"
        );
        validate_unit(&unit, spec, &source.overlay)?;
        unit_sha256s.push(sha256_bytes(&bytes));
        units.push(unit);
    }

    Ok(LoadedActionLabelAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256s,
        runtime_consumer: manifest.runtime_consumer,
        units,
    })
}

fn validate_manifest(
    manifest: &ActionLabelManifest,
    source: &BonusInventoryActionLabelSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported action-label manifest kind"
    );
    ensure!(
        manifest.source.inventory_path == BONUS_INVENTORY_ACTION_LABEL_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_INVENTORY_ACTION_LABEL_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256
            && parse_hex_usize(&manifest.source.pointer_table_offset)? == POINTER_TABLE_OFFSET
            && parse_hex_u32(&manifest.source.secondary_parser_runtime_address)?
                == SECONDARY_PARSER_RUNTIME_ADDRESS,
        "action-label manifest source binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER,
        "action-label runtime consumer changed"
    );
    ensure!(
        manifest.glyphs.len() == ACTION_GLYPHS.len()
            && manifest.units.len() == ACTION_SEQUENCES.len(),
        "action-label asset population changed"
    );
    let mut cells = Vec::new();
    for (asset, (text, code, cell)) in manifest.glyphs.iter().zip(ACTION_GLYPHS) {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "action-label glyph allocation changed for {text}"
        );
        ensure!(
            cells.iter().all(|other| !cells_overlap(*other, cell)),
            "action-label glyph cells overlap"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "action-label source glyph cell changed for {text}"
        );
        cells.push(cell);
    }
    Ok(())
}

fn validate_unit(
    unit: &ActionLabelUnit,
    spec: super::overlay::ActionSequenceSpec,
    overlay: &[u8],
) -> Result<()> {
    ensure!(
        unit.kind == UNIT_KIND && unit.id == spec.id && unit.source_text == spec.source_text,
        "action-label source meaning changed for {}",
        unit.id
    );
    ensure!(
        parse_hex_usize(&unit.source.sequence_offset)? == spec.sequence_offset
            && parse_hex_usize(&unit.source.terminator_offset)? == spec.terminator_offset
            && parse_hex_usize(&unit.source.pointer_storage_offset)? == spec.pointer_storage_offset,
        "action-label sequence binding changed for {}",
        unit.id
    );
    let source_bytes = overlay
        .get(spec.sequence_offset..=spec.terminator_offset)
        .context("action-label source sequence is truncated")?;
    ensure!(
        unit.source.encoded_sha256 == sha256_bytes(source_bytes),
        "action-label source sequence hash changed for {}",
        unit.id
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated
            && unit
                .korean_text
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty()),
        "action-label unit {} lacks authored development input",
        unit.id
    );
    Ok(())
}

fn parse_hex_usize(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("expected hexadecimal offset")?,
        16,
    )
    .context("invalid hexadecimal offset")
}

fn parse_hex_u32(value: &str) -> Result<u32> {
    u32::from_str_radix(
        value
            .strip_prefix("0x")
            .context("expected hexadecimal address")?,
        16,
    )
    .context("invalid hexadecimal address")
}

fn parse_hex_u16(value: &str) -> Result<u16> {
    u16::from_str_radix(
        value
            .strip_prefix("0x")
            .context("expected hexadecimal code")?,
        16,
    )
    .context("invalid hexadecimal code")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "action-label unit path must stay inside its asset directory"
    );
    Ok(())
}
