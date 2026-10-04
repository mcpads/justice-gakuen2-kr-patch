use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::{
    MGAME_RUNTIME_REPAIR_HELPER_ORIGIN, NAME_DIALOGUE_RUNTIME_ORIGIN,
    NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN,
};

pub(in crate::name_input) const MGAME_ENTRY_RESUME_ADDRESS: u32 = 0x800a_5ce4;
pub(super) const DIALOGUE_RUNTIME_RELOAD_SIGNATURE: u32 = u32::from_le_bytes(*b" .-_");
pub(super) const DIALOGUE_RUNTIME_RELOAD_SIGNATURE_OFFSET: i16 = 4;

pub(super) fn emit_mgame_reload_wrapper(
    assembler: &mut Assembler,
    dialogue_runtime_byte_count: usize,
) -> Result<()> {
    ensure!(
        dialogue_runtime_byte_count > 0 && dialogue_runtime_byte_count.is_multiple_of(4),
        "MGAME reload wrapper requires a nonempty word-aligned payload"
    );
    let copy_counter_start = u16::try_from(dialogue_runtime_byte_count / 4)?
        .checked_sub(1)
        .context("MGAME reload wrapper copy count underflow")?;

    assembler
        .label("reload_or_repair_dialogue_runtime_before_mgame_entry")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -8,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 4,
        })
        .emit_all(load_address(
            Register::T0,
            NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN,
        ))
        .emit_all(load_address(Register::T1, NAME_DIALOGUE_RUNTIME_ORIGIN))
        .emit_all(load_address(
            Register::T4,
            DIALOGUE_RUNTIME_RELOAD_SIGNATURE,
        ))
        .emit(Instruction::Lw {
            rt: Register::T3,
            base: Register::T0,
            offset: DIALOGUE_RUNTIME_RELOAD_SIGNATURE_OFFSET,
        })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: copy_counter_start,
        })
        .beq(
            Register::T3,
            Register::T4,
            "copy_dialogue_runtime_reload_word",
        )
        .emit(Instruction::Lw {
            rt: Register::T3,
            base: Register::T1,
            offset: DIALOGUE_RUNTIME_RELOAD_SIGNATURE_OFFSET,
        })
        .emit(Instruction::nop())
        .bne(Register::T3, Register::T4, "resume_mgame_entry")
        .emit(Instruction::nop())
        // A fresh MGAME image supplies the complete selected runtime even when
        // a stale destination still retains its signature after combat. Only
        // use sparse live-MGAME repair after scene data replaces the staging.
        .jump("repair_dialogue_runtime_destination")
        .emit(Instruction::nop())
        .label("copy_dialogue_runtime_reload_word")
        .emit(Instruction::Lw {
            rt: Register::T3,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 4,
        })
        .emit(Instruction::Sw {
            rt: Register::T3,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 4,
        })
        .bgtz(Register::T2, "copy_dialogue_runtime_reload_word")
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -1,
        })
        .label("repair_dialogue_runtime_destination")
        .emit(Instruction::Jal {
            target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
        })
        .emit(Instruction::nop())
        .label("resume_mgame_entry")
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 8,
        })
        .emit(Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        })
        .emit(Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x1801,
        })
        .emit(Instruction::J {
            target: MGAME_ENTRY_RESUME_ADDRESS,
        })
        .emit(Instruction::nop());
    Ok(())
}
