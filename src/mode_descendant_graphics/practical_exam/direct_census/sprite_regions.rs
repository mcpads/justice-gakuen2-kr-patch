use super::*;

pub(super) fn audit_direct_sprite_regions(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<Vec<PracticalExamTextureRegion>> {
    match consumer {
        PracticalExamConsumer::BasicsReview => audit_basics_direct_sprite_regions(overlay),
        PracticalExamConsumer::Exam1999 => audit_exam_1999_direct_sprite_regions(overlay),
    }
}

fn audit_basics_direct_sprite_regions(overlay: &[u8]) -> Result<Vec<PracticalExamTextureRegion>> {
    validate_sprite_initialization(overlay, 0x7048, 0x7054, Register::S0, Register::S2)?;
    ensure_instruction(
        overlay,
        0x708c,
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::S1,
            shift: 1,
        },
        "SIKEN p14 UV index x2",
    )?;
    ensure_instruction(
        overlay,
        0x7090,
        Instruction::Addu {
            rd: Register::V0,
            rs: Register::V0,
            rt: Register::S1,
        },
        "SIKEN p14 UV index x3",
    )?;
    validate_indexed_byte_read(
        overlay,
        0x7098,
        0x709c,
        0x70a0,
        Register::V0,
        Register::V1,
        0x800a_2084,
    )?;
    ensure_instruction(
        overlay,
        0x70a8,
        Instruction::Sb {
            rt: Register::V1,
            base: Register::S0,
            offset: 0x14,
        },
        "SIKEN p14 sprite U write",
    )?;
    validate_indexed_byte_read(
        overlay,
        0x70ac,
        0x70b0,
        0x70b4,
        Register::V0,
        Register::V1,
        0x800a_2085,
    )?;
    ensure_instruction(
        overlay,
        0x70dc,
        Instruction::Sb {
            rt: Register::V1,
            base: Register::S0,
            offset: 0x15,
        },
        "SIKEN p14 sprite V write",
    )?;
    validate_literal_sprite_extent(
        overlay,
        Register::S0,
        0x70b8,
        0x70bc,
        56,
        0x70c0,
        0x70c4,
        32,
    )?;
    let p14_cells = read_indexed_uv_cells(overlay, 0x84, 6, 3, 512, 56, 32)?;
    ensure!(
        p14_cells
            == [
                Cell {
                    x: 616,
                    y: 64,
                    width: 56,
                    height: 32,
                },
                Cell {
                    x: 672,
                    y: 64,
                    width: 56,
                    height: 32,
                },
            ],
        "SIKEN p14 direct UV table domain changed"
    );
    // These are only the first six rows stored beside the renderer.  The live
    // 30-byte selector buffer is restored by an unchecked byte copy outside
    // this overlay, so those rows are not a selector-domain proof.  The typed
    // reader below establishes only nonzero u8 -> index 0..=254.
    validate_nonzero_byte_selector(
        overlay,
        0x6fc0,
        0x6fc8,
        0x6fcc,
        Register::S1,
        Register::S5,
        0,
        0x7120,
    )?;
    let p14_selector_cells = read_byte_selector_uv_cells(overlay, 0x84, 255, 3, 512, 56, 32)?;

    validate_sprite_initialization(overlay, 0x72b8, 0x72c4, Register::S0, Register::S2)?;
    validate_siken_decimal_remainder(overlay)?;
    ensure_instruction(
        overlay,
        0x72fc,
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::S1,
            shift: 2,
        },
        "SIKEN numeric U x4",
    )?;
    ensure_instruction(
        overlay,
        0x7300,
        Instruction::Addu {
            rd: Register::V0,
            rs: Register::V0,
            rt: Register::S1,
        },
        "SIKEN numeric U x5",
    )?;
    ensure_instruction(
        overlay,
        0x7304,
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::V0,
            shift: 2,
        },
        "SIKEN numeric U x20",
    )?;
    ensure_instruction(
        overlay,
        0x7308,
        Instruction::Sb {
            rt: Register::V0,
            base: Register::S0,
            offset: 0x14,
        },
        "SIKEN numeric sprite U write",
    )?;
    ensure_instruction(
        overlay,
        0x732c,
        Instruction::Sb {
            rt: Register::ZERO,
            base: Register::S0,
            offset: 0x15,
        },
        "SIKEN numeric sprite V write",
    )?;
    validate_shared_literal_sprite_extent(overlay, Register::S0, 0x730c, 0x7310, 0x7314, 20)?;

    let mut regions = vec![PracticalExamTextureRegion {
        // The dividend feeding the signed remainder is external to this
        // overlay.  Protect the complete U-byte strip instead of assuming
        // that the remainder is non-negative.
        id: "siken-p12-direct-numeric-byte-domain",
        cell: Cell {
            x: 0,
            y: 0,
            width: 256,
            height: 20,
        },
    }];
    regions.extend(
        p14_selector_cells
            .into_iter()
            .map(|cell| PracticalExamTextureRegion {
                id: "siken-p14-nonzero-byte-selector-domain",
                cell,
            }),
    );
    Ok(regions)
}

