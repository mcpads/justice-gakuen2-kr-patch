use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::redisplay_runtime::NAME_GLYPH_CACHE_TABLE_ADDRESS;

const NAME_FONT_PIXEL_ADDRESS: u32 = 0x800e_92e0;
const CACHE_SLOT_COUNT: i16 = 16;
const CACHE_CELL_ROW_BYTES: i16 = 10;
const CACHE_CELL_ROW_STRIDE: i16 = 384;
const CACHE_CELL_HEIGHT: i16 = 20;

pub(super) fn emit_cache_cell_clearer(assembler: &mut Assembler) {
    emit_cache_cell_access(assembler, true);
}
pub(crate) fn emit_cache_cell_resolver(assembler: &mut Assembler) {
    emit_cache_cell_access(assembler, false);
}
fn emit_cache_cell_access(assembler: &mut Assembler, clear: bool) {
    assembler
        .label(if clear {
            "clear_name_glyph_cache_cell"
        } else {
            "resolve_name_glyph_cache_cell"
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: CACHE_SLOT_COUNT,
        })
        .bne(Register::T0, Register::ZERO, "cache_slot_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("cache_slot_in_range")
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::A0,
            shift: 2,
        })
        .emit_all(load_address(Register::T1, NAME_GLYPH_CACHE_TABLE_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::T0,
            offset: 0,
        })
        .emit_all(load_address(Register::T1, NAME_FONT_PIXEL_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::T0,
            rt: Register::ZERO,
        });
    if !clear {
        assembler
            .emit(Instruction::Jr { rs: Register::RA })
            .emit(Instruction::nop());
        return;
    }
    assembler
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: CACHE_CELL_HEIGHT as u16,
        })
        .label("clear_cache_cell_row")
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: CACHE_CELL_ROW_BYTES as u16,
        })
        .label("clear_cache_cell_byte")
        .emit(Instruction::Sb {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -1,
        })
        .bgtz(Register::T2, "clear_cache_cell_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: CACHE_CELL_ROW_STRIDE - CACHE_CELL_ROW_BYTES,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .bgtz(Register::T1, "clear_cache_cell_row")
        .emit(Instruction::nop())
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
