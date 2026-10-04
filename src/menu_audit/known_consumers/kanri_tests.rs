use std::collections::BTreeSet;

use psx_r3000a::{Instruction, Register, encode};

use super::kanri::{
    BOUNDED_LOOP_CALL_OFFSET, BOUNDED_LOOP_RECORD_OFFSETS, MENU_LABEL_RECORDS, PLACEMENT_RECORDS,
    RENDERER_ADDRESS, RENDERER_OFFSET, SCHOOL_LABEL_CONSUMERS, STATE_DRIVEN_LOOP_CALL_OFFSETS,
    STATE_DRIVEN_RECORD_OFFSETS, STATE_DRIVEN_SCHEDULE_OFFSET, STATUS_OVERVIEW_RECORDS,
    STATUS_RATING_CALL_OFFSET, STATUS_RATING_RECORDS, validate_edit_save_slot_marker_consumer,
    validate_placement_record_consumers, validate_school_label_consumer,
    validate_status_rating_consumer,
};
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;

const BASE: u32 = 0x800a_2000;

#[test]
fn registration_ordinals_protect_their_period_and_both_digits() {
    let mut data = vec![0; 0xb5c];
    for index in 0..16 {
        let offset = 0xa1c + index * 8;
        let pointer = 0xa9c + index * 12 + 4;
        data[pointer..pointer + 4].copy_from_slice(&(BASE + offset as u32).to_le_bytes());
        let number = index + 1;
        let mut label = Vec::new();
        if number >= 10 {
            label.push(0u16);
        }
        label.push(((number + 9) % 10) as u16);
        label.push(0x02b6);
        data[offset..offset + 2].copy_from_slice(&(label.len() as u16).to_le_bytes());
        for (column, code) in label.iter().enumerate() {
            let start = offset + 2 + column * 2;
            data[start..start + 2].copy_from_slice(&code.to_le_bytes());
        }
    }
    let codes = super::kanri::registration_label_codes(&data).unwrap();
    assert_eq!(codes, (0..10).chain([0x02b6]).collect());
    data[0xaa0..0xaa4].copy_from_slice(&(BASE - 4).to_le_bytes());
    assert!(super::kanri::registration_label_codes(&data).is_err());
}

#[test]
fn admits_only_static_records_when_reachable_calls_cover_the_table() {
    let (data, reachable_instructions, reachable_seeds, arguments) = fixture();

    let consumers = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap();

    let admitted = consumers.keys().copied().collect::<BTreeSet<_>>();
    let expected = PLACEMENT_RECORDS
        .iter()
        .chain(MENU_LABEL_RECORDS.iter())
        .chain(STATUS_OVERVIEW_RECORDS.iter())
        .chain(STATUS_RATING_RECORDS.iter())
        .filter(|record| record.menu_atlas_candidate)
        .map(|record| record.string_offset)
        .collect::<BTreeSet<_>>();
    assert_eq!(admitted, expected);
    for source_offset in [0x02e0, 0x02e8, 0x0300] {
        assert!(admitted.contains(&source_offset));
    }
    for source_offset in [0x0720, 0x0770, 0x077c, 0x07e8] {
        assert!(admitted.contains(&source_offset));
    }
}

#[test]
fn binds_status_ratings_to_the_five_value_selector_range() {
    let (data, ..) = fixture();

    validate_status_rating_consumer(&data, BASE).unwrap();
}

#[test]
fn rejects_a_status_rating_selector_with_a_wider_result_range() {
    let (mut data, ..) = fixture();
    write_instruction(
        &mut data,
        0x5648,
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::A3,
            immediate: 6,
        },
    );

    let error = validate_status_rating_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("status-rating consumer grammar"));
}

#[test]
fn rejects_a_status_overview_loop_that_skips_one_label() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    write_instruction(
        &mut data,
        0x59a8,
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::S6,
            immediate: 6,
        },
    );

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("status-overview loop grammar"));
}

#[test]
fn rejects_a_status_overview_spirit_gauge_value_without_its_unit_buffer() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    write_instruction(
        &mut data,
        0x5a78,
        Instruction::Sh {
            rt: Register::V0,
            base: Register::SP,
            offset: 0x28,
        },
    );

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("status-overview loop grammar"));
}

#[test]
fn rejects_a_placement_record_without_a_reachable_renderer_call() {
    let (data, reachable_instructions, reachable_seeds, mut arguments) = fixture();
    arguments.remove(0);

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("source-bound placement records"));
}

