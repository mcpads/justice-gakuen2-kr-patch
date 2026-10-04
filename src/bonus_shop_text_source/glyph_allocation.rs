use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::asset_sync::validate_translation_decision;
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopTextDelegatedRecord,
    BonusShopTextSourceTokenKind, BonusShopTranslationUnit,
};
use super::runtime_glyphs::protected_runtime_glyph_codes;
use crate::bonus_shop_source::{
    DIRECT_SELECTOR_REGION, GLYPH_TIM_OFFSET, cells_overlap, glyph_cell, sprite_selector,
};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const CANONICAL_COLUMN_COUNT: u16 = 12;
const CANONICAL_ROW_COUNT: u16 = 12;
const PAGE_COUNT: u16 = 4;

#[derive(Debug)]
pub(super) struct BonusShopGlyphAllocationPlan {
    pub(super) allocations: Vec<BonusShopGlyphAllocation>,
    pub(super) glyph_codes: BTreeMap<BonusShopFontRole, BTreeMap<char, u16>>,
    pub(super) required_glyph_count: usize,
    pub(super) available_glyph_cell_count: usize,
    pub(super) protected_source_code_count: usize,
    pub(super) direct_selector_code_count: usize,
    pub(super) runtime_glyph_code_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct BonusShopGlyphAllocation {
    pub(super) font_role: BonusShopFontRole,
    pub(super) character: char,
    pub(super) code: u16,
    pub(super) cell: Cell,
    pub(super) source_indexed_sha256: String,
}

pub(super) fn allocate_bonus_shop_glyphs(
    source_overlay: &[u8],
    source_shop_ui_decoded: &[u8],
    units: &BTreeMap<String, BonusShopTranslationUnit>,
    delegated_records: &BTreeMap<String, BonusShopTextDelegatedRecord>,
    reserved_codes: impl IntoIterator<Item = u16>,
    additional_characters: impl IntoIterator<Item = (BonusShopFontRole, char)>,
) -> Result<BonusShopGlyphAllocationPlan> {
    let runtime_glyph_codes = protected_runtime_glyph_codes(source_overlay)?;
    let runtime_glyph_code_count = runtime_glyph_codes.len();
    let selection = select_bonus_shop_glyph_codes(
        source_overlay,
        units,
        delegated_records,
        reserved_codes.into_iter().chain(runtime_glyph_codes),
        additional_characters,
    )?;
    let mut glyph_codes = BTreeMap::<BonusShopFontRole, BTreeMap<char, u16>>::new();
    let mut allocations = Vec::with_capacity(selection.assignments.len());
    for ((font_role, character), code) in selection.assignments {
        let cell = glyph_cell(code);
        let source_pixels =
            read_indexed_cell_in_prefix(source_shop_ui_decoded, GLYPH_TIM_OFFSET, cell)?;
        ensure!(
            glyph_codes
                .entry(font_role)
                .or_default()
                .insert(character, code)
                .is_none(),
            "bonus-shop glyph allocation repeats a role-scoped character"
        );
        allocations.push(BonusShopGlyphAllocation {
            font_role,
            character,
            code,
            cell,
            source_indexed_sha256: sha256_bytes(&source_pixels),
        });
    }
    Ok(BonusShopGlyphAllocationPlan {
        required_glyph_count: allocations.len(),
        available_glyph_cell_count: selection.available_glyph_cell_count,
        protected_source_code_count: selection.protected_source_code_count,
        direct_selector_code_count: selection.direct_selector_code_count,
        runtime_glyph_code_count,
        allocations,
        glyph_codes,
    })
}

pub(super) struct GlyphCodeSelection {
    pub(super) assignments: Vec<((BonusShopFontRole, char), u16)>,
    pub(super) available_glyph_cell_count: usize,
    pub(super) protected_source_code_count: usize,
    pub(super) direct_selector_code_count: usize,
}

pub(super) fn select_bonus_shop_glyph_codes(
    source_overlay: &[u8],
    units: &BTreeMap<String, BonusShopTranslationUnit>,
    delegated_records: &BTreeMap<String, BonusShopTextDelegatedRecord>,
    reserved_codes: impl IntoIterator<Item = u16>,
    additional_characters: impl IntoIterator<Item = (BonusShopFontRole, char)>,
) -> Result<GlyphCodeSelection> {
    let mut required = required_role_characters(units)?;
    required.extend(additional_characters);
    let source_codes = protected_source_codes(units, delegated_records)?;
    let direct_codes = direct_selector_codes(source_overlay)?;
    let mut protected_cells = source_codes
        .iter()
        .chain(&direct_codes)
        .copied()
        .chain(reserved_codes)
        .map(glyph_cell)
        .collect::<Vec<_>>();
    let candidates = canonical_codes()
        .filter(|code| {
            let cell = glyph_cell(*code);
            protected_cells
                .iter()
                .all(|protected| !cells_overlap(*protected, cell))
        })
        .collect::<Vec<_>>();
    ensure!(
        required.len() <= candidates.len(),
        "bonus-shop text needs {} role-scoped glyphs but only {} consumer-safe atlas cells are available",
        required.len(),
        candidates.len()
    );

    let mut assignments = Vec::with_capacity(required.len());
    for (key, code) in required.into_iter().zip(candidates.iter().copied()) {
        let cell = glyph_cell(code);
        ensure!(
            protected_cells
                .iter()
                .all(|protected| !cells_overlap(*protected, cell)),
            "bonus-shop glyph allocator selected an occupied physical cell"
        );
        protected_cells.push(cell);
        assignments.push((key, code));
    }
    Ok(GlyphCodeSelection {
        assignments,
        available_glyph_cell_count: candidates.len(),
        protected_source_code_count: source_codes.len(),
        direct_selector_code_count: direct_codes.len(),
    })
}

fn required_role_characters(
    units: &BTreeMap<String, BonusShopTranslationUnit>,
) -> Result<BTreeSet<(BonusShopFontRole, char)>> {
    let mut required = BTreeSet::new();
    for unit in units.values() {
        validate_translation_decision(unit)?;
        if unit.development_status != BonusShopDevelopmentStatus::Authored {
            continue;
        }
        let role = unit
            .font_role
            .context("authored bonus-shop unit lost its font role")?;
        for character in unit
            .korean_text
            .as_deref()
            .context("authored bonus-shop unit lost its Korean text")?
            .chars()
            .filter(|character| *character != ' ' && *character != '\n')
        {
            ensure!(
                !character.is_control(),
                "bonus-shop text contains unsupported control character {character:?}"
            );
            required.insert((role, character));
        }
    }
    Ok(required)
}

fn protected_source_codes(
    units: &BTreeMap<String, BonusShopTranslationUnit>,
    delegated_records: &BTreeMap<String, BonusShopTextDelegatedRecord>,
) -> Result<BTreeSet<u16>> {
    let mut codes = BTreeSet::new();
    for source in units
        .values()
        .filter(|unit| unit.development_status == BonusShopDevelopmentStatus::Untranslated)
        .map(|unit| &unit.source)
        .chain(delegated_records.values().map(|record| &record.source))
    {
        for token in &source.tokens {
            if token.kind != BonusShopTextSourceTokenKind::Glyph {
                continue;
            }
            let value = token
                .code
                .as_deref()
                .context("bonus-shop source glyph token lost its code")?;
            codes.insert(parse_hex_u16(value)?);
        }
    }
    Ok(codes)
}

fn direct_selector_codes(source_overlay: &[u8]) -> Result<BTreeSet<u16>> {
    let bytes = source_overlay
        .get(DIRECT_SELECTOR_REGION[0]..DIRECT_SELECTOR_REGION[1])
        .context("KOUBAI direct-selector region is truncated")?;
    Ok((0u16..=0x03ff)
        .filter(|code| {
            let selector = sprite_selector(*code);
            bytes
                .windows(selector.len())
                .any(|window| window == selector)
        })
        .collect())
}

fn canonical_codes() -> impl Iterator<Item = u16> {
    (0..PAGE_COUNT).flat_map(|page| {
        (0..CANONICAL_ROW_COUNT).flat_map(move |row| {
            (0..CANONICAL_COLUMN_COUNT).map(move |column| page << 8 | row << 4 | column)
        })
    })
}

fn parse_hex_u16(value: &str) -> Result<u16> {
    u16::from_str_radix(
        value
            .strip_prefix("0x")
            .context("bonus-shop glyph code is not hexadecimal")?,
        16,
    )
    .context("invalid bonus-shop glyph code")
}
