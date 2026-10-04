//! Full 20x20 fixed glyphs, with exact 2-bit pixels and a 256-byte LZ history.
//! Tokens never cross a cell boundary, so each native call restores one cell.
//! The history lives in the existing mode-local payload reservation.
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address};
pub(super) const TABLE_OFFSET: usize = 7000;
pub(super) const HISTORY_OFFSET: usize = 7600;
const TABLE_ADDRESS: u32 = super::STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN + TABLE_OFFSET as u32;
pub(super) fn decode_table() -> Vec<u8> {
    (0..256u16)
        .flat_map(|byte| {
            let value = (0..4).fold(0u16, |out, i| {
                out | [0, 3, 14, 15][((byte >> (i * 2)) & 3) as usize] << (i * 4)
            });
            value.to_le_bytes()
        })
        .collect()
}
const HISTORY_ADDRESS: u32 = super::STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN + HISTORY_OFFSET as u32;

pub(super) fn pack_static_glyphs(payloads: &[&[u8]]) -> Result<Vec<u8>> {
    let mut source = Vec::new();
    for payload in payloads {
        ensure!(payload.len() == 200, "fixed glyph must be 20x20 4-bpp");
        for pair in payload.as_chunks::<2>().0 {
            let mut byte = 0;
            for (i, pixel) in [pair[0] & 15, pair[0] >> 4, pair[1] & 15, pair[1] >> 4]
                .into_iter()
                .enumerate()
            {
                byte |= match pixel {
                    0 => 0,
                    3 => 1,
                    14 => 2,
                    _ => anyhow::bail!("fixed glyph uses unsupported palette index {pixel}"),
                } << (i * 2);
            }
            source.push(byte);
        }
    }
    let mut output = Vec::new();
    let mut literal = Vec::new();
    let flush = |literal: &mut Vec<u8>, output: &mut Vec<u8>| {
        if !literal.is_empty() {
            output.push((literal.len() - 1) as u8);
            output.append(literal);
        }
    };
    let mut at = 0;
    while at < source.len() {
        let end = (at / 100 + 1) * 100;
        let mut best = (0, 0);
        for distance in 1..=at.min(255) {
            let mut length = 0;
            while length < (end - at).min(130)
                && source[at + length] == source[at + length - distance]
            {
                length += 1;
            }
            if length > best.0 {
                best = (length, distance);
            }
        }
        if best.0 >= 3 {
            flush(&mut literal, &mut output);
            output.extend_from_slice(&[128 + (best.0 - 3) as u8, best.1 as u8]);
            at += best.0;
        } else {
            literal.push(source[at]);
            at += 1;
            if literal.len() == 128 {
                flush(&mut literal, &mut output);
            }
        }
        if at == end {
            flush(&mut literal, &mut output);
        }
    }
    Ok(output)
}

