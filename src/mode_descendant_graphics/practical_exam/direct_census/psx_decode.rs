use super::*;

pub(super) fn validate_code_span(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
) -> Result<()> {
    ensure!(
        profile.code_start.is_multiple_of(4)
            && profile.code_end.is_multiple_of(4)
            && profile.code_start < profile.code_end
            && profile.code_end < overlay.len(),
        "{consumer:?} practical-exam executable span is invalid"
    );
    ensure!(
        decode_at(overlay, profile.code_end - 8)? == (Instruction::Jr { rs: Register::RA })
            && decode_at(overlay, profile.code_end - 4)? == Instruction::nop(),
        "{consumer:?} practical-exam executable span no longer ends at a return delay slot"
    );
    ensure!(
        profile
            .code_end
            .checked_add(4)
            .is_some_and(|end| end <= overlay.len()),
        "{consumer:?} practical-exam executable/data boundary is truncated"
    );
    let first_data_word = read_word(overlay, profile.code_end)?;
    ensure!(
        first_data_word == profile.first_data_pointer,
        "{consumer:?} practical-exam executable/data boundary changed: expected first pointer {:#010x}, found {first_data_word:#010x}",
        profile.first_data_pointer
    );
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        decode_at(overlay, offset)?;
    }
    Ok(())
}

pub(super) fn locate_get_tpage_calls(
    overlay: &[u8],
    profile: &DirectCensusProfile,
) -> Result<Vec<usize>> {
    let mut calls = Vec::new();
    for load_offset in (profile.code_start..profile.code_end.saturating_sub(11)).step_by(4) {
        let Ok(Instruction::Lw {
            rt: function_register,
            base: table_register,
            offset: GET_TPAGE_FUNCTION_OFFSET,
        }) = decode_at(overlay, load_offset)
        else {
            continue;
        };
        if !has_function_table_load(
            overlay,
            load_offset,
            table_register,
            GET_TPAGE_TABLE_ADDRESS,
            profile.code_start,
        )? {
            continue;
        }
        let call_offset = load_offset + 8;
        ensure!(
            decode_at(overlay, call_offset)?
                == Instruction::Jalr {
                    rd: Register::RA,
                    rs: function_register,
                },
            "practical-exam GetTPage function load at +0x{load_offset:04x} is not followed by its typed call"
        );
        calls.push(call_offset);
    }
    Ok(calls)
}

