use crate::text::fixed_menu_glyph_code;

use super::allocation::{
    TRANSPARENT_ADVANCE_CODE, UntranslatedGlyphCells, allocate_options_glyphs,
};
use super::contextual_glyphs::{
    OPTIONS_CONTROLLER_ICON_CODES, PRESERVED_SOURCE_GRAPHIC_CODES, SHARED_SOURCE_GRAPHIC_CODES,
};
use super::model::{
    OptionsAuthoredUnit, OptionsDevelopmentStatus, OptionsFontRole, OptionsFontStyle,
    OptionsFontStyles, OptionsReleaseStatus, OptionsTranslationUnit,
};
use std::path::PathBuf;

fn unit(role: OptionsFontRole, text: &str) -> OptionsAuthoredUnit {
    OptionsAuthoredUnit {
        id: role.key().to_string(),
        source_offset: "0x0000".to_string(),
        pointer_offsets: vec![],
        source_codes: vec![],
        source_text: "source".to_string(),
        korean_text: text.to_string(),
        font_role: role,
        release_status: OptionsReleaseStatus::NeedsHumanReview,
    }
}

fn styles() -> OptionsFontStyles {
    OptionsFontStyles {
        heading: OptionsFontStyle {
            font: PathBuf::from("heading.ttf"),
            font_px: 24.0,
        },
        help: OptionsFontStyle {
            font: PathBuf::from("help.ttf"),
            font_px: 13.0,
        },
        label: OptionsFontStyle {
            font: PathBuf::from("body.ttf"),
            font_px: 14.0,
        },
        value: OptionsFontStyle {
            font: PathBuf::from("body.ttf"),
            font_px: 14.0,
        },
        action: OptionsFontStyle {
            font: PathBuf::from("body.ttf"),
            font_px: 14.0,
        },
        records_main: super::model::RecordsMainFontStyles {
            heading: OptionsFontStyle {
                font: PathBuf::from("records-heading.ttf"),
                font_px: 24.0,
            },
            item: OptionsFontStyle {
                font: PathBuf::from("records-item.ttf"),
                font_px: 17.0,
            },
        },
        records_prompt: OptionsFontStyle {
            font: PathBuf::from("records-prompt.ttf"),
            font_px: 15.0,
        },
        records_status: super::model::RecordsStatusFontStyles {
            heading: OptionsFontStyle {
                font: PathBuf::from("records-status-heading.ttf"),
                font_px: 14.0,
            },
            message: OptionsFontStyle {
                font: PathBuf::from("records-status-message.ttf"),
                font_px: 14.0,
            },
        },
        background: super::model::OptionsBackgroundFontStyles {
            school_name: OptionsFontStyle {
                font: PathBuf::from("school-name.ttf"),
                font_px: 14.0,
            },
            crest_mark: OptionsFontStyle {
                font: PathBuf::from("crest-mark.ttf"),
                font_px: 30.0,
            },
        },
        description: OptionsFontStyle {
            font: PathBuf::from("description.ttf"),
            font_px: 16.0,
        },
    }
}

#[test]
fn role_scoped_allocation_keeps_the_same_character_in_distinct_fonts() {
    let help_text = (0..83)
        .map(|offset| char::from_u32('가' as u32 + u32::try_from(offset).unwrap()).unwrap())
        .collect::<String>();
    let units = vec![
        unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
        unit(OptionsFontRole::Help, &help_text),
        unit(OptionsFontRole::Label, "가"),
    ];

    let allocation = allocate_options_glyphs(&units, &styles()).unwrap();

    assert_ne!(
        allocation.code_for(OptionsFontRole::Help, '가').unwrap(),
        allocation.code_for(OptionsFontRole::Label, '가').unwrap()
    );
    assert_eq!(allocation.global_glyphs.len(), 101);
    assert_eq!(allocation.provisional_physical_codes.len(), 122);
    assert!(
        allocation
            .global_glyphs
            .iter()
            .filter(|glyph| glyph.role == OptionsFontRole::Heading)
            .all(|glyph| glyph.cell.width == 40 && glyph.cell.height == 40)
    );
}