fn audit_exam_1999_direct_sprite_regions(
    overlay: &[u8],
) -> Result<Vec<PracticalExamTextureRegion>> {
    validate_sprite_initialization(overlay, 0x68e0, 0x68ec, Register::S1, Register::S5)?;
    ensure_runtime_pointer(overlay, 0x685c, 0x6860, Register::S0, 0x800a_20e0)?;
    validate_halfword_table_sprite_fields(
        overlay,
        Register::S0,
        Register::S1,
        0x6984,
        0x698c,
        Register::V0,
        0x6990,
        0x69a4,
        Register::V1,
        0x6994,
        0x6998,
        16,
        0x699c,
        0x69a0,
        20,
    )?;
    let pair_cells = read_halfword_uv_cells(overlay, 0xe0, 2, 4, 2, 3, 0, 16, 20)?;
    let pair_union = contiguous_horizontal_union(&pair_cells, 16, 20)?;
    ensure!(
        pair_union
            == Cell {
                x: 160,
                y: 212,
                width: 32,
                height: 20,
            },
        "SIKEN2 p12 direct-pair UV table domain changed"
    );
    validate_exam_pair_loop(overlay)?;

    for layout in [
        DigitSpriteLayout {
            pointer_offset: 0x6b74,
            sprite_call: 0x6b80,
            packet_register: Register::S1,
            sprite_register: Register::S5,
            index_offset: 0x6b88,
            index_register: Register::S0,
            digit_register: Register::S4,
            pointer_lui_offset: 0x6b8c,
            pointer_add_offset: 0x6b90,
            pointer_add_result: Register::V0,
            pointer_sum_offset: 0x6b94,
            u_load_offset: 0x6c10,
            u_store_offset: 0x6c18,
            u_register: Register::V0,
            v_load_offset: 0x6c1c,
            v_store_offset: 0x6c38,
            v_register: Register::V1,
            extent_literal_offset: 0x6c20,
            width_store_offset: 0x6c24,
            height_store_offset: 0x6c28,
        },
        DigitSpriteLayout {
            pointer_offset: 0x6d84,
            sprite_call: 0x6d90,
            packet_register: Register::S0,
            sprite_register: Register::S2,
            index_offset: 0x6d98,
            index_register: Register::S1,
            digit_register: Register::S1,
            pointer_lui_offset: 0x6d9c,
            pointer_add_offset: 0x6da0,
            pointer_add_result: Register::V0,
            pointer_sum_offset: 0x6da4,
            u_load_offset: 0x6e20,
            u_store_offset: 0x6e28,
            u_register: Register::V0,
            v_load_offset: 0x6e2c,
            v_store_offset: 0x6e48,
            v_register: Register::V1,
            extent_literal_offset: 0x6e30,
            width_store_offset: 0x6e34,
            height_store_offset: 0x6e38,
        },
        DigitSpriteLayout {
            pointer_offset: 0x6f74,
            sprite_call: 0x6f80,
            packet_register: Register::S0,
            sprite_register: Register::S2,
            index_offset: 0x6f88,
            index_register: Register::S1,
            digit_register: Register::S1,
            pointer_lui_offset: 0x6f8c,
            pointer_add_offset: 0x6f90,
            pointer_add_result: Register::V0,
            pointer_sum_offset: 0x6f94,
            u_load_offset: 0x7010,
            u_store_offset: 0x7018,
            u_register: Register::V0,
            v_load_offset: 0x701c,
            v_store_offset: 0x7038,
            v_register: Register::V1,
            extent_literal_offset: 0x7020,
            width_store_offset: 0x7024,
            height_store_offset: 0x7028,
        },
    ] {
        validate_digit_sprite(overlay, layout)?;
    }
    let digit_cells = read_indexed_uv_cells(overlay, 0xa14, 10, 2, 0, 20, 20)?;
    let digit_union = contiguous_horizontal_union(&digit_cells, 20, 20)?;
    ensure!(
        digit_union
            == Cell {
                x: 0,
                y: 0,
                width: 200,
                height: 20,
            },
        "SIKEN2 p12 numeric UV table domain changed"
    );

    validate_sprite_initialization(overlay, 0x72fc, 0x7308, Register::S1, Register::S3)?;
    ensure_instruction(
        overlay,
        0x7334,
        Instruction::Sll {
            rd: Register::V1,
            rt: Register::S0,
            shift: 1,
        },
        "SIKEN2 p14 UV index x2",
    )?;
    ensure_instruction(
        overlay,
        0x7338,
        Instruction::Addu {
            rd: Register::V1,
            rs: Register::V1,
            rt: Register::S0,
        },
        "SIKEN2 p14 UV index x3",
    )?;
    validate_indexed_byte_read(
        overlay,
        0x7348,
        0x734c,
        0x7350,
        Register::V1,
        Register::V0,
        0x800a_2138,
    )?;
    ensure_instruction(
        overlay,
        0x7358,
        Instruction::Sb {
            rt: Register::V0,
            base: Register::S1,
            offset: 0x14,
        },
        "SIKEN2 p14 sprite U write",
    )?;
    validate_indexed_byte_read(
        overlay,
        0x735c,
        0x7360,
        0x7364,
        Register::V1,
        Register::V1,
        0x800a_2139,
    )?;
    ensure_instruction(
        overlay,
        0x738c,
        Instruction::Sb {
            rt: Register::V1,
            base: Register::S1,
            offset: 0x15,
        },
        "SIKEN2 p14 sprite V write",
    )?;
    validate_literal_sprite_extent(
        overlay,
        Register::S1,
        0x7368,
        0x736c,
        56,
        0x7370,
        0x7374,
        32,
    )?;
    let p14_cells = read_indexed_uv_cells(overlay, 0x138, 6, 3, 512, 56, 32)?;
    ensure!(
        p14_cells
            == [
                Cell {
                    x: 616,
                    y: 64,
                    width: 56,
                    height: 32,
                },
                Cell {
                    x: 672,
                    y: 64,
                    width: 56,
                    height: 32,
                },
            ],
        "SIKEN2 p14 direct UV table domain changed"
    );
    // The normal writer computes values 1..=6, but this buffer also belongs to
    // externally restored state.  Keep the renderer's complete nonzero-byte
    // domain unless the producer lifecycle, including restore, is closed.
    validate_nonzero_byte_selector(
        overlay,
        0x7260,
        0x7268,
        0x726c,
        Register::S0,
        Register::AT,
        0x5af2,
        0x7454,
    )?;
    let p14_selector_cells = read_byte_selector_uv_cells(overlay, 0x138, 255, 3, 512, 56, 32)?;

    let mut regions = vec![PracticalExamTextureRegion {
        // The three numeric paths do not carry an in-overlay proof that
        // their table index remains 0..=9.  A byte U/V pair can address
        // any pixel on p12, so protect the whole page.
        id: "siken2-p12-external-digit-selector-page-domain",
        cell: Cell {
            x: 0,
            y: 0,
            width: 256,
            height: 256,
        },
    }];
    regions.extend(
        p14_selector_cells
            .into_iter()
            .map(|cell| PracticalExamTextureRegion {
                id: "siken2-p14-nonzero-byte-selector-domain",
                cell,
            }),
    );
    Ok(regions)
}

