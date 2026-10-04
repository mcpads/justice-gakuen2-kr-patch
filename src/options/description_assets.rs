use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::assets::parse_hex_usize;
use super::description_model::{
    OptionsDescriptionAudit, OptionsDescriptionManifest, OptionsDescriptionUnit,
    OptionsDescriptionVariant,
};
use super::description_source::{
    ITEM_COUNT, ITEM_SLOT_SIZE, ITEM_STATE_COUNTS, ITEM_TABLE_OFFSETS, OPTINFO_DECODED_SHA256,
    OPTINFO_PATH, OPTINFO_STORED_SHA256, ROOT_TABLE_OFFSET, STREAM_ARENA_END, STREAM_ARENA_OFFSET,
};
use super::description_stream::parse_description_stream;
use super::model::{OptionsDevelopmentStatus, OptionsReleaseStatus};
use super::source::{OVERLAY_PATH, OVERLAY_RUNTIME_BASE, OptionsSource};

const MANIFEST_FILE: &str = "descriptions/manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 options-description translation manifest";
const UNIT_KIND: &str = "Justice Gakuen 2 options-description translation unit";

pub(super) struct LoadedDescriptionAssets {
    pub(super) manifest_sha256: String,
    pub(super) units: Vec<OptionsDescriptionUnit>,
}

pub(super) fn load_description_assets(
    root: &Path,
    source: &OptionsSource,
) -> Result<LoadedDescriptionAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: OptionsDescriptionManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest, source)?;

    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut units = Vec::with_capacity(manifest.units.len());
    for (item_index, reference) in manifest.units.iter().enumerate() {
        ensure!(
            !reference.id.is_empty() && ids.insert(reference.id.clone()),
            "duplicate or empty options-description id"
        );
        validate_relative_file(&reference.file)?;
        ensure!(
            files.insert(reference.file.clone()),
            "duplicate options-description file"
        );
        let path = root.join("descriptions").join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: OptionsDescriptionUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND && unit.id == reference.id && unit.item_index == item_index,
            "options-description unit identity differs from its manifest position"
        );
        validate_unit(&unit, source)?;
        units.push(unit);
    }
    ensure!(
        units.len() == ITEM_COUNT,
        "options descriptions must cover five items"
    );
    Ok(LoadedDescriptionAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        units,
    })
}

pub(super) fn audit_description_assets(
    loaded: &LoadedDescriptionAssets,
) -> OptionsDescriptionAudit {
    let authored_item_count = loaded
        .units
        .iter()
        .filter(|unit| unit.development_status == OptionsDevelopmentStatus::Authored)
        .count();
    let release_approved_item_count = loaded
        .units
        .iter()
        .filter(|unit| unit.release_status == OptionsReleaseStatus::Approved)
        .count();
    OptionsDescriptionAudit {
        item_count: loaded.units.len(),
        state_count: loaded.units.iter().map(|unit| unit.states.len()).sum(),
        authored_item_count,
        release_approved_item_count,
        source_stream_count: loaded.units.iter().map(|unit| unit.states.len() + 1).sum(),
        optinfo_path: OPTINFO_PATH.to_string(),
        optinfo_stored_sha256: OPTINFO_STORED_SHA256.to_string(),
        optinfo_decoded_sha256: OPTINFO_DECODED_SHA256.to_string(),
        manifest_sha256: loaded.manifest_sha256.clone(),
        source_bindings_match: true,
        development_asset_input_available: authored_item_count == ITEM_COUNT,
        release_candidate_input_eligible: release_approved_item_count == ITEM_COUNT,
    }
}

fn validate_manifest(manifest: &OptionsDescriptionManifest, source: &OptionsSource) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown options-description manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256,
        "options-description manifest source BIN changed"
    );
    ensure!(
        manifest.overlay_path == OVERLAY_PATH
            && manifest.overlay_sha256 == sha256_bytes(&source.overlay),
        "options-description manifest NEWOPT.BIN identity changed"
    );
    ensure!(
        manifest.optinfo_path == OPTINFO_PATH
            && manifest.optinfo_stored_sha256 == sha256_bytes(&source.optinfo_stored)
            && manifest.optinfo_decoded_sha256 == sha256_bytes(&source.optinfo_decoded),
        "options-description manifest OPTINFO.TIZ identity changed"
    );
    ensure!(
        parse_hex_usize(
            &manifest.stream_arena_offset,
            "description stream arena offset"
        )? == STREAM_ARENA_OFFSET
            && parse_hex_usize(&manifest.stream_arena_end, "description stream arena end")?
                == STREAM_ARENA_END,
        "options-description stream arena changed"
    );
    let item_table_offsets = manifest
        .item_table_offsets
        .iter()
        .map(|offset| parse_hex_usize(offset, "description item table offset"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        item_table_offsets == ITEM_TABLE_OFFSETS,
        "options-description item tables changed"
    );
    ensure!(
        parse_hex_usize(&manifest.root_table_offset, "description root table offset")?
            == ROOT_TABLE_OFFSET,
        "options-description root table changed"
    );
    ensure!(
        manifest.units.len() == ITEM_COUNT,
        "options-description manifest item count changed"
    );
    for (item_index, table_offset) in ITEM_TABLE_OFFSETS.into_iter().enumerate() {
        let pointer = read_u32(&source.overlay, ROOT_TABLE_OFFSET + item_index * 4)?;
        ensure!(
            pointer == OVERLAY_RUNTIME_BASE + u32::try_from(table_offset)?,
            "options-description root pointer {item_index} changed"
        );
    }
    Ok(())
}