/// A0 advances to the next cell's token. S registers survive; A1 receives
/// exactly 200 bytes. The history cursor may start anywhere: every backreference
/// is bounded by bytes already emitted in the current font stream.
pub(super) fn emit_exact_static_glyph_unpacker(a: &mut Assembler) {
    a.emit(Ori {
        rt: R::T0,
        rs: R::ZERO,
        immediate: 100,
    })
    .emit_all(load_address(R::T5, TABLE_ADDRESS))
    .emit_all(load_address(R::A2, HISTORY_ADDRESS))
    .emit(Lbu {
        rt: R::A3,
        base: R::A2,
        offset: 256,
    })
    .emit(Addu {
        rd: R::T8,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .label("fixed_group")
    .bne(R::T8, R::ZERO, "fixed_token")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Lbu {
        rt: R::V0,
        base: R::A0,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: 1,
    })
    .emit(Sltiu {
        rt: R::T9,
        rs: R::V0,
        immediate: 128,
    })
    .bne(R::T9, R::ZERO, "fixed_literal_header")
    .emit(Andi {
        rt: R::T8,
        rs: R::V0,
        immediate: 127,
    })
    .emit(Addiu {
        rt: R::T8,
        rs: R::T8,
        immediate: 3,
    })
    .emit(Lbu {
        rt: R::T9,
        base: R::A0,
        offset: 0,
    })
    .jump("fixed_token")
    .emit(Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: 1,
    })
    .label("fixed_literal_header")
    .emit(Addiu {
        rt: R::T8,
        rs: R::V0,
        immediate: 1,
    })
    .emit(Addu {
        rd: R::T9,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .label("fixed_token")
    .beq(R::T9, R::ZERO, "fixed_literal")
    .emit(Subu {
        rd: R::V1,
        rs: R::A3,
        rt: R::T9,
    })
    .emit(Andi {
        rt: R::V1,
        rs: R::V1,
        immediate: 255,
    })
    .emit(Addu {
        rd: R::V1,
        rs: R::V1,
        rt: R::A2,
    })
    .emit(Lbu {
        rt: R::T1,
        base: R::V1,
        offset: 0,
    })
    .jump("fixed_history")
    .emit(psx_r3000a::Instruction::nop())
    .label("fixed_literal")
    .emit(Lbu {
        rt: R::T1,
        base: R::A0,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: 1,
    })
    .label("fixed_history")
    .emit(Addu {
        rd: R::V1,
        rs: R::A2,
        rt: R::A3,
    })
    .emit(Sb {
        rt: R::T1,
        base: R::V1,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::A3,
        rs: R::A3,
        immediate: 1,
    })
    .emit(Andi {
        rt: R::A3,
        rs: R::A3,
        immediate: 255,
    })
    .emit(Addiu {
        rt: R::T8,
        rs: R::T8,
        immediate: -1,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T1,
        shift: 1,
    })
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::T5,
    })
    .emit(Lhu {
        rt: R::T3,
        base: R::T1,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: -1,
    })
    .emit(Sh {
        rt: R::T3,
        base: R::A1,
        offset: 0,
    })
    .bgtz(R::T0, "fixed_group")
    .emit(Addiu {
        rt: R::A1,
        rs: R::A1,
        immediate: 2,
    })
    .emit(Jr { rs: R::RA })
    .emit(Sb {
        rt: R::A3,
        base: R::A2,
        offset: 256,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute;

    #[test]
    fn runtime_restores_full_cells_and_advances_across_consecutive_glyphs() {
        let mut a = Assembler::new();
        emit_exact_static_glyph_unpacker(&mut a);
        let origin = 0x800a9000;
        let code = a.assemble(origin).unwrap();
        let cells: Vec<Vec<u8>> = (0..40)
            .map(|seed| {
                (0..200)
                    .map(|i| {
                        let pixel = |j: usize| {
                            [0, 3, 14][if seed == 0 {
                                0
                            } else {
                                (j * 7 + j / (seed + 1) + seed) % 3
                            }]
                        };
                        pixel(i * 2) | pixel(i * 2 + 1) << 4
                    })
                    .collect()
            })
            .collect();
        let mut memory = vec![0xa5; 0x200000];
        let encoded =
            pack_static_glyphs(&cells.iter().map(Vec::as_slice).collect::<Vec<_>>()).unwrap();
        memory[0x10000..0x10000 + encoded.len()].copy_from_slice(&encoded);
        let history = (HISTORY_ADDRESS & 0x1fffffff) as usize;
        memory[history..history + 257].fill(0);
        let table = (TABLE_ADDRESS & 0x1fffffff) as usize;
        memory[table..table + 512].copy_from_slice(&decode_table());
        let original = memory.clone();
        let mut source = 0x80010000;
        for cell in &cells {
            let mut r = std::array::from_fn(|i| i as u32 * 17);
            r[0] = 0;
            r[4] = source;
            r[5] = 0x80020000;
            r[29] = 0x801fe000;
            r[31] = 0x800f0000;
            let before = r;
            execute(code.bytes(), origin, &mut r, &mut memory);
            assert_eq!(&memory[0x20000..0x200c8], cell);
            assert_eq!(&memory[..0x20000], &original[..0x20000]);
            assert_eq!(&memory[0x200c8..history], &original[0x200c8..history]);
            assert_eq!(&memory[history + 257..], &original[history + 257..]);
            assert!(r[4] > source);
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], before[reg]);
            }
            source = r[4];
        }
        assert!(pack_static_glyphs(&[&[0xff; 200]]).is_err());
    }
}
