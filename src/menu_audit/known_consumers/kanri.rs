use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuConsumerEvidence;
use super::super::scanner::candidate_codes;
use crate::consumer_analysis::profiles::KANRI_MENU_TEXT_RENDERER_ADDRESS;
use crate::pipeline::sha256_bytes;
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;
use crate::tim::Cell;

const OVERLAY_PATH: &str = "DAT1/KANRI.BIN";
const SOURCE_SHA256: &str = "be317e6eb86ad7d15c68a1c6a3bf8237009a73c1b8edc4e4cdc5350576c20627";
const RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const RENDERER_OFFSET: usize = 0x1b3c;
pub(super) const RENDERER_ADDRESS: u32 = KANRI_MENU_TEXT_RENDERER_ADDRESS;
const MENU_LABEL_LOOP_CALL_OFFSET: usize = 0x24b4;
const LEGACY_NAME_MAPPING_TABLE_OFFSET: usize = 0x0d74;
const LEGACY_NAME_MAPPING_CODE_COUNT: usize = 0x061e;
const EDIT_SAVE_BUFFER_ADDRESS: u32 = 0x801a_2000;
const EDIT_SAVE_FILE_NAME_POINTER_OFFSET: usize = 0x0010;
const EDIT_SAVE_FILE_NAME_OFFSET: usize = 0x8270;
const EDIT_SAVE_FILE_NAME: &[u8] = b"BISLPS-021200\0";
const SCHOOL_LABEL_DESCRIPTOR_POINTER_TABLE_OFFSET: usize = 0x0bac;
const SCHOOL_LABEL_SELECTOR_READ_OFFSET: usize = 0x27d0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KanriSchoolLabelConsumer {
    pub(crate) id: &'static str,
    pub(crate) source_text: &'static str,
    pub(crate) selector: u8,
    pub(crate) descriptor_offset: usize,
    pub(crate) texture_page: u16,
    pub(crate) cell: Cell,
}

pub(super) const SCHOOL_LABEL_CONSUMERS: [KanriSchoolLabelConsumer; 5] = [
    school_label_consumer(
        "edit_school_taiyo_academy",
        "太陽学園",
        0,
        0x0b5c,
        0x02c0,
        Cell {
            x: 440,
            y: 96,
            width: 72,
            height: 20,
        },
    ),
    school_label_consumer(
        "edit_school_gorin_high_school",
        "五輪高校",
        1,
        0x0b6c,
        0x02c0,
        Cell {
            x: 440,
            y: 116,
            width: 72,
            height: 20,
        },
    ),
    school_label_consumer(
        "edit_school_pacific_high_school",
        "パシフィックH.S",
        2,
        0x0b7c,
        0x02c0,
        Cell {
            x: 400,
            y: 196,
            width: 80,
            height: 20,
        },
    ),
    school_label_consumer(
        "edit_school_gedo_high_school",
        "外道高校",
        3,
        0x0b8c,
        0x02c0,
        Cell {
            x: 400,
            y: 176,
            width: 72,
            height: 20,
        },
    ),
    school_label_consumer(
        "edit_school_justice_academy",
        "ジャスティス学園",
        4,
        0x0b9c,
        0x0300,
        Cell {
            x: 512,
            y: 212,
            width: 80,
            height: 20,
        },
    ),
];

const fn school_label_consumer(
    id: &'static str,
    source_text: &'static str,
    selector: u8,
    descriptor_offset: usize,
    texture_page: u16,
    cell: Cell,
) -> KanriSchoolLabelConsumer {
    KanriSchoolLabelConsumer {
        id,
        source_text,
        selector,
        descriptor_offset,
        texture_page,
        cell,
    }
}

pub(crate) fn school_label_consumers() -> &'static [KanriSchoolLabelConsumer] {
    &SCHOOL_LABEL_CONSUMERS
}

#[derive(Clone, Copy)]
pub(super) struct PlacementRecordSpec {
    pub(super) record_offset: usize,
    pub(super) string_offset: usize,
    pub(super) menu_atlas_candidate: bool,
}

pub(super) const PLACEMENT_RECORDS: [PlacementRecordSpec; 39] = [
    record(0x0520, 0x0318, true),
    record(0x052c, 0x02d0, false),
    record(0x0544, 0x034c, true),
    record(0x055c, 0x0374, true),
    record(0x0568, 0x0388, true),
    record(0x0574, 0x039c, true),
    record(0x0580, 0x03b4, true),
    record(0x058c, 0x03c4, true),
    record(0x0598, 0x03e0, true),
    record(0x05a4, 0x03f4, true),
    record(0x061c, 0x0474, true),
    record(0x0634, 0x04a4, true),
    record(0x0640, 0x04c0, true),
    record(0x0824, 0x064c, false),
    record(0x0830, 0x065c, true),
    record(0x083c, 0x0668, true),
    record(0x0848, 0x0674, false),
    record(0x0854, 0x0684, false),
    record(0x0860, 0x0690, true),
    record(0x086c, 0x0698, false),
    record(0x0878, 0x06ac, false),
    record(0x0884, 0x06cc, true),
    record(0x0890, 0x06dc, true),
    record(0x089c, 0x06f8, false),
    record(0x08a8, 0x0710, false),
    record(0x08b4, 0x0720, true),
    record(0x08c0, 0x0740, false),
    record(0x08cc, 0x0750, false),
    record(0x08d8, 0x0770, true),
    record(0x08e4, 0x077c, true),
    record(0x08f0, 0x0798, false),
    record(0x08fc, 0x07b4, false),
    record(0x0908, 0x07d4, false),
    record(0x0914, 0x07e8, true),
    record(0x0920, 0x0808, true),
    record(0x095c, 0x092c, false),
    record(0x0968, 0x0934, true),
    record(0x0974, 0x093c, false),
    record(0x0980, 0x0944, true),
];

