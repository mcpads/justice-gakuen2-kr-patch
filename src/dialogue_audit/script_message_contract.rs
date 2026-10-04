use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const OPCODE_DISPATCH_OFFSET: usize = 0x056c;
const COMPACT_MESSAGE_HANDLER_RUNTIME_ADDRESS: u32 = 0x800a_ca34;
const MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS: u32 = 0x800b_84d4;

pub(super) fn validate_compact_message_opcode_contract(mgame: &[u8]) -> Result<()> {
    for (opcode, handler, presentation) in [(0x1f, 0x800a_ca34, 9), (0x22, 0x800a_cadc, 10)] {
        validate_handler(mgame, opcode, handler, presentation)?;
    }
    Ok(())
}

fn validate_handler(mgame: &[u8], opcode: u8, handler: u32, presentation: i16) -> Result<()> {
    let dispatch_offset = OPCODE_DISPATCH_OFFSET + usize::from(opcode) * 4;
    ensure!(
        read_word(mgame, dispatch_offset)? == handler,
        "compact message opcode handler changed"
    );

    let expected: [(u32, Instruction); 15] = [
        (
            0x800a_ca68,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::A1,
                offset: 1,
            },
        ),
        (
            0x800a_ca78,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A1,
                offset: 2,
            },
        ),
        (
            0x800a_ca7c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x800a_ca80,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x1854,
            },
        ),
        (
            0x800a_ca84,
            Instruction::Sll {
                rd: Register::A0,
                rt: Register::A0,
                shift: 8,
            },
        ),
        (
            0x800a_ca88,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x800a_ca8c,
            Instruction::Or {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x800a_ca90,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8010,
            },
        ),
        (
            0x800a_ca94,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::V1,
                rt: Register::AT,
            },
        ),
        (
            0x800a_ca98,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x1000,
            },
        ),
        (
            0x800a_ca9c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x800a_caa0,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x800a_caa4,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x800a_cabc,
            Instruction::Jal {
                target: MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS,
            },
        ),
        (
            0x800a_cac0,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: presentation,
            },
        ),
    ];
    for (runtime_address, expected_instruction) in expected {
        let runtime_address = runtime_address + (handler - COMPACT_MESSAGE_HANDLER_RUNTIME_ADDRESS);
        let instruction_offset = runtime_address
            .checked_sub(MGAME_RUNTIME_BASE)
            .and_then(|offset| usize::try_from(offset).ok())
            .context("compact message handler address is outside MGAME")?;
        let decoded =
            decode(read_word(mgame, instruction_offset)?, runtime_address).with_context(|| {
                format!("failed to decode compact message handler at {runtime_address:#010x}")
            })?;
        ensure!(
            decoded == expected_instruction,
            "compact message handler changed at {runtime_address:#010x}: expected {expected_instruction:?}, got {decoded:?}"
        );
    }
    Ok(())
}

fn read_word(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated MGAME word at +0x{offset:x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
