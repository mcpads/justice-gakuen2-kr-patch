use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};

use crate::pipeline::difference_ranges;

use super::source::{SOURCE_SUFFIX_GLYPH_CODE, SUFFIX_GLYPH_CELL};

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const CONSUMER_RUNTIME_ADDRESS: u32 = 0x800a_a52c;
pub(super) const SUFFIX_TABLE_OFFSET: usize = 0x166c;
pub(super) const CURRENT_PAGE_LOOKUP_OFFSET: usize = 0x1419;
pub(super) const TOTAL_PAGE_LOOKUP_OFFSET: usize = 0x141c;
pub(super) const SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET: usize = 0x86dc;
pub(super) const SOURCE_SUFFIX_SPRITE_COUNT: usize = 4;
pub(super) const OUTPUT_SUFFIX_SPRITE_COUNT: usize = 2;

const SOURCE_SUFFIX_SPRITES: [u8; 16] = [
    1, 8, 9, 5, // `/`, screen x = 196 + 1 * 20
    3, 9, 10, 7, // `ペ`, repurposed as `쪽`, screen x = 196 + 3 * 20
    4, 8, 4, 6, // `ー`, no longer consumed
    5, 9, 5, 6, // `ジ`, no longer consumed
];
const SOURCE_PAGE_DIGIT_COLUMNS: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const SUFFIX_GLYPH_SPRITE_SELECTOR: [u8; 3] = [9, 10, 7];
const SUFFIX_GLYPH_STRING_SELECTOR: [u8; 3] = [1, 10, 7];
const SUFFIX_GLYPH_SPRITE_SELECTOR_OFFSET: usize = SUFFIX_TABLE_OFFSET + 5;

#[derive(Debug)]
pub(super) struct PatchedPageIndicatorOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
}

pub(super) fn patch_page_indicator_overlay(source: &[u8]) -> Result<PatchedPageIndicatorOverlay> {
    validate_source_consumer(source)?;
    let mut patched = source.to_vec();
    let instruction = Instruction::Slti {
        rt: Register::V0,
        rs: Register::S0,
        immediate: OUTPUT_SUFFIX_SPRITE_COUNT as i16,
    };
    let word = encode(
        &instruction,
        OVERLAY_RUNTIME_BASE + SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET as u32,
    )?;
    patched[SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET..SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET + 4]
        .copy_from_slice(&word.to_le_bytes());

    let expected_write_ranges = vec![[
        SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
        SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET + 4,
    ]];
    let changed_byte_ranges = difference_ranges(source, &patched);
    ensure!(
        !changed_byte_ranges.is_empty()
            && changed_byte_ranges.iter().all(|[start, end]| {
                expected_write_ranges
                    .iter()
                    .any(|[allowed_start, allowed_end]| {
                        allowed_start <= start && end <= allowed_end
                    })
            }),
        "page-indicator overlay changed bytes outside its loop bound"
    );
    ensure!(
        decode_instruction(&patched, SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET)? == instruction,
        "page-indicator suffix loop patch did not decode as two sprites"
    );
    ensure!(
        patched[SUFFIX_TABLE_OFFSET..SUFFIX_TABLE_OFFSET + SOURCE_SUFFIX_SPRITES.len()]
            == SOURCE_SUFFIX_SPRITES,
        "page-indicator suffix table changed while reducing its active count"
    );
    ensure!(
        patched[CURRENT_PAGE_LOOKUP_OFFSET
            ..CURRENT_PAGE_LOOKUP_OFFSET + SOURCE_PAGE_DIGIT_COLUMNS.len()]
            == SOURCE_PAGE_DIGIT_COLUMNS,
        "page-indicator dynamic page digit columns changed"
    );

    Ok(PatchedPageIndicatorOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
    })
}