fn validate_exam_pair_loop(overlay: &[u8]) -> Result<()> {
    ensure_move(overlay, 0x6868, Register::S2, Register::ZERO)?;
    ensure_increment(overlay, 0x6890, Register::S2)?;
    let (count, comparison) = slti_limit(overlay, 0x69e8, Register::S2)?;
    ensure!(count == 2, "SIKEN2 direct-pair loop count changed");
    ensure_instruction(
        overlay,
        0x69ec,
        Instruction::Bne {
            rs: comparison,
            rt: Register::ZERO,
            target: runtime_address(0x6894)?,
        },
        "SIKEN2 direct-pair loop branch",
    )?;
    ensure_increment(overlay, 0x69f0, Register::S2)?;
    ensure_instruction(
        overlay,
        0x69f4,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: -1,
        },
        "SIKEN2 direct-pair terminal index restore",
    )
}

#[derive(Clone, Copy)]
struct DigitSpriteLayout {
    pointer_offset: usize,
    sprite_call: usize,
    packet_register: Register,
    sprite_register: Register,
    index_offset: usize,
    index_register: Register,
    digit_register: Register,
    pointer_lui_offset: usize,
    pointer_add_offset: usize,
    pointer_add_result: Register,
    pointer_sum_offset: usize,
    u_load_offset: usize,
    u_store_offset: usize,
    u_register: Register,
    v_load_offset: usize,
    v_store_offset: usize,
    v_register: Register,
    extent_literal_offset: usize,
    width_store_offset: usize,
    height_store_offset: usize,
}

