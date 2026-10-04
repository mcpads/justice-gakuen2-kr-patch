use super::*;

pub(super) struct PacketSubmissionAudit {
    pub(super) packet_window_stores: BTreeSet<usize>,
    pub(super) packet_pointer_calls: BTreeSet<usize>,
}

pub(super) fn validate_packet_submission_denominator(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
) -> Result<PacketSubmissionAudit> {
    let cat_calls = locate_function_table_calls(
        overlay,
        profile,
        GPU_HELPER_TABLE_ADDRESS,
        CAT_PRIM_FUNCTION_OFFSET,
    )?;
    let add_calls = locate_function_table_calls(
        overlay,
        profile,
        GPU_HELPER_TABLE_ADDRESS,
        ADD_PRIM_FUNCTION_OFFSET,
    )?;
    ensure!(
        cat_calls.len() == profile.get_tpage_calls.len()
            && add_calls.len() == profile.get_tpage_calls.len(),
        "{consumer:?} practical-exam packet-submission denominator changed: GetTPage {}, CatPrim {}, AddPrim {}",
        profile.get_tpage_calls.len(),
        cat_calls.len(),
        add_calls.len()
    );

    let primitive_constructor_calls = PRIMITIVE_CONSTRUCTOR_OFFSETS
        .into_iter()
        .map(|function_offset| {
            locate_function_table_calls(overlay, profile, GPU_HELPER_TABLE_ADDRESS, function_offset)
                .map(|calls| (function_offset, calls))
        })
        .collect::<Result<Vec<_>>>()?;
    let all_constructor_calls = primitive_constructor_calls
        .iter()
        .flat_map(|(_, calls)| calls.iter().copied())
        .collect::<BTreeSet<_>>();
    ensure!(
        all_constructor_calls.len() == profile.get_tpage_calls.len(),
        "{consumer:?} practical-exam primitive-constructor denominator changed: expected {}, found {}",
        profile.get_tpage_calls.len(),
        all_constructor_calls.len()
    );
    let semi_trans_calls = locate_function_table_calls(
        overlay,
        profile,
        GPU_HELPER_TABLE_ADDRESS,
        SET_SEMI_TRANS_FUNCTION_OFFSET,
    )?;
    let mut classified_constructors = BTreeSet::new();
    let mut classified_semi_trans_calls = BTreeSet::new();
    let mut unclassified_packet_window_stores = BTreeSet::new();
    let mut packet_window_stores = BTreeSet::new();
    let mut packet_pointer_calls = BTreeSet::new();
    for (index, get_tpage_call) in profile.get_tpage_calls.iter().copied().enumerate() {
        let cat_call = cat_calls[index];
        let add_call = add_calls[index];
        let next_get_tpage = profile
            .get_tpage_calls
            .get(index + 1)
            .copied()
            .unwrap_or(profile.code_end);
        ensure!(
            get_tpage_call < cat_call && cat_call < add_call && add_call < next_get_tpage,
            "{consumer:?} practical-exam GetTPage +0x{get_tpage_call:04x} lost its ordered CatPrim/AddPrim submission"
        );

        let draw_packet = moved_register(&decode_at(overlay, get_tpage_call + 8)?, Register::A0)
            .with_context(|| {
                format!(
                    "{consumer:?} GetTPage +0x{get_tpage_call:04x} lost its draw-packet pointer"
                )
            })?;
        let function_start =
            find_containing_function_start(overlay, get_tpage_call, profile.code_start)?;
        ensure!(
            register_originates_in_packet_pool(
                overlay,
                get_tpage_call + 8,
                draw_packet,
                function_start,
                consumer,
                profile,
                0,
            )?,
            "{consumer:?} GetTPage +0x{get_tpage_call:04x} draw packet {draw_packet:?} lost its primitive-pool provenance within function +0x{function_start:04x}"
        );
        let cat_draw_packet =
            required_argument_move_source(overlay, cat_call, Register::A0, get_tpage_call)?;
        let primitive_packet =
            required_argument_move_source(overlay, cat_call, Register::A1, get_tpage_call)?;
        let submitted_packet =
            required_argument_move_source(overlay, add_call, Register::A1, cat_call)?;
        ensure!(
            draw_packet == cat_draw_packet
                && draw_packet == submitted_packet
                && primitive_packet != draw_packet,
            "{consumer:?} practical-exam packet chain after GetTPage +0x{get_tpage_call:04x} changed"
        );

        let previous_submission_end = index
            .checked_sub(1)
            .and_then(|previous| add_calls.get(previous).copied())
            .filter(|previous| *previous >= function_start)
            .and_then(|previous| previous.checked_add(8))
            .unwrap_or(function_start);
        let constructor_window_start = function_start.max(previous_submission_end);
        let mut matching_constructors = Vec::new();
        for (function_offset, call) in primitive_constructor_calls
            .iter()
            .flat_map(|(function_offset, calls)| {
                calls
                    .iter()
                    .copied()
                    .map(move |call| (*function_offset, call))
            })
            .filter(|(_, call)| constructor_window_start < *call && *call < cat_call)
        {
            let target = required_argument_move_source(
                overlay,
                call,
                Register::A0,
                constructor_window_start,
            )?;
            if target == primitive_packet {
                matching_constructors.push((function_offset, call));
            }
        }
        ensure!(
            matching_constructors.len() == 1,
            "{consumer:?} practical-exam CatPrim +0x{cat_call:04x} must have exactly one matching typed primitive constructor, found {}",
            matching_constructors.len()
        );
        let (_, constructor_call) = matching_constructors[0];
        classified_constructors.insert(constructor_call);

        for semi_trans_call in semi_trans_calls
            .iter()
            .copied()
            .filter(|call| constructor_call < *call && *call < cat_call)
        {
            let target = required_argument_move_source(
                overlay,
                semi_trans_call,
                Register::A0,
                constructor_call,
            )?;
            let enabled = required_argument_literal(overlay, semi_trans_call, Register::A1)?;
            ensure!(
                target == primitive_packet && enabled <= 1,
                "{consumer:?} SetSemiTrans +0x{semi_trans_call:04x} lost its typed primitive/boolean arguments"
            );
            classified_semi_trans_calls.insert(semi_trans_call);
        }

        ensure!(
            !is_call_clobbered(draw_packet) && !is_call_clobbered(primitive_packet),
            "{consumer:?} practical-exam packet chain after GetTPage +0x{get_tpage_call:04x} stopped keeping its pointers in callee-saved registers"
        );
        ensure_register_not_written(
            overlay,
            get_tpage_call + 0x0c,
            add_call,
            draw_packet,
            "draw-packet pointer",
        )?;
        ensure_register_not_written(
            overlay,
            constructor_call + 8,
            cat_call,
            primitive_packet,
            "primitive-packet pointer",
        )?;

        let primitive_delta = resolve_affine_pointer_delta_before(
            overlay,
            constructor_call,
            primitive_packet,
            draw_packet,
            constructor_window_start,
            0,
        )?
        .with_context(|| {
            format!(
                "{consumer:?} primitive passed at +0x{constructor_call:04x} lost its typed relation to the draw packet"
            )
        })?;
        let draw_packet_size = draw_packet_size(profile, get_tpage_call);
        ensure!(
            primitive_delta == draw_packet_size,
            "{consumer:?} primitive passed at +0x{constructor_call:04x} moved from draw-packet {draw_packet_size:+} to {primitive_delta:+}"
        );

        let draw_helper_call = draw_packet_helper_call(profile, get_tpage_call);
        packet_pointer_calls.insert(draw_helper_call);
        packet_pointer_calls.insert(constructor_call);
        packet_pointer_calls.insert(cat_call);
        packet_pointer_calls.insert(add_call);
        packet_pointer_calls.extend(
            semi_trans_calls
                .iter()
                .copied()
                .filter(|call| constructor_call < *call && *call < cat_call),
        );
        ensure_packet_fields_not_overwritten(
            overlay,
            consumer,
            get_tpage_call,
            draw_packet,
            primitive_packet,
            constructor_window_start,
            constructor_call + 8,
            constructor_call,
            draw_helper_call,
            cat_call,
            add_call,
            draw_packet_size,
            primitive_delta,
            &mut unclassified_packet_window_stores,
            &mut packet_window_stores,
        )?;
    }
    ensure!(
        classified_constructors == all_constructor_calls,
        "{consumer:?} practical-exam primitive-constructor call escaped the GetTPage submission chains"
    );
    ensure!(
        classified_semi_trans_calls == semi_trans_calls.iter().copied().collect(),
        "{consumer:?} practical-exam SetSemiTrans call escaped the typed primitive chains"
    );
    let unclassified_packet_window_stores = unclassified_packet_window_stores
        .into_iter()
        .collect::<Vec<_>>();
    ensure!(
        unclassified_packet_window_stores == profile.packet_window_nonpacket_stores,
        "{consumer:?} practical-exam packet-window nonpacket-store denominator changed: expected {}, found {}",
        format_offsets(profile.packet_window_nonpacket_stores),
        format_offsets(&unclassified_packet_window_stores)
    );
    validate_packet_window_nonpacket_store_grammars(overlay, consumer)?;
    Ok(PacketSubmissionAudit {
        packet_window_stores,
        packet_pointer_calls,
    })
}

