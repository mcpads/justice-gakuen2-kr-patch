use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, cells_overlap, read_indexed_cell_in_prefix};

use super::assets::{CharacterSelectTranslationAssets, load_character_select_translation_assets};
use super::consumer::{
    CharacterSelectDynamicTextBinding, dynamic_route_occurrences, dynamic_text_bindings,
    reclaimable_stage_label_source_cells, retained_source_referenced_20px_cells,
};
use super::model::{
    CharacterSelectAtlasPlan, CharacterSelectConsumerPlan, CharacterSelectFixedStripSet,
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectSegmentedFixedStripAllocation, CharacterSelectSourceCellOccupancy,
    CharacterSelectSourceTreatment, CharacterSelectTextureSurface,
};

#[path = "allocation/battle_ready.rs"]
mod battle_ready;
#[path = "allocation/common_pause_menu.rs"]
mod common_pause_menu;
#[path = "allocation/cooperative_background.rs"]
mod cooperative_background;
#[path = "allocation/cooperative_mode_menu.rs"]
mod cooperative_mode_menu;
#[path = "allocation/fixed_texture_strips.rs"]
mod fixed_texture_strips;
#[path = "allocation/league_standings.rs"]
mod league_standings;
#[path = "allocation/practical_selection.rs"]
mod practical_selection;
#[path = "allocation/record_owned_labels.rs"]
mod record_owned_labels;
#[path = "allocation/solo_episode_card.rs"]
mod solo_episode_card;
#[path = "allocation/solo_state_prompts.rs"]
mod solo_state_prompts;
#[path = "allocation/solo_story_intro.rs"]
mod solo_story_intro;
#[path = "allocation/source_glyph_cells.rs"]
mod source_glyph_cells;
#[path = "allocation/stage_labels.rs"]
mod stage_labels;
#[path = "allocation/tournament_bracket.rs"]
mod tournament_bracket;
#[path = "allocation/tournament_certificate.rs"]
mod tournament_certificate;
pub(super) use tournament_certificate::{TEAM_MARKER_SCREEN_X, TEAM_MARKER_SCREEN_Y};
#[path = "allocation/versus_labels.rs"]
mod versus_labels;

use super::planning_sources::CharacterSelectPlanningSources;
use super::route_census::{census_routes, route_is_complete};
use battle_ready::plan_battle_ready_rows;
use common_pause_menu::plan_common_pause_menu;
pub(crate) use common_pause_menu::register_battle_pause_return;
use cooperative_background::plan_cooperative_background;
use cooperative_mode_menu::plan_cooperative_mode_menu;
use fixed_texture_strips::{plan_fixed_texture_strips, validate_fixed_texture_strip_conflicts};
use league_standings::plan_league_standings;
use practical_selection::plan_practical_selection;
use record_owned_labels::{
    allocate_battle_ready_label_glyphs, allocate_participant_label_glyphs,
    allocate_selection_help_glyphs,
};
use solo_episode_card::plan_solo_episode_cards;
use solo_state_prompts::plan_solo_state_prompts;
use solo_story_intro::plan_solo_story_intro;
use source_glyph_cells::{collect_source_glyph_cells, validate_source_glyph_cell_conflicts};
use stage_labels::allocate_stage_label_glyphs;
use tournament_bracket::plan_tournament_bracket_label;
use tournament_certificate::plan_tournament_certificate;
use versus_labels::allocate_versus_label_glyphs;

pub(super) use super::texture_targets::SHARED_ATLAS_OFFSET;

pub fn plan_character_select_atlas(
    shared_source_decoded: &[u8],
    assets_directory: &Path,
) -> Result<CharacterSelectAtlasPlan> {
    let assets = load_character_select_translation_assets(assets_directory)?;
    plan_character_select_atlas_from_assets(
        &CharacterSelectPlanningSources::shared_only(shared_source_decoded),
        &assets,
    )
}

