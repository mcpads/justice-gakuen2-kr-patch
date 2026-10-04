use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::command_sequences::{
    MEMORY_CARD_COPY_PROMPT_OFFSET, MEMORY_CARD_DESTINATION_OFFSET,
    SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256, SOURCE_MEMORY_CARD_DESTINATION_SHA256,
};
use super::consumer::{
    COMMAND_RENDERER_RUNTIME_ADDRESS, COMPOSER_RUNTIME_ADDRESS, DIRECT_CALLER_RUNTIME_ADDRESSES,
    EXIT_CALLER_RUNTIME_ADDRESS, MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET,
    MEMORY_CARD_DESTINATION_POINTER_OFFSET,
};
use super::glyph_slots::{
    CONFIRMATION_GLYPHS, FIXED_RECORD_SPACE_CELL, FIXED_RECORD_SPACE_CODE,
    SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{ConfirmationManifest, ConfirmationTextUnit, DevelopmentStatus, ReleaseStatus};
use super::records::{
    ALTERNATE_PROMPT_RECORD_OFFSET, EXIT_PROMPT_RECORD_OFFSET, SHARED_CHOICE_RECORD_OFFSET,
    SOURCE_ALTERNATE_PROMPT_SHA256, SOURCE_EXIT_PROMPT_SHA256, SOURCE_SHARED_CHOICE_SHA256,
};
use super::source::{
    BONUS_CONFIRMATION_INVENTORY_PATH, BONUS_CONFIRMATION_OVERLAY_PATH, BonusConfirmationSource,
    GLYPH_TIM_OFFSET, INVENTORY_SOURCE_DECODED_SHA256, INVENTORY_SOURCE_STORED_SHA256,
    OVERLAY_SOURCE_SHA256,
};
use super::text_units::{ConfirmationTextSpec, TEXT_UNIT_SPECS};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_confirmation_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_confirmation_unit";
const RUNTIME_CONSUMER: &str = "bonus_confirmation_overlay_renderers";
const SHARED_CHOICE_SCOPE: &str = "all_three_direct_composer_callers";

pub(super) struct LoadedConfirmationAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256s: Vec<String>,
    pub(super) runtime_consumer: String,
    pub(super) shared_choice_scope: String,
    pub(super) units: Vec<ConfirmationTextUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusConfirmationSource,
) -> Result<LoadedConfirmationAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ConfirmationManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(TEXT_UNIT_SPECS.len());
    let mut unit_sha256s = Vec::with_capacity(TEXT_UNIT_SPECS.len());
    for (relative, spec) in manifest.units.iter().zip(TEXT_UNIT_SPECS) {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate confirmation unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: ConfirmationTextUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            ids.insert(unit.id.clone()),
            "duplicate confirmation unit id"
        );
        validate_unit(&unit, spec, &source.overlay)?;
        unit_sha256s.push(sha256_bytes(&bytes));
        units.push(unit);
    }

    Ok(LoadedConfirmationAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256s,
        runtime_consumer: manifest.runtime_consumer,
        shared_choice_scope: manifest.shared_choice_scope,
        units,
    })
}