pub(super) const MENU_LABEL_RECORDS: [PlacementRecordSpec; 5] = [
    record(0x04cc, 0x02d0, false),
    record(0x04d8, 0x02e0, true),
    record(0x04e4, 0x02e8, true),
    record(0x04f0, 0x02f0, false),
    record(0x04fc, 0x0300, true),
];

pub(super) const STATUS_OVERVIEW_RECORDS: [PlacementRecordSpec; 7] = [
    record(0x05b0, 0x0404, true),
    record(0x05bc, 0x0410, true),
    record(0x05c8, 0x041c, true),
    record(0x05d4, 0x0424, true),
    record(0x05e0, 0x042c, true),
    record(0x05ec, 0x0434, true),
    record(0x05f8, 0x0444, true),
];

pub(super) const STATUS_RATING_RECORDS: [PlacementRecordSpec; 5] = [
    record(0x09d4, 0x09a4, true),
    record(0x09e0, 0x09b0, true),
    record(0x09ec, 0x09b8, true),
    record(0x09f8, 0x09c0, true),
    record(0x0a04, 0x09c8, true),
];

const STATUS_OVERVIEW_LOOP_CALL_OFFSETS: [usize; 4] = [0x59d8, 0x5a44, 0x5aac, 0x5be4];
const STATUS_OVERVIEW_JUMP_TABLE_OFFSET: usize = 0x82d8;
const STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_SOURCE_CODE: i16 = 0x00a9;
const STATUS_OVERVIEW_JUMP_TARGETS: [u32; 7] = [
    0x800a_79cc,
    0x800a_7a38,
    0x800a_79cc,
    0x800a_79cc,
    0x800a_79cc,
    0x800a_7aa0,
    0x800a_7bd8,
];
pub(super) const STATUS_RATING_CALL_OFFSET: usize = 0x59e4;
const STATUS_RATING_RENDER_CALL_OFFSET: usize = 0x5a28;
const STATUS_RATING_SELECTOR_OFFSET: usize = 0x5508;
const STATUS_RATING_SELECTOR_JUMP_TABLE_OFFSET: usize = 0x82c0;
const STATUS_RATING_SELECTOR_JUMP_TARGETS: [u32; 5] = [
    0x800a_7540,
    0x800a_7648,
    0x800a_7584,
    0x800a_75c8,
    0x800a_760c,
];

pub(super) const BOUNDED_LOOP_CALL_OFFSET: usize = 0x76e8;
pub(super) const BOUNDED_LOOP_RECORD_OFFSETS: [usize; 4] = [0x095c, 0x0968, 0x0974, 0x0980];
pub(super) const STATE_DRIVEN_SCHEDULE_OFFSET: usize = 0x1640;
pub(super) const STATE_DRIVEN_LOOP_CALL_OFFSETS: [usize; 3] = [0x7bd8, 0x7ce8, 0x7da8];
pub(super) const STATE_DRIVEN_RECORD_OFFSETS: [usize; 12] = [
    0x0890, 0x089c, 0x08a8, 0x08b4, 0x08c0, 0x08cc, 0x08d8, 0x08e4, 0x08f0, 0x08fc, 0x0908, 0x0914,
];

const STATE_DRIVEN_RECORD_TABLE_OFFSET: usize = 0x0824;
const PLACEMENT_RECORD_SIZE: usize = 12;

#[derive(Clone, Copy)]
struct PlacementScheduleEntry {
    record_count: u8,
    first_record_index: u8,
}

const STATE_DRIVEN_SCHEDULE: [PlacementScheduleEntry; 6] = [
    schedule_entry(3, 9),
    schedule_entry(2, 12),
    schedule_entry(2, 14),
    schedule_entry(2, 16),
    schedule_entry(2, 18),
    schedule_entry(1, 20),
];

const fn record(
    record_offset: usize,
    string_offset: usize,
    menu_atlas_candidate: bool,
) -> PlacementRecordSpec {
    PlacementRecordSpec {
        record_offset,
        string_offset,
        menu_atlas_candidate,
    }
}

const fn schedule_entry(record_count: u8, first_record_index: u8) -> PlacementScheduleEntry {
    PlacementScheduleEntry {
        record_count,
        first_record_index,
    }
}

