use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::model::ActionLabelUnit;
use super::source::ACTION_GLYPHS;

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const POINTER_TABLE_OFFSET: usize = 0x11e0;
const POINTER_TABLE_END: usize = 0x1418;
const DIRECT_SPRITE_TABLE_START: usize = POINTER_TABLE_END;
const OVERLAY_ENTRYPOINT_OFFSET: usize = 0x3afc;
pub(super) const SECONDARY_PARSER_RUNTIME_ADDRESS: u32 = 0x800a_b374;
pub(super) const STATIC_CALLER_RUNTIME_ADDRESSES: [u32; 2] = [0x800a_9170, 0x800a_9600];
pub(super) const RUNTIME_OBSERVED_RETURN_ADDRESS: u32 = 0x800a_9608;

const VIEW_CARD_SOURCE: [u8; 19] = [
    1, 5, 2, 0, 4, 6, 1, 1, 7, 0, 0, 11, 0, 11, 9, 0, 8, 10, 0x81,
];
const GIVE_CARD_SOURCE: [u8; 22] = [
    1, 5, 2, 0, 4, 6, 1, 1, 7, 0, 0, 11, 0, 4, 7, 0, 5, 11, 0, 8, 10, 0x81,
];

#[derive(Clone, Copy)]
pub(super) struct ActionSequenceSpec {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) sequence_offset: usize,
    pub(super) terminator_offset: usize,
    pub(super) pointer_storage_offset: usize,
    pub(super) source_bytes: &'static [u8],
}

pub(super) const ACTION_SEQUENCES: [ActionSequenceSpec; 2] = [
    ActionSequenceSpec {
        id: "view_card",
        source_text: "カードをみる",
        sequence_offset: 0x07e8,
        terminator_offset: 0x07fa,
        pointer_storage_offset: 0x133c,
        source_bytes: &VIEW_CARD_SOURCE,
    },
    ActionSequenceSpec {
        id: "give_card",
        source_text: "カードをあげる",
        sequence_offset: 0x07fc,
        terminator_offset: 0x0811,
        pointer_storage_offset: 0x1340,
        source_bytes: &GIVE_CARD_SOURCE,
    },
];

#[derive(Debug)]
pub(super) struct PatchedActionLabelOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) output_sequence_sha256s: Vec<String>,
}

pub(super) fn patch_action_label_overlay(
    source: &[u8],
    units: &[ActionLabelUnit],
) -> Result<PatchedActionLabelOverlay> {
    validate_source_consumer(source)?;
    ensure!(
        units.len() == ACTION_SEQUENCES.len(),
        "bonus action-label unit count changed"
    );
    let glyph_codes = ACTION_GLYPHS
        .iter()
        .map(|(text, code, _)| (*text, *code))
        .collect::<BTreeMap<_, _>>();
    let mut patched = source.to_vec();
    let mut expected_write_ranges = Vec::new();
    let mut output_sequence_sha256s = Vec::new();
    for (unit, spec) in units.iter().zip(ACTION_SEQUENCES) {
        ensure!(unit.id == spec.id, "bonus action-label unit order changed");
        let korean_text = unit
            .korean_text
            .as_deref()
            .context("authored bonus action label lost Korean text")?;
        let command_capacity = (spec.terminator_offset - spec.sequence_offset) / 3;
        let encoded = encode_action_text(korean_text, command_capacity, &glyph_codes)?;
        ensure!(
            encoded.len() == spec.source_bytes.len(),
            "bonus action-label encoded length changed for {}",
            spec.id
        );
        let end = spec.terminator_offset + 1;
        patched[spec.sequence_offset..end].copy_from_slice(&encoded);
        expected_write_ranges.push([spec.sequence_offset, end]);
        output_sequence_sha256s.push(sha256_bytes(&encoded));
    }

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
        "bonus action-label overlay changed bytes outside its command sequences"
    );
    validate_output_sequences(&patched, units, &glyph_codes)?;
    validate_pointers(&patched)?;

    Ok(PatchedActionLabelOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        output_sequence_sha256s,
    })
}

