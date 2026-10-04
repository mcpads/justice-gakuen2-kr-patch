//! Rebuilds the complete finite SIKENKK result-title family.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::font::{
    HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers, RasterizedIndexedText,
};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::source_disc::MAIN_EXECUTABLE_PATH;
use crate::tim::{
    Cell, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::tim_preview::write_tim_preview;

use super::super::catalog::PRACTICAL_RESULT_TITLES_PATH;
use super::super::model::{
    ModeDescendantEntry, ModeDescendantFontRole, ModeDescendantFontSources,
    ModeDescendantPlacement, ModeDescendantSurface,
};
use super::super::source::ModeDescendantSourceRecord;
use super::model::{
    PracticalResultTermTitleBuildReport, PracticalResultTermTitleMemberReport,
    PracticalResultTermTitleRendererReport, PracticalResultTermTitleSelectorReport,
};

const SOURCE_CATALOG_FILE_OFFSET: usize = 0x76ee8;
const SOURCE_CATALOG_ENTRY_SIZE: usize = 12;
const SOURCE_CATALOG_INDEX: u16 = 0x02b6;
const MEMBER_SIZE: usize = 0x708;
const MINIMUM_RESULT_TITLE_FONT_PX: f32 = 11.0;
const RESULT_TITLE_CELL: Cell = Cell {
    x: 0,
    y: 0,
    width: 124,
    height: 28,
};

const MEMBER_SEMANTIC_IDS: [&str; 33] = [
    "practical_subject_movement_basics",
    "practical_subject_gravity_theory",
    "practical_subject_arm_strength_law",
    "practical_subject_kicking_theory",
    "practical_subject_basics_summary",
    "practical_subject_step_striking",
    "practical_subject_three_dimensional_axis",
    "practical_subject_applied_gravity",
    "practical_subject_throw_action",
    "practical_subject_special_actions_summary",
    "practical_subject_breakfall_principle",
    "practical_subject_four_way_rising_theorem",
    "practical_subject_love_and_friendship",
    "practical_subject_hot_blooded_combo",
    "practical_subject_attack_defense_reading",
    "practical_subject_air_burst_phenomenon",
    "practical_subject_applied_four_way_rising",
    "practical_subject_guts_counter",
    "practical_subject_complete_combustion",
    "practical_subject_growth_process",
    "practical_subject_evasive_movement",
    "practical_subject_throw_escape",
    "practical_subject_attack_cancellation",
    "practical_subject_infinite_guts_counter",
    "practical_subject_one_hit_to_victory",
    "practical_subject_review_test_1",
    "practical_subject_review_tests_2_3",
    "practical_subject_review_tests_4_5",
    "practical_subject_review_all_tests",
    "practical_subject_hayato_self_review",
    "practical_first_term_exam",
    "practical_second_term_exam",
    "practical_school_year_exam",
];

const MEMBER_DISPLAY_PREFIXES: [&str; 33] = [
    "1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "1", "2", "3", "4",
    "5", "1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "", "", "",
];

const SELECTORS: [SelectorSpec; 2] = [
    SelectorSpec {
        path: "DAT1/SIKEN.BIN",
        source_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
        runtime_base: 0x800a_2000,
        catalog_index_setup_offset: 0x4068,
        member_selector: "5 * lesson_group + lesson_variant (members 0..29)",
        selector_instructions: &[
            (
                0x4064,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V1,
                    offset: 12,
                },
            ),
            (
                0x4068,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: SOURCE_CATALOG_INDEX as i16,
                },
            ),
            (
                0x406c,
                Instruction::Sll {
                    rd: Register::A2,
                    rt: Register::V0,
                    shift: 2,
                },
            ),
            (
                0x4070,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::A2,
                    rt: Register::V0,
                },
            ),
            (
                0x407c,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: 13,
                },
            ),
            (
                0x408c,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::A2,
                    rt: Register::V1,
                },
            ),
        ],
    },
    SelectorSpec {
        path: "DAT1/SIKEN2.BIN",
        source_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
        runtime_base: 0x800a_2000,
        catalog_index_setup_offset: 0x3774,
        member_selector: "30 + exam_index (members 30..32)",
        selector_instructions: &[
            (
                0x3774,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: SOURCE_CATALOG_INDEX as i16,
                },
            ),
            (
                0x3788,
                Instruction::Lbu {
                    rt: Register::A2,
                    base: Register::V0,
                    offset: 12,
                },
            ),
            (
                0x3798,
                Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::A2,
                    immediate: 30,
                },
            ),
        ],
    },
];

