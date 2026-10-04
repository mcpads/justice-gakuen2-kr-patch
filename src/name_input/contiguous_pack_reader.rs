use anyhow::Result;
use psx_r3000a::{Assembler, Instruction, Register, load_address};

pub(crate) fn emit_contiguous_pack_byte_reader(
    assembler: &mut Assembler,
    pack_address: u32,
    byte_count: usize,
) -> Result<()> {
    assembler
        .label("read_name_glyph_pack_byte")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: i16::try_from(byte_count)?,
        })
        .bne(Register::T0, Register::ZERO, "name_pack_offset_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("name_pack_offset_in_range")
        .emit_all(load_address(Register::T0, pack_address))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::A0,
        })
        .emit(Instruction::Lbu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    Ok(())
}
