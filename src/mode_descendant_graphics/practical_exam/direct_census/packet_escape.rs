use super::*;

pub(super) fn validate_no_inline_draw_mode_packet(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
    packet_submission: &PacketSubmissionAudit,
) -> Result<()> {
    let mut literal_packets = Vec::new();
    let mut constructed_packets = Vec::new();
    // A prebuilt E1 packet may live in the overlay's trailing data tables, so
    // keep the literal-word census over the complete pinned overlay.
    for offset in (0..overlay.len().saturating_sub(3)).step_by(4) {
        let word = read_word(overlay, offset)?;
        if word >> 24 == 0xe1 {
            literal_packets.push(offset);
        }
    }
    // LUI is an executable construction route.  Trailing function-pointer and
    // lookup tables are data and must not be decoded as instructions.
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        let word = read_word(overlay, offset)?;
        if matches!(
            decode(word, OVERLAY_RUNTIME_BASE + u32::try_from(offset)?),
            Ok(Instruction::Lui { immediate, .. }) if immediate >> 8 == 0xe1
        ) {
            constructed_packets.push(offset);
        }
    }
    ensure!(
        literal_packets.is_empty() && constructed_packets.is_empty(),
        "{consumer:?} practical-exam overlay contains uncatalogued direct E1 draw-mode packets: literal {}, constructed {}",
        format_offsets(&literal_packets),
        format_offsets(&constructed_packets)
    );
    validate_no_direct_gpu_port_access(overlay, consumer, profile)?;
    let found_non_stack_word_stores = locate_non_stack_word_stores(overlay, profile)?;
    ensure!(
        found_non_stack_word_stores == profile.non_stack_word_stores,
        "{consumer:?} practical-exam non-stack word-store denominator changed: expected {}, found {}",
        format_offsets(profile.non_stack_word_stores),
        format_offsets(&found_non_stack_word_stores)
    );
    validate_non_packet_word_store_grammars(overlay, consumer)?;
    validate_constant_store_denominator(overlay, consumer, profile, packet_submission)?;
    Ok(())
}

fn locate_non_stack_word_stores(
    overlay: &[u8],
    profile: &DirectCensusProfile,
) -> Result<Vec<usize>> {
    let mut stores = Vec::new();
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        match decode_at(overlay, offset) {
            Ok(Instruction::Sw { base, .. })
            | Ok(Instruction::Swl { base, .. })
            | Ok(Instruction::Swr { base, .. })
                if base != Register::SP =>
            {
                stores.push(offset);
            }
            _ => {}
        }
    }
    Ok(stores)
}

fn validate_no_direct_gpu_port_access(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
) -> Result<()> {
    let mut gpu_port_high_literals = Vec::new();
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        if matches!(
            decode_at(overlay, offset),
            Ok(Instruction::Lui {
                immediate: 0x1f80 | 0x9f80 | 0xbf80,
                ..
            })
        ) {
            gpu_port_high_literals.push(offset);
        }
    }
    ensure!(
        gpu_port_high_literals.is_empty(),
        "{consumer:?} practical-exam overlay gained a direct GPU-register address at {}",
        format_offsets(&gpu_port_high_literals)
    );
    Ok(())
}

fn validate_constant_store_denominator(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
    packet_submission: &PacketSubmissionAudit,
) -> Result<()> {
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        let instruction = decode_at(overlay, offset)?;
        let (source, width) = match instruction {
            Instruction::Sw { rt, .. }
            | Instruction::Swl { rt, .. }
            | Instruction::Swr { rt, .. } => (rt, 4),
            Instruction::Sh { rt, .. } => (rt, 2),
            Instruction::Sb { rt, .. } => (rt, 1),
            _ => continue,
        };
        match resolve_constant_before(overlay, offset, source, profile.code_start, 0)? {
            Some(value) => {
                let bytes = value.to_le_bytes();
                ensure!(
                    !bytes[..width].contains(&0xe1),
                    "{consumer:?} practical-exam store +0x{offset:04x} can materialize an inline E1 command byte from {value:#010x}"
                );
            }
            None => validate_unknown_store_value_route(
                overlay,
                consumer,
                profile,
                packet_submission,
                offset,
                &instruction,
            )?,
        }
    }
    Ok(())
}

fn validate_unknown_store_value_route(
    _overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
    packet_submission: &PacketSubmissionAudit,
    offset: usize,
    instruction: &Instruction,
) -> Result<()> {
    match instruction {
        Instruction::Sw { base, .. }
        | Instruction::Swl { base, .. }
        | Instruction::Swr { base, .. } => {
            ensure!(
                *base == Register::SP || profile.non_stack_word_stores.contains(&offset),
                "{consumer:?} unknown-value word store +0x{offset:04x} escaped the complete word-store destination denominator"
            );
        }
        Instruction::Sb { .. } | Instruction::Sh { .. } => {
            // An unknown byte/halfword is not assumed harmless.  If it occurs
            // in a submitted packet's construction window, the packet audit
            // has already resolved its base alias and rejected writes to the
            // draw command or primitive command byte.  Outside those windows
            // it can reach the GPU only through an additional packet/OT word
            // insertion, whose complete non-stack denominator is closed above.
            if packet_submission.packet_window_stores.contains(&offset) {
                return Ok(());
            }
        }
        _ => anyhow::bail!(
            "{consumer:?} unknown-value store +0x{offset:04x} escaped the typed store denominator"
        ),
    }
    Ok(())
}

