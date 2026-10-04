//! Source-owned cooperative mode-menu graphics loaded from `AISYOU.TIZ`.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectSourceCellOccupancy,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::resource_loads::resource_load_byte_offsets;
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, pending_occurrence, producer_target,
};
use crate::character_select_graphics::texture_targets::COOPERATIVE_MODE_MENU_TIM_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, cells_overlap, read_indexed_cell_in_prefix};

const SOURCE_TIM_SHA256: &str = "f09e0d8386ff595f75415b0f542ebf8b84840ed908c85723d57b49a2db72abd7";

const MODE_MENU_ENTRIES: [(&str, &str); 4] = [
    ("cooperative_mode_menu_heading", "協力戦"),
    ("cooperative_compatibility_mode", "相性診断モード"),
    ("cooperative_versus_mode", "対戦モード"),
    ("cooperative_return_to_mode_menu", "モードメニューに戻る"),
];

const HEADING_CELLS: [Cell; 3] = [
    Cell {
        x: 144,
        y: 0,
        width: 40,
        height: 40,
    },
    Cell {
        x: 184,
        y: 0,
        width: 40,
        height: 40,
    },
    Cell {
        x: 144,
        y: 40,
        width: 40,
        height: 40,
    },
];

const LABEL_STRIPS: [(&str, Cell); 3] = [
    (
        "cooperative_compatibility_mode",
        Cell {
            x: 0,
            y: 128,
            width: 168,
            height: 32,
        },
    ),
    (
        "cooperative_versus_mode",
        Cell {
            x: 0,
            y: 160,
            width: 122,
            height: 32,
        },
    ),
    (
        "cooperative_return_to_mode_menu",
        Cell {
            x: 0,
            y: 96,
            width: 238,
            height: 32,
        },
    ),
];

