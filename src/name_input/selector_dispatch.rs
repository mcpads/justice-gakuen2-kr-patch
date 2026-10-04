//! PLSEL1 name-loop adapter. Register/stack inputs are source-bound, not an ABI.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub const SELECTOR_NAME_ADVANCE: i16 = 16;
pub const SELECTOR_LEGACY_NAME_CONTINUATION: u32 = 0x800a4880;
pub const SELECTOR_NEXT_NAME_GLYPH: u32 = 0x800a49e4;

/// Source loop guarantees side/role in 0..2 and glyph index < length <= 4.
/// Legacy codes keep the original renderer. Tagged Hangul uses disjoint slots;
/// tagged ASCII uses its own supplier; unsupported families retain the source skip.
/// S0..S8 and SP are preserved. Native legacy spacing must use the same advance.
pub fn build_selector_name_dispatch(
    origin: u32,
    capacity: usize,
    materializer: u32,
    ascii_materializer: u32,
    uploader: u32,
    sprite: u32,
) -> Result<Vec<u8>> {
    let code = ram_range(origin, capacity)?;
    ensure!(origin.is_multiple_of(4), "unaligned selector dispatch");
    for target in [
        materializer,
        ascii_materializer,
        uploader,
        sprite,
        SELECTOR_LEGACY_NAME_CONTINUATION,
        SELECTOR_NEXT_NAME_GLYPH,
    ] {
        ensure!(
            target.is_multiple_of(4) && !overlaps(&code, &ram_range(target, 4)?),
            "selector dispatch overlaps a callee or continuation"
        );
    }
    let mut a = Assembler::new();
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::S4,
        immediate: 256,
    })
    .bne(R::T0, R::ZERO, "selector_legacy_name")
    .emit(Andi {
        rt: R::T0,
        rs: R::S4,
        immediate: 0xc000,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x8000,
    })
    .emit_all(load_address(R::A3, materializer))
    .beq(R::T0, R::T1, "selector_render_name")
    .emit(Andi {
        rt: R::T0,
        rs: R::S4,
        immediate: 0xff80,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x4000,
    })
    .bne(R::T0, R::T1, "selector_skip_name")
    .emit(psx_r3000a::Instruction::nop())
    .emit_all(load_address(R::A3, ascii_materializer))
    .label("selector_render_name")
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -32,
    })
    .emit(Sw {
        rt: R::RA,
        base: R::SP,
        offset: 28,
    })
    .emit(Lw {
        rt: R::T0,
        base: R::SP,
        offset: 0x40,
    }) // original side at +20
    .emit(Sll {
        rd: R::T1,
        rt: R::S7,
        shift: 2,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 3,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::S1,
    })
    .emit(Sw {
        rt: R::T0,
        base: R::SP,
        offset: 16,
    })
    .emit_all(load_address(R::T0, 0x801f608c))
    .emit(Lw {
        rt: R::T0,
        base: R::T0,
        offset: 0,
    })
    .emit(Lw {
        rt: R::T1,
        base: R::SP,
        offset: 0x90,
    }) // native name packet base
    .emit(Sll {
        rd: R::T2,
        rt: R::T0,
        shift: 3,
    })
    .emit(Subu {
        rd: R::T2,
        rs: R::T2,
        rt: R::T0,
    })
    .emit(Sll {
        rd: R::T2,
        rt: R::T2,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::T2,
    })
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::S5,
    })
    .emit(Sw {
        rt: R::T1,
        base: R::SP,
        offset: 20,
    })
    // Match the source's centered 48-pixel name anchor at sixteen-pixel advance.
    .emit(Sll {
        rd: R::T0,
        rt: R::S3,
        shift: 4,
    })
    .emit(Addiu {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 48,
    })
    .emit(Subu {
        rd: R::T0,
        rs: R::T1,
        rt: R::T0,
    })
    .emit(Sra {
        rd: R::T0,
        rt: R::T0,
        shift: 1,
    })
    .emit(Lw {
        rt: R::T1,
        base: R::SP,
        offset: 0x50,
    })
    .emit(Sll {
        rd: R::T2,
        rt: R::S1,
        shift: 4,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T2,
    })
    .emit(Andi {
        rt: R::T0,
        rs: R::T0,
        immediate: 0xffff,
    })
    .emit(Addiu {
        rt: R::T1,
        rs: R::FP,
        immediate: 73,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T1,
        shift: 16,
    })
    .emit(Or {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Sw {
        rt: R::T0,
        base: R::SP,
        offset: 24,
    })
    .emit(Jalr {
        rd: R::RA,
        rs: R::A3,
    })
    .emit(Andi {
        rt: R::A0,
        rs: R::S4,
        immediate: 0x3fff,
    })
    .beq(R::V0, R::ZERO, "selector_dispatch_return")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Lw {
        rt: R::A0,
        base: R::SP,
        offset: 16,
    })
    .emit(Jal { target: uploader })
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::T0,
        rs: R::V0,
        immediate: 1,
    })
    .beq(R::T0, R::ZERO, "selector_dispatch_return")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Lw {
        rt: R::A0,
        base: R::SP,
        offset: 16,
    })
    .emit(Lw {
        rt: R::A1,
        base: R::SP,
        offset: 20,
    })
    .emit(Lw {
        rt: R::A2,
        base: R::SP,
        offset: 24,
    })
    .emit_all(load_address(R::A3, 0x801f6090))
    .emit(Lw {
        rt: R::A3,
        base: R::A3,
        offset: 0,
    })
    .emit(Jal { target: sprite })
    .emit(Addiu {
        rt: R::A3,
        rs: R::A3,
        immediate: 0x1068,
    })
    .label("selector_dispatch_return")
    .emit(Lw {
        rt: R::RA,
        base: R::SP,
        offset: 28,
    })
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 32,
    })
    .label("selector_skip_name")
    .emit(J {
        target: SELECTOR_NEXT_NAME_GLYPH,
    })
    .emit(psx_r3000a::Instruction::nop())
    .label("selector_legacy_name")
    .emit(J {
        target: SELECTOR_LEGACY_NAME_CONTINUATION,
    })
    .emit(Addu {
        rd: R::A1,
        rs: R::ZERO,
        rt: R::ZERO,
    });
    let bytes = a.assemble(origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= capacity,
        "selector dispatch exceeds capacity"
    );
    let instructions = verify_placed_program(&bytes, origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        origin,
        "selector name dispatch",
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute_with_callbacks;
    #[test]
    fn dispatch_separates_four_names_and_preserves_source_loop_state() {
        let origin = 0x800a8700;
        let materializer = 0x800a8100;
        let ascii_materializer = 0x800a8500;
        let uploader = 0x800a8300;
        let sprite = 0x800a8400;
        let bytes = build_selector_name_dispatch(
            origin,
            512,
            materializer,
            ascii_materializer,
            uploader,
            sprite,
        )
        .unwrap();
        let off = |a: u32| (a & 0x1fffffff) as usize;
        for side in 0u32..2 {
            for role in 0u32..2 {
                for length in 1u32..=4 {
                    for index in 0..length {
                        for mode in 0..8 {
                            for frame in 0u32..2 {
                                let code = match mode {
                                    0 => 0x41,
                                    1 => 0x4041,
                                    5 => 0x4100,
                                    6 => 0x405f,
                                    7 => 0x403a,
                                    _ => 0x8000,
                                };
                                let mut r = std::array::from_fn(|i| i as u32 * 37);
                                r[0] = 0;
                                r[17] = index;
                                r[19] = length;
                                r[20] = code;
                                r[21] = index * 56;
                                r[23] = role;
                                r[29] = 0x801fe000;
                                r[30] = 20;
                                r[31] = 0x800f0000;
                                let saved = r;
                                let mut memory = vec![0x55; 0x200000];
                                for (address, value) in [
                                    (r[29] + 0x20, side),
                                    (r[29] + 0x30, 78),
                                    (r[29] + 0x70, 0x801c9000),
                                    (0x801f608c, frame),
                                    (0x801f6090, 0x801f6800),
                                ] {
                                    let at = off(address);
                                    memory[at..at + 4].copy_from_slice(&value.to_le_bytes());
                                }
                                let before = memory.clone();
                                let mut calls = Vec::new();
                                let mut continuation = None;
                                execute_with_callbacks(
                                    &bytes,
                                    origin,
                                    &mut r,
                                    &mut memory,
                                    None,
                                    &mut |pc, r, m| {
                                        if [
                                            SELECTOR_LEGACY_NAME_CONTINUATION,
                                            SELECTOR_NEXT_NAME_GLYPH,
                                        ]
                                        .contains(&pc)
                                        {
                                            continuation = Some(pc);
                                            return true;
                                        }
                                        if ![materializer, ascii_materializer, uploader, sprite]
                                            .contains(&pc)
                                        {
                                            return false;
                                        }
                                        calls.push(pc);
                                        let slot = side * 8 + role * 4 + index;
                                        if pc == materializer {
                                            assert_eq!(r[4], 0);
                                        } else if pc == ascii_materializer {
                                            assert_eq!(r[4], code & 0x7f);
                                        } else if pc == uploader {
                                            assert_eq!(r[4], slot);
                                        } else {
                                            assert_eq!(r[4], slot);
                                            assert_eq!(r[5], 0x801c9000 + index * 56 + frame * 28);
                                            let x = 78
                                                + ((48 - length as i32 * 16) >> 1)
                                                + index as i32 * 16;
                                            assert_eq!(r[6], (93 << 16) | (x as u32 & 65535));
                                            assert_eq!(r[7], 0x801f7868);
                                        }
                                        for reg in
                                            [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25]
                                        {
                                            r[reg] = 0xdead0000 + reg as u32;
                                        }
                                        let sp = off(r[29]);
                                        m[sp..sp + 16].fill(0xcc);
                                        r[2] = if pc == materializer || pc == ascii_materializer {
                                            if mode == 3 || mode == 6 {
                                                0
                                            } else {
                                                0x800b4a38
                                            }
                                        } else if pc == uploader && (mode == 4 || mode == 7) {
                                            u32::MAX
                                        } else {
                                            0
                                        };
                                        true
                                    },
                                );
                                assert_eq!(
                                    continuation,
                                    Some(if mode == 0 {
                                        SELECTOR_LEGACY_NAME_CONTINUATION
                                    } else {
                                        SELECTOR_NEXT_NAME_GLYPH
                                    })
                                );
                                assert_eq!(
                                    calls,
                                    match mode {
                                        0 | 5 => vec![],
                                        1 => vec![ascii_materializer, uploader, sprite],
                                        6 => vec![ascii_materializer],
                                        7 => vec![ascii_materializer, uploader],
                                        2 => vec![materializer, uploader, sprite],
                                        3 => vec![materializer],
                                        _ => vec![materializer, uploader],
                                    }
                                );
                                if mode == 0 {
                                    assert_eq!(r[5], 0);
                                }
                                for reg in (16..24).chain([28, 29, 30, 31]) {
                                    assert_eq!(r[reg], saved[reg]);
                                }
                                let mut expected = before;
                                if mode != 0 && mode != 5 {
                                    let sp = off(saved[29]);
                                    expected[sp - 32..sp].copy_from_slice(&memory[sp - 32..sp]);
                                }
                                assert_eq!(memory, expected);
                            }
                        }
                    }
                }
            }
        }
    }
}
