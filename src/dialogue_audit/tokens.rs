use anyhow::{Result, bail, ensure};

use super::runtime_insertions::dialogue_control_spec;

pub(super) const SKIPPED_CODE: u16 = 0x061e;
pub(super) const MESSAGE_END_CODE: u16 = 0x3001;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DialogueTokenKind {
    FixedGlyph,
    RuntimeExtensionGlyph,
    SkippedCode,
    RuntimeInsertion,
    ParameterizedRuntimeInsertion,
    LineBreak,
    MessageEnd,
    RendererMode,
    PaletteStyle,
}

impl DialogueTokenKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::FixedGlyph => "fixed_glyph",
            Self::RuntimeExtensionGlyph => "runtime_extension_glyph",
            Self::SkippedCode => "skipped_code",
            Self::RuntimeInsertion => "runtime_insertion",
            Self::ParameterizedRuntimeInsertion => "parameterized_runtime_insertion",
            Self::LineBreak => "line_break",
            Self::MessageEnd => "message_end",
            Self::RendererMode => "renderer_mode",
            Self::PaletteStyle => "palette_style",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedDialogueToken {
    pub(super) kind: DialogueTokenKind,
    pub(super) code: u16,
    pub(super) arguments: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TokenizedDialogueMessage {
    pub(super) tokens: Vec<ParsedDialogueToken>,
    pub(super) alignment_padding_word_count: usize,
}

pub(super) fn tokenize_dialogue_message(
    raw_words: &[u16],
    fixed_cell_count: usize,
    addressable_slot_count: usize,
) -> Result<TokenizedDialogueMessage> {
    ensure!(
        fixed_cell_count <= addressable_slot_count,
        "fixed dialogue cells exceed addressable slots"
    );

    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut terminated = false;
    while cursor < raw_words.len() {
        let code = raw_words[cursor];
        cursor += 1;
        if code == MESSAGE_END_CODE {
            tokens.push(ParsedDialogueToken {
                kind: DialogueTokenKind::MessageEnd,
                code,
                arguments: Vec::new(),
            });
            terminated = true;
            break;
        }

        let (kind, argument_word_count) = match code {
            SKIPPED_CODE => (DialogueTokenKind::SkippedCode, 0),
            0x2000..=0x2002 | 0x200a..=0x200d => (DialogueTokenKind::RuntimeInsertion, 0),
            0x2003..=0x2009 => (DialogueTokenKind::ParameterizedRuntimeInsertion, 1),
            0x3000 => (DialogueTokenKind::LineBreak, 0),
            0x3002 => (DialogueTokenKind::RendererMode, 1),
            0x3003 => (DialogueTokenKind::PaletteStyle, 1),
            _ if usize::from(code) < fixed_cell_count => (DialogueTokenKind::FixedGlyph, 0),
            _ if usize::from(code) < addressable_slot_count => {
                (DialogueTokenKind::RuntimeExtensionGlyph, 0)
            }
            _ => bail!("unrecognized dialogue code 0x{code:04x}"),
        };
        let spec = dialogue_control_spec(code)
            .filter(|spec| spec.argument_word_count == argument_word_count);
        ensure!(
            matches!(
                kind,
                DialogueTokenKind::FixedGlyph | DialogueTokenKind::RuntimeExtensionGlyph
            ) || spec.is_some(),
            "dialogue control code 0x{code:04x} has no matching semantic specification"
        );
        ensure!(
            cursor + argument_word_count <= raw_words.len(),
            "dialogue code 0x{code:04x} lacks {argument_word_count} argument word(s)"
        );
        let arguments = raw_words[cursor..cursor + argument_word_count].to_vec();
        cursor += argument_word_count;
        tokens.push(ParsedDialogueToken {
            kind,
            code,
            arguments,
        });
    }

    ensure!(terminated, "dialogue message lacks 0x3001 terminator");
    let trailing = &raw_words[cursor..];
    ensure!(
        trailing.is_empty() || trailing == [0],
        "dialogue message has non-alignment data after 0x3001 terminator"
    );
    Ok(TokenizedDialogueMessage {
        tokens,
        alignment_padding_word_count: trailing.len(),
    })
}