#[test]
fn rejects_an_unexpected_renderer_call_outside_the_bounded_loop() {
    let (data, mut reachable_instructions, mut reachable_seeds, mut arguments) = fixture();
    let seed_offset = 0x1800;
    let instruction_offset = seed_offset + 4;
    reachable_seeds.insert(seed_offset);
    reachable_instructions.insert(instruction_offset);
    arguments.push(ResolvedDirectCallArgument {
        seed_offset,
        instruction_offset,
        target: RENDERER_ADDRESS,
        argument_register: Register::A1,
        value: BASE + 0x0abc,
    });

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("proven four-iteration loop bound")
    );
    assert!(error.to_string().contains("seed=+0x1800/call=+0x1804"));
}

#[test]
fn admits_state_driven_records_from_the_bounded_schedule_without_flow_enumeration() {
    let (data, reachable_instructions, reachable_seeds, arguments) = fixture();
    assert!(arguments.iter().all(|argument| {
        !STATE_DRIVEN_LOOP_CALL_OFFSETS.contains(&argument.instruction_offset)
            && !STATE_DRIVEN_RECORD_OFFSETS
                .iter()
                .any(|offset| argument.value == BASE + *offset as u32)
    }));

    let consumers = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap();

    for source_offset in [0x0720, 0x0770, 0x077c, 0x07e8] {
        assert!(consumers.contains_key(&source_offset));
    }
}

#[test]
fn rejects_state_loop_values_outside_the_source_bound_schedule() {
    let (data, reachable_instructions, mut reachable_seeds, mut arguments) = fixture();
    let seed_offset = 0x7bb0;
    reachable_seeds.insert(seed_offset);
    arguments.push(ResolvedDirectCallArgument {
        seed_offset,
        instruction_offset: STATE_DRIVEN_LOOP_CALL_OFFSETS[0],
        target: RENDERER_ADDRESS,
        argument_register: Register::A1,
        value: BASE + 0x0abc,
    });

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("state-driven schedule bounds"));
    assert!(error.to_string().contains("call=+0x7bd8"));
}

#[test]
fn rejects_a_changed_state_driven_record_schedule() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    data[STATE_DRIVEN_SCHEDULE_OFFSET] = 4;

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("placement schedule changed"));
}

#[test]
fn rejects_a_changed_state_driven_record_stride() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    write_instruction(
        &mut data,
        STATE_DRIVEN_LOOP_CALL_OFFSETS[0] - 0x0c,
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 8,
        },
    );

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("placement loop grammar changed"));
}

#[test]
fn rejects_an_unreachable_state_driven_renderer_call() {
    let (data, mut reachable_instructions, reachable_seeds, arguments) = fixture();
    reachable_instructions.remove(&STATE_DRIVEN_LOOP_CALL_OFFSETS[0]);

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("is not entrypoint-reachable"));
}

#[test]
fn rejects_an_alternate_texture_record_that_looks_like_a_menu_atlas_candidate() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    let dynamic = PLACEMENT_RECORDS
        .iter()
        .find(|record| !record.menu_atlas_candidate)
        .unwrap();
    data[dynamic.string_offset + 2..dynamic.string_offset + 4]
        .copy_from_slice(&0x0047u16.to_le_bytes());

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("MENU-atlas classification"));
}

#[test]
fn rejects_a_renderer_that_no_longer_reads_the_record_string_pointer() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    write_instruction(&mut data, 0x1b74, Instruction::nop());

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("renderer grammar changed"));
}

#[test]
fn binds_edit_save_rows_to_five_markers_after_the_loaded_header() {
    let (data, ..) = fixture();

    validate_edit_save_slot_marker_consumer(&data, BASE).unwrap();
}

#[test]
fn rejects_an_edit_save_row_scan_with_a_different_marker_offset() {
    let (mut data, ..) = fixture();
    write_instruction(
        &mut data,
        0x2414,
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V0,
            offset: 2,
        },
    );

    let error = validate_edit_save_slot_marker_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("save-slot consumer grammar"));
}

#[test]
fn rejects_an_edit_save_row_scan_with_a_different_slot_count() {
    let (mut data, ..) = fixture();
    write_instruction(
        &mut data,
        0x242c,
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 6,
        },
    );

    let error = validate_edit_save_slot_marker_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("save-slot consumer grammar"));
}

#[test]
fn rejects_a_different_actionable_edit_save_marker() {
    let (mut data, ..) = fixture();
    write_instruction(
        &mut data,
        0x3768,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 2,
        },
    );

    let error = validate_edit_save_slot_marker_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("save-slot consumer grammar"));
}

