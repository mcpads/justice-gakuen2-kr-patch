use psx_r3000a::{Assembler, Instruction, Register, load_address};

const CACHE_CELL_WORD_COUNT: i16 = 50;
const CACHE_SLOT_COUNT: i16 = 16;

pub(super) fn emit_dialogue_cache_cell_clearer(
    assembler: &mut Assembler,
    current_atlas_pointer_address: u32,
    cache_code_start: u16,
) {
    emit_dialogue_cache_cell_access(
        assembler,
        current_atlas_pointer_address,
        cache_code_start,
        true,
    );
}

pub(super) fn emit_dialogue_cache_cell_resolver(
    assembler: &mut Assembler,
    current_atlas_pointer_address: u32,
    cache_code_start: u16,
) {
    emit_dialogue_cache_cell_access(
        assembler,
        current_atlas_pointer_address,
        cache_code_start,
        false,
    );
}

fn emit_dialogue_cache_cell_access(
    assembler: &mut Assembler,
    current_atlas_pointer_address: u32,
    cache_code_start: u16,
    clear: bool,
) {
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
        .bne(Register::T0, Register::ZERO, "dialogue_cache_slot_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("dialogue_cache_slot_in_range")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: cache_code_start as i16,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 200,
        })
        .emit(Instruction::Multu {
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit_all(load_address(Register::T0, current_atlas_pointer_address))
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::V1,
            rs: Register::ZERO,
            immediate: CACHE_CELL_WORD_COUNT as u16,
        })
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
        .label("clear_dialogue_cache_cell_word")
        .emit(Instruction::Sw {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: -1,
        })
        .bgtz(Register::V1, "clear_dialogue_cache_cell_word")
        .emit(Instruction::nop())
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