const RENDERERS: [RendererSpec; 4] = [
    RendererSpec {
        path: "DAT1/SIKENG21.BIN",
        source_sha256: "aa9e1e89b934b6381516671297d8ba30f96e115e899236e719d1166741c222e8",
        runtime_base: 0x8015_2000,
        renderer_offset: 0x10b0,
        screen_y: 182,
    },
    RendererSpec {
        path: "DAT1/SIKENG22.BIN",
        source_sha256: "c793a08264d3b41916efdcb96bc4aee71ac638c873e40c25a4c010476b6ee9c9",
        runtime_base: 0x8017_a000,
        renderer_offset: 0x10b0,
        screen_y: 182,
    },
    RendererSpec {
        path: "DAT1/SIKENGO1.BIN",
        source_sha256: "105b5b89ea29827c8e010d454919cfdc11715f40c3fc50688873ed40e84b0d03",
        runtime_base: 0x8015_2000,
        renderer_offset: 0x1bbc,
        screen_y: 118,
    },
    RendererSpec {
        path: "DAT1/SIKENGO2.BIN",
        source_sha256: "931238dad4a20764eef58f376265e900b4bb1bb6d50faf2028c563884886cba6",
        runtime_base: 0x8017_a000,
        renderer_offset: 0x1bbc,
        screen_y: 118,
    },
];

#[derive(Clone, Copy)]
struct SelectorSpec {
    path: &'static str,
    source_sha256: &'static str,
    runtime_base: u32,
    catalog_index_setup_offset: usize,
    member_selector: &'static str,
    selector_instructions: &'static [(usize, Instruction)],
}

#[derive(Clone, Copy)]
struct RendererSpec {
    path: &'static str,
    source_sha256: &'static str,
    runtime_base: u32,
    renderer_offset: usize,
    screen_y: i16,
}

pub(in crate::mode_descendant_graphics) struct ResultTermTitleBuild {
    pub(in crate::mode_descendant_graphics) decoded: Vec<u8>,
    pub(in crate::mode_descendant_graphics) claims: Vec<DecodedDataClaim>,
    pub(in crate::mode_descendant_graphics) report: PracticalResultTermTitleBuildReport,
}

