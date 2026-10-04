//! Source-bound CDEMO contract for the indexed TITLE.BIN episode-card family.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

pub(super) const CONSUMER_RECORD: &str = "DAT1/CDEMO.BIN";
pub(super) const TITLE_MEMBER_SELECTOR_RAM_OFFSET: usize = 0x1f_64be;
pub(super) const TITLE_MEMBER_IMAGE_HEIGHTS: [usize; 8] = [144, 112, 128, 112, 128, 176, 128, 112];
pub(super) const PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS: [usize; 14] = [
    0x0010, 0x07b0, 0x0870, 0x09f8, 0x0a00, 0x0a08, 0x0a14, 0x0a24, 0x0a64, 0x0a78, 0x0a80, 0x0b14,
    0x0b98, 0x0ba0,
];

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const LAYOUT_POINTER_TABLE_OFFSET: usize = 0x07b0;
const LAYOUT_POINTER_TABLE_RUNTIME_LOW: i16 = 0x27b0;
const ACTIVE_PHASE_COUNT: usize = 6;
const ACTIVE_LAYOUT_COUNT: usize = TITLE_MEMBER_IMAGE_HEIGHTS.len() * ACTIVE_PHASE_COUNT;
const LAYOUT_POINTER_COUNT: usize = ACTIVE_LAYOUT_COUNT + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrimitiveLayoutRecord {
    texture_v: i16,
    width: i16,
    height: i16,
    screen_x: i16,
    screen_y: i16,
}

const HEADING_LAYOUT_RECORD: PrimitiveLayoutRecord = PrimitiveLayoutRecord {
    texture_v: 0,
    width: 120,
    height: 40,
    screen_x: -60,
    screen_y: -76,
};

pub(super) fn validate_solo_episode_card_consumer(overlay: &[u8]) -> Result<()> {
    validate_member_selection_and_layout_reader(overlay)?;
    validate_layout_table(overlay)
}