pub(super) fn plan_character_select_atlas_from_assets(
    sources: &CharacterSelectPlanningSources<'_>,
    assets: &CharacterSelectTranslationAssets,
) -> Result<CharacterSelectAtlasPlan> {
    let shared_source_decoded = sources.shared_atlas_record();
    let dynamic_bindings = dynamic_text_bindings()?;
    let requested = requested_glyphs(&assets.localized_sources, &dynamic_bindings)?;
    let native_cells = super::native_regions::protected_cells()?;
    let shared_source_ink = SourceInkMap::read(shared_source_decoded, SHARED_ATLAS_OFFSET)?;
    ensure!(
        shared_source_ink.width == 1024 && shared_source_ink.height == 256,
        "SELP shared atlas geometry changed"
    );
    let source_glyph_cells = if sources.has_record_context() {
        collect_source_glyph_cells(
            shared_source_decoded,
            &retained_source_referenced_20px_cells(&assets.localized_sources)?,
        )?
    } else {
        Vec::new()
    };
    let fixed_texture_strips = plan_fixed_texture_strips(
        shared_source_decoded,
        &assets.localized_sources,
        assets
            .required_fixed_strip_sets
            .contains(&CharacterSelectFixedStripSet::RosterNames),
    )?;
    let battle_ready = plan_battle_ready_rows(shared_source_decoded, &assets.localized_sources)?;
    let practical_selection =
        plan_practical_selection(shared_source_decoded, &assets.localized_sources)?;
    let league_standings = plan_league_standings(shared_source_decoded, &assets.localized_sources)?;
    let solo_state_prompts = plan_solo_state_prompts(
        sources.record(super::texture_targets::OVER_PATH),
        &assets.localized_sources,
    )?;
    let common_pause_menu = plan_common_pause_menu(
        sources.record(super::texture_targets::OVER_PATH),
        &assets.localized_sources,
    )?;
    let solo_story_intro = plan_solo_story_intro(
        sources.record(super::texture_targets::OP01_PATH),
        &assets.localized_sources,
    )?;
    let solo_episode_cards = plan_solo_episode_cards(
        sources.record(super::texture_targets::TITLE_PATH),
        &assets.localized_sources,
    )?;
    let mut route_occurrences = dynamic_route_occurrences(&assets.localized_sources);
    route_occurrences.extend(fixed_texture_strips.occurrences);
    route_occurrences.extend(battle_ready.occurrences);
    route_occurrences.extend(practical_selection.occurrences);
    route_occurrences.extend(league_standings.occurrences);
    route_occurrences.extend(solo_state_prompts.occurrences);
    route_occurrences.extend(common_pause_menu.occurrences);
    route_occurrences.extend(solo_story_intro.occurrences);
    route_occurrences.extend(solo_episode_cards.occurrences);
    let mut fixed_strips = fixed_texture_strips.allocations;
    fixed_strips.extend(battle_ready.fixed_strips);
    fixed_strips.extend(practical_selection.fixed_strips);
    fixed_strips.extend(league_standings.fixed_strips);
    fixed_strips.extend(solo_state_prompts.fixed_strips);
    fixed_strips.extend(common_pause_menu.fixed_strips);
    let mut segmented_fixed_strips = solo_story_intro.segmented_fixed_strips;
    segmented_fixed_strips.extend(solo_episode_cards.segmented_fixed_strips);
    segmented_fixed_strips.extend(common_pause_menu.segmented_fixed_strips);
    let protected_select_heading_cells = protected_cells_for_surface(
        CharacterSelectTextureSurface::SharedSelectAtlas,
        &source_glyph_cells,
        &fixed_strips,
        &native_cells,
    );
    let mut allocations = Vec::with_capacity(requested.len());
    let mut available_select_heading_cell_count_on_one_texture_page = None;
    let mut select_heading_texture_page_index = None;
    for role in [CharacterSelectFontRole::SelectHeading] {
        let role_requested = requested
            .iter()
            .filter(|(requested_role, _)| *requested_role == role)
            .copied()
            .collect::<Vec<_>>();
        let source_ink = &shared_source_ink;
        let surface = CharacterSelectTextureSurface::SharedSelectAtlas;
        let tim_offset = SHARED_ATLAS_OFFSET;
        let candidates = available_cells(source_ink, role, &protected_select_heading_cells);
        let available_count = pack_on_one_texture_page(candidates.clone(), usize::MAX).len();
        let selected = pack_on_one_texture_page(candidates, role_requested.len());
        ensure!(
            selected.len() == role_requested.len(),
            "character-select {} needs {} glyphs but only {} non-overlapping source-blank atlas cells can be packed",
            role_key(role),
            role_requested.len(),
            selected.len(),
        );
        let selected_page = selected
            .first()
            .map(|(texture_page_index, _, _)| *texture_page_index);
        ensure!(
            selected
                .iter()
                .all(|(texture_page_index, _, _)| Some(*texture_page_index) == selected_page),
            "character-select select-heading allocation escaped one texture page"
        );
        for ((_, character), (texture_page_index, texture_uv, cell)) in
            role_requested.into_iter().zip(selected)
        {
            allocations.push(CharacterSelectGlyphAllocation {
                font_role: role,
                character,
                surface,
                tim_offset,
                texture_page_index,
                texture_uv,
                cell,
                source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
            });
        }
        if role == CharacterSelectFontRole::SelectHeading {
            available_select_heading_cell_count_on_one_texture_page = Some(available_count);
            select_heading_texture_page_index = selected_page;
        }
    }
    allocate_versus_label_glyphs(
        &assets.localized_sources,
        shared_source_decoded,
        &mut allocations,
    )?;
    allocate_participant_label_glyphs(&assets.localized_sources, &mut allocations)?;
    allocate_battle_ready_label_glyphs(
        &assets.localized_sources,
        &shared_source_ink,
        &protected_cells_for_surface(
            CharacterSelectTextureSurface::BattleReadyAtlas,
            &source_glyph_cells,
            &fixed_strips,
            &native_cells,
        ),
        &mut allocations,
    )?;
    allocate_selection_help_glyphs(
        &assets.localized_sources,
        &shared_source_ink,
        &protected_cells_for_surface(
            CharacterSelectTextureSurface::SelectionHelpAtlas,
            &source_glyph_cells,
            &fixed_strips,
            &native_cells,
        ),
        &mut allocations,
    )?;
    allocate_stage_label_glyphs(
        &assets.localized_sources,
        &dynamic_bindings,
        &shared_source_ink,
        &reclaimable_stage_label_source_cells(&assets.localized_sources)?,
        &protected_cells_for_surface(
            CharacterSelectTextureSurface::StageLabelAtlas,
            &source_glyph_cells,
            &fixed_strips,
            &native_cells,
        ),
        &fixed_strips,
        &mut allocations,
    )?;
    let cooperative_mode_menu = plan_cooperative_mode_menu(
        sources.record(super::texture_targets::AISYOU_PATH),
        &assets.localized_sources,
    )?;
    let cooperative_background = plan_cooperative_background(
        sources.record(super::texture_targets::SELP5_PATH),
        &assets.localized_sources,
    )?;
    let tournament_certificate = plan_tournament_certificate(
        sources.record(super::texture_targets::TOROFY_PATH),
        &assets.localized_sources,
    )?;
    let tournament_bracket = plan_tournament_bracket_label(
        sources.record(super::texture_targets::SELP4_PATH),
        &assets.localized_sources,
    )?;
    route_occurrences.extend(cooperative_mode_menu.occurrences.iter().cloned());
    route_occurrences.extend(cooperative_background.occurrences.iter().cloned());
    route_occurrences.extend(tournament_certificate.occurrences.iter().cloned());
    route_occurrences.extend(tournament_bracket.occurrences.iter().cloned());
    allocations.extend(cooperative_mode_menu.glyphs);
    fixed_strips.extend(cooperative_mode_menu.fixed_strips);
    fixed_strips.extend(cooperative_background.fixed_strips);
    fixed_strips.extend(tournament_certificate.fixed_strips);
    fixed_strips.extend(tournament_bracket.fixed_strips);
    let source_ink_cleanups = tournament_certificate.source_ink_cleanups;
    let available_select_heading_cell_count_on_one_texture_page =
        available_select_heading_cell_count_on_one_texture_page
            .context("character-select select-heading allocation was not planned")?;
    validate_allocations(&allocations)?;
    for allocation in &allocations {
        if allocation
            .surface
            .shares_physical_texture(CharacterSelectTextureSurface::SharedSelectAtlas)
        {
            ensure!(
                native_cells
                    .iter()
                    .all(|cell| !cells_overlap(*cell, allocation.cell)),
                "dynamic glyph {:?} overlaps a retained native sprite",
                allocation.character
            );
        }
    }
    validate_fixed_texture_strip_conflicts(&fixed_strips, &allocations)?;
    validate_segmented_fixed_strip_conflicts(&segmented_fixed_strips, &fixed_strips, &allocations)?;
    validate_source_glyph_cell_conflicts(&source_glyph_cells, &allocations)?;
    let segmented_fixed_strip_segment_count = segmented_fixed_strips
        .iter()
        .map(|strip| strip.segments.len())
        .sum::<usize>();
    let localized_source_ui_ids = assets
        .localized_sources
        .iter()
        .map(|entry| entry.source_ui_id.as_str())
        .collect::<BTreeSet<_>>();
    let dynamic_source_ui_ids = dynamic_bindings
        .iter()
        .filter(|binding| localized_source_ui_ids.contains(binding.source_ui_id))
        .map(|binding| binding.source_ui_id)
        .collect::<BTreeSet<_>>();
    let route_census = census_routes(&localized_source_ui_ids, route_occurrences)?;
    let unresolved_route_source_ui_ids = route_census
        .occurrences
        .iter()
        .filter(|occurrence| !route_is_complete(occurrence))
        .flat_map(|occurrence| occurrence.source_ui_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let unresolved_source_ui_id_set = unresolved_route_source_ui_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let fully_routed_source_ui_ids = localized_source_ui_ids
        .difference(&unresolved_source_ui_id_set)
        .copied()
        .collect::<BTreeSet<_>>();
    let unresolved_route_translation_ids = assets
        .source_inventory_entries
        .iter()
        .filter(|entry| unresolved_source_ui_id_set.contains(entry.id.as_str()))
        .map(|entry| entry.translation_id().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let source_translation_coverage = assets.source_translation_coverage();
    let untranslated_source_ui_ids = source_translation_coverage.untranslated_source_ui_ids;
    let translated_source_ui_count = source_translation_coverage.translated_source_ui_count;
    let preserved_source_ui_ids = assets
        .source_inventory_entries
        .iter()
        .filter(|entry| entry.treatment == CharacterSelectSourceTreatment::Preserve)
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let excluded_source_ui_ids = assets
        .source_inventory_entries
        .iter()
        .filter(|entry| entry.treatment == CharacterSelectSourceTreatment::Exclude)
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    Ok(CharacterSelectAtlasPlan {
        kind: "justice_gakuen2_character_select_dynamic_atlas_plan".to_string(),
        source_shared_atlas_sha256: source_atlas_sha256(
            shared_source_decoded,
            SHARED_ATLAS_OFFSET,
        )?,
        translation_assets_sha256: assets.assets_sha256.clone(),
        source_inventory_complete: assets.source_inventory_complete,
        source_inventory_unit_count: assets.source_inventory_unit_count,
        source_inventory_entry_count: assets.source_inventory_entries.len(),
        translated_source_ui_count,
        untranslated_source_ui_count: untranslated_source_ui_ids.len(),
        untranslated_source_ui_ids,
        preserved_source_ui_ids,
        excluded_source_ui_ids,
        translation_unit_count: assets.unit_count,
        translation_entry_count: assets.entries.len(),
        dynamic_atlas_entry_count: dynamic_source_ui_ids.len()
            + usize::from(
                cooperative_mode_menu
                    .rendered_source_ui_ids
                    .iter()
                    .any(|id| id == "cooperative_mode_menu_heading"),
            ),
        fixed_texture_strip_entry_count: fixed_strips.len() + segmented_fixed_strip_segment_count,
        source_ink_cleanup_count: source_ink_cleanups.len(),
        bound_fixed_texture_strip_count: fixed_strips.len() + segmented_fixed_strip_segment_count,
        runtime_composed_texture_entry_count: cooperative_mode_menu.entry_count,
        unresolved_route_occurrence_count: route_census.unresolved_route_occurrence_count,
        fully_routed_source_ui_ids: fully_routed_source_ui_ids
            .into_iter()
            .map(str::to_string)
            .collect(),
        unresolved_route_source_ui_ids,
        unresolved_route_translation_ids,
        route_census,
        consumer: CharacterSelectConsumerPlan {
            shared_atlas_vram_word_x: shared_source_ink.vram_word_x,
            select_heading_texture_page_index: select_heading_texture_page_index
                .context("character-select select-heading texture page was not planned")?,
        },
        required_glyph_count: allocations.len(),
        available_select_heading_cell_count_on_one_texture_page,
        source_ink_cells_protected: true,
        source_glyph_cell_count: source_glyph_cells.len(),
        physical_cells_are_unique_and_non_overlapping: true,
        source_glyph_cells,
        allocations,
        fixed_strips,
        segmented_fixed_strips,
        source_ink_cleanups,
    })
}

#[derive(Debug)]
struct SourceInkMap {
    width: usize,
    height: usize,
    vram_word_x: u16,
    summed_area: Vec<usize>,
}

impl SourceInkMap {
    fn read(source_decoded: &[u8], tim_offset: usize) -> Result<Self> {
        let tim = crate::tim::parse_4bpp_prefix(
            source_decoded
                .get(tim_offset..)
                .context("character-select atlas offset is outside the decoded source")?,
        )?;
        let width = tim.pixel_width();
        let height = tim.image_height;
        let pixels = read_indexed_cell_in_prefix(
            source_decoded,
            tim_offset,
            Cell {
                x: 0,
                y: 0,
                width,
                height,
            },
        )?;
        let stride = width + 1;
        let mut summed_area = vec![0usize; stride * (height + 1)];
        for y in 0..height {
            let mut row_ink = 0usize;
            for x in 0..width {
                row_ink += usize::from(pixels[y * width + x] != 0);
                summed_area[(y + 1) * stride + x + 1] = summed_area[y * stride + x + 1] + row_ink;
            }
        }
        Ok(Self {
            width,
            height,
            vram_word_x: tim.image_x,
            summed_area,
        })
    }

    fn is_blank(&self, cell: Cell) -> bool {
        let stride = self.width + 1;
        let x0 = cell.x;
        let y0 = cell.y;
        let x1 = cell.x + cell.width;
        let y1 = cell.y + cell.height;
        let ink = self.summed_area[y1 * stride + x1] + self.summed_area[y0 * stride + x0]
            - self.summed_area[y0 * stride + x1]
            - self.summed_area[y1 * stride + x0];
        ink == 0
    }
}

fn pack_non_overlapping(
    candidates: Vec<(u8, [u8; 2], Cell)>,
    limit: usize,
) -> Vec<(u8, [u8; 2], Cell)> {
    if limit == 0 {
        return Vec::new();
    }
    let mut selected = Vec::new();
    for candidate in candidates {
        if selected
            .iter()
            .all(|(_, _, selected_cell)| !cells_overlap(*selected_cell, candidate.2))
        {
            selected.push(candidate);
            if selected.len() == limit {
                break;
            }
        }
    }
    selected
}

fn pack_on_one_texture_page(
    candidates: Vec<(u8, [u8; 2], Cell)>,
    limit: usize,
) -> Vec<(u8, [u8; 2], Cell)> {
    if limit == 0 {
        return Vec::new();
    }
    let mut best = Vec::new();
    let page_count = candidates
        .iter()
        .map(|(page, _, _)| usize::from(*page) + 1)
        .max()
        .unwrap_or(0);
    for page in 0..page_count as u8 {
        let selected = pack_non_overlapping(
            candidates
                .iter()
                .filter(|(candidate_page, _, _)| *candidate_page == page)
                .copied()
                .collect(),
            limit,
        );
        if selected.len() == limit {
            return selected;
        }
        if selected.len() > best.len() {
            best = selected;
        }
    }
    best
}

fn available_cells(
    source_ink: &SourceInkMap,
    role: CharacterSelectFontRole,
    occupied: &[Cell],
) -> Vec<(u8, [u8; 2], Cell)> {
    let size = role.cell_size();
    let alignment = match role {
        CharacterSelectFontRole::SelectHeading => 32,
        CharacterSelectFontRole::ModeMenuHeading => 4,
        CharacterSelectFontRole::ModeMenuLabel => 4,
        CharacterSelectFontRole::CooperativeEmblemCharacter => 2,
        CharacterSelectFontRole::TournamentBracketLabel => 1,
        CharacterSelectFontRole::TournamentCertificateTitle => 1,
        CharacterSelectFontRole::TournamentCertificateLabel => 2,
        CharacterSelectFontRole::TournamentCertificateBody => 6,
        CharacterSelectFontRole::Label => 20,
        CharacterSelectFontRole::RosterName => 4,
        CharacterSelectFontRole::FixedPrompt => 4,
        CharacterSelectFontRole::CompactPrompt => 4,
        CharacterSelectFontRole::SoloStatePrompt => 4,
        CharacterSelectFontRole::CommonPauseMenu => 4,
        CharacterSelectFontRole::SoloStoryIntro => 1,
        CharacterSelectFontRole::SoloEpisodeCard => 1,
        CharacterSelectFontRole::PracticalSelectionLabel => 1,
        CharacterSelectFontRole::StageLabel => 20,
        CharacterSelectFontRole::LeagueStandingLabel => 1,
        CharacterSelectFontRole::SelectionHelp => 4,
    };
    available_rectangles(source_ink, size, alignment, occupied)
}

fn available_rectangles(
    source_ink: &SourceInkMap,
    [width, height]: [usize; 2],
    alignment: usize,
    occupied: &[Cell],
) -> Vec<(u8, [u8; 2], Cell)> {
    let mut cells = Vec::new();
    let page_count = source_ink.width.div_ceil(256);
    for page in 0..page_count {
        let page_width = (source_ink.width - page * 256).min(256);
        if page_width < width || source_ink.height < height {
            continue;
        }
        for y in (0..=source_ink.height - height).step_by(alignment) {
            for u in (0..=page_width - width).step_by(alignment) {
                let cell = Cell {
                    x: page * 256 + u,
                    y,
                    width,
                    height,
                };
                if source_ink.is_blank(cell)
                    && occupied
                        .iter()
                        .all(|protected| !cells_overlap(*protected, cell))
                {
                    cells.push((page as u8, [u as u8, y as u8], cell));
                }
            }
        }
    }
    cells
}

fn requested_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    bindings: &[CharacterSelectDynamicTextBinding],
) -> Result<BTreeSet<(CharacterSelectFontRole, char)>> {
    let localized_by_id = localized_sources
        .iter()
        .map(|source| (source.source_ui_id.as_str(), source))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut requested = BTreeSet::new();
    for binding in bindings.iter().filter(|binding| {
        matches!(
            binding.font_role,
            CharacterSelectFontRole::SelectHeading | CharacterSelectFontRole::ModeMenuHeading
        )
    }) {
        let Some(source) = localized_by_id.get(binding.source_ui_id) else {
            continue;
        };
        requested.extend(
            source
                .korean_text
                .chars()
                .map(|character| (binding.font_role, character)),
        );
    }
    Ok(requested)
}

fn source_atlas_sha256(source_decoded: &[u8], tim_offset: usize) -> Result<String> {
    let tim = crate::tim::parse_4bpp_prefix(
        source_decoded
            .get(tim_offset..)
            .context("character-select atlas offset is outside the decoded source")?,
    )?;
    Ok(sha256_bytes(
        &source_decoded[tim_offset..tim_offset + tim.total_size],
    ))
}

fn protected_cells_for_surface(
    surface: CharacterSelectTextureSurface,
    source_glyph_cells: &[super::model::CharacterSelectSourceGlyphCell],
    fixed_strips: &[super::model::CharacterSelectFixedStripAllocation],
    native_cells: &[Cell],
) -> Vec<Cell> {
    source_glyph_cells
        .iter()
        .filter(|source_cell| source_cell.surface.shares_physical_texture(surface))
        .map(|source_cell| source_cell.cell)
        .chain(
            fixed_strips
                .iter()
                .filter(|strip| strip.surface.shares_physical_texture(surface))
                .map(|strip| strip.cell),
        )
        .chain(native_cells.iter().copied())
        .collect()
}

fn validate_allocations(allocations: &[CharacterSelectGlyphAllocation]) -> Result<()> {
    for (index, left) in allocations.iter().enumerate() {
        for right in &allocations[index + 1..] {
            ensure!(
                !left.surface.shares_physical_texture(right.surface)
                    || !cells_overlap(left.cell, right.cell),
                "character-select allocations {:?} and {:?} overlap",
                left.character,
                right.character
            );
        }
    }
    Ok(())
}

fn validate_segmented_fixed_strip_conflicts(
    segmented: &[CharacterSelectSegmentedFixedStripAllocation],
    fixed: &[super::model::CharacterSelectFixedStripAllocation],
    glyphs: &[CharacterSelectGlyphAllocation],
) -> Result<()> {
    let mut physical_segments = Vec::new();
    for strip in segmented {
        ensure!(
            strip.logical_width > 0
                && strip.logical_height > 0
                && strip.tracking_px.is_finite()
                && strip.horizontal_scale > 0
                && strip.logical_width.is_multiple_of(strip.horizontal_scale)
                && !strip.segments.is_empty(),
            "segmented character-select strip {} has an empty layout",
            strip.logical_text_region_id
        );
        for segment in &strip.segments {
            ensure!(
                segment.logical_origin[0] + segment.cell.width <= strip.logical_width
                    && segment.logical_origin[1] + segment.cell.height <= strip.logical_height,
                "segmented character-select strip {} escapes its logical raster",
                strip.logical_text_region_id
            );
            for fixed_strip in fixed {
                ensure!(
                    strip.tim_offset != fixed_strip.tim_offset
                        || !strip.surface.shares_physical_texture(fixed_strip.surface)
                        || !cells_overlap(segment.cell, fixed_strip.cell),
                    "segmented character-select region {} overlaps fixed region {}",
                    segment.physical_text_region_id,
                    fixed_strip.physical_text_region_id
                );
            }
            for glyph in glyphs {
                ensure!(
                    strip.tim_offset != glyph.tim_offset
                        || !strip.surface.shares_physical_texture(glyph.surface)
                        || !cells_overlap(segment.cell, glyph.cell),
                    "segmented character-select region {} overlaps glyph {:?}",
                    segment.physical_text_region_id,
                    glyph.character
                );
            }
            for (surface, tim_offset, cell, region_id) in &physical_segments {
                ensure!(
                    strip.tim_offset != *tim_offset
                        || !strip.surface.shares_physical_texture(*surface)
                        || !cells_overlap(segment.cell, *cell),
                    "segmented character-select regions {} and {} overlap",
                    segment.physical_text_region_id,
                    region_id
                );
            }
            physical_segments.push((
                strip.surface,
                strip.tim_offset,
                segment.cell,
                segment.physical_text_region_id,
            ));
        }
    }
    Ok(())
}

const fn role_key(role: CharacterSelectFontRole) -> &'static str {
    match role {
        CharacterSelectFontRole::SelectHeading => "select-heading",
        CharacterSelectFontRole::ModeMenuHeading => "mode-menu-heading",
        CharacterSelectFontRole::ModeMenuLabel => "mode-menu-label",
        CharacterSelectFontRole::CooperativeEmblemCharacter => "cooperative-emblem-character",
        CharacterSelectFontRole::TournamentBracketLabel => "tournament-bracket-label",
        CharacterSelectFontRole::TournamentCertificateTitle => "tournament-certificate-title",
        CharacterSelectFontRole::TournamentCertificateLabel => "tournament-certificate-label",
        CharacterSelectFontRole::TournamentCertificateBody => "tournament-certificate-body",
        CharacterSelectFontRole::Label => "label",
        CharacterSelectFontRole::RosterName => "roster-name",
        CharacterSelectFontRole::FixedPrompt => "fixed-prompt",
        CharacterSelectFontRole::CompactPrompt => "compact-prompt",
        CharacterSelectFontRole::SoloStatePrompt => "solo-state-prompt",
        CharacterSelectFontRole::CommonPauseMenu => "common-pause-menu",
        CharacterSelectFontRole::SoloStoryIntro => "solo-story-intro",
        CharacterSelectFontRole::SoloEpisodeCard => "solo-episode-card",
        CharacterSelectFontRole::PracticalSelectionLabel => "practical-selection-label",
        CharacterSelectFontRole::StageLabel => "stage-label",
        CharacterSelectFontRole::LeagueStandingLabel => "league-standing-label",
        CharacterSelectFontRole::SelectionHelp => "selection-help",
    }
}