pub(super) fn validate_source_consumer(source: &[u8]) -> Result<()> {
    ensure!(
        source.get(SUFFIX_TABLE_OFFSET..SUFFIX_TABLE_OFFSET + SOURCE_SUFFIX_SPRITES.len())
            == Some(SOURCE_SUFFIX_SPRITES.as_slice()),
        "KOUBAI2 page-indicator suffix sprite table changed"
    );
    let slash_record = &SOURCE_SUFFIX_SPRITES[..4];
    let korean_suffix_record = &SOURCE_SUFFIX_SPRITES[4..8];
    ensure!(
        sprite_glyph_code(slash_record)? == 0x0059
            && sprite_glyph_code(korean_suffix_record)? == SOURCE_SUFFIX_GLYPH_CODE
            && sprite_glyph_cell(korean_suffix_record)? == SUFFIX_GLYPH_CELL,
        "KOUBAI2 suffix table no longer addresses the bound KOUBAI1 glyph cells"
    );
    let (sprite_offsets, string_offsets) = suffix_glyph_reference_offsets(source);
    ensure!(
        sprite_offsets == [SUFFIX_GLYPH_SPRITE_SELECTOR_OFFSET] && string_offsets.is_empty(),
        "KOUBAI1 suffix glyph 0x017a is no longer exclusive to the page-indicator sprite"
    );
    ensure!(
        source.get(
            CURRENT_PAGE_LOOKUP_OFFSET
                ..CURRENT_PAGE_LOOKUP_OFFSET + SOURCE_PAGE_DIGIT_COLUMNS.len()
        ) == Some(SOURCE_PAGE_DIGIT_COLUMNS.as_slice())
            && source.get(TOTAL_PAGE_LOOKUP_OFFSET) == Some(&3),
        "KOUBAI2 dynamic page digit lookup table changed"
    );
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 page-indicator consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

pub(super) fn suffix_glyph_reference_offsets(source: &[u8]) -> (Vec<usize>, Vec<usize>) {
    (
        byte_pattern_offsets(source, &SUFFIX_GLYPH_SPRITE_SELECTOR),
        byte_pattern_offsets(source, &SUFFIX_GLYPH_STRING_SELECTOR),
    )
}

fn byte_pattern_offsets(source: &[u8], pattern: &[u8]) -> Vec<usize> {
    source
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == pattern).then_some(offset))
        .collect()
}

fn sprite_glyph_code(record: &[u8]) -> Result<u16> {
    ensure!(
        record.len() == 4,
        "page-indicator sprite record is truncated"
    );
    let page = record[1]
        .checked_sub(8)
        .context("page-indicator sprite left the KOUBAI1 glyph texture pages")?;
    ensure!(
        record[2] < 16 && record[3] < 16,
        "page-indicator sprite glyph coordinate exceeds the 16x16 code grid"
    );
    Ok(u16::from(page) << 8 | u16::from(record[3]) << 4 | u16::from(record[2]))
}

fn sprite_glyph_cell(record: &[u8]) -> Result<crate::tim::Cell> {
    let code = sprite_glyph_code(record)?;
    let page = usize::from(code >> 8);
    let column = usize::from(code & 0x000f);
    let row = usize::from((code >> 4) & 0x000f);
    Ok(crate::tim::Cell {
        x: page * 256 + ((column * 20) & 0xff),
        y: (row * 20) & 0xff,
        width: 20,
        height: 20,
    })
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            0x852c,
            Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: -56,
            },
        ),
        (
            0x8544,
            Instruction::Lui {
                rt: Register::FP,
                immediate: 0x800a,
            },
        ),
        (
            0x8548,
            Instruction::Addiu {
                rt: Register::FP,
                rs: Register::FP,
                immediate: 0x366c,
            },
        ),
        (
            0x8550,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::FP,
                immediate: 1,
            },
        ),
        (
            0x8558,
            Instruction::Addu {
                rd: Register::S5,
                rs: Register::FP,
                rt: Register::ZERO,
            },
        ),
        (
            0x85ec,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x85f0,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 4,
            },
        ),
        (
            0x8614,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::T0,
                rt: Register::V0,
            },
        ),
        (
            0x8620,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V0,
                offset: 2,
            },
        ),
        (
            0x8638,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::A2,
                offset: 3,
            },
        ),
        (
            0x8660,
            Instruction::Lbu {
                rt: Register::A2,
                base: Register::S6,
                offset: 0,
            },
        ),
        (
            0x8664,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::S6,
                immediate: 4,
            },
        ),
        (
            SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: SOURCE_SUFFIX_SPRITE_COUNT as i16,
            },
        ),
        (
            0x86e0,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: OVERLAY_RUNTIME_BASE + 0x8574,
            },
        ),
        (
            0x872c,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 196,
            },
        ),
        (
            0x8734,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 40,
            },
        ),
        (
            0x8748,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S4,
                offset: 30,
            },
        ),
        (
            0x8750,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x8754,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x8758,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x3419,
            },
        ),
        (
            0x8858,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 196,
            },
        ),
        (
            0x885c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::T0,
                immediate: 40,
            },
        ),
        (
            0x8864,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 40,
            },
        ),
        (
            0x887c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x800a,
            },
        ),
        (
            0x8880,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x341c,
            },
        ),
    ]
}

fn decode_instruction(data: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 instruction at +0x{offset:04x}"))?;
    let word = u32::from_le_bytes(bytes.try_into().expect("four-byte instruction"));
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode KOUBAI2 instruction at +0x{offset:04x}"))
}