#[test]
fn options_allocation_uses_its_complete_reserved_partition() {
    let capacity = crate::menu_glyph_slots::OPTIONS_SMALL_GLYPH_CODE_CANDIDATES
        .iter()
        .filter(|code| !PRESERVED_SOURCE_GRAPHIC_CODES.contains(code))
        .count();
    let small_text = (0..capacity)
        .map(|offset| char::from_u32('가' as u32 + u32::try_from(offset).unwrap()).unwrap())
        .collect::<String>();
    let units = vec![
        unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
        unit(OptionsFontRole::Help, &small_text),
    ];

    let allocation = allocate_options_glyphs(&units, &styles()).unwrap();

    assert_eq!(allocation.global_glyphs.len(), capacity + 17);
    assert_eq!(allocation.provisional_physical_codes.len(), capacity + 38);

    let overflow_text = (0..capacity + 1)
        .map(|offset| char::from_u32('가' as u32 + u32::try_from(offset).unwrap()).unwrap())
        .collect::<String>();
    let error = match allocate_options_glyphs(
        &[
            unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
            unit(OptionsFontRole::Help, &overflow_text),
        ],
        &styles(),
    ) {
        Ok(_) => panic!("options allocation accepted a glyph beyond its reserved partition"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("MENU atlas capacity"));
}

#[test]
fn matching_body_styles_share_the_same_character_cell() {
    let units = vec![
        unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
        unit(OptionsFontRole::Label, "가"),
        unit(OptionsFontRole::Value, "가"),
    ];

    let allocation = allocate_options_glyphs(&units, &styles()).unwrap();

    assert_eq!(
        allocation.code_for(OptionsFontRole::Label, '가').unwrap(),
        allocation.code_for(OptionsFontRole::Value, '가').unwrap()
    );
}

#[test]
fn records_main_uses_independent_large_heading_and_item_styles() {
    let units = vec![
        unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
        unit(OptionsFontRole::RecordsMainHeading, "기록"),
        unit(OptionsFontRole::RecordsMainItem, "불러오기저장자동"),
    ];

    let allocation = allocate_options_glyphs(&units, &styles()).unwrap();

    assert_eq!(
        allocation
            .code_for(OptionsFontRole::RecordsMainHeading, '기')
            .unwrap(),
        0x0368
    );
    assert_eq!(
        allocation
            .code_for(OptionsFontRole::RecordsMainHeading, '록')
            .unwrap(),
        0x036a
    );
    assert!(
        allocation
            .global_glyphs
            .iter()
            .filter(|glyph| glyph.role == OptionsFontRole::RecordsMainHeading)
            .all(|glyph| glyph.cell.width == 40 && glyph.cell.height == 40)
    );
    assert!(
        allocation
            .global_glyphs
            .iter()
            .filter(|glyph| glyph.role == OptionsFontRole::RecordsMainItem)
            .all(|glyph| glyph.cell.width == 20 && glyph.cell.height == 20)
    );
}

#[test]
fn native_ascii_codes_and_transparent_space_keep_their_encoding() {
    assert_eq!(fixed_menu_glyph_code('C'), Some(0x0010));
    assert_eq!(fixed_menu_glyph_code('1'), Some(0x0000));
    assert_eq!(fixed_menu_glyph_code('0'), Some(0x0009));
    assert_eq!(fixed_menu_glyph_code('?'), Some(0x0055));
    assert_eq!(fixed_menu_glyph_code(' '), Some(TRANSPARENT_ADVANCE_CODE));
    assert_eq!(fixed_menu_glyph_code('가'), None);
}

#[test]
fn untranslated_pointer_text_blocks_wrapped_physical_cell_reuse() {
    let unit = OptionsTranslationUnit {
        kind: String::new(),
        id: "still_japanese".to_string(),
        source_offset: "0x0000".to_string(),
        pointer_offsets: vec![],
        source_codes: vec!["0x000c".to_string(), "0x0fff".to_string()],
        source_text: None,
        korean_text: None,
        font_role: None,
        development_status: OptionsDevelopmentStatus::Untranslated,
        release_status: OptionsReleaseStatus::Untranslated,
    };

    let units = [unit];
    let cells = UntranslatedGlyphCells::from_units(&units, &[]).unwrap();
    assert!(cells.preserved_codes().contains(&0x0000));
    let conflicts = cells.conflicts_for(&[0x0000]);

    assert_eq!(
        conflicts,
        ["0x0000 overlaps source 0x000c used by still_japanese"]
    );
}

#[test]
fn large_untranslated_glyph_reserves_its_continuation_cells() {
    let unit = OptionsTranslationUnit {
        kind: String::new(),
        id: "large_title".to_string(),
        source_offset: "0x0100".to_string(),
        pointer_offsets: vec![],
        source_codes: vec!["0x0000".to_string()],
        source_text: None,
        korean_text: None,
        font_role: None,
        development_status: OptionsDevelopmentStatus::Untranslated,
        release_status: OptionsReleaseStatus::Untranslated,
    };

    let units = [unit];
    let cells = UntranslatedGlyphCells::from_units(&units, &[0x0100]).unwrap();
    assert!(cells.preserved_codes().contains(&0x0001));
    let conflicts = cells.conflicts_for(&[0x0001]);

    assert_eq!(
        conflicts,
        ["0x0001 overlaps source 0x0000 used by large_title"]
    );
}

#[test]
#[ignore = "requires assets/"]
fn tracked_options_preserve_controller_icons_and_shared_graphics_outside_records() {
    let allocation =
        allocate_options_glyphs(&tracked_authored_units(), &production_styles()).unwrap();

    assert!(
        allocation
            .global_glyphs
            .iter()
            .all(|glyph| !PRESERVED_SOURCE_GRAPHIC_CODES.contains(&glyph.code))
    );
    assert!(allocation.records_contextual_glyphs.len() <= PRESERVED_SOURCE_GRAPHIC_CODES.len());
    assert!(
        allocation
            .records_contextual_glyphs
            .iter()
            .all(|glyph| PRESERVED_SOURCE_GRAPHIC_CODES.contains(&glyph.code))
    );
    assert!(
        OPTIONS_CONTROLLER_ICON_CODES
            .into_iter()
            .chain(SHARED_SOURCE_GRAPHIC_CODES)
            .all(|code| !allocation
                .global_glyphs
                .iter()
                .any(|glyph| glyph.code == code))
    );
}

fn tracked_authored_units() -> Vec<OptionsAuthoredUnit> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/menu/options");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    manifest["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|reference| {
            let file = reference["file"].as_str().unwrap();
            let unit: OptionsTranslationUnit =
                serde_json::from_slice(&std::fs::read(root.join(file)).unwrap()).unwrap();
            (unit.development_status == OptionsDevelopmentStatus::Authored).then(|| {
                OptionsAuthoredUnit {
                    id: unit.id,
                    source_offset: unit.source_offset,
                    pointer_offsets: unit.pointer_offsets,
                    source_codes: unit.source_codes,
                    source_text: unit.source_text.unwrap(),
                    korean_text: unit.korean_text.unwrap(),
                    font_role: unit.font_role.unwrap(),
                    release_status: unit.release_status,
                }
            })
        })
        .collect()
}

