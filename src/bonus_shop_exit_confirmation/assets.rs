use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::command_sequence::{RECORD_SPECS, SOURCE_UNIT_SPECS};
use super::consumer::{
    CALLER_RUNTIME_ADDRESS, COMMAND_RENDERER_RUNTIME_ADDRESS, DIRECT_SELECTOR_REGION,
    OVERLAY_RUNTIME_BASE, POINTER_COMMAND_TABLE_RANGES,
};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::glyph_slots::{
    GLYPH_CODES, QUESTION_MARK_CODE, QUESTION_MARK_SOURCE_INDEXED_SHA256,
    SOURCE_BLANK_GLYPH_INDEXED_SHA256, glyph_cell,
};
use super::model::{
    DevelopmentStatus, ReleaseStatus, ShopExitGlyphAllocation, ShopExitManifest, ShopExitTextUnit,
};
use super::source::{
    BonusShopExitConfirmationSource, FIXED_UI_TIM_OFFSET, GLYPH_TIM_OFFSET, GLYPH_TIM_SHA256,
    OVERLAY_PATH, OVERLAY_SOURCE_SHA256, SHOP_UI_PATH, SHOP_UI_SOURCE_DECODED_SHA256,
    SHOP_UI_SOURCE_STORED_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_shop_exit_confirmation_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_shop_exit_confirmation_unit";
const RUNTIME_CONSUMER: &str = "bonus_shop_exit_confirmation";
const FONT_ROLE: &str = "bonus_shop_exit_confirmation";
const UNIT_FILES: [&str; 6] = [
    "woman-prompt.json",
    "woman-yes.json",
    "woman-no.json",
    "elder-prompt.json",
    "elder-yes.json",
    "elder-no.json",
];

pub(super) struct LoadedShopExitAssets {
    pub(super) manifest_sha256: String,
    pub(super) unit_sha256s: Vec<String>,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) units: Vec<ShopExitTextUnit>,
    pub(super) glyph_allocations: Vec<ShopExitGlyphAllocation>,
    pub(super) physical_alias_source_cells_match_blank_hash: bool,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusShopExitConfirmationSource,
) -> Result<LoadedShopExitAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ShopExitManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest_source(&manifest, source)?;

    ensure!(
        manifest.units.len() == UNIT_FILES.len(),
        "shop exit-confirmation manifest must own exactly six clerk-scoped semantic units"
    );
    let mut unit_sha256s = Vec::with_capacity(UNIT_FILES.len());
    let mut units = Vec::with_capacity(UNIT_FILES.len());
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for ((relative, expected_file), spec) in
        manifest.units.iter().zip(UNIT_FILES).zip(SOURCE_UNIT_SPECS)
    {
        ensure_relative_path(relative)?;
        ensure!(
            relative == Path::new(expected_file) && paths.insert(relative.clone()),
            "shop exit-confirmation unit file set changed"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: ShopExitTextUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(ids.insert(unit.id.clone()), "duplicate shop exit unit id");
        validate_unit(&unit, spec, &source.overlay)?;
        unit_sha256s.push(sha256_bytes(&bytes));
        units.push(unit);
    }

    let glyph_allocations = validate_glyphs(&manifest, &units, source)?;
    let physical_alias_source_cells_match_blank_hash =
        validate_physical_alias_source_cells(source)?;
    Ok(LoadedShopExitAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        unit_sha256s,
        runtime_consumer: manifest.runtime_consumer,
        font_role: manifest.font_role,
        units,
        glyph_allocations,
        physical_alias_source_cells_match_blank_hash,
    })
}