fn validate_digit_sprite(overlay: &[u8], layout: DigitSpriteLayout) -> Result<()> {
    validate_sprite_initialization(
        overlay,
        layout.pointer_offset,
        layout.sprite_call,
        layout.packet_register,
        layout.sprite_register,
    )?;
    ensure_instruction(
        overlay,
        layout.index_offset,
        Instruction::Sll {
            rd: layout.index_register,
            rt: layout.digit_register,
            shift: 1,
        },
        "SIKEN2 numeric UV index",
    )?;
    ensure_runtime_pointer(
        overlay,
        layout.pointer_lui_offset,
        layout.pointer_add_offset,
        layout.pointer_add_result,
        0x800a_2a14,
    )?;
    ensure_instruction(
        overlay,
        layout.pointer_sum_offset,
        Instruction::Addu {
            rd: layout.index_register,
            rs: layout.index_register,
            rt: layout.pointer_add_result,
        },
        "SIKEN2 numeric UV table index",
    )?;
    ensure_instruction(
        overlay,
        layout.u_load_offset,
        Instruction::Lbu {
            rt: layout.u_register,
            base: layout.index_register,
            offset: 0,
        },
        "SIKEN2 numeric U table read",
    )?;
    ensure_instruction(
        overlay,
        layout.u_store_offset,
        Instruction::Sb {
            rt: layout.u_register,
            base: layout.packet_register,
            offset: 0x14,
        },
        "SIKEN2 numeric sprite U write",
    )?;
    ensure_instruction(
        overlay,
        layout.v_load_offset,
        Instruction::Lbu {
            rt: layout.v_register,
            base: layout.index_register,
            offset: 1,
        },
        "SIKEN2 numeric V table read",
    )?;
    ensure_instruction(
        overlay,
        layout.v_store_offset,
        Instruction::Sb {
            rt: layout.v_register,
            base: layout.packet_register,
            offset: 0x15,
        },
        "SIKEN2 numeric sprite V write",
    )?;
    validate_shared_literal_sprite_extent(
        overlay,
        layout.packet_register,
        layout.extent_literal_offset,
        layout.width_store_offset,
        layout.height_store_offset,
        20,
    )
}

fn validate_sprite_initialization(
    overlay: &[u8],
    pointer_offset: usize,
    sprite_call: usize,
    packet_register: Register,
    sprite_register: Register,
) -> Result<()> {
    ensure_instruction(
        overlay,
        pointer_offset,
        Instruction::Addiu {
            rt: sprite_register,
            rs: packet_register,
            immediate: 8,
        },
        "SPRT pointer derivation",
    )?;
    let function_register = validate_function_table_call_at(
        overlay,
        sprite_call,
        GPU_HELPER_TABLE_ADDRESS,
        SET_SPRT_FUNCTION_OFFSET,
    )?;
    ensure!(
        function_register != sprite_register,
        "practical-exam SPRT function pointer aliases its packet pointer"
    );
    ensure_move(overlay, sprite_call + 4, Register::A0, sprite_register)
}