fn production_styles() -> OptionsFontStyles {
    let body = OptionsFontStyle {
        font: PathBuf::from("maplestory-bold.ttf"),
        font_px: 14.0,
    };
    OptionsFontStyles {
        heading: OptionsFontStyle {
            font: body.font.clone(),
            font_px: 24.0,
        },
        help: body.clone(),
        label: body.clone(),
        value: body.clone(),
        action: body.clone(),
        records_main: super::model::RecordsMainFontStyles {
            heading: OptionsFontStyle {
                font: body.font.clone(),
                font_px: 32.0,
            },
            item: body.clone(),
        },
        records_prompt: body.clone(),
        records_status: super::model::RecordsStatusFontStyles {
            heading: body.clone(),
            message: body.clone(),
        },
        background: super::model::OptionsBackgroundFontStyles {
            school_name: body.clone(),
            crest_mark: OptionsFontStyle {
                font: body.font.clone(),
                font_px: 30.0,
            },
        },
        description: OptionsFontStyle {
            font: body.font,
            font_px: 16.0,
        },
    }
}

#[test]
fn added_heading_reuses_owned_cells_without_overlapping_small_glyphs_or_icons() {
    let mut heading = unit(OptionsFontRole::Heading, "옵션키설정게임사");
    heading.source_codes = vec!["0x0348".to_string()];
    let allocation = allocate_options_glyphs(
        &[heading, unit(OptionsFontRole::Label, "가나다")],
        &styles(),
    )
    .unwrap();
    let large = allocation
        .global_glyphs
        .iter()
        .find(|g| g.character == '사')
        .unwrap();
    assert_eq!(large.code, 0x0348);
    assert_eq!((large.cell.width, large.cell.height), (40, 40));
    for other in allocation
        .global_glyphs
        .iter()
        .filter(|g| g.character != '사')
    {
        assert!(!crate::menu_audit::wrapped_cells_overlap_sized(
            large.code,
            40,
            other.code,
            other.cell.width
        ));
    }
    for icon in PRESERVED_SOURCE_GRAPHIC_CODES {
        assert!(!crate::menu_audit::wrapped_cells_overlap_sized(
            large.code, 40, icon, 20
        ));
    }
}