pub(super) fn has_function_table_load(
    overlay: &[u8],
    function_load_offset: usize,
    table_register: Register,
    table_address: u32,
    code_start: usize,
) -> Result<bool> {
    let first = function_load_offset.saturating_sub(10 * 4).max(code_start);
    for table_load_offset in (first..function_load_offset).step_by(4).rev() {
        let Ok(Instruction::Lw {
            rt,
            base: seed_register,
            offset,
        }) = decode_at(overlay, table_load_offset)
        else {
            continue;
        };
        if rt != table_register || offset != table_address as i16 {
            continue;
        }
        if (table_load_offset + 4..function_load_offset)
            .step_by(4)
            .any(|offset| {
                decode_at(overlay, offset)
                    .map(|instruction| instruction.written_gpr() == Some(table_register))
                    .unwrap_or(true)
            })
        {
            continue;
        }
        let lui_first = table_load_offset.saturating_sub(4 * 4).max(code_start);
        for lui_offset in (lui_first..table_load_offset).step_by(4).rev() {
            if decode_at(overlay, lui_offset)?
                == (Instruction::Lui {
                    rt: seed_register,
                    immediate: (table_address >> 16) as u16,
                })
                && !(lui_offset + 4..table_load_offset)
                    .step_by(4)
                    .any(|offset| {
                        decode_at(overlay, offset)
                            .map(|instruction| instruction.written_gpr() == Some(seed_register))
                            .unwrap_or(true)
                    })
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn locate_function_table_calls(
    overlay: &[u8],
    profile: &DirectCensusProfile,
    table_address: u32,
    function_offset: i16,
) -> Result<Vec<usize>> {
    let mut calls = Vec::new();
    for load_offset in (profile.code_start..profile.code_end.saturating_sub(11)).step_by(4) {
        let Ok(Instruction::Lw {
            rt: function_register,
            base: table_register,
            offset,
        }) = decode_at(overlay, load_offset)
        else {
            continue;
        };
        if offset != function_offset
            || !has_function_table_load(
                overlay,
                load_offset,
                table_register,
                table_address,
                profile.code_start,
            )?
        {
            continue;
        }
        let call_offset = load_offset + 8;
        ensure!(
            decode_at(overlay, call_offset)?
                == Instruction::Jalr {
                    rd: Register::RA,
                    rs: function_register,
                },
            "practical-exam function-table load at +0x{load_offset:04x} is not followed by its typed call"
        );
        calls.push(call_offset);
    }
    Ok(calls)
}

pub(super) fn validate_function_table_call_at(
    overlay: &[u8],
    call_offset: usize,
    table_address: u32,
    function_offset: i16,
) -> Result<Register> {
    let (function_register, table_register) = match decode_at(overlay, call_offset - 8)? {
        Instruction::Lw { rt, base, offset } if offset == function_offset => (rt, base),
        _ => anyhow::bail!(
            "practical-exam function-table call +0x{call_offset:04x} lost its function load"
        ),
    };
    ensure!(
        has_function_table_load(overlay, call_offset - 8, table_register, table_address, 0)?,
        "practical-exam function-table call +0x{call_offset:04x} lost its table provenance"
    );
    ensure_instruction(
        overlay,
        call_offset - 4,
        Instruction::nop(),
        "function-table load-delay slot",
    )?;
    ensure_instruction(
        overlay,
        call_offset,
        Instruction::Jalr {
            rd: Register::RA,
            rs: function_register,
        },
        "function-table call",
    )?;
    Ok(function_register)
}

pub(super) fn find_containing_function_start(
    overlay: &[u8],
    offset: usize,
    code_start: usize,
) -> Result<usize> {
    let first = code_start.min(offset);
    for candidate in (first..offset).step_by(4).rev() {
        if decode_at(overlay, candidate)? == (Instruction::Jr { rs: Register::RA }) {
            return candidate
                .checked_add(8)
                .context("practical-exam function start overflow");
        }
    }
    Ok(first)
}

pub(super) fn argument_move_source(
    overlay: &[u8],
    call_offset: usize,
    argument: Register,
    scan_start: usize,
) -> Result<Option<Register>> {
    let delay_offset = call_offset
        .checked_add(4)
        .context("practical-exam call delay offset overflow")?;
    let delay = decode_at(overlay, delay_offset)?;
    if delay.written_gpr() == Some(argument) {
        return Ok(moved_register(&delay, argument));
    }
    let first = scan_start.max(call_offset.saturating_sub(24 * 4));
    for offset in (first..call_offset).step_by(4).rev() {
        let instruction = decode_at(overlay, offset)?;
        if is_linked_call(&instruction) {
            // A0-A3 are caller-saved.  A value written for an earlier call is
            // not evidence that a later call receives the same argument.
            return Ok(None);
        }
        if instruction.written_gpr() == Some(argument) {
            return Ok(moved_register(&instruction, argument));
        }
    }
    Ok(None)
}

pub(super) fn resolve_affine_pointer_delta_before(
    overlay: &[u8],
    before: usize,
    register: Register,
    origin: Register,
    scan_start: usize,
    depth: usize,
) -> Result<Option<i32>> {
    if register == origin {
        return Ok(Some(0));
    }
    if register == Register::ZERO || depth > 12 {
        return Ok(None);
    }
    let first = before.saturating_sub(96 * 4).max(scan_start);
    for offset in (first..before).step_by(4).rev() {
        let Ok(instruction) = decode_at(overlay, offset) else {
            continue;
        };
        if is_linked_call(&instruction) && is_call_clobbered(register) {
            return Ok(None);
        }
        if instruction.written_gpr() != Some(register) {
            continue;
        }
        let relation = match instruction {
            Instruction::Addiu { rs, immediate, .. } | Instruction::Addi { rs, immediate, .. } => {
                resolve_affine_pointer_delta_before(
                    overlay,
                    offset,
                    rs,
                    origin,
                    scan_start,
                    depth + 1,
                )?
                .and_then(|delta| delta.checked_add(i32::from(immediate)))
            }
            _ => match moved_register(&instruction, register) {
                Some(source) => resolve_affine_pointer_delta_before(
                    overlay,
                    offset,
                    source,
                    origin,
                    scan_start,
                    depth + 1,
                )?,
                None => None,
            },
        };
        return Ok(relation);
    }
    Ok(None)
}

pub(super) fn required_argument_move_source(
    overlay: &[u8],
    call_offset: usize,
    argument: Register,
    scan_start: usize,
) -> Result<Register> {
    let delay_offset = call_offset
        .checked_add(4)
        .context("practical-exam call delay offset overflow")?;
    if let Some(source) = moved_register(&decode_at(overlay, delay_offset)?, argument) {
        return Ok(source);
    }
    let first = scan_start.max(call_offset.saturating_sub(24 * 4));
    for offset in (first..call_offset).step_by(4).rev() {
        let instruction = decode_at(overlay, offset)?;
        if instruction.written_gpr() == Some(argument) {
            return moved_register(&instruction, argument).with_context(|| {
                format!(
                    "practical-exam call +0x{call_offset:04x} argument {argument:?} is not a typed pointer move"
                )
            });
        }
    }
    anyhow::bail!("practical-exam call +0x{call_offset:04x} has no closed writer for {argument:?}")
}

pub(super) fn function_start_table(
    overlay: &[u8],
    profile: &DirectCensusProfile,
) -> Result<Vec<usize>> {
    let mut starts = Vec::new();
    let mut current_start = profile.code_start;
    let mut next_start = None;
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        if next_start == Some(offset) {
            current_start = offset;
            next_start = None;
        }
        starts.push(current_start);
        if decode_at(overlay, offset)? == (Instruction::Jr { rs: Register::RA }) {
            next_start = offset.checked_add(8);
        }
    }
    Ok(starts)
}

pub(super) fn resolve_constant_before(
    overlay: &[u8],
    before: usize,
    register: Register,
    code_start: usize,
    depth: usize,
) -> Result<Option<u32>> {
    if register == Register::ZERO {
        return Ok(Some(0));
    }
    if depth > 12 {
        return Ok(None);
    }
    let first = before.saturating_sub(96 * 4).max(code_start);
    for offset in (first..before).step_by(4).rev() {
        let Ok(instruction) = decode_at(overlay, offset) else {
            continue;
        };
        if is_linked_call(&instruction) && is_call_clobbered(register) {
            return Ok(None);
        }
        if instruction.has_delay_slot() {
            return Ok(None);
        }
        if instruction.written_gpr() != Some(register) {
            continue;
        }
        let source =
            |source| resolve_constant_before(overlay, offset, source, code_start, depth + 1);
        return match instruction {
            Instruction::Lui { immediate, .. } => Ok(Some(u32::from(immediate) << 16)),
            Instruction::Addi { rs, immediate, .. } | Instruction::Addiu { rs, immediate, .. } => {
                Ok(source(rs)?.map(|value| value.wrapping_add_signed(i32::from(immediate))))
            }
            Instruction::Andi { rs, immediate, .. } => {
                Ok(source(rs)?.map(|value| value & u32::from(immediate)))
            }
            Instruction::Ori { rs, immediate, .. } => {
                Ok(source(rs)?.map(|value| value | u32::from(immediate)))
            }
            Instruction::Xori { rs, immediate, .. } => {
                Ok(source(rs)?.map(|value| value ^ u32::from(immediate)))
            }
            Instruction::Sll { rt, shift, .. } => {
                Ok(source(rt)?.map(|value| value.wrapping_shl(u32::from(shift))))
            }
            Instruction::Srl { rt, shift, .. } => {
                Ok(source(rt)?.map(|value| value.wrapping_shr(u32::from(shift))))
            }
            Instruction::Sra { rt, shift, .. } => {
                Ok(source(rt)?.map(|value| ((value as i32) >> u32::from(shift)) as u32))
            }
            Instruction::Addu { rs, rt, .. } | Instruction::Add { rs, rt, .. } => {
                Ok(match (source(rs)?, source(rt)?) {
                    (Some(left), Some(right)) => Some(left.wrapping_add(right)),
                    _ => None,
                })
            }
            Instruction::Subu { rs, rt, .. } | Instruction::Sub { rs, rt, .. } => {
                Ok(match (source(rs)?, source(rt)?) {
                    (Some(left), Some(right)) => Some(left.wrapping_sub(right)),
                    _ => None,
                })
            }
            Instruction::Or { rs, rt, .. } => Ok(match (source(rs)?, source(rt)?) {
                (Some(left), Some(right)) => Some(left | right),
                _ => None,
            }),
            Instruction::And { rs, rt, .. } => Ok(match (source(rs)?, source(rt)?) {
                (Some(left), Some(right)) => Some(left & right),
                _ => None,
            }),
            Instruction::Xor { rs, rt, .. } => Ok(match (source(rs)?, source(rt)?) {
                (Some(left), Some(right)) => Some(left ^ right),
                _ => None,
            }),
            _ => Ok(None),
        };
    }
    Ok(None)
}

pub(super) fn is_linked_call(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Jal { .. }
            | Instruction::Jalr { .. }
            | Instruction::Bltzal { .. }
            | Instruction::Bgezal { .. }
    )
}

pub(super) fn is_call_clobbered(register: Register) -> bool {
    [
        Register::V0,
        Register::V1,
        Register::A0,
        Register::A1,
        Register::A2,
        Register::A3,
        Register::T0,
        Register::T1,
        Register::T2,
        Register::T3,
        Register::T4,
        Register::T5,
        Register::T6,
        Register::T7,
        Register::T8,
        Register::T9,
        Register::RA,
    ]
    .contains(&register)
}

pub(super) fn ensure_runtime_pointer(
    overlay: &[u8],
    lui_offset: usize,
    add_offset: usize,
    register: Register,
    runtime_address: u32,
) -> Result<()> {
    ensure_instruction(
        overlay,
        lui_offset,
        Instruction::Lui {
            rt: register,
            immediate: (runtime_address >> 16) as u16,
        },
        "runtime pointer high address",
    )?;
    ensure_instruction(
        overlay,
        add_offset,
        Instruction::Addiu {
            rt: register,
            rs: register,
            immediate: runtime_address as i16,
        },
        "runtime pointer low address",
    )
}

pub(super) fn ensure_argument_at(
    overlay: &[u8],
    offset: usize,
    register: Register,
    source: Register,
    value: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        offset,
        Instruction::Addiu {
            rt: register,
            rs: source,
            immediate: value,
        },
        "literal argument",
    )
}

