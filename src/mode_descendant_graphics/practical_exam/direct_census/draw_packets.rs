use super::*;

pub(super) fn validate_draw_packet_denominator(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
) -> Result<()> {
    let draw_mode_get_tpage_calls = profile
        .draw_mode_links
        .iter()
        .map(|link| link.get_tpage_call)
        .collect::<BTreeSet<_>>();
    ensure!(
        draw_mode_get_tpage_calls.len() == profile.draw_mode_links.len()
            && draw_mode_get_tpage_calls
                .iter()
                .all(|call| profile.get_tpage_calls.contains(call)),
        "{consumer:?} practical-exam SetDrawMode profile is not a unique GetTPage subset"
    );

    let expected_set_draw_tpage_calls = profile
        .get_tpage_calls
        .iter()
        .filter(|offset| !draw_mode_get_tpage_calls.contains(offset))
        .map(|offset| offset + 0x24)
        .collect::<Vec<_>>();
    let found_set_draw_tpage_calls = locate_function_table_calls(
        overlay,
        profile,
        GPU_HELPER_TABLE_ADDRESS,
        SET_DRAW_TPAGE_FUNCTION_OFFSET,
    )?;
    ensure!(
        found_set_draw_tpage_calls == expected_set_draw_tpage_calls,
        "{consumer:?} practical-exam SetDrawTPage denominator changed: expected {}, found {}",
        format_offsets(&expected_set_draw_tpage_calls),
        format_offsets(&found_set_draw_tpage_calls)
    );

    let expected_set_draw_mode_calls = profile
        .draw_mode_links
        .iter()
        .map(|link| link.get_tpage_call + link.call_delta)
        .collect::<Vec<_>>();
    let found_set_draw_mode_calls = locate_function_table_calls(
        overlay,
        profile,
        GPU_HELPER_TABLE_ADDRESS,
        SET_DRAW_MODE_FUNCTION_OFFSET,
    )?;
    ensure!(
        found_set_draw_mode_calls == expected_set_draw_mode_calls,
        "{consumer:?} practical-exam SetDrawMode denominator changed: expected {}, found {}",
        format_offsets(&expected_set_draw_mode_calls),
        format_offsets(&found_set_draw_mode_calls)
    );

    for get_tpage_call in profile.get_tpage_calls {
        if let Some(link) = profile
            .draw_mode_links
            .iter()
            .find(|link| link.get_tpage_call == *get_tpage_call)
        {
            validate_get_tpage_to_draw_mode_packet_link(overlay, consumer, *link)?;
        } else {
            validate_get_tpage_to_tpage_packet_link(overlay, consumer, *get_tpage_call)?;
        }
    }
    Ok(())
}

fn validate_get_tpage_to_tpage_packet_link(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    get_tpage_call: usize,
) -> Result<()> {
    ensure!(
        decode_at(overlay, get_tpage_call + 4)?.written_gpr() != Some(Register::V0),
        "{consumer:?} GetTPage return is overwritten before packet construction at +0x{get_tpage_call:04x}"
    );
    let packet_register = moved_register(
        &decode_at(overlay, get_tpage_call + 8)?,
        Register::A0,
    )
    .with_context(|| {
        format!(
            "{consumer:?} GetTPage site +0x{get_tpage_call:04x} lost its draw-packet destination"
        )
    })?;
    ensure!(
        packet_register != Register::V0,
        "{consumer:?} GetTPage site +0x{get_tpage_call:04x} aliases its result and packet pointer"
    );
    ensure_move(overlay, get_tpage_call + 0x0c, Register::A1, Register::ZERO)?;
    let table_register = match decode_at(overlay, get_tpage_call + 0x10)? {
        Instruction::Lui {
            rt,
            immediate: 0x801f,
        } => rt,
        _ => anyhow::bail!(
            "{consumer:?} GetTPage site +0x{get_tpage_call:04x} lost its SetDrawTPage table load"
        ),
    };
    ensure_instruction(
        overlay,
        get_tpage_call + 0x14,
        Instruction::Lw {
            rt: table_register,
            base: table_register,
            offset: GPU_HELPER_TABLE_ADDRESS as i16,
        },
        "SetDrawTPage table pointer",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + 0x18,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 1,
        },
        "SetDrawTPage draw-to-display flag",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + 0x1c,
        Instruction::Lw {
            rt: table_register,
            base: table_register,
            offset: SET_DRAW_TPAGE_FUNCTION_OFFSET,
        },
        "SetDrawTPage function pointer",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + 0x20,
        Instruction::nop(),
        "SetDrawTPage load-delay slot",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + 0x24,
        Instruction::Jalr {
            rd: Register::RA,
            rs: table_register,
        },
        "SetDrawTPage call",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + 0x28,
        Instruction::Andi {
            rt: Register::A3,
            rs: Register::V0,
            immediate: u16::MAX,
        },
        "GetTPage result transfer",
    )?;
    Ok(())
}

