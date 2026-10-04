use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::rasterize_menu_glyphs;
use crate::name_input::{
    NAME_GLYPH_CELL_BYTES, NAME_GLYPH_PACK_STORAGE_BYTES, NameGlyphConsumerLayout,
    build_name_glyph_band_pack,
};
use crate::pipeline::{map_ordered_parallel, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;

use super::atlas::parse_dialogue_atlas;
use super::dialogue_code_allocation_model::{
    DialogueCharacterCodeAssignment, DialogueCodeAllocationAsset,
};
use super::dialogue_fixed_code_consumers_model::DialogueFixedCodeConsumerAssetAudit;
use super::dialogue_font::{
    DialogueExtensionGlyphInstall, install_dialogue_allocated_glyph_in_atlas,
    install_dialogue_cell_bytes_in_atlas,
};
use super::dialogue_font_build_model::{
    DialogueFontBuild, DialogueFontBuildAsset, DialogueFontBuildConfig, DialogueFontBuildReport,
    DialogueFontBuildSkippedAsset, DialogueFontRecordBuild,
};
use super::dialogue_message_plan_model::DialogueMessageBuildPlan;
use super::format::hex_code;
use super::parser::SELECTOR_TABLE_OFFSET;
use crate::paged_compression::{
    PagedCompressionProfile, compress_page_safe_image, profile_paged_compression,
};

const OUTLINE_PALETTE_INDEX: u8 = 3;
const FILL_PALETTE_INDEX: u8 = 14;
const MATERIALIZED_MESSAGE_OUTPUT_DIRECTORY: &str = "message-images";
const BUILD_MANIFEST_FILE: &str = "dialogue-font-build.json";

#[derive(Debug, serde::Serialize)]
struct InstalledGlyphRecord {
    character: String,
    code: String,
    role: String,
    target_was_fixed_source_cell: bool,
    expected_source_cell_sha256: Option<String>,
    install: DialogueExtensionGlyphInstall,
}

struct BuiltDialogueFontAsset {
    record: DialogueFontRecordBuild,
    report: DialogueFontBuildAsset,
}

pub fn build_dialogue_font_images(config: &DialogueFontBuildConfig) -> Result<DialogueFontBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_dialogue_font_images_from_source(config, &source)
}

