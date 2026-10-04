use super::super::NameInputRuntimePackLayout;
use anyhow::Result;
use psx_r3000a::{Assembler, Instruction, Register};

use super::membership_rank::emit_prefixed_membership_rank;

const MODERN_HANGUL_COUNT: i16 = 11_172;
const SYLLABLES_PER_INITIAL: u16 = 588;
const FINALS_PER_MEDIAL: u16 = 28;
const MEDIALS_PER_INITIAL: u16 = 21;
const FINALS_WITHOUT_NONE: u16 = 27;
const FAILURE: u16 = u16::MAX;

pub(crate) fn emit_component_resolver(
    assembler: &mut Assembler,
    pack: &NameInputRuntimePackLayout,
) -> Result<()> {
    let repertoire_membership_offset = i16::try_from(pack.repertoire_membership_byte_range[0])?;
    let no_final_membership_offset = i16::try_from(pack.no_final_membership_byte_range[0])?;
    let final_bearing_membership_offset =
        i16::try_from(pack.final_bearing_membership_byte_range[0])?;
    let final_membership_offset = i16::try_from(pack.final_membership_byte_range[0])?;
    let no_final_rank_prefix_offset = i16::try_from(pack.no_final_rank_prefix_byte_range[0])?;
    let final_bearing_rank_prefix_offset =
        i16::try_from(pack.final_bearing_rank_prefix_byte_range[0])?;
    let final_rank_prefix_offset = i16::try_from(pack.final_rank_prefix_byte_range[0])?;
    let component_mask_offset = i16::try_from(pack.component_mask_byte_range[0])?;
    let bytes_per_component_mask = u16::try_from(pack.bytes_per_component_mask)?;
    let no_final_base_component_count = i16::try_from(pack.no_final_base_component_count)?;
    let base_component_count = i16::try_from(
        pack.no_final_base_component_count + pack.final_bearing_base_component_count,
    )?;
    assembler
        .label("resolve_name_glyph_components")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -24,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: MODERN_HANGUL_COUNT,
        })
        .beq(Register::T0, Register::ZERO, "component_resolution_failed")
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::S0,
            shift: 3,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::T0,
            immediate: repertoire_membership_offset,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::S0,
            immediate: 7,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Sllv {
            rd: Register::T1,
            rt: Register::T1,
            rs: Register::T0,
        })
        .emit(Instruction::And {
            rd: Register::T1,
            rs: Register::V0,
            rt: Register::T1,
        })
        .beq(Register::T1, Register::ZERO, "component_resolution_failed")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: SYLLABLES_PER_INITIAL,
        })
        .emit(Instruction::Divu {
            rs: Register::S0,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::S1 })
        .emit(Instruction::Mfhi { rd: Register::T1 })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FINALS_PER_MEDIAL,
        })
        .emit(Instruction::Divu {
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::S2 })
        .emit(Instruction::Mfhi { rd: Register::S3 })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: MEDIALS_PER_INITIAL,
        })
        .emit(Instruction::Multu {
            rs: Register::S1,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::A2 })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::A2,
            rt: Register::S2,
        })
        .bne(Register::S3, Register::ZERO, "resolve_final_bearing_base")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: no_final_membership_offset,
        })
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: no_final_rank_prefix_offset,
        })
        .emit(Instruction::Ori {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: 2,
        })
        .call("rank_name_glyph_membership")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::V0, Register::T0, "component_resolution_failed")
        .emit(Instruction::Ori {
            rt: Register::V1,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .jump("base_component_rank_ready")
        .emit(Instruction::nop())
        .label("resolve_final_bearing_base")
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: final_bearing_membership_offset,
        })
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: final_bearing_rank_prefix_offset,
        })
        .emit(Instruction::Ori {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: 2,
        })
        .call("rank_name_glyph_membership")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::V0, Register::T0, "component_resolution_failed")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: no_final_base_component_count,
        })
        .label("base_component_rank_ready")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: bytes_per_component_mask,
        })
        .emit(Instruction::Multu {
            rs: Register::V0,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::V0 })
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: component_mask_offset,
        })
        .beq(
            Register::S3,
            Register::ZERO,
            "component_resolution_complete",
        )
        .emit(Instruction::Addu {
            rd: Register::S1,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FINALS_WITHOUT_NONE,
        })
        .emit(Instruction::Multu {
            rs: Register::S2,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::A2 })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::A2,
            rt: Register::S3,
        })
        .emit(Instruction::Addiu {
            rt: Register::A2,
            rs: Register::A2,
            immediate: -1,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: final_membership_offset,
        })
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: final_rank_prefix_offset,
        })
        .emit(Instruction::Ori {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: 1,
        })
        .call("rank_name_glyph_membership")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::V0, Register::T0, "component_resolution_failed")
        .emit(Instruction::Addu {
            rd: Register::V1,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: base_component_count,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: bytes_per_component_mask,
        })
        .emit(Instruction::Multu {
            rs: Register::V1,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::V1 })
        .emit(Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: component_mask_offset,
        })
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::S1,
            rt: Register::ZERO,
        })
        .jump("component_resolution_complete")
        .emit(Instruction::nop())
        .label("component_resolution_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .emit(Instruction::Ori {
            rt: Register::V1,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .label("component_resolution_complete")
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 24,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());

    emit_prefixed_membership_rank(assembler);
    Ok(())
}
