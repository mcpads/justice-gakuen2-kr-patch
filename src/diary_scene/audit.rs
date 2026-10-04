use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::tim::{Cell, parse_8bpp, read_8bpp_indexed_cell};
use crate::tim_preview::write_tim_preview;
use crate::tzz::parse_tzz;

use super::model::{
    DIARY_SCENE_RUNTIME_CATALOGUE_KIND, DIARY_SCENE_SOURCE_CATALOGUE_KIND,
    DiarySceneCatalogueMemberBinding, DiarySceneLocationPaletteRoles,
    DiarySceneRuntimeBundleBinding, DiarySceneRuntimeCatalogue, DiarySceneRuntimeMemberBinding,
    DiarySceneSourceCatalogue,
};
use super::source::{DIARY_SCENE_PATH, SOURCE_ARCHIVE_SHA256, SOURCE_MEMBER_COUNT};

const LOCATION_BLOCK: Cell = Cell {
    x: 0,
    y: 220,
    width: 128,
    height: 24,
};
const RUNTIME_BUNDLE_DIRECTORY: &str = "DAT2/";
const RUNTIME_BUNDLE_PREFIX: &str = "MGBG";
const RUNTIME_BUNDLE_SUFFIX: &str = ".BZZ";

#[derive(Debug, Clone)]
pub struct DiaryLocationGraphicsAuditConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug)]
pub struct DiaryLocationGraphicsAuditReport {
    pub catalogue_member_count: usize,
    pub location_block_member_count: usize,
    pub runtime_bundle_count: usize,
    pub runtime_member_count: usize,
    pub runtime_member_binding_count: usize,
    pub output_files: Vec<PathBuf>,
}

struct DecodedCatalogueMember {
    binding: DiarySceneCatalogueMemberBinding,
    bytes: Vec<u8>,
}

#[derive(Serialize)]
struct DiarySceneCataloguePreviewIndex {
    kind: String,
    source_archive_sha256: String,
    entries: Vec<DiarySceneCataloguePreview>,
}

#[derive(Serialize)]
struct DiarySceneCataloguePreview {
    member_index: usize,
    decoded_sha256: String,
    tim_pixel_size: [usize; 2],
    preview_file: String,
    preview_sha256: String,
}

pub fn audit_diary_location_graphics(
    config: &DiaryLocationGraphicsAuditConfig,
) -> Result<DiaryLocationGraphicsAuditReport> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let mut track = RawTrack::open(&cue.image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let files = iso.files()?;
    let catalogue_record = files
        .iter()
        .find(|(path, _)| path == DIARY_SCENE_PATH)
        .with_context(|| format!("missing {DIARY_SCENE_PATH}"))?;
    let catalogue_archive = iso.read_record(&catalogue_record.1)?;
    ensure!(
        sha256_bytes(&catalogue_archive) == SOURCE_ARCHIVE_SHA256,
        "MGBG.TZZ source hash changed"
    );
    let catalogue_members = decode_catalogue_members(&catalogue_archive)?;

    let mut runtime_records = files
        .into_iter()
        .filter(|(path, _)| is_diary_runtime_bundle(path))
        .collect::<Vec<_>>();
    runtime_records.sort_by(|left, right| left.0.cmp(&right.0));
    ensure!(
        !runtime_records.is_empty(),
        "no diary runtime bundles found"
    );

    let mut runtime_bundles = Vec::with_capacity(runtime_records.len());
    let mut runtime_member_count = 0usize;
    for (path, record) in runtime_records {
        let archive = iso
            .read_record(&record)
            .with_context(|| format!("failed to read {path}"))?;
        let members = parse_tzz(&archive)
            .with_context(|| format!("failed to parse runtime bundle {path}"))?;
        runtime_member_count += members.len();
        let mut bindings = Vec::with_capacity(members.len());
        for member in &members {
            let compressed = &archive[member.compressed_range()];
            let decoded = decompress(compressed, false).with_context(|| {
                format!("failed to decode runtime member {} in {path}", member.index)
            })?;
            let catalogue_member_indices = matching_catalogue_indices(
                &decoded,
                catalogue_members
                    .iter()
                    .map(|member| member.bytes.as_slice()),
            );
            ensure!(
                !catalogue_member_indices.is_empty(),
                "runtime member {} in {path} has no MGBG.TZZ prefix",
                member.index
            );
            bindings.push(DiarySceneRuntimeMemberBinding {
                index: member.index,
                offset: member.offset,
                compressed_size: member.compressed_size,
                compressed_sha256: sha256_bytes(compressed),
                decoded_size: decoded.len(),
                decoded_sha256: sha256_bytes(&decoded),
                catalogue_member_indices,
            });
        }
        runtime_bundles.push(DiarySceneRuntimeBundleBinding {
            path,
            archive_sha256: sha256_bytes(&archive),
            member_count: members.len(),
            members: bindings,
        });
    }

    let location_block_member_count = catalogue_members
        .iter()
        .filter(|member| member.binding.location_block_sha256.is_some())
        .count();
    let preview_output_files = write_catalogue_previews(&config.output_dir, &catalogue_members)?;
    let source_catalogue = DiarySceneSourceCatalogue {
        kind: DIARY_SCENE_SOURCE_CATALOGUE_KIND.to_string(),
        source_bin_sha256,
        path: DIARY_SCENE_PATH.to_string(),
        archive_sha256: SOURCE_ARCHIVE_SHA256.to_string(),
        location_block: LOCATION_BLOCK,
        members: catalogue_members
            .into_iter()
            .map(|member| member.binding)
            .collect(),
    };
    let runtime_bundle_count = runtime_bundles.len();
    let mut output_files =
        write_catalogues(&config.output_dir, &source_catalogue, runtime_bundles)?;
    output_files.extend(preview_output_files);
    Ok(DiaryLocationGraphicsAuditReport {
        catalogue_member_count: source_catalogue.members.len(),
        location_block_member_count,
        runtime_bundle_count,
        runtime_member_count,
        runtime_member_binding_count: runtime_member_count,
        output_files,
    })
}

