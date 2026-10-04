use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::{compress, decompress};
use crate::container_plan::{ContainerMemberSpec, ContainerPlan, MemberContribution};
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{Cell, parse_8bpp, read_8bpp_indexed_cell, write_8bpp_indexed_cell};
use crate::tzz::parse_tzz;

use super::assets::{DiarySceneAssets, load_diary_scene_assets};
use super::model::{
    DiarySceneBuild, DiarySceneBuildConfig, DiarySceneBuildReport, DiarySceneCatalogueMemberReport,
    DiarySceneEntryBuild, DiarySceneFontBuild, DiarySceneFontRole, DiarySceneFontSources,
};
use super::runtime_bundle_build::build_runtime_bundles;
use super::source::{DIARY_SCENE_PATH, SOURCE_ARCHIVE_SHA256, load_source_from_disc};

pub const DIARY_SCENE_OUTPUT_FILE: &str = "MGBG.TZZ";
pub const DIARY_SCENE_BUILD_MANIFEST_FILE: &str = "diary-scene-build.json";
const TZZ_MEMBER_ALIGNMENT: usize = 0x800;
const CATALOGUE_CONTAINER_OWNER: &str = "diary-scene catalogue compositor";
const CATALOGUE_CONTAINER_PURPOSE: &str = "compose translated diary-scene members";

pub fn build_diary_scene(config: &DiarySceneBuildConfig) -> Result<DiarySceneBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_diary_scene_from_source(config, &source)
}

