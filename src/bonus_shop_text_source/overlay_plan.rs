use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::asset_sync::validate_translation_decision;
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopTextSourceUnit,
    BonusShopTranslationUnit,
};
use crate::pipeline::{difference_ranges, sha256_bytes};

const BLANK_COMMAND: [u8; 3] = [0x63; 3];
const LINE_BREAK_COMMAND: u8 = 0x80;
const TERMINATOR_COMMAND: u8 = 0x81;

#[derive(Debug)]
pub(super) struct BonusShopTextOverlayPlan {
    pub(super) bytes: Vec<u8>,
    pub(super) authored_unit_count: usize,
    pub(super) untranslated_unit_count: usize,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) units: Vec<BonusShopTextOverlayUnitPlan>,
}

#[derive(Debug)]
pub(super) struct BonusShopTextOverlayUnitPlan {
    pub(super) id: String,
    pub(super) source_record_byte_count: usize,
    pub(super) output_record_byte_count: usize,
    pub(super) changed: bool,
}

pub(super) fn plan_bonus_shop_text_overlay(
    source: &[u8],
    units: &BTreeMap<String, BonusShopTranslationUnit>,
    glyph_codes: &BTreeMap<BonusShopFontRole, BTreeMap<char, u16>>,
) -> Result<BonusShopTextOverlayPlan> {
    let mut patched = source.to_vec();
    let mut expected_write_ranges = Vec::new();
    let mut unit_plans = Vec::with_capacity(units.len());
    let mut authored_unit_count = 0usize;
    let mut untranslated_unit_count = 0usize;
    let mut pointer_intervals = BTreeSet::new();
    let mut capacity_errors = Vec::new();

    for unit in units.values() {
        validate_translation_decision(unit)?;
        let interval = validate_source_interval(source, &unit.source)?;
        ensure!(
            pointer_intervals.insert(interval),
            "bonus-shop text units repeat pointer interval 0x{:04x}..0x{:04x}",
            interval[0],
            interval[1]
        );

        let output_record_byte_count = match unit.development_status {
            BonusShopDevelopmentStatus::Untranslated => {
                untranslated_unit_count += 1;
                unit.source.source_record_byte_count
            }
            BonusShopDevelopmentStatus::Authored => {
                authored_unit_count += 1;
                ensure!(
                    unit.source.trailing_interval_all_zero,
                    "authored bonus-shop unit {} has nonzero trailing pointer-interval bytes",
                    unit.id
                );
                let korean_text = unit
                    .korean_text
                    .as_deref()
                    .context("authored bonus-shop unit lost Korean text")?;
                let font_role = unit
                    .font_role
                    .context("authored bonus-shop unit lost its font role")?;
                validate_message_width(korean_text, font_role).with_context(|| {
                    format!("bonus-shop unit {} exceeds its message window", unit.id)
                })?;
                let encoded = encode_bonus_shop_text(korean_text, font_role, glyph_codes)?;
                let owned_byte_count = interval[1] - interval[0];
                if encoded.len() > owned_byte_count {
                    capacity_errors.push(format!(
                        "authored bonus-shop unit {} needs {} bytes but its pointer interval owns {}",
                        unit.id,
                        encoded.len(),
                        owned_byte_count
                    ));
                } else {
                    patched[interval[0]..interval[1]].fill(0);
                    patched[interval[0]..interval[0] + encoded.len()].copy_from_slice(&encoded);
                    expected_write_ranges.push(interval);
                }
                encoded.len()
            }
        };
        unit_plans.push(BonusShopTextOverlayUnitPlan {
            id: unit.id.clone(),
            source_record_byte_count: unit.source.source_record_byte_count,
            output_record_byte_count,
            changed: source[interval[0]..interval[1]] != patched[interval[0]..interval[1]],
        });
    }

    ensure!(
        capacity_errors.is_empty(),
        "authored bonus-shop units exceed their pointer intervals:\n{}",
        capacity_errors.join("\n")
    );

    expected_write_ranges.sort_unstable();
    ensure!(
        expected_write_ranges
            .windows(2)
            .all(|pair| pair[0][1] <= pair[1][0]),
        "authored bonus-shop pointer intervals overlap"
    );
    let changed_byte_ranges = difference_ranges(source, &patched);
    ensure!(
        changed_byte_ranges
            .iter()
            .all(|range| range_is_covered(*range, &expected_write_ranges)),
        "bonus-shop text overlay plan changed bytes outside authored pointer intervals"
    );
    ensure!(
        authored_unit_count != 0 || changed_byte_ranges.is_empty(),
        "an untranslated-only bonus-shop overlay plan changed source bytes"
    );

    Ok(BonusShopTextOverlayPlan {
        bytes: patched,
        authored_unit_count,
        untranslated_unit_count,
        expected_write_ranges,
        changed_byte_ranges,
        units: unit_plans,
    })
}

