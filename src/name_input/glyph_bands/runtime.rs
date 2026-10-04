use super::*;
use psx_r3000a::{Assembler, Instruction::*, Register as R, verify_placed_program};

impl NameGlyphBandPack {
    /// A0 is the untagged Hangul offset, A1 the top-left packed 20x20 cell.
    /// Returns its address, or zero for an unsupported code without cell writes.
    /// Clears and writes only the ten pixel bytes per row; outline follows later.
    /// The reader takes a pack-relative byte offset in A0 and returns V0. It must
    /// preserve S0..S7 and SP. All other argument/scratch registers may change.
    pub fn build_fill_program(
        &self,
        origin: u32,
        reader: u32,
        row_bytes: usize,
    ) -> Result<Vec<u8>> {
        self.build_program(origin, reader, row_bytes, None)
    }

    /// Materializes exact fill and regenerates the shared eight-neighbor outline.
    pub fn build_render_program(
        &self,
        origin: u32,
        reader: u32,
        row_bytes: usize,
        outline: &crate::name_input::SharedNameOutlineRuntimeProgram,
    ) -> Result<Vec<u8>> {
        self.build_render_program_with_outline_addresses(
            origin,
            reader,
            row_bytes,
            outline.outline_pixel_address,
            outline.outline_cleanup_address,
        )
    }

    pub fn build_render_program_with_outline_addresses(
        &self,
        origin: u32,
        reader: u32,
        row_bytes: usize,
        outline_pixel_address: u32,
        outline_cleanup_address: u32,
    ) -> Result<Vec<u8>> {
        let [x, y, width, height] = self.crop;
        ensure!(
            x > 0 && y > 0 && x + width < 20 && y + height < 20,
            "band fill has no outline border"
        );
        self.build_program(
            origin,
            reader,
            row_bytes,
            Some((outline_pixel_address, outline_cleanup_address)),
        )
    }

