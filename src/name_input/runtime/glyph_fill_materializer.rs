use super::super::NameInputRuntimePackLayout;
use anyhow::Result;
use psx_r3000a::{Assembler, Instruction, Register};

const FILL_PALETTE_INDEX: u16 = 13;
const FAILURE: u16 = u16::MAX;

pub(crate) fn emit_glyph_fill_materializer(
    assembler: &mut Assembler,
    pack: &NameInputRuntimePackLayout,
    coordinate_list_offset: u16,
    cache_cell_row_bytes: i16,
    outline_pixel_address: u32,
    outline_cleanup_address: u32,
    nickname_hud_store_address: Option<u32>,
) -> Result<()> {
    let crop_x = i16::try_from(pack.crop[0])?;
    let crop_y = i16::try_from(pack.crop[1])?;
    let coordinate_count = i16::try_from(pack.runtime_coordinate_list_byte_count)?;
    assembler
        .label("materialize_name_glyph_fill")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -16,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Sw {
            rt: Register::A1,
            base: Register::SP,
            offset: 8,
        })
        .call("resolve_name_glyph_components")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::V0, Register::T0, "glyph_fill_failed")
        .emit(Instruction::Addu {
            rd: Register::T4,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::T5,
            rs: Register::V1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Lw {
            rt: Register::A0,
            base: Register::SP,
            offset: 8,
        })
        .call("clear_name_glyph_cache_cell")
        .emit(Instruction::nop())
        .beq(Register::V0, Register::ZERO, "glyph_fill_failed")
        .emit(Instruction::Addu {
            rd: Register::T6,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::T7,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("glyph_fill_coordinate")
        .emit(Instruction::Srl {
            rd: Register::T9,
            rt: Register::T7,
            shift: 3,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T4,
            rt: Register::T9,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T7,
            immediate: 7,
        })
        .emit(Instruction::Ori {
            rt: Register::T8,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Sllv {
            rd: Register::T8,
            rt: Register::T8,
            rs: Register::T0,
        })
        .emit(Instruction::And {
            rd: Register::T0,
            rs: Register::V0,
            rt: Register::T8,
        })
        .bne(Register::T0, Register::ZERO, "write_name_glyph_fill")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::T5, Register::T0, "next_glyph_fill_coordinate")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T5,
            rt: Register::T9,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::And {
            rd: Register::T0,
            rs: Register::V0,
            rt: Register::T8,
        })
        .beq(Register::T0, Register::ZERO, "next_glyph_fill_coordinate")
        .emit(Instruction::nop())
        .label("write_name_glyph_fill")
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::T7,
            immediate: i16::from_ne_bytes(coordinate_list_offset.to_ne_bytes()),
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::V0,
            shift: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: crop_y,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: cache_cell_row_bytes as u16,
        })
        .emit(Instruction::Multu {
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Mflo { rd: Register::T0 })
        .emit(Instruction::Andi {
            rt: Register::T1,
            rs: Register::V0,
            immediate: 0x0f,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: crop_x,
        })
        .emit(Instruction::Srl {
            rd: Register::T2,
            rt: Register::T1,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T2,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T6,
        })
        .emit(Instruction::Addu {
            rd: Register::A3,
            rs: Register::T0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Andi {
            rt: Register::T9,
            rs: Register::T1,
            immediate: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::A3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::T9,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::T8,
            rs: Register::ZERO,
            immediate: cache_cell_row_bytes as u16,
        })
        .emit(Instruction::Jal {
            target: outline_pixel_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::RA,
            rt: Register::ZERO,
        })
        .emit(Instruction::Lbu {
            rt: Register::T2,
            base: Register::A3,
            offset: 0,
        })
        .emit(Instruction::Sll {
            rd: Register::T9,
            rt: Register::T9,
            shift: 2,
        })
        .emit(Instruction::Ori {
            rt: Register::T3,
            rs: Register::ZERO,
            immediate: FILL_PALETTE_INDEX,
        })
        .emit(Instruction::Sllv {
            rd: Register::T3,
            rt: Register::T3,
            rs: Register::T9,
        })
        .emit(Instruction::Or {
            rd: Register::T2,
            rs: Register::T2,
            rt: Register::T3,
        })
        .emit(Instruction::Sb {
            rt: Register::T2,
            base: Register::A3,
            offset: 0,
        })
        .label("next_glyph_fill_coordinate")
        .emit(Instruction::Addiu {
            rt: Register::T7,
            rs: Register::T7,
            immediate: 1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T7,
            immediate: coordinate_count,
        })
        .bne(Register::T0, Register::ZERO, "glyph_fill_coordinate")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T6,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::T8,
            rs: Register::ZERO,
            immediate: cache_cell_row_bytes as u16,
        })
        .emit(Instruction::Jal {
            target: outline_cleanup_address,
        })
        .emit(Instruction::nop());
    if let Some(nickname_hud_store_address) = nickname_hud_store_address {
        assembler
            .emit(Instruction::Lw {
                rt: Register::A0,
                base: Register::SP,
                offset: 8,
            })
            .emit(Instruction::Addu {
                rd: Register::A1,
                rs: Register::T6,
                rt: Register::ZERO,
            })
            .emit(Instruction::Jal {
                target: nickname_hud_store_address,
            })
            .emit(Instruction::Ori {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: cache_cell_row_bytes as u16,
            });
    } else {
        assembler.emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::A0,
            immediate: -(cache_cell_row_bytes * 20),
        });
    }
    assembler
        .jump("glyph_fill_complete")
        .emit(Instruction::nop())
        .label("glyph_fill_failed")
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("glyph_fill_complete")
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 16,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    Ok(())
}