fn validate_manifest_source(
    manifest: &ShopExitManifest,
    source: &BonusShopExitConfirmationSource,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported shop exit-confirmation manifest kind"
    );
    ensure!(
        manifest.source.shop_ui_path == SHOP_UI_PATH
            && manifest.source.shop_ui_stored_sha256 == SHOP_UI_SOURCE_STORED_SHA256
            && manifest.source.shop_ui_decoded_sha256 == SHOP_UI_SOURCE_DECODED_SHA256
            && parse_hex_usize(&manifest.source.fixed_ui_tim_offset)? == FIXED_UI_TIM_OFFSET
            && parse_hex_usize(&manifest.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && manifest.source.glyph_tim_sha256 == GLYPH_TIM_SHA256
            && manifest.source.overlay_path == OVERLAY_PATH
            && manifest.source.overlay_sha256 == OVERLAY_SOURCE_SHA256,
        "shop exit-confirmation manifest media binding changed"
    );
    let pointer_ranges = manifest
        .source
        .pointer_command_table_ranges
        .iter()
        .map(|range| Ok([parse_hex_usize(&range[0])?, parse_hex_usize(&range[1])?]))
        .collect::<Result<Vec<_>>>()?;
    let direct_selector_region = [
        parse_hex_usize(&manifest.source.direct_selector_region[0])?,
        parse_hex_usize(&manifest.source.direct_selector_region[1])?,
    ];
    ensure!(
        parse_hex_u32(&manifest.source.overlay_runtime_base)? == OVERLAY_RUNTIME_BASE
            && parse_hex_u32(&manifest.source.caller_runtime_address)? == CALLER_RUNTIME_ADDRESS
            && parse_hex_u32(&manifest.source.command_renderer_runtime_address)?
                == COMMAND_RENDERER_RUNTIME_ADDRESS
            && pointer_ranges == POINTER_COMMAND_TABLE_RANGES
            && direct_selector_region == DIRECT_SELECTOR_REGION,
        "shop exit-confirmation manifest consumer binding changed"
    );
    ensure!(
        manifest.source.records.len() == RECORD_SPECS.len(),
        "shop exit-confirmation clerk record population changed"
    );
    for (binding, spec) in manifest.source.records.iter().zip(RECORD_SPECS) {
        ensure!(
            binding.variant_id == spec.variant_id
                && binding.clerk_index == spec.clerk_index
                && parse_hex_usize(&binding.record_offset)? == spec.record_offset
                && binding.record_length == spec.source_record.len()
                && binding.record_sha256 == spec.source_record_sha256
                && parse_hex_usize(&binding.terminator_offset)? == spec.terminator_offset
                && parse_hex_usize(&binding.pointer_storage_offset)? == spec.pointer_storage_offset
                && parse_hex_u32(&binding.pointer_value)?
                    == OVERLAY_RUNTIME_BASE + spec.record_offset as u32,
            "shop exit-confirmation {} record binding changed",
            spec.variant_id
        );
    }
    ensure!(
        manifest.runtime_consumer == RUNTIME_CONSUMER && manifest.font_role == FONT_ROLE,
        "shop exit-confirmation manifest role changed"
    );
    ensure!(
        source.overlay.len() == 0x90c0,
        "shop exit-confirmation source overlay length changed"
    );
    Ok(())
}

fn validate_unit(
    unit: &ShopExitTextUnit,
    spec: super::command_sequence::ShopExitSourceUnitSpec,
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
        "shop exit-confirmation text meaning changed for {}",
        spec.id
    );
    ensure!(
        parse_hex_usize(&unit.source.encoded_offset)? == spec.encoded_offset
            && unit.source.encoded_length == spec.encoded_length
            && unit.source.encoded_sha256 == spec.source_sha256,
        "shop exit-confirmation source binding changed for {}",
        spec.id
    );
    let source_bytes = overlay
        .get(spec.encoded_offset..spec.encoded_offset + spec.encoded_length)
        .context("shop exit-confirmation source unit is truncated")?;
    ensure!(
        sha256_bytes(source_bytes) == spec.source_sha256,
        "shop exit-confirmation source bytes changed for {}",
        spec.id
    );
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && unit.release_status != ReleaseStatus::Untranslated,
        "shop exit-confirmation unit {} lacks authored development input",
        spec.id
    );
    Ok(())
}