fn validate_unit(unit: &OptionsDescriptionUnit, source: &OptionsSource) -> Result<()> {
    ensure!(
        unit.item_index < ITEM_COUNT && unit.states.len() == ITEM_STATE_COUNTS[unit.item_index],
        "options-description {} state population changed",
        unit.id
    );
    ensure!(
        unit.development_status == OptionsDevelopmentStatus::Authored
            && unit.release_status != OptionsReleaseStatus::Untranslated,
        "options-description {} is not an authored development asset",
        unit.id
    );
    let slot_offset = parse_hex_usize(&unit.source_slot_offset, "description slot offset")?;
    ensure!(
        slot_offset == unit.item_index * ITEM_SLOT_SIZE,
        "options-description {} slot offset changed",
        unit.id
    );
    let slot = source
        .optinfo_decoded
        .get(slot_offset..slot_offset + ITEM_SLOT_SIZE)
        .context("options-description source slot is truncated")?;
    ensure!(
        sha256_bytes(slot) == unit.source_slot_sha256,
        "options-description {} source slot changed",
        unit.id
    );
    let variants = std::iter::once(&unit.common).chain(&unit.states);
    let table_offset = ITEM_TABLE_OFFSETS[unit.item_index];
    for (variant_index, variant) in variants.enumerate() {
        validate_variant(&unit.id, variant, source)?;
        let stream_offset = parse_hex_usize(
            &variant.source_stream_offset,
            "description source stream offset",
        )?;
        let pointer = read_u32(&source.overlay, table_offset + variant_index * 4)?;
        ensure!(
            pointer == OVERLAY_RUNTIME_BASE + u32::try_from(stream_offset)?,
            "options-description {} table pointer {variant_index} changed",
            unit.id
        );
    }
    Ok(())
}

fn validate_variant(
    id: &str,
    variant: &OptionsDescriptionVariant,
    source: &OptionsSource,
) -> Result<()> {
    ensure!(
        !variant.source_lines.is_empty()
            && !variant.korean_lines.is_empty()
            && variant
                .source_lines
                .iter()
                .all(|line| !line.trim().is_empty())
            && variant
                .korean_lines
                .iter()
                .all(|line| !line.trim().is_empty()),
        "options-description {id} contains a blank source or Korean line"
    );
    let stream_offset = parse_hex_usize(
        &variant.source_stream_offset,
        "description source stream offset",
    )?;
    ensure!(
        (STREAM_ARENA_OFFSET..STREAM_ARENA_END).contains(&stream_offset),
        "options-description {id} stream leaves its source arena"
    );
    let source_bytes = variant
        .source_bytes
        .iter()
        .map(|byte| {
            u8::try_from(parse_hex_usize(byte, "description source byte")?)
                .context("description source byte exceeds u8")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        source
            .overlay
            .get(stream_offset..stream_offset + source_bytes.len())
            == Some(source_bytes.as_slice()),
        "options-description {id} source stream bytes changed"
    );
    let parsed = parse_description_stream(&source_bytes)?;
    ensure!(
        parsed.lines.len() == variant.source_lines.len(),
        "options-description {id} source line count changed"
    );
    for (line, spans) in variant.source_lines.iter().zip(&parsed.lines) {
        let cell_count = spans
            .iter()
            .map(|span| usize::from(span.cell_count))
            .sum::<usize>();
        let character_count = line.chars().count();
        ensure!(
            character_count <= cell_count && cell_count - character_count <= 2,
            "options-description {id} transcription does not match its source strip width"
        );
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("truncated options-description pointer")?
            .try_into()?,
    ))
}

fn validate_relative_file(file: &str) -> Result<()> {
    let path = Path::new(file);
    ensure!(
        !file.is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "options-description manifest contains an unsafe file path"
    );
    Ok(())
}