pub(super) fn literal_addiu(
    overlay: &[u8],
    offset: usize,
    destination: Register,
    source: Register,
) -> Result<u32> {
    match decode_at(overlay, offset)? {
        Instruction::Addiu { rt, rs, immediate } if rt == destination && rs == source => {
            u32::try_from(immediate).context("practical-exam domain uses a negative addiu")
        }
        _ => anyhow::bail!("practical-exam addiu domain grammar changed at +0x{offset:04x}"),
    }
}

pub(super) fn ensure_increment(overlay: &[u8], offset: usize, register: Register) -> Result<()> {
    ensure_instruction(
        overlay,
        offset,
        Instruction::Addiu {
            rt: register,
            rs: register,
            immediate: 1,
        },
        "loop increment",
    )
}

pub(super) fn ensure_pointer_advance(
    overlay: &[u8],
    offset: usize,
    register: Register,
    amount: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        offset,
        Instruction::Addiu {
            rt: register,
            rs: register,
            immediate: amount,
        },
        "table pointer advance",
    )
}

pub(super) fn slti_limit(
    overlay: &[u8],
    offset: usize,
    register: Register,
) -> Result<(u32, Register)> {
    match decode_at(overlay, offset)? {
        Instruction::Slti { rt, rs, immediate } if rs == register => Ok((
            u32::try_from(immediate).context("practical-exam loop limit is negative")?,
            rt,
        )),
        _ => anyhow::bail!("practical-exam loop-limit grammar changed at +0x{offset:04x}"),
    }
}

