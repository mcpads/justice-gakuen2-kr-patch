//! Relabel the native password values, never the codec or its input actions.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::sha256_bytes;
use crate::text::{common_menu_ascii_glyph_code, fixed_menu_glyph_code};

use super::pass_fixed_presentation::PASSWORD_GRID_CODES;

const SOURCE_ALPHABET: &str = "アイウエオABCDEカキクケコFGHIJサシスセソKLMNOタチツテトPQRSTナニヌネノUVWXYハヒフヘホZ1234マミムメモ56789ラリルレロ0";
const DISPLAY_START: usize = 0x2cc;
const VALUE_START: usize = 0x36c;

// Source MENU cells visually checked against the password input renderer.
// Native cells 0x65/0x66/0x67 are not ASCII period/quotes and are not admitted.
pub(super) const NATIVE_PASSWORD_SYMBOL_CODES: [u16; 3] = [0x0061, 0x0060, 0x0062];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PasswordAlphabet {
    source: String,
    ascii: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PasswordAlphabetReport {
    source: String,
    ascii: String,
    display_codes: Vec<String>,
    value_bytes_and_actions_preserved: bool,
    changes_confined_to_display_table: bool,
    definition_sha256: String,
}

impl PasswordAlphabet {
    pub(super) fn load(assets: &Path) -> Result<Self> {
        let path = assets.join("edit/runtime-text/password-alphabet.json");
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let alphabet: Self = serde_json::from_slice(&bytes)?;
        alphabet.validate()?;
        Ok(alphabet)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.source == SOURCE_ALPHABET,
            "password source alphabet changed"
        );
        let ascii = self.ascii.chars().collect::<Vec<_>>();
        ensure!(
            ascii.len() == 76 && ascii.iter().collect::<BTreeSet<_>>().len() == 76,
            "password alphabet requires 76 distinct characters"
        );
        for (source, replacement) in self.source.chars().zip(ascii) {
            ensure!(
                replacement.is_ascii_graphic(),
                "password alphabet must use visible ASCII"
            );
            ensure!(
                !source.is_ascii() || source == replacement,
                "password alphabet must preserve existing Latin/digit values"
            );
            Self::glyph_code(replacement)?;
        }
        Ok(())
    }

    pub(super) fn glyph_code(character: char) -> Result<u16> {
        match character {
            '<' => Some(NATIVE_PASSWORD_SYMBOL_CODES[0]),
            '>' => Some(NATIVE_PASSWORD_SYMBOL_CODES[1]),
            '$' => Some(NATIVE_PASSWORD_SYMBOL_CODES[2]),
            '.' | '\"' | '\'' => None,
            _ => {
                common_menu_ascii_glyph_code(character).or_else(|| fixed_menu_glyph_code(character))
            }
        }
        .with_context(|| format!("password alphabet lacks a native ASCII glyph for {character:?}"))
    }

    pub(super) fn characters(&self) -> impl Iterator<Item = char> + '_ {
        self.ascii.chars()
    }

    pub(super) fn install(
        &self,
        pass: &mut [u8],
    ) -> Result<(PasswordAlphabetReport, Vec<DecodedDataClaim>)> {
        self.validate()?;
        let source = pass.to_vec();
        ensure!(pass.len() >= 0x3d4, "truncated password tables");
        ensure!(
            pass[VALUE_START..VALUE_START + 76]
                .iter()
                .copied()
                .eq(0u8..76)
                && pass[VALUE_START + 76..VALUE_START + 80] == [0x80, 0x81, 0x82, 0x83],
            "password input values or actions changed"
        );
        let mut codes = Vec::new();
        for (index, character) in self.ascii.chars().enumerate() {
            let offset = DISPLAY_START + index * 2;
            ensure!(
                u16::from_le_bytes(pass[offset..offset + 2].try_into()?)
                    == PASSWORD_GRID_CODES[index],
                "password display slot {index} changed before relabeling"
            );
            let code = Self::glyph_code(character)?;
            ensure!(code < 0x400, "password glyph leaves MENU pages");
            codes.push(code);
        }
        ensure!(
            codes.iter().collect::<BTreeSet<_>>().len() == 76,
            "password display glyphs collide"
        );
        // Grid selection stores VALUE_START[index]. Both the 20-character and
        // 14-character displays then read DISPLAY_START[value * 2]. Values
        // 0..75 are dense; the four sparse action values are not password data.
        for (index, code) in codes.iter().enumerate() {
            let offset = DISPLAY_START + index * 2;
            pass[offset..offset + 2].copy_from_slice(&code.to_le_bytes());
        }
        let claims = DecodedDataClaim::from_effective_ranges(
            "password-alphabet",
            "relabel shared password input and display glyphs",
            &source,
            pass,
            [[DISPLAY_START, DISPLAY_START + 76 * 2]],
        )?;
        ensure!(
            source[..DISPLAY_START] == pass[..DISPLAY_START]
                && source[DISPLAY_START + 76 * 2..] == pass[DISPLAY_START + 76 * 2..],
            "password relabeling changed codec, value bytes or action glyphs"
        );
        Ok((
            PasswordAlphabetReport {
                source: self.source.clone(),
                ascii: self.ascii.clone(),
                display_codes: codes.iter().map(|code| format!("0x{code:04x}")).collect(),
                value_bytes_and_actions_preserved: true,
                changes_confined_to_display_table: true,
                definition_sha256: sha256_bytes(
                    format!("{}\n{}\n", self.source, self.ascii).as_bytes(),
                ),
            },
            claims,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alphabet() -> PasswordAlphabet {
        serde_json::from_str(crate::test_input::read_str(
            "assets/menu/mode-descendants/edit/runtime-text/password-alphabet.json",
        ))
        .unwrap()
    }

    #[test]
    #[ignore = "requires assets/"]
    fn every_password_value_roundtrips_through_the_relabelled_grid() {
        let alphabet = alphabet();
        let mut pass = vec![0xa5; 0x8000];
        for (index, code) in PASSWORD_GRID_CODES.iter().enumerate() {
            pass[DISPLAY_START + index * 2..DISPLAY_START + index * 2 + 2]
                .copy_from_slice(&code.to_le_bytes());
        }
        for (index, value) in (0u8..76).chain([0x80, 0x81, 0x82, 0x83]).enumerate() {
            pass[VALUE_START + index] = value;
        }
        let before = pass.clone();
        alphabet.install(&mut pass).unwrap();
        let grid = pass[DISPLAY_START..DISPLAY_START + 152].as_chunks::<2>().0;
        for (index, value) in pass[VALUE_START..VALUE_START + 76]
            .iter()
            .copied()
            .enumerate()
        {
            let displayed = grid[usize::from(value)];
            let selected = grid.iter().position(|code| *code == displayed).unwrap();
            assert_eq!(selected, index);
            assert_eq!(pass[VALUE_START + selected], value);
        }
        assert_eq!(&before[..DISPLAY_START], &pass[..DISPLAY_START]);
        assert_eq!(&before[DISPLAY_START + 152..], &pass[DISPLAY_START + 152..]);
    }

    #[test]
    fn password_symbols_do_not_borrow_visually_mismatched_common_cells() {
        assert_eq!(PasswordAlphabet::glyph_code('<').unwrap(), 0x61);
        assert_eq!(PasswordAlphabet::glyph_code('>').unwrap(), 0x60);
        assert_eq!(PasswordAlphabet::glyph_code('$').unwrap(), 0x62);
        assert!(PasswordAlphabet::glyph_code('.').is_err());
        assert!(PasswordAlphabet::glyph_code('\"').is_err());
        assert!(PasswordAlphabet::glyph_code('\'').is_err());
    }

    #[test]
    #[ignore = "requires assets/"]
    fn duplicate_or_reassigned_alphabet_characters_are_rejected() {
        let mut alphabet = alphabet();
        alphabet.ascii = alphabet.ascii.replacen('b', "a", 1);
        assert!(alphabet.validate().is_err());
        alphabet = serde_json::from_str(crate::test_input::read_str(
            "assets/menu/mode-descendants/edit/runtime-text/password-alphabet.json",
        ))
        .unwrap();
        alphabet.ascii = alphabet.ascii.replacen('A', "~", 1);
        assert!(alphabet.validate().is_err());
    }
}
