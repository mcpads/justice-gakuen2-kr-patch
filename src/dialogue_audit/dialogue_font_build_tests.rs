use std::path::PathBuf;

use super::dialogue_fixed_code_consumers_model::DialogueFixedCodeConsumerAssetAudit;
use super::dialogue_font_build::{classify_font_build_assets, prepare_output_directory};
use crate::test_support::temporary_directory;

#[test]
fn authored_subset_installs_only_assets_that_preserve_untranslated_source_glyphs() {
    let assets = [asset("DAT2/SAFE.BIZ", 0), asset("DAT2/BLOCKED.BIZ", 7)];

    let (installable, skipped) = classify_font_build_assets(&assets);

    assert_eq!(
        installable.into_iter().collect::<Vec<_>>(),
        ["DAT2/SAFE.BIZ"]
    );
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].source_path, "DAT2/BLOCKED.BIZ");
    assert!(skipped[0].reason.starts_with('7'));
}

#[test]
fn forced_font_build_removes_obsolete_materialized_message_images() {
    let output_dir = test_output_dir("remove-message-images");
    let message_output_dir = output_dir.join("message-images");
    std::fs::create_dir_all(&message_output_dir).unwrap();
    std::fs::write(message_output_dir.join("legacy.rebuilt.biz"), b"obsolete").unwrap();
    std::fs::write(output_dir.join("owned-current-output.bin"), b"preserve").unwrap();

    prepare_output_directory(&output_dir, true).unwrap();

    assert!(!message_output_dir.exists());
    assert_eq!(
        std::fs::read(output_dir.join("owned-current-output.bin")).unwrap(),
        b"preserve"
    );
    std::fs::remove_dir_all(output_dir).unwrap();
}

fn asset(source_path: &str, protected_overwrites: usize) -> DialogueFixedCodeConsumerAssetAudit {
    DialogueFixedCodeConsumerAssetAudit {
        source_path: source_path.to_string(),
        stored_coordinate_count: 1,
        primary_referenced_coordinate_count: 1,
        primary_referenced_coordinate_rewrite_count: 1,
        authored_coordinate_rewrite_count: 1,
        selector_authored_coordinate_rewrite_count: 0,
        runtime_insertion_coordinate_rewrite_count: 0,
        preserved_primary_untranslated_coordinate_count: usize::from(protected_overwrites > 0),
        preserved_primary_unreferenced_coordinate_count: 0,
        preserved_selector_coordinate_count: 0,
        protected_source_glyph_code_count: protected_overwrites,
        protected_source_glyph_overwrite_count: protected_overwrites,
        preserved_source_glyph_protection_complete: protected_overwrites == 0,
        decimal_control_coordinate_count: 0,
        runtime_decimal_codes_preserve_source_glyphs: true,
    }
}

fn test_output_dir(label: &str) -> PathBuf {
    temporary_directory(&format!("dialogue-font-{label}"))
}

#[test]
fn reused_unicode_punctuation_is_restyled_but_blank_cells_are_preserved() {
    use super::dialogue_code_allocation_model::DialogueCharacterCodeAssignment;
    use super::dialogue_font_build::render_dialogue_assignment;
    for character in "!?…「」＝♥‘’“”―～・（）♪×○ーⅢ".chars() {
        let mut assignment = DialogueCharacterCodeAssignment {
            character: character.to_string(),
            code: "0x0001".into(),
            source_glyph_reused: true,
            global_name_glyph_reused: false,
            requires_glyph_install: false,
            target_was_fixed_source_cell: true,
            target_source_glyph_protected: false,
            target_source_cell_sha256: None,
        };
        assert!(render_dialogue_assignment(&assignment), "{character}");
        assignment.character = " ".into();
        assert!(!render_dialogue_assignment(&assignment));
    }
}
