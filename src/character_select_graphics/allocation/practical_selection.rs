//! Record-owned source strips used by the practical-exam selector in PLSEL1.

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

const SOURCE_RECORD: &str = "DAT2/SELP1.BIZ";
const CONSUMER_RECORD: &str = "DAT1/PLSEL1.BIN";
const RUNTIME_STATE: &str = "top-level-state-3/substate-2/practical-selection";

#[derive(Clone, Copy)]
struct PracticalSelectionStrip {
    source_ui_id: &'static str,
    source_text: &'static str,
    occurrence_id: &'static str,
    physical_region_id: &'static str,
    cell: Cell,
    source_indexed_sha256: &'static str,
    descriptor_offsets: &'static [usize],
}

const PRACTICAL_SELECTION_STRIPS: [PracticalSelectionStrip; 2] = [
    PracticalSelectionStrip {
        source_ui_id: "practical_1999_exam",
        source_text: "99年度試験",
        occurrence_id: "plsel1-practical-1999-exam",
        physical_region_id: "practical-1999-exam-source-region",
        cell: Cell {
            x: 894,
            y: 132,
            width: 126,
            height: 20,
        },
        source_indexed_sha256: "6a2399ddd519801af716915e1a1bfac429da82f944621edf0a2322e0e2d2b5a3",
        descriptor_offsets: &[0x0954, 0x0956, 0x0958, 0x095a],
    },
    PracticalSelectionStrip {
        source_ui_id: "practical_basics_review",
        source_text: "基礎のおさらい",
        occurrence_id: "plsel1-practical-basics-review",
        physical_region_id: "practical-basics-review-source-region",
        cell: Cell {
            x: 880,
            y: 112,
            width: 140,
            height: 20,
        },
        source_indexed_sha256: "93d1efb7e9ee0b7602e2c7e2d82d973c7a2e54b69db7000b22832c60f0cda15f",
        descriptor_offsets: &[0x095c, 0x095e, 0x0960, 0x0962],
    },
];

const SHARED_PRIMITIVE_SETUP_OFFSETS: [usize; 7] = [
    0x5854, // TPage 0x3c0
    0x58d4, // CLUT x 176
    0x58e4, // CLUT y 482
    0x5914, // descriptor U
    0x5924, // descriptor V
    0x5930, // descriptor width
    0x593c, // descriptor height
];

pub(super) struct PracticalSelectionPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_practical_selection(
    shared_source_decoded: &[u8],
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<PracticalSelectionPlan> {
    let requested = PRACTICAL_SELECTION_STRIPS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(PracticalSelectionPlan {
            fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }
    ensure!(
        requested.len() == PRACTICAL_SELECTION_STRIPS.len(),
        "practical selection translations must be supplied as one complete screen"
    );

    let mut fixed_strips = Vec::with_capacity(requested.len());
    let mut occurrences = Vec::with_capacity(requested.len());
    for (spec, entry) in requested {
        ensure!(
            entry.source_text == spec.source_text,
            "practical selection {} source identity changed",
            spec.source_ui_id
        );
        ensure!(
            !entry.korean_text.trim().is_empty() && entry.korean_text.lines().count() == 1,
            "practical selection {} must fit one source strip",
            spec.source_ui_id
        );
        let source_pixels =
            read_indexed_cell_in_prefix(shared_source_decoded, SHARED_ATLAS_OFFSET, spec.cell)?;
        let source_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "practical selection {} source cell changed: found {source_sha256}",
            spec.source_ui_id
        );

        fixed_strips.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.physical_region_id.to_string(),
            source_ui_ids: vec![spec.source_ui_id.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: CharacterSelectTextSelection::Entire,
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::PracticalSelectionLabel,
            surface: CharacterSelectTextureSurface::PracticalSelectionAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            cell: spec.cell,
            text_cell: spec.cell,
            clear_index: 0,
        });

        let mut consumer_offsets = spec.descriptor_offsets.to_vec();
        consumer_offsets.extend(SHARED_PRIMITIVE_SETUP_OFFSETS);
        occurrences.push(bound_occurrence(
            spec.occurrence_id,
            "selp1_practical_selection",
            [spec.source_ui_id],
            vec![producer_target(
                SOURCE_RECORD,
                Some(SHARED_ATLAS_OFFSET),
                Some(CharacterSelectTextureSurface::PracticalSelectionAtlas),
                [spec.physical_region_id],
            )],
            vec![consumer_target(
                SOURCE_RECORD,
                CONSUMER_RECORD,
                CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                consumer_offsets,
                [RUNTIME_STATE],
            )],
        ));
    }

    Ok(PracticalSelectionPlan {
        fixed_strips,
        occurrences,
    })
}
