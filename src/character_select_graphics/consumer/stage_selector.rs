//! Exact PLSEL1 stage-selector records and their absolute-pointer table.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::pipeline::sha256_bytes;
use crate::tim::Cell;

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectDescriptorBuild, CharacterSelectFontRole,
    CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource, CharacterSelectRouteOccurrence,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::SHARED_ATLAS_OFFSET;

const TEXTURE_PAGE_WIDTH: usize = 256;
const GLYPH_SIZE: usize = 20;
const SOURCE_PATH: &str = "DAT1/PLSEL1.BIN";
const TEXTURE_SOURCE_PATH: &str = "DAT2/SELP1.BIZ";

pub(super) struct StageLabelSpec {
    pub(super) source_ui_id: &'static str,
    pub(super) route_occurrence_id: &'static str,
    pub(super) offset: usize,
    pub(super) expected: &'static [u8],
    pointer: StageLabelPointer,
}

#[derive(Clone, Copy)]
enum StageLabelPointer {
    Table {
        offset: usize,
        expected_runtime_pointer: u32,
    },
    Materialized {
        lui_offset: usize,
        addiu_offset: usize,
        register: Register,
        expected_runtime_pointer: u32,
    },
}

const STAGE_00: &[u8] = &[
    0x0b, 0x0b, 0x02, 0x0a, 0x02, 0x07, 0x03, 0x08, 0x03, 0x08, 0x04, 0x09, 0x04, 0x09, 0x02, 0x0a,
    0x04, 0x07, 0x01, 0x02, 0x02, 0x03, 0x02,
];
const STAGE_01: &[u8] = &[
    0x0c, 0x00, 0x06, 0x01, 0x06, 0x02, 0x06, 0x03, 0x06, 0x04, 0x06, 0x05, 0x06, 0x06, 0x06, 0x06,
    0x04, 0x01, 0x03, 0x07, 0x04, 0x04, 0x03, 0x00, 0x03,
];
const STAGE_02: &[u8] = &[
    0x09, 0x00, 0x06, 0x01, 0x06, 0x02, 0x06, 0x03, 0x06, 0x07, 0x06, 0x05, 0x06, 0x06, 0x06, 0x08,
    0x06, 0x09, 0x06,
];
const STAGE_03: &[u8] = &[
    0x09, 0x00, 0x06, 0x01, 0x06, 0x02, 0x06, 0x03, 0x06, 0x07, 0x06, 0x05, 0x06, 0x06, 0x06, 0x0a,
    0x06, 0x0b, 0x06,
];
const STAGE_04: &[u8] = &[
    0x09, 0x00, 0x07, 0x01, 0x07, 0x02, 0x07, 0x03, 0x07, 0x04, 0x07, 0x05, 0x07, 0x06, 0x07, 0x07,
    0x07, 0x08, 0x07,
];
const STAGE_05: &[u8] = &[
    0x0a, 0x00, 0x07, 0x01, 0x07, 0x02, 0x07, 0x03, 0x07, 0x00, 0x08, 0x01, 0x08, 0x02, 0x08, 0x03,
    0x08, 0x04, 0x08, 0x09, 0x07,
];
const STAGE_06: &[u8] = &[
    0x0b, 0x05, 0x02, 0x06, 0x02, 0x07, 0x02, 0x08, 0x02, 0x04, 0x04, 0x09, 0x02, 0x0b, 0x00, 0x00,
    0x01, 0x0a, 0x07, 0x0b, 0x07, 0x09, 0x07,
];
const STAGE_07: &[u8] = &[
    0x0a, 0x00, 0x03, 0x01, 0x03, 0x02, 0x03, 0x03, 0x03, 0x02, 0x03, 0x04, 0x03, 0x06, 0x02, 0x05,
    0x03, 0x06, 0x03, 0x07, 0x03,
];
const STAGE_08: &[u8] = &[
    0x09, 0x00, 0x09, 0x01, 0x09, 0x02, 0x07, 0x03, 0x07, 0x0a, 0x07, 0x06, 0x09, 0x07, 0x09, 0x08,
    0x09, 0x09, 0x09,
];
const STAGE_09: &[u8] = &[
    0x08, 0x00, 0x09, 0x01, 0x09, 0x02, 0x07, 0x03, 0x07, 0x09, 0x07, 0x02, 0x09, 0x03, 0x09, 0x04,
    0x09,
];
const STAGE_10: &[u8] = &[
    0x0b, 0x08, 0x03, 0x09, 0x03, 0x0b, 0x02, 0x0a, 0x02, 0x08, 0x02, 0x0b, 0x02, 0x02, 0x06, 0x03,
    0x06, 0x0a, 0x08, 0x0b, 0x07, 0x09, 0x07,
];
const STAGE_11: &[u8] = &[
    0x0b, 0x08, 0x03, 0x09, 0x03, 0x0b, 0x02, 0x0a, 0x02, 0x08, 0x02, 0x0b, 0x02, 0x02, 0x06, 0x03,
    0x06, 0x0a, 0x09, 0x0b, 0x09, 0x08, 0x07,
];
const STAGE_12: &[u8] = &[
    0x0d, 0x08, 0x03, 0x09, 0x03, 0x0b, 0x02, 0x0a, 0x02, 0x08, 0x02, 0x0b, 0x02, 0x02, 0x06, 0x03,
    0x06, 0x05, 0x08, 0x06, 0x08, 0x07, 0x08, 0x08, 0x08, 0x09, 0x08,
];
const STAGE_13: &[u8] = &[
    0x0a, 0x00, 0x07, 0x01, 0x07, 0x02, 0x07, 0x03, 0x07, 0x05, 0x04, 0x07, 0x03, 0x00, 0x44, 0x01,
    0x44, 0x02, 0x03, 0x00, 0x03,
];
const STAGE_START: &[u8] = &[0x04, 0x0b, 0x02, 0x0b, 0x03, 0x07, 0x03, 0x0a, 0x04];
const STAGE_OPTIONS: &[u8] = &[
    0x05, 0x0a, 0x0b, 0x05, 0x04, 0x06, 0x02, 0x0b, 0x0b, 0x04, 0x03,
];

