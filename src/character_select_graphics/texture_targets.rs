use super::model::CharacterSelectTextureSurface;

pub(super) const SHARED_ATLAS_OFFSET: usize = 0x17800;

pub(super) const SELP1_PATH: &str = "DAT2/SELP1.BIZ";
pub(super) const SELP2_PATH: &str = "DAT2/SELP2.BIZ";
pub(super) const SELP3_PATH: &str = "DAT2/SELP3.BIZ";
pub(super) const SELP4_PATH: &str = "DAT2/SELP4.BIZ";
pub(super) const SELP5_PATH: &str = "DAT2/SELP5.BIZ";
pub(super) const AISYOU_PATH: &str = "DAT2/AISYOU.TIZ";
pub(super) const OVER_PATH: &str = "DAT2/OVER.TIZ";
pub(super) const OP01_PATH: &str = "DAT2/OP01.BIZ";
pub(super) const TITLE_PATH: &str = "DAT2/TITLE.BIN";
pub(super) const SOLO_STORY_INTRO_TIM_OFFSET: usize = 0x3c800;
pub(super) const SOLO_EPISODE_CARD_TIM_OFFSETS: [usize; 8] = [
    0x00000, 0x04840, 0x08080, 0x0c0c0, 0x0f900, 0x13940, 0x19180, 0x1d1c0,
];
pub(super) const SOLO_STATE_PROMPT_TIM_OFFSET: usize = 0;
pub(super) const COOPERATIVE_BACKGROUND_TIM_OFFSET: usize = 0x6800;
pub(super) const COOPERATIVE_MODE_MENU_TIM_OFFSET: usize = 0x45000;
pub(super) const TOURNAMENT_CERTIFICATE_TIM_OFFSETS: [usize; 2] = [0x00000, 0x4a000];
pub(super) const TOROFY_PATH: &str = "DAT2/TOROFY.BIZ";

#[derive(Clone, Copy, PartialEq, Eq)]
struct CharacterSelectTextureTarget {
    source_path: &'static str,
    tim_offset: usize,
}

const fn shared_atlas(source_path: &'static str) -> CharacterSelectTextureTarget {
    CharacterSelectTextureTarget {
        source_path,
        tim_offset: SHARED_ATLAS_OFFSET,
    }
}

const ALL_SHARED_ATLASES: [CharacterSelectTextureTarget; 5] = [
    shared_atlas(SELP1_PATH),
    shared_atlas(SELP2_PATH),
    shared_atlas(SELP3_PATH),
    shared_atlas(SELP4_PATH),
    shared_atlas(SELP5_PATH),
];
const VERSUS_LABEL_ATLASES: [CharacterSelectTextureTarget; 2] =
    [shared_atlas(SELP1_PATH), shared_atlas(SELP5_PATH)];
const VERSUS_HANDICAP_ATLAS: [CharacterSelectTextureTarget; 1] = [shared_atlas(SELP1_PATH)];
const PARTICIPANT_LABEL_ATLASES: [CharacterSelectTextureTarget; 2] =
    [shared_atlas(SELP2_PATH), shared_atlas(SELP3_PATH)];
const PRACTICAL_SELECTION_ATLAS: [CharacterSelectTextureTarget; 1] = [shared_atlas(SELP1_PATH)];
const STAGE_LABEL_ATLAS: [CharacterSelectTextureTarget; 1] = [shared_atlas(SELP1_PATH)];
const LEAGUE_STANDINGS_ATLAS: [CharacterSelectTextureTarget; 1] = [shared_atlas(SELP3_PATH)];
const SELECTION_HELP_ATLAS: [CharacterSelectTextureTarget; 2] =
    [shared_atlas(SELP3_PATH), shared_atlas(SELP4_PATH)];
const TOURNAMENT_BRACKET_LABEL_ATLAS: [CharacterSelectTextureTarget; 1] =
    [shared_atlas(SELP4_PATH)];
const LEAGUE_TOURNAMENT_PROMPT_ATLASES: [CharacterSelectTextureTarget; 2] =
    [shared_atlas(SELP3_PATH), shared_atlas(SELP4_PATH)];
const BATTLE_READY_ATLASES: [CharacterSelectTextureTarget; 3] = [
    shared_atlas(SELP3_PATH),
    shared_atlas(SELP4_PATH),
    shared_atlas(SELP5_PATH),
];
const TOURNAMENT_PROMPT_ATLAS: [CharacterSelectTextureTarget; 1] = [shared_atlas(SELP4_PATH)];
const COOPERATIVE_MODE_MENU_ATLAS: [CharacterSelectTextureTarget; 1] =
    [CharacterSelectTextureTarget {
        source_path: AISYOU_PATH,
        tim_offset: COOPERATIVE_MODE_MENU_TIM_OFFSET,
    }];
