//! Preserve the chosen native-name sampling while storing only consumed columns.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

/// Precompute the horizontal half of the 24x8 -> 16x16 nearest sampler.
/// Vertical duplication stays in the runtime. Every stored palette index is
/// copied from the source; no quantization, glyph substitution or recoloring.
pub(crate) fn pack_battle_legacy_glyphs(source: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        source.len() == 16916
            && source[..8] == [16, 0, 0, 0, 0, 0, 0, 0]
            && u32::from_le_bytes(source[8..12].try_into()?) == 16908
            && source[16..20] == [6, 0, 128, 5],
        "native battle glyph TIM layout changed"
    );
    let mut out = source[..20].to_vec();
    out[8..12].copy_from_slice(&11276u32.to_le_bytes());
    out[16..18].copy_from_slice(&4u16.to_le_bytes());
    for glyph in 0..176 {
        for row in 0..8 {
            for column in 0..8 {
                let x = column * 3;
                let pixel =
                    |x: usize| (source[20 + glyph * 96 + row * 12 + x / 2] >> (4 * (x % 2))) & 15;
                out.push(pixel(x) | (pixel(x + 1) << 4));
            }
        }
    }
    ensure!(
        out.len() == super::BATTLE_NAME_LEGACY_STORAGE_BYTES,
        "legacy battle atlas extent changed"
    );
    Ok(out)
}

/// A0 is a native glyph index 0..176. Return V0 scratch or zero without writes.
/// The stored 16x8 rows are duplicated into [2,1,16,16] of a cleared 20x20 cell.
/// This is pixel-exact to sampling the original at (floor(3*x/2),floor(y/2)).
pub fn build_battle_legacy_glyph(
    origin: u32,
    capacity: usize,
    supplier: u32,
    scratch: u32,
) -> Result<Vec<u8>> {
    let ranges = [
        ram_range(origin, capacity)?,
        ram_range(supplier, super::BATTLE_NAME_LEGACY_STORAGE_BYTES)?,
        ram_range(scratch, 200)?,
    ];
    ensure!(
        origin.is_multiple_of(4) && scratch.is_multiple_of(4),
        "unaligned legacy battle glyph program"
    );
    for (i, r) in ranges.iter().enumerate() {
        ensure!(
            ranges[i + 1..].iter().all(|q| !overlaps(r, q)),
            "legacy battle glyph aliases code, native supplier or scratch"
        );
    }
    let mut a = Assembler::new();
    macro_rules! nop {
        () => {
            a.emit(psx_r3000a::Instruction::nop());
        };
    }
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A0,
        immediate: 176,
    })
    .beq(R::T0, R::ZERO, "reject");
    nop!();
    a.emit_all(load_address(R::V0, scratch))
        .emit(Addu {
            rd: R::T1,
            rs: R::V0,
            rt: R::ZERO,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 50,
        });
    a.label("clear")
        .emit(Sw {
            rt: R::ZERO,
            base: R::T1,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: -1,
        })
        .bne(R::T0, R::ZERO, "clear")
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: 4,
        });
    a.emit(Sll {
        rd: R::T0,
        rt: R::A0,
        shift: 6,
    })
    .emit_all(load_address(R::T8, supplier + 20))
    .emit(Addu {
        rd: R::T8,
        rs: R::T8,
        rt: R::T0,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .emit(Addiu {
        rt: R::T6,
        rs: R::V0,
        immediate: 11,
    });
    a.label("row")
        .emit(Srl {
            rd: R::T1,
            rt: R::T0,
            shift: 1,
        })
        .emit(Sll {
            rd: R::T1,
            rt: R::T1,
            shift: 3,
        })
        .emit(Addu {
            rd: R::T1,
            rs: R::T1,
            rt: R::T8,
        })
        .emit(Addiu {
            rt: R::T3,
            rs: R::ZERO,
            immediate: 8,
        });
    a.label("column")
        .emit(Lbu {
            rt: R::T5,
            base: R::T1,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: 1,
        })
        .emit(Sb {
            rt: R::T5,
            base: R::T6,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T3,
            rs: R::T3,
            immediate: -1,
        })
        .bne(R::T3, R::ZERO, "column")
        .emit(Addiu {
            rt: R::T6,
            rs: R::T6,
            immediate: 1,
        });
    a.emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: 1,
    })
    .emit(Sltiu {
        rt: R::T7,
        rs: R::T0,
        immediate: 16,
    })
    .bne(R::T7, R::ZERO, "row")
    .emit(Addiu {
        rt: R::T6,
        rs: R::T6,
        immediate: 2,
    })
    .emit(Jr { rs: R::RA });
    nop!();
    a.label("reject").emit(Jr { rs: R::RA }).emit(Addu {
        rd: R::V0,
        rs: R::ZERO,
        rt: R::ZERO,
    });
    let bytes = a.assemble(origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= capacity,
        "legacy battle glyph exceeds capacity"
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, origin)?,
        origin,
        "legacy battle glyph",
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_native_glyphs_match_original_sampling_and_invalid_indexes_do_not_write() {
        let origin = 0x800d8400;
        let source = 0x800d4000;
        let scratch = 0x800deb00;
        let code = build_battle_legacy_glyph(origin, 512, source, scratch).unwrap();
        let mut original = vec![0; 16916];
        original[..8].copy_from_slice(&[16, 0, 0, 0, 0, 0, 0, 0]);
        original[8..12].copy_from_slice(&16908u32.to_le_bytes());
        original[16..20].copy_from_slice(&[6, 0, 128, 5]);
        for (i, b) in original[20..].iter_mut().enumerate() {
            *b = (i.wrapping_mul(13) + i / 96 * 7) as u8;
        }
        let stored = pack_battle_legacy_glyphs(&original).unwrap();
        let mut m = vec![0x55; 0x200000];
        m[0xd4000..0xd4000 + stored.len()].copy_from_slice(&stored);
        for glyph in 0..176 {
            m[0xdeb00..0xdebc8].fill(0x55);
            let before = m.clone();
            let mut expected = vec![0u8; 200];
            for y in 0..16 {
                for x in 0..16 {
                    let sx = x * 3 / 2;
                    let b = original[20 + glyph * 96 + y / 2 * 12 + sx / 2];
                    let pixel = (b >> (4 * (sx % 2))) & 15;
                    let target = (y + 1) * 20 + x + 2;
                    expected[target / 2] |= pixel << (4 * (target % 2));
                }
            }
            let mut r = std::array::from_fn(|i| i as u32 * 37);
            r[0] = 0;
            r[4] = glyph as u32;
            r[29] = 0x801fe000;
            r[31] = 0x80027f24;
            let saved = r;
            crate::name_input::runtime_test_machine::execute(&code, origin, &mut r, &mut m);
            assert_eq!(r[2], scratch);
            assert_eq!(&m[0xdeb00..0xdebc8], expected);
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], saved[reg]);
            }
            let mut want = before;
            want[0xdeb00..0xdebc8].copy_from_slice(&expected);
            assert_eq!(m, want);
        }
        for invalid in [176, 0x339, 0x8118, u32::MAX] {
            let before = m.clone();
            let mut r = [0; 32];
            r[4] = invalid;
            r[31] = 0x80027f24;
            crate::name_input::runtime_test_machine::execute(&code, origin, &mut r, &mut m);
            assert_eq!(r[2], 0);
            assert_eq!(m, before);
        }
        assert!(pack_battle_legacy_glyphs(&original[..16915]).is_err());
        original[16] = 4;
        assert!(pack_battle_legacy_glyphs(&original).is_err());
    }
}