fn validate_member_selection_and_layout_reader(overlay: &[u8]) -> Result<()> {
    for (offset, expected) in member_selection_and_layout_reader_instructions() {
        let actual = decode_instruction(overlay, offset)?;
        ensure!(
            actual == expected,
            "CDEMO TITLE member-selection or layout-reader contract changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

fn member_selection_and_layout_reader_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const AT: Register = Register::AT;
    const RA: Register = Register::RA;
    const S1: Register = Register::S1;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S5: Register = Register::S5;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // The animation phase contributes the eight-member stride. The exact
        // member byte then selects both the layout entry and TITLE.BIN stream.
        sll(0x09f8, V1, V1, 3),
        lui(0x09fc, V0, 0x801f),
        lbu(
            0x0a00,
            V0,
            V0,
            TITLE_MEMBER_SELECTOR_RAM_OFFSET as u16 as i16,
        ),
        addu(0x0a08, V0, V0, V1),
        addu(0x0a0c, A2, V0, ZERO),
        sh(0x0a14, A2, S5, 2),
        lui(0x0a20, A2, 0x801f),
        lbu(
            0x0a24,
            A2,
            A2,
            TITLE_MEMBER_SELECTOR_RAM_OFFSET as u16 as i16,
        ),
        lui(0x0a28, A0, 0x800d),
        lui(0x0a2c, V0, 0x801f),
        lw(0x0a30, V0, V0, 0x6360),
        ori(0x0a34, A0, A0, 0x4000),
        lw(0x0a38, V0, V0, 0x015c),
        (0x0a3c, Instruction::nop()),
        (0x0a40, Instruction::Jalr { rd: RA, rs: V0 }),
        addiu(0x0a44, A1, ZERO, 0x024a),
        // The stored layout index addresses the 49-entry pointer table. Each
        // record begins with a primitive count followed by five signed fields:
        // source Y, width, height, screen X, and screen Y.
        lh(0x0a64, V0, S5, 2),
        sll(0x0a6c, V0, V0, 2),
        lui(0x0a70, AT, 0x800a),
        addu(0x0a74, AT, AT, V0),
        lw(0x0a78, S1, AT, LAYOUT_POINTER_TABLE_RUNTIME_LOW),
        lhu(0x0a80, V0, S1, 0),
        addiu(0x0a84, S1, S1, 2),
        lh(0x0b14, A2, S1, 0),
        lh(0x0b1c, S4, S1, 0),
        lh(0x0b24, S3, S1, 0),
        lh(0x0b98, V0, S1, 0),
        lh(0x0ba0, V1, S1, 0),
    ]
}

fn validate_layout_table(overlay: &[u8]) -> Result<()> {
    let table_end = LAYOUT_POINTER_TABLE_OFFSET
        .checked_add(LAYOUT_POINTER_COUNT * 4)
        .context("CDEMO TITLE layout pointer-table range overflow")?;
    ensure!(
        table_end <= overlay.len(),
        "CDEMO TITLE layout pointer table is truncated"
    );

    let mut layout_offsets = Vec::with_capacity(LAYOUT_POINTER_COUNT);
    let mut distinct_offsets = BTreeSet::new();
    for layout_index in 0..LAYOUT_POINTER_COUNT {
        let pointer_offset = LAYOUT_POINTER_TABLE_OFFSET + layout_index * 4;
        let runtime_address = read_u32(overlay, pointer_offset)?;
        let relative = runtime_address
            .checked_sub(OVERLAY_RUNTIME_BASE)
            .with_context(|| {
                format!(
                    "CDEMO TITLE layout {layout_index} points below its overlay: 0x{runtime_address:08x}"
                )
            })?;
        let relative = usize::try_from(relative)?;
        ensure!(
            relative < LAYOUT_POINTER_TABLE_OFFSET,
            "CDEMO TITLE layout {layout_index} points outside the layout-record region: +0x{relative:04x}"
        );
        ensure!(
            distinct_offsets.insert(relative),
            "CDEMO TITLE layout {layout_index} aliases another layout at +0x{relative:04x}"
        );
        if let Some(previous) = layout_offsets.last() {
            ensure!(
                *previous < relative,
                "CDEMO TITLE layout pointers are not ordered at index {layout_index}"
            );
        }
        layout_offsets.push(relative);
    }

    for (layout_index, &layout_offset) in layout_offsets.iter().enumerate() {
        let (primitives, record_end) = read_layout_record(overlay, layout_offset)?;
        let next_boundary = layout_offsets
            .get(layout_index + 1)
            .copied()
            .unwrap_or(LAYOUT_POINTER_TABLE_OFFSET);
        ensure!(
            record_end <= next_boundary,
            "CDEMO TITLE layout {layout_index} overlaps the following record"
        );
        if layout_index == ACTIVE_LAYOUT_COUNT {
            ensure!(
                primitives.len() == 1
                    && primitives[0].texture_v == 0
                    && primitives[0].width == 120
                    && primitives[0].height == 40,
                "CDEMO TITLE terminal layout no longer retains the heading texture region"
            );
            continue;
        }

        let member_index = layout_index % TITLE_MEMBER_IMAGE_HEIGHTS.len();
        let phase_index = layout_index / TITLE_MEMBER_IMAGE_HEIGHTS.len();
        ensure!(
            primitives.first() == Some(&HEADING_LAYOUT_RECORD),
            "CDEMO TITLE member {member_index} phase {phase_index} lost its proven heading texture region"
        );
        for primitive in primitives {
            ensure!(
                primitive.texture_v >= 0
                    && primitive.width > 0
                    && primitive.width <= 256
                    && primitive.height > 0,
                "CDEMO TITLE member {member_index} phase {phase_index} has an invalid primitive layout record: {primitive:?}"
            );
        }
    }
    Ok(())
}

fn read_layout_record(
    overlay: &[u8],
    layout_offset: usize,
) -> Result<(Vec<PrimitiveLayoutRecord>, usize)> {
    let primitive_count = usize::from(read_u16(overlay, layout_offset)?);
    ensure!(
        primitive_count > 0,
        "CDEMO TITLE layout at +0x{layout_offset:04x} has no primitives"
    );
    let mut cursor = layout_offset + 2;
    let mut primitives = Vec::with_capacity(primitive_count);
    for primitive_index in 0..primitive_count {
        let record = overlay.get(cursor..cursor + 10).with_context(|| {
            format!(
                "CDEMO TITLE primitive {primitive_index} at +0x{layout_offset:04x} is truncated"
            )
        })?;
        primitives.push(PrimitiveLayoutRecord {
            texture_v: i16::from_le_bytes(record[0..2].try_into()?),
            width: i16::from_le_bytes(record[2..4].try_into()?),
            height: i16::from_le_bytes(record[4..6].try_into()?),
            screen_x: i16::from_le_bytes(record[6..8].try_into()?),
            screen_y: i16::from_le_bytes(record[8..10].try_into()?),
        });
        cursor += 10;
    }
    Ok((primitives, cursor))
}

fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let word = read_u32(source, offset)?;
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode CDEMO instruction at +0x{offset:04x}"))
}

fn read_u16(source: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        source
            .get(offset..offset + 2)
            .with_context(|| format!("truncated CDEMO halfword at +0x{offset:04x}"))?
            .try_into()?,
    ))
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated CDEMO word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}

fn addiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Addiu { rt, rs, immediate })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
}

