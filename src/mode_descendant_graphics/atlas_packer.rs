use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::tim::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AtlasRectangle {
    pub(super) width: usize,
    pub(super) height: usize,
}

#[derive(Debug, Clone, Copy)]
enum HeightOrder {
    TallFirst,
    ShortFirst,
}

#[derive(Debug, Clone, Copy)]
enum WidthOrder {
    WideFirst,
    NarrowFirst,
}

#[derive(Debug, Clone, Copy)]
enum RunChoice {
    Tightest,
    Widest,
}

#[derive(Debug, Clone, Copy)]
enum RunEdge {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct PackingStrategy {
    height_order: HeightOrder,
    width_order: WidthOrder,
    run_choice: RunChoice,
    run_edge: RunEdge,
}

const HEIGHT_ORDERS: [HeightOrder; 2] = [HeightOrder::TallFirst, HeightOrder::ShortFirst];
const WIDTH_ORDERS: [WidthOrder; 2] = [WidthOrder::WideFirst, WidthOrder::NarrowFirst];
const RUN_CHOICES: [RunChoice; 2] = [RunChoice::Tightest, RunChoice::Widest];
const RUN_EDGES: [RunEdge; 2] = [RunEdge::Left, RunEdge::Right];

#[derive(Clone)]
struct PackingState {
    occupied: Vec<bool>,
    placements: Vec<Option<Cell>>,
}

#[derive(Debug, Clone, Copy)]
struct FreeRun {
    start: usize,
    end: usize,
}

impl FreeRun {
    fn width(self) -> usize {
        self.end - self.start
    }
}

struct BandPlan {
    placements: Vec<(usize, Cell)>,
    packed_width: usize,
    stranded_width: usize,
    contact: usize,
    page: usize,
    y: usize,
}

/// Packs rectangles into a row-major occupancy bitmap made of adjacent pages.
///
/// `true` cells are immutable pixels and are never returned by this function.
/// The caller defines the available page set through the bitmap, and no
/// placement crosses a page boundary. Returned cells correspond one-for-one
/// with `rectangles` in input order.
pub(super) fn pack_atlas_rectangles(
    occupied: &[bool],
    page_width: usize,
    page_height: usize,
    rectangles: &[AtlasRectangle],
) -> Result<Vec<Cell>> {
    let page_count = validate_inputs(occupied, page_width, page_height, rectangles)?;
    if rectangles.is_empty() {
        return Ok(Vec::new());
    }

    let required_pixels = rectangles
        .iter()
        .map(|rectangle| rectangle.width * rectangle.height)
        .sum::<usize>();
    let free_pixels = occupied.iter().filter(|pixel| !**pixel).count();
    ensure!(
        required_pixels <= free_pixels,
        "atlas needs {required_pixels} rectangle pixels but only {free_pixels} pixels are free"
    );

    let mut best_placed_count = 0usize;
    for height_order in HEIGHT_ORDERS {
        for width_order in WIDTH_ORDERS {
            for run_choice in RUN_CHOICES {
                for run_edge in RUN_EDGES {
                    let mut state = PackingState {
                        occupied: occupied.to_vec(),
                        placements: vec![None; rectangles.len()],
                    };
                    pack_with_strategy(
                        &mut state,
                        page_width,
                        page_height,
                        page_count,
                        rectangles,
                        PackingStrategy {
                            height_order,
                            width_order,
                            run_choice,
                            run_edge,
                        },
                    );
                    let placed_count = state
                        .placements
                        .iter()
                        .filter(|placement| placement.is_some())
                        .count();
                    best_placed_count = best_placed_count.max(placed_count);
                    if placed_count == rectangles.len() {
                        return collect_placements(state.placements);
                    }
                }
            }
        }
    }

    // Bands are fast, but can strand space when several glyph heights share
    // irregular free regions. Retry individual rectangles without enlarging
    // the caller's proven writable complement.
    for order in 0..4 {
        for prefer_contact in [false, true] {
            for edge in RUN_EDGES {
                let mut state = PackingState {
                    occupied: occupied.to_vec(),
                    placements: vec![None; rectangles.len()],
                };
                let mut indices = (0..rectangles.len()).collect::<Vec<_>>();
                indices.sort_by_key(|&index| {
                    let r = rectangles[index];
                    let major = match order {
                        0 => r.width * r.height,
                        1 => r.height,
                        2 => r.width,
                        _ => r.width.min(r.height),
                    };
                    (usize::MAX - major, usize::MAX - r.width * r.height, index)
                });
                for index in indices {
                    let rectangle = rectangles[index];
                    let mut best = None;
                    for page in 0..page_count {
                        for y in band_y_candidates(
                            &state.occupied,
                            page_width,
                            page_height,
                            page_count,
                            page,
                            rectangle.height,
                        ) {
                            for run in free_runs_for_band(
                                &state.occupied,
                                page_width,
                                page_count,
                                page,
                                y,
                                rectangle.height,
                            ) {
                                if run.width() < rectangle.width {
                                    continue;
                                }
                                let cell = Cell {
                                    x: match edge {
                                        RunEdge::Left => run.start,
                                        RunEdge::Right => run.end - rectangle.width,
                                    },
                                    y,
                                    width: rectangle.width,
                                    height: rectangle.height,
                                };
                                let contact =
                                    contact_score(&state.occupied, page_width, page_count, cell);
                                let slack = run.width() - rectangle.width;
                                let score = if prefer_contact {
                                    (
                                        contact,
                                        usize::MAX - slack,
                                        usize::MAX - y,
                                        usize::MAX - cell.x,
                                    )
                                } else {
                                    (
                                        usize::MAX - slack,
                                        contact,
                                        usize::MAX - y,
                                        usize::MAX - cell.x,
                                    )
                                };
                                if best.as_ref().is_none_or(|(old, _)| score > *old) {
                                    best = Some((score, cell));
                                }
                            }
                        }
                    }
                    let Some((_, cell)) = best else {
                        break;
                    };
                    mark_cell(&mut state.occupied, page_width, page_count, cell);
                    state.placements[index] = Some(cell);
                }
                let placed = state
                    .placements
                    .iter()
                    .filter(|cell| cell.is_some())
                    .count();
                best_placed_count = best_placed_count.max(placed);
                if placed == rectangles.len() {
                    return collect_placements(state.placements);
                }
            }
        }
    }

    anyhow::bail!(
        "atlas cannot place all {} rectangles in the supplied free-space complement; best deterministic placement strategy placed {best_placed_count}",
        rectangles.len()
    )
}

fn validate_inputs(
    occupied: &[bool],
    page_width: usize,
    page_height: usize,
    rectangles: &[AtlasRectangle],
) -> Result<usize> {
    ensure!(
        page_width > 0 && page_height > 0,
        "atlas page geometry must be nonzero"
    );
    let page_pixels = page_width
        .checked_mul(page_height)
        .context("atlas page geometry overflowed")?;
    ensure!(
        !occupied.is_empty() && occupied.len().is_multiple_of(page_pixels),
        "atlas occupancy has {} pixels, which is not a whole number of {page_width}x{page_height} pages",
        occupied.len()
    );
    let page_count = occupied.len() / page_pixels;
    for (index, rectangle) in rectangles.iter().enumerate() {
        ensure!(
            rectangle.width > 0
                && rectangle.height > 0
                && rectangle.width <= page_width
                && rectangle.height <= page_height,
            "atlas rectangle {index} has invalid geometry {}x{}",
            rectangle.width,
            rectangle.height
        );
    }
    Ok(page_count)
}

fn pack_with_strategy(
    state: &mut PackingState,
    page_width: usize,
    page_height: usize,
    page_count: usize,
    rectangles: &[AtlasRectangle],
    strategy: PackingStrategy,
) {
    while let Some(height) = next_unplaced_height(state, rectangles, strategy.height_order) {
        let indices = ordered_indices_for_height(state, rectangles, height, strategy.width_order);
        let Some(plan) = choose_band(
            &state.occupied,
            page_width,
            page_height,
            page_count,
            rectangles,
            &indices,
            strategy,
        ) else {
            return;
        };
        for (index, cell) in plan.placements {
            mark_cell(&mut state.occupied, page_width, page_count, cell);
            state.placements[index] = Some(cell);
        }
    }
}

fn next_unplaced_height(
    state: &PackingState,
    rectangles: &[AtlasRectangle],
    order: HeightOrder,
) -> Option<usize> {
    let heights = rectangles
        .iter()
        .enumerate()
        .filter(|(index, _)| state.placements[*index].is_none())
        .map(|(_, rectangle)| rectangle.height);
    match order {
        HeightOrder::TallFirst => heights.max(),
        HeightOrder::ShortFirst => heights.min(),
    }
}

fn ordered_indices_for_height(
    state: &PackingState,
    rectangles: &[AtlasRectangle],
    height: usize,
    order: WidthOrder,
) -> Vec<usize> {
    let mut indices = rectangles
        .iter()
        .enumerate()
        .filter(|(index, rectangle)| {
            state.placements[*index].is_none() && rectangle.height == height
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indices.sort_by_key(|index| match order {
        WidthOrder::WideFirst => (usize::MAX - rectangles[*index].width, *index),
        WidthOrder::NarrowFirst => (rectangles[*index].width, *index),
    });
    indices
}

#[allow(clippy::too_many_arguments)]
fn choose_band(
    occupied: &[bool],
    page_width: usize,
    page_height: usize,
    page_count: usize,
    rectangles: &[AtlasRectangle],
    indices: &[usize],
    strategy: PackingStrategy,
) -> Option<BandPlan> {
    let height = rectangles[*indices.first()?].height;
    let mut best = None;
    for page in 0..page_count {
        for y in band_y_candidates(occupied, page_width, page_height, page_count, page, height) {
            let runs = free_runs_for_band(occupied, page_width, page_count, page, y, height);
            let Some(candidate) = pack_band(
                occupied, page_width, page_count, rectangles, indices, runs, page, y, strategy,
            ) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|current| band_is_better(&candidate, current))
            {
                best = Some(candidate);
            }
        }
    }
    best
}

fn band_y_candidates(
    occupied: &[bool],
    page_width: usize,
    page_height: usize,
    page_count: usize,
    page: usize,
    height: usize,
) -> Vec<usize> {
    let texture_width = page_width * page_count;
    let page_start = page * page_width;
    let mut candidates = BTreeSet::from([0, page_height - height]);
    for boundary in 1..page_height {
        let previous = &occupied[(boundary - 1) * texture_width + page_start
            ..(boundary - 1) * texture_width + page_start + page_width];
        let current = &occupied[boundary * texture_width + page_start
            ..boundary * texture_width + page_start + page_width];
        if previous == current {
            continue;
        }
        if boundary <= page_height - height {
            candidates.insert(boundary);
        }
        if boundary >= height {
            candidates.insert(boundary - height);
        }
    }
    candidates.into_iter().collect()
}

fn free_runs_for_band(
    occupied: &[bool],
    page_width: usize,
    page_count: usize,
    page: usize,
    y: usize,
    height: usize,
) -> Vec<FreeRun> {
    let texture_width = page_width * page_count;
    let page_start = page * page_width;
    let mut runs = Vec::new();
    let mut local_x = 0usize;
    while local_x < page_width {
        while local_x < page_width
            && column_is_occupied(occupied, texture_width, page_start + local_x, y, height)
        {
            local_x += 1;
        }
        let start = local_x;
        while local_x < page_width
            && !column_is_occupied(occupied, texture_width, page_start + local_x, y, height)
        {
            local_x += 1;
        }
        if start < local_x {
            runs.push(FreeRun {
                start: page_start + start,
                end: page_start + local_x,
            });
        }
    }
    runs
}

fn column_is_occupied(
    occupied: &[bool],
    texture_width: usize,
    x: usize,
    y: usize,
    height: usize,
) -> bool {
    (y..y + height).any(|row| occupied[row * texture_width + x])
}

#[allow(clippy::too_many_arguments)]
fn pack_band(
    occupied: &[bool],
    page_width: usize,
    page_count: usize,
    rectangles: &[AtlasRectangle],
    indices: &[usize],
    mut runs: Vec<FreeRun>,
    page: usize,
    y: usize,
    strategy: PackingStrategy,
) -> Option<BandPlan> {
    let mut placements = Vec::new();
    let mut packed_width = 0usize;
    for &index in indices {
        let rectangle = rectangles[index];
        let Some(run_index) = choose_run(&runs, rectangle.width, strategy.run_choice) else {
            continue;
        };
        let run = &mut runs[run_index];
        let x = match strategy.run_edge {
            RunEdge::Left => {
                let x = run.start;
                run.start += rectangle.width;
                x
            }
            RunEdge::Right => {
                run.end -= rectangle.width;
                run.end
            }
        };
        placements.push((
            index,
            Cell {
                x,
                y,
                width: rectangle.width,
                height: rectangle.height,
            },
        ));
        packed_width += rectangle.width;
    }
    if placements.is_empty() {
        return None;
    }

    let minimum_remaining_width = indices
        .iter()
        .filter(|index| {
            placements
                .iter()
                .all(|(placed_index, _)| placed_index != *index)
        })
        .map(|index| rectangles[*index].width)
        .min();
    let stranded_width = minimum_remaining_width.map_or(0, |minimum| {
        runs.iter()
            .map(|run| run.width())
            .filter(|width| *width < minimum)
            .sum()
    });
    let contact = placements
        .iter()
        .map(|(_, cell)| contact_score(occupied, page_width, page_count, *cell))
        .sum();
    Some(BandPlan {
        placements,
        packed_width,
        stranded_width,
        contact,
        page,
        y,
    })
}

fn choose_run(runs: &[FreeRun], width: usize, choice: RunChoice) -> Option<usize> {
    runs.iter()
        .enumerate()
        .filter(|(_, run)| run.width() >= width)
        .min_by_key(|(index, run)| match choice {
            RunChoice::Tightest => (run.width() - width, *index),
            RunChoice::Widest => (usize::MAX - run.width(), *index),
        })
        .map(|(index, _)| index)
}

fn band_is_better(candidate: &BandPlan, current: &BandPlan) -> bool {
    (
        candidate.packed_width,
        candidate.placements.len(),
        usize::MAX - candidate.stranded_width,
        candidate.contact,
        usize::MAX - candidate.page,
        usize::MAX - candidate.y,
    ) > (
        current.packed_width,
        current.placements.len(),
        usize::MAX - current.stranded_width,
        current.contact,
        usize::MAX - current.page,
        usize::MAX - current.y,
    )
}

fn contact_score(occupied: &[bool], page_width: usize, page_count: usize, cell: Cell) -> usize {
    let texture_width = page_width * page_count;
    let page_start = (cell.x / page_width) * page_width;
    let page_end = page_start + page_width;
    let page_height = occupied.len() / texture_width;
    let mut score = 0usize;
    if cell.x == page_start {
        score += cell.height;
    } else {
        score += (cell.y..cell.y + cell.height)
            .filter(|y| occupied[*y * texture_width + cell.x - 1])
            .count();
    }
    if cell.x + cell.width == page_end {
        score += cell.height;
    } else {
        score += (cell.y..cell.y + cell.height)
            .filter(|y| occupied[*y * texture_width + cell.x + cell.width])
            .count();
    }
    if cell.y == 0 {
        score += cell.width;
    } else {
        score += (cell.x..cell.x + cell.width)
            .filter(|x| occupied[(cell.y - 1) * texture_width + *x])
            .count();
    }
    if cell.y + cell.height == page_height {
        score += cell.width;
    } else {
        score += (cell.x..cell.x + cell.width)
            .filter(|x| occupied[(cell.y + cell.height) * texture_width + *x])
            .count();
    }
    score
}

fn mark_cell(occupied: &mut [bool], page_width: usize, page_count: usize, cell: Cell) {
    let texture_width = page_width * page_count;
    for y in cell.y..cell.y + cell.height {
        occupied[y * texture_width + cell.x..y * texture_width + cell.x + cell.width].fill(true);
    }
}

fn collect_placements(placements: Vec<Option<Cell>>) -> Result<Vec<Cell>> {
    placements
        .into_iter()
        .enumerate()
        .map(|(index, placement)| {
            placement.with_context(|| format!("atlas packer omitted rectangle {index}"))
        })
        .collect()
}