pub(super) fn register_writes_in_range(
    overlay: &[u8],
    start: usize,
    end: usize,
    register: Register,
) -> Result<Vec<usize>> {
    ensure!(
        start <= end && start.is_multiple_of(4) && end.is_multiple_of(4),
        "practical-exam register-write census range is invalid"
    );
    let mut writes = Vec::new();
    for offset in (start..end).step_by(4) {
        if decode_at(overlay, offset)?.written_gpr() == Some(register) {
            writes.push(offset);
        }
    }
    Ok(writes)
}

pub(super) fn runtime_address(offset: usize) -> Result<u32> {
    OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("practical-exam runtime address overflow")
}

pub(super) fn moved_register(instruction: &Instruction, destination: Register) -> Option<Register> {
    match instruction {
        Instruction::Addu {
            rd,
            rs,
            rt: Register::ZERO,
        }
        | Instruction::Or {
            rd,
            rs,
            rt: Register::ZERO,
        } if *rd == destination => Some(*rs),
        Instruction::Addu {
            rd,
            rs: Register::ZERO,
            rt,
        }
        | Instruction::Or {
            rd,
            rs: Register::ZERO,
            rt,
        } if *rd == destination => Some(*rt),
        _ => None,
    }
}

pub(super) fn ensure_move(
    overlay: &[u8],
    offset: usize,
    destination: Register,
    source: Register,
) -> Result<()> {
    ensure!(
        moved_register(&decode_at(overlay, offset)?, destination) == Some(source),
        "practical-exam register move changed at +0x{offset:04x}"
    );
    Ok(())
}

