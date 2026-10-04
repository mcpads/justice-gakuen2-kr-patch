use std::path::Path;

use serde_json::{Value, json};

use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::mode_select::source::PANEL_INDEX_BY_MODE_INDEX;
use crate::source_disc::{
    MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE,
    profile::{
        BONUS_MENU_OVERLAY_RECORD, BONUS_MENU_RECORD, CHARACTER_SELECT_COOPERATIVE_MENU_RECORD,
        CHARACTER_SELECT_OVERLAY_RECORDS, CHARACTER_SELECT_TEXTURE_RECORDS,
        EDIT_REGISTRATION_OVERLAY_RECORD, EDIT_REGISTRATION_UI_RECORD, GORIN_MAIN_MENU_RECORD,
        GORIN_SELECTION_OVERLAY_RECORD, MAIN_EXECUTABLE_RECORD, MENU_RECORD,
        MODE_SELECT_OVERLAY_RECORD, OPTIONS_INFO_RECORD, OPTIONS_OVERLAY_RECORD,
        TITLE_MENU_OVERLAY_RECORD,
    },
};
use crate::test_support::temporary_directory;

use super::load_surface_inventory;
use super::mode_select_binding::validate_mode_select_root_population;
use super::mode_select_character_select_routes::validate_mode_select_character_select_routes;
use super::mode_select_direct_entry_routes::{
    ModeSelectDirectEntrySources, validate_mode_select_direct_entry_routes,
};
use super::mode_select_dispatcher::validate_mode_select_dispatcher;
use super::mode_select_options_records::validate_mode_select_options_and_records;
use super::mode_select_practical_entry::validate_mode_select_practical_entry;
use super::model::ModeSelectPanelSelection;

const SOURCE_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn adopted_root_population_must_match_the_built_mode_assets() {
    let root = temporary_directory("surface-inventory-root-population");
    write_inventory(&root, valid_shard());
    let inventory = load_surface_inventory(&root.join("manifest.json")).unwrap();

    assert_eq!(
        validate_mode_select_root_population(&inventory, &["solo"]).unwrap(),
        1
    );

    let error = validate_mode_select_root_population(&inventory, &["solo", "versus"]).unwrap_err();
    assert!(error.to_string().contains("root population differs"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the original disc in roms/ and specs/"]
fn adopted_dispatcher_rejects_translation_order_and_source_table_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory =
        load_surface_inventory(&root.join("specs/surfaces/mode-select/manifest.json")).unwrap();
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, overlay) =
        rebuild::read_record(&cue.image_path, MODE_SELECT_OVERLAY_RECORD.path).unwrap();
    let (_, mut main_executable) =
        rebuild::read_record(&cue.image_path, MAIN_EXECUTABLE_RECORD.path).unwrap();
    let asset_ids = [
        "diary",
        "solo",
        "versus",
        "league",
        "team",
        "tournament",
        "cooperative",
        "training",
        "practical_exam_99",
        "gorin_festival",
        "edit_registration",
        "bonus",
        "records",
        "options",
    ];
    let selections = asset_ids
        .iter()
        .enumerate()
        .map(|(mode_index, asset_id)| ModeSelectPanelSelection {
            panel_index: PANEL_INDEX_BY_MODE_INDEX[mode_index],
            asset_id,
        })
        .collect::<Vec<_>>();

    let report =
        validate_mode_select_dispatcher(&inventory, &overlay, &main_executable, &selections)
            .unwrap();
    assert_eq!(report.entry_count, selections.len());
    assert!(report.writer_sequence_matches && report.dispatcher_sequence_matches);

    let translation_order = selections
        .iter()
        .enumerate()
        .map(|(panel_index, selection)| ModeSelectPanelSelection {
            panel_index,
            asset_id: selection.asset_id,
        })
        .collect::<Vec<_>>();
    let error =
        validate_mode_select_dispatcher(&inventory, &overlay, &main_executable, &translation_order)
            .unwrap_err();
    assert!(
        error.to_string().contains("built panel identity"),
        "{error:#}"
    );

    let table_file_offset =
        PSX_EXE_HEADER_SIZE + usize::try_from(0x8008_4358_u32 - MAIN_TEXT_RUNTIME_BASE).unwrap();
    main_executable[table_file_offset..table_file_offset + 4]
        .copy_from_slice(&0x8001_7090_u32.to_le_bytes());
    let error =
        validate_mode_select_dispatcher(&inventory, &overlay, &main_executable, &selections)
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("dispatcher source record changed"),
        "{error:#}"
    );
}