macro_rules! stage {
    ($id:literal, $index:literal, $offset:literal, $expected:ident) => {
        StageLabelSpec {
            source_ui_id: $id,
            route_occurrence_id: concat!("plsel1-stage-selector-", $index),
            offset: $offset,
            expected: $expected,
            pointer: StageLabelPointer::Table {
                offset: 0x06f4 + 4 * $index,
                expected_runtime_pointer: 0x800a_2000 + $offset,
            },
        }
    };
}

pub(super) const STAGE_LABELS: &[StageLabelSpec] = &[
    stage!("stage_select_disabled", 0, 0x05a4, STAGE_00),
    stage!("stage_taiyo_middle_school_ground", 1, 0x05bc, STAGE_01),
    stage!("stage_taiyo_high_school_classroom", 2, 0x05d8, STAGE_02),
    stage!("stage_taiyo_high_school_rooftop", 3, 0x05ec, STAGE_03),
    stage!("stage_gorin_first_gymnasium", 4, 0x0600, STAGE_04),
    stage!("stage_gorin_seaside_lodge", 5, 0x0614, STAGE_05),
    stage!("stage_pacific_back_gate", 6, 0x062c, STAGE_06),
    stage!("stage_drive_in_theater", 7, 0x0644, STAGE_07),
    stage!("stage_gedo_construction_site", 8, 0x065c, STAGE_08),
    stage!("stage_gedo_riverbank", 9, 0x0670, STAGE_09),
    stage!("stage_justice_front_gate", 10, 0x0684, STAGE_10),
    stage!("stage_justice_library", 11, 0x069c, STAGE_11),
    stage!("stage_justice_student_council_room", 12, 0x06c0, STAGE_12),
    stage!("stage_gorin_poolside", 13, 0x06dc, STAGE_13),
    StageLabelSpec {
        source_ui_id: "stage_start_label",
        route_occurrence_id: "plsel1-stage-selector-start",
        offset: 0x073c,
        expected: STAGE_START,
        pointer: StageLabelPointer::Materialized {
            lui_offset: 0x4948,
            addiu_offset: 0x494c,
            register: Register::A1,
            expected_runtime_pointer: 0x800a_273c,
        },
    },
    StageLabelSpec {
        source_ui_id: "stage_options_label",
        route_occurrence_id: "plsel1-stage-selector-options",
        offset: 0x0748,
        expected: STAGE_OPTIONS,
        pointer: StageLabelPointer::Materialized {
            lui_offset: 0x4968,
            addiu_offset: 0x496c,
            register: Register::A1,
            expected_runtime_pointer: 0x800a_2748,
        },
    },
];

