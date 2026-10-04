use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::contextual_texture_upload::{ContextualMenuGlyph, pack_4bpp_pixels};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::menu_audit::wrapped_cells_overlap;
use crate::menu_compression::profile_menu_stream;
use crate::menu_glyph_slots::{OPTIONS_SMALL_GLYPH_CODE_CANDIDATES, TITLE_MENU_GLYPH_CODES};
use crate::paged_compression::{
    PagedCompressionProfile, compress_page_safe_image_with_seeded_control_block,
    profile_paged_compression, source_paged_compression_profile,
};
use crate::pipeline::{
    EMBEDDED_MOJI2_TIM_SIZE, difference_ranges, sha256_bytes, write_pretty_json_and_hash,
};
use crate::source_disc::SupportedSourceDisc;
use crate::text::{atlas_position, read_length_prefixed_codes};
use crate::tim::{Cell, install_indexed_glyph};
use crate::title_overlay_runtime::install_title_glyph_upload;
use crate::write_scope::changed_ranges_are_within;

use super::assets::{load_assets, parse_hex_usize};
use super::model::{
    ReleaseStatus, TitleMenuBuildConfig, TitleMenuBuildReport, TitleMenuEntryBuild,
    TitleMenuGlyphBuild, TitleMenuRecordBuild,
};
use super::source::load_title_menu_source_from_disc;

pub const BUILD_MANIFEST_FILE: &str = "title-menu-build.json";
const MENU_OUTPUT_FILE: &str = "title-menu-menu.biz";
const OVERLAY_OUTPUT_FILE: &str = "title-menu-mgtit.biz";
const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;

pub fn build_title_menu_assets(config: &TitleMenuBuildConfig) -> Result<TitleMenuRecordBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_title_menu_assets_from_source(config, &source)
}

