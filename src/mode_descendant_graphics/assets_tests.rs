use super::assets::load_mode_descendant_assets;
use crate::test_support::temporary_directory;

#[test]
fn sharded_units_reject_overlapping_cells_in_one_source_record() {
    let root = temporary_directory("mode-descendant-assets");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        br#"{"kind":"justice_gakuen2_mode_descendant_manifest","edit_command_sheets":"edit-command-sheets.json","edit_technique_names":"technique-names.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{
          "kind":"justice_gakuen2_mode_descendant_unit",
          "entries":[
            {"id":"first","surface":"gorin_main_menu","source_text":"a","korean_text":"가","font_role":"gorin_menu","placement":{"kind":"fixed","bits_per_pixel":4,"tim_offset":"0x0","cell":{"x":0,"y":0,"width":20,"height":20},"alignment":"left","source_region_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}},
            {"id":"second","surface":"gorin_main_menu","source_text":"b","korean_text":"나","font_role":"gorin_menu","placement":{"kind":"fixed","bits_per_pixel":4,"tim_offset":"0x0","cell":{"x":10,"y":0,"width":20,"height":20},"alignment":"left","source_region_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}
          ]
        }"#,
    )
    .unwrap();

    let error = load_mode_descendant_assets(&root).unwrap_err();
    assert!(error.to_string().contains("first and second overlap"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn identical_coordinates_are_independent_when_their_tim_textures_differ() {
    let root = temporary_directory("mode-descendant-texture-ownership");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        br#"{"kind":"justice_gakuen2_mode_descendant_manifest","edit_command_sheets":"edit-command-sheets.json","edit_technique_names":"technique-names.json","units":["unit.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("unit.json"),
        r#"{
          "kind":"justice_gakuen2_mode_descendant_unit",
          "entries":[
            {"id":"first","surface":"gorin_main_menu","source_text":"a","korean_text":"가","font_role":"gorin_menu","placement":{"kind":"fixed","bits_per_pixel":4,"tim_offset":"0x0","cell":{"x":0,"y":0,"width":20,"height":20},"alignment":"left","source_region_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}},
            {"id":"second","surface":"gorin_main_menu","source_text":"b","korean_text":"나","font_role":"gorin_heading","placement":{"kind":"fixed","bits_per_pixel":8,"tim_offset":"0x18800","cell":{"x":0,"y":0,"width":20,"height":20},"alignment":"center","source_region_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}
          ]
        }"#,
    )
    .unwrap();

    let assets = load_mode_descendant_assets(&root).unwrap();
    assert_eq!(assets.entries.len(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn physical_consumers_reject_empty_duplicate_and_foreign_records() {
    let root = temporary_directory("mode-descendant-physical-consumers");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("manifest.json"), br#"{"kind":"justice_gakuen2_mode_descendant_manifest","edit_command_sheets":"commands.json","edit_technique_names":"techniques.json","units":["unit.json"]}"#).unwrap();
    let mut entry = serde_json::json!({
        "id":"selector", "surface":"gorin_main_menu", "source_text":"決定",
        "korean_text":"결정", "font_role":"gorin_menu",
        "placement":{"kind":"fixed","bits_per_pixel":4,"tim_offset":"0x91800",
            "cell":{"x":0,"y":200,"width":40,"height":20},"alignment":"center",
            "source_region_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}
    });
    for (records, valid) in [
        (serde_json::json!(["gorin_title9", "gorin_title10"]), true),
        (serde_json::json!([]), false),
        (serde_json::json!(["gorin_title9", "gorin_title9"]), false),
        (serde_json::json!(["edit_shared_ui"]), false),
    ] {
        entry["records"] = records;
        std::fs::write(
            root.join("unit.json"),
            serde_json::to_vec(&serde_json::json!({
                "kind":"justice_gakuen2_mode_descendant_unit", "entries":[entry.clone()]
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(load_mode_descendant_assets(&root).is_ok(), valid);
    }
    std::fs::remove_dir_all(root).unwrap();
}
