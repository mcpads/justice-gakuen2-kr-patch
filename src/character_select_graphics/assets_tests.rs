use super::assets::load_character_select_translation_assets;
use crate::test_support::temporary_directory;

#[test]
fn sharded_assets_reject_duplicate_semantic_ids() {
    let root = temporary_directory("character-select-assets");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["first.json","second.json"]}"#,
    )
    .unwrap();
    let unit = r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"same","korean_text":"번역"}]}"#;
    std::fs::write(root.join("first.json"), unit).unwrap();
    std::fs::write(root.join("second.json"), unit).unwrap();

    let error = load_character_select_translation_assets(&root).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("duplicate character-select translation id")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn translation_assets_hash_includes_unit_contents() {
    let root = temporary_directory("character-select-assets");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(root.join("unit.json"), unit_with_translation("번역")).unwrap();
    write_inventory(
        &root,
        serde_json::json!([
            {"id": "entry", "source_text": "source", "treatment": "translate"}
        ]),
    );
    let before = load_character_select_translation_assets(&root)
        .unwrap()
        .assets_sha256;

    std::fs::write(root.join("unit.json"), unit_with_translation("다른 번역")).unwrap();
    let after = load_character_select_translation_assets(&root)
        .unwrap()
        .assets_sha256;

    assert_ne!(before, after);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_inventory_keeps_untranslated_source_ui_in_the_denominator() {
    let root = temporary_directory("character-select-source-inventory");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(root.join("unit.json"), unit_with_translation("번역")).unwrap();
    write_inventory(
        &root,
        serde_json::json!([
            {"id": "entry", "source_text": "source", "treatment": "translate"},
            {"id": "still_untranslated", "source_text": "未翻訳", "treatment": "translate"}
        ]),
    );

    let assets = load_character_select_translation_assets(&root).unwrap();

    assert_eq!(assets.entries.len(), 1);
    assert_eq!(assets.source_inventory_entries.len(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn one_translation_covers_multiple_source_occurrences() {
    let root = temporary_directory("character-select-shared-translation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(root.join("unit.json"), unit_with_translation("번역")).unwrap();
    write_inventory(
        &root,
        serde_json::json!([
            {"id": "first_occurrence", "translation_id": "entry", "source_text": "source", "treatment": "translate"},
            {"id": "second_occurrence", "translation_id": "entry", "source_text": "source", "treatment": "translate"}
        ]),
    );

    let assets = load_character_select_translation_assets(&root).unwrap();
    let coverage = assets.source_translation_coverage();

    assert_eq!(assets.entries.len(), 1);
    assert_eq!(assets.source_inventory_entries.len(), 2);
    assert_eq!(coverage.translated_source_ui_count, 2);
    assert!(coverage.untranslated_source_ui_ids.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

fn unit_with_translation(korean_text: &str) -> String {
    format!(
        r#"{{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{{"id":"entry","korean_text":"{korean_text}"}}]}}"#
    )
}

fn write_inventory(root: &std::path::Path, entries: serde_json::Value) {
    let directory = root.join("source-inventory");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_source_inventory","complete":true,"units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("unit.json"),
        serde_json::to_vec(&serde_json::json!({
            "kind": "justice_gakuen2_character_select_source_inventory_unit",
            "entries": entries
        }))
        .unwrap(),
    )
    .unwrap();
}