#[test]
#[ignore = "requires the original disc in roms/ and specs/"]
fn resolved_options_and_records_reject_source_or_consumer_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut inventory =
        load_surface_inventory(&root.join("specs/surfaces/mode-select/manifest.json")).unwrap();
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, main_executable) =
        rebuild::read_record(&cue.image_path, MAIN_EXECUTABLE_RECORD.path).unwrap();
    let (_, options_overlay) =
        rebuild::read_record(&cue.image_path, OPTIONS_OVERLAY_RECORD.path).unwrap();
    let (_, options_information) =
        rebuild::read_record(&cue.image_path, OPTIONS_INFO_RECORD.path).unwrap();

    validate_mode_select_options_and_records(
        &inventory,
        &main_executable,
        &options_overlay,
        &options_information,
    )
    .unwrap();

    let mut changed_main_executable = main_executable.clone();
    let setup_index_offset =
        PSX_EXE_HEADER_SIZE + usize::try_from(0x8001_cd7c_u32 - MAIN_TEXT_RUNTIME_BASE).unwrap();
    changed_main_executable[setup_index_offset] ^= 1;
    let error = validate_mode_select_options_and_records(
        &inventory,
        &changed_main_executable,
        &options_overlay,
        &options_information,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("main-executable source changed"),
        "{error:#}"
    );

    inventory
        .nodes
        .iter_mut()
        .find(|node| node.id == "mode-select/entry/options")
        .unwrap()
        .consumer_class = Some("options/unbound-renderer".to_string());
    let error = validate_mode_select_options_and_records(
        &inventory,
        &main_executable,
        &options_overlay,
        &options_information,
    )
    .unwrap_err();
    assert!(error.to_string().contains("consumer class changed"));
}

#[test]
#[ignore = "requires the original disc in roms/ and specs/"]
fn resolved_character_select_entries_reject_source_or_route_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut inventory =
        load_surface_inventory(&root.join("specs/surfaces/mode-select/manifest.json")).unwrap();
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, main_executable) =
        rebuild::read_record(&cue.image_path, MAIN_EXECUTABLE_RECORD.path).unwrap();
    let overlays = CHARACTER_SELECT_OVERLAY_RECORDS
        .iter()
        .map(|record| {
            rebuild::read_record(&cue.image_path, record.path)
                .unwrap()
                .1
        })
        .collect::<Vec<_>>();
    let textures = CHARACTER_SELECT_TEXTURE_RECORDS
        .iter()
        .map(|record| {
            rebuild::read_record(&cue.image_path, record.path)
                .unwrap()
                .1
        })
        .collect::<Vec<_>>();
    let (_, cooperative_menu) = rebuild::read_record(
        &cue.image_path,
        CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.path,
    )
    .unwrap();

    validate_mode_select_character_select_routes(
        &inventory,
        &main_executable,
        &overlays,
        &textures,
        &cooperative_menu,
    )
    .unwrap();

    let mut changed_overlays = overlays.clone();
    changed_overlays[0][0x0ba8] ^= 1;
    let error = validate_mode_select_character_select_routes(
        &inventory,
        &main_executable,
        &changed_overlays,
        &textures,
        &cooperative_menu,
    )
    .unwrap_err();
    assert!(error.to_string().contains("PLSEL1 source binding changed"));

    let mut changed_cooperative_menu = cooperative_menu.clone();
    changed_cooperative_menu[0] ^= 1;
    let error = validate_mode_select_character_select_routes(
        &inventory,
        &main_executable,
        &overlays,
        &textures,
        &changed_cooperative_menu,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cooperative-menu stored source binding changed")
    );

    inventory
        .nodes
        .iter_mut()
        .find(|node| node.id == "mode-select/entry/team")
        .unwrap()
        .consumer_class = Some("character-select/unbound-renderer".to_string());
    let error = validate_mode_select_character_select_routes(
        &inventory,
        &main_executable,
        &overlays,
        &textures,
        &cooperative_menu,
    )
    .unwrap_err();
    assert!(error.to_string().contains("consumer class changed"));
}

