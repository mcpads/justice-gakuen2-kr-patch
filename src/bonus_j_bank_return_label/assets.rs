use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::command_sequence::RETURN_LABEL_SEQUENCE;
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, COMMAND_RENDERER_RUNTIME_ADDRESS, DIRECT_SELECTOR_REGION,
    OVERLAY_RUNTIME_BASE, POINTER_TABLE_END, POINTER_TABLE_OFFSET,
};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::glyph_slots::{GLYPH_CODES, SOURCE_BLANK_GLYPH_INDEXED_SHA256, glyph_cell};
use super::model::{
    DevelopmentStatus, JBankReturnLabelGlyphAllocation, JBankReturnLabelManifest,
    JBankReturnLabelUnit, ReleaseStatus,
};
use super::source::{
    BONUS_J_BANK_RETURN_LABEL_INVENTORY_PATH, BONUS_J_BANK_RETURN_LABEL_OVERLAY_PATH,
    BonusJBankReturnLabelSource, GLYPH_TIM_OFFSET, INVENTORY_SOURCE_DECODED_SHA256,
    INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_j_bank_return_label_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_j_bank_return_label_unit";
const RUNTIME_CONSUMER: &str = "bonus_j_bank_return_label";
const FONT_ROLE: &str = "bonus_j_bank_return_label";

pub(super) struct LoadedJBankReturnLabelAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256: String,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) unit: JBankReturnLabelUnit,
    pub(super) glyph_allocations: Vec<JBankReturnLabelGlyphAllocation>,
    pub(super) physical_alias_source_cells_match_blank_hash: bool,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusJBankReturnLabelSource,
) -> Result<LoadedJBankReturnLabelAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: JBankReturnLabelManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest_source(&manifest, source)?;
    ensure!(
        manifest.units.len() == 1,
        "J-BANK return-label manifest must own one unit"
    );
    ensure_relative_path(&manifest.units[0])?;
    let unit_path = root.join(&manifest.units[0]);
    let unit_bytes = std::fs::read(&unit_path)
        .with_context(|| format!("failed to read {}", unit_path.display()))?;
    let unit: JBankReturnLabelUnit = serde_json::from_slice(&unit_bytes)
        .with_context(|| format!("failed to parse {}", unit_path.display()))?;
    validate_unit(&unit, &source.overlay)?;
    let glyph_allocations = validate_glyphs(&manifest, &unit, source)?;
    let physical_alias_source_cells_match_blank_hash =
        validate_physical_alias_source_cells(source)?;

    Ok(LoadedJBankReturnLabelAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256: sha256_bytes(&unit_bytes),
        runtime_consumer: manifest.runtime_consumer,
        font_role: manifest.font_role,
        unit,
        glyph_allocations,
        physical_alias_source_cells_match_blank_hash,
    })
}

fn validate_physical_alias_source_cells(source: &BonusJBankReturnLabelSource) -> Result<bool> {
    let mut all_match = true;
    for code in allocated_physical_alias_codes() {
        let pixels = read_indexed_cell_in_prefix(
            &source.inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph_cell(code),
        )?;
        all_match &= sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256;
    }
    ensure!(
        all_match,
        "J-BANK return-label physical alias source cell is not blank"
    );
    Ok(all_match)
}

fn validate_manifest_source(
    manifest: &JBankReturnLabelManifest,
    source: &BonusJBankReturnLabelSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported J-BANK return-label manifest kind"
    );
    ensure!(
        manifest.source.inventory_path == BONUS_J_BANK_RETURN_LABEL_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_J_BANK_RETURN_LABEL_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256,
        "J-BANK return-label manifest media binding changed"
    );
    let selector_region = manifest
        .source
        .direct_selector_region
        .iter()
        .map(|offset| parse_hex_usize(offset))
        .collect::<Result<Vec<_>>>()?;
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
            && selector_region == DIRECT_SELECTOR_REGION
            && parse_hex_u32(&manifest.source.command_renderer_runtime_address)?
                == COMMAND_RENDERER_RUNTIME_ADDRESS
            && parse_hex_usize(&manifest.source.pointer_storage_offset)?
                == RETURN_LABEL_SEQUENCE.pointer_storage_offset
            && callers == CALLER_RUNTIME_ADDRESSES,
        "J-BANK return-label manifest consumer binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER && manifest.font_role == FONT_ROLE,
        "J-BANK return-label manifest role changed"
    );
    ensure!(
        source.overlay.len() == 94_704,
        "J-BANK return-label source overlay length changed"
    );
    Ok(())
}