pub(crate) fn build_diary_scene_from_source(
    config: &DiarySceneBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<DiarySceneBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let assets = load_diary_scene_assets(&config.assets)?;
    let source = load_source_from_disc(source_disc)?;
    validate_source_bindings(&source, &assets)?;
    let backgrounds = super::scene_backgrounds::load(
        config,
        assets.scene_backgrounds.as_deref(),
        &source.decoded_members,
    )?;
    ensure!(
        backgrounds.keys().all(|index| assets
            .entries
            .iter()
            .any(|entry| entry.member_index == *index)),
        "scene background has no owning catalogue entry"
    );

    let catalogue_specs = source
        .members
        .iter()
        .zip(&assets.source.members)
        .map(|(member, binding)| ContainerMemberSpec {
            id: catalogue_member_id(member.index),
            writable_range: member.compressed_range(),
            slot_range: member.slot_range(),
            slot_start_alignment: TZZ_MEMBER_ALIGNMENT,
            expected_source_sha256: binding.compressed_sha256.clone(),
        })
        .collect();
    let mut archive_plan = ContainerPlan::new(
        DIARY_SCENE_PATH,
        CATALOGUE_CONTAINER_OWNER,
        CATALOGUE_CONTAINER_PURPOSE,
        &source.archive,
        SOURCE_ARCHIVE_SHA256,
        catalogue_specs,
    )?;
    let mut patched_members = source.decoded_members.clone();
    let mut rebuilt_member_sizes = source
        .members
        .iter()
        .map(|member| member.compressed_size)
        .collect::<Vec<_>>();
    let mut entry_builds = Vec::with_capacity(assets.entries.len());
    let mut font_builds = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let entries_by_member = assets
        .entries
        .iter()
        .map(|entry| (entry.member_index, entry))
        .collect::<BTreeMap<_, _>>();

    for (member, binding) in source.members.iter().zip(&assets.source.members) {
        let source_decoded = &source.decoded_members[member.index];
        let Some(entry) = entries_by_member.get(&member.index) else {
            continue;
        };
        let source_region_sha256 = binding
            .location_block_sha256
            .as_ref()
            .context("translated diary scene member has no location block hash")?;
        let palette_roles = binding
            .location_palette_roles
            .context("translated diary scene member has no location palette roles")?;
        let source_pixels = read_8bpp_indexed_cell(source_decoded, assets.source.location_block)?;
        ensure!(
            sha256_bytes(&source_pixels) == *source_region_sha256,
            "diary scene source region {} changed",
            entry.id
        );

        let style = font_for_role(&config.fonts, entry.font_role);
        let raster = rasterizers.for_font(&style.path)?.rasterize(
            &entry.korean_text,
            entry.cell.width,
            entry.cell.height,
            style.font_px,
            0.0,
            palette_roles.clear_index,
            Some(palette_roles.outline_index),
            palette_roles.fill_index,
            HorizontalTextAlignment::Left,
        )?;
        let mut entry_candidate = source_decoded.clone();
        let (clear_cell, allowed_ranges) = clear_location_label(
            &mut entry_candidate,
            assets.source.location_block,
            entry.cell,
            palette_roles.clear_index,
        )?;
        let entry_ranges =
            write_8bpp_indexed_cell(&mut entry_candidate, entry.cell, &raster.pixels)?;
        ensure!(
            entry_ranges.iter().all(|[start, end]| allowed_ranges
                .iter()
                .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)),
            "diary scene entry {} escaped its owned location block",
            entry.id
        );
        let output_pixels = read_8bpp_indexed_cell(&entry_candidate, assets.source.location_block)?;
        let mut preserved_location_background_pixel_count = 0;
        for (offset, (source, output)) in source_pixels.iter().zip(&output_pixels).enumerate() {
            let local_x = offset % assets.source.location_block.width;
            let local_y = offset / assets.source.location_block.width;
            let x = assets.source.location_block.x + local_x;
            let y = assets.source.location_block.y + local_y;
            let outside_text_cell = x < clear_cell.x
                || x >= clear_cell.x + clear_cell.width
                || y < clear_cell.y
                || y >= clear_cell.y + clear_cell.height;
            if outside_text_cell {
                ensure!(
                    source == output,
                    "diary scene entry {} changed background outside its cleared label region",
                    entry.id
                );
                preserved_location_background_pixel_count += 1;
            }
        }
        ensure!(
            preserved_location_background_pixel_count
                == assets.source.location_block.width * assets.source.location_block.height
                    - clear_cell.width * clear_cell.height,
            "diary scene entry {} changed background outside its cleared label region",
            entry.id
        );
        let changed_decoded_byte_count = source_pixels
            .iter()
            .zip(&output_pixels)
            .filter(|(left, right)| left != right)
            .count();
        ensure!(
            changed_decoded_byte_count > 0,
            "diary scene entry {} changed no bytes",
            entry.id
        );

        let source_decoded_sha256 = sha256_bytes(source_decoded);
        let decoded_claims = DecodedDataClaim::from_effective_ranges(
            &format!("diary-scene-entry:{}", entry.id),
            "clear and render one diary-scene location label",
            source_decoded,
            &entry_candidate,
            allowed_ranges,
        )?;
        let decoded_target = format!("{DIARY_SCENE_PATH}:member:{}", member.index);
        let mut decoded_plan =
            DecodedRecordWritePlan::new(&decoded_target, source_decoded, &source_decoded_sha256)?;
        let decoded_owner = format!("diary-scene entry producer: {}", entry.id);
        decoded_plan.register_data_candidate(
            &decoded_owner,
            &source_decoded_sha256,
            &entry_candidate,
            &decoded_claims,
        )?;
        if let Some(background) = backgrounds.get(&member.index) {
            decoded_plan.register_data_candidate(
                "full scene background lettering",
                &source_decoded_sha256,
                &background.bytes,
                &DecodedDataClaim::from_ranges(
                    "scene-background",
                    "compose full image while retaining the location strip",
                    vec![background.range],
                ),
            )?;
            entry_candidate[background.range[0]..background.range[1]]
                .copy_from_slice(&background.bytes[background.range[0]..background.range[1]]);
        }
        let patched_decoded = decoded_plan.apply(None)?;
        ensure!(
            patched_decoded == entry_candidate,
            "diary scene entry {} decoded plan omitted rendered bytes",
            entry.id
        );

        let reencoded = compress(&patched_decoded, 0)?;
        ensure!(
            decompress(&reencoded, false)? == patched_decoded,
            "diary scene member {} compression roundtrip changed decoded bytes",
            member.index
        );
        ensure!(
            reencoded.len() <= member.compressed_size,
            "rebuilt diary scene member {} exceeds its original compressed extent",
            member.index
        );
        let rebuilt_size = reencoded.len();
        let mut stored = vec![0; member.compressed_size];
        stored[..rebuilt_size].copy_from_slice(&reencoded);
        ensure!(
            decompress(&stored, true)? == patched_decoded,
            "padded diary scene member {} changed decoded bytes",
            member.index
        );
        let patched_decoded_sha256 = sha256_bytes(&patched_decoded);
        let patched_stored_sha256 = sha256_bytes(&stored);
        let member_id = catalogue_member_id(member.index);
        let stored_owner = format!("diary-scene member producer: {}", entry.id);
        let stored_purpose = format!("store translated diary-scene entry {}", entry.id);
        let lineage_id = format!(
            "diary-scene-stored-result:{}:{}:{}",
            entry.id, patched_decoded_sha256, patched_stored_sha256
        );
        archive_plan.register_member(MemberContribution {
            member_id: &member_id,
            owner: &stored_owner,
            purpose: &stored_purpose,
            consumed_source_sha256: &binding.compressed_sha256,
            candidate: &stored,
            lineage_id: &lineage_id,
        })?;
        rebuilt_member_sizes[member.index] = rebuilt_size;
        patched_members[member.index] = patched_decoded;
        font_builds
            .entry(entry.font_role)
            .or_insert(DiarySceneFontBuild {
                role: entry.font_role,
                font_name: raster.font_name,
                font_sha256: raster.font_sha256,
                font_px: style.font_px,
            });
        entry_builds.push(DiarySceneEntryBuild {
            id: entry.id.clone(),
            member_index: entry.member_index,
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            font_role: entry.font_role,
            cell: entry.cell,
            cleared_cell: clear_cell,
            source_region_sha256: source_region_sha256.clone(),
            measured_advance_px: raster.measured_advance_px,
            ink_bounds: raster.ink_bounds,
            changed_decoded_byte_count,
            preserved_location_background_pixel_count,
        });
    }
    let mut calendar_panels = Vec::new();
    if let Some(path) = &assets.calendar_backgrounds {
        for candidate in super::calendar::render(config, path, &source.decoded_members)? {
            let index = candidate.index;
            let member = &source.members[index];
            let binding = &assets.source.members[index];
            ensure!(
                patched_members[index] == source.decoded_members[index],
                "calendar overlaps another scene producer"
            );
            let encoded = compress(&candidate.bytes, 0)?;
            ensure!(
                encoded.len() <= member.compressed_size,
                "calendar member {index} exceeds source compressed extent: {} > {}",
                encoded.len(),
                member.compressed_size
            );
            let mut stored = vec![0; member.compressed_size];
            stored[..encoded.len()].copy_from_slice(&encoded);
            ensure!(
                decompress(&stored, true)? == candidate.bytes,
                "calendar compression roundtrip failed"
            );
            let ranges = crate::pipeline::difference_ranges(
                &source.decoded_members[index],
                &candidate.bytes,
            );
            let claims = DecodedDataClaim::from_effective_ranges(
                "calendar-panel",
                "render masked calendar title/date",
                &source.decoded_members[index],
                &candidate.bytes,
                ranges,
            )?;
            let decoded_target = format!("calendar:{index}");
            let mut plan = DecodedRecordWritePlan::new(
                &decoded_target,
                &source.decoded_members[index],
                &binding.decoded_sha256,
            )?;
            plan.register_data_candidate(
                "calendar-panel",
                &binding.decoded_sha256,
                &candidate.bytes,
                &claims,
            )?;
            ensure!(
                plan.apply(None)? == candidate.bytes,
                "calendar decoded plan mismatch"
            );
            archive_plan.register_member(MemberContribution {
                member_id: &catalogue_member_id(index),
                owner: "calendar-panel",
                purpose: "store translated calendar panel",
                consumed_source_sha256: &binding.compressed_sha256,
                candidate: &stored,
                lineage_id: &format!("calendar:{index}:{}", sha256_bytes(&stored)),
            })?;
            rebuilt_member_sizes[index] = encoded.len();
            patched_members[index] = candidate.bytes;
            calendar_panels.push(candidate.report);
        }
    }
    let archive_output = archive_plan
        .seal()?
        .context("diary-scene catalogue produced no member contributions")?;
    ensure!(
        archive_output.path == DIARY_SCENE_PATH
            && archive_output.owner == CATALOGUE_CONTAINER_OWNER
            && archive_output.purpose == CATALOGUE_CONTAINER_PURPOSE
            && archive_output.report.source_sha256 == SOURCE_ARCHIVE_SHA256
            && archive_output.report.candidate_sha256 == sha256_bytes(&archive_output.data),
        "diary-scene catalogue container identity changed"
    );
    let archive = archive_output.data;
    ensure!(
        parse_tzz(&archive)? == source.members,
        "MGBG.TZZ member table changed"
    );
    let catalogue_member_reports = source
        .members
        .iter()
        .zip(&assets.source.members)
        .map(|(member, binding)| DiarySceneCatalogueMemberReport {
            index: member.index,
            source_compressed_sha256: binding.compressed_sha256.clone(),
            source_decoded_sha256: binding.decoded_sha256.clone(),
            patched_compressed_sha256: sha256_bytes(&archive[member.compressed_range()]),
            patched_decoded_sha256: sha256_bytes(&patched_members[member.index]),
            source_compressed_size: member.compressed_size,
            rebuilt_compressed_size: rebuilt_member_sizes[member.index],
            tim_vram: binding.tim_vram,
            tim_pixel_size: binding.tim_pixel_size,
            clut_vram: binding.clut_vram,
            clut_size: binding.clut_size,
            clear_palette_index: binding
                .location_palette_roles
                .map(|roles| roles.clear_index),
            outline_palette_index: binding
                .location_palette_roles
                .map(|roles| roles.outline_index),
            fill_palette_index: binding.location_palette_roles.map(|roles| roles.fill_index),
            changed: source.decoded_members[member.index] != patched_members[member.index],
        })
        .collect::<Vec<_>>();
    ensure!(
        archive_output
            .report
            .members
            .iter()
            .filter(|member| member.changed_byte_count > 0)
            .count()
            == entry_builds.len() + calendar_panels.len(),
        "diary-scene catalogue contribution count changed"
    );

    let portraits =
        super::portraits::build(source_disc, assets.portraits.as_deref(), &config.output_dir)?;
    let mut artwork_archives = portraits.archives;
    if let Some(path) = &assets.month_overview {
        artwork_archives.push(super::month_overview::build(config, source_disc, path)?);
    }
    let mut runtime_sources = source.decoded_members.clone();
    let mut runtime_patched = patched_members.clone();
    runtime_sources.extend(portraits.sources);
    runtime_patched.extend(portraits.patched);
    let runtime_bundles = build_runtime_bundles(
        source_disc,
        &assets.runtime_bundles,
        &runtime_sources,
        &runtime_patched,
        &assets.fixed_presentations,
        &config.fonts,
    )?;
    let runtime_bundle_reports = runtime_bundles
        .iter()
        .map(|bundle| bundle.report.clone())
        .collect::<Vec<_>>();
    let source_runtime_member_count = assets
        .runtime_bundles
        .iter()
        .map(|bundle| bundle.members.len())
        .sum();
    let patched_runtime_member_count = runtime_bundle_reports
        .iter()
        .map(|bundle| bundle.members.len())
        .sum();
    let fixed_presentation_consumer_count = assets
        .fixed_presentations
        .iter()
        .map(|family| family.consumers.len())
        .sum();
    let patched_fixed_presentation_consumer_count = runtime_bundle_reports
        .iter()
        .flat_map(|bundle| &bundle.members)
        .map(|member| member.fixed_presentation_family_ids.len())
        .sum::<usize>();
    ensure!(
        patched_fixed_presentation_consumer_count == fixed_presentation_consumer_count,
        "diary scene fixed-presentation consumer coverage changed"
    );
    let runtime_unclaimed_decoded_bytes_preserved = runtime_bundle_reports
        .iter()
        .flat_map(|bundle| &bundle.members)
        .all(|member| member.unclaimed_decoded_bytes_preserved);
    ensure!(
        runtime_unclaimed_decoded_bytes_preserved,
        "diary scene runtime bundle changed unclaimed decoded bytes"
    );
    let runtime_compression_requirements_satisfied = runtime_bundle_reports
        .iter()
        .flat_map(|bundle| &bundle.members)
        .all(|member| member.compression_requirement_satisfied);
    ensure!(
        runtime_compression_requirements_satisfied,
        "diary scene runtime bundle exceeded a source compression contract"
    );

    let report = DiarySceneBuildReport {
        kind: "Justice Gakuen 2 Korean diary scene graphics build".to_string(),
        source_path: DIARY_SCENE_PATH.to_string(),
        source_archive_size: source.archive.len(),
        source_archive_sha256: SOURCE_ARCHIVE_SHA256.to_string(),
        patched_archive_sha256: sha256_bytes(&archive),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_regions_match: true,
        changed_bytes_confined_to_owned_cells: true,
        catalogue_background_outside_text_cells_preserved: backgrounds.is_empty()
            && calendar_panels.is_empty(),
        archive_changes_confined_to_owned_members: true,
        source_runtime_bundle_count: assets.runtime_bundles.len(),
        source_runtime_member_count,
        patched_runtime_bundle_count: runtime_bundles.len(),
        patched_runtime_member_count,
        fixed_presentation_family_count: assets.fixed_presentations.len(),
        fixed_presentation_translation_catalogues: assets
            .fixed_presentations
            .iter()
            .filter_map(|family| {
                family
                    .team_up_names_sha256
                    .as_ref()
                    .map(|sha256| (family.id.clone(), sha256.clone()))
            })
            .collect(),
        fixed_presentation_consumer_count,
        fixed_presentation_source_regions_match: true,
        fixed_presentation_preserved_regions_unchanged: true,
        runtime_unclaimed_decoded_bytes_preserved,
        runtime_compression_requirements_satisfied,
        development_input_available: true,
        release_candidate_input_eligible: false,
        catalogue_members: catalogue_member_reports,
        runtime_bundles: runtime_bundle_reports,
        fonts: font_builds.into_values().collect(),
        entries: entry_builds,
        calendar_panels,
        artwork_archives: artwork_archives.iter().map(|a| a.report.clone()).collect(),
    };
    std::fs::write(config.output_dir.join(DIARY_SCENE_OUTPUT_FILE), &archive)?;
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    let manifest_sha256 = sha256_bytes(&report_bytes);
    std::fs::write(
        config.output_dir.join(DIARY_SCENE_BUILD_MANIFEST_FILE),
        &report_bytes,
    )?;
    Ok(DiarySceneBuild {
        archive,
        artwork_archives,
        runtime_bundles,
        manifest_sha256,
        report,
    })
}

