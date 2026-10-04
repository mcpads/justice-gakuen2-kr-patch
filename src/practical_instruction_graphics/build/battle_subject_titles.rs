use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Deserialize;

use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::embedded_tim::detect_embedded_tim_images;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::read_4bpp_palette_words_in_prefix;
use crate::tim_preview::write_tim_preview;
use crate::tzz::TzzMember;

use super::{
    DynamicOverlayFamilyBuild, GameplayPromptFamilyBuild, IndexedPresentation,
    PracticalBattleSubjectTitleBuildReport, PracticalBattleSubjectTitleConsumerBuild,
    PracticalBattleSubjectTitleMemberReport, PracticalInstructionGraphicsBuildConfig,
    PracticalInstructionMemberBinding, TILE_HEIGHT, TILE_WIDTH, TranslationState, indexed_to_rgba,
    resolved_presentation_text_palette_indices, source_presentation_palette_roles,
    write_dynamic_overlay_tiles,
};

const SUBJECT_TITLE_CATALOG_KIND: &str = "justice_gakuen2_mode_descendant_unit";
const SUBJECT_TITLE_CATALOG_RELATIVE_PATH: &str = "mode-descendants/practical/basics-subjects.json";
const CONSUMER_PATH: &str = "DAT1/SIKEN.BIN";
const CONSUMER_RUNTIME_BASE: u32 = 0x800a_2000;
const CONSUMER_SOURCE_SHA256: &str =
    "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9";
const CONSUMER_FUNCTION_OFFSET: usize = 0x5768;
const SOURCE_WIDTH_TABLE_OFFSET: usize = 0x0cc0;
const SOURCE_WIDTH_TABLE_RUNTIME_OFFSET: i16 = 0x2cc0;
const SOURCE_WIDTH_TABLE_SHA256: &str =
    "e3762d950ba3ca264a12eac7ad5819e59014c2ca8d56bc538d642aef1e0ebcda";
