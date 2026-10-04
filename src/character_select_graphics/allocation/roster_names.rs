//! Complete source-cell inventory for character roster name consumers.

use super::{FixedStripSpec, roster_strip};

pub(super) const ROSTER_NAME_STRIPS: &[FixedStripSpec] = &[
    roster_strip("roster_name_batsu", "バツ", 2, 0x00, 0x00, 48, 16),
    roster_strip("roster_name_hinata", "ひなた", 2, 0x30, 0x00, 48, 16),
    roster_strip("roster_name_kyosuke", "恭介", 2, 0x60, 0x00, 48, 16),
    roster_strip("roster_name_shoma", "将馬", 2, 0x90, 0x00, 48, 16),
    roster_strip("roster_name_natsu", "夏", 2, 0xc0, 0x00, 48, 16),
    roster_strip("roster_name_roberto", "ロベルト", 2, 0x00, 0x10, 48, 16),
    roster_strip("roster_name_roy", "ロイ", 2, 0x30, 0x10, 48, 16),
    roster_strip("roster_name_tiffany", "ティファニー", 2, 0x60, 0x10, 48, 16),
    roster_strip("roster_name_bowman", "ボーマン", 2, 0x90, 0x10, 48, 16),
    roster_strip("roster_name_edge", "エッジ", 2, 0xc0, 0x10, 48, 16),
    roster_strip("roster_name_akira", "アキラ", 2, 0x00, 0x20, 48, 16),
    roster_strip("roster_name_gan", "岩", 2, 0x30, 0x20, 48, 16),
    roster_strip("roster_name_hideo", "英雄", 2, 0x60, 0x20, 48, 16),
    roster_strip("roster_name_kyoko", "響子", 2, 0x90, 0x20, 48, 16),
    roster_strip("roster_name_raizo", "雷蔵", 2, 0xc0, 0x20, 48, 16),
    roster_strip("roster_name_hyo", "雹", 2, 0x00, 0x30, 48, 16),
    roster_strip("roster_name_sakura", "さくら", 2, 0x30, 0x30, 48, 16),
    roster_strip("roster_name_daigo", "醍醐", 2, 0x60, 0x30, 48, 16),
    roster_strip("roster_name_hayato", "隼人", 2, 0x90, 0x30, 48, 16),
    roster_strip(
        "roster_name_akira_unmasked",
        "あきら",
        2,
        0xc0,
        0x30,
        48,
        16,
    ),
    roster_strip("roster_name_hinata_2", "ひなた2", 2, 0x00, 0x40, 48, 16),
    roster_strip("roster_name_natsu_2", "夏2", 2, 0x30, 0x40, 48, 16),
    roster_strip("roster_name_akira_2", "あきら2", 2, 0x60, 0x40, 48, 16),
    roster_strip(
        "roster_name_tiffany_2",
        "ティファニー2",
        2,
        0x90,
        0x40,
        48,
        16,
    ),
    roster_strip("roster_name_kyoko_2", "響子2", 2, 0xc0, 0x40, 48, 16),
    roster_strip("roster_name_edit", "EDIT", 2, 0x00, 0x50, 48, 16),
    roster_strip("roster_name_edit_2", "EDIT2", 2, 0x30, 0x50, 48, 16),
    // PLSEL's extended roster switches to the next texture page. These
    // irregular source cells are why the roster cannot be inferred as one grid.
    roster_strip("roster_name_nagare", "流", 3, 0x70, 0x60, 48, 16),
    roster_strip("roster_name_ran", "ラン", 3, 0xa0, 0x60, 48, 16),
];