    fn build_program(
        &self,
        origin: u32,
        reader: u32,
        row_bytes: usize,
        outline: Option<(u32, u32)>,
    ) -> Result<Vec<u8>> {
        ensure!(
            origin.is_multiple_of(4) && reader.is_multiple_of(4),
            "unaligned band program address"
        );
        ensure!(
            (10..=i16::MAX as usize / 20).contains(&row_bytes),
            "invalid band destination stride"
        );
        let mut a = Assembler::new();
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
        macro_rules! nop {
            () => {
                a.emit(psx_r3000a::Instruction::nop());
            };
        }
        macro_rules! read {
            () => {
                a.emit(Jal { target: reader });
                nop!();
            };
        }
        let saved = [
            R::S0,
            R::S1,
            R::S2,
            R::S3,
            R::S4,
            R::S5,
            R::S6,
            R::S7,
            R::RA,
        ];
        add!(R::SP, R::SP, -64);
        for (i, r) in saved.iter().enumerate() {
            a.emit(Sw {
                rt: *r,
                base: R::SP,
                offset: (16 + i * 4) as i16,
            });
        }
        mov!(R::S2, R::A0);
        mov!(R::S1, R::A1);
        a.emit(Sltiu {
            rt: R::T0,
            rs: R::S2,
            immediate: SYLLABLES as i16,
        })
        .beq(R::T0, R::ZERO, "band_unsupported");
        nop!();
        a.emit(Srl {
            rd: R::A0,
            rt: R::S2,
            shift: 3,
        });
        add!(R::A0, R::A0, self.membership);
        read!();
        a.emit(Andi {
            rt: R::T0,
            rs: R::S2,
            immediate: 7,
        })
        .emit(Srlv {
            rd: R::V0,
            rt: R::V0,
            rs: R::T0,
        })
        .emit(Andi {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .beq(R::V0, R::ZERO, "band_unsupported");
        nop!();
        // Prefixes may be unaligned or cross a scattered atlas-cell boundary.
        a.emit(Srl {
            rd: R::S3,
            rt: R::S2,
            shift: 5,
        })
        .emit(Sll {
            rd: R::S3,
            rt: R::S3,
            shift: 1,
        });
        add!(R::A0, R::S3, self.ranks);
        read!();
        mov!(R::S0, R::V0);
        add!(R::A0, R::S3, self.ranks + 1);
        read!();
        a.emit(Sll {
            rd: R::V0,
            rt: R::V0,
            shift: 8,
        })
        .emit(Or {
            rd: R::S0,
            rs: R::S0,
            rt: R::V0,
        });
        a.emit(Srl {
            rd: R::S3,
            rt: R::S2,
            shift: 5,
        })
        .emit(Sll {
            rd: R::S3,
            rt: R::S3,
            shift: 5,
        })
        .beq(R::S3, R::S2, "band_clear");
        nop!();
        a.label("band_rank").emit(Srl {
            rd: R::A0,
            rt: R::S3,
            shift: 3,
        });
        add!(R::A0, R::A0, self.membership);
        read!();
        a.emit(Andi {
            rt: R::T0,
            rs: R::S3,
            immediate: 7,
        })
        .emit(Srlv {
            rd: R::V0,
            rt: R::V0,
            rs: R::T0,
        })
        .emit(Andi {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .emit(Addu {
            rd: R::S0,
            rs: R::S0,
            rt: R::V0,
        });
        add!(R::S3, R::S3, 1);
        a.bne(R::S3, R::S2, "band_rank");
        nop!();
        a.label("band_clear");
        mov!(R::S3, R::S1);
        imm!(R::S4, 20);
        a.label("band_clear_row");
        for byte in 0..10 {
            a.emit(Sb {
                rt: R::ZERO,
                base: R::S3,
                offset: byte,
            });
        }
        add!(R::S3, R::S3, row_bytes);
        add!(R::S4, R::S4, -1);
        a.bne(R::S4, R::ZERO, "band_clear_row");
        nop!();
        let [x, y, width, _] = self.crop;
        for band in &self.bands {
            mov!(R::S4, R::ZERO);
            if band.bits > 0 {
                imm!(R::S6, band.indices);
                imm!(R::S7, band.bits);
                a.call("band_read_index");
                nop!();
            }
            imm!(R::T0, band.stride);
            a.emit(Multu {
                rs: R::S4,
                rt: R::T0,
            })
            .emit(Mflo { rd: R::S5 });
            add!(R::S5, R::S5, band.dictionary);
            add!(R::S2, R::S1, (y + band.start) * row_bytes + x / 2);
            mov!(R::S6, R::ZERO);
            imm!(R::S7, x);
            imm!(R::S3, (band.end - band.start) * width);
            a.call("band_paint_mask");
            nop!();
        }
        if let Some(outline) = outline {
            mov!(R::A0, R::S1);
            imm!(R::T8, row_bytes);
            a.emit(Jal { target: outline.1 });
            nop!();
        }
        mov!(R::V0, R::S1);
        a.jump("band_return");
        nop!();
        a.label("band_unsupported");
        mov!(R::V0, R::ZERO);
        a.label("band_return");
        for (i, r) in saved.iter().enumerate() {
            a.emit(Lw {
                rt: *r,
                base: R::SP,
                offset: (16 + i * 4) as i16,
            });
        }
        nop!();
        a.emit(Jr { rs: R::RA });
        add!(R::SP, R::SP, 64);

        // All bands use the same bit-index decoder. Keep only their parameters
        // at each call site; the reader preserves the saved-register arguments.
        a.label("band_read_index")
            .emit(Sw {
                rt: R::RA,
                base: R::SP,
                offset: 60,
            })
            .emit(Multu {
                rs: R::S0,
                rt: R::S7,
            })
            .emit(Mflo { rd: R::S2 });
        mov!(R::S3, R::ZERO);
        a.label("band_index_bit")
            .emit(Srl {
                rd: R::A0,
                rt: R::S2,
                shift: 3,
            })
            .emit(Addu {
                rd: R::A0,
                rs: R::A0,
                rt: R::S6,
            });
        read!();
        a.emit(Andi {
            rt: R::T0,
            rs: R::S2,
            immediate: 7,
        })
        .emit(Srlv {
            rd: R::V0,
            rt: R::V0,
            rs: R::T0,
        })
        .emit(Andi {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .emit(Sllv {
            rd: R::V0,
            rt: R::V0,
            rs: R::S3,
        })
        .emit(Or {
            rd: R::S4,
            rs: R::S4,
            rt: R::V0,
        });
        add!(R::S2, R::S2, 1);
        add!(R::S3, R::S3, 1);
        a.bne(R::S3, R::S7, "band_index_bit");
        nop!();
        a.emit(Lw {
            rt: R::RA,
            base: R::SP,
            offset: 60,
        });
        nop!();
        a.emit(Jr { rs: R::RA });
        nop!();

        // Reserve the o32 outgoing argument area separately from saved state.
        a.label("band_paint_mask").emit(Sw {
            rt: R::RA,
            base: R::SP,
            offset: 56,
        });
        let pixel = "band_pixel";
        let next = "band_next";
        let column = "band_column";
        let limit = "band_limit";
        a.label(pixel)
            .emit(Srl {
                rd: R::A0,
                rt: R::S6,
                shift: 3,
            })
            .emit(Addu {
                rd: R::A0,
                rs: R::A0,
                rt: R::S5,
            });
        read!();
        a.emit(Andi {
            rt: R::T0,
            rs: R::S6,
            immediate: 7,
        })
        .emit(Srlv {
            rd: R::V0,
            rt: R::V0,
            rs: R::T0,
        })
        .emit(Andi {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .beq(R::V0, R::ZERO, next);
        nop!();
        if let Some(outline) = outline {
            mov!(R::A0, R::S2);
            a.emit(Andi {
                rt: R::A1,
                rs: R::S7,
                immediate: 1,
            });
            imm!(R::T8, row_bytes);
            a.emit(Jal { target: outline.0 });
            mov!(R::A2, R::RA);
        }
        a.emit(Andi {
            rt: R::T0,
            rs: R::S7,
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
            rd: R::T2,
            rs: R::T2,
            rt: R::T1,
        })
        .emit(Sb {
            rt: R::T2,
            base: R::S2,
            offset: 0,
        });
        a.label(next);
        add!(R::S6, R::S6, 1);
        add!(R::S7, R::S7, 1);
        a.emit(Andi {
            rt: R::T0,
            rs: R::S7,
            immediate: 1,
        })
        .bne(R::T0, R::ZERO, column);
        nop!();
        add!(R::S2, R::S2, 1);
        a.label(column);
        imm!(R::T0, x + width);
        a.bne(R::S7, R::T0, limit);
        nop!();
        add!(R::S2, R::S2, row_bytes + x / 2 - (x + width) / 2);
        imm!(R::S7, x);
        a.label(limit)
            .emit(Sltu {
                rd: R::T0,
                rs: R::S6,
                rt: R::S3,
            })
            .bne(R::T0, R::ZERO, pixel);
        nop!();
        a.emit(Lw {
            rt: R::RA,
            base: R::SP,
            offset: 56,
        });
        nop!();
        a.emit(Jr { rs: R::RA });
        nop!();
        let placed = a.assemble(origin)?;
        ensure!(
            verify_placed_program(placed.bytes(), origin)? == placed.instructions(),
            "band program readback differs"
        );
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            placed.instructions(),
            origin,
            "band fill materializer",
        )?;
        Ok(placed.bytes().to_vec())
    }
}
