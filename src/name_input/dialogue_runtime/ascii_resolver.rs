use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::{DIGIT_KEYS, LATIN_KEYS, NameGlyphConsumerLayout, SYMBOL_KEYS};

const FAILURE: u16 = u16::MAX;

struct AsciiNameCodeLayout {
    uppercase_start: u16,
    lowercase_start: u16,
    digit_start: u16,
    symbol_start: u16,
    legacy_indices: BTreeSet<u16>,
}

pub(super) fn ascii_name_legacy_indices(layout: &NameGlyphConsumerLayout) -> Result<BTreeSet<u16>> {
    Ok(ascii_name_code_layout(layout)?.legacy_indices)
}

pub(super) fn emit_ascii_name_code_resolver(
    assembler: &mut Assembler,
    layout: &NameGlyphConsumerLayout,
    symbol_table_address: u32,
) -> Result<()> {
    let code_layout = ascii_name_code_layout(layout)?;
    let uppercase_start = code_layout.uppercase_start;
    let lowercase_start = code_layout.lowercase_start;
    let digit_start = code_layout.digit_start;
    let symbol_start = code_layout.symbol_start;

    assembler
        .label("resolve_ascii_name_code")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: -(b'A' as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: 26,
        })
        .bne(Register::T1, Register::ZERO, "ascii_uppercase_code")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: -(b'a' as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: 26,
        })
        .bne(Register::T1, Register::ZERO, "ascii_lowercase_code")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: -(b'0' as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: 10,
        })
        .bne(Register::T1, Register::ZERO, "ascii_digit_code")
        .emit(Instruction::nop())
        .emit_all(load_address(Register::T1, symbol_table_address))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("scan_ascii_symbol")
        .emit(Instruction::Lbu {
            rt: Register::T2,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 1,
        })
        .beq(Register::T2, Register::A0, "ascii_symbol_code")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T2,
            rs: Register::T0,
            immediate: SYMBOL_KEYS.len() as i16,
        })
        .bne(Register::T2, Register::ZERO, "scan_ascii_symbol")
        .emit(Instruction::nop())
        .jump("ascii_code_failed")
        .emit(Instruction::nop())
        .label("ascii_uppercase_code")
        .jump("ascii_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::T0,
            immediate: uppercase_start as i16,
        })
        .label("ascii_lowercase_code")
        .jump("ascii_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::T0,
            immediate: lowercase_start as i16,
        })
        .label("ascii_digit_code")
        .jump("ascii_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::T0,
            immediate: digit_start as i16,
        })
        .label("ascii_symbol_code")
        .jump("ascii_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::T0,
            immediate: symbol_start as i16 - 1,
        })
        .label("ascii_code_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .label("ascii_code_ready")
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
    Ok(())
}

fn ascii_name_code_layout(layout: &NameGlyphConsumerLayout) -> Result<AsciiNameCodeLayout> {
    let uppercase_start = layout
        .code_for_active_character('A')
        .context("shared name layout lost uppercase Latin keys")?;
    let lowercase_start = layout
        .code_for_active_character('a')
        .context("shared name layout lost lowercase Latin keys")?;
    let digit_start = layout
        .code_for_active_character('0')
        .context("shared name layout lost decimal keys")?;
    let symbol_start = layout
        .code_for_active_character(' ')
        .context("shared name layout lost symbol keys")?;
    let mut legacy_indices = BTreeSet::new();
    for (characters, start) in [
        (
            LATIN_KEYS.get(..26).context("Latin key prefix changed")?,
            uppercase_start,
        ),
        (
            LATIN_KEYS.get(26..).context("Latin key suffix changed")?,
            lowercase_start,
        ),
        (DIGIT_KEYS, digit_start),
        (SYMBOL_KEYS, symbol_start),
    ] {
        for (index, character) in characters.chars().enumerate() {
            let code = layout
                .code_for_active_character(character)
                .with_context(|| format!("shared name layout lost ASCII key {character:?}"))?;
            ensure!(
                Some(code) == start.checked_add(u16::try_from(index)?),
                "shared ASCII name codes are not consecutive in keyboard order"
            );
            ensure!(
                legacy_indices.insert(code),
                "shared ASCII name code is assigned to more than one key"
            );
        }
    }
    Ok(AsciiNameCodeLayout {
        uppercase_start,
        lowercase_start,
        digit_start,
        symbol_start,
        legacy_indices,
    })
}
