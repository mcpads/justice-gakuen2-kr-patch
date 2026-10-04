//! Source signatures and typed runtime bindings for each supported PLSEL overlay.

use psx_r3000a::Register;

use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectTextureSurface,
};

#[derive(Clone, Copy)]
pub(super) enum DescriptorEncoding {
    ColumnRow,
    DirectURow,
    DirectUv,
    DirectLabelGrid,
    CountedLabelGrid,
    ParticipantRuns,
    SelectionHelp,
}

impl DescriptorEncoding {
    pub(super) fn retains_spaces(self) -> bool {
        matches!(
            self,
            Self::ColumnRow | Self::DirectURow | Self::SelectionHelp
        )
    }
}

pub(super) struct DescriptorSpec {
    pub(super) route_occurrence_id: &'static str,
    pub(super) source_ui_id: &'static str,
    pub(super) font_role: CharacterSelectFontRole,
    pub(super) surface: CharacterSelectTextureSurface,
    pub(super) offset: usize,
    pub(super) expected: &'static [u8],
    pub(super) pointer_offset: usize,
    pub(super) pointer_register: Register,
    pub(super) pointer_immediate: i16,
    pub(super) encoding: DescriptorEncoding,
}

pub(super) struct OverlaySpec {
    pub(super) source_path: &'static str,
    pub(super) texture_source_path: &'static str,
    pub(super) descriptors: &'static [DescriptorSpec],
    pub(super) source_guards: &'static [SourceBytesGuard],
    pub(super) texture_pages: &'static [TexturePageSpec],
    pub(super) addiu_patches: &'static [AddiuPatch],
    pub(super) slot_count_patches: &'static [SltiPatch],
}

pub(super) struct TexturePageSpec {
    pub(super) source_word_x: i16,
    pub(super) offset: usize,
    pub(super) source_ui_id: &'static str,
}

const fn texture_page(offset: usize, source_ui_id: &'static str) -> TexturePageSpec {
    TexturePageSpec {
        source_word_x: 0x0340,
        offset,
        source_ui_id,
    }
}

pub(super) struct SourceBytesGuard {
    pub(super) role: &'static str,
    pub(super) activation_source_ui_id: Option<&'static str>,
    pub(super) offset: usize,
    pub(super) expected: &'static [u8],
}

pub(super) struct AddiuPatch {
    pub(super) activation_source_ui_id: Option<&'static str>,
    pub(super) offset: usize,
    pub(super) rt: Register,
    pub(super) rs: Register,
    pub(super) replacement_rs: Register,
    pub(super) expected_immediate: i16,
    pub(super) replacement_immediate: i16,
}

pub(super) struct SltiPatch {
    pub(super) activation_source_ui_id: Option<&'static str>,
    pub(super) offset: usize,
    pub(super) rt: Register,
    pub(super) rs: Register,
    pub(super) expected_immediate: i16,
    pub(super) replacement_immediate: i16,
}