fn lbu(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lbu {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn lh(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lh {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn lhu(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lhu {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn lui(offset: usize, rt: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Lui { rt, immediate })
}

fn lw(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lw {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn ori(offset: usize, rt: Register, rs: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Ori { rt, rs, immediate })
}

fn sh(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Sh {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

#[cfg(test)]
mod tests {
    use psx_r3000a::{Instruction, Register, encode};

    use super::{
        ACTIVE_LAYOUT_COUNT, HEADING_LAYOUT_RECORD, LAYOUT_POINTER_COUNT,
        LAYOUT_POINTER_TABLE_OFFSET, OVERLAY_RUNTIME_BASE, PrimitiveLayoutRecord,
        TITLE_MEMBER_SELECTOR_RAM_OFFSET, member_selection_and_layout_reader_instructions,
        validate_solo_episode_card_consumer,
    };

    #[test]
    fn title_loader_uses_the_byte_that_indexes_the_layout_family() {
        let source = valid_consumer_fixture(true);
        validate_solo_episode_card_consumer(&source).unwrap();

        let mut drifted = source;
        write_instruction(
            &mut drifted,
            0x0a24,
            Instruction::Lbu {
                rt: Register::A2,
                base: Register::A2,
                offset: 0x64c0,
            },
        );

        let error = validate_solo_episode_card_consumer(&drifted).unwrap_err();
        assert!(error.to_string().contains("member-selection"));
        assert_eq!(TITLE_MEMBER_SELECTOR_RAM_OFFSET, 0x1f_64be);
    }

    #[test]
    fn heading_only_layout_is_sufficient_for_the_observed_presentation_contract() {
        let source = valid_consumer_fixture(false);
        validate_solo_episode_card_consumer(&source).unwrap();
    }

    #[test]
    fn additional_layout_records_remain_structural_evidence_not_visibility_proof() {
        let source = valid_consumer_fixture(true);
        validate_solo_episode_card_consumer(&source).unwrap();

        let mut changed_heading = source;
        let first_layout = read_fixture_u32(&changed_heading, LAYOUT_POINTER_TABLE_OFFSET)
            .checked_sub(OVERLAY_RUNTIME_BASE)
            .unwrap() as usize;
        changed_heading[first_layout + 2..first_layout + 4].copy_from_slice(&1_i16.to_le_bytes());

        let error = validate_solo_episode_card_consumer(&changed_heading).unwrap_err();
        assert!(error.to_string().contains("proven heading texture region"));
    }

    #[test]
    fn layout_entries_cannot_alias_one_another() {
        let source = valid_consumer_fixture(true);
        validate_solo_episode_card_consumer(&source).unwrap();

        let mut aliased = source;
        let first_pointer = read_fixture_u32(&aliased, LAYOUT_POINTER_TABLE_OFFSET);
        aliased[LAYOUT_POINTER_TABLE_OFFSET + 4..LAYOUT_POINTER_TABLE_OFFSET + 8]
            .copy_from_slice(&first_pointer.to_le_bytes());

        let error = validate_solo_episode_card_consumer(&aliased).unwrap_err();
        assert!(error.to_string().contains("aliases another layout"));
    }

    fn valid_consumer_fixture(include_additional_records: bool) -> Vec<u8> {
        let mut source = vec![0_u8; 0x10d8];
        for (offset, instruction) in member_selection_and_layout_reader_instructions() {
            write_instruction(&mut source, offset, instruction);
        }

        let mut record_offset = 0x10usize;
        for layout_index in 0..LAYOUT_POINTER_COUNT {
            let pointer_offset = LAYOUT_POINTER_TABLE_OFFSET + layout_index * 4;
            source[pointer_offset..pointer_offset + 4].copy_from_slice(
                &(OVERLAY_RUNTIME_BASE + u32::try_from(record_offset).unwrap()).to_le_bytes(),
            );
            let primitives = if layout_index == ACTIVE_LAYOUT_COUNT {
                vec![PrimitiveLayoutRecord {
                    screen_y: -20,
                    ..HEADING_LAYOUT_RECORD
                }]
            } else if !include_additional_records {
                vec![HEADING_LAYOUT_RECORD]
            } else {
                vec![
                    HEADING_LAYOUT_RECORD,
                    PrimitiveLayoutRecord {
                        texture_v: 40,
                        width: 120,
                        height: 40,
                        screen_x: -60,
                        screen_y: -20,
                    },
                    PrimitiveLayoutRecord {
                        texture_v: 80,
                        width: 120,
                        height: 24,
                        screen_x: -60,
                        screen_y: 36,
                    },
                ]
            };
            source[record_offset..record_offset + 2]
                .copy_from_slice(&(primitives.len() as u16).to_le_bytes());
            record_offset += 2;
            for primitive in primitives {
                for value in [
                    primitive.texture_v,
                    primitive.width,
                    primitive.height,
                    primitive.screen_x,
                    primitive.screen_y,
                ] {
                    source[record_offset..record_offset + 2].copy_from_slice(&value.to_le_bytes());
                    record_offset += 2;
                }
            }
        }
        assert!(record_offset <= LAYOUT_POINTER_TABLE_OFFSET);
        source
    }

    fn write_instruction(target: &mut [u8], offset: usize, instruction: Instruction) {
        let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
        target[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    fn read_fixture_u32(source: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap())
    }
}