pub(super) fn build_dialogue_font_images_from_source(
    config: &DialogueFontBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<DialogueFontBuild> {
    let (message_plan, allocation, prepared_dialogue_sha256) =
        super::prepared_dialogue::load_prepared_dialogue(config, source)?;
    ensure!(
        !config.input_policy.requires_complete_scope()
            || allocation.fixed_code_consumer_ownership_complete,
        "complete-scope dialogue font build requires complete fixed-code consumer ownership"
    );
    ensure!(
        message_plan.development_build_input_available
            && message_plan.selector_development_full_input_available
            && message_plan
                .assets
                .iter()
                .all(|asset| asset.parse_back_verified),
        "dialogue font build requires complete parse-verified development messages"
    );

    prepare_output_directory(&config.output_dir, config.force)?;
    let raster_characters = collect_raster_characters(&allocation.assets)?;
    let rasterized = rasterize_menu_glyphs(
        &config.font,
        &raster_characters,
        config.font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    let font_name = rasterized.font_name;
    let font_sha256 = rasterized.font_sha256;
    let glyph_pixels = rasterized
        .glyphs
        .into_iter()
        .map(|glyph| (glyph.character, glyph.pixels))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        glyph_pixels.len() == raster_characters.chars().count(),
        "rasterized dialogue character population changed"
    );
    let active_name_characters = allocation
        .name_glyph_layout
        .active_glyphs
        .iter()
        .filter(|cell| cell.character != " ")
        .map(|cell| cell.character.as_str())
        .collect::<String>();
    let active_name_rasterized = rasterize_menu_glyphs(
        &config.name_glyph_font,
        &active_name_characters,
        config.name_glyph_font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    let name_glyph_font_name = active_name_rasterized.font_name;
    let name_glyph_font_sha256 = active_name_rasterized.font_sha256;
    let mut active_name_glyph_pixels = active_name_rasterized
        .glyphs
        .into_iter()
        .map(|glyph| (glyph.character, glyph.pixels))
        .collect::<BTreeMap<_, _>>();
    active_name_glyph_pixels.insert(' ', vec![0; 20 * 20]);
    ensure!(
        active_name_glyph_pixels.len() == allocation.name_glyph_layout.active_glyphs.len(),
        "shared name active-glyph raster population changed"
    );
    let name_glyph_pack = build_name_glyph_band_pack(
        &config.name_glyph_font,
        config.name_glyph_font_px,
        NAME_GLYPH_PACK_STORAGE_BYTES,
    )?;
    let name_glyph_materialization =
        crate::name_input::NameGlyphMaterializationBundle::from_bands(&name_glyph_pack.pack);
    let mut name_glyph_pack_payload = name_glyph_pack.pack.bytes().to_vec();
    ensure!(
        name_glyph_pack_payload.len() <= NAME_GLYPH_PACK_STORAGE_BYTES,
        "shared name glyph pack and runtime coordinates exceed their cells"
    );
    name_glyph_pack_payload.resize(NAME_GLYPH_PACK_STORAGE_BYTES, 0);

    let (installable_assets, skipped_assets) =
        classify_font_build_assets(&allocation.fixed_code_consumers);
    ensure!(
        !installable_assets.is_empty(),
        "dialogue font build has no source-safe development assets"
    );
    let built_assets = install_asset_fonts(DialogueFontInstallInputs {
        output_dir: &config.output_dir,
        message_plan: &message_plan,
        allocation_assets: &allocation.assets,
        installable_assets: &installable_assets,
        name_glyph_layout: &allocation.name_glyph_layout,
        glyph_pixels: &glyph_pixels,
        active_name_glyph_pixels: &active_name_glyph_pixels,
        name_glyph_pack_payload: &name_glyph_pack_payload,
    })?;
    let (records, assets): (Vec<_>, Vec<_>) = built_assets
        .into_iter()
        .map(|asset| (asset.record, asset.report))
        .unzip();
    let report = DialogueFontBuildReport {
        prepared_dialogue_sha256,
        backup_slot_text: message_plan.backup_slot_text,
        stat_result_layout: message_plan.stat_result_layout,
        primary_layout: message_plan.primary_layout,
        kind: "Justice Gakuen 2 non-release development dialogue font build".to_string(),
        source_bin_sha256: message_plan.source_bin_sha256,
        codebook_sha256: message_plan.codebook_sha256,
        input_policy: message_plan.input_policy,
        font_name,
        font_sha256,
        font_px: config.font_px,
        name_glyph_font_name,
        name_glyph_font_sha256,
        name_glyph_font_px: config.name_glyph_font_px,
        name_glyph_pack: name_glyph_pack.report,
        outline_palette_index: OUTLINE_PALETTE_INDEX,
        fill_palette_index: FILL_PALETTE_INDEX,
        rasterized_character_count: glyph_pixels.len(),
        selector_development_authored_group_count: message_plan
            .selector_development_authored_group_count,
        development_build_input_available: message_plan.development_build_input_available
            && !assets.is_empty(),
        development_translation_input_available: message_plan
            .development_translation_input_available,
        release_candidate_translation_input_eligible: message_plan
            .release_candidate_translation_input_eligible
            && message_plan.release_candidate_selector_input_eligible,
        compression_maximum_match_words: message_plan
            .assets
            .iter()
            .map(|asset| asset.source_compression_profile.maximum_match_words)
            .max()
            .unwrap_or(0),
        compression_maximum_control_block_output_words: message_plan
            .assets
            .iter()
            .map(|asset| {
                asset
                    .source_compression_profile
                    .maximum_control_block_output_words
            })
            .max()
            .unwrap_or(0),
        fixed_code_consumer_ownership_complete: allocation.fixed_code_consumer_ownership_complete,
        source_asset_count: message_plan.assets.len(),
        asset_count: assets.len(),
        skipped_asset_count: skipped_assets.len(),
        skipped_assets,
        installed_local_glyph_count: assets
            .iter()
            .map(|asset| asset.installed_local_glyph_count)
            .sum(),
        installed_shared_name_cell_count: assets
            .iter()
            .map(|asset| asset.installed_shared_name_cell_count)
            .sum(),
        all_message_regions_preserved: assets.iter().all(|asset| asset.message_region_preserved),
        all_assets_compress_within_original_extents: assets
            .iter()
            .all(|asset| asset.compressed_within_original_extent),
        assets,
    };
    let manifest = write_json(&config.output_dir.join(BUILD_MANIFEST_FILE), &report)?;
    Ok(DialogueFontBuild {
        records,
        manifest_sha256: sha256_bytes(&manifest),
        report,
        name_glyph_materialization,
    })
}

pub(super) fn classify_font_build_assets(
    assets: &[DialogueFixedCodeConsumerAssetAudit],
) -> (BTreeSet<String>, Vec<DialogueFontBuildSkippedAsset>) {
    let installable = assets
        .iter()
        .filter(|asset| asset.preserved_source_glyph_protection_complete)
        .map(|asset| asset.source_path.clone())
        .collect();
    let skipped = assets
        .iter()
        .filter(|asset| !asset.preserved_source_glyph_protection_complete)
        .map(|asset| DialogueFontBuildSkippedAsset {
            source_path: asset.source_path.clone(),
            reason: format!(
                "{} allocated glyph writes would overwrite source glyphs required by preserved untranslated messages",
                asset.protected_source_glyph_overwrite_count
            ),
        })
        .collect();
    (installable, skipped)
}

struct DialogueFontInstallInputs<'a> {
    output_dir: &'a Path,
    message_plan: &'a DialogueMessageBuildPlan,
    allocation_assets: &'a [DialogueCodeAllocationAsset],
    installable_assets: &'a BTreeSet<String>,
    name_glyph_layout: &'a NameGlyphConsumerLayout,
    glyph_pixels: &'a BTreeMap<char, Vec<u8>>,
    active_name_glyph_pixels: &'a BTreeMap<char, Vec<u8>>,
    name_glyph_pack_payload: &'a [u8],
}

fn install_asset_fonts(
    inputs: DialogueFontInstallInputs<'_>,
) -> Result<Vec<BuiltDialogueFontAsset>> {
    let DialogueFontInstallInputs {
        output_dir,
        message_plan,
        allocation_assets,
        installable_assets,
        name_glyph_layout,
        glyph_pixels,
        active_name_glyph_pixels,
        name_glyph_pack_payload,
    } = inputs;
    let assets = map_ordered_parallel(&message_plan.assets, |message_asset| {
        if !installable_assets.contains(&message_asset.source_path) {
            return Ok(None);
        }
        let allocation_asset = allocation_assets
            .iter()
            .find(|asset| asset.source_path == message_asset.source_path)
            .with_context(|| {
                format!(
                    "{} disappeared from the dialogue allocation",
                    message_asset.source_path
                )
            })?;
        ensure!(
            sha256_bytes(&message_asset.rebuilt_decoded) == message_asset.rebuilt_decoded_sha256,
            "dialogue message image changed before font installation"
        );
        let message_decoded = &message_asset.rebuilt_decoded;
        let source_decoded = &message_asset.original_decoded;
        ensure!(
            sha256_bytes(source_decoded) == message_asset.original_decoded_sha256,
            "dialogue source image changed before contribution planning"
        );
        let source_atlas = parse_dialogue_atlas(source_decoded)?;
        let mut font_candidate = source_decoded.clone();
        let mut installed_codes = BTreeSet::new();
        let mut installs = Vec::new();

        for assignment in allocation_asset
            .assignments
            .iter()
            .filter(|assignment| render_dialogue_assignment(assignment))
        {
            installs.push(install_assignment(
                &mut font_candidate,
                &source_atlas,
                assignment,
                if assignment.requires_glyph_install {
                    "asset_local"
                } else {
                    "source_text_restyled"
                },
                glyph_pixels,
                &mut installed_codes,
            )?);
        }
        let installed_local_glyph_count = installs.len();
        let blank_cache_cell = [0_u8; NAME_GLYPH_CELL_BYTES];
        for assignment in &name_glyph_layout.active_glyphs {
            let character = one_character(&assignment.character)?;
            let code = assignment.code;
            ensure!(
                installed_codes.insert(code),
                "dialogue glyph code is assigned to more than one installed character"
            );
            let expected_source_cell_sha256 = (usize::from(code) < source_atlas.fixed_cell_count)
                .then(|| source_atlas.fixed_cell_sha256[usize::from(code)].clone());
            let pixels = active_name_glyph_pixels
                .get(&character)
                .with_context(|| format!("missing shared name glyph {character:?}"))?;
            let install = if character == ' ' {
                install_dialogue_cell_bytes_in_atlas(
                    &mut font_candidate,
                    &source_atlas,
                    code,
                    &blank_cache_cell,
                    expected_source_cell_sha256.as_deref(),
                )?
            } else {
                install_dialogue_allocated_glyph_in_atlas(
                    &mut font_candidate,
                    &source_atlas,
                    code,
                    pixels,
                    expected_source_cell_sha256.as_deref(),
                )?
            };
            installs.push(InstalledGlyphRecord {
                character: character.to_string(),
                code: hex_code(code),
                role: "shared_name_active_glyph".to_string(),
                target_was_fixed_source_cell: expected_source_cell_sha256.is_some(),
                expected_source_cell_sha256,
                install,
            });
        }
        for slot in &name_glyph_layout.cache_slots {
            let code = slot.cache_code;
            ensure!(
                installed_codes.insert(code),
                "dialogue glyph code is assigned to more than one shared name cell"
            );
            let expected_source_cell_sha256 = (usize::from(code) < source_atlas.fixed_cell_count)
                .then(|| source_atlas.fixed_cell_sha256[usize::from(code)].clone());
            let install = install_dialogue_cell_bytes_in_atlas(
                &mut font_candidate,
                &source_atlas,
                code,
                &blank_cache_cell,
                expected_source_cell_sha256.as_deref(),
            )?;
            installs.push(InstalledGlyphRecord {
                character: String::new(),
                code: hex_code(code),
                role: "shared_name_cache".to_string(),
                target_was_fixed_source_cell: expected_source_cell_sha256.is_some(),
                expected_source_cell_sha256,
                install,
            });
        }
        ensure!(
            name_glyph_pack_payload.len()
                == name_glyph_layout.pack_cells.len() * NAME_GLYPH_CELL_BYTES,
            "shared name glyph pack payload no longer fills its cells"
        );
        for (cell, payload) in name_glyph_layout.pack_cells.iter().zip(
            name_glyph_pack_payload
                .as_chunks::<NAME_GLYPH_CELL_BYTES>()
                .0,
        ) {
            let code = cell.code;
            ensure!(
                installed_codes.insert(code),
                "dialogue glyph code is assigned to more than one shared name cell"
            );
            let expected_source_cell_sha256 = (usize::from(code) < source_atlas.fixed_cell_count)
                .then(|| source_atlas.fixed_cell_sha256[usize::from(code)].clone());
            let install = install_dialogue_cell_bytes_in_atlas(
                &mut font_candidate,
                &source_atlas,
                code,
                payload,
                expected_source_cell_sha256.as_deref(),
            )?;
            installs.push(InstalledGlyphRecord {
                character: String::new(),
                code: hex_code(code),
                role: "shared_name_component_pack".to_string(),
                target_was_fixed_source_cell: expected_source_cell_sha256.is_some(),
                expected_source_cell_sha256,
                install,
            });
        }

        let source_decoded_sha256 = sha256_bytes(source_decoded);
        let mut write_plan = DecodedRecordWritePlan::new(
            &message_asset.source_path,
            source_decoded,
            &source_decoded_sha256,
        )?;
        let message_claims = DecodedDataClaim::from_effective_ranges(
            &format!("dialogue-message:{}", message_asset.source_path),
            "install rebuilt dialogue message and selector data",
            source_decoded,
            message_decoded,
            [[SELECTOR_TABLE_OFFSET, source_decoded.len()]],
        )?;
        if !message_claims.is_empty() {
            write_plan.register_data_candidate(
                "dialogue message image producer",
                &source_decoded_sha256,
                message_decoded,
                &message_claims,
            )?;
        }
        let font_claims = DecodedDataClaim::from_effective_ranges(
            &format!("dialogue-font:{}", message_asset.source_path),
            "install dialogue and shared-name glyph cells",
            source_decoded,
            &font_candidate,
            installs
                .iter()
                .map(|install| install.install.decoded_byte_range),
        )?;
        write_plan.register_data_candidate(
            "dialogue font atlas producer",
            &source_decoded_sha256,
            &font_candidate,
            &font_claims,
        )?;
        let font_decoded = write_plan.apply(None)?;
        let message_region_preserved =
            message_decoded[SELECTOR_TABLE_OFFSET..] == font_decoded[SELECTOR_TABLE_OFFSET..];
        ensure!(
            message_region_preserved,
            "dialogue font installation changed message or script bytes"
        );
        let changed_decoded_byte_count = message_decoded
            .iter()
            .zip(&font_decoded)
            .filter(|(before, after)| before != after)
            .count();
        let font_installed_decoded_sha256 = sha256_bytes(&font_decoded);
        let compressed = compress_page_safe_image(
            &font_decoded,
            PagedCompressionProfile::from_source_contract(
                message_asset.source_compression_profile.maximum_match_words,
                message_asset
                    .source_compression_profile
                    .maximum_control_block_output_words,
                message_asset
                    .source_compression_profile
                    .control_blocks_crossing_input_pages,
                message_asset.source_compression_profile.stream_byte_count,
            ),
        )
        .with_context(|| {
            format!(
                "failed to compress font-installed {}",
                message_asset.source_path
            )
        })?;
        let rebuilt_compression_profile = profile_paged_compression(&compressed)?;
        ensure!(
            decompress(&compressed, false)? == font_decoded,
            "font-installed dialogue compression roundtrip changed bytes"
        );
        ensure!(
            compressed.len() <= message_asset.original_stored_byte_count,
            "font-installed dialogue asset exceeds its original extent"
        );
        ensure!(
            compressed.get(..4) == Some(message_asset.source_catalog_prefix.as_slice()),
            "font-installed dialogue compression changed the catalog prefix"
        );
        let compressed_byte_count = compressed.len();
        let mut padded = compressed;
        padded.resize(message_asset.original_stored_byte_count, 0);
        let font_installed_stored_sha256 = sha256_bytes(&padded);

        let stem = asset_stem(&message_asset.source_path)?;
        let decoded_output_file = format!("{stem}.decoded.bin");
        let stored_output_file = format!("{stem}.rebuilt.biz");
        std::fs::write(output_dir.join(&decoded_output_file), &font_decoded)?;
        std::fs::write(output_dir.join(&stored_output_file), &padded)?;
        let installed_fixed_cell_count = installs
            .iter()
            .filter(|install| install.target_was_fixed_source_cell)
            .count();
        let installed_extension_cell_count = installs.len() - installed_fixed_cell_count;
        let installed_code_map_sha256 = sha256_bytes(&serde_json::to_vec(&installs)?);
        let report = DialogueFontBuildAsset {
            source_path: message_asset.source_path.clone(),
            source_stored_sha256: message_asset.original_stored_sha256.clone(),
            message_decoded_sha256: message_asset.rebuilt_decoded_sha256.clone(),
            font_installed_decoded_sha256,
            font_installed_stored_sha256,
            decoded_output_file,
            stored_output_file,
            installed_code_map_sha256,
            installed_local_glyph_count,
            installed_shared_name_cell_count: name_glyph_layout.code_count,
            installed_fixed_cell_count,
            installed_extension_cell_count,
            changed_decoded_byte_count,
            font_write_decoded_byte_range: [source_atlas.pixel_data_offset, SELECTOR_TABLE_OFFSET],
            message_region_preserved,
            compressed_byte_count,
            compression_maximum_match_words: message_asset
                .source_compression_profile
                .maximum_match_words,
            compression_maximum_control_block_output_words: message_asset
                .source_compression_profile
                .maximum_control_block_output_words,
            source_compression_control_blocks_crossing_input_pages: message_asset
                .source_compression_profile
                .control_blocks_crossing_input_pages,
            rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression_profile
                .control_blocks_crossing_input_pages,
            source_compression_stream_byte_count: message_asset
                .source_compression_profile
                .stream_byte_count,
            stored_padding_byte_count: message_asset.original_stored_byte_count
                - compressed_byte_count,
            original_stored_byte_count: message_asset.original_stored_byte_count,
            compression_roundtrip_verified: true,
            compressed_within_original_extent: true,
        };
        Ok(Some(BuiltDialogueFontAsset {
            record: DialogueFontRecordBuild {
                source_path: message_asset.source_path.clone(),
                stored: padded,
            },
            report,
        }))
    })?;
    Ok(assets.into_iter().flatten().collect())
}

fn install_assignment(
    decoded: &mut [u8],
    source_atlas: &super::atlas::ParsedDialogueAtlas,
    assignment: &DialogueCharacterCodeAssignment,
    role: &str,
    glyph_pixels: &BTreeMap<char, Vec<u8>>,
    installed_codes: &mut BTreeSet<u16>,
) -> Result<InstalledGlyphRecord> {
    let character = one_character(&assignment.character)?;
    let code = parse_hex_code(&assignment.code)?;
    ensure!(
        installed_codes.insert(code),
        "dialogue glyph code is assigned to more than one installed character"
    );
    let target_was_fixed_source_cell = usize::from(code) < source_atlas.fixed_cell_count;
    ensure!(
        assignment.target_was_fixed_source_cell == target_was_fixed_source_cell,
        "dialogue allocation target class changed before glyph installation"
    );
    let expected_source_cell_sha256 = target_was_fixed_source_cell
        .then(|| source_atlas.fixed_cell_sha256[usize::from(code)].clone());
    ensure!(
        assignment.target_source_cell_sha256 == expected_source_cell_sha256,
        "dialogue allocation source-cell binding changed before glyph installation"
    );
    let pixels = glyph_pixels
        .get(&character)
        .with_context(|| format!("missing rasterized dialogue glyph {character:?}"))?;
    let install = install_dialogue_allocated_glyph_in_atlas(
        decoded,
        source_atlas,
        code,
        pixels,
        expected_source_cell_sha256.as_deref(),
    )?;
    Ok(InstalledGlyphRecord {
        character: character.to_string(),
        code: hex_code(code),
        role: role.to_string(),
        target_was_fixed_source_cell,
        expected_source_cell_sha256,
        install,
    })
}

pub(super) fn render_dialogue_assignment(assignment: &DialogueCharacterCodeAssignment) -> bool {
    assignment.requires_glyph_install
        || (assignment.source_glyph_reused
            && assignment
                .character
                .chars()
                .any(|character| !character.is_whitespace()))
}

fn collect_raster_characters(assets: &[DialogueCodeAllocationAsset]) -> Result<String> {
    let mut characters = BTreeSet::new();
    for assignment in assets
        .iter()
        .flat_map(|asset| &asset.assignments)
        .filter(|assignment| render_dialogue_assignment(assignment))
    {
        characters.insert(one_character(&assignment.character)?);
    }
    ensure!(
        !characters.is_empty(),
        "dialogue font build has no glyph demand"
    );
    Ok(characters.into_iter().collect())
}

fn one_character(value: &str) -> Result<char> {
    let mut characters = value.chars();
    let character = characters.next().context("empty dialogue character")?;
    ensure!(
        characters.next().is_none(),
        "dialogue character contains more than one scalar"
    );
    Ok(character)
}

fn parse_hex_code(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .context("dialogue code lacks 0x prefix")?;
    Ok(u16::from_str_radix(digits, 16)?)
}

fn asset_stem(source_path: &str) -> Result<String> {
    Ok(Path::new(source_path)
        .file_stem()
        .context("dialogue source path has no stem")?
        .to_string_lossy()
        .to_ascii_lowercase())
}

pub(super) fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        ensure!(
            output_dir.is_dir(),
            "dialogue font output is not a directory"
        );
        if !force && std::fs::read_dir(output_dir)?.next().is_some() {
            bail!(
                "dialogue font output is not empty; pass --force to replace owned files in {}",
                output_dir.display()
            );
        }
        if force {
            let materialized_message_output =
                output_dir.join(MATERIALIZED_MESSAGE_OUTPUT_DIRECTORY);
            if materialized_message_output.exists() {
                ensure!(
                    materialized_message_output.is_dir(),
                    "obsolete dialogue message output is not a directory"
                );
                std::fs::remove_dir_all(&materialized_message_output).with_context(|| {
                    format!(
                        "failed to remove obsolete owned output {}",
                        materialized_message_output.display()
                    )
                })?;
            }
            for entry in std::fs::read_dir(output_dir)? {
                let path = entry?.path();
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".glyph-installs.json"))
                {
                    std::fs::remove_file(&path).with_context(|| {
                        format!("failed to remove obsolete owned output {}", path.display())
                    })?;
                }
            }
        }
        return Ok(());
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<Vec<u8>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::fs::write(path, &bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(bytes)
}