pub(in crate::mode_descendant_graphics) fn build_result_term_titles(
    entries: &[&ModeDescendantEntry],
    sources: &[ModeDescendantSourceRecord],
    fonts: &ModeDescendantFontSources,
    output_dir: &Path,
) -> Result<ResultTermTitleBuild> {
    let source = source_for_path(sources, PRACTICAL_RESULT_TITLES_PATH)?;
    ensure!(
        source.decoded.len() == MEMBER_SEMANTIC_IDS.len() * MEMBER_SIZE,
        "SIKENKK decoded member denominator changed"
    );
    validate_source_catalog_binding(sources, source)?;
    let producer_selectors = validate_selectors(sources)?;
    let renderer_consumers = validate_renderers(sources)?;

    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), *entry))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        entries_by_id.len() == entries.len(),
        "practical result-title inputs contain duplicate semantic ids"
    );
    let preview_dir = output_dir.join("previews");
    std::fs::create_dir_all(&preview_dir)?;
    let style = &fonts.practical_menu_label;
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&style.path)?;
    let mut patched = source.decoded.clone();
    let mut claims = Vec::new();
    let mut members = Vec::with_capacity(MEMBER_SEMANTIC_IDS.len());
    for (member_index, semantic_id) in MEMBER_SEMANTIC_IDS.into_iter().enumerate() {
        let entry = entries_by_id.get(semantic_id).with_context(|| {
            format!("SIKENKK member {member_index} lost semantic {semantic_id}")
        })?;
        ensure!(
            entry.surface == ModeDescendantSurface::PracticalExamSharedUi
                && entry.font_role == ModeDescendantFontRole::PracticalMenuLabel
                && matches!(entry.placement, ModeDescendantPlacement::Dynamic)
                && !entry.source_text.trim().is_empty()
                && !entry.korean_text.trim().is_empty(),
            "SIKENKK member {member_index} semantic role changed"
        );
        let tim_offset = member_index * MEMBER_SIZE;
        let tim = parse_embedded_tim_at(&source.decoded, tim_offset)?;
        ensure!(
            tim.offset == tim_offset
                && tim.bits_per_pixel == 4
                && tim.total_size == MEMBER_SIZE
                && tim.pixel_width == RESULT_TITLE_CELL.width
                && tim.pixel_height == RESULT_TITLE_CELL.height
                && tim.image_vram_word_x == 896
                && tim.image_vram_y == 0
                && tim.clut_vram_x == 0
                && tim.clut_vram_y == 486
                && tim.palette_count == 1,
            "SIKENKK member {member_index} TIM geometry changed"
        );
        let palette = read_4bpp_palette_words_in_prefix(&source.decoded, tim_offset, 0)?;
        let source_pixels =
            read_indexed_cell_in_prefix(&source.decoded, tim_offset, RESULT_TITLE_CELL)?;
        let rendered_text = format!(
            "{}{}",
            MEMBER_DISPLAY_PREFIXES[member_index], entry.korean_text
        );
        let (mut raster, font_px, tracking_px) = rasterize_result_title(
            rasterizer,
            &rendered_text,
            style.font_px,
            style.vertical_shift_px,
        )?;
        let palette_mapping =
            map_coverage_to_member_palette(&mut raster.pixels, &palette, &source_pixels)?;
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched,
            tim_offset,
            RESULT_TITLE_CELL,
            &raster.pixels,
        )?;
        let member_claims = DecodedDataClaim::from_effective_ranges(
            &format!("practical-result-title:{semantic_id}"),
            "replace one complete source-bound SIKENKK result-title TIM",
            &source.decoded,
            &patched,
            write.allowed_ranges,
        )?;
        ensure!(
            !member_claims.is_empty(),
            "SIKENKK member {member_index} changed no producer bytes"
        );
        let changed_decoded_byte_count = member_claims.iter().map(|claim| claim.range.len()).sum();
        claims.extend(member_claims);

        let source_preview_file = format!("previews/sikenkk-member-{member_index:03}-source.png");
        let source_preview_path = output_dir.join(&source_preview_file);
        write_tim_preview(
            &source_preview_path,
            &decode_embedded_tim_preview(&source.decoded, &tim)?,
        )?;
        let preview_file = format!("previews/sikenkk-member-{member_index:03}-result-title.png");
        let preview_path = output_dir.join(&preview_file);
        let patched_tim = parse_embedded_tim_at(&patched, tim_offset)?;
        write_tim_preview(
            &preview_path,
            &decode_embedded_tim_preview(&patched, &patched_tim)?,
        )?;
        members.push(PracticalResultTermTitleMemberReport {
            member_index,
            semantic_id: semantic_id.to_string(),
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            rendered_text,
            source_tim_sha256: tim.source_tim_sha256,
            source_indexed_sha256: sha256_bytes(&source_pixels),
            patched_indexed_sha256: sha256_bytes(&raster.pixels),
            transparent_palette_index: palette_mapping.transparent_index,
            coverage_palette_indices: palette_mapping.ink_indices,
            font_px,
            tracking_px,
            measured_advance_px: raster.measured_advance_px,
            changed_decoded_byte_count,
            source_preview_file,
            source_preview_sha256: sha256_file(&source_preview_path)?,
            preview_file,
            preview_sha256: sha256_file(&preview_path)?,
        });
    }
    ensure!(
        members.len() == MEMBER_SEMANTIC_IDS.len() && claims.len() >= members.len(),
        "SIKENKK result-title build omitted a finite member"
    );
    Ok(ResultTermTitleBuild {
        decoded: patched,
        claims,
        report: PracticalResultTermTitleBuildReport {
            source_path: PRACTICAL_RESULT_TITLES_PATH.to_string(),
            source_catalog_index: SOURCE_CATALOG_INDEX,
            archive_member_count: MEMBER_SEMANTIC_IDS.len(),
            localized_member_count: members.len(),
            producer_selector_count: producer_selectors.len(),
            renderer_consumer_count: renderer_consumers.len(),
            full_124_by_28_sprite_extent_preserved: true,
            changes_confined_to_complete_member_tim_pixels: true,
            producer_selectors,
            renderer_consumers,
            members,
        },
    })
}

struct MemberPaletteMapping {
    transparent_index: u8,
    ink_indices: Vec<u8>,
}

