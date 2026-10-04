//! Compact only the registration-list call of KANRI's shared name renderer.
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::kanri_name_runtime::decode_instruction;

const LIST_RETURN_ADDRESS: u32 = 0x800a_4c44;
const NATIVE_ADVANCE: i16 = 20;
const LIST_ADVANCE: i16 = 16;

/// The renderer has already advanced S4 by 20 since the previous glyph.
/// Its saved caller identifies this list; the resolver's own RA does not.
/// Tail-call the existing resolver without changing its arguments or return link.
pub(super) fn emit_list_name_resolver(assembler: &mut Assembler, resolver: u32) {
    assembler
        .beq(Register::S7, Register::ZERO, "resolve_list_name_code")
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::SP,
            offset: 0x34,
        })
        .emit_all(load_address(Register::T1, LIST_RETURN_ADDRESS))
        .bne(Register::T0, Register::T1, "resolve_list_name_code")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::S4,
            rs: Register::S4,
            immediate: LIST_ADVANCE - NATIVE_ADVANCE,
        })
        .label("resolve_list_name_code")
        .emit(Instruction::J { target: resolver })
        .emit(Instruction::nop());
}

pub(super) fn validate_list_name_consumer(source: &[u8]) -> Result<()> {
    // Stack caller identity, loop index, both native x advances and list call.
    for (offset, expected) in [
        (
            0x1fa0,
            Instruction::Sw {
                rt: Register::RA,
                base: Register::SP,
                offset: 0x34,
            },
        ),
        (
            0x1f9c,
            Instruction::Addu {
                rd: Register::S7,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x1ff8,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: NATIVE_ADVANCE,
            },
        ),
        (
            0x2114,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: NATIVE_ADVANCE,
            },
        ),
        (
            0x2140,
            Instruction::Addiu {
                rt: Register::S7,
                rs: Register::S7,
                immediate: 1,
            },
        ),
        (
            0x2c3c,
            Instruction::Jal {
                target: 0x800a_3f74,
            },
        ),
    ] {
        ensure!(
            decode_instruction(source, offset)? == expected,
            "KANRI registration-list name spacing consumer changed at {offset:#x}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute_with_callbacks;

    #[test]
    fn compact_list_advance_preserves_other_name_calls_and_resolver_inputs() {
        let origin = 0x800a_b800;
        let resolver = 0x800a_b4fc;
        let mut assembler = Assembler::new();
        emit_list_name_resolver(&mut assembler, resolver);
        let placed = assembler.assemble(origin).unwrap();
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            placed.instructions(),
            origin,
            "list resolver",
        )
        .unwrap();
        for caller in [LIST_RETURN_ADDRESS, 0x800a_47ac, 0, u32::MAX] {
            for count in 1..=4 {
                let mut x = 62u32;
                for index in 0..count {
                    let mut memory = vec![0x55; 4096];
                    memory[0x834..0x838].copy_from_slice(&caller.to_le_bytes());
                    let mut r = std::array::from_fn(|i| i as u32 * 13);
                    r[0] = 0;
                    r[20] = x;
                    r[23] = index;
                    r[29] = 0x800;
                    r[31] = 0x800a_3fd4;
                    let before = r;
                    let before_memory = memory.clone();
                    let mut calls = 0;
                    execute_with_callbacks(
                        placed.bytes(),
                        origin,
                        &mut r,
                        &mut memory,
                        None,
                        &mut |pc, _, _| {
                            if pc != resolver {
                                return false;
                            }
                            calls += 1;
                            true
                        },
                    );
                    assert_eq!(calls, 1);
                    let advance = if caller == LIST_RETURN_ADDRESS {
                        LIST_ADVANCE
                    } else {
                        NATIVE_ADVANCE
                    };
                    assert_eq!(r[20], 62 + index * advance as u32);
                    for reg in 0..32 {
                        if ![8, 9, 20].contains(&reg) {
                            assert_eq!(r[reg], before[reg], "register {reg}");
                        }
                    }
                    assert_eq!(memory, before_memory);
                    x = r[20] + NATIVE_ADVANCE as u32;
                }
            }
        }
    }
}