#[test]
#[ignore = "requires the original disc in roms/ and specs/"]
fn practical_exam_entry_rejects_source_or_consumer_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut inventory =
        load_surface_inventory(&root.join("specs/surfaces/mode-select/manifest.json")).unwrap();
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, main_executable) =
        rebuild::read_record(&cue.image_path, MAIN_EXECUTABLE_RECORD.path).unwrap();
    let (_, primary_overlay) =
        rebuild::read_record(&cue.image_path, CHARACTER_SELECT_OVERLAY_RECORDS[0].path).unwrap();
    let (_, primary_texture) =
        rebuild::read_record(&cue.image_path, CHARACTER_SELECT_TEXTURE_RECORDS[0].path).unwrap();

    validate_mode_select_practical_entry(
        &inventory,
        &main_executable,
        &primary_overlay,
        &primary_texture,
    )
    .unwrap();

    let mut changed_main_executable = main_executable.clone();
    let overlay_call_offset =
        PSX_EXE_HEADER_SIZE + usize::try_from(0x8001_75f8_u32 - MAIN_TEXT_RUNTIME_BASE).unwrap();
    changed_main_executable[overlay_call_offset] ^= 1;
    let error = validate_mode_select_practical_entry(
        &inventory,
        &changed_main_executable,
        &primary_overlay,
        &primary_texture,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("main-executable source changed"),
        "{error:#}"
    );

    inventory
        .nodes
        .iter_mut()
        .find(|node| node.id == "mode-select/entry/practical-exam-99")
        .unwrap()
        .consumer_class = Some("practical-exam/unbound-renderer".to_string());
    let error = validate_mode_select_practical_entry(
        &inventory,
        &main_executable,
        &primary_overlay,
        &primary_texture,
    )
    .unwrap_err();
    assert!(error.to_string().contains("consumer binding changed"));
}

#[test]
#[ignore = "requires the original disc in roms/ and specs/"]
fn direct_entries_reject_source_or_consumer_drift() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut inventory =
        load_surface_inventory(&root.join("specs/surfaces/mode-select/manifest.json")).unwrap();
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, main_executable) =
        rebuild::read_record(&cue.image_path, MAIN_EXECUTABLE_RECORD.path).unwrap();
    let (_, diary_overlay) =
        rebuild::read_record(&cue.image_path, TITLE_MENU_OVERLAY_RECORD.path).unwrap();
    let (_, menu) = rebuild::read_record(&cue.image_path, MENU_RECORD.path).unwrap();
    let (_, gorin_overlay) =
        rebuild::read_record(&cue.image_path, GORIN_SELECTION_OVERLAY_RECORD.path).unwrap();
    let (_, gorin_menu) =
        rebuild::read_record(&cue.image_path, GORIN_MAIN_MENU_RECORD.path).unwrap();
    let (_, edit_overlay) =
        rebuild::read_record(&cue.image_path, EDIT_REGISTRATION_OVERLAY_RECORD.path).unwrap();
    let (_, edit_ui) =
        rebuild::read_record(&cue.image_path, EDIT_REGISTRATION_UI_RECORD.path).unwrap();
    let (_, bonus_overlay) =
        rebuild::read_record(&cue.image_path, BONUS_MENU_OVERLAY_RECORD.path).unwrap();
    let (_, bonus_menu) = rebuild::read_record(&cue.image_path, BONUS_MENU_RECORD.path).unwrap();

    validate_mode_select_direct_entry_routes(
        &inventory,
        &main_executable,
        &ModeSelectDirectEntrySources {
            diary_overlay_stored: &diary_overlay,
            menu_stored: &menu,
            gorin_overlay: &gorin_overlay,
            gorin_menu_stored: &gorin_menu,
            edit_overlay: &edit_overlay,
            edit_ui_stored: &edit_ui,
            bonus_overlay: &bonus_overlay,
            bonus_menu_stored: &bonus_menu,
        },
    )
    .unwrap();

    let mut changed_edit_overlay = edit_overlay.clone();
    changed_edit_overlay[0] ^= 1;
    let error = validate_mode_select_direct_entry_routes(
        &inventory,
        &main_executable,
        &ModeSelectDirectEntrySources {
            diary_overlay_stored: &diary_overlay,
            menu_stored: &menu,
            gorin_overlay: &gorin_overlay,
            gorin_menu_stored: &gorin_menu,
            edit_overlay: &changed_edit_overlay,
            edit_ui_stored: &edit_ui,
            bonus_overlay: &bonus_overlay,
            bonus_menu_stored: &bonus_menu,
        },
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("EDIT registration overlay source changed")
    );

    inventory
        .nodes
        .iter_mut()
        .find(|node| node.id == "mode-select/entry/bonus")
        .unwrap()
        .consumer_class = Some("bonus/unbound-renderer".to_string());
    let error = validate_mode_select_direct_entry_routes(
        &inventory,
        &main_executable,
        &ModeSelectDirectEntrySources {
            diary_overlay_stored: &diary_overlay,
            menu_stored: &menu,
            gorin_overlay: &gorin_overlay,
            gorin_menu_stored: &gorin_menu,
            edit_overlay: &edit_overlay,
            edit_ui_stored: &edit_ui,
            bonus_overlay: &bonus_overlay,
            bonus_menu_stored: &bonus_menu,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("consumer binding changed"));
}