fn map_coverage_to_member_palette(
    pixels: &mut [u8],
    palette: &[u16; 16],
    source_pixels: &[u8],
) -> Result<MemberPaletteMapping> {
    let mut histogram = [0usize; 16];
    for &pixel in source_pixels {
        let count = histogram
            .get_mut(usize::from(pixel))
            .context("SIKENKK source pixel exceeds its 4-bpp palette")?;
        *count += 1;
    }
    let transparent_index = palette
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == 0)
        .max_by_key(|(index, _)| histogram[*index])
        .map(|(index, _)| index as u8)
        .context("SIKENKK member has no transparent palette entry")?;
    ensure!(
        histogram[usize::from(transparent_index)] > 0,
        "SIKENKK transparent palette entry is unused"
    );
    let mut ink_indices = palette
        .iter()
        .enumerate()
        .filter(|(_, word)| **word != 0)
        .map(|(index, word)| (palette_luminance(*word), index as u8))
        .collect::<Vec<_>>();
    ink_indices.sort_unstable();
    let ink_indices = ink_indices
        .into_iter()
        .map(|(_, index)| index)
        .collect::<Vec<_>>();
    ensure!(
        ink_indices.len() >= 2,
        "SIKENKK member has no usable ink coverage ramp"
    );
    for pixel in pixels {
        if *pixel == 0 {
            *pixel = transparent_index;
            continue;
        }
        let rank = usize::from(*pixel - 1) * (ink_indices.len() - 1) / 254;
        *pixel = ink_indices[rank];
    }
    Ok(MemberPaletteMapping {
        transparent_index,
        ink_indices,
    })
}

fn palette_luminance(word: u16) -> u16 {
    let red = word & 0x1f;
    let green = (word >> 5) & 0x1f;
    let blue = (word >> 10) & 0x1f;
    red * 3 + green * 6 + blue
}

fn rasterize_result_title(
    rasterizer: &IndexedTextRasterizer,
    text: &str,
    preferred_font_px: f32,
    vertical_shift_px: i32,
) -> Result<(RasterizedIndexedText, f32, f32)> {
    let mut font_px = preferred_font_px;
    let mut last_error = None;
    while font_px >= MINIMUM_RESULT_TITLE_FONT_PX {
        for tracking_px in [0.0, -0.25, -0.5, -0.75] {
            match rasterizer.rasterize_shifted_with_coverage_ramp(
                text,
                RESULT_TITLE_CELL.width,
                RESULT_TITLE_CELL.height,
                font_px,
                tracking_px,
                vertical_shift_px,
                0,
                1,
                u8::MAX,
                HorizontalTextAlignment::Left,
            ) {
                Ok(raster) => return Ok((raster, font_px, tracking_px)),
                Err(error) => last_error = Some(error),
            }
        }
        font_px -= 1.0;
    }
    let last_error = last_error.context("result-title fit range was empty")?;
    bail!(
        "result title {text:?} does not fit its 124x28 consumer at any admitted font size and \
         tracking combination from {preferred_font_px}px through \
         {MINIMUM_RESULT_TITLE_FONT_PX}px: {last_error:#}"
    )
}

fn validate_source_catalog_binding(
    sources: &[ModeDescendantSourceRecord],
    result_titles: &ModeDescendantSourceRecord,
) -> Result<()> {
    let executable = source_for_path(sources, MAIN_EXECUTABLE_PATH)?;
    let offset =
        SOURCE_CATALOG_FILE_OFFSET + usize::from(SOURCE_CATALOG_INDEX) * SOURCE_CATALOG_ENTRY_SIZE;
    let entry = executable
        .decoded
        .get(offset..offset + SOURCE_CATALOG_ENTRY_SIZE)
        .context("SIKENKK source catalog entry is truncated")?;
    let mut expected = [0u8; SOURCE_CATALOG_ENTRY_SIZE];
    expected[..8].copy_from_slice(&catalog_key(
        result_titles.extent_lba,
        u32::try_from(result_titles.stored.len())?,
    )?);
    expected[8..].copy_from_slice(
        result_titles
            .stored
            .get(..4)
            .context("SIKENKK source record has no first word")?,
    );
    ensure!(
        entry == expected,
        "source catalog index 0x02b6 no longer identifies DAT2/SIKENKK.BIZ"
    );
    Ok(())
}

fn validate_selectors(
    sources: &[ModeDescendantSourceRecord],
) -> Result<Vec<PracticalResultTermTitleSelectorReport>> {
    SELECTORS
        .iter()
        .map(|spec| {
            let consumer = source_for_path(sources, spec.path)?;
            ensure!(
                sha256_bytes(&consumer.decoded) == spec.source_sha256,
                "{} result-title selector identity changed",
                spec.path
            );
            for (offset, instruction) in spec.selector_instructions {
                ensure_instruction(
                    &consumer.decoded,
                    spec.runtime_base,
                    *offset,
                    instruction,
                    "result-title selector",
                )?;
            }
            Ok(PracticalResultTermTitleSelectorReport {
                path: spec.path.to_string(),
                source_sha256: spec.source_sha256.to_string(),
                catalog_index_setup_offset: format!("0x{:04x}", spec.catalog_index_setup_offset),
                member_selector: spec.member_selector.to_string(),
            })
        })
        .collect()
}