const COOPERATIVE_BACKGROUND_EMBLEM: [CharacterSelectTextureTarget; 1] =
    [CharacterSelectTextureTarget {
        source_path: SELP5_PATH,
        tim_offset: COOPERATIVE_BACKGROUND_TIM_OFFSET,
    }];
const SOLO_STATE_PROMPT_ATLAS: [CharacterSelectTextureTarget; 1] = [CharacterSelectTextureTarget {
    source_path: OVER_PATH,
    tim_offset: SOLO_STATE_PROMPT_TIM_OFFSET,
}];
const COMMON_PAUSE_MENU_ATLAS: [CharacterSelectTextureTarget; 1] = [CharacterSelectTextureTarget {
    source_path: OVER_PATH,
    tim_offset: SOLO_STATE_PROMPT_TIM_OFFSET,
}];
const SOLO_STORY_INTRO_ATLAS: [CharacterSelectTextureTarget; 1] = [CharacterSelectTextureTarget {
    source_path: OP01_PATH,
    tim_offset: SOLO_STORY_INTRO_TIM_OFFSET,
}];
const SOLO_EPISODE_CARD_ATLAS: [CharacterSelectTextureTarget; 8] = [
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[0],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[1],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[2],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[3],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[4],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[5],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[6],
    },
    CharacterSelectTextureTarget {
        source_path: TITLE_PATH,
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[7],
    },
];
const TOURNAMENT_CERTIFICATE: [CharacterSelectTextureTarget; 2] = [
    CharacterSelectTextureTarget {
        source_path: TOROFY_PATH,
        tim_offset: TOURNAMENT_CERTIFICATE_TIM_OFFSETS[0],
    },
    CharacterSelectTextureTarget {
        source_path: TOROFY_PATH,
        tim_offset: TOURNAMENT_CERTIFICATE_TIM_OFFSETS[1],
    },
];

impl CharacterSelectTextureSurface {
    fn physical_targets(self) -> &'static [CharacterSelectTextureTarget] {
        match self {
            Self::SharedSelectAtlas | Self::SharedFixedStripAtlas => &ALL_SHARED_ATLASES,
            Self::BattleReadyAtlas => &BATTLE_READY_ATLASES,
            Self::LeagueTournamentPromptAtlas => &LEAGUE_TOURNAMENT_PROMPT_ATLASES,
            Self::TournamentPromptAtlas => &TOURNAMENT_PROMPT_ATLAS,
            Self::CooperativeModeMenuAtlas => &COOPERATIVE_MODE_MENU_ATLAS,
            Self::CooperativeBackgroundEmblem => &COOPERATIVE_BACKGROUND_EMBLEM,
            Self::TournamentBracketLabelAtlas => &TOURNAMENT_BRACKET_LABEL_ATLAS,
            Self::TournamentCertificate => &TOURNAMENT_CERTIFICATE,
            Self::VersusLabelAtlas => &VERSUS_LABEL_ATLASES,
            Self::VersusHandicapAtlas => &VERSUS_HANDICAP_ATLAS,
            Self::ParticipantLabelAtlas => &PARTICIPANT_LABEL_ATLASES,
            Self::PracticalSelectionAtlas => &PRACTICAL_SELECTION_ATLAS,
            Self::StageLabelAtlas => &STAGE_LABEL_ATLAS,
            Self::LeagueStandingsAtlas => &LEAGUE_STANDINGS_ATLAS,
            Self::SelectionHelpAtlas => &SELECTION_HELP_ATLAS,
            Self::SoloStatePromptAtlas => &SOLO_STATE_PROMPT_ATLAS,
            Self::CommonPauseMenuAtlas => &COMMON_PAUSE_MENU_ATLAS,
            Self::SoloStoryIntroAtlas => &SOLO_STORY_INTRO_ATLAS,
            Self::SoloEpisodeCardAtlas => &SOLO_EPISODE_CARD_ATLAS,
        }
    }

    pub(super) fn targets_record(self, source_path: &str, tim_offset: usize) -> bool {
        self.physical_targets()
            .iter()
            .any(|target| target.source_path == source_path && target.tim_offset == tim_offset)
    }

    pub(super) fn shares_physical_texture(self, other: Self) -> bool {
        self.physical_targets()
            .iter()
            .any(|left| other.physical_targets().contains(left))
    }
}