fn validate_indexed_byte_read(
    overlay: &[u8],
    lui_offset: usize,
    add_offset: usize,
    load_offset: usize,
    index_register: Register,
    result_register: Register,
    runtime_address: u32,
) -> Result<()> {
    ensure_instruction(
        overlay,
        lui_offset,
        Instruction::Lui {
            rt: Register::AT,
            immediate: (runtime_address >> 16) as u16,
        },
        "indexed UV table high address",
    )?;
    ensure_instruction(
        overlay,
        add_offset,
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: index_register,
        },
        "indexed UV table address",
    )?;
    ensure_instruction(
        overlay,
        load_offset,
        Instruction::Lbu {
            rt: result_register,
            base: Register::AT,
            offset: runtime_address as i16,
        },
        "indexed UV table byte read",
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_literal_sprite_extent(
    overlay: &[u8],
    packet_register: Register,
    width_literal_offset: usize,
    width_store_offset: usize,
    width: i16,
    height_literal_offset: usize,
    height_store_offset: usize,
    height: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        width_literal_offset,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: width,
        },
        "SPRT width literal",
    )?;
    ensure_instruction(
        overlay,
        width_store_offset,
        Instruction::Sh {
            rt: Register::V0,
            base: packet_register,
            offset: 0x18,
        },
        "SPRT width write",
    )?;
    ensure_instruction(
        overlay,
        height_literal_offset,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: height,
        },
        "SPRT height literal",
    )?;
    ensure_instruction(
        overlay,
        height_store_offset,
        Instruction::Sh {
            rt: Register::V0,
            base: packet_register,
            offset: 0x1a,
        },
        "SPRT height write",
    )
}