fn validate_source_bindings(
    source: &super::source::DiarySceneSource,
    assets: &DiarySceneAssets,
) -> Result<()> {
    ensure!(
        source.members.len() == assets.source.members.len(),
        "diary scene source catalogue member count changed"
    );
    for ((member, decoded), binding) in source
        .members
        .iter()
        .zip(&source.decoded_members)
        .zip(&assets.source.members)
    {
        ensure!(
            member.index == binding.index
                && member.offset == binding.offset
                && member.compressed_size == binding.compressed_size,
            "diary scene source member {} geometry changed",
            binding.index
        );
        let compressed = &source.archive[member.compressed_range()];
        ensure!(
            sha256_bytes(compressed) == binding.compressed_sha256,
            "diary scene source member {} compressed hash changed",
            member.index
        );
        ensure!(
            decoded.len() == binding.decoded_size
                && sha256_bytes(decoded) == binding.decoded_sha256,
            "diary scene source member {} decoded hash changed",
            member.index
        );
        let tim = parse_8bpp(decoded)?;
        ensure!(
            [tim.image_x, tim.image_y] == binding.tim_vram
                && [tim.pixel_width(), tim.image_height] == binding.tim_pixel_size
                && [tim.clut_x, tim.clut_y] == binding.clut_vram
                && [tim.clut_width, tim.clut_height] == binding.clut_size,
            "diary scene source member {} TIM geometry changed",
            member.index
        );
    }
    Ok(())
}