#[test]
fn dangling_surface_edges_are_rejected_before_product_adoption() {
    let root = temporary_directory("surface-inventory-dangling-edge");
    let mut shard = valid_shard();
    shard["edges"][0]["to"] = json!("mode-select/entry/missing");
    write_inventory(&root, shard);

    let error = load_surface_inventory(&root.join("manifest.json")).unwrap_err();

    assert!(error.to_string().contains("unknown target"));
    std::fs::remove_dir_all(root).unwrap();
}

fn write_inventory(root: &Path, shard: Value) {
    std::fs::create_dir_all(root).unwrap();
    let manifest = json!({
        "kind": "justice_gakuen2_surface_inventory_manifest",
        "family_id": "mode-select-descendants",
        "source_bin_sha256": SOURCE_SHA256,
        "root_surface_id": "mode-select/root",
        "traversal_boundary": {
            "entry": "MODE SELECT root",
            "terminal_rule": "First stable descendant",
            "beyond_boundary": "Later shards"
        },
        "shards": ["root.json"],
        "bindings": ["dispatcher.json"]
    });
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("root.json"),
        serde_json::to_vec_pretty(&shard).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("dispatcher.json"), b"{}\n").unwrap();
}

fn valid_shard() -> Value {
    json!({
        "kind": "justice_gakuen2_surface_inventory_shard",
        "family_id": "mode-select-descendants",
        "nodes": [
            {
                "id": "mode-select/root",
                "resolution": "resolved",
                "entry_route": "Enter the root",
                "first_stable_stop": "Root accepts selection",
                "consumer_class": "mode-select/panel-renderer",
                "target_objects": [
                    {
                        "path": "DAT2/MENU.BIZ",
                        "layer": "decoded_record",
                        "role": "Root graphics"
                    },
                    {
                        "path": "DAT1/MODESEL.BIN",
                        "layer": "iso_record",
                        "role": "Root consumer"
                    }
                ],
                "unresolved_reason": null
            },
            {
                "id": "mode-select/entry/solo",
                "resolution": "unresolved",
                "entry_route": "Select solo",
                "first_stable_stop": "First solo descendant",
                "consumer_class": null,
                "target_objects": [],
                "unresolved_reason": "Consumer binding is not adopted"
            }
        ],
        "edges": [
            {
                "id": "mode-select/root/select/solo",
                "from": "mode-select/root",
                "to": "mode-select/entry/solo",
                "selection_asset_id": "solo"
            }
        ]
    })
}
