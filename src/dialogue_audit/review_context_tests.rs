use std::collections::BTreeMap;

use super::{render_context_markup, select_diverse_contexts};
use crate::dialogue_audit::codebook_model::ResolvedDialogueGlyph;
use crate::dialogue_audit::review_model::DialogueCodebookReviewContext;
use crate::dialogue_audit::tokens::{DialogueTokenKind, ParsedDialogueToken};

#[test]
fn context_marks_the_target_without_inventing_unknown_text() {
    let hashes = vec!["known-hash".to_string(), "target-hash".to_string()];
    let glyphs = BTreeMap::from([(
        "known-hash".to_string(),
        ResolvedDialogueGlyph {
            text: "前".to_string(),
            semantic_id: None,
            codebook_status: "source_pixel_verified".to_string(),
        },
    )]);
    let tokens = vec![
        token(DialogueTokenKind::FixedGlyph, 0),
        token(DialogueTokenKind::FixedGlyph, 1),
        token(DialogueTokenKind::LineBreak, 0x3000),
        token(DialogueTokenKind::FixedGlyph, 0),
        token(DialogueTokenKind::MessageEnd, 0x3001),
    ];

    assert_eq!(
        render_context_markup(&tokens, &hashes, &glyphs, "target-hash", 1).unwrap(),
        "前⟦TARGET⟧\n前{#message_end}"
    );
}

#[test]
fn context_selection_prefers_distinct_assets() {
    let selected = select_diverse_contexts(vec![
        context("DAT2/MGC01.BIZ", 0),
        context("DAT2/MGC01.BIZ", 1),
        context("DAT2/MGG04T.BIZ", 2),
    ]);

    assert_eq!(selected.len(), 3);
    assert_eq!(selected[0].source_asset, "DAT2/MGC01.BIZ");
    assert_eq!(selected[1].source_asset, "DAT2/MGG04T.BIZ");
    assert_eq!(selected[2].entry_index, 1);
}

fn token(kind: DialogueTokenKind, code: u16) -> ParsedDialogueToken {
    ParsedDialogueToken {
        kind,
        code,
        arguments: Vec::new(),
    }
}

fn context(source_asset: &str, entry_index: usize) -> DialogueCodebookReviewContext {
    DialogueCodebookReviewContext {
        coordinate_id: format!("{source_asset}#{entry_index}"),
        source_asset: source_asset.to_string(),
        bank_index: 0,
        entry_index,
        decoded_offset: "0x00000".to_string(),
        target_code: "0x0000".to_string(),
        target_token_index: 0,
        target_occurrence_count: 1,
        context_markup: "⟦TARGET⟧".to_string(),
    }
}