pub(super) struct CooperativeModeMenuPlan {
    pub(super) entry_count: usize,
    pub(super) rendered_source_ui_ids: Vec<String>,
    pub(super) glyphs: Vec<CharacterSelectGlyphAllocation>,
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_cooperative_mode_menu(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<CooperativeModeMenuPlan> {
    let entries = MODE_MENU_ENTRIES
        .iter()
        .filter_map(|(source_ui_id, source_text)| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == *source_ui_id)
                .map(|entry| (*source_ui_id, *source_text, entry))
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Ok(CooperativeModeMenuPlan {
            entry_count: 0,
            rendered_source_ui_ids: Vec::new(),
            glyphs: Vec::new(),
            fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }
    ensure!(
        entries.len() == MODE_MENU_ENTRIES.len(),
        "cooperative mode-menu inventory changed: expected {}, found {}",
        MODE_MENU_ENTRIES.len(),
        entries.len()
    );
    for (source_ui_id, source_text, entry) in &entries {
        ensure!(
            entry.source_text == *source_text,
            "cooperative mode-menu source identity changed for {source_ui_id}"
        );
    }
    let Some(source_decoded) = source_decoded else {
        return Ok(CooperativeModeMenuPlan {
            entry_count: entries.len(),
            rendered_source_ui_ids: Vec::new(),
            glyphs: Vec::new(),
            fixed_strips: Vec::new(),
            occurrences: entries
                .iter()
                .map(|(source_ui_id, _, _)| mode_menu_occurrence(source_ui_id, false))
                .collect::<Result<Vec<_>>>()?,
        });
    };

    validate_source_tim(source_decoded)?;
    let heading = entries
        .iter()
        .find(|(source_ui_id, _, _)| *source_ui_id == "cooperative_mode_menu_heading")
        .map(|(_, _, entry)| *entry)
        .context("cooperative mode-menu heading disappeared")?;
    let heading_characters = heading
        .korean_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    ensure!(
        heading_characters.len() == HEADING_CELLS.len(),
        "cooperative mode-menu heading must occupy exactly {} source cells",
        HEADING_CELLS.len()
    );
    let mut glyphs = Vec::with_capacity(HEADING_CELLS.len());
    for (character, cell) in heading_characters.into_iter().zip(HEADING_CELLS) {
        ensure_source_ink(source_decoded, cell, heading.source_ui_id.as_str())?;
        glyphs.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::ModeMenuHeading,
            character,
            surface: CharacterSelectTextureSurface::CooperativeModeMenuAtlas,
            tim_offset: COOPERATIVE_MODE_MENU_TIM_OFFSET,
            texture_page_index: 0,
            texture_uv: [u8::try_from(cell.x)?, u8::try_from(cell.y)?],
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }

    let mut fixed_strips = Vec::with_capacity(LABEL_STRIPS.len());
    for (source_ui_id, cell) in LABEL_STRIPS {
        let entry = entries
            .iter()
            .find(|(candidate, _, _)| *candidate == source_ui_id)
            .map(|(_, _, entry)| *entry)
            .with_context(|| format!("cooperative mode-menu label {source_ui_id} disappeared"))?;
        ensure_source_ink(source_decoded, cell, source_ui_id)?;
        fixed_strips.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: source_ui_id.to_string(),
            source_ui_ids: vec![source_ui_id.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: super::super::model::CharacterSelectTextSelection::Entire,
            text_flow: super::super::model::CharacterSelectTextFlow::Horizontal,
            write_mode: super::super::model::CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::ModeMenuLabel,
            surface: CharacterSelectTextureSurface::CooperativeModeMenuAtlas,
            tim_offset: COOPERATIVE_MODE_MENU_TIM_OFFSET,
            cell,
            text_cell: cell,
            clear_index: 0,
        });
    }
    ensure!(
        glyphs.iter().all(|glyph| {
            fixed_strips
                .iter()
                .all(|strip| !cells_overlap(glyph.cell, strip.cell))
        }),
        "cooperative mode-menu heading and label cells overlap"
    );
    Ok(CooperativeModeMenuPlan {
        entry_count: entries.len(),
        rendered_source_ui_ids: entries
            .iter()
            .map(|(_, _, entry)| entry.source_ui_id.clone())
            .collect(),
        glyphs,
        fixed_strips,
        occurrences: entries
            .iter()
            .map(|(source_ui_id, _, _)| mode_menu_occurrence(source_ui_id, true))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn mode_menu_occurrence(source_ui_id: &str, bound: bool) -> Result<CharacterSelectRouteOccurrence> {
    let producer_targets = vec![producer_target(
        "DAT2/AISYOU.TIZ",
        Some(COOPERATIVE_MODE_MENU_TIM_OFFSET),
        Some(CharacterSelectTextureSurface::CooperativeModeMenuAtlas),
        [source_ui_id],
    )];
    let byte_offsets = resource_load_byte_offsets("DAT2/AISYOU.TIZ", "DAT1/PLSEL5.BIN")
        .context("cooperative mode menu lost its AISYOU resource-load relation")?;
    let consumer_targets = vec![consumer_target(
        "DAT2/AISYOU.TIZ",
        "DAT1/PLSEL5.BIN",
        CharacterSelectConsumerReferenceKind::ResourceLoad,
        byte_offsets.iter().copied(),
        ["cooperative-mode-menu"],
    )];
    if bound {
        Ok(bound_occurrence(
            format!("aisyou-mode-menu:{source_ui_id}"),
            "aisyou_cooperative_mode_menu",
            [source_ui_id],
            producer_targets,
            consumer_targets,
        ))
    } else {
        Ok(pending_occurrence(
            format!("aisyou-mode-menu:{source_ui_id}"),
            "aisyou_cooperative_mode_menu",
            [source_ui_id],
            producer_targets,
            consumer_targets,
        ))
    }
}

fn validate_source_tim(source_decoded: &[u8]) -> Result<()> {
    let source = source_decoded
        .get(COOPERATIVE_MODE_MENU_TIM_OFFSET..)
        .context("AISYOU cooperative mode-menu TIM offset moved")?;
    let tim = crate::tim::parse_4bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == 256
            && tim.image_height == 256
            && tim.image_x == 768
            && tim.image_y == 256
            && tim.clut_x == 0
            && tim.clut_y == 484
            && tim.clut_width * tim.clut_height / 16 == 5,
        "AISYOU cooperative mode-menu TIM geometry changed"
    );
    ensure!(
        sha256_bytes(&source[..tim.total_size]) == SOURCE_TIM_SHA256,
        "AISYOU cooperative mode-menu source TIM identity changed"
    );
    Ok(())
}

fn ensure_source_ink(source_decoded: &[u8], cell: Cell, source_ui_id: &str) -> Result<()> {
    let pixels =
        read_indexed_cell_in_prefix(source_decoded, COOPERATIVE_MODE_MENU_TIM_OFFSET, cell)?;
    ensure!(
        pixels.iter().any(|pixel| *pixel != 0),
        "AISYOU source cell for {source_ui_id} lost its source ink"
    );
    Ok(())
}
