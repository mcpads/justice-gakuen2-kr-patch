use std::path::Path;

use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus, BonusShopTranslationUnit,
};
use super::{BonusShopTextBuildConfig, BonusShopTextFontSources, build_bonus_shop_text};
use crate::bonus_inventory::BonusInventoryTextStyleSource;
use crate::bonus_shop_exit_confirmation::{
    BonusShopExitConfirmationBuildConfig, build_bonus_shop_exit_confirmation,
};
use crate::bonus_shop_source::{OVERLAY_SOURCE_SHA256, SHOP_UI_SOURCE_DECODED_SHA256, load_source};
use crate::pipeline::sha256_bytes;

#[test]
fn adjacent_changed_ranges_are_covered_by_the_union_of_owned_cells() {
    assert!(super::build::range_is_covered(
        [10, 20],
        &[[10, 15], [15, 20]]
    ));
    assert!(!super::build::range_is_covered(
        [10, 20],
        &[[10, 14], [15, 20]]
    ));
}

#[test]
#[ignore = "requires the user-supplied supported source disc and Maplestory font"]
fn source_build_consumes_all_units_without_competing_with_the_exit_writer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let font = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let output = std::env::temp_dir().join(format!(
        "justice-bonus-shop-text-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output);
    let build_spec_sha256 = "0".repeat(64);
    let exit = build_bonus_shop_exit_confirmation(&BonusShopExitConfirmationBuildConfig {
        cue: cue.clone(),
        assets: root.join("assets/menu/shop-ui/dynamic/exit-confirmation"),
        font: style(&font, 15.0),
        build_spec_sha256: build_spec_sha256.clone(),
        output_dir: output.join("exit-confirmation"),
        force: false,
    })
    .unwrap();
    let build = build_bonus_shop_text(
        &BonusShopTextBuildConfig {
            cue: cue.clone(),
            assets: root.join("assets/menu/shop-text"),
            fonts: BonusShopTextFontSources {
                product_description: style(&font, 15.0),
                product_label: style(&font, 17.0),
                clerk_dialogue: style(&font, 15.0),
            },
            build_spec_sha256,
            output_dir: output.join("pointer-text"),
            force: false,
        },
        &exit,
    )
    .unwrap();
    let source = load_source(&cue).unwrap();
    let runtime_glyph_codes =
        super::runtime_glyphs::protected_runtime_glyph_codes(&source.overlay).unwrap();

    assert_eq!(build.report.source_record_count, 186);
    assert_eq!(build.report.tracked_unit_count, 184);
    assert_eq!(build.report.delegated_record_count, 2);
    assert_eq!(build.report.untranslated_unit_count, 0);
    assert_eq!(build.report.authored_unit_count, 184);
    assert!(build.report.delegated_records_match_existing_writer);
    assert_eq!(
        build.report.runtime_glyph_code_count,
        runtime_glyph_codes.len()
    );
    assert!(build.report.glyphs.iter().all(|glyph| {
        let code = u16::from_str_radix(glyph.code.trim_start_matches("0x"), 16).unwrap();
        !runtime_glyph_codes.iter().copied().any(|protected| {
            crate::bonus_shop_source::cells_overlap(
                crate::bonus_shop_source::glyph_cell(code),
                crate::bonus_shop_source::glyph_cell(protected),
            )
        })
    }));
    assert!(build.report.complete_source_population);
    assert!(!build.report.shop_ui_expected_write_ranges.is_empty());
    assert!(!build.report.overlay_expected_write_ranges.is_empty());
    assert_ne!(
        build.report.output_shop_ui_decoded_sha256,
        SHOP_UI_SOURCE_DECODED_SHA256
    );
    assert_ne!(build.report.output_overlay_sha256, OVERLAY_SOURCE_SHA256);
    assert!(
        build
            .report
            .shop_ui_changes_confined_to_allocated_glyph_cells
    );
    assert!(build.report.overlay_changes_confined_to_authored_records);
    assert!(build.report.runtime_verification_required);

    let mut composed_shop_ui = source.shop_ui_decoded.clone();
    exit.apply_to_shop_ui_decoded(&mut composed_shop_ui)
        .unwrap();
    assert!(
        !build
            .apply_to_shop_ui_decoded(&mut composed_shop_ui)
            .unwrap()
            .is_empty()
    );

    for code in [0x0197, 0x0198] {
        let cell = crate::bonus_shop_source::glyph_cell(code);
        let read = |bytes: &[u8]| {
            crate::tim::read_indexed_cell_in_prefix(
                bytes,
                crate::bonus_shop_source::GLYPH_TIM_OFFSET,
                cell,
            )
            .unwrap()
        };
        assert_eq!(read(&composed_shop_ui), read(&source.shop_ui_decoded));
    }

    let mut composed_overlay = source.overlay;
    exit.apply_to_overlay(&mut composed_overlay).unwrap();
    let exit_record = composed_overlay[0x2f0c..0x2f66].to_vec();
    assert!(
        !build
            .apply_to_overlay(&mut composed_overlay)
            .unwrap()
            .is_empty()
    );
    assert_eq!(&composed_overlay[0x2f0c..0x2f66], exit_record);
    for (start, expected) in [(0x3bac, "카드No"), (0x3bbc, "획득했습니다")] {
        let count = usize::from(composed_overlay[start]);
        let decoded: String = composed_overlay[start + 1..start + 1 + count * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|s| {
                let code = (u16::from(s[0]) << 8) | (u16::from(s[2]) << 4) | u16::from(s[1]);
                *build.glyph_codes[&BonusShopFontRole::ClerkDialogue]
                    .iter()
                    .find(|(_, v)| **v == code)
                    .unwrap()
                    .0
            })
            .collect();
        assert_eq!(decoded, expected);
    }
    assert_ne!(sha256_bytes(&composed_overlay), OVERLAY_SOURCE_SHA256);
    std::fs::remove_dir_all(output).unwrap();
}

#[test]
#[ignore = "requires the user-supplied supported source disc and Maplestory font"]
fn one_authored_unit_writes_its_record_and_role_scoped_glyph_after_the_exit_writer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let font = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let output = std::env::temp_dir().join(format!(
        "justice-bonus-shop-authored-build-{}",
        std::process::id()
    ));
    let assets = output.join("assets");
    let _ = std::fs::remove_dir_all(&output);
    copy_tree(&root.join("assets/menu/shop-text"), &assets);
    make_all_units_untranslated(&assets);
    let authored_path = assets.join("authored/product-labels/product-label-000.json");
    let mut authored: BonusShopTranslationUnit =
        serde_json::from_slice(&std::fs::read(&authored_path).unwrap()).unwrap();
    authored.korean_text = Some("가".to_string());
    authored.font_role = Some(BonusShopFontRole::ProductLabel);
    authored.development_status = BonusShopDevelopmentStatus::Authored;
    authored.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let mut authored_json = serde_json::to_vec_pretty(&authored).unwrap();
    authored_json.push(b'\n');
    std::fs::write(&authored_path, authored_json).unwrap();

    let build_spec_sha256 = "0".repeat(64);
    let exit = build_bonus_shop_exit_confirmation(&BonusShopExitConfirmationBuildConfig {
        cue: cue.clone(),
        assets: root.join("assets/menu/shop-ui/dynamic/exit-confirmation"),
        font: style(&font, 15.0),
        build_spec_sha256: build_spec_sha256.clone(),
        output_dir: output.join("exit-confirmation"),
        force: false,
    })
    .unwrap();
    let build = build_bonus_shop_text(
        &BonusShopTextBuildConfig {
            cue: cue.clone(),
            assets,
            fonts: BonusShopTextFontSources {
                product_description: style(&font, 15.0),
                product_label: style(&font, 17.0),
                clerk_dialogue: style(&font, 15.0),
            },
            build_spec_sha256,
            output_dir: output.join("pointer-text"),
            force: false,
        },
        &exit,
    )
    .unwrap();

    assert_eq!(build.report.authored_unit_count, 1);
    assert_eq!(build.report.untranslated_unit_count, 183);
    let label_glyphs = build
        .report
        .glyphs
        .iter()
        .filter(|g| g.role == BonusShopFontRole::ProductLabel)
        .collect::<Vec<_>>();
    assert_eq!(label_glyphs.len(), 1);
    assert_eq!(label_glyphs[0].text, "가");
    assert_eq!(build.report.card_result_record_count, 2);
    assert!(!build.report.shop_ui_changed_byte_ranges.is_empty());
    assert!(!build.report.overlay_changed_byte_ranges.is_empty());
    assert!(
        build
            .report
            .shop_ui_changes_confined_to_allocated_glyph_cells
    );
    assert!(build.report.overlay_changes_confined_to_authored_records);
    assert!(build.report.runtime_verification_required);

    let source = load_source(&cue).unwrap();
    let mut composed_shop_ui = source.shop_ui_decoded.clone();
    exit.apply_to_shop_ui_decoded(&mut composed_shop_ui)
        .unwrap();
    assert!(
        !build
            .apply_to_shop_ui_decoded(&mut composed_shop_ui)
            .unwrap()
            .is_empty()
    );
    let mut composed_overlay = source.overlay;
    exit.apply_to_overlay(&mut composed_overlay).unwrap();
    let exit_record = composed_overlay[0x2f0c..0x2f66].to_vec();
    assert!(
        !build
            .apply_to_overlay(&mut composed_overlay)
            .unwrap()
            .is_empty()
    );
    assert_eq!(&composed_overlay[0x2f0c..0x2f66], exit_record);
    std::fs::remove_dir_all(output).unwrap();
}

fn style(font: &Path, font_px: f32) -> BonusInventoryTextStyleSource {
    BonusInventoryTextStyleSource {
        path: font.to_path_buf(),
        font_px,
        tracking_px: 0.0,
        vertical_shift_px: -1,
    }
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            std::fs::copy(source_path, target_path).unwrap();
        }
    }
}

fn make_all_units_untranslated(root: &Path) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            make_all_units_untranslated(&path);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(mut unit) = serde_json::from_slice::<BonusShopTranslationUnit>(&bytes) else {
            continue;
        };
        unit.korean_text = None;
        unit.font_role = None;
        unit.development_status = BonusShopDevelopmentStatus::Untranslated;
        unit.release_status = BonusShopReleaseStatus::Untranslated;
        let mut json = serde_json::to_vec_pretty(&unit).unwrap();
        json.push(b'\n');
        std::fs::write(path, json).unwrap();
    }
}