fn validate_shared_literal_sprite_extent(
    overlay: &[u8],
    packet_register: Register,
    literal_offset: usize,
    width_store_offset: usize,
    height_store_offset: usize,
    extent: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        literal_offset,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: extent,
        },
        "square SPRT extent literal",
    )?;
    for (offset, field) in [(width_store_offset, 0x18), (height_store_offset, 0x1a)] {
        ensure_instruction(
            overlay,
            offset,
            Instruction::Sh {
                rt: Register::V0,
                base: packet_register,
                offset: field,
            },
            "square SPRT extent write",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_halfword_table_sprite_fields(
    overlay: &[u8],
    table_register: Register,
    packet_register: Register,
    u_load_offset: usize,
    u_store_offset: usize,
    u_register: Register,
    v_load_offset: usize,
    v_store_offset: usize,
    v_register: Register,
    width_literal_offset: usize,
    width_store_offset: usize,
    width: i16,
    height_literal_offset: usize,
    height_store_offset: usize,
    height: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        u_load_offset,
        Instruction::Lhu {
            rt: u_register,
            base: table_register,
            offset: 0,
        },
        "halfword UV table U read",
    )?;
    ensure_instruction(
        overlay,
        u_store_offset,
        Instruction::Sb {
            rt: u_register,
            base: packet_register,
            offset: 0x14,
        },
        "halfword UV sprite U write",
    )?;
    ensure_instruction(
        overlay,
        v_load_offset,
        Instruction::Lhu {
            rt: v_register,
            base: table_register,
            offset: 0,
        },
        "halfword UV table V read",
    )?;
    ensure_instruction(
        overlay,
        v_store_offset,
        Instruction::Sb {
            rt: v_register,
            base: packet_register,
            offset: 0x15,
        },
        "halfword UV sprite V write",
    )?;
    validate_literal_sprite_extent(
        overlay,
        packet_register,
        width_literal_offset,
        width_store_offset,
        width,
        height_literal_offset,
        height_store_offset,
        height,
    )
}

fn validate_siken_decimal_remainder(overlay: &[u8]) -> Result<()> {
    let expected = [
        (
            0x7210,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x6666,
            },
        ),
        (
            0x7214,
            Instruction::Ori {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x6667,
            },
        ),
        (
            0x7224,
            Instruction::Mult {
                rs: Register::T1,
                rt: Register::V0,
            },
        ),
        (
            0x7258,
            Instruction::Sra {
                rd: Register::V0,
                rt: Register::T1,
                shift: 31,
            },
        ),
        (0x725c, Instruction::Mfhi { rd: Register::T2 }),
        (
            0x7260,
            Instruction::Sra {
                rd: Register::V1,
                rt: Register::T2,
                shift: 2,
            },
        ),
        (
            0x7264,
            Instruction::Subu {
                rd: Register::S1,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x7268,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::S1,
                shift: 2,
            },
        ),
        (
            0x726c,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::S1,
            },
        ),
        (
            0x7278,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x7288,
            Instruction::Subu {
                rd: Register::S1,
                rs: Register::T1,
                rt: Register::V0,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(overlay, offset, instruction, "SIKEN decimal remainder")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_nonzero_byte_selector(
    overlay: &[u8],
    load_offset: usize,
    branch_offset: usize,
    subtract_offset: usize,
    selector_register: Register,
    base_register: Register,
    field_offset: i16,
    zero_target_offset: usize,
) -> Result<()> {
    ensure_instruction(
        overlay,
        load_offset,
        Instruction::Lbu {
            rt: selector_register,
            base: base_register,
            offset: field_offset,
        },
        "direct UV byte selector load",
    )?;
    ensure_instruction(
        overlay,
        load_offset + 4,
        Instruction::nop(),
        "direct UV selector load delay",
    )?;
    ensure_instruction(
        overlay,
        branch_offset,
        Instruction::Beq {
            rs: selector_register,
            rt: Register::ZERO,
            target: runtime_address(zero_target_offset)?,
        },
        "direct UV zero-selector branch",
    )?;
    ensure_instruction(
        overlay,
        subtract_offset,
        Instruction::Addiu {
            rt: selector_register,
            rs: selector_register,
            immediate: -1,
        },
        "direct UV one-based selector conversion",
    )
}

fn read_byte_selector_uv_cells(
    overlay: &[u8],
    table_offset: usize,
    selector_count: usize,
    stride: usize,
    page_x: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Cell>> {
    ensure!(stride >= 2, "direct byte-selector UV stride is too small");
    let table = overlay
        .get(table_offset..table_offset + selector_count * stride)
        .context("truncated direct byte-selector UV domain")?;
    let mut cells = Vec::new();
    for entry in table.chunks_exact(stride) {
        for cell in wrapped_texture_cells(
            page_x,
            usize::from(entry[0]),
            usize::from(entry[1]),
            width,
            height,
        ) {
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }
    Ok(cells)
}

fn wrapped_texture_cells(
    page_x: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
) -> Vec<Cell> {
    let horizontal = if u + width <= 256 {
        vec![(u, width)]
    } else {
        vec![(u, 256 - u), (0, u + width - 256)]
    };
    let vertical = if v + height <= 256 {
        vec![(v, height)]
    } else {
        vec![(v, 256 - v), (0, v + height - 256)]
    };
    horizontal
        .into_iter()
        .flat_map(|(x, cell_width)| {
            vertical.iter().copied().map(move |(y, cell_height)| Cell {
                x: page_x + x,
                y,
                width: cell_width,
                height: cell_height,
            })
        })
        .collect()
}

fn read_indexed_uv_cells(
    overlay: &[u8],
    table_offset: usize,
    entry_count: usize,
    stride: usize,
    page_x: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Cell>> {
    ensure!(stride >= 2, "direct UV table stride is too small");
    let table = overlay
        .get(table_offset..table_offset + entry_count * stride)
        .context("truncated direct UV byte table")?;
    let mut coordinates = BTreeSet::new();
    for entry in table.chunks_exact(stride) {
        ensure!(
            entry[2..].iter().all(|value| *value == 0),
            "direct UV byte table has nonzero reserved data"
        );
        coordinates.insert((usize::from(entry[0]), usize::from(entry[1])));
    }
    Ok(coordinates
        .into_iter()
        .map(|(u, v)| Cell {
            x: page_x + u,
            y: v,
            width,
            height,
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn read_halfword_uv_cells(
    overlay: &[u8],
    table_offset: usize,
    entry_count: usize,
    halfword_stride: usize,
    u_index: usize,
    v_index: usize,
    page_x: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Cell>> {
    ensure!(
        u_index < halfword_stride && v_index < halfword_stride,
        "direct halfword UV indexes exceed their record"
    );
    let mut coordinates = BTreeSet::new();
    for index in 0..entry_count {
        let offset = table_offset + index * halfword_stride * 2;
        let u = usize::from(read_halfword(overlay, offset + u_index * 2)?);
        let v = usize::from(read_halfword(overlay, offset + v_index * 2)?);
        coordinates.insert((u, v));
    }
    Ok(coordinates
        .into_iter()
        .map(|(u, v)| Cell {
            x: page_x + u,
            y: v,
            width,
            height,
        })
        .collect())
}

fn contiguous_horizontal_union(cells: &[Cell], width: usize, height: usize) -> Result<Cell> {
    let first = cells.first().context("direct UV cell set is empty")?;
    ensure!(
        cells.iter().enumerate().all(|(index, cell)| {
            cell.x == first.x + index * width
                && cell.y == first.y
                && cell.width == width
                && cell.height == height
        }),
        "direct UV cells are not one closed horizontal domain"
    );
    Ok(Cell {
        x: first.x,
        y: first.y,
        width: cells.len() * width,
        height,
    })
}