pub(super) fn validate_packet_pool_escape_denominator(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
    packet_submission: &PacketSubmissionAudit,
) -> Result<()> {
    let function_starts = function_start_table(overlay, profile)?;
    let mut found_packet_pointer_calls = BTreeSet::new();
    for offset in (profile.code_start..profile.code_end).step_by(4) {
        let instruction = decode_at(overlay, offset)?;
        let function_start = function_starts[(offset - profile.code_start) / 4];
        match instruction {
            Instruction::Sb { rt, base, .. }
            | Instruction::Sh { rt, base, .. }
            | Instruction::Sw { rt, base, .. }
            | Instruction::Swl { rt, base, .. }
            | Instruction::Swr { rt, base, .. } => {
                if register_originates_in_packet_pool(
                    overlay,
                    offset,
                    base,
                    function_start,
                    consumer,
                    profile,
                    0,
                )? {
                    ensure!(
                        packet_submission.packet_window_stores.contains(&offset),
                        "{consumer:?} packet-pool store +0x{offset:04x} escaped the typed construction windows"
                    );
                }
                ensure!(
                    !register_originates_in_packet_pool(
                        overlay,
                        offset,
                        rt,
                        function_start,
                        consumer,
                        profile,
                        0,
                    )?,
                    "{consumer:?} packet-pool pointer escapes through store +0x{offset:04x}"
                );
            }
            _ if is_linked_call(&instruction) => {
                for argument in [Register::A0, Register::A1, Register::A2, Register::A3] {
                    let Some(source) =
                        argument_move_source(overlay, offset, argument, function_start)?
                    else {
                        continue;
                    };
                    if register_originates_in_packet_pool(
                        overlay,
                        offset,
                        source,
                        function_start,
                        consumer,
                        profile,
                        0,
                    )? {
                        found_packet_pointer_calls.insert(offset);
                        ensure!(
                            packet_submission.packet_pointer_calls.contains(&offset),
                            "{consumer:?} packet-pool pointer escapes through untyped call +0x{offset:04x}"
                        );
                    }
                }
            }
            _ => {}
        }
    }
    ensure!(
        found_packet_pointer_calls == packet_submission.packet_pointer_calls,
        "{consumer:?} packet-pointer call denominator changed: expected {}, found {}",
        format_offsets(
            &packet_submission
                .packet_pointer_calls
                .iter()
                .copied()
                .collect::<Vec<_>>()
        ),
        format_offsets(
            &found_packet_pointer_calls
                .iter()
                .copied()
                .collect::<Vec<_>>()
        )
    );
    Ok(())
}