const COMMON_SOURCE: &[u8] = &[
    0x0a, 0x01, 0x02, 0x02, 0x02, 0x03, 0x02, 0x04, 0x02, 0x05, 0x02, 0x02, 0x01, 0x06, 0x02, 0x07,
    0x02, 0x04, 0x02, 0x00, 0x03,
];
const VERSUS_SOURCE: &[u8] = &[0x03, 0x05, 0x00, 0x00, 0x00, 0x04, 0x00];
const TEAM_SOURCE: &[u8] = &[0x03, 0x06, 0x00, 0x07, 0x00, 0x04, 0x00];
const LEAGUE_SOURCE: &[u8] = &[0x04, 0x01, 0x01, 0x02, 0x01, 0x03, 0x01, 0x04, 0x00];
const TOURNAMENT_SOURCE: &[u8] = &[
    0x07, 0x00, 0x03, 0x02, 0x01, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x03, 0x04, 0x00,
];
const COOPERATIVE_SOURCE: &[u8] = &[0x03, 0xa8, 0x07, 0xc8, 0x07, 0x80, 0x00];
const ALTERNATE_VERSUS_SOURCE: &[u8] = &[0x03, 0xa0, 0x00, 0x00, 0x00, 0x80, 0x00];
const WIN_COUNT_SOURCE: &[u8] = &[0x08, 0x0a, 0x09, 0x0a, 0x03, 0x05];
const HANDICAP_SOURCE: &[u8] = &[
    0x07, 0x00, 0x04, 0x04, 0x03, 0x01, 0x04, 0x02, 0x04, 0x03, 0x04, 0x04, 0x04, 0x05, 0x04,
];
const BATTLE_READY_PLSEL3_SOURCE: &[u8] = &[
    0x60, 0x94, 0x74, 0x94, 0xc4, 0x80, 0xd8, 0x80, 0x88, 0x94, 0x00, 0x00,
];
const BATTLE_READY_PLSEL4_SOURCE: &[u8] = &[
    0x60, 0x94, 0x74, 0x94, 0xc4, 0x80, 0xd8, 0x80, 0x88, 0x94, 0x04, 0x00,
];
const BATTLE_READY_PLSEL5_SOURCE: &[u8] = BATTLE_READY_PLSEL3_SOURCE;
const TEAM_PARTICIPANT_SOURCE: &[u8] =
    &[0x03, 0x06, 0x05, 0x04, 0x07, 0x01, 0x04, 0x06, 0x01, 0x01];
const LEAGUE_PARTICIPANT_SOURCE: &[u8] = &[
    0x08, 0x06, 0x05, 0x07, 0x05, 0x08, 0x05, 0x09, 0x05, 0x07, 0x01, 0x08, 0x01, 0x09, 0x01, 0x0a,
    0x01,
];
const SELECTION_HELP_SOURCE: &[u8] = &[
    0x0d, 0x01, 0x11, 0x00, 0x06, 0x01, 0x11, 0x00, 0x0f, 0x00, 0x0e, 0x01, 0x0c, 0x07, 0x04, 0x05,
    0x0d, 0x06, 0x00, 0x04, 0x08, 0x04, 0x00, 0x04, 0x0e, 0x00, 0x0d, 0x00, 0x0f, 0x00, 0x05, 0x01,
];
const PLSEL1_DESCRIPTORS: &[DescriptorSpec] = &[
    DescriptorSpec {
        route_occurrence_id: "plsel1-character-select-heading",
        source_ui_id: "character_select_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x056c,
        expected: COMMON_SOURCE,
        pointer_offset: 0x1ee0,
        pointer_register: Register::S2,
        pointer_immediate: 0x256c,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel1-versus-heading",
        source_ui_id: "versus_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x0584,
        expected: VERSUS_SOURCE,
        pointer_offset: 0x1ef4,
        pointer_register: Register::S2,
        pointer_immediate: 0x2584,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel1-win-count-label",
        source_ui_id: "win_count_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::VersusLabelAtlas,
        offset: 0x058c,
        expected: WIN_COUNT_SOURCE,
        pointer_offset: 0x2df4,
        pointer_register: Register::S2,
        pointer_immediate: 0x258c,
        encoding: DescriptorEncoding::DirectLabelGrid,
    },
    DescriptorSpec {
        route_occurrence_id: "selp1-versus-handicap-label",
        source_ui_id: "versus_handicap_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::VersusHandicapAtlas,
        offset: 0x0594,
        expected: HANDICAP_SOURCE,
        pointer_offset: 0x35dc,
        pointer_register: Register::S2,
        pointer_immediate: 0x2594,
        encoding: DescriptorEncoding::CountedLabelGrid,
    },
];

