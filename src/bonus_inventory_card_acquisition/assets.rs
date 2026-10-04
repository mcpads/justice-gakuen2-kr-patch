use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::command_sequences::{CARD_ACQUISITION_SEQUENCES, CardAcquisitionSequenceSpec};
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, FOLLOWING_POINTER_STORAGE_OFFSET, OVERLAY_RUNTIME_BASE,
    POINTER_TABLE_OFFSET, SECONDARY_PARSER_RUNTIME_ADDRESS,
};
use super::glyph_slots::{
    CARD_ACQUISITION_GLYPHS, REUSED_SOURCE_GLYPHS, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{
    CardAcquisitionManifest, CardAcquisitionUnit, DevelopmentStatus, ReleaseStatus,
};
use super::source::{
    BONUS_INVENTORY_CARD_ACQUISITION_INVENTORY_PATH, BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_PATH,
    BonusInventoryCardAcquisitionSource, GLYPH_TIM_OFFSET, INVENTORY_SOURCE_DECODED_SHA256,
    INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_inventory_card_acquisition_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_inventory_card_acquisition_unit";
const RUNTIME_CONSUMER: &str = "bonus_inventory_card_acquisition_message";
const FONT_ROLE: &str = "bonus_card_acquisition";

pub(super) struct LoadedCardAcquisitionAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256s: Vec<String>,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) units: Vec<CardAcquisitionUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusInventoryCardAcquisitionSource,
) -> Result<LoadedCardAcquisitionAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: CardAcquisitionManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(CARD_ACQUISITION_SEQUENCES.len());
    let mut unit_sha256s = Vec::with_capacity(CARD_ACQUISITION_SEQUENCES.len());
    for (relative, spec) in manifest.units.iter().zip(CARD_ACQUISITION_SEQUENCES) {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate card-acquisition unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: CardAcquisitionUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            ids.insert(unit.id.clone()),
            "duplicate card-acquisition unit id"
        );
        validate_unit(&unit, spec, &source.overlay)?;
        unit_sha256s.push(sha256_bytes(&bytes));
        units.push(unit);
    }

    Ok(LoadedCardAcquisitionAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256s,
        runtime_consumer: manifest.runtime_consumer,
        font_role: manifest.font_role,
        units,
    })
}

fn validate_manifest(
    manifest: &CardAcquisitionManifest,
    source: &BonusInventoryCardAcquisitionSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported card-acquisition manifest kind"
    );
    ensure!(
        manifest.source.inventory_path == BONUS_INVENTORY_CARD_ACQUISITION_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256,
        "card-acquisition manifest media binding changed"
    );
    let pointer_offsets = manifest
        .source
        .pointer_storage_offsets
        .iter()
        .map(|offset| parse_hex_usize(offset))
        .collect::<Result<Vec<_>>>()?;
    let expected_pointer_offsets = CARD_ACQUISITION_SEQUENCES
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
            && parse_hex_u32(&manifest.source.secondary_parser_runtime_address)?
                == SECONDARY_PARSER_RUNTIME_ADDRESS
            && pointer_offsets == expected_pointer_offsets
            && parse_hex_usize(&manifest.source.following_pointer_storage_offset)?
                == FOLLOWING_POINTER_STORAGE_OFFSET
            && callers == CALLER_RUNTIME_ADDRESSES,
        "card-acquisition manifest consumer binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER && manifest.font_role == FONT_ROLE,
        "card-acquisition manifest role changed"
    );
    ensure!(
        manifest.glyphs.len() == CARD_ACQUISITION_GLYPHS.len()
            && manifest.reused_source_glyphs.len() == REUSED_SOURCE_GLYPHS.len()
            && manifest.units.len() == CARD_ACQUISITION_SEQUENCES.len(),
        "card-acquisition asset population changed"
    );

    let mut cells = Vec::new();
    for (asset, (text, code, cell)) in manifest.glyphs.iter().zip(CARD_ACQUISITION_GLYPHS) {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "card-acquisition glyph allocation changed for {text}"
        );
        ensure!(
            cells.iter().all(|other| !cells_overlap(*other, cell)),
            "card-acquisition glyph cells overlap"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "card-acquisition source glyph cell changed for {text}"
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
            "card-acquisition reused source glyph changed for {text}"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == indexed_sha256,
            "card-acquisition reused source glyph pixels changed for {text}"
        );
    }
    Ok(())
}

fn validate_unit(
    unit: &CardAcquisitionUnit,
    spec: CardAcquisitionSequenceSpec,
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
        "card-acquisition text meaning changed for {}",
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
            && callers == spec.caller_runtime_addresses,
        "card-acquisition source binding changed for {}",
        unit.id
    );
    let source_bytes = overlay
        .get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
        .context("card-acquisition source command record is truncated")?;
    ensure!(
        source_bytes == spec.source_bytes && sha256_bytes(source_bytes) == spec.source_sha256,
        "card-acquisition source command record changed for {}",
        unit.id
    );
    ensure!(
        source_bytes.get(spec.terminator_offset - spec.sequence_offset) == Some(&0x81),
        "card-acquisition source terminator changed for {}",
        unit.id
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated,
        "card-acquisition unit {} lacks authored development input",
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
        "card-acquisition unit path must stay inside its asset directory"
    );
    Ok(())
}
