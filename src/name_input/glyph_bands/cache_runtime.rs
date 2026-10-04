use super::*;
use crate::name_input::SharedNameOutlineRuntimeProgram;
use psx_r3000a::{Assembler, Instruction::*, Register as R, verify_placed_program};

impl NameGlyphBandPack {
    /// Adapts the existing A0=syllable/A1=cache-slot interface. The cell resolver
    /// must return the slot's address (or zero) without changing its pixels.
    /// Success returns the glyph pointer without a HUD store, or the store's
    /// result when present; unsupported glyphs or slots return zero.
    pub fn build_cache_program(
        &self,
        origin: u32,
        reader: u32,
        cell_resolver: u32,
        row_bytes: usize,
        outline: &SharedNameOutlineRuntimeProgram,
        nickname_store: Option<u32>,
    ) -> Result<Vec<u8>> {
        let mut a = Assembler::new();
        a.emit(Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: -32,
        })
        .emit(Sw {
            rt: R::RA,
            base: R::SP,
            offset: 16,
        })
        .emit(Sw {
            rt: R::A0,
            base: R::SP,
            offset: 20,
        })
        .emit(Sw {
            rt: R::A1,
            base: R::SP,
            offset: 24,
        })
        .emit(Jal {
            target: cell_resolver,
        })
        .emit(Addu {
            rd: R::A0,
            rs: R::A1,
            rt: R::ZERO,
        })
        .beq(R::V0, R::ZERO, "cache_return")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Lw {
            rt: R::A0,
            base: R::SP,
            offset: 20,
        })
        .emit(Addu {
            rd: R::A1,
            rs: R::V0,
            rt: R::ZERO,
        })
        .call("render_bands")
        .emit(psx_r3000a::Instruction::nop());
        if let Some(store) = nickname_store {
            a.beq(R::V0, R::ZERO, "cache_return")
                .emit(psx_r3000a::Instruction::nop())
                .emit(Lw {
                    rt: R::A0,
                    base: R::SP,
                    offset: 24,
                })
                .emit(Addu {
                    rd: R::A1,
                    rs: R::V0,
                    rt: R::ZERO,
                })
                .emit(Jal { target: store })
                .emit(Ori {
                    rt: R::A2,
                    rs: R::ZERO,
                    immediate: u16::try_from(row_bytes)?,
                });
        }
        a.label("cache_return")
            .emit(Lw {
                rt: R::RA,
                base: R::SP,
                offset: 16,
            })
            .emit(Addiu {
                rt: R::SP,
                rs: R::SP,
                immediate: 32,
            })
            .emit(Jr { rs: R::RA })
            .emit(psx_r3000a::Instruction::nop())
            .label("render_bands");
        let prefix = a.assemble(origin)?;
        let render_origin = origin
            .checked_add(u32::try_from(prefix.bytes().len())?)
            .ok_or_else(|| anyhow::anyhow!("band cache program address overflow"))?;
        let renderer = self.build_render_program(render_origin, reader, row_bytes, outline)?;
        a.emit_all(verify_placed_program(&renderer, render_origin)?);
        let placed = a.assemble(origin)?;
        ensure!(
            verify_placed_program(placed.bytes(), origin)? == placed.instructions(),
            "band cache program readback differs"
        );
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            placed.instructions(),
            origin,
            "band cache adapter",
        )?;
        Ok(placed.bytes().to_vec())
    }
}
