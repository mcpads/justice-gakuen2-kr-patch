//! EDIT-only adapter for the native battle name packet, before double-buffer copy.
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub const BATTLE_NAME_SPRITE_CONTINUATION: u32 = 0x8005_d2f0;

/// Source context: S3 points at the packet, S6 is side 0/1. Native partner state
/// is boolean-by-nonzero. Builtin fighters must bypass this helper. Allocation:
/// side 0 occupies the old EDIT area; side 1 requires the separately admitted
/// (928,496,32,16) word rectangle. Blank pixels alone do not admit that area.
pub fn build_battle_name_sprite(origin: u32, capacity: usize) -> Result<Vec<u8>> {
    super::selector_loader::ram_range(origin, capacity)?;
    ensure!(origin.is_multiple_of(4), "unaligned battle sprite helper");
    let mut a = Assembler::new();
    a.emit_all(load_address(R::V1, 0x8009_d2b0))
        .emit(Addu {
            rd: R::V1,
            rs: R::V1,
            rt: R::S6,
        })
        .emit(Lbu {
            rt: R::V0,
            base: R::V1,
            offset: 0,
        })
        .emit(Lw {
            rt: R::V1,
            base: R::S3,
            offset: 4,
        })
        .emit(Sltiu {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .emit(Xori {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .emit(Srl {
            rd: R::V1,
            rt: R::V1,
            shift: 5,
        })
        .emit(Sll {
            rd: R::V1,
            rt: R::V1,
            shift: 5,
        })
        .bne(R::S6, R::ZERO, "right")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Ori {
            rt: R::V1,
            rs: R::V1,
            immediate: 0x1c,
        })
        .emit(Sw {
            rt: R::V1,
            base: R::S3,
            offset: 4,
        })
        .emit(Sll {
            rd: R::V0,
            rt: R::V0,
            shift: 4,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::V0,
            immediate: 64,
        })
        .emit(Sb {
            rt: R::ZERO,
            base: R::S3,
            offset: 20,
        })
        .emit(Sb {
            rt: R::V0,
            base: R::S3,
            offset: 21,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 8,
        })
        .beq(R::ZERO, R::ZERO, "dimensions")
        .emit(psx_r3000a::Instruction::nop())
        .label("right")
        .emit(Ori {
            rt: R::V1,
            rs: R::V1,
            immediate: 0x1e,
        })
        .emit(Sw {
            rt: R::V1,
            base: R::S3,
            offset: 4,
        })
        .emit(Sll {
            rd: R::V0,
            rt: R::V0,
            shift: 6,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::V0,
            immediate: 128,
        })
        .emit(Sb {
            rt: R::V0,
            base: R::S3,
            offset: 20,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 240,
        })
        .emit(Sb {
            rt: R::V0,
            base: R::S3,
            offset: 21,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 438,
        })
        .label("dimensions")
        .emit(Sh {
            rt: R::V0,
            base: R::S3,
            offset: 16,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 64,
        })
        .emit(Sh {
            rt: R::V0,
            base: R::S3,
            offset: 24,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 16,
        })
        .emit(Sh {
            rt: R::V0,
            base: R::S3,
            offset: 26,
        })
        .emit(J {
            target: BATTLE_NAME_SPRITE_CONTINUATION,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 30,
        });
    let bytes = a.assemble(origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= capacity,
        "battle sprite helper exceeds capacity"
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, origin)?,
        origin,
        "battle name sprite",
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_use_distinct_full_size_slots_and_keep_packet_links_and_colors() {
        let origin = 0x80029180;
        let code = build_battle_name_sprite(origin, 0xec).unwrap();
        for side in 0..2 {
            for partner in [0, 1, 7] {
                let mut m = vec![0x55; 0x200000];
                let packet = 0x1c7730;
                m[0x9d2b0 + side] = partner;
                m[packet + 4..packet + 8].copy_from_slice(&0xe100021cu32.to_le_bytes());
                let mut expected = m.clone();
                let (page, u, v, x) = if side == 0 {
                    (0x1c, 0, 64 + 16 * u8::from(partner != 0), 8u16)
                } else {
                    (0x1e, 128 + 64 * u8::from(partner != 0), 240, 438u16)
                };
                expected[packet + 4] = (expected[packet + 4] & 0xe0) | page;
                expected[packet + 20] = u;
                expected[packet + 21] = v;
                for (o, value) in [(16, x), (24, 64), (26, 16)] {
                    expected[packet + o..packet + o + 2].copy_from_slice(&value.to_le_bytes());
                }
                let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
                r[0] = 0;
                r[19] = 0x801c7730;
                r[22] = side as u32;
                r[31] = BATTLE_NAME_SPRITE_CONTINUATION;
                let saved = r;
                crate::name_input::runtime_test_machine::execute(&code, origin, &mut r, &mut m);
                assert_eq!(m, expected);
                for reg in 0..32 {
                    if ![2, 3, 8].contains(&reg) {
                        assert_eq!(r[reg], saved[reg]);
                    }
                }
                assert_eq!(r[8], 30);
            }
        }
    }
}