#[cfg(test)]
mod segmented_conflict_tests {
    use super::validate_segmented_fixed_strip_conflicts;
    use crate::character_select_graphics::model::{
        CharacterSelectFixedStripSegment, CharacterSelectFixedStripWriteMode,
        CharacterSelectFontRole, CharacterSelectSegmentedFixedStripAllocation,
        CharacterSelectTextFlow, CharacterSelectTextSelection, CharacterSelectTextureSurface,
    };
    use crate::tim::Cell;

    fn episode_card_segment(
        logical_text_region_id: &str,
        physical_text_region_id: &'static str,
        tim_offset: usize,
    ) -> CharacterSelectSegmentedFixedStripAllocation {
        CharacterSelectSegmentedFixedStripAllocation {
            logical_text_region_id: logical_text_region_id.to_string(),
            source_ui_ids: vec![logical_text_region_id.to_string()],
            translation_id: logical_text_region_id.to_string(),
            text_selection: CharacterSelectTextSelection::Entire,
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::SoloEpisodeCard,
            surface: CharacterSelectTextureSurface::SoloEpisodeCardAtlas,
            tim_offset,
            logical_width: 120,
            logical_height: 40,
            tracking_px: 0.0,
            horizontal_scale: 1,
            segments: vec![CharacterSelectFixedStripSegment {
                physical_text_region_id,
                cell: Cell {
                    x: 0,
                    y: 0,
                    width: 120,
                    height: 40,
                },
                logical_origin: [0, 0],
            }],
            clear_index: 15,
        }
    }

    #[test]
    fn identical_cells_in_distinct_tim_members_do_not_conflict() {
        let first = episode_card_segment("card-1-heading", "card-1-heading", 0);
        let second = episode_card_segment("card-2-heading", "card-2-heading", 0x4840);

        assert!(validate_segmented_fixed_strip_conflicts(&[first, second], &[], &[]).is_ok());
    }

    #[test]
    fn overlapping_cells_in_one_tim_member_still_fail_closed() {
        let first = episode_card_segment("card-1-heading", "card-1-heading", 0);
        let second = episode_card_segment("card-1-alias", "card-1-alias", 0);

        assert!(validate_segmented_fixed_strip_conflicts(&[first, second], &[], &[]).is_err());
    }
}