pub(super) fn validate_source_consumer(source: &[u8]) -> Result<()> {
    validate_pointers(source)?;
    for spec in ACTION_SEQUENCES {
        ensure!(
            source.get(spec.sequence_offset..=spec.terminator_offset) == Some(spec.source_bytes),
            "KOUBAI2 source action sequence changed for {}",
            spec.id
        );
    }
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 action-label consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    validate_allocated_glyph_ownership(source)
}

fn validate_pointers(source: &[u8]) -> Result<()> {
    for spec in ACTION_SEQUENCES {
        ensure!(
            read_u32(source, spec.pointer_storage_offset)?
                == OVERLAY_RUNTIME_BASE + spec.sequence_offset as u32,
            "KOUBAI2 action-label pointer changed for {}",
            spec.id
        );
    }
    ensure!(
        read_u32(source, 0x1344)? == OVERLAY_RUNTIME_BASE + 0x0814,
        "KOUBAI2 action-label trailing sequence boundary changed"
    );
    Ok(())
}

fn validate_output_sequences(
    overlay: &[u8],
    units: &[ActionLabelUnit],
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<()> {
    for (unit, spec) in units.iter().zip(ACTION_SEQUENCES) {
        let expected = encode_action_text(
            unit.korean_text
                .as_deref()
                .context("authored bonus action label lost Korean text")?,
            (spec.terminator_offset - spec.sequence_offset) / 3,
            glyph_codes,
        )?;
        ensure!(
            overlay.get(spec.sequence_offset..=spec.terminator_offset) == Some(expected.as_slice()),
            "bonus action-label output sequence differs for {}",
            spec.id
        );
    }
    Ok(())
}

pub(super) fn encode_action_text(
    text: &str,
    command_capacity: usize,
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<Vec<u8>> {
    let mut encoded = Vec::with_capacity(command_capacity * 3 + 1);
    let mut command_count = 0;
    for character in text.chars() {
        if character == ' ' {
            encoded.extend_from_slice(&[0x63; 3]);
        } else {
            let mut utf8 = [0; 4];
            let text = character.encode_utf8(&mut utf8);
            let code = glyph_codes.get(text).with_context(|| {
                format!("bonus action label lacks an allocated glyph for {text}")
            })?;
            encoded.extend_from_slice(&glyph_command(*code));
        }
        command_count += 1;
    }
    ensure!(
        command_count <= command_capacity,
        "bonus action label exceeds its source command capacity"
    );
    for _ in command_count..command_capacity {
        encoded.extend_from_slice(&[0x63; 3]);
    }
    encoded.push(0x81);
    Ok(encoded)
}

fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}

pub(super) fn validate_allocated_glyph_ownership(source: &[u8]) -> Result<()> {
    let aliases = allocated_physical_alias_codes();
    let referenced = pointer_table_glyph_codes(source)?;
    ensure!(
        aliases.is_disjoint(&referenced),
        "KOUBAI1 action-label allocation aliases a source command glyph"
    );
    let direct_table = source
        .get(DIRECT_SPRITE_TABLE_START..OVERLAY_ENTRYPOINT_OFFSET)
        .context("KOUBAI2 direct-sprite table region is truncated")?;
    for code in aliases {
        let selector = [
            8 + (code >> 8) as u8,
            (code & 0x000f) as u8,
            ((code >> 4) & 0x000f) as u8,
        ];
        ensure!(
            !direct_table
                .windows(selector.len())
                .any(|bytes| bytes == selector),
            "KOUBAI1 action-label allocation aliases source direct-sprite glyph 0x{code:04x}"
        );
    }
    Ok(())
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let mut written_x = [false; 256];
    for column in 0..5 {
        mark_axis_pixels(&mut written_x, column);
    }
    let mut written_y = [false; 256];
    mark_axis_pixels(&mut written_y, 2);
    (0_u16..=0xff)
        .filter_map(|low| {
            let column = usize::from(low & 0x0f);
            let row = usize::from(low >> 4);
            (axis_intersects(&written_x, column) && axis_intersects(&written_y, row))
                .then_some(0x0300 | low)
        })
        .collect()
}

fn mark_axis_pixels(pixels: &mut [bool; 256], index: usize) {
    for local in 0..20 {
        pixels[(index * 20 + local) & 0xff] = true;
    }
}

fn axis_intersects(pixels: &[bool; 256], index: usize) -> bool {
    (0..20).any(|local| pixels[(index * 20 + local) & 0xff])
}

fn pointer_table_glyph_codes(source: &[u8]) -> Result<BTreeSet<u16>> {
    let mut codes = BTreeSet::new();
    for pointer_offset in (POINTER_TABLE_OFFSET..POINTER_TABLE_END).step_by(4) {
        let pointer = read_u32(source, pointer_offset)?;
        ensure!(
            (OVERLAY_RUNTIME_BASE..OVERLAY_RUNTIME_BASE + POINTER_TABLE_OFFSET as u32)
                .contains(&pointer),
            "KOUBAI2 command pointer left its source string region"
        );
        let mut cursor = usize::try_from(pointer - OVERLAY_RUNTIME_BASE)?;
        loop {
            let opcode = *source
                .get(cursor)
                .context("KOUBAI2 command sequence is unterminated")?;
            match opcode {
                0x81 => break,
                0x80 => cursor += 1,
                0x63 => {
                    ensure!(
                        source.get(cursor..cursor + 3) == Some(&[0x63; 3]),
                        "KOUBAI2 blank command changed"
                    );
                    cursor += 3;
                }
                page => {
                    let column = *source.get(cursor + 1).context("truncated glyph command")?;
                    let row = *source.get(cursor + 2).context("truncated glyph command")?;
                    ensure!(
                        page <= 3 && column <= 15 && row <= 15,
                        "invalid KOUBAI2 glyph command"
                    );
                    codes.insert(u16::from(page) << 8 | u16::from(row) << 4 | u16::from(column));
                    cursor += 3;
                }
            }
        }
    }
    Ok(codes)
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    let mut instructions = Vec::new();
    push_action_caller(&mut instructions, 0x7144, 0x715c, 0x7170, 0x7178, false);
    push_action_caller(&mut instructions, 0x75d4, 0x75f8, 0x7600, 0x7608, true);
    instructions.extend([
        (
            0x9374,
            Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: -64,
            },
        ),
        (
            0x937c,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::A1,
                rt: Register::ZERO,
            },
        ),
        (
            0x9394,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x81,
            },
        ),
        (
            0x93b4,
            Instruction::Lbu {
                rt: Register::A2,
                base: Register::S2,
                offset: 0,
            },
        ),
        (
            0x93d4,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::S6,
                target: 0x800a_b3ec,
            },
        ),
        (
            0x93d8,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x63,
            },
        ),
        (
            0x93ec,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: 0x800a_b400,
            },
        ),
        (
            0x93f4,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::S3,
                immediate: 20,
            },
        ),
        (
            0x93fc,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 3,
            },
        ),
    ]);
    instructions
}