fn validate_glyphs(
    manifest: &JBankReturnLabelManifest,
    unit: &JBankReturnLabelUnit,
    source: &BonusJBankReturnLabelSource,
) -> Result<Vec<JBankReturnLabelGlyphAllocation>> {
    let text = unit
        .korean_text
        .as_deref()
        .context("authored J-BANK return-label text disappeared")?;
    let characters = text.chars().collect::<Vec<_>>();
    ensure!(
        manifest.glyphs.len() == GLYPH_CODES.len()
            && characters.len() == manifest.glyphs.len()
            && characters.iter().copied().collect::<BTreeSet<_>>().len() == characters.len()
            && characters
                .iter()
                .all(|character| !character.is_control() && !character.is_whitespace()),
        "J-BANK return-label glyph repertoire does not match its authored text"
    );

    let mut allocations = Vec::with_capacity(manifest.glyphs.len());
    for (expected_index, ((asset, code), text)) in manifest
        .glyphs
        .iter()
        .zip(GLYPH_CODES)
        .zip(characters)
        .enumerate()
    {
        let cell = glyph_cell(code);
        ensure!(
            asset.text_index == expected_index
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "J-BANK return-label glyph allocation changed at text index {expected_index}"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "J-BANK return-label source glyph cell changed at text index {expected_index}"
        );
        allocations.push(JBankReturnLabelGlyphAllocation { text, code, cell });
    }
    ensure!(
        allocations.iter().enumerate().all(|(index, allocation)| {
            allocations
                .iter()
                .skip(index + 1)
                .all(|other| !cells_overlap(allocation.cell, other.cell))
        }),
        "J-BANK return-label canonical glyph cells overlap"
    );
    Ok(allocations)
}

fn validate_unit(unit: &JBankReturnLabelUnit, overlay: &[u8]) -> Result<()> {
    ensure!(
        unit.kind == UNIT_KIND
            && unit.id == RETURN_LABEL_SEQUENCE.id
            && unit.source_text == RETURN_LABEL_SEQUENCE.source_text
            && unit
                .korean_text
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
        "J-BANK return-label text meaning changed"
    );
    let callers = unit
        .source
        .caller_runtime_addresses
        .iter()
        .map(|address| parse_hex_u32(address))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        parse_hex_usize(&unit.source.sequence_offset)? == RETURN_LABEL_SEQUENCE.sequence_offset
            && unit.source.storage_length == RETURN_LABEL_SEQUENCE.storage_length
            && parse_hex_usize(&unit.source.terminator_offset)?
                == RETURN_LABEL_SEQUENCE.terminator_offset
            && parse_hex_usize(&unit.source.pointer_storage_offset)?
                == RETURN_LABEL_SEQUENCE.pointer_storage_offset
            && unit.source.encoded_sha256 == RETURN_LABEL_SEQUENCE.source_sha256
            && callers == RETURN_LABEL_SEQUENCE.caller_runtime_addresses,
        "J-BANK return-label source binding changed"
    );
    let source_bytes = overlay
        .get(
            RETURN_LABEL_SEQUENCE.sequence_offset
                ..RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length,
        )
        .context("J-BANK return-label source command record is truncated")?;
    ensure!(
        source_bytes == RETURN_LABEL_SEQUENCE.source_bytes
            && sha256_bytes(source_bytes) == RETURN_LABEL_SEQUENCE.source_sha256,
        "J-BANK return-label source command record changed"
    );
    ensure!(
        source_bytes
            .get(RETURN_LABEL_SEQUENCE.terminator_offset - RETURN_LABEL_SEQUENCE.sequence_offset)
            == Some(&0x81)
            && source_bytes
                .get(
                    RETURN_LABEL_SEQUENCE.terminator_offset + 1
                        - RETURN_LABEL_SEQUENCE.sequence_offset..
                )
                .is_some_and(|padding| padding.iter().all(|byte| *byte == 0)),
        "J-BANK return-label source terminator or padding changed"
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated,
        "J-BANK return-label unit lacks authored development input"
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
        "J-BANK return-label unit path must stay inside its asset directory"
    );
    Ok(())
}