#[test]
fn binds_every_saved_school_selector_to_its_editmoji_cell() {
    let (data, ..) = fixture();

    validate_school_label_consumer(&data, BASE).unwrap();

    assert_eq!(
        SCHOOL_LABEL_CONSUMERS
            .iter()
            .map(|consumer| consumer.selector)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(SCHOOL_LABEL_CONSUMERS[0].source_text, "太陽学園");
    assert_eq!(SCHOOL_LABEL_CONSUMERS[0].cell.x, 440);
    assert_eq!(SCHOOL_LABEL_CONSUMERS[4].source_text, "ジャスティス学園");
    assert_eq!(SCHOOL_LABEL_CONSUMERS[4].cell.x, 512);
}

#[test]
fn rejects_a_school_selector_pointing_at_another_descriptor() {
    let (mut data, ..) = fixture();
    let last_pointer_offset = 0x0bac + 4 * 4;
    data[last_pointer_offset..last_pointer_offset + 4]
        .copy_from_slice(&(BASE + 0x0b5c).to_le_bytes());

    let error = validate_school_label_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("selector 4 descriptor pointer"));
}

#[test]
fn rejects_a_school_selector_read_from_another_saved_field() {
    let (mut data, ..) = fixture();
    write_instruction(
        &mut data,
        0x27d0,
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::S5,
            offset: 0x005b,
        },
    );

    let error = validate_school_label_consumer(&data, BASE).unwrap_err();

    assert!(error.to_string().contains("school-label consumer grammar"));
}

fn fixture() -> (
    Vec<u8>,
    BTreeSet<usize>,
    BTreeSet<usize>,
    Vec<ResolvedDirectCallArgument>,
) {
    let mut data = vec![0u8; 0x83a8];
    for record in PLACEMENT_RECORDS
        .into_iter()
        .chain(MENU_LABEL_RECORDS)
        .chain(STATUS_OVERVIEW_RECORDS)
        .chain(STATUS_RATING_RECORDS)
    {
        let code = if record.menu_atlas_candidate {
            0x0047u16
        } else {
            0xff20u16
        };
        data[record.string_offset..record.string_offset + 2].copy_from_slice(&1u16.to_le_bytes());
        data[record.string_offset + 2..record.string_offset + 4]
            .copy_from_slice(&code.to_le_bytes());
        data[record.record_offset + 4..record.record_offset + 8]
            .copy_from_slice(&(BASE + record.string_offset as u32).to_le_bytes());
    }
    write_renderer_grammar(&mut data);
    write_menu_label_loop_grammar(&mut data);
    write_status_overview_loop_grammar(&mut data);
    write_status_rating_consumer_grammar(&mut data);
    write_state_driven_placement_loop_grammar(&mut data);
    write_edit_save_slot_marker_consumer_grammar(&mut data);
    write_school_label_consumer_grammar(&mut data);

    let mut reachable_instructions = BTreeSet::from([
        RENDERER_OFFSET,
        BOUNDED_LOOP_CALL_OFFSET,
        STATUS_RATING_CALL_OFFSET,
    ]);
    reachable_instructions.extend(STATE_DRIVEN_LOOP_CALL_OFFSETS);
    let mut reachable_seeds = BTreeSet::new();
    let mut arguments = PLACEMENT_RECORDS
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            if STATE_DRIVEN_RECORD_OFFSETS.contains(&record.record_offset) {
                return None;
            }
            let in_bounded_loop = BOUNDED_LOOP_RECORD_OFFSETS.contains(&record.record_offset);
            let seed_offset = if in_bounded_loop {
                0x76c4
            } else {
                0x1000 + index * 8
            };
            let instruction_offset = if in_bounded_loop {
                BOUNDED_LOOP_CALL_OFFSET
            } else {
                seed_offset + 4
            };
            reachable_seeds.insert(seed_offset);
            reachable_instructions.insert(instruction_offset);
            Some(ResolvedDirectCallArgument {
                seed_offset,
                instruction_offset,
                target: RENDERER_ADDRESS,
                argument_register: Register::A1,
                value: BASE + record.record_offset as u32,
            })
        })
        .collect::<Vec<_>>();
    for record_offset in [0x098c, 0x0998, 0x09a4, 0x09b0, 0x09bc] {
        let seed_offset = 0x76c4;
        let instruction_offset = BOUNDED_LOOP_CALL_OFFSET;
        reachable_seeds.insert(seed_offset);
        reachable_instructions.insert(instruction_offset);
        arguments.push(ResolvedDirectCallArgument {
            seed_offset,
            instruction_offset,
            target: RENDERER_ADDRESS,
            argument_register: Register::A1,
            value: BASE + record_offset as u32,
        });
    }
    (data, reachable_instructions, reachable_seeds, arguments)
}

