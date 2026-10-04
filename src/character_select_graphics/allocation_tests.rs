use super::allocation::{SHARED_ATLAS_OFFSET, plan_character_select_atlas};
use super::model::CharacterSelectFontRole;
use crate::test_support::temporary_directory;

#[test]
#[ignore = "requires assets/"]
fn heading_space_reserves_a_full_transparent_cell_on_the_heading_page() {
    let root = temporary_directory("heading-space");
    write_assets(&root, "가 나", "협력전");
    // Native ink at the page origin must not become the space glyph.
    let source = synthetic_source(&[(0, 0)]);
    let plan = plan_character_select_atlas(&source, &root).unwrap();
    let blank = plan
        .allocations
        .iter()
        .find(|glyph| glyph.character == ' ')
        .unwrap();
    assert_eq!(blank.font_role, CharacterSelectFontRole::SelectHeading);
    assert_eq!((blank.cell.width, blank.cell.height), (32, 32));
    assert_ne!((blank.cell.x, blank.cell.y), (0, 0));
    assert!(
        plan.allocations
            .iter()
            .all(|glyph| glyph.texture_page_index == blank.texture_page_index)
    );
    assert_eq!(plan.required_glyph_count, 3);
    assert!(plan.source_ink_cells_protected);
    assert!(plan.physical_cells_are_unique_and_non_overlapping);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn runtime_composed_mode_menu_keeps_record_only_consumers_out_of_complete_routes() {
    let root = temporary_directory("plan");
    write_assets(&root, "가나", "협력전");
    let source = synthetic_source(&[(0, 0)]);

    let plan = plan_character_select_atlas(&source, &root).unwrap();

    assert_eq!(plan.translation_unit_count, 2);
    assert_eq!(plan.translation_entry_count, 5);
    assert_eq!(plan.source_inventory_entry_count, 5);
    assert_eq!(plan.translated_source_ui_count, 5);
    assert!(plan.untranslated_source_ui_ids.is_empty());
    assert_eq!(plan.required_glyph_count, 2);
    assert_eq!(
        plan.allocations
            .iter()
            .filter(|glyph| glyph.font_role == CharacterSelectFontRole::SelectHeading)
            .count(),
        2
    );
    assert!(plan.allocations.iter().all(|glyph| {
        glyph.font_role == CharacterSelectFontRole::SelectHeading
            && glyph.cell.width == 32
            && glyph.cell.height == 32
    }));
    assert!(
        plan.allocations
            .iter()
            .filter(|glyph| glyph.font_role == CharacterSelectFontRole::SelectHeading)
            .all(|glyph| !(glyph.cell.x == 0 && glyph.cell.y == 0))
    );
    assert!(plan.source_ink_cells_protected);
    assert!(plan.physical_cells_are_unique_and_non_overlapping);
    assert_eq!(plan.runtime_composed_texture_entry_count, 4);
    assert_eq!(
        plan.route_census.unresolved_only_source_ui_ids,
        [
            "cooperative_compatibility_mode",
            "cooperative_mode_menu_heading",
            "cooperative_return_to_mode_menu",
            "cooperative_versus_mode",
        ]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn atlas_capacity_overflow_fails_instead_of_dropping_glyphs() {
    let root = temporary_directory("overflow");
    let too_many = (0..300)
        .map(|offset| char::from_u32('가' as u32 + offset).unwrap())
        .collect::<String>();
    write_assets(&root, &too_many, "협력전");
    let source = synthetic_source(&[]);

    let error = plan_character_select_atlas(&source, &root).unwrap_err();

    assert!(error.to_string().contains("source-blank atlas cells"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn fixed_texture_strips_do_not_consume_dynamic_atlas_cells() {
    let root = temporary_directory("fixed-strips");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"character_select_heading","korean_text":"가"},{"id":"protagonist_label","korean_text":"주역"}]}"#,
    )
    .unwrap();
    write_translate_inventory(&root);
    let source = synthetic_source(&[(0, 220)]);

    let plan = plan_character_select_atlas(&source, &root).unwrap();

    assert_eq!(plan.translation_entry_count, 2);
    assert_eq!(plan.dynamic_atlas_entry_count, 1);
    assert_eq!(plan.fixed_texture_strip_entry_count, 1);
    assert_eq!(plan.bound_fixed_texture_strip_count, 1);
    assert_eq!(plan.fixed_strips.len(), 1);
    assert_eq!(plan.required_glyph_count, 1);
    assert_eq!(plan.allocations[0].character, '가');
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn required_roster_set_fails_when_a_source_consumer_has_no_translation_asset() {
    let root = temporary_directory("incomplete-roster");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","required_fixed_strip_sets":["roster_names"],"units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"roster_name_batsu","korean_text":"바츠"}]}"#,
    )
    .unwrap();
    write_translate_inventory(&root);
    let source = synthetic_source(&[(512, 0)]);

    let error = plan_character_select_atlas(&source, &root).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("required roster-name source binding roster_name_hinata is missing")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn unbound_entries_do_not_consume_shared_atlas_cells() {
    let root = temporary_directory("one-tpage");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"cooperative_mode_menu_heading","korean_text":"협력전"},{"id":"character_select_heading","korean_text":"라마바"},{"id":"cooperative_compatibility_mode","korean_text":"궁합 진단 모드"},{"id":"cooperative_versus_mode","korean_text":"대전 모드"},{"id":"cooperative_return_to_mode_menu","korean_text":"모드 메뉴로 돌아가기"},{"id":"ready_label","korean_text":"준비"}]}"#,
    )
    .unwrap();
    write_translate_inventory(&root);
    let source = synthetic_source(&[]);

    let plan = plan_character_select_atlas(&source, &root).unwrap();

    let select_pages = plan
        .allocations
        .iter()
        .filter(|glyph| glyph.font_role == CharacterSelectFontRole::SelectHeading)
        .map(|glyph| glyph.texture_page_index)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(select_pages.len(), 1);
    assert!(
        plan.allocations
            .iter()
            .all(|glyph| glyph.font_role != CharacterSelectFontRole::ModeMenuHeading)
    );
    assert_eq!(plan.runtime_composed_texture_entry_count, 4);
    assert_eq!(plan.unresolved_route_occurrence_count, 5);
    assert_eq!(
        plan.route_census.unclassified_pending_occurrence_ids,
        ["unclassified:ready_label"]
    );
    assert!(
        plan.unresolved_route_translation_ids
            .contains(&"ready_label".to_string())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn unresolved_consumers_count_source_occurrences_not_shared_translations() {
    let root = temporary_directory("shared-unresolved-translation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"shared","korean_text":"선택"}]}"#,
    )
    .unwrap();
    let inventory = root.join("source-inventory");
    std::fs::create_dir_all(&inventory).unwrap();
    std::fs::write(
        inventory.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_source_inventory","complete":true,"units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        inventory.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_source_inventory_unit","entries":[{"id":"character_select_heading","translation_id":"shared","source_text":"SELECT","treatment":"translate"},{"id":"first","translation_id":"shared","source_text":"SELECT","treatment":"translate"},{"id":"second","translation_id":"shared","source_text":"SELECT","treatment":"translate"}]}"#,
    )
    .unwrap();

    let plan = plan_character_select_atlas(&synthetic_source(&[]), &root).unwrap();

    assert_eq!(plan.translated_source_ui_count, 3);
    assert_eq!(
        plan.fully_routed_source_ui_ids,
        ["character_select_heading"]
    );
    assert_eq!(plan.unresolved_route_occurrence_count, 2);
    assert_eq!(plan.unresolved_route_source_ui_ids, ["first", "second"]);
    assert_eq!(plan.unresolved_route_translation_ids, ["shared"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn translations_without_consumer_bindings_remain_unresolved() {
    let root = temporary_directory("unbound-label");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{"id":"character_select_heading","korean_text":"선택"},{"id":"label","korean_text":"가"}]}"#,
    )
    .unwrap();
    write_translate_inventory(&root);
    let source = synthetic_source(&[]);

    let plan = plan_character_select_atlas(&source, &root).unwrap();

    assert_eq!(plan.unresolved_route_source_ui_ids, ["label"]);
    std::fs::remove_dir_all(root).unwrap();
}

fn write_assets(root: &std::path::Path, heading: &str, label: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"kind":"justice_gakuen2_character_select_translation_manifest","source_inventory":"source-inventory/manifest.json","units":["heading.json","label.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("heading.json"),
        format!(
            r#"{{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{{"id":"character_select_heading","korean_text":{heading:?}}}]}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("label.json"),
        format!(
            r#"{{"kind":"justice_gakuen2_character_select_translation_unit","entries":[{{"id":"cooperative_mode_menu_heading","korean_text":{label:?}}},{{"id":"cooperative_compatibility_mode","korean_text":"궁합 진단 모드"}},{{"id":"cooperative_versus_mode","korean_text":"대전 모드"}},{{"id":"cooperative_return_to_mode_menu","korean_text":"모드 메뉴로 돌아가기"}}]}}"#
        ),
    )
    .unwrap();
    write_translate_inventory(root);
}

fn write_translate_inventory(root: &std::path::Path) {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let mut inventory_entries = Vec::new();
    for unit_path in manifest["units"].as_array().unwrap() {
        let unit: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(unit_path.as_str().unwrap())).unwrap())
                .unwrap();
        for entry in unit["entries"].as_array().unwrap() {
            let id = entry["id"].as_str().unwrap();
            let source_text = match id {
                "protagonist_label" => "主役",
                "roster_name_batsu" => "バツ",
                "cooperative_mode_menu_heading" => "協力戦",
                "cooperative_compatibility_mode" => "相性診断モード",
                "cooperative_versus_mode" => "対戦モード",
                "cooperative_return_to_mode_menu" => "モードメニューに戻る",
                _ => "source",
            };
            inventory_entries.push(serde_json::json!({
                "id": id,
                "source_text": source_text,
                "treatment": "translate"
            }));
        }
    }
    let inventory_directory = root.join("source-inventory");
    std::fs::create_dir_all(&inventory_directory).unwrap();
    std::fs::write(
        inventory_directory.join("manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "kind": "justice_gakuen2_character_select_source_inventory",
            "complete": true,
            "units": ["unit.json"]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        inventory_directory.join("unit.json"),
        serde_json::to_vec(&serde_json::json!({
            "kind": "justice_gakuen2_character_select_source_inventory_unit",
            "entries": inventory_entries
        }))
        .unwrap(),
    )
    .unwrap();
}

fn synthetic_source(ink_pixels: &[(usize, usize)]) -> Vec<u8> {
    let shared_tim = synthetic_tim(1024, 256, ink_pixels);
    let mut source = vec![0; SHARED_ATLAS_OFFSET + shared_tim.len()];
    source[SHARED_ATLAS_OFFSET..SHARED_ATLAS_OFFSET + shared_tim.len()]
        .copy_from_slice(&shared_tim);
    source
}

fn synthetic_tim(width: usize, height: usize, ink_pixels: &[(usize, usize)]) -> Vec<u8> {
    let clut_size = 12 + 16 * 2;
    let image_byte_count = width * height / 2;
    let image_size = 12 + image_byte_count;
    let mut tim = Vec::with_capacity(8 + clut_size + image_size);
    tim.extend_from_slice(&0x10u32.to_le_bytes());
    tim.extend_from_slice(&0x08u32.to_le_bytes());
    tim.extend_from_slice(&(clut_size as u32).to_le_bytes());
    tim.extend_from_slice(&0u16.to_le_bytes());
    tim.extend_from_slice(&0u16.to_le_bytes());
    tim.extend_from_slice(&16u16.to_le_bytes());
    tim.extend_from_slice(&1u16.to_le_bytes());
    tim.extend_from_slice(&[0u8; 32]);
    tim.extend_from_slice(&(image_size as u32).to_le_bytes());
    tim.extend_from_slice(&0u16.to_le_bytes());
    tim.extend_from_slice(&0u16.to_le_bytes());
    tim.extend_from_slice(&((width / 4) as u16).to_le_bytes());
    tim.extend_from_slice(&(height as u16).to_le_bytes());
    tim.extend_from_slice(&vec![0u8; image_byte_count]);
    let image_offset = 8 + clut_size + 12;
    for &(x, y) in ink_pixels {
        let pixel_offset = y * width + x;
        tim[image_offset + pixel_offset / 2] |= if pixel_offset.is_multiple_of(2) {
            1
        } else {
            0x10
        };
    }
    tim
}
