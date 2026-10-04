use std::path::Path;

use super::description_model::OptionsDescriptionUnit;
use super::description_rebuild::plan_description_layout;
use super::description_source::{ITEM_COUNT, STREAM_ARENA_END, STREAM_ARENA_OFFSET};
use super::description_stream::{CELL_CAPACITY, CELLS_PER_ROW, encode_description_stream};

const FILES: [&str; ITEM_COUNT] = [
    "breakfall.json",
    "spirit-gauge.json",
    "guard.json",
    "back-dash.json",
    "back-jump.json",
];

#[test]
#[ignore = "requires assets/"]
fn authored_descriptions_fit_the_item_atlases_and_stream_arena() {
    let units = FILES
        .into_iter()
        .map(|file| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/menu/options/descriptions")
                .join(file);
            serde_json::from_slice::<OptionsDescriptionUnit>(&std::fs::read(path).unwrap()).unwrap()
        })
        .collect::<Vec<_>>();

    let plans = plan_description_layout(&units).unwrap();
    let mut stream_bytes = 0usize;
    for plan in &plans {
        assert!(plan.atlas_cells.len() <= CELL_CAPACITY);
        for variant in &plan.variants {
            stream_bytes += encode_description_stream(&variant.stream).unwrap().len();
            let rebuilt_lines = variant
                .stream
                .lines
                .iter()
                .map(|line| {
                    line.iter()
                        .flat_map(|span| {
                            let start = usize::from(span.start_cell);
                            let end = start + usize::from(span.cell_count);
                            assert_eq!(start / CELLS_PER_ROW, (end - 1) / CELLS_PER_ROW);
                            plan.atlas_cells[start..end].iter().copied()
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            assert_eq!(rebuilt_lines, variant.korean_lines);
        }
    }
    assert!(
        stream_bytes <= STREAM_ARENA_END - STREAM_ARENA_OFFSET,
        "description streams need {stream_bytes} bytes but own {} (cells: {:?})",
        STREAM_ARENA_END - STREAM_ARENA_OFFSET,
        plans
            .iter()
            .map(|plan| (&plan.id, plan.atlas_cells.len()))
            .collect::<Vec<_>>()
    );
}