const PLSEL2_DESCRIPTORS: &[DescriptorSpec] = &[
    DescriptorSpec {
        route_occurrence_id: "plsel2-team-battle-heading",
        source_ui_id: "team_battle_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x052c,
        expected: TEAM_SOURCE,
        pointer_offset: 0x178c,
        pointer_register: Register::S2,
        pointer_immediate: 0x252c,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel2-team-participant-count-prompt",
        source_ui_id: "team_participant_count_prompt",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::ParticipantLabelAtlas,
        offset: 0x0534,
        expected: TEAM_PARTICIPANT_SOURCE,
        pointer_offset: 0x1980,
        pointer_register: Register::S2,
        pointer_immediate: 0x2534,
        encoding: DescriptorEncoding::ParticipantRuns,
    },
];

const PLSEL3_DESCRIPTORS: &[DescriptorSpec] = &[
    DescriptorSpec {
        route_occurrence_id: "plsel3-selection-back-help",
        source_ui_id: "selection_back_help",
        font_role: CharacterSelectFontRole::SelectionHelp,
        surface: CharacterSelectTextureSurface::SelectionHelpAtlas,
        offset: 0x04a4,
        expected: SELECTION_HELP_SOURCE,
        pointer_offset: 0x2538,
        pointer_register: Register::S2,
        pointer_immediate: 0x24a4,
        encoding: DescriptorEncoding::SelectionHelp,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel3-league-heading",
        source_ui_id: "league_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x046c,
        expected: LEAGUE_SOURCE,
        pointer_offset: 0x1b64,
        pointer_register: Register::S2,
        pointer_immediate: 0x246c,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel3-character-select-heading",
        source_ui_id: "character_select_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x0478,
        expected: COMMON_SOURCE,
        pointer_offset: 0x1d58,
        pointer_register: Register::S2,
        pointer_immediate: 0x2478,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel3-league-participant-count-prompt",
        source_ui_id: "league_participant_count_prompt",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::ParticipantLabelAtlas,
        offset: 0x0490,
        expected: LEAGUE_PARTICIPANT_SOURCE,
        pointer_offset: 0x1f2c,
        pointer_register: Register::S2,
        pointer_immediate: 0x2490,
        encoding: DescriptorEncoding::CountedLabelGrid,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel3-battle-ready-label",
        source_ui_id: "battle_ready_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::BattleReadyAtlas,
        offset: 0x0594,
        expected: BATTLE_READY_PLSEL3_SOURCE,
        pointer_offset: 0x8618,
        pointer_register: Register::S2,
        pointer_immediate: 0x2594,
        encoding: DescriptorEncoding::DirectUv,
    },
];

const PLSEL4_DESCRIPTORS: &[DescriptorSpec] = &[
    DescriptorSpec {
        route_occurrence_id: "plsel4-tournament-heading",
        source_ui_id: "tournament_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x0494,
        expected: TOURNAMENT_SOURCE,
        pointer_offset: 0x26b0,
        pointer_register: Register::S2,
        pointer_immediate: 0x2494,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel4-character-select-heading",
        source_ui_id: "character_select_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x04a4,
        expected: COMMON_SOURCE,
        pointer_offset: 0x28a4,
        pointer_register: Register::S2,
        pointer_immediate: 0x24a4,
        encoding: DescriptorEncoding::ColumnRow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel4-selection-back-help",
        source_ui_id: "selection_back_help",
        font_role: CharacterSelectFontRole::SelectionHelp,
        surface: CharacterSelectTextureSurface::SelectionHelpAtlas,
        offset: 0x0474,
        expected: SELECTION_HELP_SOURCE,
        pointer_offset: 0x1eb0,
        pointer_register: Register::S2,
        pointer_immediate: 0x2474,
        encoding: DescriptorEncoding::SelectionHelp,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel4-battle-ready-label",
        source_ui_id: "battle_ready_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::BattleReadyAtlas,
        offset: 0x0834,
        expected: BATTLE_READY_PLSEL4_SOURCE,
        pointer_offset: 0x9aec,
        pointer_register: Register::S2,
        pointer_immediate: 0x2834,
        encoding: DescriptorEncoding::DirectUv,
    },
];