fn validate_non_packet_word_store_grammars(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<()> {
    match consumer {
        PracticalExamConsumer::BasicsReview => {
            for (offset, source, high, displacement) in [
                (0x0e18, Register::V0, 0x800b, -26736),
                (0x0e28, Register::A1, 0x800b, -26740),
                (0x0fa4, Register::V0, 0x800b, -26736),
                (0x0fb4, Register::V1, 0x800b, -26740),
            ] {
                validate_fixed_global_word_store(overlay, offset, source, high, displacement)?;
            }
            ensure_instruction(
                overlay,
                0x2d30,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V1,
                    offset: 0,
                },
                "SIKEN typed object-field copy load",
            )?;
            ensure_instruction(
                overlay,
                0x2d34,
                Instruction::nop(),
                "SIKEN typed object-field copy load delay",
            )?;
            ensure_instruction(
                overlay,
                0x2d38,
                Instruction::Sw {
                    rt: Register::V0,
                    base: Register::A1,
                    offset: 0x10,
                },
                "SIKEN typed object-field copy",
            )?;
            for (offset, source, base) in [
                (0x2dac, Register::ZERO, Register::A0),
                (0x2ff4, Register::A0, Register::V0),
                (0x3954, Register::V0, Register::A0),
            ] {
                ensure_instruction(
                    overlay,
                    offset,
                    Instruction::Sw {
                        rt: source,
                        base,
                        offset: 0x54,
                    },
                    "SIKEN typed object state-field store",
                )?;
            }
            validate_counter_store(
                overlay,
                0x43d8,
                0x43e0,
                0x43e4,
                Register::S4,
                Register::V0,
                Register::V0,
            )?;
            ensure_instruction(
                overlay,
                0x631c,
                Instruction::Subu {
                    rd: Register::A0,
                    rs: Register::A0,
                    rt: Register::V0,
                },
                "SIKEN arithmetic result before mirror store",
            )?;
            ensure_instruction(
                overlay,
                0x6320,
                Instruction::Sw {
                    rt: Register::A0,
                    base: Register::FP,
                    offset: 0,
                },
                "SIKEN arithmetic result store",
            )?;
            ensure_instruction(
                overlay,
                0x6324,
                Instruction::Sw {
                    rt: Register::A0,
                    base: Register::SP,
                    offset: 0x24,
                },
                "SIKEN arithmetic result stack mirror",
            )?;
        }
        PracticalExamConsumer::Exam1999 => {
            for (offset, source, high, displacement) in [
                (0x0ac8, Register::V0, 0x800b, -26708),
                (0x0ad8, Register::A1, 0x800b, -26712),
                (0x0c54, Register::V0, 0x800b, -26708),
                (0x0c64, Register::V1, 0x800b, -26712),
                (0x2200, Register::V0, 0x801f, -1776),
                (0x220c, Register::V0, 0x801f, -752),
                (0x2e70, Register::ZERO, 0x801f, -1772),
                (0x2e78, Register::ZERO, 0x801f, -748),
                (0x2e80, Register::ZERO, 0x801f, -1768),
                (0x2e88, Register::ZERO, 0x801f, -744),
                (0x2ea0, Register::V0, 0x801f, -1776),
                (0x2eb4, Register::V0, 0x801f, -1776),
                (0x2ec0, Register::V0, 0x801f, -752),
            ] {
                validate_fixed_global_word_store(overlay, offset, source, high, displacement)?;
            }
            ensure_instruction(
                overlay,
                0x2450,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0,
                },
                "SIKEN2 typed object-field copy load",
            )?;
            ensure_instruction(
                overlay,
                0x2454,
                Instruction::nop(),
                "SIKEN2 typed object-field copy load delay",
            )?;
            ensure_instruction(
                overlay,
                0x2458,
                Instruction::Sw {
                    rt: Register::V0,
                    base: Register::V1,
                    offset: 0x10,
                },
                "SIKEN2 typed object-field copy",
            )?;
            ensure_instruction(
                overlay,
                0x24c8,
                Instruction::Sw {
                    rt: Register::ZERO,
                    base: Register::V1,
                    offset: 0x54,
                },
                "SIKEN2 typed object state-field store",
            )?;
            validate_counter_store(
                overlay,
                0x3b00,
                0x3b08,
                0x3b0c,
                Register::S4,
                Register::V0,
                Register::V0,
            )?;
            ensure_instruction(
                overlay,
                0x590c,
                Instruction::Subu {
                    rd: Register::A0,
                    rs: Register::A0,
                    rt: Register::V0,
                },
                "SIKEN2 arithmetic result before mirror store",
            )?;
            ensure_instruction(
                overlay,
                0x5910,
                Instruction::Sw {
                    rt: Register::A0,
                    base: Register::FP,
                    offset: 0,
                },
                "SIKEN2 arithmetic result store",
            )?;
            ensure_instruction(
                overlay,
                0x5914,
                Instruction::Sw {
                    rt: Register::A0,
                    base: Register::SP,
                    offset: 0x24,
                },
                "SIKEN2 arithmetic result stack mirror",
            )?;
            for (load, increment, store, base, loaded, written) in [
                (
                    0x68a0,
                    0x68d0,
                    0x68d4,
                    Register::T0,
                    Register::A1,
                    Register::A1,
                ),
                (
                    0x6b34,
                    0x6b64,
                    0x6b68,
                    Register::T0,
                    Register::A1,
                    Register::A1,
                ),
                (
                    0x6d3c,
                    0x6d50,
                    0x6d54,
                    Register::S5,
                    Register::V1,
                    Register::V0,
                ),
                (
                    0x6f2c,
                    0x6f40,
                    0x6f44,
                    Register::S5,
                    Register::V1,
                    Register::V0,
                ),
            ] {
                validate_counter_store(overlay, load, increment, store, base, loaded, written)?;
            }
        }
    }
    Ok(())
}

fn validate_fixed_global_word_store(
    overlay: &[u8],
    store_offset: usize,
    source: Register,
    address_high: u16,
    displacement: i16,
) -> Result<()> {
    ensure_instruction(
        overlay,
        store_offset - 4,
        Instruction::Lui {
            rt: Register::AT,
            immediate: address_high,
        },
        "fixed-global word-store address",
    )?;
    ensure_instruction(
        overlay,
        store_offset,
        Instruction::Sw {
            rt: source,
            base: Register::AT,
            offset: displacement,
        },
        "fixed-global word store",
    )
}

fn validate_counter_store(
    overlay: &[u8],
    load_offset: usize,
    increment_offset: usize,
    store_offset: usize,
    base: Register,
    loaded: Register,
    written: Register,
) -> Result<()> {
    ensure_instruction(
        overlay,
        load_offset,
        Instruction::Lw {
            rt: loaded,
            base,
            offset: 0,
        },
        "counter load",
    )?;
    ensure_instruction(
        overlay,
        increment_offset,
        Instruction::Addiu {
            rt: written,
            rs: loaded,
            immediate: 1,
        },
        "counter increment",
    )?;
    ensure_instruction(
        overlay,
        store_offset,
        Instruction::Sw {
            rt: written,
            base,
            offset: 0,
        },
        "counter store",
    )
}