const BATTLE_SUBJECT_MEMBER_COUNT: usize = 30;
const TITLE_MEMBER_COUNT: usize = 33;
const OFFSCREEN_X: i16 = 512;
const TITLE_STRIP_SCREEN_X: i16 = 30;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectTitleCatalog {
    kind: String,
    entries: Vec<SubjectTitleTranslation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectTitleTranslation {
    id: String,
    surface: String,
    source_text: String,
    korean_text: String,
    font_role: String,
    placement: SubjectTitlePlacement,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectTitlePlacement {
    kind: String,
}

pub(super) struct BattleSubjectTitleMemberPatch {
    pub(super) candidate: Vec<u8>,
    pub(super) claims: Vec<DecodedDataClaim>,
    pub(super) owned_tiles: BTreeSet<usize>,
}

pub(super) struct BattleSubjectTitleFamilyBuild {
    pub(super) member_patches: BTreeMap<usize, BattleSubjectTitleMemberPatch>,
    pub(super) consumer: PracticalBattleSubjectTitleConsumerBuild,
    pub(super) report: PracticalBattleSubjectTitleBuildReport,
}

pub(super) fn build_battle_subject_title_family(
    config: &PracticalInstructionGraphicsBuildConfig,
    source: &SupportedSourceDisc,
    bindings: &[PracticalInstructionMemberBinding],
    archive: &[u8],
    source_members: &[TzzMember],
) -> Result<BattleSubjectTitleFamilyBuild> {
    ensure!(
        bindings.len() == TITLE_MEMBER_COUNT
            && source_members.len() == bindings.len()
            && bindings
                .iter()
                .take(TITLE_MEMBER_COUNT)
                .all(|binding| binding.translation_state == TranslationState::Authored),
        "battle subject-title producer denominator changed"
    );
    let catalog_path = subject_title_catalog_path(config)?;
    let catalog_bytes = std::fs::read(&catalog_path)
        .with_context(|| format!("failed to read {}", catalog_path.display()))?;
    let mut catalog: SubjectTitleCatalog = serde_json::from_slice(&catalog_bytes)
        .with_context(|| format!("failed to parse {}", catalog_path.display()))?;
    ensure!(
        catalog.kind == SUBJECT_TITLE_CATALOG_KIND,
        "battle subject-title translation catalog kind changed"
    );
    let term_catalog_path = catalog_path.with_file_name("exam-term-menu.json");
    let term_catalog: SubjectTitleCatalog =
        serde_json::from_slice(&std::fs::read(&term_catalog_path)?)?;
    ensure!(
        term_catalog.kind == SUBJECT_TITLE_CATALOG_KIND,
        "term-title catalog kind changed"
    );
    catalog
        .entries
        .extend(term_catalog.entries.into_iter().filter(|entry| {
            [
                "practical_first_term_exam",
                "practical_second_term_exam",
                "practical_school_year_exam",
            ]
            .contains(&entry.id.as_str())
        }));
    let (_, term_consumer) = source.read_record("DAT1/SIKEN2.BIN")?;
    ensure!(
        sha256_bytes(&term_consumer)
            == "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
        "term-title consumer changed"
    );
    let term_widths = &term_consumer[0x0a40..0x0a43];
    let translations = catalog
        .entries
        .iter()
        .map(|entry| {
            ensure!(
                !entry.id.is_empty()
                    && entry.surface == "practical_exam_shared_ui"
                    && entry.font_role == "practical_menu_label"
                    && entry.placement.kind == "dynamic"
                    && !entry.source_text.trim().is_empty()
                    && !entry.korean_text.trim().is_empty(),
                "battle subject-title translation {} changed role or text shape",
                entry.id
            );
            Ok((entry.id.as_str(), entry))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(
        translations.len() == TITLE_MEMBER_COUNT,
        "battle subject-title translation denominator changed"
    );

    let (_, source_consumer) = source.read_record(CONSUMER_PATH)?;
    ensure!(
        sha256_bytes(&source_consumer) == CONSUMER_SOURCE_SHA256,
        "battle subject-title consumer source identity changed"
    );
    let source_widths = audit_subject_title_consumer(&source_consumer)?;
    let mut packet_widths = Vec::with_capacity(BATTLE_SUBJECT_MEMBER_COUNT);
    let mut member_patches = BTreeMap::new();
    let mut member_reports = Vec::with_capacity(BATTLE_SUBJECT_MEMBER_COUNT);
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&config.body_font.path)?;

    for (member_index, binding) in bindings.iter().take(TITLE_MEMBER_COUNT).enumerate() {
        ensure!(
            binding.member_index == member_index,
            "battle subject-title member ordering changed"
        );
        let translation = translations
            .get(binding.title_semantic_id.as_str())
            .with_context(|| {
                format!(
                    "battle subject-title translation {} disappeared",
                    binding.title_semantic_id
                )
            })?;
        let layout = binding
            .presentation_layout
            .as_ref()
            .context("battle subject-title member has no instruction-panel ownership boundary")?;
        let allocated_strip_width = title_strip_capacity(layout)?;
        let source_width = if member_index < BATTLE_SUBJECT_MEMBER_COUNT {
            source_widths[member_index]
        } else {
            term_widths[member_index - BATTLE_SUBJECT_MEMBER_COUNT]
        };
        ensure!(
            usize::from(source_width) <= allocated_strip_width,
            "battle subject-title source width escaped member {member_index}'s title strip"
        );

        let member = source_members
            .get(member_index)
            .context("battle subject-title source member disappeared")?;
        let source_decoded = decompress(
            archive
                .get(member.compressed_range())
                .context("battle subject-title source member extent disappeared")?,
            true,
        )?;
        let tim = detect_embedded_tim_images(&source_decoded)
            .into_iter()
            .next()
            .context("battle subject-title source member has no TIM")?;
        ensure!(
            tim.offset == 0
                && tim.bits_per_pixel == 4
                && tim.pixel_width == 256
                && tim.pixel_height == 256
                && tim.image_vram_word_x == 896
                && tim.image_vram_y == 256
                && tim.clut_vram_y == 503,
            "battle subject-title source member no longer matches its runtime texture"
        );

        let subject_number = member_index % 5 + 1;
        let korean_text = if member_index < BATTLE_SUBJECT_MEMBER_COUNT {
            format!("과목 {subject_number}: {}", translation.korean_text)
        } else {
            translation.korean_text.clone()
        };
        let source_text = if member_index < BATTLE_SUBJECT_MEMBER_COUNT {
            format!("科目{subject_number}：{}", translation.source_text)
        } else {
            translation.source_text.clone()
        };
        let palette = read_4bpp_palette_words_in_prefix(&source_decoded, 0, 0)?;
        if member_index >= BATTLE_SUBJECT_MEMBER_COUNT {
            // The native term prefix and decimal glyphs use this same CLUT.
            // Each loader has its own gray midtones; coverage order and
            // transparent/solid endpoints agree with the shared HUD raster.
            ensure!(
                palette[0] == 0
                    && palette[15] & 0x7fff == 0x7fff
                    && palette[1..].iter().all(|word| {
                        let gray = word & 31;
                        *word != 0 && (word >> 5) & 31 == gray && (word >> 10) & 31 == gray
                    })
                    && palette[1..]
                        .windows(2)
                        .all(|pair| (pair[0] & 31) <= (pair[1] & 31)),
                "term gameplay HUD palette changed for member {member_index}"
            );
        }
        let palette_roles = source_presentation_palette_roles(&source_decoded)?;
        let transparent_index = palette_roles.background_index;
        let text_palette_indices =
            resolved_presentation_text_palette_indices(layout, &palette_roles)?;
        let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
            &korean_text,
            allocated_strip_width,
            TILE_HEIGHT,
            config.body_font.font_px,
            0.0,
            config.body_font.vertical_shift_px,
            0,
            1,
            u8::try_from(text_palette_indices.len())?,
            HorizontalTextAlignment::Left,
        )?;
        ensure!(
            raster.measured_advance_px <= allocated_strip_width as f32,
            "battle subject-title member {member_index} does not fit its owned strip"
        );
        let packet_width = usize::max(raster.measured_advance_px.ceil() as usize, 1);
        let packet_width = u8::try_from(packet_width).with_context(|| {
            format!("battle subject-title member {member_index} exceeds one SPRT width byte")
        })?;
        let pixels = raster
            .pixels
            .into_iter()
            .map(|pixel| {
                if pixel == 0 {
                    Ok(transparent_index)
                } else {
                    text_palette_indices
                        .get(usize::from(pixel - 1))
                        .copied()
                        .context("battle subject-title coverage left its text palette")
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let owned_tiles = (0..allocated_strip_width / TILE_WIDTH).collect::<BTreeSet<_>>();
        let mut candidate = source_decoded.clone();
        let mut allowed_ranges = Vec::new();
        write_dynamic_overlay_tiles(
            &mut candidate,
            &owned_tiles.iter().copied().collect::<Vec<_>>(),
            &pixels,
            &mut allowed_ranges,
        )?;
        let claims = DecodedDataClaim::from_effective_ranges(
            &format!("battle-subject-title:{}", binding.title_semantic_id),
            "replace only the active member's source-bound battle subject-title strip",
            &source_decoded,
            &candidate,
            allowed_ranges,
        )?;
        ensure!(
            !claims.is_empty(),
            "battle subject-title member {member_index} changed no producer bytes"
        );
        let changed_decoded_byte_count = claims.iter().map(|claim| claim.range.len()).sum();
        let preview_file = format!("previews/member-{member_index:03}-battle-subject-title.png");
        let preview_path = config.output_dir.join(&preview_file);
        let preview = IndexedPresentation {
            width: allocated_strip_width,
            height: TILE_HEIGHT,
            pixels,
        };
        write_tim_preview(&preview_path, &indexed_to_rgba(&preview, &palette))?;
        let preview_sha256 = sha256_file(&preview_path)?;

        if member_index < BATTLE_SUBJECT_MEMBER_COUNT {
            packet_widths.push(packet_width);
        } else {
            ensure!(
                packet_width <= source_width,
                "term-title exceeds native sprite width"
            );
        }
        member_reports.push(PracticalBattleSubjectTitleMemberReport {
            member_index,
            title_semantic_id: binding.title_semantic_id.clone(),
            source_text,
            korean_text,
            source_packet_width: source_width,
            patched_packet_width: if member_index < BATTLE_SUBJECT_MEMBER_COUNT {
                packet_width
            } else {
                source_width
            },
            allocated_strip_width,
            owned_title_tiles: owned_tiles.iter().copied().collect(),
            changed_decoded_byte_count,
            preview_file,
            preview_sha256,
        });
        ensure!(
            member_patches
                .insert(
                    member_index,
                    BattleSubjectTitleMemberPatch {
                        candidate,
                        claims,
                        owned_tiles,
                    },
                )
                .is_none(),
            "battle subject-title producer member was duplicated"
        );
    }

    let patched_consumer = patch_subject_title_consumer(&source_consumer, &packet_widths)?;
    let consumer = PracticalBattleSubjectTitleConsumerBuild {
        path: CONSUMER_PATH.to_string(),
        source_record: source_consumer.clone(),
        source_record_size: source_consumer.len(),
        source_record_sha256: sha256_bytes(&source_consumer),
        patched_record_sha256: sha256_bytes(&patched_consumer),
        record: patched_consumer,
    };
    let report = PracticalBattleSubjectTitleBuildReport {
        term_translation_catalog_path: term_catalog_path.display().to_string(),
        term_translation_catalog_sha256: sha256_bytes(&std::fs::read(&term_catalog_path)?),
        native_term_title_member_count: TITLE_MEMBER_COUNT - BATTLE_SUBJECT_MEMBER_COUNT,
        translation_catalog_path: catalog_path.display().to_string(),
        translation_catalog_sha256: sha256_bytes(&catalog_bytes),
        consumer_path: CONSUMER_PATH.to_string(),
        consumer_runtime_base: format!("0x{CONSUMER_RUNTIME_BASE:08x}"),
        consumer_function_offset: format!("0x{CONSUMER_FUNCTION_OFFSET:04x}"),
        source_consumer_sha256: consumer.source_record_sha256.clone(),
        patched_consumer_sha256: consumer.patched_record_sha256.clone(),
        member_count: TITLE_MEMBER_COUNT,
        localized_member_count: member_reports.len(),
        old_prefix_packets_moved_offscreen: true,
        title_strip_screen_x: TITLE_STRIP_SCREEN_X,
        all_changes_confined_to_title_strips_and_consumer_bindings: true,
        members: member_reports,
    };
    Ok(BattleSubjectTitleFamilyBuild {
        member_patches,
        consumer,
        report,
    })
}

pub(super) fn validate_battle_subject_title_ownership(
    titles: &BattleSubjectTitleFamilyBuild,
    prompts: &GameplayPromptFamilyBuild,
    overlays: &DynamicOverlayFamilyBuild,
    bindings: &[PracticalInstructionMemberBinding],
) -> Result<()> {
    ensure!(
        titles.member_patches.len() == TITLE_MEMBER_COUNT,
        "battle subject-title owned-member denominator changed"
    );
    for (&member_index, title) in &titles.member_patches {
        let panel_tiles = bindings
            .get(member_index)
            .and_then(|binding| binding.presentation_layout.as_ref())
            .context("battle subject-title member lost its panel layout")?
            .source_tile_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        ensure!(
            title.owned_tiles.is_disjoint(&panel_tiles)
                && prompts
                    .member_patches
                    .get(&member_index)
                    .is_none_or(|prompt| title.owned_tiles.is_disjoint(&prompt.owned_tile_indices))
                && overlays
                    .member_patches
                    .get(&member_index)
                    .is_none_or(|overlay| title.owned_tiles.is_disjoint(&overlay.owned_tiles)),
            "practical member {member_index} reuses a battle subject-title tile"
        );
    }
    Ok(())
}

fn subject_title_catalog_path(config: &PracticalInstructionGraphicsBuildConfig) -> Result<PathBuf> {
    let menu_assets = config
        .assets
        .parent()
        .context("practical-instruction assets have no menu-assets parent")?;
    Ok(menu_assets.join(SUBJECT_TITLE_CATALOG_RELATIVE_PATH))
}

fn title_strip_capacity(layout: &super::PresentationLayout) -> Result<usize> {
    let first_panel_tile = layout
        .source_tile_indices
        .iter()
        .copied()
        .min()
        .context("practical instruction panel has no source tile")?;
    let width = usize::min(first_panel_tile * TILE_WIDTH, 256);
    ensure!(
        width > 0 && width.is_multiple_of(TILE_WIDTH),
        "battle subject-title has no tile-aligned capacity before its panel"
    );
    Ok(width)
}

fn audit_subject_title_consumer(consumer: &[u8]) -> Result<Vec<u8>> {
    let widths = consumer
        .get(SOURCE_WIDTH_TABLE_OFFSET..SOURCE_WIDTH_TABLE_OFFSET + BATTLE_SUBJECT_MEMBER_COUNT)
        .context("battle subject-title width table is truncated")?;
    ensure!(
        sha256_bytes(widths) == SOURCE_WIDTH_TABLE_SHA256,
        "battle subject-title source width table changed: {} {widths:02x?}",
        sha256_bytes(widths),
    );
    let expected = [
        (
            0x5b3c,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::A0,
                offset: 0x000c,
            },
        ),
        (
            0x5b40,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::A0,
                offset: 0x000d,
            },
        ),
        (
            0x5b44,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x5b48,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x5b4c,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x5b50,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x5b54,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x5b58,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::AT,
                offset: SOURCE_WIDTH_TABLE_RUNTIME_OFFSET,
            },
        ),
        (
            0x58c0,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 30,
            },
        ),
        (
            0x5a20,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 70,
            },
        ),
        (
            0x5b64,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 110,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(consumer, offset, instruction)?;
    }
    Ok(widths.to_vec())
}

fn patch_subject_title_consumer(source: &[u8], packet_widths: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        packet_widths.len() == BATTLE_SUBJECT_MEMBER_COUNT
            && packet_widths.iter().all(|&width| width > 0),
        "battle subject-title output width table changed shape"
    );
    let mut candidate = source.to_vec();
    replace_instruction(
        source,
        &mut candidate,
        0x58c0,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: 30,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: OFFSCREEN_X,
        },
    )?;
    replace_instruction(
        source,
        &mut candidate,
        0x5a20,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 70,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: OFFSCREEN_X,
        },
    )?;
    replace_instruction(
        source,
        &mut candidate,
        0x5b64,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 110,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: TITLE_STRIP_SCREEN_X,
        },
    )?;
    candidate[SOURCE_WIDTH_TABLE_OFFSET..SOURCE_WIDTH_TABLE_OFFSET + BATTLE_SUBJECT_MEMBER_COUNT]
        .copy_from_slice(packet_widths);
    ensure!(
        sha256_bytes(&candidate) != sha256_bytes(source),
        "battle subject-title consumer patch changed no bytes"
    );
    Ok(candidate)
}

fn ensure_instruction(consumer: &[u8], offset: usize, expected: Instruction) -> Result<()> {
    let bytes = consumer
        .get(offset..offset + 4)
        .context("battle subject-title consumer instruction span is truncated")?;
    let actual = decode(
        u32::from_le_bytes(bytes.try_into()?),
        CONSUMER_RUNTIME_BASE + u32::try_from(offset)?,
    )?;
    ensure!(
        actual == expected,
        "battle subject-title consumer instruction +0x{offset:04x} changed"
    );
    Ok(())
}

fn replace_instruction(
    source: &[u8],
    candidate: &mut [u8],
    offset: usize,
    expected: Instruction,
    replacement: Instruction,
) -> Result<()> {
    ensure_instruction(source, offset, expected)?;
    let pc = CONSUMER_RUNTIME_BASE + u32::try_from(offset)?;
    let encoded = encode(&replacement, pc)?;
    candidate[offset..offset + 4].copy_from_slice(&encoded.to_le_bytes());
    ensure!(
        decode(encoded, pc)? == replacement,
        "battle subject-title replacement +0x{offset:04x} failed typed roundtrip"
    );
    Ok(())
}