pub(super) fn identify_placement_record_consumers(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
    if overlay_path != OVERLAY_PATH {
        return Ok(BTreeMap::new());
    }
    ensure!(
        sha256_bytes(data) == SOURCE_SHA256,
        "unsupported {OVERLAY_PATH} source bytes"
    );
    ensure!(
        runtime_base == Some(RUNTIME_BASE),
        "unexpected {OVERLAY_PATH} runtime base"
    );
    validate_placement_record_consumers(
        data,
        RUNTIME_BASE,
        reachable_instruction_offsets,
        reachable_lui_seed_offsets,
        direct_call_arguments,
    )
}

pub(crate) fn source_menu_codes(data: &[u8]) -> Result<BTreeSet<u16>> {
    ensure!(
        sha256_bytes(data) == SOURCE_SHA256,
        "unsupported {OVERLAY_PATH} source bytes"
    );
    validate_records(data, RUNTIME_BASE)?;
    validate_school_label_consumer(data, RUNTIME_BASE)?;
    let mut codes = PLACEMENT_RECORDS
        .into_iter()
        .chain(MENU_LABEL_RECORDS)
        .chain(STATUS_OVERVIEW_RECORDS)
        .chain(STATUS_RATING_RECORDS)
        .filter(|record| record.menu_atlas_candidate)
        .try_fold(BTreeSet::new(), |mut codes, record| {
            let record_codes = candidate_codes(data, record.string_offset).with_context(|| {
                format!(
                    "KANRI source string +0x{:04x} is no longer a MENU-atlas record",
                    record.string_offset
                )
            })?;
            codes.extend(
                record_codes
                    .into_iter()
                    .map(|code| code & 0x0fff)
                    .filter(|code| *code != 0x0fff),
            );
            Ok::<BTreeSet<u16>, anyhow::Error>(codes)
        })?;
    let mapping_end = LEGACY_NAME_MAPPING_TABLE_OFFSET + LEGACY_NAME_MAPPING_CODE_COUNT * 2;
    let mapping = data
        .get(LEGACY_NAME_MAPPING_TABLE_OFFSET..mapping_end)
        .context("KANRI legacy name mapping table is truncated")?;
    ensure!(
        codes.iter().all(|code| *code < 0x0400),
        "KANRI placement record selected a code outside the global MENU atlas"
    );
    codes.extend(
        mapping
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]) & 0x0fff)
            .filter(|code| *code != 0x0fff && *code < 0x0400),
    );
    codes.extend(registration_label_codes(data)?);
    Ok(codes)
}

// The source-bound loop at +0x2b84 draws these sixteen descriptors through
// the same MENU renderer. They are separate from the translated UI tables.
// In particular, 0x02b6 is the native ordinal period, not a translated unit.
pub(super) fn registration_label_codes(data: &[u8]) -> Result<BTreeSet<u16>> {
    let mut codes = BTreeSet::new();
    for index in 0..16 {
        let pointer_offset = 0x0a9c + index * 12 + 4;
        let pointer = data
            .get(pointer_offset..pointer_offset + 4)
            .context("truncated KANRI registration label descriptor")?;
        let offset = u32::from_le_bytes(pointer.try_into()?)
            .checked_sub(RUNTIME_BASE)
            .context("KANRI registration label points before its overlay")?
            as usize;
        let label = crate::text::read_length_prefixed_codes(data, offset)?;
        ensure!(
            label.iter().all(|code| *code < 0x0400),
            "KANRI registration label escaped the MENU atlas"
        );
        codes.extend(label);
    }
    Ok(codes)
}

