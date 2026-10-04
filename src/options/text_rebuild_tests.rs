use super::allocation::{TRANSPARENT_ADVANCE_CODE, allocate_options_glyphs};
use super::model::{OptionsAuthoredUnit, OptionsFontRole, OptionsReleaseStatus};
use super::text_rebuild::{encode_unit, pack_records_for_test};

#[test]
fn packed_records_preserve_length_prefixes_and_exact_offsets() {
    let first = [0x0010, 0x0021, 0x0026];
    let second = [0x00b7, 0x00bb];

    let (bytes, offsets) =
        pack_records_for_test(&[("first", &first), ("second", &second)], 16).unwrap();

    assert_eq!(offsets, [0, 8]);
    assert_eq!(&bytes[..8], &[3, 0, 0x10, 0, 0x21, 0, 0x26, 0]);
    assert_eq!(&bytes[8..14], &[2, 0, 0xb7, 0, 0xbb, 0]);
    assert_eq!(&bytes[14..], &[0, 0]);
}

#[test]
fn packed_records_reject_growth_beyond_the_owned_pool() {
    let codes = [0x00b7, 0x00bb, 0x011b];

    let error = pack_records_for_test(&[("too_long", &codes)], 6).unwrap_err();

    assert!(error.to_string().contains("need 8 bytes"));
}

#[test]
fn heading_word_space_is_wider_than_inter_character_spacing() {
    let unit = OptionsAuthoredUnit {
        id: "heading".to_string(),
        source_offset: "0x0000".to_string(),
        pointer_offsets: vec![],
        source_codes: vec![],
        source_text: "source".to_string(),
        korean_text: "키 설정".to_string(),
        font_role: OptionsFontRole::Heading,
        release_status: OptionsReleaseStatus::NeedsHumanReview,
    };
    let repertoire = OptionsAuthoredUnit {
        korean_text: "옵션 키 설정 게임 설정".to_string(),
        ..unit.clone()
    };
    let help_text = (0..83)
        .map(|offset| char::from_u32('가' as u32 + offset).unwrap())
        .collect::<String>();
    let help = OptionsAuthoredUnit {
        korean_text: help_text,
        font_role: OptionsFontRole::Help,
        ..unit.clone()
    };
    let label = OptionsAuthoredUnit {
        korean_text: "가".to_string(),
        font_role: OptionsFontRole::Label,
        ..unit.clone()
    };
    let styles = super::model::OptionsFontStyles {
        heading: super::model::OptionsFontStyle {
            font: "heading.ttf".into(),
            font_px: 24.0,
        },
        help: super::model::OptionsFontStyle {
            font: "help.ttf".into(),
            font_px: 13.0,
        },
        label: super::model::OptionsFontStyle {
            font: "body.ttf".into(),
            font_px: 14.0,
        },
        value: super::model::OptionsFontStyle {
            font: "body.ttf".into(),
            font_px: 14.0,
        },
        action: super::model::OptionsFontStyle {
            font: "body.ttf".into(),
            font_px: 14.0,
        },
        records_main: super::model::RecordsMainFontStyles {
            heading: super::model::OptionsFontStyle {
                font: "records-heading.ttf".into(),
                font_px: 24.0,
            },
            item: super::model::OptionsFontStyle {
                font: "records-item.ttf".into(),
                font_px: 17.0,
            },
        },
        records_prompt: super::model::OptionsFontStyle {
            font: "records-prompt.ttf".into(),
            font_px: 15.0,
        },
        records_status: super::model::RecordsStatusFontStyles {
            heading: super::model::OptionsFontStyle {
                font: "records-status-heading.ttf".into(),
                font_px: 14.0,
            },
            message: super::model::OptionsFontStyle {
                font: "records-status-message.ttf".into(),
                font_px: 14.0,
            },
        },
        background: super::model::OptionsBackgroundFontStyles {
            school_name: super::model::OptionsFontStyle {
                font: "school-name.ttf".into(),
                font_px: 14.0,
            },
            crest_mark: super::model::OptionsFontStyle {
                font: "crest-mark.ttf".into(),
                font_px: 30.0,
            },
        },
        description: super::model::OptionsFontStyle {
            font: "description.ttf".into(),
            font_px: 16.0,
        },
    };
    let allocation = allocate_options_glyphs(&[repertoire, help, label], &styles).unwrap();

    let codes = encode_unit(&unit, &allocation).unwrap();

    assert_eq!(codes.len(), 6);
    assert_ne!(codes[0], TRANSPARENT_ADVANCE_CODE);
    assert_eq!(codes[1..3], [TRANSPARENT_ADVANCE_CODE; 2]);
    assert_ne!(codes[3], TRANSPARENT_ADVANCE_CODE);
    assert_eq!(codes[4], TRANSPARENT_ADVANCE_CODE);
    assert_ne!(codes[5], TRANSPARENT_ADVANCE_CODE);
}
