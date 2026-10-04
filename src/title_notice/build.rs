use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::contextual_texture_upload::{ContextualMenuGlyph, pack_4bpp_pixels};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::menu_atlas_plan::MenuGlyphAllocation;
use crate::pipeline::{
    EMBEDDED_MOJI2_TIM_SIZE, difference_ranges, sha256_bytes, write_pretty_json_and_hash,
};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::install_indexed_glyph;
use crate::write_scope::changed_ranges_are_within;

use super::assets::{load_assets, parse_hex_usize};
use super::model::{
    ReleaseStatus, TitleNoticeBuild, TitleNoticeBuildConfig, TitleNoticeBuildReport,
    TitleNoticeEntryBuild, TitleNoticeGlyphBuild,
};
use super::source::load_title_notice_source_from_disc;

pub(crate) const BUILD_MANIFEST_FILE: &str = "title-notice-build.json";
const OVERLAY_DECODED_OUTPUT_FILE: &str = "title-notice-mgtit.bin";
const MENU_DECODED_OUTPUT_FILE: &str = "title-notice-menu.bin";
const TRANSPARENT_ADVANCE_CODE: u16 = 0x0fff;
const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;

pub(crate) fn build_title_notice_from_source(
    config: &TitleNoticeBuildConfig,
    source_disc: &SupportedSourceDisc,
    plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<TitleNoticeBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    ensure!(
        config.font.font_px.is_finite() && config.font.font_px > 0.0,
        "title notice font size must be finite and positive"
    );
    let source = load_title_notice_source_from_disc(source_disc)?;
    let assets = load_assets(&config.assets, &source)?;
    let allocation = assets
        .entries
        .iter()
        .flat_map(|entry| entry.korean_text.chars())
        .filter(|c| *c != ' ')
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|character| {
            let glyph = plan.glyph(&format!("title_notice:{:04x}", character as u32))?;
            Ok((
                character,
                MenuGlyphAllocation {
                    character,
                    code: glyph.code,
                    cell: glyph.cell,
                    reused: false,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    let mut rendered_menu_decoded = source.menu_decoded.clone();
    let (font_name, font_sha256, glyphs, contextual_glyphs) = render_contextual_glyphs(
        &mut rendered_menu_decoded[..EMBEDDED_MOJI2_TIM_SIZE],
        &allocation,
        &config.font,
    )?;
    let rendered_menu_decoded_changed_byte_ranges =
        difference_ranges(&source.menu_decoded, &rendered_menu_decoded);
    let allowed_menu_ranges = glyphs
        .iter()
        .filter_map(|glyph| glyph.install.as_ref())
        .flat_map(|install| install.allowed_decoded_byte_ranges.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        !rendered_menu_decoded_changed_byte_ranges.is_empty()
            && changed_ranges_are_within(
                &rendered_menu_decoded_changed_byte_ranges,
                &allowed_menu_ranges,
            ),
        "title notice glyph render escaped its contextual MENU cells"
    );
    let menu_decoded = source.menu_decoded.clone();
    let menu_decoded_changed_byte_ranges = difference_ranges(&source.menu_decoded, &menu_decoded);
    ensure!(
        menu_decoded_changed_byte_ranges.is_empty(),
        "title notice build retained a global MENU.BIZ write"
    );
    let menu_write_claims = Vec::new();

    let mut overlay_decoded = source.overlay_decoded.clone();
    let entries = rebuild_text_records(&mut overlay_decoded, &assets.entries, &allocation)?;
    let overlay_decoded_changed_byte_ranges =
        difference_ranges(&source.overlay_decoded, &overlay_decoded);
    let mut allowed_overlay_ranges = Vec::with_capacity(assets.entries.len());
    let mut overlay_write_claims = Vec::with_capacity(assets.entries.len());
    for entry in &assets.entries {
        let start = parse_hex_usize(&entry.source_offset, "source offset")?;
        let range = [start, start + entry.source_record_size];
        allowed_overlay_ranges.push(range);
        overlay_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("mgtit:title-notice:record:{}", entry.id),
            &format!("replace title notice text record {}", entry.id),
            [range],
        ));
    }
    ensure!(
        !overlay_decoded_changed_byte_ranges.is_empty()
            && changed_ranges_are_within(
                &overlay_decoded_changed_byte_ranges,
                &allowed_overlay_ranges,
            ),
        "title notice rebuild escaped its source-owned MGTIT records"
    );
    overlay_write_claims.retain(|claim| {
        source.overlay_decoded[claim.range.clone()] != overlay_decoded[claim.range.clone()]
    });

    let release_approved_entry_count = assets
        .entries
        .iter()
        .filter(|entry| entry.release_status == ReleaseStatus::Approved)
        .count();
    let report = TitleNoticeBuildReport {
        kind: "Justice Gakuen 2 source-bound title notice development build".to_string(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_bin_sha256: source.source_bin_sha256,
        source_menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        output_menu_decoded_sha256: sha256_bytes(&menu_decoded),
        source_overlay_decoded_sha256: sha256_bytes(&source.overlay_decoded),
        output_overlay_decoded_sha256: sha256_bytes(&overlay_decoded),
        translation_manifest_sha256: assets.manifest_sha256,
        font_name,
        font_sha256,
        font_px: config.font.font_px,
        entry_count: assets.entries.len(),
        release_approved_entry_count,
        development_input_available: true,
        release_candidate_input_eligible: release_approved_entry_count == assets.entries.len(),
        source_records_match: true,
        source_placements_match: true,
        changed_bytes_confined_to_owned_ranges: true,
        reused_glyph_count: glyphs.iter().filter(|glyph| glyph.reused).count(),
        installed_glyph_count: glyphs.iter().filter(|glyph| !glyph.reused).count(),
        global_menu_write_count: 0,
        source_menu_graphics_preserved: true,
        menu_decoded_changed_byte_ranges,
        overlay_decoded_changed_byte_ranges,
        glyphs,
        entries,
        runtime_consumer_verified: false,
    };
    let build_manifest_sha256 =
        write_outputs(&config.output_dir, &overlay_decoded, &menu_decoded, &report)?;
    Ok(TitleNoticeBuild {
        source_overlay_decoded: source.overlay_decoded,
        overlay_decoded,
        menu_decoded,
        menu_write_claims,
        overlay_write_claims,
        contextual_glyphs,
        build_manifest_sha256,
        report,
    })
}

fn render_contextual_glyphs(
    menu_font_tim: &mut [u8],
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    style: &super::model::TitleNoticeFontStyle,
) -> Result<(
    String,
    String,
    Vec<TitleNoticeGlyphBuild>,
    Vec<ContextualMenuGlyph>,
)> {
    let rasterizer = IndexedTextRasterizer::load(&style.font)?;
    let mut font_identity = None;
    let mut glyphs = Vec::with_capacity(allocation.len());
    let mut contextual_glyphs = Vec::with_capacity(allocation.len());
    for allocation in allocation.values() {
        if allocation.reused {
            glyphs.push(TitleNoticeGlyphBuild {
                character: allocation.character,
                code: format!("0x{:04x}", allocation.code),
                cell: allocation.cell,
                reused: true,
                global_menu_resident: true,
                runtime_context: "mgtit".to_string(),
                ink_bounds: None,
                install: None,
            });
            continue;
        }
        let rendered = rasterizer.rasterize(
            &allocation.character.to_string(),
            allocation.cell.width,
            allocation.cell.height,
            style.font_px,
            0.0,
            CLEAR_INDEX,
            Some(OUTLINE_INDEX),
            FILL_INDEX,
            HorizontalTextAlignment::Center,
        )?;
        match &font_identity {
            None => {
                font_identity = Some((rendered.font_name.clone(), rendered.font_sha256.clone()))
            }
            Some(identity) => ensure!(
                identity == &(rendered.font_name.clone(), rendered.font_sha256.clone()),
                "title notice font identity changed within one build"
            ),
        }
        let install = install_indexed_glyph(
            menu_font_tim,
            allocation.cell,
            &rendered.pixels,
            &format!("title notice glyph {:?}", allocation.character),
        )?;
        let payload = pack_4bpp_pixels(&rendered.pixels)?;
        glyphs.push(TitleNoticeGlyphBuild {
            character: allocation.character,
            code: format!("0x{:04x}", allocation.code),
            cell: allocation.cell,
            reused: false,
            global_menu_resident: false,
            runtime_context: "mgtit".to_string(),
            ink_bounds: Some(rendered.ink_bounds),
            install: Some(install),
        });
        contextual_glyphs.push(ContextualMenuGlyph {
            role: "title_notice".to_string(),
            character: allocation.character,
            code: allocation.code,
            cell: allocation.cell,
            payload,
        });
    }
    let (font_name, font_sha256) = font_identity.context("title notice installed no new glyphs")?;
    Ok((font_name, font_sha256, glyphs, contextual_glyphs))
}

fn rebuild_text_records(
    overlay: &mut [u8],
    entries: &[super::model::TitleNoticeTranslation],
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
) -> Result<Vec<TitleNoticeEntryBuild>> {
    let mut builds = Vec::with_capacity(entries.len());
    for entry in entries {
        let output_codes = entry
            .korean_text
            .chars()
            .map(|character| {
                if character == ' ' {
                    Ok(TRANSPARENT_ADVANCE_CODE)
                } else {
                    allocation
                        .get(&character)
                        .map(|glyph| glyph.code)
                        .with_context(|| format!("no title notice glyph for {character:?}"))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            2 + output_codes.len() * 2 <= entry.source_record_size,
            "title notice {} Korean text exceeds its {}-byte source record",
            entry.id,
            entry.source_record_size
        );
        let offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        let record = overlay
            .get_mut(offset..offset + entry.source_record_size)
            .context("title notice output record is out of bounds")?;
        write_record(record, &output_codes)?;
        builds.push(TitleNoticeEntryBuild {
            id: entry.id.clone(),
            source_offset: entry.source_offset.clone(),
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            output_codes: output_codes
                .iter()
                .map(|code| format!("0x{code:04x}"))
                .collect(),
            consumer: entry.consumer.clone(),
            development_status: entry.development_status,
            release_status: entry.release_status,
        });
    }
    Ok(builds)
}

fn write_record(record: &mut [u8], codes: &[u16]) -> Result<()> {
    ensure!(
        2 + codes.len() * 2 <= record.len(),
        "encoded title notice exceeds its source record"
    );
    record.fill(0);
    record[..2].copy_from_slice(&u16::try_from(codes.len())?.to_le_bytes());
    for (index, code) in codes.iter().copied().enumerate() {
        let start = 2 + index * 2;
        record[start..start + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for name in [
        OVERLAY_DECODED_OUTPUT_FILE,
        MENU_DECODED_OUTPUT_FILE,
        BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(name);
        if path.exists() && !force {
            bail!("title notice output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay_decoded: &[u8],
    menu_decoded: &[u8],
    report: &TitleNoticeBuildReport,
) -> Result<String> {
    std::fs::write(
        output_dir.join(OVERLAY_DECODED_OUTPUT_FILE),
        overlay_decoded,
    )?;
    std::fs::write(output_dir.join(MENU_DECODED_OUTPUT_FILE), menu_decoded)?;
    write_pretty_json_and_hash(&output_dir.join(BUILD_MANIFEST_FILE), report, true)
}

#[cfg(test)]
pub(super) fn rebuild_record_for_test(
    source_record: &[u8],
    korean_text: &str,
    character_codes: &BTreeMap<char, u16>,
) -> Result<Vec<u8>> {
    let codes = korean_text
        .chars()
        .map(|character| {
            if character == ' ' {
                Ok(TRANSPARENT_ADVANCE_CODE)
            } else {
                character_codes
                    .get(&character)
                    .copied()
                    .context("missing test glyph")
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let mut record = source_record.to_vec();
    write_record(&mut record, &codes)?;
    Ok(record)
}