fn validate_manifest(
    manifest: &ConfirmationManifest,
    source: &BonusConfirmationSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported confirmation manifest kind"
    );
    ensure!(
        manifest.source.inventory_path == BONUS_CONFIRMATION_INVENTORY_PATH
            && manifest.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && manifest.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.overlay_path == BONUS_CONFIRMATION_OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256,
        "confirmation manifest media binding changed"
    );
    let direct_callers = manifest
        .source
        .direct_caller_runtime_addresses
        .iter()
        .map(|address| parse_hex_u32(address))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        parse_hex_u32(&manifest.source.composer_runtime_address)? == COMPOSER_RUNTIME_ADDRESS
            && parse_hex_u32(&manifest.source.command_renderer_runtime_address)?
                == COMMAND_RENDERER_RUNTIME_ADDRESS
            && parse_hex_u32(&manifest.source.exit_caller_runtime_address)?
                == EXIT_CALLER_RUNTIME_ADDRESS
            && direct_callers == DIRECT_CALLER_RUNTIME_ADDRESSES,
        "confirmation manifest consumer binding changed"
    );
    ensure!(
        parse_hex_usize(&manifest.source.memory_card_destination_pointer_offset)?
            == MEMORY_CARD_DESTINATION_POINTER_OFFSET
            && parse_hex_usize(&manifest.source.memory_card_copy_prompt_pointer_offset)?
                == MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET
            && parse_hex_usize(&manifest.source.memory_card_destination_offset)?
                == MEMORY_CARD_DESTINATION_OFFSET
            && manifest.source.memory_card_destination_sha256
                == SOURCE_MEMORY_CARD_DESTINATION_SHA256
            && parse_hex_usize(&manifest.source.memory_card_copy_prompt_offset)?
                == MEMORY_CARD_COPY_PROMPT_OFFSET
            && manifest.source.memory_card_copy_prompt_sha256
                == SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256,
        "confirmation manifest memory-card command binding changed"
    );
    ensure!(
        parse_hex_usize(&manifest.source.exit_prompt_record_offset)? == EXIT_PROMPT_RECORD_OFFSET
            && manifest.source.exit_prompt_record_sha256 == SOURCE_EXIT_PROMPT_SHA256
            && parse_hex_usize(&manifest.source.alternate_prompt_record_offset)?
                == ALTERNATE_PROMPT_RECORD_OFFSET
            && manifest.source.alternate_prompt_record_sha256 == SOURCE_ALTERNATE_PROMPT_SHA256
            && parse_hex_usize(&manifest.source.shared_choice_record_offset)?
                == SHARED_CHOICE_RECORD_OFFSET
            && manifest.source.shared_choice_record_sha256 == SOURCE_SHARED_CHOICE_SHA256,
        "confirmation manifest record binding changed"
    );
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER
            && manifest.shared_choice_scope == SHARED_CHOICE_SCOPE,
        "confirmation manifest consumer meaning changed"
    );
    ensure!(
        manifest.glyphs.len() == CONFIRMATION_GLYPHS.len()
            && manifest.units.len() == TEXT_UNIT_SPECS.len(),
        "confirmation asset population changed"
    );
    let mut cells = Vec::new();
    for (asset, (text, code, cell)) in manifest.glyphs.iter().zip(CONFIRMATION_GLYPHS) {
        ensure!(
            asset.text == text
                && parse_hex_u16(&asset.code)? == code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "confirmation glyph allocation changed for {text}"
        );
        ensure!(
            cells.iter().all(|other| !cells_overlap(*other, cell)),
            "confirmation glyph cells overlap"
        );
        let pixels =
            read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "confirmation source glyph cell changed for {text}"
        );
        cells.push(cell);
    }
    let blank = &manifest.fixed_record_space;
    ensure!(
        blank.text == " "
            && parse_hex_u16(&blank.code)? == FIXED_RECORD_SPACE_CODE
            && blank.cell == FIXED_RECORD_SPACE_CELL
            && blank.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
        "fixed confirmation record space reservation changed"
    );
    ensure!(
        cells
            .iter()
            .all(|other| !cells_overlap(*other, FIXED_RECORD_SPACE_CELL)),
        "fixed confirmation record space overlaps a visible glyph cell"
    );
    let blank_pixels = read_indexed_cell_in_prefix(
        &source.inventory_decoded,
        GLYPH_TIM_OFFSET,
        FIXED_RECORD_SPACE_CELL,
    )?;
    ensure!(
        sha256_bytes(&blank_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
        "fixed confirmation record space source cell is not blank"
    );
    Ok(())
}

pub(super) fn validate_unit(
    unit: &ConfirmationTextUnit,
    spec: ConfirmationTextSpec,
    overlay: &[u8],
) -> Result<()> {
    ensure!(
        unit.kind == UNIT_KIND
            && unit.id == spec.id
            && unit.source_text == spec.source_text
            && unit.korean_text.as_deref() == Some(spec.korean_text),
        "confirmation text meaning changed for {}",
        unit.id
    );
    ensure!(
        parse_hex_usize(&unit.source.record_offset)? == spec.record_offset
            && unit.source.record_length == spec.record_length
            && unit.source.encoded_sha256 == spec.source_sha256,
        "confirmation record binding changed for {}",
        unit.id
    );
    let source_bytes = overlay
        .get(spec.record_offset..spec.record_offset + spec.record_length)
        .context("confirmation source record is truncated")?;
    ensure!(
        sha256_bytes(source_bytes) == spec.source_sha256,
        "confirmation source record hash changed for {}",
        unit.id
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated,
        "confirmation unit {} lacks authored development input",
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
        "confirmation unit path must stay inside its asset directory"
    );
    Ok(())
}