fn write_catalogue_previews(
    output_dir: &Path,
    members: &[DecodedCatalogueMember],
) -> Result<Vec<PathBuf>> {
    const PREVIEW_INDEX_KIND: &str = "justice_gakuen2_diary_scene_catalogue_previews";
    const PREVIEW_DIRECTORY: &str = "previews";

    let preview_directory = output_dir.join(PREVIEW_DIRECTORY);
    std::fs::create_dir_all(&preview_directory)?;
    let mut entries = Vec::with_capacity(members.len());
    let mut output_files = Vec::with_capacity(members.len() + 1);
    for member in members {
        let tim = parse_embedded_tim_at(&member.bytes, 0).with_context(|| {
            format!(
                "failed to parse MGBG.TZZ member {} preview",
                member.binding.index
            )
        })?;
        let rgba = decode_embedded_tim_preview(&member.bytes, &tim).with_context(|| {
            format!(
                "failed to decode MGBG.TZZ member {} preview",
                member.binding.index
            )
        })?;
        let relative_file = format!("{PREVIEW_DIRECTORY}/member-{:03}.png", member.binding.index);
        let preview_file = output_dir.join(&relative_file);
        write_tim_preview(&preview_file, &rgba)?;
        entries.push(DiarySceneCataloguePreview {
            member_index: member.binding.index,
            decoded_sha256: member.binding.decoded_sha256.clone(),
            tim_pixel_size: member.binding.tim_pixel_size,
            preview_file: relative_file,
            preview_sha256: sha256_file(&preview_file)?,
        });
        output_files.push(preview_file);
    }

    let index = DiarySceneCataloguePreviewIndex {
        kind: PREVIEW_INDEX_KIND.to_string(),
        source_archive_sha256: SOURCE_ARCHIVE_SHA256.to_string(),
        entries,
    };
    let index_path = output_dir.join("catalogue-previews.json");
    write_json(&index_path, &index)?;
    output_files.push(index_path);
    Ok(output_files)
}