const PLSEL5_DESCRIPTORS: &[DescriptorSpec] = &[
    DescriptorSpec {
        route_occurrence_id: "plsel5-cooperative-heading",
        source_ui_id: "cooperative_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x3704,
        expected: COOPERATIVE_SOURCE,
        pointer_offset: 0x8168,
        pointer_register: Register::S2,
        pointer_immediate: 0x5704,
        encoding: DescriptorEncoding::DirectURow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel5-versus-heading",
        source_ui_id: "versus_heading",
        font_role: CharacterSelectFontRole::SelectHeading,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        offset: 0x370c,
        expected: ALTERNATE_VERSUS_SOURCE,
        pointer_offset: 0x81a0,
        pointer_register: Register::S2,
        pointer_immediate: 0x570c,
        encoding: DescriptorEncoding::DirectURow,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel5-win-count-label",
        source_ui_id: "win_count_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::VersusLabelAtlas,
        offset: 0x371c,
        expected: WIN_COUNT_SOURCE,
        pointer_offset: 0x91b8,
        pointer_register: Register::S2,
        pointer_immediate: 0x571c,
        encoding: DescriptorEncoding::DirectLabelGrid,
    },
    DescriptorSpec {
        route_occurrence_id: "plsel5-battle-ready-label",
        source_ui_id: "battle_ready_label",
        font_role: CharacterSelectFontRole::Label,
        surface: CharacterSelectTextureSurface::BattleReadyAtlas,
        offset: 0x378c,
        expected: BATTLE_READY_PLSEL5_SOURCE,
        pointer_offset: 0xb2c4,
        pointer_register: Register::S2,
        pointer_immediate: 0x578c,
        encoding: DescriptorEncoding::DirectUv,
    },
];

const PLSEL3_ADDUI_PATCHES: &[AddiuPatch] = &[AddiuPatch {
    activation_source_ui_id: Some("battle_ready_label"),
    offset: 0x8638,
    rt: Register::S4,
    rs: Register::ZERO,
    replacement_rs: Register::ZERO,
    expected_immediate: 0x00ce,
    replacement_immediate: super::super::ready_layout::SCREEN_X,
}];

const PLSEL5_ADDUI_PATCHES: &[AddiuPatch] = &[AddiuPatch {
    activation_source_ui_id: Some("battle_ready_label"),
    offset: 0xb2e4,
    rt: Register::S4,
    rs: Register::ZERO,
    replacement_rs: Register::ZERO,
    expected_immediate: 0x00ce,
    replacement_immediate: super::super::ready_layout::SCREEN_X,
}];

const PLSEL4_ADDUI_PATCHES: &[AddiuPatch] = &[
    AddiuPatch {
        activation_source_ui_id: Some("battle_ready_label"),
        offset: 0x9b0c,
        rt: Register::S4,
        rs: Register::ZERO,
        replacement_rs: Register::ZERO,
        expected_immediate: 0x00ce,
        replacement_immediate: super::super::ready_layout::SCREEN_X,
    },
    AddiuPatch {
        activation_source_ui_id: Some("tournament_certificate_body"),
        offset: 0x7edc,
        rt: Register::V0,
        rs: Register::ZERO,
        replacement_rs: Register::ZERO,
        expected_immediate: 0x0146,
        replacement_immediate: super::super::allocation::TEAM_MARKER_SCREEN_X,
    },
    AddiuPatch {
        activation_source_ui_id: Some("tournament_certificate_body"),
        offset: 0x7ee4,
        rt: Register::V0,
        rs: Register::ZERO,
        replacement_rs: Register::ZERO,
        expected_immediate: 0x00e0,
        replacement_immediate: super::super::allocation::TEAM_MARKER_SCREEN_Y,
    },
    // Append the marker after the final paper packet in the native chain,
    // rather than into the retry dialog's OT bucket. Both double buffers use
    // marker base 0x3978 and first paper packet tag 0x3860. Native CatPrim
    // merges the adjacent GPU commands into that first tag; +8 is payload.
    AddiuPatch {
        activation_source_ui_id: Some("tournament_certificate_body"),
        offset: 0x7f78,
        rt: Register::A0,
        rs: Register::A0,
        replacement_rs: Register::S0,
        expected_immediate: 0x1058,
        replacement_immediate: 0x3860 - 0x3978,
    },
];