fn catalogue_member_id(index: usize) -> String {
    format!("catalogue-member-{index}")
}

fn font_for_role(fonts: &DiarySceneFontSources, role: DiarySceneFontRole) -> &SizedFontSource {
    match role {
        DiarySceneFontRole::LocationLabel => &fonts.location_label,
    }
}

fn prepare_output_directory(path: &Path, force: bool) -> Result<()> {
    if path.exists() {
        if !force {
            bail!("output directory already exists: {}", path.display());
        }
        std::fs::remove_dir_all(path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
    }
    std::fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}

fn clear_location_label(
    candidate: &mut [u8],
    location_block: Cell,
    text_cell: Cell,
    clear_index: u8,
) -> Result<(Cell, Vec<[usize; 2]>)> {
    // Source glyphs can lean left of the Korean text inset. The right edge
    // stays at the authored boundary because narrow rows contain adjacent art.
    let clear_cell = Cell {
        x: location_block.x,
        width: text_cell.x + text_cell.width - location_block.x,
        ..text_cell
    };
    let pixels = vec![clear_index; clear_cell.width * clear_cell.height];
    let ranges = write_8bpp_indexed_cell(candidate, clear_cell, &pixels)?;
    Ok((clear_cell, ranges))
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn clears_original_ink_before_the_korean_inset_and_preserves_neighboring_art() {
        // A native location row can include background art to the right of its label.
        let mut tim = Vec::new();
        for value in [0x10u32, 9, 524] {
            tim.extend(value.to_le_bytes());
        }
        for value in [0u16, 0, 256, 1] {
            tim.extend(value.to_le_bytes());
        }
        tim.extend([0u8; 512]);
        tim.extend(36u32.to_le_bytes());
        for value in [0u16, 0, 6, 2] {
            tim.extend(value.to_le_bytes());
        }
        tim.extend([7u8; 24]);
        let source = tim.clone();
        let block = Cell {
            x: 0,
            y: 1,
            width: 12,
            height: 1,
        };
        let text = Cell {
            x: 2,
            y: 1,
            width: 6,
            height: 1,
        };
        clear_location_label(&mut tim, block, text, 0).unwrap();
        write_8bpp_indexed_cell(&mut tim, text, &[0, 1, 1, 1, 0, 0]).unwrap();
        assert_eq!(
            read_8bpp_indexed_cell(&tim, block).unwrap(),
            [0, 0, 0, 1, 1, 1, 0, 0, 7, 7, 7, 7]
        );
        // Header, palette, and the preceding picture row are never erased.
        assert_eq!(&tim[..tim.len() - 12], &source[..source.len() - 12]);
    }
}
