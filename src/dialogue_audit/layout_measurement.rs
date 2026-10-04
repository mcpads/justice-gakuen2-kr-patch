use anyhow::{Result, ensure};

use super::layout_model::{DialogueLineMeasurement, DialogueRuntimeInsertionWidth};
use super::translation_model::DialogueTranslationControl;

pub(super) const CELL_WIDTH_PIXELS: usize = 20;
pub(super) const LINE_ADVANCE_PIXELS: usize = 28;
pub(super) const TEXT_ORIGIN_X_PIXELS: usize = 84;
pub(super) const TEXT_ORIGIN_Y_PIXELS: usize = 332;
const TEXT_WINDOW_RIGHT_PIXELS: usize = 496;
const TEXT_WINDOW_BOTTOM_PIXELS: usize = 448;
const GLYPH_HEIGHT_PIXELS: usize = 20;
// Original MGAME geometry and paired native window packets. Corpus maximums
// are not screen bounds.
pub(super) const MAXIMUM_CELLS_PER_LINE: usize =
    (TEXT_WINDOW_RIGHT_PIXELS - TEXT_ORIGIN_X_PIXELS) / CELL_WIDTH_PIXELS;
pub(super) const MAXIMUM_LINES_PER_MESSAGE: usize =
    (TEXT_WINDOW_BOTTOM_PIXELS - TEXT_ORIGIN_Y_PIXELS - GLYPH_HEIGHT_PIXELS) / LINE_ADVANCE_PIXELS
        + 1;

const FAMILY_NAME_VISIBLE_CELLS: usize = 6;
const GIVEN_NAME_VISIBLE_CELLS: usize = 6;
const NICKNAME_VISIBLE_CELLS: usize = 4;
const RELATIONSHIP_NAME_VISIBLE_CELLS: usize = GIVEN_NAME_VISIBLE_CELLS;
const KOREAN_HONORIFIC_CELLS: usize = 2;
const RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS: usize =
    RELATIONSHIP_NAME_VISIBLE_CELLS + KOREAN_HONORIFIC_CELLS;

pub(super) fn runtime_insertion_widths() -> Vec<DialogueRuntimeInsertionWidth> {
    [
        (
            "protagonist_family_name",
            1,
            FAMILY_NAME_VISIBLE_CELLS,
        ),
        ("protagonist_given_name", 1, GIVEN_NAME_VISIBLE_CELLS),
        ("protagonist_nickname", 1, NICKNAME_VISIBLE_CELLS),
        (
            "relationship_name_plain",
            1,
            RELATIONSHIP_NAME_VISIBLE_CELLS,
        ),
        (
            "relationship_name_kun_kanji",
            1,
            RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS,
        ),
        (
            "relationship_name_kun_katakana",
            1,
            RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS,
        ),
        (
            "relationship_family_name_kun_katakana",
            1,
            RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS,
        ),
        (
            "relationship_name_san",
            1,
            RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS,
        ),
        (
            "relationship_name_san_or_chan",
            1,
            RELATIONSHIP_NAME_WITH_HONORIFIC_CELLS,
        ),
    ]
    .into_iter()
    .map(
        |(semantic_name, minimum_cells, maximum_cells)| DialogueRuntimeInsertionWidth {
            semantic_name: semantic_name.to_string(),
            minimum_cells,
            maximum_cells,
            basis: match semantic_name {
                "protagonist_family_name" => {
                    "the original enrollment producer admits six glyphs and writes message_end at 0x801f1872"
                }
                "protagonist_given_name" => {
                    "the original enrollment producer admits six glyphs and writes message_end at 0x801f1882"
                }
                "protagonist_nickname" => {
                    "the original enrollment producer admits four glyphs and writes message_end at 0x801f189e"
                }
                "relationship_name_plain" => {
                    "relationship state may select a six-cell family or given name, or a four-cell nickname"
                }
                "relationship_family_name_kun_katakana" => {
                    "the six-cell family-name branch adds a two-cell Korean honorific; given-name and nickname branches are no wider"
                }
                _ => {
                    "a six-cell family or given name may add a two-cell Korean honorific; the four-cell nickname branch adds none"
                }
            }
            .to_string(),
        },
    )
    .collect()
}

pub(super) fn measure_dialogue_lines(
    segments: &[String],
    controls: &[DialogueTranslationControl],
) -> Result<Vec<DialogueLineMeasurement>> {
    ensure!(
        segments.len() == controls.len() + 1,
        "dialogue segment/control shape changed"
    );
    let mut lines = Vec::new();
    let mut line = DialogueLineMeasurement {
        line_index: 0,
        static_cells: 0,
        minimum_cells: 0,
        maximum_cells: 0,
        runtime_insertions: Vec::new(),
    };
    add_static_segment(&mut line, &segments[0])?;
    let mut ended = false;
    let mut pending_padding = 0;

    for (index, control) in controls.iter().enumerate() {
        ensure!(!ended, "dialogue control occurs after message_end");
        match control.semantic_name.as_str() {
            "line_break" => {
                pending_padding = 0;
                lines.push(line);
                line = DialogueLineMeasurement {
                    line_index: lines.len(),
                    static_cells: 0,
                    minimum_cells: 0,
                    maximum_cells: 0,
                    runtime_insertions: Vec::new(),
                };
            }
            "message_end" => {
                ended = true;
                lines.push(line.clone());
            }
            // MGAME's main-window consumer advances one column at
            // 0x800b7d64..0x800b7d6c. The small-font consumer skips this
            // code, but that consumer does not own this layout bound.
            "name_buffer_padding" => pending_padding += 1,
            "palette_style" | "renderer_mode" => {}
            semantic_name => {
                add_static_segment(&mut line, &" ".repeat(pending_padding))?;
                pending_padding = 0;
                let (minimum_cells, maximum_cells) = insertion_width(semantic_name)?;
                line.minimum_cells += minimum_cells;
                line.maximum_cells += maximum_cells;
                line.runtime_insertions.push(semantic_name.to_string());
            }
        }
        // Trailing blanks draw nothing. Charge them only when another
        // visible segment follows, retaining two-column choice spacing.
        if !segments[index + 1].is_empty() {
            add_static_segment(&mut line, &" ".repeat(pending_padding))?;
            pending_padding = 0;
        }
        add_static_segment(&mut line, &segments[index + 1])?;
    }
    ensure!(ended, "dialogue message lacks message_end");
    ensure!(
        segments.last().is_some_and(String::is_empty),
        "dialogue contains text after message_end"
    );
    Ok(lines)
}

fn add_static_segment(line: &mut DialogueLineMeasurement, segment: &str) -> Result<()> {
    ensure!(
        !segment.contains(['\r', '\n']),
        "line breaks must remain structured controls"
    );
    let cells = segment.chars().count();
    line.static_cells += cells;
    line.minimum_cells += cells;
    line.maximum_cells += cells;
    Ok(())
}

fn insertion_width(semantic_name: &str) -> Result<(usize, usize)> {
    let width = runtime_insertion_widths()
        .into_iter()
        .find(|width| width.semantic_name == semantic_name);
    ensure!(
        width.is_some(),
        "layout width is undefined for runtime insertion {semantic_name}"
    );
    let width = width.unwrap();
    Ok((width.minimum_cells, width.maximum_cells))
}