pub(super) const STAGE_POINTER_TABLE: &[u8] = &[
    0xa4, 0x25, 0x0a, 0x80, 0xbc, 0x25, 0x0a, 0x80, 0xd8, 0x25, 0x0a, 0x80, 0xec, 0x25, 0x0a, 0x80,
    0x00, 0x26, 0x0a, 0x80, 0x14, 0x26, 0x0a, 0x80, 0x2c, 0x26, 0x0a, 0x80, 0x44, 0x26, 0x0a, 0x80,
    0x5c, 0x26, 0x0a, 0x80, 0x70, 0x26, 0x0a, 0x80, 0x84, 0x26, 0x0a, 0x80, 0x9c, 0x26, 0x0a, 0x80,
    0xc0, 0x26, 0x0a, 0x80, 0xdc, 0x26, 0x0a, 0x80,
];
const TRAILING_DUPLICATE_POINTER_OFFSET: usize = 0x072c;
const TRAILING_DUPLICATE_POINTER: &[u8; 4] = &[0xdc, 0x26, 0x0a, 0x80];

pub(super) fn source_ui_ids() -> impl Iterator<Item = &'static str> {
    STAGE_LABELS.iter().map(|spec| spec.source_ui_id)
}

pub(super) fn route_occurrences(
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Vec<CharacterSelectRouteOccurrence> {
    STAGE_LABELS
        .iter()
        .filter(|spec| {
            localized_sources
                .iter()
                .any(|source| source.source_ui_id == spec.source_ui_id)
        })
        .map(|spec| {
            let mut consumer_offsets = vec![spec.offset];
            match spec.pointer {
                StageLabelPointer::Table { offset, .. } => consumer_offsets.push(offset),
                StageLabelPointer::Materialized {
                    lui_offset,
                    addiu_offset,
                    ..
                } => consumer_offsets.extend([lui_offset, addiu_offset]),
            }
            bound_occurrence(
                spec.route_occurrence_id,
                "selp1_stage_label_grid",
                [spec.source_ui_id],
                vec![producer_target(
                    TEXTURE_SOURCE_PATH,
                    Some(SHARED_ATLAS_OFFSET),
                    Some(CharacterSelectTextureSurface::StageLabelAtlas),
                    ["stage-label-glyph-allocations"],
                )],
                vec![consumer_target(
                    TEXTURE_SOURCE_PATH,
                    SOURCE_PATH,
                    CharacterSelectConsumerReferenceKind::DescriptorAndPointer,
                    consumer_offsets,
                    [spec.route_occurrence_id],
                )],
            )
        })
        .collect()
}

pub(super) fn rewrite(
    source_path: &str,
    source: &[u8],
    patched: &mut [u8],
    allocations: &[CharacterSelectGlyphAllocation],
    localized_sources: &[CharacterSelectLocalizedSource],
    expected_write_ranges: &mut Vec<[usize; 2]>,
    descriptors: &mut Vec<CharacterSelectDescriptorBuild>,
) -> Result<()> {
    if source_path != SOURCE_PATH {
        return Ok(());
    }
    let active = STAGE_LABELS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|source| source.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    if active.is_empty() {
        return Ok(());
    }
    ensure!(
        active.len() == STAGE_LABELS.len(),
        "PLSEL1 stage-selector rewrite requires all {} labels, found {}",
        STAGE_LABELS.len(),
        active.len()
    );
    let pointer_table = source
        .get(0x06f4..0x06f4 + STAGE_POINTER_TABLE.len())
        .context("PLSEL1 stage-selector pointer table is truncated")?;
    ensure!(
        pointer_table == STAGE_POINTER_TABLE,
        "PLSEL1 stage-selector pointer table changed: expected {}, found {}",
        sha256_bytes(STAGE_POINTER_TABLE),
        sha256_bytes(pointer_table)
    );
    let trailing_pointer = source
        .get(TRAILING_DUPLICATE_POINTER_OFFSET..TRAILING_DUPLICATE_POINTER_OFFSET + 4)
        .context("PLSEL1 stage-selector trailing duplicate pointer is truncated")?;
    ensure!(
        trailing_pointer == TRAILING_DUPLICATE_POINTER,
        "PLSEL1 stage-selector trailing duplicate pointer changed at +0x{TRAILING_DUPLICATE_POINTER_OFFSET:04x}"
    );
    for (spec, entry) in active {
        let source_descriptor = source
            .get(spec.offset..spec.offset + spec.expected.len())
            .with_context(|| {
                format!(
                    "PLSEL1 stage-selector descriptor {} is truncated",
                    spec.source_ui_id
                )
            })?;
        ensure!(
            source_descriptor == spec.expected,
            "PLSEL1 stage-selector source descriptor changed for {} at +0x{:04x}",
            spec.source_ui_id,
            spec.offset
        );
        validate_stage_label_pointer(source, spec)?;
        let logical_character_count = entry.korean_text.chars().count();
        ensure!(
            entry
                .korean_text
                .chars()
                .all(|character| character == ' ' || !character.is_whitespace()),
            "stage-selector translation {} contains unsupported whitespace",
            spec.source_ui_id
        );
        let visible = entry
            .korean_text
            .chars()
            .filter(|character| *character != ' ')
            .collect::<Vec<_>>();
        let capacity = usize::from(spec.expected[0]);
        ensure!(
            visible.len() <= capacity,
            "stage-selector translation {} needs {} visible glyphs but its source record holds {}",
            spec.source_ui_id,
            visible.len(),
            capacity
        );
        let mut encoded = vec![0_u8; spec.expected.len()];
        encoded[0] = u8::try_from(visible.len())?;
        let mut pages = BTreeSet::new();
        for (index, character) in visible.iter().copied().enumerate() {
            let allocation = allocations
                .iter()
                .find(|allocation| {
                    allocation.surface == CharacterSelectTextureSurface::StageLabelAtlas
                        && allocation.font_role == CharacterSelectFontRole::StageLabel
                        && allocation.character == character
                })
                .with_context(|| {
                    format!(
                        "stage-selector translation {} has no allocation for {character:?}",
                        spec.source_ui_id
                    )
                })?;
            let [u, v] = allocation.texture_uv;
            ensure!(
                u % 20 == 0 && v % 20 == 0 && u <= 220 && v <= 220,
                "stage-selector glyph {character:?} escaped the canonical 20px grid"
            );
            let encoded_row = allocation
                .texture_page_index
                .checked_mul(20)
                .and_then(|page| page.checked_add(v / 20))
                .context("stage-selector encoded row overflow")?;
            encoded[1 + index * 2..1 + index * 2 + 2].copy_from_slice(&[u / 20, encoded_row]);
            pages.insert(allocation.texture_page_index);
        }
        patched[spec.offset..spec.offset + encoded.len()].copy_from_slice(&encoded);
        expected_write_ranges.push([spec.offset, spec.offset + spec.expected.len()]);
        descriptors.push(CharacterSelectDescriptorBuild {
            route_occurrence_id: spec.route_occurrence_id.to_string(),
            source_ui_id: spec.source_ui_id.to_string(),
            translation_id: entry.translation_id.clone(),
            descriptor_offset: spec.offset,
            logical_character_count,
            encoded_glyph_count: visible.len(),
            omitted_separator_count: logical_character_count - visible.len(),
            texture_page_indices: pages.into_iter().collect(),
        });
    }
    Ok(())
}

fn validate_stage_label_pointer(source: &[u8], spec: &StageLabelSpec) -> Result<()> {
    match spec.pointer {
        StageLabelPointer::Table {
            offset,
            expected_runtime_pointer,
        } => {
            let pointer = u32::from_le_bytes(
                source
                    .get(offset..offset + 4)
                    .context("PLSEL1 stage-selector pointer is truncated")?
                    .try_into()?,
            );
            ensure!(
                pointer == expected_runtime_pointer,
                "PLSEL1 stage-selector pointer changed for {} at +0x{offset:04x}",
                spec.source_ui_id
            );
        }
        StageLabelPointer::Materialized {
            lui_offset,
            addiu_offset,
            register,
            expected_runtime_pointer,
        } => {
            let upper = (expected_runtime_pointer >> 16) as u16;
            let lower = expected_runtime_pointer as u16 as i16;
            let expected_lui = Instruction::Lui {
                rt: register,
                immediate: upper,
            };
            let expected_addiu = Instruction::Addiu {
                rt: register,
                rs: register,
                immediate: lower,
            };
            let actual_lui = decode_overlay_instruction(source, lui_offset)?;
            let actual_addiu = decode_overlay_instruction(source, addiu_offset)?;
            ensure!(
                actual_lui == expected_lui && actual_addiu == expected_addiu,
                "PLSEL1 stage-selector materialized pointer changed for {} at +0x{lui_offset:04x}/+0x{addiu_offset:04x}",
                spec.source_ui_id
            );
        }
    }
    Ok(())
}

fn decode_overlay_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let word = u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("PLSEL1 instruction at +0x{offset:04x} is truncated"))?
            .try_into()?,
    );
    decode(word, 0x800a_2000 + u32::try_from(offset)?)
        .with_context(|| format!("failed to decode PLSEL1 instruction at +0x{offset:04x}"))
}

pub(super) fn source_glyph_cells() -> Result<Vec<(&'static str, Cell)>> {
    let mut cells = Vec::new();
    for spec in STAGE_LABELS {
        let count = usize::from(spec.expected[0]);
        ensure!(
            spec.expected.len() == 1 + count * 2,
            "stage selector {} source descriptor has an invalid count",
            spec.source_ui_id
        );
        for pair in spec.expected[1..].as_chunks::<2>().0 {
            cells.push((spec.source_ui_id, decode_cell(pair[0], pair[1])?));
        }
    }
    Ok(cells)
}

pub(super) fn decode_cell(column: u8, encoded_row: u8) -> Result<Cell> {
    let page = usize::from(encoded_row / 20);
    let row = usize::from(encoded_row % 20);
    ensure!(
        page < 4 && usize::from(column) < 12 && row < 12,
        "stage-selector glyph code escaped the shared 20px grid: column={column}, row={encoded_row}"
    );
    Ok(Cell {
        x: page * TEXTURE_PAGE_WIDTH + usize::from(column) * GLYPH_SIZE,
        y: row * GLYPH_SIZE,
        width: GLYPH_SIZE,
        height: GLYPH_SIZE,
    })
}
