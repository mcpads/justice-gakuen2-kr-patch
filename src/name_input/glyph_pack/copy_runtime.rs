use psx_r3000a::{Assembler, Instruction, Register, load_address};

use crate::name_input::NAME_GLYPH_PACK_CELL_COUNT;

/// Copy MA_ENT's scattered 20x20 packed cells into contiguous pack storage.
/// Coordinates are byte offsets into the 384-byte-stride font image.
/// Clobbers S0..S3 and T0..T7; the enclosing initializer preserves its caller.
pub(crate) fn emit_name_glyph_pack_copy(
    assembler: &mut Assembler,
    coordinate_address: u32,
    pixel_address: u32,
    destination: u32,
) {
    assembler
        .emit_all(load_address(Register::S0, coordinate_address))
        .emit_all(load_address(Register::S1, pixel_address))
        .emit_all(load_address(Register::S2, destination))
        .emit(Instruction::Ori {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: NAME_GLYPH_PACK_CELL_COUNT as u16,
        })
        .label("copy_name_pack_cell")
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 2,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::S1,
            rt: Register::T0,
        })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: 20,
        })
        .label("copy_name_pack_cell_row");
    for (register, offset) in [
        (Register::T3, 0_i16),
        (Register::T4, 2),
        (Register::T5, 4),
        (Register::T6, 6),
        (Register::T7, 8),
    ] {
        assembler.emit(Instruction::Lhu {
            rt: register,
            base: Register::T1,
            offset,
        });
    }
    for (register, offset) in [
        (Register::T3, 0_i16),
        (Register::T4, 2),
        (Register::T5, 4),
        (Register::T6, 6),
        (Register::T7, 8),
    ] {
        assembler.emit(Instruction::Sh {
            rt: register,
            base: Register::S2,
            offset,
        });
    }
    assembler
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 384,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 10,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -1,
        })
        .bgtz(Register::T2, "copy_name_pack_cell_row")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::S3,
            rs: Register::S3,
            immediate: -1,
        })
        .bgtz(Register::S3, "copy_name_pack_cell")
        .emit(Instruction::nop());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::{NAME_GLYPH_PACK_STORAGE_BYTES, runtime_test_machine};

    #[test]
    fn scattered_font_cells_copy_without_touching_other_memory() {
        let origin = 0x8008_0000;
        for displacement in [0, 0x800] {
            let coordinates = 0x1000 + displacement;
            let pixels = 0x10000 + displacement;
            let destination = 0x30000 + displacement;
            let mut memory = vec![0xa5; 0x40000];
            let mut expected = Vec::new();
            for cell in 0..NAME_GLYPH_PACK_CELL_COUNT {
                let offset = (cell / 19) * 20 * 384 + (cell % 19) * 10;
                memory[coordinates + cell * 2..coordinates + cell * 2 + 2]
                    .copy_from_slice(&(offset as u16).to_le_bytes());
                for row in 0..20 {
                    for column in 0..10 {
                        let value = (cell * 37 + row * 11 + column) as u8;
                        memory[pixels + offset + row * 384 + column] = value;
                        expected.push(value);
                    }
                }
            }
            assert_eq!(expected.len(), NAME_GLYPH_PACK_STORAGE_BYTES);
            let before = memory.clone();
            let mut assembler = Assembler::new();
            emit_name_glyph_pack_copy(
                &mut assembler,
                0x8000_0000 + coordinates as u32,
                0x8000_0000 + pixels as u32,
                0x8000_0000 + destination as u32,
            );
            assembler
                .emit(Instruction::Jr { rs: Register::RA })
                .emit(Instruction::nop());
            let program = assembler.assemble(origin).unwrap();
            let mut registers = [0; 32];
            runtime_test_machine::execute(program.bytes(), origin, &mut registers, &mut memory);
            assert_eq!(&memory[destination..destination + expected.len()], expected);
            assert_eq!(&memory[..destination], &before[..destination]);
            assert_eq!(
                &memory[destination + expected.len()..],
                &before[destination + expected.len()..]
            );
        }
    }
}