pub(super) fn validate_placement_record_consumers(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "KANRI placement renderer moved from its source-bound runtime base"
    );
    ensure!(
        reachable_instruction_offsets.contains(&RENDERER_OFFSET),
        "KANRI placement renderer is not entrypoint-reachable"
    );
    validate_renderer_grammar(data)?;
    validate_records(data, runtime_base)?;
    validate_menu_label_loop(data, runtime_base)?;
    validate_status_overview_loop(data, runtime_base)?;
    ensure!(
        reachable_instruction_offsets.contains(&STATUS_RATING_CALL_OFFSET),
        "KANRI status-rating selector call is not entrypoint-reachable"
    );
    validate_status_rating_consumer(data, runtime_base)?;
    validate_state_driven_placement_loops(data, runtime_base)?;
    validate_school_label_consumer(data, runtime_base)?;
    let unreachable_state_driven_calls = STATE_DRIVEN_LOOP_CALL_OFFSETS
        .into_iter()
        .filter(|offset| !reachable_instruction_offsets.contains(offset))
        .map(|offset| format!("+0x{offset:04x}"))
        .collect::<Vec<_>>();
    ensure!(
        unreachable_state_driven_calls.is_empty(),
        "KANRI state-driven placement renderer call is not entrypoint-reachable: {}",
        unreachable_state_driven_calls.join(", ")
    );
    validate_edit_save_slot_marker_consumer(data, runtime_base)?;

    let expected_record_addresses = PLACEMENT_RECORDS
        .iter()
        .map(|record| runtime_base + record.record_offset as u32)
        .collect::<BTreeSet<_>>();
    let relevant_arguments = direct_call_arguments
        .iter()
        .filter(|argument| {
            argument.target == RENDERER_ADDRESS
                && argument.argument_register == Register::A1
                && reachable_lui_seed_offsets.contains(&argument.seed_offset)
                && reachable_instruction_offsets.contains(&argument.instruction_offset)
        })
        .collect::<Vec<_>>();
    let expected_loop_addresses = BOUNDED_LOOP_RECORD_OFFSETS
        .into_iter()
        .map(|offset| runtime_base + offset as u32)
        .collect::<BTreeSet<_>>();
    let observed_loop_addresses = relevant_arguments
        .iter()
        .filter(|argument| argument.instruction_offset == BOUNDED_LOOP_CALL_OFFSET)
        .map(|argument| argument.value)
        .collect::<BTreeSet<_>>();
    ensure!(
        expected_loop_addresses.is_subset(&observed_loop_addresses),
        "KANRI bounded placement loop does not cover its four source-bound records; missing: {}",
        display_addresses(expected_loop_addresses.difference(&observed_loop_addresses)),
    );
    let state_driven_record_addresses = STATE_DRIVEN_RECORD_OFFSETS
        .into_iter()
        .map(|offset| runtime_base + offset as u32)
        .collect::<BTreeSet<_>>();
    let observed_record_addresses = relevant_arguments
        .iter()
        .filter(|argument| {
            argument.instruction_offset != BOUNDED_LOOP_CALL_OFFSET
                || expected_loop_addresses.contains(&argument.value)
        })
        .map(|argument| argument.value)
        .chain(state_driven_record_addresses.iter().copied())
        .collect::<BTreeSet<_>>();
    let missing_record_addresses = expected_record_addresses
        .difference(&observed_record_addresses)
        .copied()
        .collect::<BTreeSet<_>>();
    let unexpected_record_addresses = observed_record_addresses
        .difference(&expected_record_addresses)
        .copied()
        .collect::<BTreeSet<_>>();
    let unexpected_argument_contexts = relevant_arguments
        .iter()
        .filter(|argument| {
            argument.instruction_offset != BOUNDED_LOOP_CALL_OFFSET
                && unexpected_record_addresses.contains(&argument.value)
        })
        .map(|argument| {
            format!(
                "value=0x{:08x}/seed=+0x{:04x}/call=+0x{:04x}",
                argument.value, argument.seed_offset, argument.instruction_offset
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        missing_record_addresses.is_empty() && unexpected_record_addresses.is_empty(),
        "KANRI renderer call sites do not match the source-bound placement records after applying the proven four-iteration loop bound and state-driven schedule bounds; missing: {}; unexpected: {}; unexpected provenance: {}",
        display_addresses(missing_record_addresses.iter()),
        display_addresses(unexpected_record_addresses.iter()),
        unexpected_argument_contexts.join(", "),
    );

    let mut consumers = BTreeMap::new();
    for record in MENU_LABEL_RECORDS
        .iter()
        .filter(|record| record.menu_atlas_candidate)
    {
        ensure!(
            consumers
                .insert(
                    record.string_offset,
                    vec![MenuConsumerEvidence::KanriMenuLabelTable],
                )
                .is_none(),
            "KANRI menu-label records repeat an admitted static string target"
        );
    }
    for record in PLACEMENT_RECORDS
        .iter()
        .filter(|record| record.menu_atlas_candidate)
    {
        ensure!(
            consumers
                .insert(
                    record.string_offset,
                    vec![MenuConsumerEvidence::KanriPlacementRecordTable],
                )
                .is_none(),
            "KANRI placement records repeat an admitted static string target"
        );
    }
    for record in STATUS_OVERVIEW_RECORDS {
        ensure!(
            consumers
                .insert(
                    record.string_offset,
                    vec![MenuConsumerEvidence::KanriPlacementRecordTable],
                )
                .is_none(),
            "KANRI status-overview records repeat an admitted static string target"
        );
    }
    for record in STATUS_RATING_RECORDS {
        ensure!(
            consumers
                .insert(
                    record.string_offset,
                    vec![MenuConsumerEvidence::KanriStatusRatingTable],
                )
                .is_none(),
            "KANRI status-rating records repeat an admitted string target"
        );
    }
    Ok(consumers)
}

fn validate_status_overview_loop(data: &[u8], runtime_base: u32) -> Result<()> {
    let expected = [
        (
            0x5990,
            Instruction::Addu {
                rd: Register::S6,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x5994,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x800a,
            },
        ),
        (
            0x5998,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x25b0,
            },
        ),
        (
            0x59a8,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::S6,
                immediate: 7,
            },
        ),
        (
            0x59b0,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::S6,
                shift: 2,
            },
        ),
        (
            0x59b4,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800b,
            },
        ),
        (
            0x59b8,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x59bc,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: -0x5d28,
            },
        ),
        (0x59c4, Instruction::Jr { rs: Register::V0 }),
        (
            0x5a5c,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::T0,
                offset: 0x0c,
            },
        ),
        (
            0x5a64,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x5a68,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x5a6c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x5a70,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x35e4,
            },
        ),
        (
            0x5a74,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: STATUS_OVERVIEW_SPIRIT_GAUGE_UNIT_SOURCE_CODE,
            },
        ),
        (
            0x5a78,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x2a,
            },
        ),
        (
            0x5a7c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x5a80,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x2c,
            },
        ),
        (
            0x5a88,
            Instruction::Sh {
                rt: Register::V1,
                base: Register::SP,
                offset: 0x28,
            },
        ),
        (
            0x5a90,
            Instruction::Jal {
                target: runtime_base + 0x1f74,
            },
        ),
        (
            0x5d54,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 12,
            },
        ),
        (
            0x5d60,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::S6,
                immediate: 1,
            },
        ),
        (
            0x5d64,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S6,
                immediate: 7,
            },
        ),
        (
            0x5d68,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x59a8,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = runtime_base + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
            format!("failed to decode KANRI status-overview loop at +0x{offset:04x}")
        })?;
        ensure!(
            instruction == expected_instruction,
            "KANRI status-overview loop grammar changed at +0x{offset:04x}"
        );
    }
    for call_offset in STATUS_OVERVIEW_LOOP_CALL_OFFSETS {
        let argument = decode(
            read_u32(data, call_offset - 8)?,
            runtime_base + call_offset as u32 - 8,
        )?;
        let call = decode(
            read_u32(data, call_offset)?,
            runtime_base + call_offset as u32,
        )?;
        ensure!(
            argument
                == Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::S5,
                    rt: Register::ZERO,
                }
                && call
                    == Instruction::Jal {
                        target: RENDERER_ADDRESS,
                    },
            "KANRI status-overview renderer call changed at +0x{call_offset:04x}"
        );
    }
    for (index, target) in STATUS_OVERVIEW_JUMP_TARGETS.into_iter().enumerate() {
        ensure!(
            read_u32(data, STATUS_OVERVIEW_JUMP_TABLE_OFFSET + index * 4)? == target,
            "KANRI status-overview jump-table target changed at index {index}"
        );
    }
    Ok(())
}

