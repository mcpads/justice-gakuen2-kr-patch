//! Shared and mode-specific fixed prompt consumers in the SELP atlas.

use super::{FixedStripPlacement, FixedStripSpec, prompt_strip};
use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectTextureSurface,
};

pub(super) const PROMPT_STRIPS: &[FixedStripSpec] = &[
    prompt_strip(
        "combination_confirmation_prompt",
        &[],
        "この組み合わせでよろしいですか?",
        CharacterSelectFontRole::FixedPrompt,
        FixedStripPlacement {
            surface: CharacterSelectTextureSurface::LeagueTournamentPromptAtlas,
            texture_page_index: 2,
            texture_uv: [0, 216],
            size: [200, 20],
            preserve_source_background: false,
        },
    ),
    prompt_strip(
        "mode_retry_prompt",
        &[],
        "もう一回しますか?",
        CharacterSelectFontRole::FixedPrompt,
        FixedStripPlacement {
            surface: CharacterSelectTextureSurface::LeagueTournamentPromptAtlas,
            texture_page_index: 2,
            texture_uv: [0, 236],
            size: [163, 20],
            preserve_source_background: false,
        },
    ),
    prompt_strip(
        "tournament_shuffle_prompt",
        &[],
        "チームの並びをシャッフルしますか?",
        CharacterSelectFontRole::FixedPrompt,
        FixedStripPlacement {
            surface: CharacterSelectTextureSurface::TournamentPromptAtlas,
            texture_page_index: 3,
            texture_uv: [0, 72],
            size: [200, 24],
            preserve_source_background: false,
        },
    ),
    prompt_strip(
        "mode_retry_yes",
        &["tournament_shuffle_yes"],
        "はい",
        CharacterSelectFontRole::Label,
        FixedStripPlacement {
            surface: CharacterSelectTextureSurface::LeagueTournamentPromptAtlas,
            texture_page_index: 3,
            texture_uv: [32, 32],
            size: [64, 32],
            preserve_source_background: true,
        },
    ),
    prompt_strip(
        "mode_retry_no",
        &["tournament_shuffle_no"],
        "いいえ",
        CharacterSelectFontRole::Label,
        FixedStripPlacement {
            surface: CharacterSelectTextureSurface::LeagueTournamentPromptAtlas,
            texture_page_index: 3,
            texture_uv: [104, 32],
            size: [80, 32],
            preserve_source_background: true,
        },
    ),
];
