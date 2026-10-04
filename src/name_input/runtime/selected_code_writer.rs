use psx_r3000a::{Assembler, Instruction, Register};

pub(super) fn emit_selected_code_writer(assembler: &mut Assembler) {
    assembler
        .label("write_selected_name_code")
        .emit(Instruction::Sh {
            rt: Register::V1,
            base: Register::V0,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