fn validate_get_tpage_to_draw_mode_packet_link(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    link: DrawModeLink,
) -> Result<()> {
    let get_tpage_call = link.get_tpage_call;
    ensure!(
        decode_at(overlay, get_tpage_call + 4)?.written_gpr() != Some(Register::V0),
        "{consumer:?} GetTPage return is overwritten before DrawMode construction at +0x{get_tpage_call:04x}"
    );
    let packet_register = moved_register(&decode_at(overlay, get_tpage_call + 8)?, Register::A0)
        .with_context(|| {
            format!(
                "{consumer:?} GetTPage site +0x{get_tpage_call:04x} lost its DrawMode destination"
            )
        })?;
    ensure!(
        packet_register != Register::V0,
        "{consumer:?} GetTPage site +0x{get_tpage_call:04x} aliases its result and DrawMode pointer"
    );
    ensure_move(overlay, get_tpage_call + 0x0c, Register::A1, Register::ZERO)?;
    ensure_instruction(
        overlay,
        get_tpage_call + link.draw_to_display_delta,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 1,
        },
        "SetDrawMode draw-to-display flag",
    )?;

    let table_register = match decode_at(overlay, get_tpage_call + link.table_lui_delta)? {
        Instruction::Lui {
            rt,
            immediate: 0x801f,
        } => rt,
        _ => anyhow::bail!(
            "{consumer:?} GetTPage site +0x{get_tpage_call:04x} lost its SetDrawMode table load"
        ),
    };
    ensure_instruction(
        overlay,
        get_tpage_call + link.table_load_delta,
        Instruction::Lw {
            rt: table_register,
            base: table_register,
            offset: GPU_HELPER_TABLE_ADDRESS as i16,
        },
        "SetDrawMode table pointer",
    )?;

    ensure_instruction(
        overlay,
        link.texture_window_origin,
        Instruction::Addiu {
            rt: link.texture_window_register,
            rs: Register::SP,
            immediate: 0x18,
        },
        "SetDrawMode texture-window pointer origin",
    )?;
    let texture_window_store = get_tpage_call + link.texture_window_store_delta;
    ensure!(
        (link.texture_window_origin + 4..texture_window_store)
            .step_by(4)
            .all(|offset| decode_at(overlay, offset)
                .map(|instruction| {
                    instruction.written_gpr() != Some(link.texture_window_register)
                })
                .unwrap_or(false)),
        "{consumer:?} SetDrawMode texture-window pointer is overwritten before +0x{texture_window_store:04x}"
    );
    ensure_instruction(
        overlay,
        texture_window_store,
        Instruction::Sw {
            rt: link.texture_window_register,
            base: Register::SP,
            offset: 0x10,
        },
        "SetDrawMode texture-window argument",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + link.function_load_delta,
        Instruction::Lw {
            rt: table_register,
            base: table_register,
            offset: SET_DRAW_MODE_FUNCTION_OFFSET,
        },
        "SetDrawMode function pointer",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + link.function_load_delta + 4,
        Instruction::nop(),
        "SetDrawMode load-delay slot",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + link.call_delta,
        Instruction::Jalr {
            rd: Register::RA,
            rs: table_register,
        },
        "SetDrawMode call",
    )?;
    ensure_instruction(
        overlay,
        get_tpage_call + link.result_delta,
        Instruction::Andi {
            rt: Register::A3,
            rs: Register::V0,
            immediate: u16::MAX,
        },
        "GetTPage result transfer to SetDrawMode",
    )?;
    Ok(())
}