#[test]
fn reclaimed_source_cells_respect_the_untranslated_population() {
    let mut label = unit(OptionsFontRole::Label, "가나다");
    label.source_codes = vec!["0x0137".to_string(), "0x0154".to_string()];
    let protected = [0x0137, 0x0154].into_iter().collect();
    let allocation = super::allocation::allocate_options_glyphs_preserving(
        &[unit(OptionsFontRole::Heading, "옵션키설정게임"), label],
        &styles(),
        &protected,
    )
    .unwrap();
    for code in protected {
        assert!(!allocation.provisional_physical_codes.contains(&code));
    }
}

#[test]
fn unsupported_fixed_letters_and_punctuation_receive_raster_cells() {
    let allocation = allocate_options_glyphs(
        &[
            unit(OptionsFontRole::Heading, "옵션키설정게임"),
            unit(OptionsFontRole::Label, "BGM XA:"),
        ],
        &styles(),
    )
    .unwrap();
    for ch in "BGMXA:"
        .chars()
        .filter(|ch| fixed_menu_glyph_code(*ch).is_none())
    {
        let code = allocation.code_for(OptionsFontRole::Label, ch).unwrap();
        assert!(
            allocation
                .global_glyphs
                .iter()
                .any(|g| g.character == ch && g.code == code)
        );
    }
}

#[test]
fn untranslated_cell_ownership_merges_repeated_users_without_reserving_authored_text() {
    let make = |id: &str, code: &str, status| OptionsTranslationUnit {
        kind: String::new(),
        id: id.into(),
        source_offset: "0x0000".into(),
        pointer_offsets: vec![],
        source_codes: vec![code.into(), code.into()],
        source_text: None,
        korean_text: None,
        font_role: None,
        development_status: status,
        release_status: OptionsReleaseStatus::Untranslated,
    };
    let units = [
        make("b", "0x800c", OptionsDevelopmentStatus::Untranslated),
        make("a", "0x000c", OptionsDevelopmentStatus::Untranslated),
        make("translated", "0x0100", OptionsDevelopmentStatus::Authored),
        make("space", "0x0fff", OptionsDevelopmentStatus::Untranslated),
    ];
    let cells = UntranslatedGlyphCells::from_units(&units, &[]).unwrap();
    assert_eq!(
        cells.conflicts_for(&[0, 0]),
        ["0x0000 overlaps source 0x000c used by a,b"]
    );
    assert!(!cells.preserved_codes().contains(&0x0100));
    assert!(cells.conflicts_for(&[0x0100]).is_empty());
}

#[test]
fn latin_and_digits_are_encoded_with_their_role_font_instead_of_native_cells() {
    let units = vec![
        unit(OptionsFontRole::Heading, "옵션 키 설정 게임 설정"),
        unit(OptionsFontRole::Help, "CPU 1"),
        unit(OptionsFontRole::Value, "CPU 1"),
    ];
    let allocation = allocate_options_glyphs(&units, &styles()).unwrap();
    let help = super::text_rebuild::encode_unit(&units[1], &allocation).unwrap();
    let value = super::text_rebuild::encode_unit(&units[2], &allocation).unwrap();
    for (index, character) in "CPU 1".chars().enumerate() {
        if character == ' ' {
            assert_eq!(help[index], crate::text::SKIP_GLYPH_CODE);
            assert_eq!(value[index], crate::text::SKIP_GLYPH_CODE);
        } else {
            assert_ne!(help[index], value[index]);
            assert_ne!(Some(help[index]), fixed_menu_glyph_code(character));
            assert_ne!(Some(value[index]), fixed_menu_glyph_code(character));
        }
    }
    for character in '0'..='9' {
        let native_code = fixed_menu_glyph_code(character).unwrap();
        let glyph = allocation
            .global_glyphs
            .iter()
            .find(|glyph| glyph.code == native_code)
            .unwrap();
        assert_eq!(
            (glyph.role, glyph.character),
            (OptionsFontRole::Value, character)
        );
    }
    // Adding the dynamic pixel suppliers must not redirect authored strings.
    assert_eq!(
        super::text_rebuild::encode_unit(&units[1], &allocation).unwrap(),
        help
    );
    assert_eq!(
        super::text_rebuild::encode_unit(&units[2], &allocation).unwrap(),
        value
    );
}
