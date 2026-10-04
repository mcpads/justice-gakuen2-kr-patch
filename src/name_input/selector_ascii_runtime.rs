//! Expand selector-owned ASCII masks into the shared 20x20 scratch format.
use super::selector_loader::{overlaps, ram_range};
use super::{SelectorAsciiGlyphs, SharedNameOutlineRuntimeProgram};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

impl SelectorAsciiGlyphs {
    /// A0 is an untagged ASCII scalar. Unsupported input returns zero without
    /// writes; supported input returns scratch, preserving callee-saved registers.
    pub fn build_render_program(
        &self,
        origin: u32,
        capacity: usize,
        data: u32,
        scratch: u32,
        outline: &SharedNameOutlineRuntimeProgram,
    ) -> Result<Vec<u8>> {
        let [x, y, w, h] = self.crop;
        ensure!(
            x > 0
                && y > 0
                && w > 0
                && h > 0
                && x < 20
                && y < 20
                && w < 20
                && h < 20
                && x + w < 20
                && y + h < 20
                && self.mask_stride == (w * h).div_ceil(8)
                && self.glyph_count > 0
                && self.glyph_count < 255
                && self.bytes.len() == 128 + self.glyph_count * self.mask_stride
                && self.bytes[..128]
                    .iter()
                    .all(|&v| v == 255 || usize::from(v) < self.glyph_count),
            "invalid selector ASCII mask layout"
        );
        ensure!(
            origin.is_multiple_of(4) && scratch.is_multiple_of(4),
            "unaligned ASCII code or scratch"
        );
        let ranges = [
            ram_range(origin, capacity)?,
            ram_range(data, self.bytes.len())?,
            ram_range(scratch, 200)?,
            ram_range(outline.outline_pixel_address, outline.bytes.len())?,
        ];
        for (i, r) in ranges.iter().enumerate() {
            ensure!(
                ranges[i + 1..].iter().all(|q| !overlaps(r, q)),
                "ASCII renderer aliases code, data, scratch or outline"
            );
        }
        let mut a = Assembler::new();
        macro_rules! nop {
            () => {
                a.emit(psx_r3000a::Instruction::nop());
            };
        }
        macro_rules! mov {
            ($d:expr,$s:expr) => {
                a.emit(Addu {
                    rd: $d,
                    rs: $s,
                    rt: R::ZERO,
                });
            };
        }
        macro_rules! imm {
            ($d:expr,$v:expr) => {
                a.emit(Ori {
                    rt: $d,
                    rs: R::ZERO,
                    immediate: u16::try_from($v)?,
                });
            };
        }
        macro_rules! add {
            ($d:expr,$s:expr,$v:expr) => {
                a.emit(Addiu {
                    rt: $d,
                    rs: $s,
                    immediate: i16::try_from($v)?,
                });
            };
        }
        a.emit(Sltiu {
            rt: R::T0,
            rs: R::A0,
            immediate: 128,
        })
        .beq(R::T0, R::ZERO, "unsupported");
        nop!();
        a.emit_all(load_address(R::T0, data))
            .emit(Addu {
                rd: R::T0,
                rs: R::T0,
                rt: R::A0,
            })
            .emit(Lbu {
                rt: R::T0,
                base: R::T0,
                offset: 0,
            });
        nop!();
        a.emit(Sltiu {
            rt: R::T1,
            rs: R::T0,
            immediate: i16::try_from(self.glyph_count)?,
        })
        .beq(R::T1, R::ZERO, "unsupported");
        nop!();
        add!(R::SP, R::SP, -48);
        let saved = [R::S0, R::S1, R::S2, R::S3, R::S4, R::S5, R::S6, R::RA];
        for (i, r) in saved.iter().enumerate() {
            a.emit(Sw {
                rt: *r,
                base: R::SP,
                offset: (16 + i * 4) as i16,
            });
        }
        imm!(R::T1, self.mask_stride);
        a.emit(Multu {
            rs: R::T0,
            rt: R::T1,
        })
        .emit(Mflo { rd: R::S0 });
        a.emit_all(load_address(R::T1, data + 128)).emit(Addu {
            rd: R::S0,
            rs: R::S0,
            rt: R::T1,
        });
        a.emit_all(load_address(R::S1, scratch));
        mov!(R::T0, R::S1);
        imm!(R::T1, 50);
        a.label("clear").emit(Sw {
            rt: R::ZERO,
            base: R::T0,
            offset: 0,
        });
        add!(R::T1, R::T1, -1);
        a.bne(R::T1, R::ZERO, "clear");
        add!(R::T0, R::T0, 4);
        mov!(R::S4, R::ZERO);
        mov!(R::S6, R::ZERO);
        a.label("row");
        mov!(R::S5, R::ZERO);
        a.label("pixel")
            .emit(Srl {
                rd: R::T0,
                rt: R::S4,
                shift: 3,
            })
            .emit(Addu {
                rd: R::T0,
                rs: R::T0,
                rt: R::S0,
            })
            .emit(Lbu {
                rt: R::T0,
                base: R::T0,
                offset: 0,
            })
            .emit(Andi {
                rt: R::T1,
                rs: R::S4,
                immediate: 7,
            })
            .emit(Srlv {
                rd: R::T0,
                rt: R::T0,
                rs: R::T1,
            })
            .emit(Andi {
                rt: R::T0,
                rs: R::T0,
                immediate: 1,
            })
            .beq(R::T0, R::ZERO, "next");
        nop!();
        add!(R::T0, R::S6, y);
        a.emit(Sll {
            rd: R::T1,
            rt: R::T0,
            shift: 2,
        })
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::T1,
        })
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 1,
        });
        add!(R::S3, R::S5, x);
        a.emit(Srl {
            rd: R::T1,
            rt: R::S3,
            shift: 1,
        })
        .emit(Addu {
            rd: R::S2,
            rs: R::T0,
            rt: R::T1,
        })
        .emit(Addu {
            rd: R::S2,
            rs: R::S2,
            rt: R::S1,
        });
        mov!(R::A0, R::S2);
        a.emit(Andi {
            rt: R::A1,
            rs: R::S3,
            immediate: 1,
        });
        imm!(R::T8, 10);
        a.emit(Jal {
            target: outline.outline_pixel_address,
        });
        mov!(R::A2, R::RA);
        a.emit(Andi {
            rt: R::T0,
            rs: R::S3,
            immediate: 1,
        })
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 2,
        });
        imm!(R::T1, 13);
        a.emit(Sllv {
            rd: R::T1,
            rt: R::T1,
            rs: R::T0,
        })
        .emit(Lbu {
            rt: R::T2,
            base: R::S2,
            offset: 0,
        });
        nop!();
        a.emit(Or {
            rd: R::T1,
            rs: R::T1,
            rt: R::T2,
        })
        .emit(Sb {
            rt: R::T1,
            base: R::S2,
            offset: 0,
        });
        a.label("next");
        add!(R::S4, R::S4, 1);
        add!(R::S5, R::S5, 1);
        imm!(R::T0, w);
        a.bne(R::S5, R::T0, "pixel");
        nop!();
        add!(R::S6, R::S6, 1);
        imm!(R::T0, h);
        a.bne(R::S6, R::T0, "row");
        nop!();
        mov!(R::A0, R::S1);
        imm!(R::T8, 10);
        a.emit(Jal {
            target: outline.outline_cleanup_address,
        });
        nop!();
        mov!(R::V0, R::S1);
        for (i, r) in saved.iter().enumerate() {
            a.emit(Lw {
                rt: *r,
                base: R::SP,
                offset: (16 + i * 4) as i16,
            });
        }
        nop!();
        a.emit(Jr { rs: R::RA });
        add!(R::SP, R::SP, 48);
        a.label("unsupported").emit(Jr { rs: R::RA });
        mov!(R::V0, R::ZERO);
        let bytes = a.assemble(origin)?.bytes().to_vec();
        ensure!(
            bytes.len() <= capacity,
            "ASCII renderer exceeds code capacity"
        );
        let instructions = verify_placed_program(&bytes, origin)?;
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            &instructions,
            origin,
            "selector ASCII renderer",
        )?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        font::rasterize_menu_glyphs,
        name_input::{
            DIGIT_KEYS, LATIN_KEYS, SYMBOL_KEYS, build_shared_name_outline_runtime_program,
            runtime_test_machine::execute,
        },
    };
    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn emitted_ascii_matches_every_full_glyph_and_preserves_other_memory() {
        let font =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
        let reference = rasterize_menu_glyphs(
            &font,
            &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
            12.0,
            3,
            13,
        )
        .unwrap();
        let pack = SelectorAsciiGlyphs::build(&reference, 1860).unwrap();
        let outline = build_shared_name_outline_runtime_program().unwrap();
        let origin = 0x80010000;
        let entry = 0x800a8800;
        let data = 0x800b422c;
        let scratch = 0x800b4a38;
        let renderer = pack
            .build_render_program(entry, 992, data, scratch, &outline)
            .unwrap();
        let mut code = vec![0; (entry - origin) as usize + renderer.len()];
        let mut jump = Assembler::new();
        jump.emit(J { target: entry })
            .emit(psx_r3000a::Instruction::nop());
        code[..8].copy_from_slice(jump.assemble(origin).unwrap().bytes());
        let at = (outline.outline_pixel_address - origin) as usize;
        code[at..at + outline.bytes.len()].copy_from_slice(&outline.bytes);
        code[(entry - origin) as usize..].copy_from_slice(&renderer);
        let off = |v: u32| (v & 0x1fffffff) as usize;
        let mut memory = vec![0x55; 0x200000];
        memory[off(data)..off(data) + pack.bytes.len()].copy_from_slice(&pack.bytes);
        for scalar in (0..128).chain([128, 0x4000, 0x4041, 0xffffffff]) {
            let glyph = reference
                .glyphs
                .iter()
                .find(|g| g.character as u32 == scalar);
            let supported = glyph.is_some() || scalar == 32;
            let before = memory.clone();
            let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
            r[0] = 0;
            r[4] = scalar;
            r[29] = 0x801fe000;
            r[31] = 0x800f0000;
            let saved = r;
            execute(&code, origin, &mut r, &mut memory);
            assert_eq!(r[2], if supported { scratch } else { 0 }, "code {scalar}");
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], saved[reg], "code {scalar} reg {reg}");
            }
            let mut expected = before;
            if supported {
                let pixels = if let Some(g) = glyph {
                    g.pixels.clone()
                } else {
                    vec![0; 400]
                };
                let packed: Vec<_> = pixels
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| p[0] | p[1] << 4)
                    .collect();
                expected[off(scratch)..off(scratch) + 200].copy_from_slice(&packed);
                expected[off(saved[29]) - 48..off(saved[29])]
                    .copy_from_slice(&memory[off(saved[29]) - 48..off(saved[29])]);
            }
            assert_eq!(memory, expected, "full pixel/memory mismatch for {scalar}");
        }
        assert!(
            pack.build_render_program(entry, 4, data, scratch, &outline)
                .is_err()
        );
        assert!(
            pack.build_render_program(entry, 992, scratch, scratch, &outline)
                .is_err()
        );
    }
}