fn draw_packet_helper_call(profile: &DirectCensusProfile, get_tpage_call: usize) -> usize {
    profile
        .draw_mode_links
        .iter()
        .find(|link| link.get_tpage_call == get_tpage_call)
        .map(|link| get_tpage_call + link.call_delta)
        .unwrap_or(get_tpage_call + 0x24)
}

fn draw_packet_size(profile: &DirectCensusProfile, get_tpage_call: usize) -> i32 {
    if profile
        .draw_mode_links
        .iter()
        .any(|link| link.get_tpage_call == get_tpage_call)
    {
        12
    } else {
        8
    }
}

fn ensure_register_not_written(
    overlay: &[u8],
    start: usize,
    end: usize,
    register: Register,
    purpose: &str,
) -> Result<()> {
    let writes = register_writes_in_range(overlay, start, end, register)?;
    ensure!(
        writes.is_empty(),
        "practical-exam {purpose} is redefined before submission at {}",
        format_offsets(&writes)
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ensure_packet_fields_not_overwritten(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    get_tpage_call: usize,
    draw_packet: Register,
    primitive_packet: Register,
    function_start: usize,
    packet_fields_start: usize,
    constructor_call: usize,
    draw_helper_call: usize,
    cat_call: usize,
    add_call: usize,
    draw_packet_size: i32,
    primitive_delta: i32,
    unclassified_destinations: &mut BTreeSet<usize>,
    packet_window_stores: &mut BTreeSet<usize>,
) -> Result<()> {
    for offset in (packet_fields_start..add_call).step_by(4) {
        let Ok(instruction) = decode_at(overlay, offset) else {
            continue;
        };
        let (base, displacement, width) = match instruction {
            Instruction::Sb { base, offset, .. } => (base, i32::from(offset), 1),
            Instruction::Sh { base, offset, .. } => (base, i32::from(offset), 2),
            Instruction::Sw { base, offset, .. } => (base, i32::from(offset), 4),
            // These two instructions can touch either side of the nominal
            // address.  Conservatively cover the containing four-byte word.
            Instruction::Swl { base, offset, .. } | Instruction::Swr { base, offset, .. } => {
                (base, i32::from(offset) & !3, 4)
            }
            _ => continue,
        };
        packet_window_stores.insert(offset);
        let Some(base_delta) = resolve_affine_pointer_delta_before(
            overlay,
            offset,
            base,
            draw_packet,
            function_start,
            0,
        )?
        else {
            if base != Register::SP {
                unclassified_destinations.insert(offset);
            }
            continue;
        };
        let start = base_delta
            .checked_add(displacement)
            .context("practical-exam packet store displacement overflow")?;
        let end = start
            .checked_add(width)
            .context("practical-exam packet store width overflow")?;
        ensure!(
            end <= 0 || start >= draw_packet_size,
            "{consumer:?} draw packet from GetTPage +0x{get_tpage_call:04x} is overwritten at +0x{offset:04x}"
        );
        if offset >= constructor_call + 8 {
            let command = primitive_delta + 7;
            ensure!(
                end <= command || start > command,
                "{consumer:?} primitive command byte after constructor +0x{constructor_call:04x} is overwritten at +0x{offset:04x}"
            );
        }
    }
    validate_packet_pointer_call_arguments(
        overlay,
        consumer,
        packet_fields_start,
        add_call,
        draw_packet,
        primitive_packet,
        primitive_delta,
        function_start,
        get_tpage_call,
        constructor_call,
        draw_helper_call,
        cat_call,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_packet_pointer_call_arguments(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    start: usize,
    add_call: usize,
    draw_packet: Register,
    primitive_packet: Register,
    primitive_delta: i32,
    function_start: usize,
    get_tpage_call: usize,
    constructor_call: usize,
    draw_helper_call: usize,
    cat_call: usize,
) -> Result<()> {
    for call_offset in (start..=add_call).step_by(4) {
        let Ok(instruction) = decode_at(overlay, call_offset) else {
            continue;
        };
        if !is_linked_call(&instruction) {
            continue;
        }
        for argument in [Register::A0, Register::A1, Register::A2, Register::A3] {
            let Some(source) =
                argument_move_source(overlay, call_offset, argument, start.saturating_sub(0x100))?
            else {
                continue;
            };
            let Some(source_delta) = resolve_affine_pointer_delta_before(
                overlay,
                call_offset,
                source,
                draw_packet,
                function_start,
                0,
            )?
            else {
                continue;
            };
            let typed = (call_offset == constructor_call
                && argument == Register::A0
                && source == primitive_packet
                && source_delta == primitive_delta)
                || (call_offset == cat_call
                    && ((argument == Register::A0 && source == draw_packet && source_delta == 0)
                        || (argument == Register::A1
                            && source == primitive_packet
                            && source_delta == primitive_delta)))
                || (call_offset == add_call
                    && argument == Register::A1
                    && source == draw_packet
                    && source_delta == 0)
                || (call_offset == draw_helper_call
                    && argument == Register::A0
                    && source == draw_packet
                    && source_delta == 0)
                || (argument == Register::A0
                    && source == primitive_packet
                    && source_delta == primitive_delta
                    && validate_function_table_call_at(
                        overlay,
                        call_offset,
                        GPU_HELPER_TABLE_ADDRESS,
                        SET_SEMI_TRANS_FUNCTION_OFFSET,
                    )
                    .is_ok());
            ensure!(
                typed,
                "{consumer:?} call +0x{call_offset:04x} gained an untyped packet-pointer argument after GetTPage +0x{get_tpage_call:04x}"
            );
        }
    }
    Ok(())
}

pub(super) fn register_originates_in_packet_pool(
    overlay: &[u8],
    before: usize,
    register: Register,
    scan_start: usize,
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
    depth: usize,
) -> Result<bool> {
    if register == Register::ZERO || depth > 16 {
        return Ok(false);
    }
    // A callee-saved packet-pool pointer may be initialized at the function
    // prologue and first consumed hundreds of instructions later.  The
    // containing function is the evidence boundary; an arbitrary instruction
    // window would turn a valid long-lived S register into an unknown origin.
    let first = scan_start;
    for offset in (first..before).step_by(4).rev() {
        let Ok(instruction) = decode_at(overlay, offset) else {
            continue;
        };
        if is_linked_call(&instruction)
            && is_call_clobbered(register)
            && offset.checked_add(4) != Some(before)
        {
            return Ok(false);
        }
        if instruction.written_gpr() != Some(register) {
            continue;
        }
        let packet_pool_displacement = match consumer {
            PracticalExamConsumer::BasicsReview => -26736,
            PracticalExamConsumer::Exam1999 => -26708,
        };
        if let Instruction::Lw {
            rt,
            base,
            offset: displacement,
        } = instruction
            && rt == register
            && base == register
            && displacement == packet_pool_displacement
        {
            let lui_first = offset.saturating_sub(8 * 4).max(scan_start);
            for lui_offset in (lui_first..offset).step_by(4).rev() {
                if decode_at(overlay, lui_offset)?
                    == (Instruction::Lui {
                        rt: register,
                        immediate: 0x800b,
                    })
                    && !(lui_offset + 4..offset).step_by(4).any(|middle| {
                        decode_at(overlay, middle)
                            .map(|candidate| candidate.written_gpr() == Some(register))
                            .unwrap_or(true)
                    })
                {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        if let Some(root) = profile
            .static_packet_arena_roots
            .iter()
            .find(|root| root.writer_offset == offset)
        {
            let resolved = resolve_constant_before(overlay, offset + 4, register, scan_start, 0)?;
            ensure!(
                resolved == Some(root.address),
                "{consumer:?} static packet-arena root +0x{offset:04x} changed"
            );
            return Ok(true);
        }
        let source_has_pool = |source| {
            register_originates_in_packet_pool(
                overlay,
                offset,
                source,
                scan_start,
                consumer,
                profile,
                depth + 1,
            )
        };
        return match instruction {
            Instruction::Addiu { rs, .. }
            | Instruction::Addi { rs, .. }
            | Instruction::Sll { rt: rs, .. }
            | Instruction::Srl { rt: rs, .. }
            | Instruction::Sra { rt: rs, .. } => source_has_pool(rs),
            Instruction::Addu { rs, rt, .. }
            | Instruction::Add { rs, rt, .. }
            | Instruction::Subu { rs, rt, .. }
            | Instruction::Sub { rs, rt, .. }
            | Instruction::Or { rs, rt, .. } => Ok(source_has_pool(rs)? || source_has_pool(rt)?),
            _ => match moved_register(&instruction, register) {
                Some(source) => source_has_pool(source),
                None => Ok(false),
            },
        };
    }
    Ok(false)
}

fn validate_packet_window_nonpacket_store_grammars(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<()> {
    match consumer {
        PracticalExamConsumer::BasicsReview => {
            for store_offset in BASICS_PACKET_WINDOW_NONPACKET_STORES {
                ensure_instruction(
                    overlay,
                    store_offset - 0x1c,
                    Instruction::Lui {
                        rt: Register::V0,
                        immediate: 0x800b,
                    },
                    "SIKEN progress-object pointer high address",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x18,
                    Instruction::Lw {
                        rt: Register::V0,
                        base: Register::V0,
                        offset: -26740,
                    },
                    "SIKEN progress-object pointer load",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x14,
                    Instruction::nop(),
                    "SIKEN progress-object pointer load delay",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x10,
                    Instruction::Lhu {
                        rt: Register::A0,
                        base: Register::V0,
                        offset: 4,
                    },
                    "SIKEN progress-field read",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 4,
                    Instruction::Addiu {
                        rt: Register::A0,
                        rs: Register::A0,
                        immediate: 1,
                    },
                    "SIKEN progress-field increment",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset,
                    Instruction::Sh {
                        rt: Register::A0,
                        base: Register::V0,
                        offset: 4,
                    },
                    "SIKEN progress-field write",
                )?;
            }
        }
        PracticalExamConsumer::Exam1999 => {
            for store_offset in EXAM_1999_PACKET_WINDOW_NONPACKET_STORES {
                ensure_instruction(
                    overlay,
                    store_offset - 0x1c,
                    Instruction::Lui {
                        rt: Register::V0,
                        immediate: 0x800b,
                    },
                    "SIKEN2 progress-object pointer high address",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x18,
                    Instruction::Lw {
                        rt: Register::V0,
                        base: Register::V0,
                        offset: -26712,
                    },
                    "SIKEN2 progress-object pointer load",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x14,
                    Instruction::nop(),
                    "SIKEN2 progress-object pointer load delay",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 0x10,
                    Instruction::Lhu {
                        rt: Register::A0,
                        base: Register::V0,
                        offset: 4,
                    },
                    "SIKEN2 progress-field read",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset - 4,
                    Instruction::Addiu {
                        rt: Register::A0,
                        rs: Register::A0,
                        immediate: 1,
                    },
                    "SIKEN2 progress-field increment",
                )?;
                ensure_instruction(
                    overlay,
                    store_offset,
                    Instruction::Sh {
                        rt: Register::A0,
                        base: Register::V0,
                        offset: 4,
                    },
                    "SIKEN2 progress-field write",
                )?;
            }
        }
    }
    Ok(())
}