pub(super) fn validate_status_rating_consumer(data: &[u8], runtime_base: u32) -> Result<()> {
    let expected = [
        (
            STATUS_RATING_CALL_OFFSET - 4,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::SP,
                offset: 0x38,
            },
        ),
        (
            STATUS_RATING_CALL_OFFSET,
            Instruction::Jal {
                target: runtime_base + STATUS_RATING_SELECTOR_OFFSET as u32,
            },
        ),
        (
            STATUS_RATING_CALL_OFFSET + 4,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::S6,
                rt: Register::ZERO,
            },
        ),
        (
            0x59ec,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S3,
                rt: Register::S7,
            },
        ),
        (
            0x59f0,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::SP,
                immediate: 0x18,
            },
        ),
        (
            0x59f4,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x59f8,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x59fc,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x5a00,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x800a,
            },
        ),
        (
            0x5a04,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x29d4,
            },
        ),
        (
            0x5a08,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x5a0c,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0x5a1c,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V1,
                offset: 4,
            },
        ),
        (
            STATUS_RATING_RENDER_CALL_OFFSET,
            Instruction::Jal {
                target: RENDERER_ADDRESS,
            },
        ),
        (
            STATUS_RATING_RENDER_CALL_OFFSET + 4,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x1c,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 4,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x10,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 5,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x14,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x5648,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x1c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x24,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x28,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: -0x5d40,
            },
        ),
        (
            STATUS_RATING_SELECTOR_OFFSET + 0x30,
            Instruction::Jr { rs: Register::V0 },
        ),
        (
            0x5648,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A3,
                immediate: 5,
            },
        ),
        (
            0x564c,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x565c,
            },
        ),
        (
            0x5650,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::A3,
                rt: Register::ZERO,
            },
        ),
        (
            0x5654,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            0x5658,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::A3,
                rt: Register::ZERO,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = runtime_base + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
            format!("failed to decode KANRI status-rating consumer at +0x{offset:04x}")
        })?;
        ensure!(
            instruction == expected_instruction,
            "KANRI status-rating consumer grammar changed at +0x{offset:04x}"
        );
    }
    for (index, target) in STATUS_RATING_SELECTOR_JUMP_TARGETS.into_iter().enumerate() {
        ensure!(
            read_u32(data, STATUS_RATING_SELECTOR_JUMP_TABLE_OFFSET + index * 4)? == target,
            "KANRI status-rating selector jump-table target changed at index {index}"
        );
    }
    Ok(())
}