fn push_action_caller(
    instructions: &mut Vec<(usize, Instruction)>,
    table_offset: usize,
    load_offset: usize,
    call_offset: usize,
    bound_offset: usize,
    increment_in_delay_slot: bool,
) {
    instructions.extend([
        (
            table_offset,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x800a,
            },
        ),
        (
            table_offset + 4,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x333c,
            },
        ),
        (
            table_offset + 8,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::S1,
                shift: 2,
            },
        ),
        (
            table_offset + 12,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::V0,
                rt: Register::S5,
            },
        ),
        (
            load_offset,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::S0,
                offset: 0,
            },
        ),
        (
            call_offset,
            Instruction::Jal {
                target: SECONDARY_PARSER_RUNTIME_ADDRESS,
            },
        ),
        (
            bound_offset,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 2,
            },
        ),
    ]);
    let increment_offset = if increment_in_delay_slot {
        call_offset + 4
    } else {
        load_offset + 8
    };
    instructions.push((
        increment_offset,
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 1,
        },
    ));
}

fn decode_instruction(data: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 instruction at +0x{offset:04x}"))?;
    let word = u32::from_le_bytes(bytes.try_into().expect("four-byte instruction"));
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode KOUBAI2 instruction at +0x{offset:04x}"))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI2 word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}