fn validate_glyphs(
    manifest: &ShopExitManifest,
    units: &[ShopExitTextUnit],
    source: &BonusShopExitConfirmationSource,
) -> Result<Vec<ShopExitGlyphAllocation>> {
    ensure!(
        manifest.glyphs.len() == GLYPH_CODES.len(),
        "shop exit-confirmation glyph population changed"
    );
    let unit_by_id = units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let required = units
        .iter()
        .flat_map(|unit| {
            unit.korean_text
                .as_deref()
                .unwrap_or_default()
                .chars()
                .filter(|character| *character != ' ' && *character != '?')
        })
        .collect::<BTreeSet<_>>();
    let mut allocated_text = BTreeSet::new();
    let mut allocated_cells = Vec::new();
    let mut allocations = Vec::with_capacity(GLYPH_CODES.len());
    for (asset, expected_code) in manifest.glyphs.iter().zip(GLYPH_CODES) {
        let unit = unit_by_id
            .get(asset.unit_id.as_str())
            .with_context(|| format!("glyph refers to unknown unit {}", asset.unit_id))?;
        let text = unit
            .korean_text
            .as_deref()
            .context("authored shop exit-confirmation text disappeared")?
            .chars()
            .nth(asset.text_index)
            .context("shop exit-confirmation glyph text index is out of bounds")?;
        let code = parse_hex_u16(&asset.code)?;
        let cell = glyph_cell(code);
        ensure!(
            code == expected_code
                && asset.cell == cell
                && asset.source_indexed_pixel_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256
                && text != ' '
                && text != '?'
                && !text.is_control()
                && allocated_text.insert(text)
                && allocated_cells
                    .iter()
                    .all(|other| !cells_overlap(*other, cell)),
            "shop exit-confirmation glyph allocation changed at code 0x{expected_code:04x}"
        );
        let pixels = read_indexed_cell_in_prefix(&source.shop_ui_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "shop exit-confirmation source glyph cell changed at code 0x{code:04x}"
        );
        allocated_cells.push(cell);
        allocations.push(ShopExitGlyphAllocation { text, code, cell });
    }
    ensure!(
        allocated_text == required,
        "shop exit-confirmation glyph repertoire does not match the six authored units"
    );

    let reused = &manifest.reused_question_mark;
    ensure!(
        parse_hex_u16(&reused.code)? == QUESTION_MARK_CODE
            && reused.cell == glyph_cell(QUESTION_MARK_CODE)
            && reused.source_indexed_pixel_sha256 == QUESTION_MARK_SOURCE_INDEXED_SHA256,
        "shop exit-confirmation reused question-mark binding changed"
    );
    let pixels = read_indexed_cell_in_prefix(
        &source.shop_ui_decoded,
        GLYPH_TIM_OFFSET,
        glyph_cell(QUESTION_MARK_CODE),
    )?;
    ensure!(
        sha256_bytes(&pixels) == QUESTION_MARK_SOURCE_INDEXED_SHA256,
        "shop exit-confirmation source question-mark cell changed"
    );
    Ok(allocations)
}

fn validate_physical_alias_source_cells(source: &BonusShopExitConfirmationSource) -> Result<bool> {
    let mut all_match = true;
    for code in allocated_physical_alias_codes() {
        let pixels = read_indexed_cell_in_prefix(
            &source.shop_ui_decoded,
            GLYPH_TIM_OFFSET,
            glyph_cell(code),
        )?;
        all_match &= sha256_bytes(&pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256;
    }
    ensure!(
        all_match,
        "shop exit-confirmation physical alias source cell is not blank"
    );
    Ok(all_match)
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
        "shop exit-confirmation unit path must stay inside its asset directory"
    );
    Ok(())
}
