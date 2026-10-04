use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers, RasterizedIndexedText};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    Cell, cells_overlap, read_4bpp_indexed_image_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::{changed_ranges_are_within, merge_byte_ranges};

use super::super::atlas_packer::{AtlasRectangle, pack_atlas_rectangles};
use super::super::model::{ModeDescendantFontRole, ModeDescendantFontSources};
use super::super::practical_results::RESIDENT_RESULT_PROVIDER_CELLS;
use super::catalog::{
    PRACTICAL_EXAM_CATALOG, PracticalExamCatalogEntry, PracticalExamPlacement,
    PracticalExamTranslation,
};
use super::descriptor::{
    PracticalExamConsumer, PracticalExamConsumerAudit, PracticalExamTextureRegion,
    cell_avoids_protected_regions, reclaimed_source_regions, retained_protected_union,
    validate_bound_descriptor,
};

const PRACTICAL_EXAM_TIM_OFFSET: usize = 0x3c800;
const PRACTICAL_EXAM_TIM_SIZE: usize = 0x18140;
const PRACTICAL_EXAM_TIM_SHA256: &str =
    "b1de25ba8ea198280830cdd9777ca7a4e733a1b73cabec32bc52d695e08aa2e0";
const TEXTURE_WIDTH: usize = 768;
const TEXTURE_HEIGHT: usize = 256;
const TEXTURE_PAGE_WIDTH: usize = 256;
const ALLOCATABLE_PAGE_COUNT: usize = 3;
const ALLOCATABLE_WIDTH: usize = TEXTURE_PAGE_WIDTH * ALLOCATABLE_PAGE_COUNT;
const GLYPH_RASTER_WIDTH: usize = 64;
const MAX_GLYPH_WIDTH: usize = 32;
const GLYPH_ATLAS_TOP: usize = 2;
const CLEAR_INDEX: u8 = 0;
const MENU_OUTLINE_INDEX: u8 = 2;
const MENU_FILL_INDEX: u8 = 14;

const HINT_COMPOSITE_CELL: Cell = Cell {
    x: 512,
    y: 64,
    width: 104,
    height: 64,
};
const HINT_COMPOSITE_INDEXED_SHA256: &str =
    "783e689d4e57919774fdc962f10cce454dde98cd43bc2627ab8ff59525d5a879";
const HINT_LABEL_CELLS: [Cell; 2] = [
    Cell {
        x: 568,
        y: 68,
        width: 48,
        height: 28,
    },
    Cell {
        x: 568,
        y: 96,
        width: 48,
        height: 28,
    },
];
const HINT_BACKGROUND_PERIOD: usize = 2;
const HINT_OUTLINE_INDEX: u8 = 5;
const HINT_FILL_INDEX: u8 = 1;
const ACTIVE_TITLE_CACHE_RASTER_HEIGHT: usize = 16;
const ACTIVE_TITLE_CACHE_TOP_PADDING: usize = 4;
const ACTIVE_TITLE_CACHE_STRIPS: [Cell; 2] = [
    Cell {
        x: 704,
        y: 204,
        width: 64,
        height: 20,
    },
    Cell {
        x: 544,
        y: 236,
        width: 32,
        height: 20,
    },
];
const ACTIVE_TITLE_SEMANTIC_IDS: [&str; 3] = [
    "practical_first_term_exam",
    "practical_second_term_exam",
    "practical_school_year_exam",
];

pub(super) type PracticalExamGlyphKey = (ModeDescendantFontRole, char);

#[derive(Debug, Clone)]
pub(super) struct PracticalExamGlyphBuild {
    pub(super) code: u8,
    pub(super) cell: Cell,
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) measured_advance_px: f32,
    pub(super) atlas_advance_px: u8,
    pub(super) changed_decoded_byte_count: usize,
}

#[derive(Debug, Clone)]
pub(super) enum PracticalExamSemanticPlacementBuild {
    GlyphStream {
        glyph_keys: Vec<PracticalExamGlyphKey>,
        glyph_codes: Vec<u8>,
    },
    GuardedHintComposite {
        cell: Cell,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PracticalExamSemanticBuild {
    pub(super) id: &'static str,
    pub(super) descriptor_index: usize,
    pub(super) korean_text: String,
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) measured_advance_px: f32,
    pub(super) atlas_advance_px: usize,
    pub(super) placement: PracticalExamSemanticPlacementBuild,
}

#[derive(Debug, Clone)]
pub(super) struct PracticalExamHintBuild {
    pub(super) id: &'static str,
    pub(super) korean_text: String,
    pub(super) cell: Cell,
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) measured_advance_px: f32,
    pub(super) changed_decoded_byte_count: usize,
}

