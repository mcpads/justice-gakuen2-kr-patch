//! Source-owned rows used by the battle-ready screen.

use anyhow::{Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextFlow, CharacterSelectTextSelection,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::SHARED_ATLAS_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const CONTROLLER_INSTRUCTION_ID: &str = "battle_ready_controller_instruction";
const CONTROLLER_INSTRUCTION_SOURCE_TEXT: &str = "使用するコントローラの";
const CONTROLLER_INSTRUCTION_OCCURRENCE_ID: &str = "selp-shared-ready-controller-instruction";
const CONTROLLER_INSTRUCTION_REGION_ID: &str = "ready-controller-instruction-source-region";
const START_PROMPT_ID: &str = "press_start_prompt";
const START_PROMPT_SOURCE_TEXT: &str = "STARTボタンを押してください!";
const START_PROMPT_OCCURRENCE_ID: &str = "selp-shared-ready-start-prompt";
const START_PROMPT_REGION_ID: &str = "ready-start-prompt-source-region";

const CONTROLLER_INSTRUCTION_WRITE_CELL: Cell = Cell {
    x: 512,
    y: 176,
    width: 200,
    height: 21,
};

const CONTROLLER_PANEL_CELL: Cell = Cell {
    x: 512,
    y: 176,
    width: 200,
    height: 40,
};

const START_PROMPT_WRITE_CELL: Cell = Cell {
    x: 512,
    y: 197,
    width: 200,
    height: 19,
};

const START_PROMPT_TEXT_CELL: Cell = Cell {
    x: 512,
    y: 200,
    width: 200,
    height: 16,
};

const ADJACENT_PLSEL3_CELL: Cell = Cell {
    x: 712,
    y: 200,
    width: 40,
    height: 16,
};

// Filled from the hash-pinned source cell. The guard deliberately covers indexed pixels,
// not a rendered preview.
const CONTROLLER_INSTRUCTION_WRITE_CELL_SHA256: &str =
    "2656024df5b30e6796e48e98c222f49ebd05ccc56f170fc72c4533627e7617be";
const CONTROLLER_PANEL_SOURCE_CELL_SHA256: &str =
    "75e136519a5dee27c7e328e6d7263ef2fef8f495a32bb701b8ec554d51908cc5";
const START_PROMPT_WRITE_CELL_SHA256: &str =
    "ad068c4c693d1dd60ae06f298d4a239489ecd32a0ff0e9d0b9cb1cb7fede8cd2";
const START_PROMPT_TEXT_CELL_SHA256: &str =
    "ab19979663400e0e0bb5b9f3c76f4cf565d97004b91bad641c0167a5609cb08d";
const ADJACENT_PLSEL3_CELL_SHA256: &str =
    "782b886bd72c88436705f4f47c07dabb39e1d054f8159ae490448e34f9208da9";

const BATTLE_READY_TARGETS: [(&str, &str, &[usize]); 3] = [
    (
        "DAT2/SELP3.BIZ",
        "DAT1/PLSEL3.BIN",
        &[0x8424, 0x842c, 0x8434, 0x8440],
    ),
    (
        "DAT2/SELP4.BIZ",
        "DAT1/PLSEL4.BIN",
        &[0x98f8, 0x9900, 0x9908, 0x9914],
    ),
    (
        "DAT2/SELP5.BIZ",
        "DAT1/PLSEL5.BIN",
        &[0xb0c0, 0xb0c8, 0xb0d0, 0xb0d8, 0xb0f0],
    ),
];

pub(super) struct BattleReadyPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_battle_ready_rows(
    shared_source_decoded: &[u8],
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<BattleReadyPlan> {
    let Some(entry) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == CONTROLLER_INSTRUCTION_ID)
    else {
        return Ok(BattleReadyPlan {
            fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    };
    ensure!(
        entry.source_text == CONTROLLER_INSTRUCTION_SOURCE_TEXT,
        "battle-ready controller instruction source identity changed"
    );
    ensure!(
        !entry.korean_text.trim().is_empty() && entry.korean_text.lines().count() == 1,
        "battle-ready controller instruction must fit its first source row"
    );
    let start_prompt = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == START_PROMPT_ID)
        .ok_or_else(|| anyhow::anyhow!("battle-ready screen lost its START prompt translation"))?;
    ensure!(
        start_prompt.source_text == START_PROMPT_SOURCE_TEXT,
        "battle-ready START prompt source identity changed"
    );
    ensure!(
        !start_prompt.korean_text.trim().is_empty()
            && start_prompt.korean_text.lines().count() == 1,
        "battle-ready START prompt must fit its second source row"
    );

    let source_pixels = read_indexed_cell_in_prefix(
        shared_source_decoded,
        SHARED_ATLAS_OFFSET,
        CONTROLLER_INSTRUCTION_WRITE_CELL,
    )?;
    let source_sha256 = sha256_bytes(&source_pixels);
    ensure!(
        source_sha256 == CONTROLLER_INSTRUCTION_WRITE_CELL_SHA256,
        "battle-ready controller instruction write cell changed: found {source_sha256}"
    );
    let panel_sha256 = sha256_bytes(&read_indexed_cell_in_prefix(
        shared_source_decoded,
        SHARED_ATLAS_OFFSET,
        CONTROLLER_PANEL_CELL,
    )?);
    ensure!(
        panel_sha256 == CONTROLLER_PANEL_SOURCE_CELL_SHA256,
        "battle-ready controller panel source footprint changed: found {panel_sha256}"
    );
    let start_prompt_write_sha256 = sha256_bytes(&read_indexed_cell_in_prefix(
        shared_source_decoded,
        SHARED_ATLAS_OFFSET,
        START_PROMPT_WRITE_CELL,
    )?);
    ensure!(
        start_prompt_write_sha256 == START_PROMPT_WRITE_CELL_SHA256,
        "battle-ready START prompt write cell changed: found {start_prompt_write_sha256}"
    );
    let start_prompt_text_sha256 = sha256_bytes(&read_indexed_cell_in_prefix(
        shared_source_decoded,
        SHARED_ATLAS_OFFSET,
        START_PROMPT_TEXT_CELL,
    )?);
    ensure!(
        start_prompt_text_sha256 == START_PROMPT_TEXT_CELL_SHA256,
        "battle-ready START prompt text cell changed: found {start_prompt_text_sha256}"
    );
    let adjacent_cell_sha256 = sha256_bytes(&read_indexed_cell_in_prefix(
        shared_source_decoded,
        SHARED_ATLAS_OFFSET,
        ADJACENT_PLSEL3_CELL,
    )?);
    ensure!(
        adjacent_cell_sha256 == ADJACENT_PLSEL3_CELL_SHA256,
        "battle-ready START prompt adjacent PLSEL3 cell changed: found {adjacent_cell_sha256}"
    );
    ensure!(
        CONTROLLER_INSTRUCTION_WRITE_CELL.y + CONTROLLER_INSTRUCTION_WRITE_CELL.height
            == START_PROMPT_WRITE_CELL.y
            && START_PROMPT_WRITE_CELL.y + START_PROMPT_WRITE_CELL.height
                == CONTROLLER_PANEL_CELL.y + CONTROLLER_PANEL_CELL.height,
        "battle-ready controller and START prompt writers no longer compose at the row boundary"
    );

    Ok(BattleReadyPlan {
        fixed_strips: vec![
            CharacterSelectFixedStripAllocation {
                physical_text_region_id: CONTROLLER_INSTRUCTION_REGION_ID.to_string(),
                source_ui_ids: vec![CONTROLLER_INSTRUCTION_ID.to_string()],
                translation_id: entry.translation_id.clone(),
                text_selection: CharacterSelectTextSelection::Entire,
                text_flow: CharacterSelectTextFlow::Horizontal,
                write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
                font_role: CharacterSelectFontRole::FixedPrompt,
                surface: CharacterSelectTextureSurface::BattleReadyAtlas,
                tim_offset: SHARED_ATLAS_OFFSET,
                cell: CONTROLLER_INSTRUCTION_WRITE_CELL,
                text_cell: CONTROLLER_INSTRUCTION_WRITE_CELL,
                clear_index: 0,
            },
            CharacterSelectFixedStripAllocation {
                physical_text_region_id: START_PROMPT_REGION_ID.to_string(),
                source_ui_ids: vec![START_PROMPT_ID.to_string()],
                translation_id: start_prompt.translation_id.clone(),
                text_selection: CharacterSelectTextSelection::Entire,
                text_flow: CharacterSelectTextFlow::Horizontal,
                write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
                font_role: CharacterSelectFontRole::CompactPrompt,
                surface: CharacterSelectTextureSurface::BattleReadyAtlas,
                tim_offset: SHARED_ATLAS_OFFSET,
                cell: START_PROMPT_WRITE_CELL,
                text_cell: START_PROMPT_TEXT_CELL,
                clear_index: 0,
            },
        ],
        occurrences: vec![
            battle_ready_occurrence(
                CONTROLLER_INSTRUCTION_OCCURRENCE_ID,
                CONTROLLER_INSTRUCTION_ID,
                CONTROLLER_INSTRUCTION_REGION_ID,
            ),
            battle_ready_occurrence(
                START_PROMPT_OCCURRENCE_ID,
                START_PROMPT_ID,
                START_PROMPT_REGION_ID,
            ),
        ],
    })
}

fn battle_ready_occurrence(
    occurrence_id: &'static str,
    source_ui_id: &'static str,
    physical_region_id: &'static str,
) -> CharacterSelectRouteOccurrence {
    bound_occurrence(
        occurrence_id,
        "selp_shared_ready_screen",
        [source_ui_id],
        BATTLE_READY_TARGETS
            .iter()
            .map(|(record, _, _)| {
                producer_target(
                    *record,
                    Some(SHARED_ATLAS_OFFSET),
                    Some(CharacterSelectTextureSurface::BattleReadyAtlas),
                    [physical_region_id],
                )
            })
            .collect(),
        BATTLE_READY_TARGETS
            .iter()
            .map(|(source_record, consumer_record, offsets)| {
                consumer_target(
                    *source_record,
                    *consumer_record,
                    CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                    offsets.iter().copied(),
                    ["battle-ready"],
                )
            })
            .collect(),
    )
}
