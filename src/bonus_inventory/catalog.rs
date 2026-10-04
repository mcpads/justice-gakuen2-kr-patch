use crate::tim::Cell;

use super::model::BonusInventoryFontRole;

pub(super) const EXPECTED_UNITS: [(&str, &str); 17] = [
    ("title", "おまけを見る"),
    ("card_category", "熱血カード"),
    ("poster_category", "熱血ポスター"),
    ("cd_category", "熱血CD"),
    ("video_category", "熱血ビデオ"),
    ("illustrations_category", "熱血イラスト集"),
    ("pocketstation_category", "ポケットステーション"),
    ("return", "戻る"),
    ("select_help", "方向キー：選択"),
    ("decide_help", "ボタン：決定"),
    ("back_help", "ボタン：戻る"),
    ("playback_stop_help", "ボタン：ストップ・戻る"),
    ("viewer_back", "BACK"),
    ("viewer_next", "NEXT"),
    ("viewer_help", "カードを見る："),
    ("viewer_exit", "EXIT："),
    ("no_card", "No Card"),
];

#[derive(Clone, Copy)]
pub(super) struct ExpectedOccurrence {
    pub(super) id: &'static str,
    pub(super) font_role: Option<BonusInventoryFontRole>,
    pub(super) cell: Cell,
}

pub(super) fn expected_occurrences(id: &str) -> Vec<ExpectedOccurrence> {
    let category = match id {
        "card_category" => Some(0),
        "poster_category" => Some(1),
        "cd_category" => Some(2),
        "video_category" => Some(3),
        "illustrations_category" => Some(4),
        "pocketstation_category" => Some(5),
        _ => None,
    };
    if let Some(index) = category {
        let compact_width = match id {
            "illustrations_category" => 144,
            "pocketstation_category" => 200,
            _ => 128,
        };
        return vec![
            occurrence(
                "compact",
                Some(BonusInventoryFontRole::CompactLabel),
                0,
                48 + index * 24,
                compact_width,
                24,
            ),
            occurrence(
                "large",
                Some(BonusInventoryFontRole::LargeLabel),
                256,
                index * 32,
                192,
                32,
            ),
        ];
    }
    match id {
        "title" => vec![occurrence(
            "main",
            Some(BonusInventoryFontRole::Title),
            0,
            0,
            256,
            48,
        )],
        "return" => vec![
            occurrence(
                "compact",
                Some(BonusInventoryFontRole::CompactAction),
                128,
                96,
                64,
                24,
            ),
            occurrence(
                "large",
                Some(BonusInventoryFontRole::LargeAction),
                448,
                0,
                64,
                32,
            ),
        ],
        "select_help" => vec![authored_help_occurrence("main", 32, 208, 112)],
        "decide_help" => vec![authored_help_occurrence("main", 160, 208, 88)],
        "back_help" => vec![authored_help_occurrence("main", 16, 224, 88)],
        // The cancel icon occupies x=0..14; Japanese outline pixels start at x=15.
        "playback_stop_help" => vec![authored_help_occurrence("main", 15, 240, 241)],
        "viewer_back" => vec![occurrence(
            "card_viewer",
            Some(BonusInventoryFontRole::ViewerNavigation),
            512,
            200,
            48,
            16,
        )],
        "viewer_next" => vec![occurrence(
            "card_viewer",
            Some(BonusInventoryFontRole::ViewerNavigation),
            // NEXT starts at x=717; x=720 leaves the N's left stem visible.
            717,
            200,
            51,
            16,
        )],
        "viewer_help" => vec![authored_help_occurrence("card_viewer", 512, 216, 88)],
        "viewer_exit" => vec![authored_help_occurrence("card_viewer", 616, 216, 60)],
        "no_card" => vec![occurrence(
            "card_placeholder",
            Some(BonusInventoryFontRole::ViewerCardPlaceholder),
            704,
            72,
            48,
            64,
        )],
        _ => Vec::new(),
    }
}

const fn authored_help_occurrence(
    id: &'static str,
    x: usize,
    y: usize,
    width: usize,
) -> ExpectedOccurrence {
    occurrence(id, Some(BonusInventoryFontRole::Help), x, y, width, 16)
}

const fn occurrence(
    id: &'static str,
    font_role: Option<BonusInventoryFontRole>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> ExpectedOccurrence {
    ExpectedOccurrence {
        id,
        font_role,
        cell: Cell {
            x,
            y,
            width,
            height,
        },
    }
}