fn validate_renderers(
    sources: &[ModeDescendantSourceRecord],
) -> Result<Vec<PracticalResultTermTitleRendererReport>> {
    RENDERERS
        .iter()
        .map(|spec| {
            let consumer = source_for_path(sources, spec.path)?;
            ensure!(
                sha256_bytes(&consumer.decoded) == spec.source_sha256,
                "{} result-title renderer identity changed",
                spec.path
            );
            for (relative_offset, instruction) in [
                (
                    0x14,
                    Instruction::Addiu {
                        rt: Register::A2,
                        rs: Register::ZERO,
                        immediate: 0x0380,
                    },
                ),
                (
                    0xcc,
                    Instruction::Addiu {
                        rt: Register::A1,
                        rs: Register::ZERO,
                        immediate: 0x01e6,
                    },
                ),
                (
                    0xe4,
                    Instruction::Addiu {
                        rt: Register::V0,
                        rs: Register::ZERO,
                        immediate: RESULT_TITLE_CELL.width as i16,
                    },
                ),
                (
                    0xec,
                    Instruction::Addiu {
                        rt: Register::V0,
                        rs: Register::ZERO,
                        immediate: RESULT_TITLE_CELL.height as i16,
                    },
                ),
                (
                    0xf4,
                    Instruction::Addiu {
                        rt: Register::V0,
                        rs: Register::ZERO,
                        immediate: 75,
                    },
                ),
                (
                    0xfc,
                    Instruction::Addiu {
                        rt: Register::V0,
                        rs: Register::ZERO,
                        immediate: spec.screen_y,
                    },
                ),
            ] {
                ensure_instruction(
                    &consumer.decoded,
                    spec.runtime_base,
                    spec.renderer_offset + relative_offset,
                    &instruction,
                    "result-title renderer",
                )?;
            }
            Ok(PracticalResultTermTitleRendererReport {
                path: spec.path.to_string(),
                source_sha256: spec.source_sha256.to_string(),
                runtime_base: format!("0x{:08x}", spec.runtime_base),
                renderer_offset: format!("0x{:04x}", spec.renderer_offset),
                screen_x: 75,
                screen_y: spec.screen_y,
                sprite_width: RESULT_TITLE_CELL.width as i16,
                sprite_height: RESULT_TITLE_CELL.height as i16,
            })
        })
        .collect()
}

fn source_for_path<'a>(
    sources: &'a [ModeDescendantSourceRecord],
    path: &str,
) -> Result<&'a ModeDescendantSourceRecord> {
    sources
        .iter()
        .find(|source| source.path == path)
        .with_context(|| format!("result-title source {path} was not loaded"))
}

fn ensure_instruction(
    consumer: &[u8],
    runtime_base: u32,
    offset: usize,
    expected: &Instruction,
    role: &str,
) -> Result<()> {
    let bytes = consumer
        .get(offset..offset + 4)
        .with_context(|| format!("{role} instruction span is truncated"))?;
    let actual = decode(
        u32::from_le_bytes(bytes.try_into()?),
        runtime_base + u32::try_from(offset)?,
    )?;
    ensure!(
        actual == *expected,
        "{role} instruction +0x{offset:04x} changed"
    );
    Ok(())
}

fn catalog_key(extent_lba: u32, byte_count: u32) -> Result<[u8; 8]> {
    let absolute_sector = extent_lba
        .checked_add(150)
        .context("SIKENKK source catalog sector overflow")?;
    let minute = absolute_sector / (75 * 60);
    let remainder = absolute_sector % (75 * 60);
    let second = remainder / 75;
    let frame = remainder % 75;
    let mut key = [0u8; 8];
    key[..4].copy_from_slice(&[bcd(minute)?, bcd(second)?, bcd(frame)?, 0]);
    key[4..].copy_from_slice(&byte_count.to_le_bytes());
    Ok(key)
}

fn bcd(value: u32) -> Result<u8> {
    ensure!(value <= 99, "SIKENKK source catalog BCD value is too large");
    Ok(u8::try_from(((value / 10) << 4) | (value % 10))?)
}