fn validate_state_driven_placement_loops(data: &[u8], runtime_base: u32) -> Result<()> {
    for (state, schedule) in STATE_DRIVEN_SCHEDULE.iter().enumerate() {
        let offset = STATE_DRIVEN_SCHEDULE_OFFSET + state * 2;
        let actual = data
            .get(offset..offset + 2)
            .with_context(|| format!("truncated KANRI placement schedule at +0x{offset:04x}"))?;
        ensure!(
            actual == [schedule.record_count, schedule.first_record_index],
            "KANRI state-driven placement schedule changed for state {state} at +0x{offset:04x}"
        );
    }

    let scheduled_record_offsets = STATE_DRIVEN_SCHEDULE
        .iter()
        .flat_map(|schedule| {
            (0..usize::from(schedule.record_count)).map(move |step| {
                STATE_DRIVEN_RECORD_TABLE_OFFSET
                    + (usize::from(schedule.first_record_index) + step) * PLACEMENT_RECORD_SIZE
            })
        })
        .collect::<BTreeSet<_>>();
    ensure!(
        scheduled_record_offsets == BTreeSet::from(STATE_DRIVEN_RECORD_OFFSETS),
        "KANRI state-driven placement schedule no longer selects the source-bound record set"
    );

    for call_offset in STATE_DRIVEN_LOOP_CALL_OFFSETS {
        let expected = [
            (
                call_offset - 0x58,
                Instruction::Lh {
                    rt: Register::V0,
                    base: Register::S0,
                    offset: 0x16,
                },
            ),
            (
                call_offset - 0x50,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 1,
                },
            ),
            (
                call_offset - 0x4c,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x800a,
                },
            ),
            (
                call_offset - 0x48,
                Instruction::Addu {
                    rd: Register::AT,
                    rs: Register::AT,
                    rt: Register::V0,
                },
            ),
            (
                call_offset - 0x44,
                Instruction::Lbu {
                    rt: Register::S4,
                    base: Register::AT,
                    offset: 0x3640,
                },
            ),
            (
                call_offset - 0x40,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x800a,
                },
            ),
            (
                call_offset - 0x3c,
                Instruction::Addu {
                    rd: Register::AT,
                    rs: Register::AT,
                    rt: Register::V0,
                },
            ),
            (
                call_offset - 0x38,
                Instruction::Lbu {
                    rt: Register::A0,
                    base: Register::AT,
                    offset: 0x3641,
                },
            ),
            (
                call_offset - 0x34,
                Instruction::Beq {
                    rs: Register::S4,
                    rt: Register::ZERO,
                    target: runtime_base + call_offset as u32 + 0x18,
                },
            ),
            (
                call_offset - 0x30,
                Instruction::Addu {
                    rd: Register::S2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                call_offset - 0x28,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x800a,
                },
            ),
            (
                call_offset - 0x24,
                Instruction::Addiu {
                    rt: Register::V1,
                    rs: Register::V1,
                    immediate: 0x2824,
                },
            ),
            (
                call_offset - 0x20,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::A0,
                    shift: 1,
                },
            ),
            (
                call_offset - 0x1c,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: Register::A0,
                },
            ),
            (
                call_offset - 0x18,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 2,
                },
            ),
            (
                call_offset - 0x14,
                Instruction::Addu {
                    rd: Register::S1,
                    rs: Register::V0,
                    rt: Register::V1,
                },
            ),
            (
                call_offset - 0x10,
                Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::S1,
                    rt: Register::ZERO,
                },
            ),
            (
                call_offset - 0x0c,
                Instruction::Addiu {
                    rt: Register::S1,
                    rs: Register::S1,
                    immediate: 12,
                },
            ),
            (
                call_offset,
                Instruction::Jal {
                    target: RENDERER_ADDRESS,
                },
            ),
            (
                call_offset + 0x08,
                Instruction::Addiu {
                    rt: Register::S2,
                    rs: Register::S2,
                    immediate: 1,
                },
            ),
            (
                call_offset + 0x0c,
                Instruction::Slt {
                    rd: Register::V0,
                    rs: Register::S2,
                    rt: Register::S4,
                },
            ),
            (
                call_offset + 0x10,
                Instruction::Bne {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: runtime_base + call_offset as u32 - 0x10,
                },
            ),
            (
                call_offset + 0x14,
                Instruction::Addiu {
                    rt: Register::S3,
                    rs: Register::S3,
                    immediate: 0x0380,
                },
            ),
        ];
        for (offset, expected_instruction) in expected {
            let pc = runtime_base + offset as u32;
            let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
                format!("failed to decode KANRI state-driven placement loop at +0x{offset:04x}")
            })?;
            ensure!(
                instruction == expected_instruction,
                "KANRI state-driven placement loop grammar changed at +0x{offset:04x}"
            );
        }
    }
    Ok(())
}

fn validate_records(data: &[u8], runtime_base: u32) -> Result<()> {
    for record in PLACEMENT_RECORDS
        .into_iter()
        .chain(MENU_LABEL_RECORDS)
        .chain(STATUS_OVERVIEW_RECORDS)
        .chain(STATUS_RATING_RECORDS)
    {
        let pointer = read_u32(data, record.record_offset + 4)?;
        ensure!(
            pointer == runtime_base + record.string_offset as u32,
            "KANRI placement record +0x{:04x} string pointer changed",
            record.record_offset
        );
        ensure!(
            read_u32(data, record.record_offset + 8)? == 0,
            "KANRI placement record +0x{:04x} style word changed",
            record.record_offset
        );
        ensure!(
            candidate_codes(data, record.string_offset).is_some() == record.menu_atlas_candidate,
            "KANRI placement record +0x{:04x} MENU-atlas classification changed",
            record.record_offset
        );
    }
    Ok(())
}