pub(super) fn validate_message_width(text: &str, role: BonusShopFontRole) -> Result<()> {
    if role == BonusShopFontRole::ProductLabel {
        return Ok(());
    }
    // KOUBAI's validated command renderer resets x to 80 and advances both
    // glyphs and blanks by 20. Twenty cells fit before the right message border;
    // a twenty-first cell visibly crosses it. Font ink width cannot replace
    // this native fixed-advance limit. Product-list labels use another window.
    for (line, text) in text.split('\n').enumerate() {
        ensure!(
            text.chars().count() <= 20,
            "message line {} exceeds 20 native cells (spaces included)",
            line + 1
        );
    }
    Ok(())
}

fn validate_source_interval(source: &[u8], unit: &BonusShopTextSourceUnit) -> Result<[usize; 2]> {
    let start = parse_hex_offset(&unit.source_offset, "source offset")?;
    let record_end = parse_hex_offset(&unit.source_record_end_offset, "source record end")?;
    let interval_end = parse_hex_offset(
        &unit.source_pointer_interval_end_offset,
        "source pointer interval end",
    )?;
    ensure!(
        start < record_end && record_end <= interval_end && interval_end <= source.len(),
        "bonus-shop unit {} has an invalid source pointer interval",
        unit.unit_id
    );
    ensure!(
        record_end - start == unit.source_record_byte_count
            && interval_end - start == unit.source_pointer_interval_byte_count
            && interval_end - record_end == unit.trailing_interval_byte_count,
        "bonus-shop unit {} pointer interval measurements changed",
        unit.unit_id
    );
    ensure!(
        sha256_bytes(&source[start..record_end]) == unit.source_record_sha256,
        "bonus-shop unit {} source record bytes changed",
        unit.unit_id
    );
    let trailing = &source[record_end..interval_end];
    ensure!(
        sha256_bytes(trailing) == unit.trailing_interval_sha256
            && trailing.iter().all(|byte| *byte == 0) == unit.trailing_interval_all_zero,
        "bonus-shop unit {} trailing pointer-interval bytes changed",
        unit.unit_id
    );
    Ok([start, interval_end])
}

fn encode_bonus_shop_text(
    text: &str,
    font_role: BonusShopFontRole,
    glyph_codes: &BTreeMap<BonusShopFontRole, BTreeMap<char, u16>>,
) -> Result<Vec<u8>> {
    let role_codes = glyph_codes.get(&font_role);
    let mut output = Vec::with_capacity(text.chars().count() * 3 + 1);
    for character in text.chars() {
        match character {
            ' ' => output.extend_from_slice(&BLANK_COMMAND),
            '\n' => output.push(LINE_BREAK_COMMAND),
            character => {
                ensure!(
                    !character.is_control(),
                    "bonus-shop text contains unsupported control character {character:?}"
                );
                let code = role_codes
                    .and_then(|codes| codes.get(&character))
                    .with_context(|| {
                        format!("bonus-shop text has no allocated glyph for {character:?}")
                    })?;
                output.extend_from_slice(&glyph_command(*code)?);
            }
        }
    }
    output.push(TERMINATOR_COMMAND);
    Ok(output)
}

pub(super) fn glyph_command(code: u16) -> Result<[u8; 3]> {
    ensure!(
        code <= 0x03ff,
        "bonus-shop glyph code 0x{code:04x} leaves the four-page atlas"
    );
    Ok([
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ])
}

fn parse_hex_offset(value: &str, label: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .with_context(|| format!("{label} is not hexadecimal"))?,
        16,
    )
    .with_context(|| format!("invalid {label}"))
}

fn range_is_covered(range: [usize; 2], allowed: &[[usize; 2]]) -> bool {
    (range[0]..range[1]).all(|offset| {
        allowed
            .iter()
            .any(|[start, end]| *start <= offset && offset < *end)
    })
}