pub(crate) fn build_title_menu_assets_from_source(
    config: &TitleMenuBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<TitleMenuRecordBuild> {
    let requests = collect_menu_requests(&config.assets, source_disc)?;
    let plan = crate::menu_atlas_plan::MenuAtlasPlan::build(&requests, &[])?;
    build_title_menu_assets_with_plan(config, source_disc, &plan)
}

pub(crate) fn collect_menu_requests(
    assets_root: &Path,
    source_disc: &SupportedSourceDisc,
) -> Result<Vec<crate::menu_atlas_plan::MenuAtlasRequest>> {
    let source = load_title_menu_source_from_disc(source_disc)?;
    let assets = load_assets(assets_root, &source)?;
    let characters = assets
        .entries
        .iter()
        .flat_map(|entry| entry.korean_text.chars())
        .collect::<BTreeSet<_>>();
    Ok(characters
        .into_iter()
        .map(|character| crate::menu_atlas_plan::MenuAtlasRequest {
            key: format!("title_menu:{:04x}", character as u32),
            contexts: BTreeSet::from(["mgtit".to_owned()]),
            candidates: TITLE_MENU_GLYPH_CODES.to_vec(),
            width: 20,
            height: 20,
        })
        .collect())
}

pub(crate) fn build_title_menu_assets_with_plan(
    config: &TitleMenuBuildConfig,
    source_disc: &SupportedSourceDisc,
    plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<TitleMenuRecordBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    ensure!(
        config.font.font_px.is_finite() && config.font.font_px > 0.0,
        "title-adjacent menu font size must be finite and positive"
    );
    let source = load_title_menu_source_from_disc(source_disc)?;
    let assets = load_assets(&config.assets, &source)?;
    let characters = assets
        .entries
        .iter()
        .flat_map(|entry| entry.korean_text.chars())
        .collect::<BTreeSet<_>>();
    ensure!(
        !characters.is_empty() && characters.len() <= TITLE_MENU_GLYPH_CODES.len(),
        "title-adjacent menu needs {} unique glyphs but owns {} cells",
        characters.len(),
        TITLE_MENU_GLYPH_CODES.len()
    );
    ensure_title_menu_slots_are_partitioned()?;
    let allocation = characters
        .into_iter()
        .map(|character| {
            Ok((
                character,
                plan.glyph(&format!("title_menu:{:04x}", character as u32))?
                    .code,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let provisional_codes = allocation.values().copied().collect::<Vec<_>>();
    let mut rendered_menu_decoded = source.menu_decoded.clone();
    let mut glyphs = Vec::with_capacity(allocation.len());
    let mut contextual_glyphs = Vec::with_capacity(allocation.len());
    let mut font_identity = None;
    let rasterizer = IndexedTextRasterizer::load(&config.font.font)?;
    for (character, code) in &allocation {
        let position = atlas_position(*code)?;
        let cell = Cell {
            x: position.x,
            y: position.y,
            width: 20,
            height: 20,
        };
        let rendered = rasterizer.rasterize(
            &character.to_string(),
            cell.width,
            cell.height,
            config.font.font_px,
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
                "title-adjacent menu font identity changed within one build"
            ),
        }
        let install = install_indexed_glyph(
            &mut rendered_menu_decoded[..EMBEDDED_MOJI2_TIM_SIZE],
            cell,
            &rendered.pixels,
            &format!("title-adjacent menu glyph {character:?}"),
        )?;
        let payload = pack_4bpp_pixels(&rendered.pixels)?;
        glyphs.push(TitleMenuGlyphBuild {
            character: *character,
            code: format!("0x{code:04x}"),
            cell,
            ink_bounds: rendered.ink_bounds,
            install,
            global_menu_resident: false,
            runtime_context: "mgtit".to_string(),
            packed_payload_sha256: sha256_bytes(&payload),
        });
        contextual_glyphs.push(ContextualMenuGlyph {
            role: "title_menu".to_string(),
            character: *character,
            code: *code,
            cell,
            payload,
        });
    }
    let rendered_menu_decoded_changed_byte_ranges =
        difference_ranges(&source.menu_decoded, &rendered_menu_decoded);
    let allowed_menu_ranges = glyphs
        .iter()
        .flat_map(|glyph| glyph.install.allowed_decoded_byte_ranges.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        !rendered_menu_decoded_changed_byte_ranges.is_empty()
            && changed_ranges_are_within(
                &rendered_menu_decoded_changed_byte_ranges,
                &allowed_menu_ranges,
            ),
        "title-adjacent menu glyph render escaped its contextual MENU cells"
    );
    let menu_decoded = source.menu_decoded.clone();
    let menu_stored = source.menu_stored.clone();
    let menu_decoded_changed_byte_ranges = difference_ranges(&source.menu_decoded, &menu_decoded);
    ensure!(
        menu_decoded_changed_byte_ranges.is_empty() && menu_stored == source.menu_stored,
        "title-adjacent build retained a global MENU.BIZ write"
    );
    let menu_write_claims = Vec::new();

    let mut overlay_decoded = source.overlay_decoded.clone();
    let mut entry_reports = Vec::with_capacity(assets.entries.len());
    let mut allowed_overlay_ranges = Vec::with_capacity(assets.entries.len());
    let mut overlay_write_claims = Vec::with_capacity(assets.entries.len());
    for entry in assets.entries {
        let codes = entry
            .korean_text
            .chars()
            .map(|character| {
                allocation.get(&character).copied().with_context(|| {
                    format!(
                        "title-adjacent menu {} has no glyph for {character:?}",
                        entry.id
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        write_length_prefixed_record(
            &mut overlay_decoded,
            source_offset,
            entry.source_record_size,
            &codes,
        )?;
        ensure!(
            read_length_prefixed_codes(&overlay_decoded, source_offset)? == codes,
            "title-adjacent menu {} rebuilt codes changed",
            entry.id
        );
        allowed_overlay_ranges.push([source_offset, source_offset + entry.source_record_size]);
        overlay_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("mgtit:title-adjacent:record:{}", entry.id),
            &format!("replace title-adjacent menu text record {}", entry.id),
            [[source_offset, source_offset + entry.source_record_size]],
        ));
        entry_reports.push(TitleMenuEntryBuild {
            id: entry.id,
            source_offset: entry.source_offset,
            source_text: entry.source_text,
            korean_text: entry.korean_text,
            output_codes: codes.iter().map(|code| format!("0x{code:04x}")).collect(),
            placement_record_offset: entry.placement_record_offset,
            x: entry.source_x,
            y: entry.source_y,
            development_status: entry.development_status,
            release_status: entry.release_status,
        });
    }
    let text_overlay_changed_byte_ranges =
        difference_ranges(&source.overlay_decoded, &overlay_decoded);
    ensure!(
        !text_overlay_changed_byte_ranges.is_empty()
            && changed_ranges_are_within(
                &text_overlay_changed_byte_ranges,
                &allowed_overlay_ranges,
            ),
        "title-adjacent menu rebuild escaped its source-owned MGTIT records"
    );
    overlay_write_claims.retain(|claim| {
        source.overlay_decoded[claim.range.clone()] != overlay_decoded[claim.range.clone()]
    });
    let overlay_text_decoded = overlay_decoded.clone();
    let overlay_text_write_claims = overlay_write_claims.clone();
    let runtime_upload = install_title_glyph_upload(
        &source.overlay_decoded,
        &mut overlay_decoded,
        &contextual_glyphs,
    )?;
    overlay_write_claims.extend(runtime_upload.write_claims);
    let overlay_changed_byte_ranges = difference_ranges(&source.overlay_decoded, &overlay_decoded);
    let allowed_overlay_ranges = overlay_write_claims
        .iter()
        .map(|claim| [claim.range.start, claim.range.end])
        .collect::<Vec<_>>();
    ensure!(
        changed_ranges_are_within(&overlay_changed_byte_ranges, &allowed_overlay_ranges),
        "title-adjacent menu build escaped its MGTIT Expected Writes"
    );

    let (overlay_reencoded, source_overlay_compression, rebuilt_overlay_compression) =
        compress_title_overlay_with_source_limits(&overlay_decoded, &source.overlay_stored)?;
    ensure!(
        decompress(&overlay_reencoded, false)? == overlay_decoded,
        "title-adjacent MGTIT.BIZ compression roundtrip changed decoded bytes"
    );
    let rebuilt_overlay_stored_size = overlay_reencoded.len();
    let mut overlay_stored = overlay_reencoded;
    overlay_stored.resize(source.overlay_stored.len(), 0);
    ensure!(
        decompress(&overlay_stored, true)? == overlay_decoded,
        "padded title-adjacent MGTIT.BIZ changed decoded bytes"
    );

    let source_compression = profile_menu_stream(&source.menu_stored)?;
    let rebuilt_compression = source_compression;
    let rebuilt_menu_stored_size = source_compression.stream_byte_count;

    let release_approved_entry_count = entry_reports
        .iter()
        .filter(|entry| entry.release_status == ReleaseStatus::Approved)
        .count();
    let (font_name, font_sha256) =
        font_identity.context("title-adjacent menu build rendered no glyphs")?;
    let report = TitleMenuBuildReport {
        kind: "Justice Gakuen 2 source-bound title-adjacent menu build".to_string(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_bin_sha256: source.source_bin_sha256,
        source_menu_stored_sha256: sha256_bytes(&source.menu_stored),
        source_menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        output_menu_stored_sha256: sha256_bytes(&menu_stored),
        output_menu_decoded_sha256: sha256_bytes(&menu_decoded),
        source_overlay_stored_sha256: sha256_bytes(&source.overlay_stored),
        source_overlay_decoded_sha256: sha256_bytes(&source.overlay_decoded),
        output_overlay_stored_sha256: sha256_bytes(&overlay_stored),
        output_overlay_decoded_sha256: sha256_bytes(&overlay_decoded),
        translation_manifest_sha256: assets.manifest_sha256,
        font_name,
        font_sha256,
        font_px: config.font.font_px,
        entry_count: entry_reports.len(),
        release_approved_entry_count,
        development_input_available: entry_reports.len() == 2,
        release_candidate_input_eligible: release_approved_entry_count == entry_reports.len(),
        source_records_match: true,
        source_placements_match: true,
        changed_bytes_confined_to_owned_ranges: true,
        runtime_catalog_prefix_matches: true,
        allocation_status: "MGTIT entry-scoped runtime upload; source MENU glyph pixels preserved"
            .to_string(),
        allocation_proven_reclaimable: false,
        provisional_codes: provisional_codes
            .iter()
            .map(|code| format!("0x{code:04x}"))
            .collect(),
        menu_decoded_changed_byte_ranges,
        overlay_changed_byte_ranges,
        original_menu_stored_size: source.menu_stored.len(),
        rebuilt_menu_stored_size,
        menu_padding_size: source.menu_stored.len() - rebuilt_menu_stored_size,
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        original_overlay_stored_size: source.overlay_stored.len(),
        rebuilt_overlay_stored_size,
        overlay_padding_size: source.overlay_stored.len() - rebuilt_overlay_stored_size,
        source_overlay_compression_maximum_match_words: source_overlay_compression
            .maximum_match_words,
        source_overlay_compression_maximum_control_block_output_words: source_overlay_compression
            .maximum_control_block_output_words,
        source_overlay_control_blocks_crossing_input_pages: source_overlay_compression
            .control_blocks_crossing_input_pages,
        source_overlay_stream_byte_count: source_overlay_compression.stream_byte_count,
        rebuilt_overlay_compression_maximum_match_words: rebuilt_overlay_compression
            .maximum_match_words,
        rebuilt_overlay_compression_maximum_control_block_output_words: rebuilt_overlay_compression
            .maximum_control_block_output_words,
        rebuilt_overlay_control_blocks_crossing_input_pages: rebuilt_overlay_compression
            .control_blocks_crossing_input_pages,
        rebuilt_overlay_stream_byte_count: rebuilt_overlay_compression.stream_byte_count,
        runtime_glyph_upload: runtime_upload.report,
        glyphs,
        entries: entry_reports,
        menu_output_file: MENU_OUTPUT_FILE.to_string(),
        overlay_output_file: OVERLAY_OUTPUT_FILE.to_string(),
    };
    let build_manifest_sha256 =
        write_outputs(&config.output_dir, &menu_stored, &overlay_stored, &report)?;
    Ok(TitleMenuRecordBuild {
        source_menu_stored: source.menu_stored,
        source_menu_decoded: source.menu_decoded,
        source_overlay_stored: source.overlay_stored,
        source_overlay_decoded: source.overlay_decoded,
        menu_stored,
        menu_decoded,
        menu_write_claims,
        overlay_stored,
        overlay_decoded,
        overlay_text_decoded,
        overlay_text_write_claims,
        contextual_glyphs,
        build_manifest_sha256,
        report,
    })
}

fn ensure_title_menu_slots_are_partitioned() -> Result<()> {
    for title_code in TITLE_MENU_GLYPH_CODES {
        ensure!(
            OPTIONS_SMALL_GLYPH_CODE_CANDIDATES
                .iter()
                .all(|options_code| !wrapped_cells_overlap(title_code, *options_code)),
            "title-adjacent menu glyph code 0x{title_code:04x} overlaps the options allocation"
        );
    }
    Ok(())
}

pub(super) fn write_length_prefixed_record(
    output: &mut [u8],
    offset: usize,
    record_size: usize,
    codes: &[u16],
) -> Result<()> {
    let encoded_size = 2 + codes.len() * 2;
    ensure!(
        encoded_size <= record_size,
        "title-adjacent menu record needs {encoded_size} bytes but owns {record_size}"
    );
    let record = output
        .get_mut(offset..offset + record_size)
        .context("title-adjacent menu record write is out of bounds")?;
    record.fill(0);
    record[..2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let code_offset = 2 + index * 2;
        record[code_offset..code_offset + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

pub(crate) fn compress_title_overlay_with_source_limits(
    decoded: &[u8],
    source_stored: &[u8],
) -> Result<(Vec<u8>, PagedCompressionProfile, PagedCompressionProfile)> {
    let source_profile = source_paged_compression_profile(source_stored)?;
    let seeded_control_block = build_overlay_seeded_control_block(decoded, source_stored)?;
    let compressed = compress_page_safe_image_with_seeded_control_block(
        decoded,
        source_profile,
        &seeded_control_block,
    )?;
    ensure!(
        compressed[..4] == source_stored[..4],
        "rebuilt MGTIT changed its runtime catalog prefix"
    );
    let rebuilt_profile = profile_paged_compression(&compressed)?;
    Ok((compressed, source_profile, rebuilt_profile))
}

fn build_overlay_seeded_control_block(decoded: &[u8], source_stored: &[u8]) -> Result<Vec<u8>> {
    const CONTROL_BLOCK_SIZE: usize = 34;
    const MATCH_OUTPUT_WORD_COUNT: usize = 5;
    const FIRST_TRAILING_LITERAL_WORD: usize = 8;
    const LAST_TRAILING_LITERAL_WORD: usize = 20;

    let mut block = source_stored
        .get(..CONTROL_BLOCK_SIZE)
        .context("MGTIT source control block is truncated")?
        .to_vec();
    ensure!(
        u16::from_le_bytes(block[..2].try_into()?) == 0x1000
            && u16::from_le_bytes(block[8..10].try_into()?)
                == (((MATCH_OUTPUT_WORD_COUNT as u16) << 11) | 1),
        "MGTIT source catalog control block changed"
    );
    for word_index in 0..3 {
        let decoded_offset = word_index * 2;
        let encoded_offset = 2 + word_index * 2;
        block[encoded_offset..encoded_offset + 2]
            .copy_from_slice(&decoded[decoded_offset..decoded_offset + 2]);
    }
    for word_index in FIRST_TRAILING_LITERAL_WORD..LAST_TRAILING_LITERAL_WORD {
        let decoded_offset = word_index * 2;
        let encoded_offset = 10 + (word_index - FIRST_TRAILING_LITERAL_WORD) * 2;
        block[encoded_offset..encoded_offset + 2]
            .copy_from_slice(&decoded[decoded_offset..decoded_offset + 2]);
    }
    ensure!(
        block[..4] == source_stored[..4],
        "MGTIT translated records changed its runtime catalog key"
    );
    Ok(block)
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for name in [MENU_OUTPUT_FILE, OVERLAY_OUTPUT_FILE, BUILD_MANIFEST_FILE] {
        let path = output_dir.join(name);
        if path.exists() && !force {
            bail!("title-adjacent menu output exists; pass --force to replace it");
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
    menu_stored: &[u8],
    overlay_stored: &[u8],
    report: &TitleMenuBuildReport,
) -> Result<String> {
    std::fs::write(output_dir.join(MENU_OUTPUT_FILE), menu_stored)?;
    std::fs::write(output_dir.join(OVERLAY_OUTPUT_FILE), overlay_stored)?;
    write_pretty_json_and_hash(&output_dir.join(BUILD_MANIFEST_FILE), report, true)
}