const PLSEL3_SLOT_COUNT_PATCHES: &[SltiPatch] = &[SltiPatch {
    activation_source_ui_id: Some("battle_ready_label"),
    offset: 0x8774,
    rt: Register::V0,
    rs: Register::S5,
    expected_immediate: 0x0005,
    replacement_immediate: 0x0006,
}];

const PLSEL4_SLOT_COUNT_PATCHES: &[SltiPatch] = &[SltiPatch {
    activation_source_ui_id: Some("battle_ready_label"),
    offset: 0x9c48,
    rt: Register::V0,
    rs: Register::S5,
    expected_immediate: 0x0005,
    replacement_immediate: 0x0006,
}];

const PLSEL5_SLOT_COUNT_PATCHES: &[SltiPatch] = &[SltiPatch {
    activation_source_ui_id: Some("battle_ready_label"),
    offset: 0xb420,
    rt: Register::V0,
    rs: Register::S5,
    expected_immediate: 0x0005,
    replacement_immediate: 0x0006,
}];

const TOURNAMENT_CERTIFICATE_SLICE_LAYOUT: &[u8] = &[
    0x08, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
    0xbe, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x08, 0x00, 0x01, 0x00, 0x00, 0x01, 0x89, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x09, 0x00, 0x01, 0x00, 0xbe, 0x00, 0x89, 0x00, 0x00, 0x01, 0x00, 0x01,
];
const TOURNAMENT_TEAM_MARKER_UVS: &[u8] = &[
    0xbf, 0x00, 0x00, 0x00, 0xd0, 0x00, 0x00, 0x00, 0xbf, 0x00, 0x20, 0x00, 0xd0, 0x00, 0x20, 0x00,
    0xbf, 0x00, 0x40, 0x00, 0xd0, 0x00, 0x40, 0x00, 0xbf, 0x00, 0x60, 0x00, 0xd0, 0x00, 0x60, 0x00,
];
const TOURNAMENT_BRACKET_LEFT_TPAGE: &[u8] = &[0x40, 0x03, 0x06, 0x24, 0x21, 0x38, 0x00, 0x00];
const TOURNAMENT_BRACKET_LEFT_SPRITE: &[u8] = &[
    0x80, 0x00, 0x10, 0x24, 0x16, 0x00, 0xc2, 0xa6, 0x48, 0x00, 0x02, 0x24, 0x14, 0x00, 0xc2, 0xa2,
    0xe0, 0x00, 0x02, 0x24, 0x20, 0x00, 0x11, 0x24, 0x3e, 0x00, 0x12, 0x24, 0x0c, 0x00, 0xd0, 0xa2,
    0x0d, 0x00, 0xd0, 0xa2, 0x0e, 0x00, 0xd0, 0xa2, 0x15, 0x00, 0xc2, 0xa2, 0x18, 0x00, 0xd1, 0xa6,
    0x1a, 0x00, 0xd1, 0xa6, 0x10, 0x00, 0xd5, 0xa6, 0x12, 0x00, 0xd2, 0xa6,
];
const TOURNAMENT_BRACKET_RIGHT_TPAGE: &[u8] = &[0x40, 0x03, 0x06, 0x24];
const TOURNAMENT_BRACKET_RIGHT_SPRITE: &[u8] = &[
    0x16, 0x00, 0xc2, 0xa6, 0x20, 0x01, 0x02, 0x24, 0x0c, 0x00, 0xd0, 0xa2, 0x0d, 0x00, 0xd0, 0xa2,
    0x0e, 0x00, 0xd0, 0xa2, 0x14, 0x00, 0xc0, 0xa2, 0x15, 0x00, 0xc0, 0xa2, 0x18, 0x00, 0xd1, 0xa6,
    0x1a, 0x00, 0xd1, 0xa6, 0x10, 0x00, 0xc2, 0xa6, 0x12, 0x00, 0xd2, 0xa6,
];
const BATTLE_READY_PANEL_V: &[u8] = &[0xb0, 0x00, 0x02, 0x24, 0x15, 0x00, 0x02, 0xa2];
const BATTLE_READY_PANEL_WIDTH: &[u8] = &[0xc8, 0x00, 0x02, 0x24, 0x18, 0x00, 0x02, 0xa6];
const BATTLE_READY_PANEL_HEIGHT: &[u8] = &[0x28, 0x00, 0x02, 0x24, 0x1a, 0x00, 0x02, 0xa6];
const BATTLE_READY_PANEL_HEIGHT_DELAYED_STORE: &[u8] = &[
    0x28, 0x00, 0x02, 0x24, 0x01, 0x00, 0x63, 0x32, 0x1a, 0x00, 0x02, 0xa6,
];
const BATTLE_READY_PANEL_U: &[u8] = &[0x14, 0x00, 0x00, 0xa2];