fn write_catalogues(
    output_dir: &Path,
    source: &DiarySceneSourceCatalogue,
    runtime_bundles: Vec<DiarySceneRuntimeBundleBinding>,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(output_dir)?;
    let mut output_files = Vec::new();
    let source_path = output_dir.join("catalogue.json");
    write_json(&source_path, source)?;
    output_files.push(source_path);

    let mut families = BTreeMap::<String, Vec<DiarySceneRuntimeBundleBinding>>::new();
    for bundle in runtime_bundles {
        let family = runtime_bundle_family(&bundle.path)?.to_ascii_lowercase();
        families.entry(family).or_default().push(bundle);
    }
    for (family, bundles) in families {
        let path = output_dir.join(format!("runtime-{family}.json"));
        let catalogue = DiarySceneRuntimeCatalogue {
            kind: DIARY_SCENE_RUNTIME_CATALOGUE_KIND.to_string(),
            family,
            bundles,
        };
        write_json(&path, &catalogue)?;
        output_files.push(path);
    }
    Ok(output_files)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn decode_catalogue_members(archive: &[u8]) -> Result<Vec<DecodedCatalogueMember>> {
    let members = parse_tzz(archive)?;
    ensure!(
        members.len() == SOURCE_MEMBER_COUNT,
        "MGBG.TZZ member count changed"
    );
    members
        .into_iter()
        .map(|member| {
            let compressed = &archive[member.compressed_range()];
            let decoded = decompress(compressed, false)
                .with_context(|| format!("failed to decode MGBG.TZZ member {}", member.index))?;
            let tim = parse_8bpp(&decoded)
                .with_context(|| format!("MGBG.TZZ member {} is not 8-bpp TIM", member.index))?;
            let location_pixels = (LOCATION_BLOCK.x + LOCATION_BLOCK.width <= tim.pixel_width()
                && LOCATION_BLOCK.y + LOCATION_BLOCK.height <= tim.image_height)
                .then(|| read_8bpp_indexed_cell(&decoded, LOCATION_BLOCK))
                .transpose()?;
            let location_block_sha256 = location_pixels.as_ref().map(|pixels| sha256_bytes(pixels));
            let location_palette_roles = location_pixels
                .as_ref()
                .map(|pixels| location_palette_roles(pixels, &read_first_8bpp_palette(&decoded)?))
                .transpose()?;
            Ok(DecodedCatalogueMember {
                binding: DiarySceneCatalogueMemberBinding {
                    index: member.index,
                    offset: member.offset,
                    compressed_size: member.compressed_size,
                    compressed_sha256: sha256_bytes(compressed),
                    decoded_size: decoded.len(),
                    decoded_sha256: sha256_bytes(&decoded),
                    tim_vram: [tim.image_x, tim.image_y],
                    tim_pixel_size: [tim.pixel_width(), tim.image_height],
                    clut_vram: [tim.clut_x, tim.clut_y],
                    clut_size: [tim.clut_width, tim.clut_height],
                    location_block_sha256,
                    location_palette_roles,
                },
                bytes: decoded,
            })
        })
        .collect()
}

fn read_first_8bpp_palette(data: &[u8]) -> Result<[u16; 256]> {
    const CLUT_DATA_OFFSET: usize = 20;
    let bytes = data
        .get(CLUT_DATA_OFFSET..CLUT_DATA_OFFSET + 256 * 2)
        .context("8-bpp TIM first palette is truncated")?;
    let mut palette = [0u16; 256];
    for (index, color) in palette.iter_mut().enumerate() {
        let offset = index * 2;
        *color = u16::from_le_bytes(bytes[offset..offset + 2].try_into()?);
    }
    Ok(palette)
}

pub(super) fn location_palette_roles(
    pixels: &[u8],
    palette: &[u16; 256],
) -> Result<DiarySceneLocationPaletteRoles> {
    let mut histogram = [0usize; 256];
    for pixel in pixels {
        histogram[usize::from(*pixel)] += 1;
    }
    let clear_index = histogram
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index as u8)
        .context("location block has no pixels")?;
    let used_opaque_indices = histogram
        .iter()
        .enumerate()
        .filter(|(index, count)| **count > 0 && *index != usize::from(clear_index))
        .map(|(index, _)| index as u8)
        .collect::<Vec<_>>();
    ensure!(
        used_opaque_indices.len() >= 2,
        "location block does not expose distinct outline and fill indices"
    );
    let outline_index = *used_opaque_indices
        .iter()
        .min_by_key(|index| (bgr555_luminance(palette[usize::from(**index)]), **index))
        .context("location block has no outline candidate")?;
    let fill_index = *used_opaque_indices
        .iter()
        .max_by_key(|index| (bgr555_luminance(palette[usize::from(**index)]), **index))
        .context("location block has no fill candidate")?;
    ensure!(
        outline_index != fill_index,
        "location block outline and fill palette roles collapsed"
    );
    Ok(DiarySceneLocationPaletteRoles {
        clear_index,
        outline_index,
        fill_index,
        clear_bgr555: palette[usize::from(clear_index)],
        outline_bgr555: palette[usize::from(outline_index)],
        fill_bgr555: palette[usize::from(fill_index)],
    })
}

fn bgr555_luminance(color: u16) -> u32 {
    let red = u32::from(color & 0x1f);
    let green = u32::from((color >> 5) & 0x1f);
    let blue = u32::from((color >> 10) & 0x1f);
    red * 299 + green * 587 + blue * 114
}

fn runtime_bundle_family(path: &str) -> Result<&str> {
    let name = path
        .strip_prefix(RUNTIME_BUNDLE_DIRECTORY)
        .context("runtime bundle is outside DAT2")?;
    ensure!(name.len() >= 5, "runtime bundle name is too short");
    Ok(&name[..5])
}

pub(super) fn is_diary_runtime_bundle(path: &str) -> bool {
    path.starts_with(RUNTIME_BUNDLE_DIRECTORY)
        && path
            .strip_prefix(RUNTIME_BUNDLE_DIRECTORY)
            .is_some_and(|name| name.starts_with(RUNTIME_BUNDLE_PREFIX))
        && path.ends_with(RUNTIME_BUNDLE_SUFFIX)
}

pub(super) fn matching_catalogue_indices<'a>(
    runtime_member: &[u8],
    catalogue_members: impl Iterator<Item = &'a [u8]>,
) -> Vec<usize> {
    catalogue_members
        .enumerate()
        .filter_map(|(index, member)| runtime_member.starts_with(member).then_some(index))
        .collect()
}