fn write_status_rating_consumer_grammar(data: &mut [u8]) {
    for (offset, instruction) in [
        (
            0x59e0,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::SP,
                offset: 0x38,
            },
        ),
        (
            0x59e4,
            Instruction::Jal {
                target: BASE + 0x5508,
            },
        ),
        (
            0x59e8,
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
            0x5a28,
            Instruction::Jal {
                target: RENDERER_ADDRESS,
            },
        ),
        (
            0x5a2c,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x1c,
            },
        ),
        (
            0x5508,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 4,
            },
        ),
        (
            0x5518,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 5,
            },
        ),
        (
            0x551c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: BASE + 0x5648,
            },
        ),
        (
            0x5524,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        ),
        (
            0x552c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x5530,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: -0x5d40,
            },
        ),
        (0x5538, Instruction::Jr { rs: Register::V0 }),
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
                target: BASE + 0x565c,
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
    ] {
        write_instruction(data, offset, instruction);
    }
    for (index, target) in [
        0x800a_7540u32,
        0x800a_7648,
        0x800a_7584,
        0x800a_75c8,
        0x800a_760c,
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 0x82c0 + index * 4;
        data[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
}

fn write_status_overview_loop_grammar(data: &mut [u8]) {
    for (offset, instruction) in [
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
                immediate: 0x00a9,
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
                target: BASE + 0x1f74,
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
                target: BASE + 0x59a8,
            },
        ),
    ] {
        write_instruction(data, offset, instruction);
    }
    for call_offset in [0x59d8, 0x5a44, 0x5aac, 0x5be4] {
        write_instruction(
            data,
            call_offset - 8,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::S5,
                rt: Register::ZERO,
            },
        );
        write_instruction(
            data,
            call_offset,
            Instruction::Jal {
                target: RENDERER_ADDRESS,
            },
        );
    }
    for (index, target) in [
        0x800a_79ccu32,
        0x800a_7a38,
        0x800a_79cc,
        0x800a_79cc,
        0x800a_79cc,
        0x800a_7aa0,
        0x800a_7bd8,
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 0x82d8 + index * 4;
        data[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
}

fn write_school_label_consumer_grammar(data: &mut [u8]) {
    for (offset, instruction) in [
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
            0x27d0,
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
    ] {
        write_instruction(data, offset, instruction);
    }

    for consumer in SCHOOL_LABEL_CONSUMERS {
        let pointer_offset = 0x0bac + usize::from(consumer.selector) * 4;
        data[pointer_offset..pointer_offset + 4]
            .copy_from_slice(&(BASE + consumer.descriptor_offset as u32).to_le_bytes());
        let page_origin_x = match consumer.texture_page {
            0x02c0 => 256,
            0x0300 => 512,
            _ => unreachable!(),
        };
        for (field_offset, value) in [
            (0, 0),
            (2, consumer.texture_page),
            (4, 0x0100),
            (6, (consumer.cell.x - page_origin_x) as u16),
            (8, consumer.cell.y as u16),
            (10, consumer.cell.width as u16),
            (12, consumer.cell.height as u16),
            (14, 0),
        ] {
            let offset = consumer.descriptor_offset + field_offset;
            data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
}

fn write_state_driven_placement_loop_grammar(data: &mut [u8]) {
    data[STATE_DRIVEN_SCHEDULE_OFFSET..STATE_DRIVEN_SCHEDULE_OFFSET + 12]
        .copy_from_slice(&[3, 9, 2, 12, 2, 14, 2, 16, 2, 18, 1, 20]);
    for call_offset in STATE_DRIVEN_LOOP_CALL_OFFSETS {
        for (offset, instruction) in [
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
                    target: BASE + call_offset as u32 + 0x18,
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
                    target: BASE + call_offset as u32 - 0x10,
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
        ] {
            write_instruction(data, offset, instruction);
        }
    }
}

fn write_menu_label_loop_grammar(data: &mut [u8]) {
    for (offset, instruction) in [
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
            0x24b4,
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
                target: BASE + 0x23d8,
            },
        ),
    ] {
        write_instruction(data, offset, instruction);
    }
}

fn write_renderer_grammar(data: &mut [u8]) {
    let instructions = [
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
                target: BASE + 0x76cc,
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
    for (offset, instruction) in instructions {
        write_instruction(data, offset, instruction);
    }
}

fn write_edit_save_slot_marker_consumer_grammar(data: &mut [u8]) {
    data[0x0010..0x0014].copy_from_slice(&(BASE + 0x8270).to_le_bytes());
    data[0x8270..0x827e].copy_from_slice(b"BISLPS-021200\0");
    for (offset, instruction) in [
        (
            0x17bc,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801a,
            },
        ),
        (
            0x17c0,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x2000,
            },
        ),
        (
            0x17d0,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801a,
            },
        ),
        (
            0x17f0,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x2000,
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
                target: BASE + 0x2428,
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
                target: BASE + 0x2410,
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
                target: BASE + 0x3798,
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
                target: BASE + 0x6738,
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
                target: BASE + 0x6714,
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
    ] {
        write_instruction(data, offset, instruction);
    }
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