const PLSEL3_SOURCE_GUARDS: &[SourceBytesGuard] =
    &battle_ready_panel_guards([0x8424, 0x842c, 0x8434, 0x8440]);

const PLSEL4_SOURCE_GUARDS: &[SourceBytesGuard] = &[
    battle_ready_panel_guard("battle-ready panel V", 0x98f8, BATTLE_READY_PANEL_V),
    battle_ready_panel_guard("battle-ready panel width", 0x9900, BATTLE_READY_PANEL_WIDTH),
    battle_ready_panel_guard(
        "battle-ready panel height",
        0x9908,
        BATTLE_READY_PANEL_HEIGHT,
    ),
    battle_ready_panel_guard("battle-ready panel U", 0x9914, BATTLE_READY_PANEL_U),
    SourceBytesGuard {
        role: "tournament certificate 446x393 slice layout",
        activation_source_ui_id: Some("tournament_certificate_body"),
        offset: 0x0780,
        expected: TOURNAMENT_CERTIFICATE_SLICE_LAYOUT,
    },
    SourceBytesGuard {
        role: "tournament A-H team marker UV table",
        activation_source_ui_id: Some("tournament_certificate_body"),
        offset: 0x07b0,
        expected: TOURNAMENT_TEAM_MARKER_UVS,
    },
    SourceBytesGuard {
        role: "completed bracket left champion texture page",
        activation_source_ui_id: Some("tournament_champion_label"),
        offset: 0x6990,
        expected: TOURNAMENT_BRACKET_LEFT_TPAGE,
    },
    SourceBytesGuard {
        role: "completed bracket left champion sprite",
        activation_source_ui_id: Some("tournament_champion_label"),
        offset: 0x6a44,
        expected: TOURNAMENT_BRACKET_LEFT_SPRITE,
    },
    SourceBytesGuard {
        role: "completed bracket right champion texture page",
        activation_source_ui_id: Some("tournament_champion_label"),
        offset: 0x6ac4,
        expected: TOURNAMENT_BRACKET_RIGHT_TPAGE,
    },
    SourceBytesGuard {
        role: "completed bracket right champion sprite",
        activation_source_ui_id: Some("tournament_champion_label"),
        offset: 0x6b54,
        expected: TOURNAMENT_BRACKET_RIGHT_SPRITE,
    },
];

const PLSEL5_SOURCE_GUARDS: &[SourceBytesGuard] = &[
    battle_ready_panel_guard("battle-ready panel V", 0xb0c0, BATTLE_READY_PANEL_V),
    battle_ready_panel_guard("battle-ready panel width", 0xb0c8, BATTLE_READY_PANEL_WIDTH),
    battle_ready_panel_guard(
        "battle-ready panel height with delayed store",
        0xb0d0,
        BATTLE_READY_PANEL_HEIGHT_DELAYED_STORE,
    ),
    battle_ready_panel_guard("battle-ready panel U", 0xb0f0, BATTLE_READY_PANEL_U),
];

