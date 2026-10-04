use std::path::Path;

use serde_json::json;

use crate::pipeline::sha256_bytes;
use crate::test_support::temporary_directory;

use super::load_development_build_spec;

#[test]
fn valid_spec_resolves_relative_paths_and_role_specific_font_sources() {
    let root = temporary_directory("surface-fonts");
    write_fixture(&root, b"bold-font", b"light-font");
    let mut document = build_spec_document(sha256_bytes(b"bold-font"), sha256_bytes(b"light-font"));
    document["fonts"]["diary_header"]["action_label"]["source"] = json!("light");
    std::fs::write(
        root.join("build.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();

    let loaded = load_development_build_spec(&root.join("build.json")).unwrap();

    assert_eq!(
        loaded.specifications.mode_select_descendants,
        root.join("specs/surfaces/mode-select/manifest.json")
    );
    assert_eq!(loaded.assets.diary_header, root.join("assets/diary/header"));
    assert_eq!(
        loaded.fonts.diary_header.action_label.path,
        root.join("fonts/light.ttf")
    );
    assert_eq!(
        loaded.fonts.diary_header.status_label.path,
        root.join("fonts/bold.ttf")
    );
    assert_eq!(
        loaded.fonts.mode_select.description,
        root.join("fonts/light.ttf")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_font_bytes_are_rejected_before_any_surface_build() {
    let root = temporary_directory("bad-hash");
    write_fixture(&root, b"changed-bold-font", b"light-font");
    write_spec(
        &root,
        sha256_bytes(b"bold-font"),
        sha256_bytes(b"light-font"),
    );

    let error = load_development_build_spec(&root.join("build.json")).unwrap_err();

    assert!(error.to_string().contains("bold hash mismatch"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unknown_role_font_source_is_rejected() {
    let root = temporary_directory("unknown-source");
    write_fixture(&root, b"bold-font", b"light-font");
    let mut document = build_spec_document(sha256_bytes(b"bold-font"), sha256_bytes(b"light-font"));
    document["fonts"]["mode_select"]["description"]["source"] = json!("missing");
    std::fs::write(
        root.join("build.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();

    let error = load_development_build_spec(&root.join("build.json")).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("mode_select.description references unknown font source")
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn write_fixture(root: &Path, bold: &[u8], light: &[u8]) {
    for directory in [
        "assets/dialogue/translations",
        "assets/dialogue/name-entry",
        "assets/dialogue/name-entry-graphics",
        "assets/diary/header",
        "assets/diary/scenes",
        "assets/menu/bonus-main",
        "assets/menu/bonus-inventory",
        "assets/menu/bonus-inventory/dynamic",
        "assets/menu/bonus-inventory/dynamic/action-labels",
        "assets/menu/bonus-inventory/dynamic/card-acquisition",
        "assets/menu/bonus-inventory/dynamic/confirmation",
        "assets/menu/bonus-inventory/dynamic/j-bank-return-label",
        "assets/menu/bonus-inventory/dynamic/memory-card-swap",
        "assets/menu/bonus-inventory/dynamic/stock-label",
        "assets/menu/shop-ui",
        "assets/menu/shop-ui/dynamic/exit-confirmation",
        "assets/menu/shop-text",
        "assets/menu/mode-select",
        "assets/menu/character-select",
        "assets/menu/mode-descendants",
        "assets/menu/practical-instructions",
        "assets/menu/options",
        "assets/menu/title-adjacent",
        "assets/menu/title-notice",
        "specs/surfaces/mode-select",
        "fonts",
    ] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join("assets/dialogue/codebook.json"), b"{}").unwrap();
    std::fs::write(
        root.join("assets/menu/bonus-inventory/dynamic/page-indicator.json"),
        b"{}",
    )
    .unwrap();
    std::fs::write(root.join("specs/surfaces/mode-select/manifest.json"), b"{}").unwrap();
    std::fs::write(root.join("fonts/bold.ttf"), bold).unwrap();
    std::fs::write(root.join("fonts/light.ttf"), light).unwrap();
}

fn write_spec(root: &Path, bold_sha256: String, light_sha256: String) {
    std::fs::write(
        root.join("build.json"),
        serde_json::to_vec_pretty(&build_spec_document(bold_sha256, light_sha256)).unwrap(),
    )
    .unwrap();
}

fn build_spec_document(bold_sha256: String, light_sha256: String) -> serde_json::Value {
    let mut document = json!({
        "kind": "justice_gakuen2_development_build_spec",
        "assets": {
            "bonus_menu": "assets/menu/bonus-main",
            "bonus_inventory": "assets/menu/bonus-inventory",
            "bonus_inventory_action_labels": "assets/menu/bonus-inventory/dynamic/action-labels",
            "bonus_confirmation": "assets/menu/bonus-inventory/dynamic/confirmation",
            "bonus_inventory_card_acquisition": "assets/menu/bonus-inventory/dynamic/card-acquisition",
            "bonus_inventory_memory_card_swap": "assets/menu/bonus-inventory/dynamic/memory-card-swap",
            "bonus_j_bank_return_label": "assets/menu/bonus-inventory/dynamic/j-bank-return-label",
            "bonus_inventory_stock_label": "assets/menu/bonus-inventory/dynamic/stock-label",
            "bonus_page_indicator": "assets/menu/bonus-inventory/dynamic/page-indicator.json",
            "bonus_shop_exit_confirmation": "assets/menu/shop-ui/dynamic/exit-confirmation",
            "bonus_shop_text": "assets/menu/shop-text",
            "shop_ui": "assets/menu/shop-ui",
            "dialogue_codebook": "assets/dialogue/codebook.json",
            "dialogue_translations": "assets/dialogue/translations",
            "dialogue_selector_translations": "assets/dialogue/translations",
            "name_entry_candidates": "assets/dialogue/name-entry",
            "name_entry_graphics": "assets/dialogue/name-entry-graphics",
            "diary_header": "assets/diary/header",
            "diary_scenes": "assets/diary/scenes",
            "mode_select": "assets/menu/mode-select",
            "character_select": "assets/menu/character-select",
            "mode_descendants": "assets/menu/mode-descendants",
            "practical_instructions": "assets/menu/practical-instructions",
            "options": "assets/menu/options",
            "title_menu": "assets/menu/title-adjacent",
            "title_notice": "assets/menu/title-notice"
        },
        "font_sources": {
            "bold": {"path": "fonts/bold.ttf", "sha256": bold_sha256},
            "light": {"path": "fonts/light.ttf", "sha256": light_sha256}
        },
        "fonts": {
            "bonus_menu": {
                "heading": {"source": "bold", "font_px": 32.0},
                "entry": {"source": "bold", "font_px": 20.0},
                "compact_entry": {"source": "bold", "font_px": 20.0}
            },
            "bonus_inventory": {
                "device_text": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
                "item_label": {"source": "bold", "font_px": 12.0, "tracking_px": 0.0, "vertical_shift_px": 0},
                "title": {"source": "bold", "font_px": 38.0, "tracking_px": 8.0, "vertical_shift_px": 0},
                "compact_label": {"source": "bold", "font_px": 15.0, "tracking_px": 0.0, "vertical_shift_px": 0},
                "large_label": {"source": "bold", "font_px": 22.0, "tracking_px": 0.0, "vertical_shift_px": 0},
                "compact_action": {"source": "bold", "font_px": 16.0, "tracking_px": 0.0, "vertical_shift_px": 0},
                "large_action": {"source": "bold", "font_px": 22.0, "tracking_px": 0.0, "vertical_shift_px": 0},
                "help": {"source": "bold", "font_px": 12.0, "tracking_px": 0.0, "vertical_shift_px": -3},
                "viewer_navigation": {"source": "bold", "font_px": 12.0, "tracking_px": 2.0, "vertical_shift_px": -3},
                "viewer_card_placeholder": {"source": "bold", "font_px": 22.0, "tracking_px": 0.0, "vertical_shift_px": 0}
            },
            "bonus_inventory_action_labels": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_confirmation": {"source": "bold", "font_px": 15.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_inventory_card_acquisition": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_inventory_memory_card_swap": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_j_bank_return_label": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_inventory_stock_label": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_page_indicator": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_shop_exit_confirmation": {"source": "bold", "font_px": 15.0, "tracking_px": 0.0, "vertical_shift_px": -1},
            "bonus_shop_text": {
                "product_description": {"source": "bold", "font_px": 15.0, "tracking_px": 0.0, "vertical_shift_px": -1},
                "product_label": {"source": "bold", "font_px": 17.0, "tracking_px": 0.0, "vertical_shift_px": -1},
                "clerk_dialogue": {"source": "bold", "font_px": 15.0, "tracking_px": 0.0, "vertical_shift_px": -1}
            },
            "shop_ui": {
                "heading": {"source": "bold", "font_px": 38.0},
                "current_points": {"source": "bold", "font_px": 20.0},
                "heading_tracking_px": 15.0,
                "current_points_tracking_px": 0.0
            },
            "dialogue_body": {"source": "bold", "font_px": 14.75},
            "name_entry": {
                "candidates": {"source": "light", "font_px": 14.0},
                "keyboard_keys": {"source": "light", "font_px": 13.0},
                "composed_glyphs": {"source": "light", "font_px": 13.0},
                "roster_glyphs": {"source": "light", "font_px": 12.0},
                "fixed_graphics": {"source": "light"}
            },
            "diary_header": {
                "calendar_text": {
                    "source": "bold", "font_px": 16.0,
                    "rendering": {
                        "mode": "coverage_ramp",
                        "first_ink_index": 1, "last_ink_index": 15
                    },
                    "glyph_layout": {
                        "slot_width_px": 16, "vertical_shift_px": 0
                    }
                },
                "status_label": {
                    "source": "bold", "font_px": 14.0,
                    "rendering": {
                        "mode": "outlined",
                        "outline_index": 5, "fill_index": 13
                    },
                    "glyph_layout": {
                        "slot_width_px": 16, "vertical_shift_px": -6
                    }
                },
                "club_label": {
                    "source": "bold", "font_px": 14.0,
                    "rendering": {
                        "mode": "outlined",
                        "outline_index": 5, "fill_index": 13
                    },
                    "glyph_layout": {
                        "slot_width_px": 16, "vertical_shift_px": -6
                    }
                },
                "action_label": {
                    "source": "bold", "font_px": 19.0,
                    "rendering": {
                        "mode": "coverage_ramp",
                        "first_ink_index": 1, "last_ink_index": 15
                    },
                    "glyph_layout": {
                        "slot_width_px": 24, "vertical_shift_px": 0
                    }
                }
            },
            "diary_scene": {
                "location_label": {"source": "bold", "font_px": 18.0},
                "movement_map_label": {"source": "bold", "font_px": 12.0},
                "exam_heading": {"source": "bold", "font_px": 17.0},
                "exam_stamp": {"source": "bold", "font_px": 82.0},
                "exam_evaluation": {"source": "bold", "font_px": 22.0},
                "exam_finished": {"source": "bold", "font_px": 36.0},
                "training_list_label": {"source": "bold", "font_px": 18.0},
                "training_status_axis": {"source": "bold", "font_px": 9.0},
                "cooperative_selection": {"source": "bold", "font_px": 16.0}
            },
            "mode_select": {
                "artwork": {"source": "bold"},
                "fixed_label": {"source": "bold"},
                "list_label": {"source": "bold"},
                "detail_title": {"source": "bold"},
                "description": {"source": "light"}
            },
            "character_select": {
                "select_heading": {"source": "bold", "font_px": 26.0},
                "mode_menu_heading": {"source": "bold", "font_px": 26.0},
                "mode_menu_label": {"source": "bold", "font_px": 22.0},
                "cooperative_emblem_character": {"source": "bold", "font_px": 30.0},
                "tournament_bracket_label": {"source": "bold", "font_px": 24.0},
                "tournament_certificate_title": {"source": "bold", "font_px": 30.0},
                "tournament_certificate_label": {"source": "bold", "font_px": 24.0},
                "tournament_certificate_body": {"source": "light", "font_px": 16.0},
                "label": {"source": "bold", "font_px": 16.0},
                "roster_name": {"source": "bold", "font_px": 12.0},
                "fixed_prompt": {"source": "bold", "font_px": 14.0},
                "compact_prompt": {"source": "bold", "font_px": 12.0},
                "solo_state_prompt": {"source": "bold", "font_px": 12.0},
                "practical_selection_label": {"source": "bold", "font_px": 14.0},
                "league_standing_label": {"source": "bold", "font_px": 12.0},
                "selection_help": {"source": "bold", "font_px": 12.0}
            },
            "mode_descendants": {
                "gorin_menu": {"source": "bold", "font_px": 18.0},
                "edit_heading": {"source": "bold", "font_px": 24.0},
                "edit_label": {"source": "bold", "font_px": 18.0},
                "edit_runtime_text": {"source": "bold", "font_px": 14.0, "vertical_shift_px": -1},
                "practical_title": {"source": "bold", "font_px": 23.0, "vertical_shift_px": -1},
                "practical_menu_label": {"source": "bold", "font_px": 17.0, "vertical_shift_px": -1},
                "practical_prompt": {"source": "bold", "font_px": 16.0, "vertical_shift_px": -1},
                "practical_hint": {"source": "bold", "font_px": 14.0, "vertical_shift_px": -1},
                "practical_result_heading": {"source": "bold", "font_px": 24.0, "vertical_shift_px": -1},
                "practical_result_label": {"source": "bold", "font_px": 16.0, "vertical_shift_px": -1},
                "practical_result_hint": {"source": "bold", "font_px": 13.0, "vertical_shift_px": -1},
                "practical_result_action": {"source": "bold", "font_px": 16.0, "vertical_shift_px": -1},
                "practical_result_small_judgment": {"source": "bold", "font_px": 24.0, "vertical_shift_px": 0},
                "practical_result_branding": {"source": "bold", "font_px": 20.0, "vertical_shift_px": 0}
            },
            "practical_instruction": {"source": "bold", "font_px": 13.0, "vertical_shift_px": -2},
            "options": {
                "heading": {"source": "bold", "font_px": 24.0},
                "help": {"source": "light", "font_px": 13.0},
                "label": {"source": "bold", "font_px": 14.0},
                "value": {"source": "bold", "font_px": 14.0},
                "action": {"source": "bold", "font_px": 14.0},
                "records_main": {
                    "heading": {"source": "bold", "font_px": 32.0},
                    "item": {"source": "bold", "font_px": 15.0}
                },
                "records_prompt": {"source": "bold", "font_px": 14.0},
                "records_status": {
                    "heading": {"source": "bold", "font_px": 13.0},
                    "message": {"source": "bold", "font_px": 12.0}
                },
                "background": {
                    "school_name": {"source": "bold", "font_px": 14.0},
                    "crest_mark": {"source": "bold", "font_px": 30.0}
                },
                "description": {"source": "bold", "font_px": 16.0}
            },
            "title_menu": {"source": "bold", "font_px": 17.0},
            "title_notice": {"source": "bold", "font_px": 14.0}
        },
        "runtime_glyph_layouts": {
            "nickname_hud": {
                "scale_percent": 115,
                "vertical_shift_px": -1
            }
        }
    });
    document["fonts"]["mode_descendants"]["battle_counter"] =
        json!({"source": "bold", "font_px": 16.0, "vertical_shift_px": 0});
    document["fonts"]["mode_descendants"]["training_text"] =
        json!({"source": "bold", "font_px": 8.0, "vertical_shift_px": 0});
    document["fonts"]["shared_menu_numerals"] = json!({"source": "bold", "font_px": 16.0});
    document["fonts"]["character_select"]["system_settings"] =
        json!({"source":"light","font_px":16.0});
    document["fonts"]["character_select"]["cooperative_diagnosis"] =
        json!({"source": "light", "font_px": 16.0});
    document["fonts"]["character_select"]["common_pause_menu"] =
        json!({"source": "bold", "font_px": 12.0});
    document["fonts"]["mode_descendants"]["gorin_heading"] =
        json!({"source": "bold", "font_px": 64.0});
    document["fonts"]["mode_descendants"]["edit_school_label"] =
        json!({"source": "bold", "font_px": 14.0});
    document["fonts"]["mode_descendants"]["edit_team_up_name"] =
        json!({"source": "bold", "font_px": 16.0});
    document["fonts"]["mode_descendants"]["edit_compact_label"] =
        json!({"source": "bold", "font_px": 14.0});
    document["fonts"]["mode_descendants"]["practical_result_judgment_stamp"] =
        json!({"source": "bold", "font_px": 190.0, "vertical_shift_px": 0});
    document["specifications"] = json!({
        "mode_select_descendants": "specs/surfaces/mode-select/manifest.json"
    });
    document["fonts"]["character_select"]["solo_story_intro"] =
        json!({"source": "bold", "font_px": 13.0});
    document["fonts"]["character_select"]["solo_episode_card"] =
        json!({"source": "bold", "font_px": 18.0});
    document["fonts"]["character_select"]["stage_label"] =
        json!({"source": "bold", "font_px": 14.0});
    document
}
