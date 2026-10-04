use super::atlas::ParsedDialogueAtlas;
use super::summary::DialogueSummary;
use super::tokens::{DialogueTokenKind, ParsedDialogueToken};

fn atlas(cell_hashes: &[&str]) -> ParsedDialogueAtlas {
    ParsedDialogueAtlas {
        tim_size: 700,
        clut_vram_x: 0,
        clut_vram_y: 480,
        clut_color_count: 16,
        image_vram_x: 0,
        image_vram_y: 0,
        pixel_data_offset: 100,
        fixed_cell_count: cell_hashes.len(),
        addressable_slot_count: 4,
        source_extension_start: 700,
        source_extension_end: 900,
        source_extension_nonzero_byte_count: 0,
        source_extension_sha256: "extension".to_string(),
        fixed_atlas_sha256: "atlas".to_string(),
        fixed_cell_sha256: cell_hashes.iter().map(|hash| (*hash).to_string()).collect(),
    }
}

fn token(kind: DialogueTokenKind, code: u16) -> ParsedDialogueToken {
    ParsedDialogueToken {
        kind,
        code,
        arguments: Vec::new(),
    }
}

#[test]
fn summarizes_asset_scoped_variants_and_message_usage_separately() {
    let mut summary = DialogueSummary::new();
    summary
        .record_atlas("A", &atlas(&["shared", "a-one", "a-two"]))
        .unwrap();
    summary
        .record_atlas("B", &atlas(&["shared", "b-one", "b-two"]))
        .unwrap();
    summary
        .record_message(
            "A",
            &[
                token(DialogueTokenKind::FixedGlyph, 0),
                token(DialogueTokenKind::FixedGlyph, 2),
                token(DialogueTokenKind::LineBreak, 0x3000),
                token(DialogueTokenKind::MessageEnd, 0x3001),
            ],
            1,
        )
        .unwrap();
    summary
        .record_message(
            "B",
            &[
                token(DialogueTokenKind::FixedGlyph, 0),
                token(DialogueTokenKind::RuntimeExtensionGlyph, 3),
                token(DialogueTokenKind::MessageEnd, 0x3001),
            ],
            0,
        )
        .unwrap();

    let result = summary.finish(2).unwrap();

    assert_eq!(result.token_count, 7);
    assert_eq!(result.fixed_glyph_occurrence_count, 3);
    assert_eq!(result.runtime_extension_glyph_occurrence_count, 1);
    assert_eq!(result.alignment_padding_word_count, 1);
    assert_eq!(result.font.shared_identical_prefix_cell_count, 1);
    assert_eq!(result.font.common_fixed_cells_identical_across_assets, 1);
    assert_eq!(result.font.common_fixed_cells_with_asset_variants, 2);
    assert_eq!(result.font.all_source_pixel_hash_count, 5);
    assert_eq!(result.font.used_source_pixel_hash_count, 2);
    assert_eq!(result.font.used_fixed_asset_code_pair_count, 3);
    assert_eq!(result.font.unreferenced_fixed_asset_code_pair_count, 3);
    assert_eq!(result.font.used_runtime_extension_asset_code_pair_count, 1);
    assert_eq!(result.token_codes[0].code, "0x3000");
    assert_eq!(result.token_codes[0].occurrence_count, 1);
    assert_eq!(result.token_codes[1].code, "0x3001");
    assert_eq!(result.token_codes[1].occurrence_count, 2);
}
