use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::sha256_bytes;

use super::codebook_model::ResolvedDialogueGlyph;
use super::corpus_model::DialogueCorpusToken;
use super::format::hex_code;
use super::runtime_insertions::dialogue_control_spec;
use super::tokens::{DialogueTokenKind, ParsedDialogueToken};

pub(super) fn bytes_to_words(data: &[u8]) -> Vec<u16> {
    data.as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

pub(super) fn resolve_tokens(
    source_path: &str,
    fixed_cell_sha256: &[String],
    glyphs: &BTreeMap<String, ResolvedDialogueGlyph>,
    tokens: &[ParsedDialogueToken],
) -> Result<Vec<DialogueCorpusToken>> {
    tokens
        .iter()
        .map(|token| match token.kind {
            DialogueTokenKind::FixedGlyph => {
                let pixel_sha256 = fixed_cell_sha256
                    .get(usize::from(token.code))
                    .with_context(|| {
                        format!(
                            "{} has no fixed cell for {}",
                            source_path,
                            hex_code(token.code)
                        )
                    })?;
                let glyph = glyphs.get(pixel_sha256).with_context(|| {
                    format!(
                        "{} {} pixel {} is unresolved",
                        source_path,
                        hex_code(token.code),
                        pixel_sha256
                    )
                })?;
                Ok(DialogueCorpusToken::Glyph {
                    code: hex_code(token.code),
                    pixel_sha256: pixel_sha256.clone(),
                    text: glyph.text.clone(),
                    semantic_id: glyph.semantic_id.clone(),
                    codebook_status: glyph.codebook_status.clone(),
                })
            }
            DialogueTokenKind::RuntimeExtensionGlyph => bail!(
                "{} directly uses runtime extension glyph {}",
                source_path,
                hex_code(token.code)
            ),
            _ => {
                let spec = dialogue_control_spec(token.code).with_context(|| {
                    format!("missing control spec for {}", hex_code(token.code))
                })?;
                ensure!(
                    spec.argument_word_count == token.arguments.len(),
                    "control {} argument width changed",
                    hex_code(token.code)
                );
                Ok(DialogueCorpusToken::Control {
                    code: hex_code(token.code),
                    semantic_name: spec.semantic_name.to_string(),
                    arguments: token.arguments.iter().copied().map(hex_code).collect(),
                })
            }
        })
        .collect()
}

pub(super) fn reconstruct_words(
    tokens: &[DialogueCorpusToken],
    padding: usize,
) -> Result<Vec<u16>> {
    ensure!(padding <= 1, "dialogue alignment padding exceeds one word");
    let mut words = Vec::new();
    for token in tokens {
        let (code, arguments) = match token {
            DialogueCorpusToken::Glyph { code, .. } => (code, &[][..]),
            DialogueCorpusToken::Control {
                code, arguments, ..
            } => (code, arguments.as_slice()),
        };
        words.push(parse_hex_word(code)?);
        for argument in arguments {
            words.push(parse_hex_word(argument)?);
        }
    }
    words.extend(std::iter::repeat_n(0, padding));
    Ok(words)
}

pub(super) fn parse_hex_word(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("word lacks 0x prefix: {value}"))?;
    ensure!(
        digits.len() == 4,
        "word is not four hexadecimal digits: {value}"
    );
    Ok(u16::from_str_radix(digits, 16)?)
}

pub(super) fn source_markup(tokens: &[DialogueCorpusToken]) -> String {
    let mut output = String::new();
    for token in tokens {
        match token {
            DialogueCorpusToken::Glyph { text, .. } => {
                for character in text.chars() {
                    if matches!(character, '\\' | '{' | '}') {
                        output.push('\\');
                    }
                    output.push(character);
                }
            }
            DialogueCorpusToken::Control {
                semantic_name,
                arguments,
                ..
            } => {
                output.push_str("{#");
                output.push_str(semantic_name);
                for argument in arguments {
                    output.push(':');
                    output.push_str(argument);
                }
                output.push('}');
            }
        }
    }
    output
}

pub(super) fn semantic_sha256(tokens: &[DialogueCorpusToken]) -> String {
    let mut canonical = Vec::new();
    for token in tokens {
        match token {
            DialogueCorpusToken::Glyph {
                text, semantic_id, ..
            } => {
                canonical.push(1);
                append_field(&mut canonical, text.as_bytes());
                append_field(
                    &mut canonical,
                    semantic_id.as_deref().unwrap_or("").as_bytes(),
                );
            }
            DialogueCorpusToken::Control {
                semantic_name,
                arguments,
                ..
            } => {
                canonical.push(2);
                append_field(&mut canonical, semantic_name.as_bytes());
                canonical.extend_from_slice(&(arguments.len() as u32).to_le_bytes());
                for argument in arguments {
                    append_field(&mut canonical, argument.as_bytes());
                }
            }
        }
    }
    sha256_bytes(&canonical)
}

fn append_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}
