//! Typed renderer, texture-page, and source-coordinate validation.

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Location, PsxR3000A, Register, decode};
use typed_isa_core::{AccessKind, ControlAction, StaticSemantics};

use super::super::super::projection_model::*;
use super::super::source_evidence::{
    ValidationCounts, address_and_span, address_only, validate_offset_range,
};
use super::source_tile_projection::{
    TileProjectionKind, ValidatedTileProjectionGeometry, validate_tile_projection_witness,
};

const GET_TPAGE_TABLE_ADDRESS: u32 = 0x801f_6370;
const GET_TPAGE_FUNCTION_OFFSET: i16 = 0x18;

pub(super) fn validate_static_renderer(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultStaticTileRendererEvidence,
    residency: &PracticalResultVramResidency,
    counts: &mut ValidationCounts,
) -> Result<ValidatedTileProjectionGeometry> {
    address_and_span(
        overlay,
        base,
        &evidence.function_offset,
        &evidence.function_runtime_address,
        evidence.function_size,
        &evidence.function_sha256,
        "static tile renderer",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "direct caller",
            &evidence.direct_caller_offset,
            &evidence.direct_caller_runtime_address,
        ),
        (
            "exported callback",
            &evidence.exported_callback_offset,
            &evidence.exported_callback_runtime_address,
        ),
        (
            "texture-page setup",
            &evidence.texture_page_setup_offset,
            &evidence.texture_page_setup_runtime_address,
        ),
        (
            "GetTPage call",
            &evidence.get_tpage_call_offset,
            &evidence.get_tpage_call_runtime_address,
        ),
        (
            "GetClut call",
            &evidence.get_clut_call_offset,
            &evidence.get_clut_call_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    validate_tile_renderer_residency(
        overlay,
        base,
        residency,
        TileRendererResidencyEvidence {
            function_offset: &evidence.function_offset,
            function_size: evidence.function_size,
            texture_page_setup_offset: &evidence.texture_page_setup_offset,
            get_tpage_call_offset: &evidence.get_tpage_call_offset,
            role: "static tile renderer",
        },
    )?;
    validate_tile_projection_witness(
        overlay,
        base,
        &evidence.function_offset,
        evidence.function_size,
        &evidence.tile_projection_witness,
        TileProjectionKind::Static,
        "static tile renderer",
    )
}

pub(super) fn validate_g_dynamic_renderer(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultGDynamicRendererEvidence,
    residency: &PracticalResultVramResidency,
    counts: &mut ValidationCounts,
) -> Result<ValidatedTileProjectionGeometry> {
    address_and_span(
        overlay,
        base,
        &evidence.function_offset,
        &evidence.function_runtime_address,
        evidence.function_size,
        &evidence.function_sha256,
        "G dynamic renderer",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "G texture-page setup",
            &evidence.texture_page_setup_offset,
            &evidence.texture_page_setup_runtime_address,
        ),
        (
            "G GetTPage call",
            &evidence.get_tpage_call_offset,
            &evidence.get_tpage_call_runtime_address,
        ),
        (
            "G GetClut call",
            &evidence.get_clut_call_offset,
            &evidence.get_clut_call_runtime_address,
        ),
        (
            "G state-row load",
            &evidence.state_row_load_offset,
            &evidence.state_row_load_runtime_address,
        ),
        (
            "G config load",
            &evidence.config_load_offset,
            &evidence.config_load_runtime_address,
        ),
        (
            "G state-column load",
            &evidence.state_column_load_offset,
            &evidence.state_column_load_runtime_address,
        ),
        (
            "G mapped-id load",
            &evidence.mapped_id_load_offset,
            &evidence.mapped_id_load_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    validate_tile_renderer_residency(
        overlay,
        base,
        residency,
        TileRendererResidencyEvidence {
            function_offset: &evidence.function_offset,
            function_size: evidence.function_size,
            texture_page_setup_offset: &evidence.texture_page_setup_offset,
            get_tpage_call_offset: &evidence.get_tpage_call_offset,
            role: "G dynamic tile renderer",
        },
    )?;
    validate_offset_range(
        overlay,
        &evidence.selector_calculation_offset_start,
        &evidence.selector_calculation_offset_end,
        "G selector calculation",
    )?;
    validate_offset_range(
        overlay,
        &evidence.stream_pointer_load_offset_start,
        &evidence.stream_pointer_load_offset_end,
        "G stream pointer load",
    )?;
    validate_tile_projection_witness(
        overlay,
        base,
        &evidence.function_offset,
        evidence.function_size,
        &evidence.tile_projection_witness,
        TileProjectionKind::Dynamic,
        "G dynamic tile renderer",
    )
}

pub(super) fn validate_go_dynamic_renderer(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultGoDynamicRendererEvidence,
    residency: &PracticalResultVramResidency,
    counts: &mut ValidationCounts,
) -> Result<ValidatedTileProjectionGeometry> {
    address_and_span(
        overlay,
        base,
        &evidence.function_offset,
        &evidence.function_runtime_address,
        evidence.function_size,
        &evidence.function_sha256,
        "GO dynamic renderer",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "GO texture-page setup",
            &evidence.texture_page_setup_offset,
            &evidence.texture_page_setup_runtime_address,
        ),
        (
            "GO GetTPage call",
            &evidence.get_tpage_call_offset,
            &evidence.get_tpage_call_runtime_address,
        ),
        (
            "GO GetClut call",
            &evidence.get_clut_call_offset,
            &evidence.get_clut_call_runtime_address,
        ),
        (
            "GO state-row load",
            &evidence.state_row_load_offset,
            &evidence.state_row_load_runtime_address,
        ),
        (
            "GO config load",
            &evidence.config_load_offset,
            &evidence.config_load_runtime_address,
        ),
        (
            "GO state-column load",
            &evidence.state_column_load_offset,
            &evidence.state_column_load_runtime_address,
        ),
        (
            "GO mapped-ID load",
            &evidence.mapped_id_load_offset,
            &evidence.mapped_id_load_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    validate_tile_renderer_residency(
        overlay,
        base,
        residency,
        TileRendererResidencyEvidence {
            function_offset: &evidence.function_offset,
            function_size: evidence.function_size,
            texture_page_setup_offset: &evidence.texture_page_setup_offset,
            get_tpage_call_offset: &evidence.get_tpage_call_offset,
            role: "GO dynamic tile renderer",
        },
    )?;
    validate_offset_range(
        overlay,
        &evidence.selector_calculation_offset_start,
        &evidence.selector_calculation_offset_end,
        "GO selector calculation",
    )?;
    validate_offset_range(
        overlay,
        &evidence.stream_pointer_load_offset_start,
        &evidence.stream_pointer_load_offset_end,
        "GO stream pointer load",
    )?;
    validate_tile_projection_witness(
        overlay,
        base,
        &evidence.function_offset,
        evidence.function_size,
        &evidence.tile_projection_witness,
        TileProjectionKind::Dynamic,
        "GO dynamic tile renderer",
    )
}

struct TileRendererResidencyEvidence<'a> {
    function_offset: &'a str,
    function_size: usize,
    texture_page_setup_offset: &'a str,
    get_tpage_call_offset: &'a str,
    role: &'a str,
}

fn validate_tile_renderer_residency(
    overlay: &[u8],
    base: u32,
    residency: &PracticalResultVramResidency,
    evidence: TileRendererResidencyEvidence<'_>,
) -> Result<()> {
    let TileRendererResidencyEvidence {
        function_offset: function_offset_text,
        function_size,
        texture_page_setup_offset: setup_offset_text,
        get_tpage_call_offset: call_offset_text,
        role,
    } = evidence;
    let function_offset = super::super::source_evidence::parse_hex(
        function_offset_text,
        "tile renderer function offset",
    )?;
    let setup_offset = super::super::source_evidence::parse_hex(
        setup_offset_text,
        "tile texture-page setup offset",
    )?;
    let call_offset =
        super::super::source_evidence::parse_hex(call_offset_text, "tile GetTPage call offset")?;
    let function_end = function_offset
        .checked_add(function_size)
        .context("tile renderer function span overflow")?;
    ensure!(
        function_size > 0
            && function_offset % 4 == 0
            && setup_offset % 4 == 0
            && call_offset % 4 == 0
            && function_offset <= setup_offset
            && setup_offset < call_offset
            && call_offset - setup_offset <= 0x80
            && call_offset
                .checked_add(8)
                .is_some_and(|call_end| call_end <= function_end),
        "{role} texture-page argument window changed"
    );
    validate_get_tpage_call(overlay, base, setup_offset, call_offset, role)?;
    let pixel_mode = match residency.bpp {
        4 => 0,
        8 => 1,
        16 => 2,
        _ => anyhow::bail!("{role} residency has unsupported bpp {}", residency.bpp),
    };
    for (register, expected, argument) in [
        (Register::A0, pixel_mode, "pixel mode"),
        (
            Register::A2,
            u32::try_from(residency.image_vram_word_x)?,
            "texture-page x",
        ),
        (
            Register::A3,
            u32::try_from(residency.image_vram_y)?,
            "texture-page y",
        ),
    ] {
        let found =
            required_literal_argument(overlay, base, setup_offset, call_offset, register, role)?;
        ensure!(
            found == expected,
            "{role} {argument} differs from residency: found {found:#x}, expected {expected:#x}"
        );
    }
    Ok(())
}

fn validate_get_tpage_call(
    overlay: &[u8],
    base: u32,
    setup_offset: usize,
    call_offset: usize,
    role: &str,
) -> Result<()> {
    let function_load_offset = call_offset
        .checked_sub(8)
        .context("tile GetTPage function-load offset underflow")?;
    let load_delay_offset = call_offset
        .checked_sub(4)
        .context("tile GetTPage load-delay offset underflow")?;
    let function_register = match decode_instruction(overlay, base, function_load_offset, role)? {
        Instruction::Lw {
            rt,
            base: table_register,
            offset: GET_TPAGE_FUNCTION_OFFSET,
        } => {
            ensure!(
                get_tpage_table_originates_in_exact_table(
                    overlay,
                    base,
                    setup_offset,
                    function_load_offset,
                    table_register,
                    role,
                )?,
                "{role} GetTPage function pointer no longer originates in the exact function table"
            );
            rt
        }
        instruction => anyhow::bail!(
            "{role} GetTPage function load changed at +0x{function_load_offset:04x}: found {instruction:?}"
        ),
    };
    ensure!(
        decode_instruction(overlay, base, load_delay_offset, role)? == Instruction::nop(),
        "{role} GetTPage function load lost its delay slot"
    );
    ensure!(
        decode_instruction(overlay, base, call_offset, role)?
            == (Instruction::Jalr {
                rd: Register::RA,
                rs: function_register,
            }),
        "{role} GetTPage function load is not followed by its typed call"
    );
    ensure_straight_line_to_get_tpage(overlay, base, setup_offset, function_load_offset, role)
}

fn get_tpage_table_originates_in_exact_table(
    overlay: &[u8],
    base: u32,
    setup_offset: usize,
    function_load_offset: usize,
    table_register: Register,
    role: &str,
) -> Result<bool> {
    for table_load_offset in (setup_offset..function_load_offset).step_by(4).rev() {
        let Instruction::Lw {
            rt,
            base: seed_register,
            offset,
        } = decode_instruction(overlay, base, table_load_offset, role)?
        else {
            continue;
        };
        if rt != table_register || offset != GET_TPAGE_TABLE_ADDRESS as i16 {
            continue;
        }
        let table_load_delay_offset = table_load_offset
            .checked_add(4)
            .context("GetTPage table-load delay offset overflow")?;
        if table_load_delay_offset >= function_load_offset
            || instruction_reads_gpr(overlay, base, table_load_delay_offset, table_register, role)?
        {
            continue;
        }
        if (table_load_offset + 4..function_load_offset)
            .step_by(4)
            .any(|offset| {
                decode_instruction(overlay, base, offset, role)
                    .map(|instruction| instruction.written_gpr() == Some(table_register))
                    .unwrap_or(true)
            })
        {
            continue;
        }
        for lui_offset in (setup_offset..table_load_offset).step_by(4).rev() {
            if decode_instruction(overlay, base, lui_offset, role)?
                == (Instruction::Lui {
                    rt: seed_register,
                    immediate: (GET_TPAGE_TABLE_ADDRESS >> 16) as u16,
                })
                && !(lui_offset + 4..table_load_offset)
                    .step_by(4)
                    .any(|offset| {
                        decode_instruction(overlay, base, offset, role)
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

fn instruction_reads_gpr(
    overlay: &[u8],
    base: u32,
    offset: usize,
    register: Register,
    role: &str,
) -> Result<bool> {
    let instruction = decode_instruction(overlay, base, offset, role)?;
    let pc = base
        .checked_add(u32::try_from(offset)?)
        .context("tile renderer instruction address overflow")?;
    let semantics = PsxR3000A::semantics(&instruction, &pc)
        .with_context(|| format!("failed to resolve {role} semantics at +0x{offset:04x}"))?;
    Ok(semantics.location_accesses.iter().any(|access| {
        access.kind == AccessKind::Read && access.location == Location::Gpr(register)
    }))
}

fn ensure_straight_line_to_get_tpage(
    overlay: &[u8],
    base: u32,
    setup_offset: usize,
    function_load_offset: usize,
    role: &str,
) -> Result<()> {
    for offset in (setup_offset..function_load_offset).step_by(4) {
        let instruction = decode_instruction(overlay, base, offset, role)?;
        let pc = base
            .checked_add(u32::try_from(offset)?)
            .context("tile renderer instruction address overflow")?;
        let semantics = PsxR3000A::semantics(&instruction, &pc)
            .with_context(|| format!("failed to resolve {role} semantics at +0x{offset:04x}"))?;
        ensure!(
            semantics.control_flow.action == ControlAction::Continue
                && semantics.control_flow.delay_slot.is_none()
                && semantics.control_flow.fallthrough == pc.checked_add(4),
            "{role} texture-page arguments no longer flow directly to GetTPage at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn required_literal_argument(
    overlay: &[u8],
    base: u32,
    setup_offset: usize,
    call_offset: usize,
    register: Register,
    role: &str,
) -> Result<u32> {
    let delay_offset = call_offset
        .checked_add(4)
        .context("tile GetTPage delay-slot offset overflow")?;
    let delay = decode_instruction(overlay, base, delay_offset, role)?;
    if delay.written_gpr() == Some(register) {
        return literal_write(&delay, register).with_context(|| {
            format!("{role} GetTPage delay slot writes a non-literal {register:?}")
        });
    }
    for offset in (setup_offset..call_offset).step_by(4).rev() {
        let instruction = decode_instruction(overlay, base, offset, role)?;
        if instruction.written_gpr() == Some(register) {
            return literal_write(&instruction, register).with_context(|| {
                format!("{role} GetTPage setup writes a non-literal {register:?}")
            });
        }
    }
    anyhow::bail!("{role} GetTPage setup does not write {register:?}")
}

fn literal_write(instruction: &Instruction, register: Register) -> Option<u32> {
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

fn decode_instruction(overlay: &[u8], base: u32, offset: usize, role: &str) -> Result<Instruction> {
    let bytes = overlay
        .get(offset..offset + 4)
        .with_context(|| format!("{role} instruction at +0x{offset:04x} is truncated"))?;
    let pc = base
        .checked_add(u32::try_from(offset)?)
        .context("tile renderer instruction address overflow")?;
    decode(u32::from_le_bytes(bytes.try_into()?), pc)
        .with_context(|| format!("failed to decode {role} instruction at +0x{offset:04x}"))
}