pub(super) fn ensure_instruction(
    overlay: &[u8],
    offset: usize,
    expected: Instruction,
    purpose: &str,
) -> Result<()> {
    let found = decode_at(overlay, offset)?;
    ensure!(
        found == expected,
        "practical-exam {purpose} changed at +0x{offset:04x}: expected {expected:?}, found {found:?}"
    );
    Ok(())
}

pub(super) fn ensure_argument_literal(
    overlay: &[u8],
    call_offset: usize,
    register: Register,
    expected: u32,
) -> Result<()> {
    let found = required_argument_literal(overlay, call_offset, register)?;
    ensure!(
        found == expected,
        "practical-exam GetTPage call +0x{call_offset:04x} has {register:?}={found:#x}, expected {expected:#x}"
    );
    Ok(())
}

pub(super) fn required_argument_literal(
    overlay: &[u8],
    call_offset: usize,
    register: Register,
) -> Result<u32> {
    argument_literal(overlay, call_offset, register)?.with_context(|| {
        format!(
            "practical-exam GetTPage call +0x{call_offset:04x} does not have a closed literal for {register:?}"
        )
    })
}

pub(super) fn argument_literal(
    overlay: &[u8],
    call_offset: usize,
    register: Register,
) -> Result<Option<u32>> {
    let delay_offset = call_offset
        .checked_add(4)
        .context("practical-exam GetTPage delay-slot offset overflow")?;
    let delay = decode_at(overlay, delay_offset)?;
    if delay.written_gpr() == Some(register) {
        return Ok(literal_write(&delay, register));
    }

    let first = call_offset.saturating_sub(ARGUMENT_SCAN_INSTRUCTION_LIMIT * 4);
    for offset in (first..call_offset).step_by(4).rev() {
        let instruction = decode_at(overlay, offset)?;
        if instruction.written_gpr() == Some(register) {
            return Ok(literal_write(&instruction, register));
        }
    }
    Ok(None)
}

pub(super) fn literal_write(instruction: &Instruction, register: Register) -> Option<u32> {
    match instruction {
        Instruction::Addiu {
            rt,
            rs: Register::ZERO,
            immediate,
        } if *rt == register => Some(u32::from_ne_bytes(i32::from(*immediate).to_ne_bytes())),
        Instruction::Ori {
            rt,
            rs: Register::ZERO,
            immediate,
        } if *rt == register => Some(u32::from(*immediate)),
        Instruction::Addu {
            rd,
            rs: Register::ZERO,
            rt: Register::ZERO,
        }
        | Instruction::Or {
            rd,
            rs: Register::ZERO,
            rt: Register::ZERO,
        } if *rd == register => Some(0),
        _ => None,
    }
}

pub(super) fn decode_at(overlay: &[u8], offset: usize) -> Result<Instruction> {
    decode(
        read_word(overlay, offset)?,
        OVERLAY_RUNTIME_BASE + u32::try_from(offset)?,
    )
    .with_context(|| format!("failed to decode practical-exam instruction at +0x{offset:04x}"))
}

pub(super) fn read_word(overlay: &[u8], offset: usize) -> Result<u32> {
    let bytes = overlay
        .get(offset..offset + 4)
        .with_context(|| format!("truncated practical-exam instruction at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

pub(super) fn read_halfword(overlay: &[u8], offset: usize) -> Result<u16> {
    let bytes = overlay
        .get(offset..offset + 2)
        .with_context(|| format!("truncated practical-exam halfword at +0x{offset:04x}"))?;
    Ok(u16::from_le_bytes(bytes.try_into()?))
}

pub(super) fn format_offsets(offsets: &[usize]) -> String {
    offsets
        .iter()
        .map(|offset| format!("+0x{offset:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}
