use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::consumer::{
    CATEGORY_HANDLER_RUNTIME_ADDRESS, CATEGORY_JUMP_TABLE_OFFSET, SCREEN_POSITIONS,
    SELECTOR_INSTRUCTION_OFFSETS, SOURCE_GLYPH_CODES, SPRITE_SIZE,
    STOCK_LABEL_RENDERER_RUNTIME_ADDRESS, TEXTURE_PAGE_INSTRUCTION_OFFSET,
    selector_instruction_sha256_input,
};
use super::model::{DevelopmentStatus, ReleaseStatus, StockLabelManifest, StockLabelUnit};
use super::source::{
    BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH, BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH,
    BonusInventoryStockLabelSource, GLYPH_TIM_OFFSET, INVENTORY_SOURCE_DECODED_SHA256,
    INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
    STOCK_LABEL_GLYPHS,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_inventory_stock_label_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_inventory_stock_label_unit";
const RUNTIME_CONSUMER: &str = "bonus_inventory_category_stock_count_header";
const FONT_ROLE: &str = "stock_label";
const UNIT_ID: &str = "stock_label";
const SOURCE_TEXT: &str = "所持";
const SOURCE_SELECTOR_INSTRUCTION_SHA256: &str =
    "d72077e33c07f0ac1504a5598ae5dd3a3db19a9cc2a1a25c17c85925d29f22b2";

pub(super) struct LoadedStockLabelAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256: String,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) unit: StockLabelUnit,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusInventoryStockLabelSource,
) -> Result<LoadedStockLabelAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: StockLabelManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let relative = manifest
        .units
        .first()
        .context("stock-label unit path disappeared")?;
    ensure_relative_path(relative)?;
    let unit_path = root.join(relative);
    let unit_bytes = std::fs::read(&unit_path)
        .with_context(|| format!("failed to read {}", unit_path.display()))?;
    let unit: StockLabelUnit = serde_json::from_slice(&unit_bytes)
        .with_context(|| format!("failed to parse {}", unit_path.display()))?;
    validate_unit(&unit, &source.overlay)?;

    Ok(LoadedStockLabelAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256: sha256_bytes(&unit_bytes),
        runtime_consumer: manifest.runtime_consumer,
        font_role: manifest.font_role,
        unit,
    })
}

fn validate_manifest(
    manifest: &StockLabelManifest,
    source: &BonusInventoryStockLabelSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported stock-label manifest kind"
    );
    let selector_offsets = manifest
        .source
        .selector_instruction_offsets
        .iter()
        .map(|value| parse_hex_usize(value))
        .collect::<Result<Vec<_>>>()?;
    let expected_selector_offsets = SELECTOR_INSTRUCTION_OFFSETS
        .into_iter()
        .chain([TEXTURE_PAGE_INSTRUCTION_OFFSET])
        .collect::<Vec<_>>();
    ensure!(
        manifest.source.inventory_path == BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256
            && parse_hex_u32(&manifest.source.renderer_runtime_address)?
                == STOCK_LABEL_RENDERER_RUNTIME_ADDRESS
            && parse_hex_u32(&manifest.source.category_handler_runtime_address)?
                == CATEGORY_HANDLER_RUNTIME_ADDRESS
            && parse_hex_usize(&manifest.source.category_jump_table_offset)?
                == CATEGORY_JUMP_TABLE_OFFSET
            && selector_offsets == expected_selector_offsets,
        "stock-label manifest source binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER && manifest.font_role == FONT_ROLE,
        "stock-label runtime consumer or font role changed"
    );
    ensure!(
        manifest.glyphs.len() == STOCK_LABEL_GLYPHS.len() && manifest.units.len() == 1,
        "stock-label asset population changed"
    );
    let mut cells = Vec::new();
    for (asset, (text, code, cell)) in manifest.glyphs.iter().zip(STOCK_LABEL_GLYPHS) {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "stock-label glyph allocation changed for {text}"
        );
        ensure!(
            cells.iter().all(|other| !cells_overlap(*other, cell)),
            "stock-label glyph cells overlap"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "stock-label source glyph cell changed for {text}"
        );
        cells.push(cell);
    }
    Ok(())
}

pub(super) fn validate_unit(unit: &StockLabelUnit, overlay: &[u8]) -> Result<()> {
    ensure!(
        unit.kind == UNIT_KIND && unit.id == UNIT_ID && unit.source_text == SOURCE_TEXT,
        "stock-label source meaning changed"
    );
    let glyph_codes = unit
        .source
        .glyph_codes
        .iter()
        .map(|value| parse_hex_u16(value))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        glyph_codes == SOURCE_GLYPH_CODES
            && unit.source.screen_positions == SCREEN_POSITIONS
            && unit.source.sprite_size == SPRITE_SIZE,
        "stock-label source consumer binding changed"
    );
    let selector_sha256 = sha256_bytes(&selector_instruction_sha256_input(overlay)?);
    ensure!(
        selector_sha256 == SOURCE_SELECTOR_INSTRUCTION_SHA256
            && unit.source.selector_instruction_sha256 == selector_sha256,
        "stock-label source selector instructions changed"
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated
            && unit.korean_text.as_deref() == Some("소지"),
        "stock-label unit lacks the authored Korean label"
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
    u32::try_from(parse_hex_usize(value)?).context("hexadecimal value exceeds u32")
}

fn parse_hex_u16(value: &str) -> Result<u16> {
    u16::try_from(parse_hex_usize(value)?).context("hexadecimal value exceeds u16")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "stock-label unit path must stay inside its asset directory"
    );
    Ok(())
}