const fn battle_ready_panel_guards(offsets: [usize; 4]) -> [SourceBytesGuard; 4] {
    [
        battle_ready_panel_guard("battle-ready panel V", offsets[0], BATTLE_READY_PANEL_V),
        battle_ready_panel_guard(
            "battle-ready panel width",
            offsets[1],
            BATTLE_READY_PANEL_WIDTH,
        ),
        battle_ready_panel_guard(
            "battle-ready panel height",
            offsets[2],
            BATTLE_READY_PANEL_HEIGHT,
        ),
        battle_ready_panel_guard("battle-ready panel U", offsets[3], BATTLE_READY_PANEL_U),
    ]
}

const fn battle_ready_panel_guard(
    role: &'static str,
    offset: usize,
    expected: &'static [u8],
) -> SourceBytesGuard {
    SourceBytesGuard {
        role,
        activation_source_ui_id: Some("battle_ready_controller_instruction"),
        offset,
        expected,
    }
}

pub(super) const OVERLAYS: &[OverlaySpec] = &[
    OverlaySpec {
        source_path: "DAT1/PLSEL1.BIN",
        texture_source_path: "DAT2/SELP1.BIZ",
        descriptors: PLSEL1_DESCRIPTORS,
        source_guards: &[],
        texture_pages: &[texture_page(0x1f60, "character_select_heading")],
        addiu_patches: &[],
        slot_count_patches: &[],
    },
    OverlaySpec {
        source_path: "DAT1/PLSEL2.BIN",
        texture_source_path: "DAT2/SELP2.BIZ",
        descriptors: PLSEL2_DESCRIPTORS,
        source_guards: &[],
        texture_pages: &[texture_page(0x17e8, "team_battle_heading")],
        addiu_patches: &[],
        slot_count_patches: &[],
    },
    OverlaySpec {
        source_path: "DAT1/PLSEL3.BIN",
        texture_source_path: "DAT2/SELP3.BIZ",
        descriptors: PLSEL3_DESCRIPTORS,
        source_guards: PLSEL3_SOURCE_GUARDS,
        texture_pages: &[
            TexturePageSpec {
                offset: 0x8658,
                source_ui_id: "battle_ready_label",
                source_word_x: 0x0380,
            },
            texture_page(0x1bc0, "league_heading"),
            texture_page(0x1dac, "character_select_heading"),
            texture_page(0x2578, "selection_back_help"),
        ],
        addiu_patches: PLSEL3_ADDUI_PATCHES,
        slot_count_patches: PLSEL3_SLOT_COUNT_PATCHES,
    },
    OverlaySpec {
        source_path: "DAT1/PLSEL4.BIN",
        texture_source_path: "DAT2/SELP4.BIZ",
        descriptors: PLSEL4_DESCRIPTORS,
        source_guards: PLSEL4_SOURCE_GUARDS,
        texture_pages: &[
            TexturePageSpec {
                offset: 0x9b2c,
                source_ui_id: "battle_ready_label",
                source_word_x: 0x0380,
            },
            texture_page(0x1ef0, "selection_back_help"),
            texture_page(0x270c, "tournament_heading"),
            texture_page(0x28f8, "character_select_heading"),
        ],
        addiu_patches: PLSEL4_ADDUI_PATCHES,
        slot_count_patches: PLSEL4_SLOT_COUNT_PATCHES,
    },
    OverlaySpec {
        source_path: "DAT1/PLSEL5.BIN",
        texture_source_path: "DAT2/SELP5.BIZ",
        descriptors: PLSEL5_DESCRIPTORS,
        source_guards: PLSEL5_SOURCE_GUARDS,
        texture_pages: &[
            texture_page(0x821c, "cooperative_heading"),
            TexturePageSpec {
                offset: 0xb304,
                source_ui_id: "battle_ready_label",
                source_word_x: 0x0380,
            },
        ],
        addiu_patches: PLSEL5_ADDUI_PATCHES,
        slot_count_patches: PLSEL5_SLOT_COUNT_PATCHES,
    },
];
