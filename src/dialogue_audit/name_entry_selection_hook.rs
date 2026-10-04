use super::name_entry::OVERLAY_RUNTIME_BASE;
use crate::name_input::{
    NAME_INPUT_MEDIAL_BACKSPACE_ADDRESS, NAME_INPUT_MEDIAL_TRANSITION_ADDRESS,
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
};
use crate::pipeline::sha256_bytes;
use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register as R, encode, load_address};
use serde::Serialize;

pub(super) const HOOK_START_OFFSET: usize = 0x7194;
const BYTE_COUNT: usize = 0x110;
const SOURCE_SHA256: &str = "1b240b7cfb55366bad606e1c558f68815636034f84287eb93a084b2d6c7ac64d";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntrySelectionHookReport {
    pub hook_file_offset: String,
    pub hook_runtime_address: String,
    pub overwritten_byte_count: usize,
    pub selected_key_handler_address: String,
    pub source_instructions_verified: bool,
    pub typed_hook_instruction_count: usize,
    pub replacement_sha256: String,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_selection_hook(
    overlay: &mut [u8],
    handler: u32,
) -> Result<NameEntrySelectionHookReport> {
    ensure!(
        (NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
            ..NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
                + NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY as u32)
            .contains(&handler)
            && handler.is_multiple_of(4),
        "selected name-key handler lies outside the typed redisplay runtime"
    );
    let source = overlay
        .get(HOOK_START_OFFSET..HOOK_START_OFFSET + BYTE_COUNT)
        .context("selected-code routine is truncated")?;
    ensure!(
        sha256_bytes(source) == SOURCE_SHA256,
        "name-entry selected-code routine source changed"
    );
    let origin = OVERLAY_RUNTIME_BASE + HOOK_START_OFFSET as u32;
    psx_r3000a::verify_placed_program(source, origin)?;
    let instructions = build_name_entry_selection_replacement(handler)?;
    let bytes: Vec<_> = instructions
        .iter()
        .enumerate()
        .map(|(i, ins)| encode(ins, origin + i as u32 * 4).map(u32::to_le_bytes))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    overlay[HOOK_START_OFFSET..HOOK_START_OFFSET + BYTE_COUNT].copy_from_slice(&bytes);
    ensure!(
        psx_r3000a::verify_placed_program(&bytes, origin)? == instructions,
        "selection routine readback changed"
    );
    Ok(NameEntrySelectionHookReport {
        hook_file_offset: format!("0x{HOOK_START_OFFSET:04x}"),
        hook_runtime_address: format!("0x{origin:08x}"),
        overwritten_byte_count: BYTE_COUNT,
        selected_key_handler_address: format!("0x{handler:08x}"),
        source_instructions_verified: true,
        typed_hook_instruction_count: instructions.len(),
        replacement_sha256: sha256_bytes(&bytes),
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(crate) fn build_name_entry_selection_replacement(handler: u32) -> Result<Vec<Instruction>> {
    use Instruction::*;
    let origin = OVERLAY_RUNTIME_BASE + HOOK_START_OFFSET as u32;
    let mut a = Assembler::new();
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -32,
    });
    for (rt, offset) in [(R::RA, 24), (R::S0, 16), (R::S1, 20)] {
        a.emit(Sw {
            rt,
            base: R::SP,
            offset,
        });
    }
    a.emit(Addu {
        rd: R::S0,
        rs: R::A0,
        rt: R::ZERO,
    })
    .emit(Lbu {
        rt: R::T0,
        base: R::S0,
        offset: 15,
    })
    .emit(Lbu {
        rt: R::T1,
        base: R::S0,
        offset: 9,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 4,
    })
    .emit(Addu {
        rd: R::A1,
        rs: R::S0,
        rt: R::T0,
    })
    .emit(Addiu {
        rt: R::A1,
        rs: R::A1,
        immediate: 0x12,
    })
    .emit(Lbu {
        rt: R::V0,
        base: R::S0,
        offset: 10,
    })
    .emit(Lb {
        rt: R::T2,
        base: R::S0,
        offset: 12,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T1,
        shift: 2,
    })
    .emit(Sll {
        rd: R::T2,
        rt: R::T2,
        shift: 1,
    })
    .emit_all(load_address(R::T0, 0x8017ad08))
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Lw {
        rt: R::A0,
        base: R::T0,
        offset: 0,
    })
    .emit(Sll {
        rd: R::V0,
        rt: R::V0,
        shift: 1,
    })
    .emit(Addu {
        rd: R::A0,
        rs: R::A0,
        rt: R::T2,
    })
    .emit(Lhu {
        rt: R::V1,
        base: R::A0,
        offset: 0,
    })
    .emit(Jal { target: handler })
    .emit(Addu {
        rd: R::V0,
        rs: R::V0,
        rt: R::A1,
    })
    .emit(Lbu {
        rt: R::T0,
        base: R::S0,
        offset: 15,
    })
    .beq(R::V0, R::ZERO, "selection_return")
    .emit(Sltiu {
        rt: R::T0,
        rs: R::T0,
        immediate: 2,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 1,
    })
    .emit(Addiu {
        rt: R::S1,
        rs: R::T0,
        immediate: 3,
    })
    .emit(Jal { target: 0x80182c08 })
    .emit(Ori {
        rt: R::A0,
        rs: R::ZERO,
        immediate: 0x91,
    })
    .emit(Lbu {
        rt: R::V1,
        base: R::S0,
        offset: 10,
    })
    .emit(Instruction::nop())
    .emit(Sltu {
        rd: R::V0,
        rs: R::V1,
        rt: R::S1,
    })
    .beq(R::V0, R::ZERO, "selection_full")
    .emit(Addiu {
        rt: R::V0,
        rs: R::V1,
        immediate: 1,
    })
    .jump("selection_return")
    .emit(Sb {
        rt: R::V0,
        base: R::S0,
        offset: 10,
    })
    .label("selection_full")
    .emit(Ori {
        rt: R::V0,
        rs: R::ZERO,
        immediate: 96,
    })
    .emit(Sb {
        rt: R::V0,
        base: R::S0,
        offset: 12,
    })
    .label("selection_return");
    for (rt, offset) in [(R::RA, 24), (R::S1, 20), (R::S0, 16)] {
        a.emit(Lw {
            rt,
            base: R::SP,
            offset,
        });
    }
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 32,
    })
    .emit(Jr { rs: R::RA })
    .emit(Instruction::nop());
    let end = origin + a.assemble(origin)?.bytes().len() as u32;
    ensure!(
        end <= NAME_INPUT_MEDIAL_BACKSPACE_ADDRESS,
        "selection bridge overlaps medial backspace: {end:#x}"
    );
    for _ in (end..NAME_INPUT_MEDIAL_BACKSPACE_ADDRESS).step_by(4) {
        a.emit(Instruction::nop());
    }
    // Entered only for a syllable with no final. LO contains payload / 28;
    // T2 is the record pointer and T9 is the native return address.
    a.emit(Mflo { rd: R::A0 })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 21,
        })
        .emit(Divu {
            rs: R::A0,
            rt: R::T0,
        })
        .emit(Mfhi { rd: R::A0 })
        .emit(Mflo { rd: R::T7 })
        .emit(Jal {
            target: NAME_INPUT_MEDIAL_TRANSITION_ADDRESS,
        })
        .emit(Ori {
            rt: R::A1,
            rs: R::ZERO,
            immediate: 31,
        })
        .emit(Addu {
            rd: R::RA,
            rs: R::T9,
            rt: R::ZERO,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 31,
        })
        .beq(R::V0, R::T0, "delete_medial_to_initial")
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 588,
        })
        .emit(Multu {
            rs: R::T7,
            rt: R::T0,
        })
        .emit(Mflo { rd: R::T3 })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 28,
        })
        .emit(Multu {
            rs: R::V0,
            rt: R::T0,
        })
        .emit(Mflo { rd: R::T0 })
        .emit(Addu {
            rd: R::T3,
            rs: R::T3,
            rt: R::T0,
        })
        .jump("delete_medial_write")
        .emit(Ori {
            rt: R::T3,
            rs: R::T3,
            immediate: 0x8000,
        })
        .label("delete_medial_to_initial")
        .emit(Ori {
            rt: R::T3,
            rs: R::T7,
            immediate: 0xc000,
        })
        .label("delete_medial_write")
        .emit(Sh {
            rt: R::T3,
            base: R::T2,
            offset: 0,
        })
        .emit(J { target: 0x80182c08 })
        .emit(Ori {
            rt: R::A0,
            rs: R::ZERO,
            immediate: 0x92,
        });
    let placed = a.assemble(origin)?;
    ensure!(
        placed.bytes().len() <= BYTE_COUNT,
        "selection/backspace routines exceed owned source function: {}",
        placed.bytes().len()
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        placed.instructions(),
        origin,
        "name selection bridge",
    )?;
    let mut instructions = placed.instructions().to_vec();
    instructions.resize(BYTE_COUNT / 4, Instruction::nop());
    Ok(instructions)
}