pub(super) fn validate_school_label_consumer(data: &[u8], runtime_base: u32) -> Result<()> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "KANRI school-label consumer moved from its source-bound runtime base"
    );
    let expected_instructions = [
        (
            0x2600,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::FP,
                shift: 9,
            },
        ),
        (
            0x2604,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x0580,
            },
        ),
        (
            0x2608,
            Instruction::Addu {
                rd: Register::S5,
                rs: Register::T1,
                rt: Register::V0,
            },
        ),
        (
            SCHOOL_LABEL_SELECTOR_READ_OFFSET,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S5,
                offset: 0x005a,
            },
        ),
        (
            0x27dc,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x27e0,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x27e4,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V1,
            },
        ),
        (
            0x27e8,
            Instruction::Lw {
                rt: Register::S0,
                base: Register::AT,
                offset: 0x2bac,
            },
        ),
    ];
    for (offset, expected_instruction) in expected_instructions {
        let pc = runtime_base + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
            format!("failed to decode KANRI school-label consumer at +0x{offset:04x}")
        })?;
        ensure!(
            instruction == expected_instruction,
            "KANRI school-label consumer grammar changed at +0x{offset:04x}"
        );
    }

    for consumer in SCHOOL_LABEL_CONSUMERS {
        let pointer_offset =
            SCHOOL_LABEL_DESCRIPTOR_POINTER_TABLE_OFFSET + usize::from(consumer.selector) * 4;
        ensure!(
            read_u32(data, pointer_offset)? == runtime_base + consumer.descriptor_offset as u32,
            "KANRI school-label selector {} descriptor pointer changed",
            consumer.selector
        );
        let descriptor = consumer.descriptor_offset;
        let page_origin_x = match consumer.texture_page {
            0x02c0 => 256,
            0x0300 => 512,
            _ => unreachable!("source-bound KANRI school texture page"),
        };
        ensure!(
            read_u16(data, descriptor)? == 0
                && read_u16(data, descriptor + 2)? == consumer.texture_page
                && read_u16(data, descriptor + 4)? == 0x0100
                && page_origin_x + usize::from(read_u16(data, descriptor + 6)?) == consumer.cell.x
                && usize::from(read_u16(data, descriptor + 8)?) == consumer.cell.y
                && usize::from(read_u16(data, descriptor + 10)?) == consumer.cell.width
                && usize::from(read_u16(data, descriptor + 12)?) == consumer.cell.height
                && read_u16(data, descriptor + 14)? == 0,
            "KANRI school-label selector {} texture descriptor changed",
            consumer.selector
        );
    }
    Ok(())
}

