use super::*;

pub(super) fn validate_eight_bit_coordinate_domains(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<()> {
    let (computed, table) = match consumer {
        PracticalExamConsumer::BasicsReview => (
            ComputedEightBitLayout {
                call_offset: 0x6d2c,
                outer_zero_offset: 0x6c28,
                inner_zero_offset: 0x6c54,
                y_shift_offset: 0x6c58,
                x_initial_offset: 0x6c5c,
                argument_offsets: [0x6cfc, 0x6d00, 0x6d04, 0x6d08],
                x_step_offset: 0x6d20,
                inner_increment_offset: 0x6d30,
                inner_limit_offset: 0x6d98,
                inner_body_offset: 0x6c64,
                outer_increment_offset: 0x6da4,
                outer_limit_offset: 0x6da8,
                outer_body_offset: 0x6c58,
                outer_register: Register::S3,
                inner_register: Register::S2,
                x_register: Register::S4,
                y_register: Register::S5,
            },
            TableEightBitLayout {
                call_offset: 0x6e7c,
                pointer_lui_offset: 0x6df8,
                pointer_add_offset: 0x6dfc,
                pointer_register: Register::S1,
                table_runtime_address: 0x800a_203c,
                loop_zero_offset: 0x6e04,
                loop_register: Register::S3,
                a2_load_offset: 0x6e2c,
                a3_load_offset: 0x6e34,
                pointer_advance_offsets: [
                    0x6e30, 0x6e38, 0x6ed0, 0x6efc, 0x6f08, 0x6f14, 0x6f20, 0x6f2c, 0x6f58,
                ],
                a0_offset: 0x6e3c,
                a1_offset: 0x6e40,
                loop_increment_offset: 0x6e70,
                loop_limit_offset: 0x6f80,
                loop_body_offset: 0x6e2c,
                table_offset: 0x3c,
            },
        ),
        PracticalExamConsumer::Exam1999 => (
            ComputedEightBitLayout {
                call_offset: 0x6790,
                outer_zero_offset: 0x668c,
                inner_zero_offset: 0x66b8,
                y_shift_offset: 0x66bc,
                x_initial_offset: 0x66c0,
                argument_offsets: [0x6760, 0x6764, 0x6768, 0x676c],
                x_step_offset: 0x6784,
                inner_increment_offset: 0x6794,
                inner_limit_offset: 0x67fc,
                inner_body_offset: 0x66c8,
                outer_increment_offset: 0x6808,
                outer_limit_offset: 0x680c,
                outer_body_offset: 0x66bc,
                outer_register: Register::S3,
                inner_register: Register::S2,
                x_register: Register::S4,
                y_register: Register::S5,
            },
            TableEightBitLayout {
                call_offset: 0x7144,
                pointer_lui_offset: 0x70bc,
                pointer_add_offset: 0x70c0,
                pointer_register: Register::S0,
                table_runtime_address: 0x800a_20f0,
                loop_zero_offset: 0x70c8,
                loop_register: Register::S2,
                a2_load_offset: 0x7128,
                a3_load_offset: 0x7138,
                pointer_advance_offsets: [
                    0x712c, 0x7148, 0x7198, 0x71c4, 0x71d0, 0x71dc, 0x71e8, 0x71f4, 0x7220,
                ],
                a0_offset: 0x70dc,
                a1_offset: 0x70e0,
                loop_increment_offset: 0x70e4,
                loop_limit_offset: 0x7248,
                loop_body_offset: 0x70dc,
                table_offset: 0xf0,
            },
        ),
    };
    let computed_coordinates = validate_computed_eight_bit_domain(overlay, computed)?;
    let table_coordinates = validate_table_eight_bit_domain(overlay, table)?;
    let expected = BTreeSet::from([(0x200, 0), (0x280, 0), (0x200, 0x100), (0x280, 0x100)]);
    ensure!(
        computed_coordinates == expected && table_coordinates == expected,
        "{consumer:?} practical-exam 8bpp coordinate domain changed"
    );
    for (x, y) in expected {
        ensure!(
            y >= 0x100
                || x + EIGHT_BIT_TPAGE_VRAM_WIDTH <= SHARED_PRODUCER_VRAM_X_START
                || x >= SHARED_PRODUCER_VRAM_X_END,
            "{consumer:?} practical-exam 8bpp page ({x:#x},{y:#x}) overlaps the shared 4bpp producer"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ComputedEightBitLayout {
    call_offset: usize,
    outer_zero_offset: usize,
    inner_zero_offset: usize,
    y_shift_offset: usize,
    x_initial_offset: usize,
    argument_offsets: [usize; 4],
    x_step_offset: usize,
    inner_increment_offset: usize,
    inner_limit_offset: usize,
    inner_body_offset: usize,
    outer_increment_offset: usize,
    outer_limit_offset: usize,
    outer_body_offset: usize,
    outer_register: Register,
    inner_register: Register,
    x_register: Register,
    y_register: Register,
}

#[derive(Clone, Copy)]
struct TableEightBitLayout {
    call_offset: usize,
    pointer_lui_offset: usize,
    pointer_add_offset: usize,
    pointer_register: Register,
    table_runtime_address: u32,
    loop_zero_offset: usize,
    loop_register: Register,
    a2_load_offset: usize,
    a3_load_offset: usize,
    pointer_advance_offsets: [usize; 9],
    a0_offset: usize,
    a1_offset: usize,
    loop_increment_offset: usize,
    loop_limit_offset: usize,
    loop_body_offset: usize,
    table_offset: usize,
}

fn validate_computed_eight_bit_domain(
    overlay: &[u8],
    layout: ComputedEightBitLayout,
) -> Result<BTreeSet<(u32, u32)>> {
    ensure_move(
        overlay,
        layout.outer_zero_offset,
        layout.outer_register,
        Register::ZERO,
    )?;
    ensure_move(
        overlay,
        layout.inner_zero_offset,
        layout.inner_register,
        Register::ZERO,
    )?;
    let y_shift = match decode_at(overlay, layout.y_shift_offset)? {
        Instruction::Sll { rd, rt, shift }
            if rd == layout.y_register && rt == layout.outer_register =>
        {
            shift
        }
        _ => anyhow::bail!("practical-exam computed 8bpp Y domain grammar changed"),
    };
    let x_initial = literal_addiu(
        overlay,
        layout.x_initial_offset,
        layout.x_register,
        Register::ZERO,
    )?;
    ensure_argument_at(
        overlay,
        layout.argument_offsets[0],
        Register::A0,
        Register::ZERO,
        1,
    )?;
    ensure_move(
        overlay,
        layout.argument_offsets[1],
        Register::A1,
        Register::ZERO,
    )?;
    ensure_move(
        overlay,
        layout.argument_offsets[2],
        Register::A2,
        layout.x_register,
    )?;
    ensure_move(
        overlay,
        layout.argument_offsets[3],
        Register::A3,
        layout.y_register,
    )?;
    let x_step = literal_addiu(
        overlay,
        layout.x_step_offset,
        layout.x_register,
        layout.x_register,
    )?;
    ensure_increment(
        overlay,
        layout.inner_increment_offset,
        layout.inner_register,
    )?;
    let (inner_count, inner_comparison) =
        slti_limit(overlay, layout.inner_limit_offset, layout.inner_register)?;
    ensure_instruction(
        overlay,
        layout.inner_limit_offset + 4,
        Instruction::Bne {
            rs: inner_comparison,
            rt: Register::ZERO,
            target: runtime_address(layout.inner_body_offset)?,
        },
        "computed 8bpp inner-loop branch",
    )?;
    ensure!(
        ![
            layout.outer_register,
            layout.inner_register,
            layout.x_register,
            layout.y_register
        ]
        .contains(
            &decode_at(overlay, layout.inner_limit_offset + 8)?
                .written_gpr()
                .unwrap_or(Register::ZERO)
        ),
        "practical-exam computed 8bpp inner-loop delay slot changes a domain register"
    );
    ensure_increment(
        overlay,
        layout.outer_increment_offset,
        layout.outer_register,
    )?;
    let (outer_count, outer_comparison) =
        slti_limit(overlay, layout.outer_limit_offset, layout.outer_register)?;
    ensure_instruction(
        overlay,
        layout.outer_limit_offset + 4,
        Instruction::Bne {
            rs: outer_comparison,
            rt: Register::ZERO,
            target: runtime_address(layout.outer_body_offset)?,
        },
        "computed 8bpp outer-loop branch",
    )?;
    ensure_move(
        overlay,
        layout.outer_limit_offset + 8,
        layout.inner_register,
        Register::ZERO,
    )?;
    ensure!(
        matches!(
            decode_at(overlay, layout.call_offset)?,
            Instruction::Jalr {
                rd: Register::RA,
                ..
            },
        ),
        "practical-exam computed 8bpp call changed"
    );

    let mut coordinates = BTreeSet::new();
    for outer in 0..outer_count {
        for inner in 0..inner_count {
            coordinates.insert((
                x_initial + inner * x_step,
                outer
                    .checked_shl(u32::from(y_shift))
                    .context("8bpp Y shift overflow")?,
            ));
        }
    }
    Ok(coordinates)
}

fn validate_table_eight_bit_domain(
    overlay: &[u8],
    layout: TableEightBitLayout,
) -> Result<BTreeSet<(u32, u32)>> {
    ensure_runtime_pointer(
        overlay,
        layout.pointer_lui_offset,
        layout.pointer_add_offset,
        layout.pointer_register,
        layout.table_runtime_address,
    )?;
    ensure_move(
        overlay,
        layout.loop_zero_offset,
        layout.loop_register,
        Register::ZERO,
    )?;
    ensure_instruction(
        overlay,
        layout.a2_load_offset,
        Instruction::Lh {
            rt: Register::A2,
            base: layout.pointer_register,
            offset: 0,
        },
        "8bpp table X read",
    )?;
    ensure_instruction(
        overlay,
        layout.a3_load_offset,
        Instruction::Lh {
            rt: Register::A3,
            base: layout.pointer_register,
            offset: 0,
        },
        "8bpp table Y read",
    )?;
    for offset in layout.pointer_advance_offsets {
        ensure_pointer_advance(overlay, offset, layout.pointer_register, 2)?;
    }
    let found_pointer_writes = register_writes_in_range(
        overlay,
        layout.loop_body_offset,
        layout.loop_limit_offset + 4,
        layout.pointer_register,
    )?;
    ensure!(
        found_pointer_writes == layout.pointer_advance_offsets,
        "practical-exam 8bpp coordinate-table pointer-write denominator changed: expected {}, found {}",
        format_offsets(&layout.pointer_advance_offsets),
        format_offsets(&found_pointer_writes)
    );
    ensure_argument_at(overlay, layout.a0_offset, Register::A0, Register::ZERO, 1)?;
    ensure_move(overlay, layout.a1_offset, Register::A1, Register::ZERO)?;
    ensure_increment(overlay, layout.loop_increment_offset, layout.loop_register)?;
    let (entry_count, comparison_register) =
        slti_limit(overlay, layout.loop_limit_offset, layout.loop_register)?;
    ensure!(
        entry_count == 4,
        "practical-exam 8bpp coordinate-table loop count changed"
    );
    ensure_instruction(
        overlay,
        layout.loop_limit_offset + 4,
        Instruction::Bne {
            rs: comparison_register,
            rt: Register::ZERO,
            target: runtime_address(layout.loop_body_offset)?,
        },
        "table 8bpp loop branch",
    )?;
    ensure!(
        decode_at(overlay, layout.loop_limit_offset + 8)?.written_gpr()
            != Some(layout.loop_register)
            && decode_at(overlay, layout.loop_limit_offset + 8)?.written_gpr()
                != Some(layout.pointer_register),
        "practical-exam table 8bpp loop delay slot changes loop state"
    );
    ensure!(
        matches!(
            decode_at(overlay, layout.call_offset)?,
            Instruction::Jalr {
                rd: Register::RA,
                ..
            }
        ),
        "practical-exam table 8bpp call changed"
    );
    let mut coordinates = BTreeSet::new();
    for index in 0..entry_count {
        let offset = layout.table_offset + usize::try_from(index)? * 18;
        coordinates.insert((
            u32::from(read_halfword(overlay, offset)?),
            u32::from(read_halfword(overlay, offset + 2)?),
        ));
    }
    Ok(coordinates)
}
