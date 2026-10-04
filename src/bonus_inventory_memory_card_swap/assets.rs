use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::command_sequences::{MEMORY_CARD_SWAP_SEQUENCES, MemoryCardSwapSequenceSpec};
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, COMMAND_RENDERER_RUNTIME_ADDRESS, OVERLAY_RUNTIME_BASE,
    POINTER_TABLE_END, POINTER_TABLE_OFFSET,
};
use super::glyph_slots::{
    MEMORY_CARD_SWAP_GLYPHS, REUSED_SOURCE_GLYPHS, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{DevelopmentStatus, MemoryCardSwapManifest, MemoryCardSwapUnit, ReleaseStatus};
use super::source::{
    BONUS_INVENTORY_MEMORY_CARD_SWAP_INVENTORY_PATH, BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_PATH,
    BonusInventoryMemoryCardSwapSource, GLYPH_TIM_OFFSET, INVENTORY_SOURCE_DECODED_SHA256,
    INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_inventory_memory_card_swap_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_inventory_memory_card_swap_unit";
const RUNTIME_CONSUMER: &str = "bonus_inventory_memory_card_swap_prompt";
const FONT_ROLE: &str = "bonus_memory_card_swap";

pub(super) struct LoadedMemoryCardSwapAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256s: Vec<String>,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) units: Vec<MemoryCardSwapUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusInventoryMemoryCardSwapSource,
) -> Result<LoadedMemoryCardSwapAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: MemoryCardSwapManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(MEMORY_CARD_SWAP_SEQUENCES.len());
    let mut unit_sha256s = Vec::with_capacity(MEMORY_CARD_SWAP_SEQUENCES.len());
    for (relative, spec) in manifest.units.iter().zip(MEMORY_CARD_SWAP_SEQUENCES) {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate memory-card-swap unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: MemoryCardSwapUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            ids.insert(unit.id.clone()),
            "duplicate memory-card-swap unit id"
        );
        validate_unit(&unit, spec, &source.overlay)?;
        unit_sha256s.push(sha256_bytes(&bytes));
        units.push(unit);
    }

    Ok(LoadedMemoryCardSwapAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256s,
        runtime_consumer: manifest.runtime_consumer,
        font_role: manifest.font_role,
        units,
    })
}

fn validate_manifest(
    manifest: &MemoryCardSwapManifest,
    source: &BonusInventoryMemoryCardSwapSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported memory-card-swap manifest kind"
    );
    ensure!(
        manifest.source.inventory_path == BONUS_INVENTORY_MEMORY_CARD_SWAP_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256,
        "memory-card-swap manifest media binding changed"
    );
    let pointer_offsets = manifest
        .source
        .pointer_storage_offsets
        .iter()
        .map(|offset| parse_hex_usize(offset))
        .collect::<Result<Vec<_>>>()?;
    let expected_pointer_offsets = MEMORY_CARD_SWAP_SEQUENCES
        .map(|spec| spec.pointer_storage_offset)
        .to_vec();
    let callers = manifest
        .source
        .caller_runtime_addresses
        .iter()
        .map(|address| parse_hex_u32(address))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        parse_hex_u32(&manifest.source.overlay_runtime_base)? == OVERLAY_RUNTIME_BASE
            && parse_hex_usize(&manifest.source.pointer_table_offset)? == POINTER_TABLE_OFFSET
            && parse_hex_usize(&manifest.source.pointer_table_end)? == POINTER_TABLE_END
            && parse_hex_u32(&manifest.source.command_renderer_runtime_address)?
                == COMMAND_RENDERER_RUNTIME_ADDRESS
            && pointer_offsets == expected_pointer_offsets
            && callers == CALLER_RUNTIME_ADDRESSES,
        "memory-card-swap manifest consumer binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER && manifest.font_role == FONT_ROLE,
        "memory-card-swap manifest role changed"
    );
    ensure!(
        manifest.glyphs.len() == MEMORY_CARD_SWAP_GLYPHS.len()
            && manifest.reused_source_glyphs.len() == REUSED_SOURCE_GLYPHS.len()
            && manifest.units.len() == MEMORY_CARD_SWAP_SEQUENCES.len(),
        "memory-card-swap asset population changed"
    );

    let mut cells = Vec::new();
    for (asset, (text, code, cell)) in manifest.glyphs.iter().zip(MEMORY_CARD_SWAP_GLYPHS) {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "memory-card-swap glyph allocation changed for {text}"
        );
        ensure!(
            cells.iter().all(|other| !cells_overlap(*other, cell)),
            "memory-card-swap glyph cells overlap"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "memory-card-swap source glyph cell changed for {text}"
        );
        cells.push(cell);
    }
    for (asset, (text, code, cell, indexed_sha256)) in manifest
        .reused_source_glyphs
        .iter()
        .zip(REUSED_SOURCE_GLYPHS)
    {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == indexed_sha256,
            "memory-card-swap reused source glyph changed for {text}"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == indexed_sha256,
            "memory-card-swap reused source glyph pixels changed for {text}"
        );
    }
    Ok(())
}

fn validate_unit(
    unit: &MemoryCardSwapUnit,
    spec: MemoryCardSwapSequenceSpec,
    overlay: &[u8],
) -> Result<()> {
    ensure!(
        unit.kind == UNIT_KIND
            && unit.id == spec.id
            && unit.source_text == spec.source_text
            && unit
                .korean_text
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
        "memory-card-swap text meaning changed for {}",
        unit.id
    );
    let callers = unit
        .source
        .caller_runtime_addresses
        .iter()
        .map(|address| parse_hex_u32(address))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        parse_hex_usize(&unit.source.sequence_offset)? == spec.sequence_offset
            && unit.source.storage_length == spec.storage_length
            && parse_hex_usize(&unit.source.terminator_offset)? == spec.terminator_offset
            && parse_hex_usize(&unit.source.pointer_storage_offset)? == spec.pointer_storage_offset
            && unit.source.encoded_sha256 == spec.source_sha256
            && unit.source.source_line_command_counts == spec.source_line_command_counts
            && callers == spec.caller_runtime_addresses,
        "memory-card-swap source binding changed for {}",
        unit.id
    );
    let source_bytes = overlay
        .get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
        .context("memory-card-swap source command record is truncated")?;
    ensure!(
        source_bytes == spec.source_bytes && sha256_bytes(source_bytes) == spec.source_sha256,
        "memory-card-swap source command record changed for {}",
        unit.id
    );
    ensure!(
        source_bytes.get(spec.terminator_offset - spec.sequence_offset) == Some(&0x81),
        "memory-card-swap source terminator changed for {}",
        unit.id
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated,
        "memory-card-swap unit {} lacks authored development input",
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
    u32::try_from(parse_hex_usize(value)?).context("address exceeds u32")
}

fn parse_hex_u16(value: &str) -> Result<u16> {
    u16::try_from(parse_hex_usize(value)?).context("glyph code exceeds u16")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "memory-card-swap unit path must stay inside its asset directory"
    );
    Ok(())
}