fn validate_menu_label_loop(data: &[u8], runtime_base: u32) -> Result<()> {
    let expected = [
        (
            0x23a8,
            Instruction::Lui {
                rt: Register::S4,
                immediate: 0x800a,
            },
        ),
        (
            0x23ac,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: 0x24cc,
            },
        ),
        (
            0x23d8,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 5,
            },
        ),
        (
            0x24a8,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::S4,
                rt: Register::ZERO,
            },
        ),
        (
            MENU_LABEL_LOOP_CALL_OFFSET,
            Instruction::Jal {
                target: RENDERER_ADDRESS,
            },
        ),
        (
            0x24bc,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: 12,
            },
        ),
        (
            0x24cc,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 1,
            },
        ),
        (
            0x24d0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 5,
            },
        ),
        (
            0x24d4,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x23d8,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = runtime_base + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
            format!("failed to decode KANRI menu-label loop at +0x{offset:04x}")
        })?;
        ensure!(
            instruction == expected_instruction,
            "KANRI menu-label loop grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

pub(super) fn validate_edit_save_slot_marker_consumer(
    data: &[u8],
    runtime_base: u32,
) -> Result<()> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "KANRI EDIT save-slot consumer moved from its source-bound runtime base"
    );
    ensure!(
        read_u32(data, EDIT_SAVE_FILE_NAME_POINTER_OFFSET)?
            == runtime_base + EDIT_SAVE_FILE_NAME_OFFSET as u32,
        "KANRI EDIT save-file name pointer changed"
    );
    ensure!(
        data.get(
            EDIT_SAVE_FILE_NAME_OFFSET..EDIT_SAVE_FILE_NAME_OFFSET + EDIT_SAVE_FILE_NAME.len()
        ) == Some(EDIT_SAVE_FILE_NAME),
        "KANRI EDIT save-file name changed"
    );

    let expected = [
        (
            0x17bc,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (EDIT_SAVE_BUFFER_ADDRESS >> 16) as u16,
            },
        ),
        (
            0x17c0,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: EDIT_SAVE_BUFFER_ADDRESS as u16,
            },
        ),
        (
            0x17d0,
            Instruction::Lui {
                rt: Register::A1,
                immediate: (EDIT_SAVE_BUFFER_ADDRESS >> 16) as u16,
            },
        ),
        (
            0x17f0,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: EDIT_SAVE_BUFFER_ADDRESS as u16,
            },
        ),
        (
            0x2400,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x2404,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x63f0,
            },
        ),
        (
            0x2408,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x240c,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::V0,
                immediate: 0x200,
            },
        ),
        (
            0x2410,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::A0,
                rt: Register::V1,
            },
        ),
        (
            0x2414,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 3,
            },
        ),
        (
            0x241c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x2428,
            },
        ),
        (
            0x2424,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x2428,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
        ),
        (
            0x242c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 5,
            },
        ),
        (
            0x2430,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x2410,
            },
        ),
        (
            0x3760,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::S1,
                rt: Register::V0,
            },
        ),
        (
            0x3764,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V0,
                offset: 3,
            },
        ),
        (
            0x3768,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x376c,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: runtime_base + 0x3798,
            },
        ),
        (
            0x61a8,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x61ac,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x63f0,
            },
        ),
        (
            0x61c4,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::V1,
                immediate: 0x200,
            },
        ),
        (
            0x6708,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x670c,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x6710,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::A2,
                rt: Register::A0,
            },
        ),
        (
            0x6714,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 3,
            },
        ),
        (
            0x671c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::V1,
                target: runtime_base + 0x6738,
            },
        ),
        (
            0x6724,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0x6728,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 5,
            },
        ),
        (
            0x672c,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_base + 0x6714,
            },
        ),
        (
            0x6730,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::A2,
                rt: Register::A0,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = runtime_base + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc).with_context(|| {
            format!("failed to decode KANRI EDIT save-slot consumer at +0x{offset:04x}")
        })?;
        ensure!(
            instruction == expected_instruction,
            "KANRI EDIT save-slot consumer grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn validate_renderer_grammar(data: &[u8]) -> Result<()> {
    let expected = [
        (
            0x1b74,
            Instruction::Lw {
                rt: Register::S5,
                base: Register::A1,
                offset: 4,
            },
        ),
        (
            0x1b78,
            Instruction::Lh {
                rt: Register::S4,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x1b7c,
            Instruction::Lhu {
                rt: Register::T0,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x1b88,
            Instruction::Lh {
                rt: Register::T0,
                base: Register::A1,
                offset: 2,
            },
        ),
        (
            0x1b98,
            Instruction::Lh {
                rt: Register::FP,
                base: Register::A1,
                offset: 8,
            },
        ),
        (
            0x1ba0,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 2,
            },
        ),
        (
            0x1ba4,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x1ba8,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x0fff,
            },
        ),
        (
            0x1bac,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x0fff,
            },
        ),
        (
            0x1bc0,
            Instruction::Sra {
                rd: Register::A2,
                rt: Register::V1,
                shift: 8,
            },
        ),
        (
            0x1bc4,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: 12,
            },
        ),
        (
            0x1bc8,
            Instruction::Sll {
                rd: Register::A2,
                rt: Register::A2,
                shift: 6,
            },
        ),
        (
            0x1bd0,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 0x000f,
            },
        ),
        (
            0x1bd4,
            Instruction::Sll {
                rd: Register::S2,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x1bd8,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::S2,
                rt: Register::V0,
            },
        ),
        (
            0x1bdc,
            Instruction::Sll {
                rd: Register::S2,
                rt: Register::S2,
                shift: 2,
            },
        ),
        (
            0x1be0,
            Instruction::Sra {
                rd: Register::V0,
                rt: Register::V1,
                shift: 4,
            },
        ),
        (
            0x1be4,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x000f,
            },
        ),
        (
            0x1be8,
            Instruction::Sll {
                rd: Register::S1,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x1bec,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::S1,
                rt: Register::V0,
            },
        ),
        (
            0x1bf8,
            Instruction::Sll {
                rd: Register::S1,
                rt: Register::S1,
                shift: 2,
            },
        ),
        (
            0x1c90,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 20,
            },
        ),
        (
            0x1d08,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::S6,
                immediate: 1,
            },
        ),
        (
            0x1d14,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::S6,
                rt: Register::T0,
            },
        ),
        (
            0x76b4,
            Instruction::Addu {
                rd: Register::S3,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x76c4,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x800a,
            },
        ),
        (
            0x76c8,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 0x295c,
            },
        ),
        (
            0x76e8,
            Instruction::Jal {
                target: RENDERER_ADDRESS,
            },
        ),
        (
            0x76f8,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::S3,
                immediate: 1,
            },
        ),
        (
            0x7700,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S3,
                immediate: 4,
            },
        ),
        (
            0x7704,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x76cc,
            },
        ),
        (
            0x7708,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 12,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = RUNTIME_BASE + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc)
            .with_context(|| format!("failed to decode KANRI renderer at +0x{offset:04x}"))?;
        ensure!(
            instruction == expected_instruction,
            "KANRI renderer grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn display_addresses<'a>(addresses: impl Iterator<Item = &'a u32>) -> String {
    addresses
        .map(|address| format!("0x{address:08x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KANRI data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .with_context(|| format!("truncated KANRI data at +0x{offset:04x}"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}
