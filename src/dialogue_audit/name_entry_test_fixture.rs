use psx_r3000a::{Instruction, Register, encode};

use super::name_entry::PAGE_POINTER_TABLE_OFFSET;
use super::name_entry_input::install_name_input_path_fixture;
use super::name_entry_renderer::install_name_renderer_fixture;

const OVERLAY_SIZE: usize = 36_796;
const OVERLAY_RUNTIME_BASE: u32 = 0x8017_a000;
const PADDING_CODE: u16 = 0x061e;

pub(super) fn name_entry_overlay() -> Vec<u8> {
    let mut overlay = vec![0; OVERLAY_SIZE];
    let pages = [(0x0b14, 90, 6), (0x0bc8, 90, 8), (0x0c7c, 70, 8)];
    let mut next_code = 1u16;
    let mut selectable_codes = Vec::new();
    for (index, (offset, cell_count, padding_count)) in pages.into_iter().enumerate() {
        overlay[PAGE_POINTER_TABLE_OFFSET + index * 4..PAGE_POINTER_TABLE_OFFSET + index * 4 + 4]
            .copy_from_slice(&(OVERLAY_RUNTIME_BASE + offset as u32).to_le_bytes());
        for cell_index in 0..cell_count {
            let code = if cell_index < padding_count {
                PADDING_CODE
            } else {
                let code = next_code;
                next_code += 1;
                selectable_codes.push(code);
                code
            };
            overlay[offset + cell_index * 2..offset + cell_index * 2 + 2]
                .copy_from_slice(&code.to_le_bytes());
        }
    }
    for (index, code) in selectable_codes.into_iter().enumerate() {
        overlay[0x0d18 + index * 2..0x0d18 + index * 2 + 2].copy_from_slice(&code.to_le_bytes());
    }
    overlay[0x0d14..0x0d16].copy_from_slice(&PADDING_CODE.to_le_bytes());
    overlay[0x0d16..0x0d18].copy_from_slice(&0x0fffu16.to_le_bytes());
    overlay[0x0ee0..0x0ee2].copy_from_slice(&u16::MAX.to_le_bytes());

    for (offset, bytes) in [
        (
            0x15f8,
            &hex_bytes(
                "0c0600a018140c0c20a090140c0500b418140c0c20b490140c0400c818140c0c20c8e0140c07008c18140c0c208c90140c0300dc18140c0c20dca014",
            )[..],
        ),
        (
            0x1674,
            &hex_bytes(
                "0d0c008c3c140d0c3c8c3c140d0c788c3c140d0cb48c3c140d0c00a03c140d0c3ca03c140d0c78a03c140d0cb4a03c14",
            )[..],
        ),
        (
            0x1724,
            &hex_bytes(
                "0d0c00b43c140d0c00c83c140d0c00dc3c140e0c00783c140d0c3cb43c140d0c3cc83c140d0c3cdc3c140e0c3c783c140d0c78b43c140d0c78c83c140d0c78dc3c140e0c78783c140d0cb4b43c140d0cb4c83c140d0cb4dc3c140e0cb4783c14",
            )[..],
        ),
    ] {
        overlay[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    for (offset, instruction) in [
        (
            0x6804,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 5,
            },
        ),
        (
            0x6818,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x6828,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 5,
            },
        ),
        (
            0x683c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x684c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 3,
            },
        ),
        (
            0x6860,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x6988,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        ),
        (
            0x69d4,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x12,
            },
        ),
        (
            0x69dc,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x22,
            },
        ),
        (
            0x69e0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A0,
                immediate: 0x32,
            },
        ),
        (
            0x69f0,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::A2,
                offset: 0,
            },
        ),
        (
            0x6a14,
            Instruction::Lui {
                rt: Register::A3,
                immediate: 0x801f,
            },
        ),
        (
            0x6a18,
            Instruction::Ori {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 0x1886,
            },
        ),
        (
            0x3f08,
            Instruction::Lui {
                rt: Register::S4,
                immediate: 0x8018,
            },
        ),
        (
            0x2f30,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x2f44,
            Instruction::Addiu {
                rt: Register::FP,
                rs: Register::ZERO,
                immediate: 20,
            },
        ),
        (
            0x2f4c,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::ZERO,
                immediate: 180,
            },
        ),
        (
            0x2ff8,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 140,
            },
        ),
        (
            0x2ffc,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::S0,
                offset: 22,
            },
        ),
        (
            0x300c,
            Instruction::Sb {
                rt: Register::S3,
                base: Register::S0,
                offset: 20,
            },
        ),
        (
            0x3010,
            Instruction::Sb {
                rt: Register::T0,
                base: Register::S0,
                offset: 21,
            },
        ),
        (
            0x3014,
            Instruction::Sh {
                rt: Register::FP,
                base: Register::S0,
                offset: 24,
            },
        ),
        (
            0x3018,
            Instruction::Sh {
                rt: Register::FP,
                base: Register::S0,
                offset: 26,
            },
        ),
        (
            0x3030,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: 50,
            },
        ),
        (
            0x3040,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::S3,
                immediate: 20,
            },
        ),
        (
            0x3068,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::S2,
                rt: Register::S6,
            },
        ),
        (
            0x306c,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: OVERLAY_RUNTIME_BASE + 0x2f54,
            },
        ),
        (
            0x3f0c,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: -18828,
            },
        ),
        (
            0x4088,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S6,
                immediate: 8,
            },
        ),
        (
            0x4104,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x8018,
            },
        ),
        (
            0x4108,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -18828,
            },
        ),
        (
            0x4834,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x8018,
            },
        ),
        (
            0x4838,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: -18652,
            },
        ),
        (
            0x49a0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S5,
                immediate: 16,
            },
        ),
        (
            0x4a50,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x8018,
            },
        ),
        (
            0x4a54,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -18652,
            },
        ),
        (
            0x7df0,
            Instruction::Lui {
                rt: Register::S2,
                immediate: 0x8018,
            },
        ),
        (
            0x7df4,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: -18952,
            },
        ),
        (
            0x7f74,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S4,
                immediate: 2,
            },
        ),
        (
            0x7f84,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S7,
                immediate: 5,
            },
        ),
        (
            0x8000,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x8018,
            },
        ),
        (
            0x8004,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -18946,
            },
        ),
    ] {
        let pc = OVERLAY_RUNTIME_BASE + offset as u32;
        overlay[offset..offset + 4]
            .copy_from_slice(&encode(&instruction, pc).unwrap().to_le_bytes());
    }
    install_name_input_path_fixture(&mut overlay);
    install_name_renderer_fixture(&mut overlay);
    overlay
}

fn hex_bytes(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