pub(super) struct PracticalExamGlyphAtlasPlan {
    pub(super) consumer: PracticalExamConsumer,
    pub(super) patched_decoded: Vec<u8>,
    pub(super) glyphs: BTreeMap<PracticalExamGlyphKey, PracticalExamGlyphBuild>,
    pub(super) semantics: Vec<PracticalExamSemanticBuild>,
    pub(super) hint: PracticalExamHintBuild,
    pub(super) active_title_cache: Option<PracticalExamActiveTitleCachePlan>,
    pub(super) glyph_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct PracticalExamActiveTitleCacheGlyph {
    pub(super) character: char,
    pub(super) cell: Cell,
    pub(super) pixels: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(in crate::mode_descendant_graphics) struct PracticalExamActiveTitleCachePlan {
    pub(super) glyphs: BTreeMap<char, PracticalExamActiveTitleCacheGlyph>,
    pub(super) covers_all_term_titles: bool,
    allowed_ranges: Vec<[usize; 2]>,
}

impl PracticalExamActiveTitleCachePlan {
    pub(in crate::mode_descendant_graphics) fn covers_all_term_titles(&self) -> bool {
        self.covers_all_term_titles && !self.glyphs.is_empty()
    }

    pub(in crate::mode_descendant_graphics) fn result_write_cells(&self) -> Result<Vec<Cell>> {
        self.glyphs
            .values()
            .map(|glyph| {
                Ok(Cell {
                    x: glyph
                        .cell
                        .x
                        .checked_sub(512)
                        .context("active-title cache glyph is outside the matched result page")?,
                    y: glyph.cell.y + ACTIVE_TITLE_CACHE_TOP_PADDING,
                    width: glyph.cell.width,
                    height: ACTIVE_TITLE_CACHE_RASTER_HEIGHT,
                })
            })
            .collect()
    }
}

struct PendingGlyph {
    key: PracticalExamGlyphKey,
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    font: FontMetadata,
    measured_advance_px: f32,
    is_blank: bool,
}

#[derive(Clone)]
struct FontMetadata {
    name: String,
    sha256: String,
    font_px: f32,
    vertical_shift_px: i32,
}

struct RenderedHint {
    build: PracticalExamHintBuild,
    allowed_ranges: Vec<[usize; 2]>,
}

pub(super) fn build_practical_exam_glyph_atlas(
    target: &str,
    source_decoded: &[u8],
    audit: &PracticalExamConsumerAudit,
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    fonts: &ModeDescendantFontSources,
) -> Result<PracticalExamGlyphAtlasPlan> {
    validate_source_texture(source_decoded)?;
    validate_consumer_bound_set(audit.consumer, bound)?;
    ensure!(
        audit.direct_texture_reads_complete,
        "practical-exam consumer {:?} has no complete direct shared-TIM execution contract",
        audit.consumer
    );

    let mut rewritten_descriptors = BTreeSet::new();
    let mut hint_binding = None;
    for (catalog, translation) in bound {
        ensure_translation_matches_catalog(catalog, translation)?;
        let binding = binding_for_consumer(catalog, audit.consumer)?;
        validate_bound_descriptor(audit, binding)?;
        match catalog.placement {
            PracticalExamPlacement::DynamicStrip => {
                ensure!(
                    rewritten_descriptors.insert((audit.consumer, binding.descriptor_index)),
                    "duplicate practical-exam rewritten descriptor {}",
                    binding.descriptor_index
                );
            }
            PracticalExamPlacement::GuardedHintComposite => ensure!(
                hint_binding.replace((*catalog, *translation)).is_none(),
                "duplicate practical-exam guarded hint binding"
            ),
        }
    }

    let audits = [audit];
    let retained = retained_protected_union(&audits, &rewritten_descriptors);
    let reclaimable = reclaimed_source_regions(&audits, &rewritten_descriptors);
    validate_reclaim_and_protection(audit, &rewritten_descriptors, &reclaimable, &retained)?;
    let mut placement_protected = retained.clone();
    placement_protected.push(PracticalExamTextureRegion {
        id: "guarded-hint-composite",
        cell: HINT_COMPOSITE_CELL,
    });
    if audit.consumer == PracticalExamConsumer::Exam1999 {
        placement_protected.extend(ACTIVE_TITLE_CACHE_STRIPS.map(|cell| {
            PracticalExamTextureRegion {
                id: "active-title-cache",
                cell,
            }
        }));
    }
    placement_protected.extend(RESIDENT_RESULT_PROVIDER_CELLS.map(|cell| {
        PracticalExamTextureRegion {
            id: "resident-practical-result-provider",
            cell,
        }
    }));

    let source_indexed =
        read_4bpp_indexed_image_in_prefix(source_decoded, PRACTICAL_EXAM_TIM_OFFSET)?;
    ensure!(
        source_indexed.width == TEXTURE_WIDTH && source_indexed.height == TEXTURE_HEIGHT,
        "practical-exam glyph atlas texture geometry changed"
    );
    let occupancy = ProtectedAtlasOccupancy::from_regions(&placement_protected)?;
    let mut rasterizers = IndexedTextRasterizers::default();
    let pending = rasterize_glyph_set(bound, fonts, &mut rasterizers)?;
    ensure!(
        pending.len() <= usize::from(u8::MAX) + 1,
        "practical-exam consumer {:?} needs {} local glyph codes but only 256 exist",
        audit.consumer,
        pending.len()
    );

    let required_pixel_count = pending
        .iter()
        .map(|glyph| glyph.width * glyph.height)
        .sum::<usize>();
    let candidate_pixel_count = occupancy.free_pixel_count();
    ensure!(
        required_pixel_count <= candidate_pixel_count,
        "practical-exam consumer {:?} needs {required_pixel_count} glyph pixels but only {candidate_pixel_count} audited unowned pixels exist in the shared TIM",
        audit.consumer
    );
    let allocated_cells = plan_glyph_cells(&occupancy, &pending, audit.consumer)?;

    let mut patched = source_decoded.to_vec();
    let mut glyph_allowed_ranges = Vec::new();
    let mut owned_cells = Vec::with_capacity(pending.len() + HINT_LABEL_CELLS.len());
    let mut glyphs = BTreeMap::new();
    for (code, (glyph, cell)) in pending.into_iter().zip(allocated_cells).enumerate() {
        ensure!(
            cell_avoids_protected_regions(cell, &placement_protected),
            "practical-exam local glyph {:?} overlaps a retained consumer",
            glyph.key
        );
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched,
            PRACTICAL_EXAM_TIM_OFFSET,
            cell,
            &glyph.pixels,
        )?;
        ensure!(
            glyph.is_blank || write.changed_byte_count > 0,
            "practical-exam local glyph {:?} changed no texture bytes",
            glyph.key
        );
        let written = read_indexed_cell_in_prefix(&patched, PRACTICAL_EXAM_TIM_OFFSET, cell)?;
        ensure!(
            written == glyph.pixels,
            "practical-exam local glyph {:?} failed indexed-pixel readback",
            glyph.key
        );
        glyph_allowed_ranges.extend(write.allowed_ranges);
        owned_cells.push(cell);
        let width = u8::try_from(glyph.width)?;
        let build = PracticalExamGlyphBuild {
            code: u8::try_from(code)?,
            cell,
            font_name: glyph.font.name,
            font_sha256: glyph.font.sha256,
            font_px: glyph.font.font_px,
            vertical_shift_px: glyph.font.vertical_shift_px,
            measured_advance_px: glyph.measured_advance_px,
            atlas_advance_px: width,
            changed_decoded_byte_count: write.changed_byte_count,
        };
        ensure!(
            glyphs.insert(glyph.key, build).is_none(),
            "duplicate practical-exam local glyph {:?}",
            glyph.key
        );
    }

    let (hint_catalog, hint_translation) =
        hint_binding.context("practical-exam guarded hint translation is missing")?;
    let rendered_hint = render_hint_composite(
        &mut patched,
        &mut rasterizers,
        hint_catalog,
        hint_translation,
        fonts,
    )?;
    let glyph_allowed_ranges = merge_byte_ranges(glyph_allowed_ranges);
    let hint_allowed_ranges = merge_byte_ranges(rendered_hint.allowed_ranges);
    owned_cells.extend(HINT_LABEL_CELLS);
    let active_title_cache = if audit.consumer == PracticalExamConsumer::Exam1999 {
        let cache = install_active_title_cache(&mut patched, bound, &glyphs)?;
        owned_cells.extend(cache.glyphs.values().map(|glyph| glyph.cell));
        Some(cache)
    } else {
        None
    };

    let semantics = build_semantic_report(bound, audit.consumer, &glyphs, &rendered_hint.build)?;
    ensure!(
        semantics.len() == bound.len(),
        "practical-exam consumer {:?} semantic report omitted a bound translation",
        audit.consumer
    );
    let mut allowed_ranges = glyph_allowed_ranges.clone();
    allowed_ranges.extend(hint_allowed_ranges.iter().copied());
    if let Some(cache) = &active_title_cache {
        allowed_ranges.extend(cache.allowed_ranges.iter().copied());
    }
    let allowed_ranges = merge_byte_ranges(allowed_ranges);
    let changed = difference_ranges(source_decoded, &patched);
    ensure!(
        !changed.is_empty() && changed_ranges_are_within(&changed, &allowed_ranges),
        "practical-exam consumer {:?} changed bytes outside glyph cells and guarded hint labels",
        audit.consumer
    );
    let patched_indexed = read_4bpp_indexed_image_in_prefix(&patched, PRACTICAL_EXAM_TIM_OFFSET)?;
    let changed_indexed_pixel_count = ensure_pixel_changes_are_owned(
        &source_indexed.pixels,
        &patched_indexed.pixels,
        &owned_cells,
    )?;
    let allocated_pixel_count = glyphs
        .values()
        .map(|glyph| glyph.cell.width * glyph.cell.height)
        .sum::<usize>();
    let changed_decoded_byte_count = changed
        .iter()
        .map(|[start, end]| end - start)
        .sum::<usize>();
    ensure!(
        allocated_pixel_count > 0
            && changed_indexed_pixel_count > 0
            && changed_decoded_byte_count > 0,
        "practical-exam glyph atlas changed no owned pixels or bytes"
    );
    let glyph_count = glyphs.len();
    ensure!(
        glyph_count
            == glyphs
                .values()
                .map(|glyph| usize::from(glyph.code))
                .collect::<BTreeSet<_>>()
                .len(),
        "practical-exam consumer {:?} local glyph codes are not unique",
        audit.consumer
    );
    let source_sha256 = sha256_bytes(source_decoded);
    let mut write_claims = DecodedDataClaim::from_effective_ranges(
        &format!("practical-texture:{:?}:glyph-atlas", audit.consumer),
        "install the practical-exam dynamic Hangul glyph atlas",
        source_decoded,
        &patched,
        glyph_allowed_ranges,
    )?;
    write_claims.extend(DecodedDataClaim::from_effective_ranges(
        &format!("practical-texture:{:?}:guarded-hint", audit.consumer),
        "replace the practical-exam guarded hint composite",
        source_decoded,
        &patched,
        hint_allowed_ranges,
    )?);
    if let Some(cache) = &active_title_cache {
        write_claims.extend(DecodedDataClaim::from_effective_ranges(
            "practical-texture:Exam1999:active-title-cache",
            "install the producer-matched active term-title glyph cache",
            source_decoded,
            &patched,
            cache.allowed_ranges.iter().copied(),
        )?);
    }
    let mut plan = DecodedRecordWritePlan::new(target, source_decoded, &source_sha256)?;
    plan.register_data_candidate(
        "practical-exam texture builder",
        &source_sha256,
        &patched,
        &write_claims,
    )?;
    let patched_decoded = plan.apply(None)?;
    ensure!(
        patched_decoded == patched,
        "practical-exam texture write plan changed its verified candidate"
    );
    Ok(PracticalExamGlyphAtlasPlan {
        consumer: audit.consumer,
        patched_decoded,
        glyphs,
        semantics,
        hint: rendered_hint.build,
        active_title_cache,
        glyph_count,
    })
}

fn install_active_title_cache(
    patched: &mut [u8],
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    glyphs: &BTreeMap<PracticalExamGlyphKey, PracticalExamGlyphBuild>,
) -> Result<PracticalExamActiveTitleCachePlan> {
    let title_entries = bound
        .iter()
        .filter(|(catalog, _)| ACTIVE_TITLE_SEMANTIC_IDS.contains(&catalog.id))
        .collect::<Vec<_>>();
    ensure!(
        title_entries.len() == ACTIVE_TITLE_SEMANTIC_IDS.len()
            && ACTIVE_TITLE_SEMANTIC_IDS
                .iter()
                .all(|id| { title_entries.iter().any(|(catalog, _)| catalog.id == *id) }),
        "1999 practical-exam active-title cache lost a term-title semantic"
    );

    let characters = title_entries
        .iter()
        .flat_map(|(_, translation)| translation.korean_text.chars())
        .collect::<BTreeSet<_>>();
    let mut pending = Vec::with_capacity(characters.len());
    for character in characters {
        let source = glyphs
            .get(&(ModeDescendantFontRole::PracticalMenuLabel, character))
            .with_context(|| format!("active-title cache glyph {character:?} is missing"))?;
        let source_pixels =
            read_indexed_cell_in_prefix(patched, PRACTICAL_EXAM_TIM_OFFSET, source.cell)?;
        // The native cache has 96 columns across two strips. Keep the
        // established 3/4 horizontal scale without rounding every glyph up;
        // centered sampling below still covers its complete source cell.
        let width = (source.cell.width * 3 / 4).max(1);
        ensure!(
            width <= MAX_GLYPH_WIDTH,
            "active-title cache glyph is too wide"
        );
        let mut pixels = vec![CLEAR_INDEX; width * 20];
        for target_y in 0..ACTIVE_TITLE_CACHE_RASTER_HEIGHT {
            // Use the complete allocated glyph, not the old hard-coded 18-row
            // band. Center samples retain its bottom row when shrinking it.
            let source_y =
                (2 * target_y + 1) * source.cell.height / (2 * ACTIVE_TITLE_CACHE_RASTER_HEIGHT);
            for target_x in 0..width {
                let source_x = (2 * target_x + 1) * source.cell.width / (2 * width);
                // Active instruction/result sprites use the grayscale CLUT at
                // (0,503), not the menu CLUT at (16,481): black=1, white=15.
                let pixel = match source_pixels[source_y * source.cell.width + source_x] {
                    CLEAR_INDEX => CLEAR_INDEX,
                    MENU_OUTLINE_INDEX => 1,
                    MENU_FILL_INDEX => 15,
                    other => {
                        anyhow::bail!("unexpected menu palette index {other} in active-title cache")
                    }
                };
                pixels[(ACTIVE_TITLE_CACHE_TOP_PADDING + target_y) * width + target_x] = pixel;
            }
        }
        pending.push((character, width, pixels));
    }
    pending.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let mut strip_used = [0usize; ACTIVE_TITLE_CACHE_STRIPS.len()];
    let mut installed = BTreeMap::new();
    let mut allowed_ranges = Vec::new();
    for (character, width, pixels) in pending {
        let strip_index = ACTIVE_TITLE_CACHE_STRIPS
            .iter()
            .enumerate()
            .find_map(|(index, strip)| (strip_used[index] + width <= strip.width).then_some(index))
            .with_context(|| format!("active-title cache has no room for {character:?}"))?;
        let strip = ACTIVE_TITLE_CACHE_STRIPS[strip_index];
        let cell = Cell {
            x: strip.x + strip_used[strip_index],
            y: strip.y,
            width,
            height: 20,
        };
        strip_used[strip_index] += width;
        let write = write_indexed_cell_in_prefix_with_report(
            patched,
            PRACTICAL_EXAM_TIM_OFFSET,
            cell,
            &pixels,
        )?;
        allowed_ranges.extend(write.allowed_ranges);
        ensure!(
            character == ' ' || write.changed_byte_count > 0,
            "active-title cache glyph {character:?} changed no pixels"
        );
        ensure!(
            installed
                .insert(
                    character,
                    PracticalExamActiveTitleCacheGlyph {
                        character,
                        cell,
                        pixels,
                    },
                )
                .is_none(),
            "active-title cache installed a duplicate glyph"
        );
    }
    let covers_all_term_titles = title_entries.iter().all(|(_, translation)| {
        translation
            .korean_text
            .chars()
            .all(|character| installed.contains_key(&character))
    });
    ensure!(
        covers_all_term_titles && strip_used.iter().all(|used| *used > 0),
        "active-title cache did not cover every term title across both producer-matched strips"
    );
    Ok(PracticalExamActiveTitleCachePlan {
        glyphs: installed,
        covers_all_term_titles,
        allowed_ranges: merge_byte_ranges(allowed_ranges),
    })
}

fn plan_glyph_cells(
    occupancy: &ProtectedAtlasOccupancy,
    glyphs: &[PendingGlyph],
    consumer: PracticalExamConsumer,
) -> Result<Vec<Cell>> {
    let rectangles = glyphs
        .iter()
        .map(|glyph| AtlasRectangle {
            width: glyph.width,
            height: glyph.height,
        })
        .collect::<Vec<_>>();
    let cells = pack_atlas_rectangles(
        &occupancy.occupied,
        TEXTURE_PAGE_WIDTH,
        TEXTURE_HEIGHT,
        &rectangles,
    )
    .with_context(|| format!("failed to pack {consumer:?} practical-exam glyph atlas"))?;
    ensure!(
        cells.len() == glyphs.len()
            && cells.iter().zip(glyphs).all(|(cell, glyph)| {
                cell.width == glyph.width
                    && cell.height == glyph.height
                    && cell.x + cell.width <= ALLOCATABLE_WIDTH
                    && cell.y + cell.height <= TEXTURE_HEIGHT
            })
            && cells.iter().enumerate().all(|(index, cell)| {
                cells[index + 1..]
                    .iter()
                    .all(|other| !cells_overlap(*cell, *other))
            }),
        "{consumer:?} practical-exam atlas packer returned an invalid placement set"
    );
    Ok(cells)
}

fn validate_source_texture(source_decoded: &[u8]) -> Result<()> {
    let tim = source_decoded
        .get(PRACTICAL_EXAM_TIM_OFFSET..PRACTICAL_EXAM_TIM_OFFSET + PRACTICAL_EXAM_TIM_SIZE)
        .context("practical-exam glyph atlas TIM is truncated")?;
    ensure!(
        sha256_bytes(tim) == PRACTICAL_EXAM_TIM_SHA256,
        "practical-exam glyph atlas TIM source changed"
    );
    Ok(())
}

fn validate_consumer_bound_set(
    consumer: PracticalExamConsumer,
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
) -> Result<()> {
    let expected = PRACTICAL_EXAM_CATALOG
        .iter()
        .filter(|entry| {
            entry
                .bindings
                .iter()
                .any(|binding| binding.consumer == consumer)
        })
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    let actual = bound
        .iter()
        .map(|(catalog, _)| catalog.id)
        .collect::<BTreeSet<_>>();
    ensure!(
        actual.len() == bound.len(),
        "practical-exam consumer {consumer:?} bound translations contain duplicate semantics"
    );
    ensure!(
        actual == expected,
        "practical-exam consumer {consumer:?} bound translation set is incomplete or contains another consumer"
    );
    Ok(())
}

fn ensure_translation_matches_catalog(
    catalog: &PracticalExamCatalogEntry,
    translation: &PracticalExamTranslation,
) -> Result<()> {
    ensure!(
        translation.id == catalog.id
            && translation.source_text == catalog.source_text
            && translation.font_role == catalog.font_role,
        "practical-exam translation {} no longer matches its catalog entry",
        catalog.id
    );
    ensure!(
        !translation.korean_text.is_empty(),
        "practical-exam translation {} is empty",
        catalog.id
    );
    Ok(())
}

fn binding_for_consumer(
    catalog: &PracticalExamCatalogEntry,
    consumer: PracticalExamConsumer,
) -> Result<super::catalog::PracticalExamDescriptorBinding> {
    let bindings = catalog
        .bindings
        .iter()
        .copied()
        .filter(|binding| binding.consumer == consumer)
        .collect::<Vec<_>>();
    ensure!(
        bindings.len() == 1,
        "practical-exam semantic {} has {} bindings for consumer {consumer:?}",
        catalog.id,
        bindings.len()
    );
    Ok(bindings[0])
}

fn validate_reclaim_and_protection(
    audit: &PracticalExamConsumerAudit,
    rewritten_descriptors: &BTreeSet<(PracticalExamConsumer, usize)>,
    reclaimable: &[PracticalExamTextureRegion],
    retained: &[PracticalExamTextureRegion],
) -> Result<()> {
    ensure!(
        !reclaimable.is_empty() && !retained.is_empty(),
        "practical-exam consumer {:?} has no reclaimable or retained texture census",
        audit.consumer
    );
    for region in reclaimable {
        ensure!(
            audit.descriptors.iter().any(|descriptor| {
                rewritten_descriptors.contains(&(audit.consumer, descriptor.index))
                    && descriptor
                        .fragments
                        .iter()
                        .any(|fragment| fragment.cell() == region.cell)
            }),
            "practical-exam consumer {:?} attempted to reclaim a non-translated source region",
            audit.consumer
        );
    }
    if audit.consumer == PracticalExamConsumer::BasicsReview {
        ensure!(
            audit.descriptors.len() > 15,
            "practical-exam basics numeric descriptor census is truncated"
        );
        for descriptor in &audit.descriptors[10..=15] {
            ensure!(
                !rewritten_descriptors.contains(&(audit.consumer, descriptor.index)),
                "practical-exam basics numeric descriptor {} was reclaimed",
                descriptor.index
            );
            for fragment in &descriptor.fragments {
                ensure!(
                    retained.iter().any(|region| region.cell == fragment.cell()),
                    "practical-exam basics numeric descriptor {} is not allocator-protected",
                    descriptor.index
                );
            }
        }
    }
    Ok(())
}

fn rasterize_glyph_set(
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    fonts: &ModeDescendantFontSources,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<Vec<PendingGlyph>> {
    let keys = bound
        .iter()
        .filter(|(catalog, _)| catalog.placement == PracticalExamPlacement::DynamicStrip)
        .flat_map(|(catalog, translation)| {
            translation
                .korean_text
                .chars()
                .map(|character| (catalog.font_role, character))
        })
        .collect::<BTreeSet<_>>();
    ensure!(!keys.is_empty(), "practical-exam local glyph set is empty");
    ensure!(
        keys.iter().all(|(_, character)| {
            *character == ' ' || (!character.is_control() && !character.is_whitespace())
        }),
        "practical-exam glyph streams contain unsupported whitespace or control characters"
    );

    let mut pending = Vec::with_capacity(keys.len());
    let mut role_metadata = BTreeMap::new();
    for key in keys
        .iter()
        .copied()
        .filter(|(_, character)| *character != ' ')
    {
        let glyph = rasterize_nonspace_glyph(rasterizers, key, fonts)?;
        if let Some(existing) = role_metadata.get(&key.0) {
            ensure_same_font(existing, &glyph.font, key.0)?;
        } else {
            role_metadata.insert(key.0, glyph.font.clone());
        }
        pending.push(glyph);
    }
    for key in keys
        .iter()
        .copied()
        .filter(|(_, character)| *character == ' ')
    {
        let font = role_metadata
            .get(&key.0)
            .with_context(|| {
                format!(
                    "practical-exam role {:?} has no font identity witness",
                    key.0
                )
            })?
            .clone();
        let height = glyph_cell_height(key.0)?;
        let width = space_width(font.font_px);
        pending.push(PendingGlyph {
            key,
            pixels: vec![CLEAR_INDEX; width * height],
            width,
            height,
            font,
            measured_advance_px: width as f32,
            is_blank: true,
        });
    }
    ensure!(
        pending.len() == keys.len(),
        "practical-exam local glyph rasterization omitted or duplicated a key"
    );
    Ok(pending)
}

fn rasterize_nonspace_glyph(
    rasterizers: &mut IndexedTextRasterizers,
    key: PracticalExamGlyphKey,
    fonts: &ModeDescendantFontSources,
) -> Result<PendingGlyph> {
    let style = font_for_role(fonts, key.0)?;
    let source_height = glyph_source_height(key.0)?;
    let atlas_bottom = glyph_atlas_bottom(key.0)?;
    // SIKEN's title selects CLUT x=32 (green ink / dark outline); ordinary
    // labels select x=16 (white / dark outline), both at y=481. These are
    // role palettes, not coverage ramps: index 15 is blue/purple, not full ink.
    let (outline_index, fill_index) = match key.0 {
        ModeDescendantFontRole::PracticalTitle => (13, 9),
        ModeDescendantFontRole::PracticalMenuLabel | ModeDescendantFontRole::PracticalPrompt => {
            (MENU_OUTLINE_INDEX, MENU_FILL_INDEX)
        }
        _ => anyhow::bail!("unexpected practical menu glyph role {:?}", key.0),
    };
    let raster = rasterizers.for_font(&style.path)?.rasterize_shifted(
        &key.1.to_string(),
        GLYPH_RASTER_WIDTH,
        source_height,
        style.font_px,
        0.0,
        style.vertical_shift_px,
        CLEAR_INDEX,
        Some(outline_index),
        fill_index,
        HorizontalTextAlignment::Center,
    )?;
    ensure!(
        raster.ink_bounds[1] >= GLYPH_ATLAS_TOP && raster.ink_bounds[3] <= atlas_bottom,
        "practical-exam glyph {:?} escapes its guarded vertical atlas band {}..{} with ink {:?}",
        key,
        GLYPH_ATLAS_TOP,
        atlas_bottom,
        raster.ink_bounds
    );
    let (mut pixels, width) = crop_to_atlas_band_with_trailing_spacing(
        &raster,
        GLYPH_RASTER_WIDTH,
        source_height,
        GLYPH_ATLAS_TOP,
        atlas_bottom,
    )?;
    let cropped_height = atlas_bottom - GLYPH_ATLAS_TOP;
    let height = glyph_cell_height(key.0)?;
    ensure!(
        cropped_height <= height,
        "practical-exam glyph {key:?} exceeds its physical cell height"
    );
    pixels.resize(width * height, CLEAR_INDEX);
    ensure!(
        width <= MAX_GLYPH_WIDTH,
        "practical-exam glyph {key:?} is {width}px wide; local codes support at most {MAX_GLYPH_WIDTH}px"
    );
    Ok(PendingGlyph {
        key,
        pixels,
        width,
        height,
        font: FontMetadata {
            name: raster.font_name,
            sha256: raster.font_sha256,
            font_px: style.font_px,
            vertical_shift_px: style.vertical_shift_px,
        },
        measured_advance_px: raster.measured_advance_px,
        is_blank: false,
    })
}

fn crop_to_atlas_band_with_trailing_spacing(
    raster: &RasterizedIndexedText,
    source_width: usize,
    source_height: usize,
    top: usize,
    bottom: usize,
) -> Result<(Vec<u8>, usize)> {
    let left = raster.ink_bounds[0];
    let right = (raster.ink_bounds[2] + 1).min(source_width);
    ensure!(
        left < right,
        "practical-exam glyph has no horizontal extent"
    );
    let width = right - left;
    ensure!(
        top < bottom && bottom <= source_height,
        "practical-exam glyph atlas band is outside its source raster"
    );
    let mut pixels = Vec::with_capacity(width * (bottom - top));
    for y in top..bottom {
        pixels.extend_from_slice(&raster.pixels[y * source_width + left..y * source_width + right]);
    }
    Ok((pixels, width))
}

fn ensure_same_font(
    left: &FontMetadata,
    right: &FontMetadata,
    role: ModeDescendantFontRole,
) -> Result<()> {
    ensure!(
        left.name == right.name
            && left.sha256 == right.sha256
            && left.font_px == right.font_px
            && left.vertical_shift_px == right.vertical_shift_px,
        "practical-exam role {role:?} produced inconsistent font metadata"
    );
    Ok(())
}

fn glyph_source_height(role: ModeDescendantFontRole) -> Result<usize> {
    match role {
        ModeDescendantFontRole::PracticalTitle => Ok(32),
        ModeDescendantFontRole::PracticalMenuLabel | ModeDescendantFontRole::PracticalPrompt => {
            // The native cell remains 20px high. A larger scratch canvas keeps
            // complete top/bottom outlines observable before cropping.
            Ok(24)
        }
        ModeDescendantFontRole::PracticalHint => {
            anyhow::bail!("practical-exam hint uses its guarded composite")
        }
        _ => anyhow::bail!("non-practical font role reached the practical-exam glyph atlas"),
    }
}

fn glyph_atlas_bottom(role: ModeDescendantFontRole) -> Result<usize> {
    match role {
        ModeDescendantFontRole::PracticalTitle => Ok(29),
        ModeDescendantFontRole::PracticalMenuLabel | ModeDescendantFontRole::PracticalPrompt => {
            Ok(22)
        }
        ModeDescendantFontRole::PracticalHint => {
            anyhow::bail!("practical-exam hint uses its guarded composite")
        }
        _ => anyhow::bail!("non-practical font role reached the practical-exam glyph atlas"),
    }
}

fn glyph_cell_height(role: ModeDescendantFontRole) -> Result<usize> {
    match role {
        ModeDescendantFontRole::PracticalTitle => Ok(27),
        ModeDescendantFontRole::PracticalMenuLabel | ModeDescendantFontRole::PracticalPrompt => {
            // Source descriptors use 20px-high cells. Both guarded margins are
            // outside this band; no outline is clipped to create padding.
            Ok(20)
        }
        ModeDescendantFontRole::PracticalHint => {
            anyhow::bail!("practical-exam hint uses its guarded composite")
        }
        _ => anyhow::bail!("non-practical font role reached the practical-exam glyph atlas"),
    }
}

fn space_width(font_px: f32) -> usize {
    ((font_px * 0.35).round() as usize).clamp(4, MAX_GLYPH_WIDTH)
}

fn font_for_role(
    fonts: &ModeDescendantFontSources,
    role: ModeDescendantFontRole,
) -> Result<&crate::development_build_spec::ShiftedSizedFontSource> {
    match role {
        ModeDescendantFontRole::PracticalTitle => Ok(&fonts.practical_title),
        ModeDescendantFontRole::PracticalMenuLabel => Ok(&fonts.practical_menu_label),
        ModeDescendantFontRole::PracticalPrompt => Ok(&fonts.practical_prompt),
        ModeDescendantFontRole::PracticalHint => Ok(&fonts.practical_hint),
        _ => anyhow::bail!("non-practical font role reached the practical-exam glyph atlas"),
    }
}

impl ProtectedAtlasOccupancy {
    fn from_regions(retained: &[PracticalExamTextureRegion]) -> Result<Self> {
        // The source-bound descriptor and direct-packet census owns every live
        // consumer region. Pixels outside that retained union are therefore
        // available even when their original palette index is nonzero.
        let mut occupied = vec![false; ALLOCATABLE_WIDTH * TEXTURE_HEIGHT];
        for region in retained {
            fill_allocator_cell(&mut occupied, region.cell, true);
        }
        ensure!(
            occupied.iter().any(|pixel| *pixel),
            "practical-exam protected atlas occupancy is empty"
        );
        Ok(Self { occupied })
    }

    fn free_pixel_count(&self) -> usize {
        self.occupied.iter().filter(|occupied| !**occupied).count()
    }
}

struct ProtectedAtlasOccupancy {
    occupied: Vec<bool>,
}

fn fill_allocator_cell(occupied: &mut [bool], cell: Cell, value: bool) {
    if cell.x >= ALLOCATABLE_WIDTH {
        return;
    }
    let end_x = (cell.x + cell.width).min(ALLOCATABLE_WIDTH);
    let end_y = (cell.y + cell.height).min(TEXTURE_HEIGHT);
    for y in cell.y.min(TEXTURE_HEIGHT)..end_y {
        occupied[y * ALLOCATABLE_WIDTH + cell.x..y * ALLOCATABLE_WIDTH + end_x].fill(value);
    }
}

fn render_hint_composite(
    patched: &mut [u8],
    rasterizers: &mut IndexedTextRasterizers,
    catalog: &'static PracticalExamCatalogEntry,
    translation: &PracticalExamTranslation,
    fonts: &ModeDescendantFontSources,
) -> Result<RenderedHint> {
    ensure!(
        catalog.placement == PracticalExamPlacement::GuardedHintComposite
            && catalog.font_role == ModeDescendantFontRole::PracticalHint,
        "practical-exam guarded hint catalog role changed"
    );
    let source_composite =
        read_indexed_cell_in_prefix(patched, PRACTICAL_EXAM_TIM_OFFSET, HINT_COMPOSITE_CELL)?;
    ensure!(
        sha256_bytes(&source_composite) == HINT_COMPOSITE_INDEXED_SHA256,
        "practical-exam guarded hint composite source changed"
    );
    let lines = translation.korean_text.lines().collect::<Vec<_>>();
    ensure!(
        lines.len() == HINT_LABEL_CELLS.len(),
        "practical-exam guarded hint requires exactly two translated labels"
    );
    let style = font_for_role(fonts, ModeDescendantFontRole::PracticalHint)?;
    let rasterizer = rasterizers.for_font(&style.path)?;
    let mut allowed_ranges = Vec::new();
    let mut changed_decoded_byte_count = 0usize;
    let mut measured_advance_px = 0.0f32;
    let mut font_name = None;
    let mut font_sha256 = None;
    for (line, cell) in lines.into_iter().zip(HINT_LABEL_CELLS) {
        let background = reconstruct_periodic_hint_background(
            &source_composite,
            HINT_COMPOSITE_CELL,
            cell,
            HINT_BACKGROUND_PERIOD,
        )?;
        // The hint CLUT runs from white at 1 towards gray at 5. Reusing an
        // ascending coverage ramp inverted its solid strokes and edge colors.
        let raster = rasterizer.rasterize_shifted(
            line,
            cell.width,
            cell.height,
            style.font_px,
            0.0,
            style.vertical_shift_px,
            CLEAR_INDEX,
            Some(HINT_OUTLINE_INDEX),
            HINT_FILL_INDEX,
            HorizontalTextAlignment::Center,
        )?;
        let mut composed = background;
        for (output, text) in composed.iter_mut().zip(&raster.pixels) {
            if *text != CLEAR_INDEX {
                *output = *text;
            }
        }
        let write = write_indexed_cell_in_prefix_with_report(
            patched,
            PRACTICAL_EXAM_TIM_OFFSET,
            cell,
            &composed,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "practical-exam guarded hint label {line:?} changed no texture bytes"
        );
        let written = read_indexed_cell_in_prefix(patched, PRACTICAL_EXAM_TIM_OFFSET, cell)?;
        ensure!(
            written == composed,
            "practical-exam guarded hint label {line:?} failed indexed-pixel readback"
        );
        allowed_ranges.extend(write.allowed_ranges);
        changed_decoded_byte_count += write.changed_byte_count;
        measured_advance_px = measured_advance_px.max(raster.measured_advance_px);
        font_name.get_or_insert(raster.font_name);
        font_sha256.get_or_insert(raster.font_sha256);
    }
    Ok(RenderedHint {
        build: PracticalExamHintBuild {
            id: catalog.id,
            korean_text: translation.korean_text.clone(),
            cell: HINT_COMPOSITE_CELL,
            font_name: font_name.context("practical-exam guarded hint font name disappeared")?,
            font_sha256: font_sha256
                .context("practical-exam guarded hint font hash disappeared")?,
            font_px: style.font_px,
            vertical_shift_px: style.vertical_shift_px,
            measured_advance_px,
            changed_decoded_byte_count,
        },
        allowed_ranges,
    })
}

fn reconstruct_periodic_hint_background(
    source: &[u8],
    source_cell: Cell,
    target_cell: Cell,
    period: usize,
) -> Result<Vec<u8>> {
    ensure!(
        source.len() == source_cell.width * source_cell.height,
        "practical-exam guarded hint composite geometry changed"
    );
    ensure!(
        period > 0 && source_cell.width >= period && source_cell.height >= period,
        "practical-exam guarded hint background period is invalid"
    );
    ensure!(
        target_cell.x >= source_cell.x
            && target_cell.y >= source_cell.y
            && target_cell.x + target_cell.width <= source_cell.x + source_cell.width
            && target_cell.y + target_cell.height <= source_cell.y + source_cell.height,
        "practical-exam guarded hint label escaped its source composite"
    );

    let mut phase_histograms = vec![[0usize; 16]; period * period];
    for (offset, index) in source.iter().copied().enumerate() {
        let x = offset % source_cell.width;
        let y = offset / source_cell.width;
        phase_histograms[(y % period) * period + x % period][usize::from(index)] += 1;
    }
    let mut phase_indices = Vec::with_capacity(period * period);
    for histogram in phase_histograms {
        let (index, count) = histogram
            .iter()
            .copied()
            .enumerate()
            .max_by_key(|(index, count)| (*count, std::cmp::Reverse(*index)))
            .context("practical-exam guarded hint background phase disappeared")?;
        ensure!(
            count * 2 > histogram.iter().sum::<usize>(),
            "practical-exam guarded hint background phase has no majority witness"
        );
        phase_indices.push(index as u8);
    }

    let relative_x = target_cell.x - source_cell.x;
    let relative_y = target_cell.y - source_cell.y;
    let mut output = Vec::with_capacity(target_cell.width * target_cell.height);
    for y in 0..target_cell.height {
        for x in 0..target_cell.width {
            let phase_x = (relative_x + x) % period;
            let phase_y = (relative_y + y) % period;
            output.push(phase_indices[phase_y * period + phase_x]);
        }
    }
    Ok(output)
}

fn build_semantic_report(
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    consumer: PracticalExamConsumer,
    glyphs: &BTreeMap<PracticalExamGlyphKey, PracticalExamGlyphBuild>,
    hint: &PracticalExamHintBuild,
) -> Result<Vec<PracticalExamSemanticBuild>> {
    let mut semantics = Vec::with_capacity(bound.len());
    for (catalog, translation) in bound {
        let binding = binding_for_consumer(catalog, consumer)?;
        let semantic = match catalog.placement {
            PracticalExamPlacement::DynamicStrip => {
                let glyph_keys = translation
                    .korean_text
                    .chars()
                    .map(|character| (catalog.font_role, character))
                    .collect::<Vec<_>>();
                ensure!(
                    !glyph_keys.is_empty(),
                    "practical-exam semantic {} has an empty glyph stream",
                    catalog.id
                );
                let builds = glyph_keys
                    .iter()
                    .map(|key| {
                        glyphs.get(key).with_context(|| {
                            format!("practical-exam semantic {} lacks glyph {key:?}", catalog.id)
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let first = builds[0];
                ensure!(
                    builds.iter().all(|glyph| {
                        glyph.font_name == first.font_name
                            && glyph.font_sha256 == first.font_sha256
                            && glyph.font_px == first.font_px
                            && glyph.vertical_shift_px == first.vertical_shift_px
                    }),
                    "practical-exam semantic {} crosses font metadata",
                    catalog.id
                );
                let glyph_codes = builds.iter().map(|glyph| glyph.code).collect();
                PracticalExamSemanticBuild {
                    id: catalog.id,
                    descriptor_index: binding.descriptor_index,
                    korean_text: translation.korean_text.clone(),
                    font_role: catalog.font_role,
                    font_name: first.font_name.clone(),
                    font_sha256: first.font_sha256.clone(),
                    font_px: first.font_px,
                    vertical_shift_px: first.vertical_shift_px,
                    measured_advance_px: builds.iter().map(|glyph| glyph.measured_advance_px).sum(),
                    atlas_advance_px: builds
                        .iter()
                        .map(|glyph| usize::from(glyph.atlas_advance_px))
                        .sum(),
                    placement: PracticalExamSemanticPlacementBuild::GlyphStream {
                        glyph_keys,
                        glyph_codes,
                    },
                }
            }
            PracticalExamPlacement::GuardedHintComposite => {
                ensure!(
                    hint.id == catalog.id && hint.korean_text == translation.korean_text,
                    "practical-exam guarded hint report changed semantic identity"
                );
                PracticalExamSemanticBuild {
                    id: catalog.id,
                    descriptor_index: binding.descriptor_index,
                    korean_text: translation.korean_text.clone(),
                    font_role: catalog.font_role,
                    font_name: hint.font_name.clone(),
                    font_sha256: hint.font_sha256.clone(),
                    font_px: hint.font_px,
                    vertical_shift_px: hint.vertical_shift_px,
                    measured_advance_px: hint.measured_advance_px,
                    atlas_advance_px: hint.cell.width,
                    placement: PracticalExamSemanticPlacementBuild::GuardedHintComposite {
                        cell: hint.cell,
                    },
                }
            }
        };
        semantics.push(semantic);
    }
    semantics.sort_by_key(|semantic| semantic.descriptor_index);
    ensure!(
        semantics
            .windows(2)
            .all(|pair| pair[0].descriptor_index < pair[1].descriptor_index),
        "practical-exam consumer {consumer:?} semantic descriptors are not unique"
    );
    Ok(semantics)
}

fn ensure_pixel_changes_are_owned(
    source: &[u8],
    patched: &[u8],
    owned_cells: &[Cell],
) -> Result<usize> {
    ensure!(
        source.len() == TEXTURE_WIDTH * TEXTURE_HEIGHT && patched.len() == source.len(),
        "practical-exam indexed-pixel diff geometry changed"
    );
    let mut changed_count = 0usize;
    for (offset, (before, after)) in source.iter().zip(patched).enumerate() {
        if before == after {
            continue;
        }
        changed_count += 1;
        let x = offset % TEXTURE_WIDTH;
        let y = offset / TEXTURE_WIDTH;
        ensure!(
            owned_cells.iter().any(|cell| {
                (cell.x..cell.x + cell.width).contains(&x)
                    && (cell.y..cell.y + cell.height).contains(&y)
            }),
            "practical-exam atlas changed unowned indexed pixel ({x}, {y})"
        );
    }
    ensure!(
        changed_count > 0,
        "practical-exam atlas changed no indexed pixels"
    );
    Ok(changed_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires assets/"]
    fn menu_glyphs_keep_role_ink_and_complete_outlines() {
        let spec = crate::development_build_spec::load_development_build_spec(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/build/development.json"),
        )
        .unwrap();
        let mut rasterizers = IndexedTextRasterizers::default();
        for (role, outline, fill) in [
            (ModeDescendantFontRole::PracticalTitle, 13, 9),
            (ModeDescendantFontRole::PracticalMenuLabel, 2, 14),
            (ModeDescendantFontRole::PracticalPrompt, 2, 14),
        ] {
            // These include the top-bearing and low-bearing glyphs that
            // exposed clipping when outlines were first restored.
            for character in "공험학1T".chars() {
                let glyph = rasterize_nonspace_glyph(
                    &mut rasterizers,
                    (role, character),
                    &spec.fonts.mode_descendants,
                )
                .unwrap();
                assert!(glyph.pixels.contains(&outline));
                assert!(glyph.pixels.contains(&fill));
                assert!(
                    glyph
                        .pixels
                        .iter()
                        .all(|pixel| [0, outline, fill].contains(pixel))
                );
                assert_eq!(glyph.pixels.len(), glyph.width * glyph.height);
            }
        }
    }

    #[test]
    fn periodic_hint_background_reconstruction_removes_foreground_strokes() {
        let source_cell = Cell {
            x: 20,
            y: 40,
            width: 12,
            height: 8,
        };
        let target_cell = Cell {
            x: 24,
            y: 42,
            width: 6,
            height: 4,
        };
        let mut source = (0..source_cell.height)
            .flat_map(|y| {
                (0..source_cell.width).map(move |x| if (x + y).is_multiple_of(2) { 7 } else { 0 })
            })
            .collect::<Vec<_>>();
        for y in 2..6 {
            for x in 4..10 {
                source[y * source_cell.width + x] = 11;
            }
        }

        let restored =
            reconstruct_periodic_hint_background(&source, source_cell, target_cell, 2).unwrap();
        let expected = (0..target_cell.height)
            .flat_map(|y| {
                (0..target_cell.width).map(move |x| {
                    let source_x = target_cell.x - source_cell.x + x;
                    let source_y = target_cell.y - source_cell.y + y;
                    if (source_x + source_y).is_multiple_of(2) {
                        7
                    } else {
                        0
                    }
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(restored, expected);
        assert!(!restored.contains(&11));
    }

    #[test]
    fn periodic_hint_background_reconstruction_rejects_an_ambiguous_phase() {
        let cell = Cell {
            x: 0,
            y: 0,
            width: 4,
            height: 2,
        };
        let source = vec![0, 0, 7, 7, 7, 7, 0, 0];

        let error = reconstruct_periodic_hint_background(&source, cell, cell, 2).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("background phase has no majority witness")
        );
    }
}
