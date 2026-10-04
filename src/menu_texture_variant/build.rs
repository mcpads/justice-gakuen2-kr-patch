use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::menu_compression::{compress_menu_with_source_limits, profile_menu_stream};
use crate::pipeline::{BASELINE_BIN_SHA256, difference_ranges, sha256_bytes, sha256_file};
use crate::tim::{
    read_indexed_cell_in_prefix, read_indexed_cell_without_clut_in_prefix,
    write_indexed_cell_in_prefix, write_indexed_cell_without_clut_in_prefix,
};

use super::model::{
    MenuGlyphReportRestoreBuild, MenuGlyphReportRestoreSpec, MenuTextureRepresentation,
    MenuTextureRestoreRegion, MenuTextureVariantBuildConfig, MenuTextureVariantBuildReport,
    MenuTextureVariantBuildSpec,
};

const SPEC_KIND: &str = "justice_gakuen2_menu_texture_variant";
const MENU_PATH: &str = "DAT2/MENU.BIZ";

pub fn build_menu_texture_variant(
    config: &MenuTextureVariantBuildConfig,
) -> Result<MenuTextureVariantBuildReport> {
    let spec_bytes = std::fs::read(&config.spec)
        .with_context(|| format!("failed to read {}", config.spec.display()))?;
    let spec: MenuTextureVariantBuildSpec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("failed to parse {}", config.spec.display()))?;
    ensure!(
        spec.kind == SPEC_KIND,
        "unsupported MENU texture variant kind"
    );
    ensure!(
        spec.source_bin_sha256 == BASELINE_BIN_SHA256,
        "MENU texture variant source identity is not the supported baseline"
    );
    let spec_dir = config.spec.parent().unwrap_or_else(|| Path::new("."));
    let mut restored_regions = spec.restored_regions.clone();
    let glyph_report = spec
        .glyph_report
        .as_ref()
        .map(|input| load_glyph_report_regions(spec_dir, input))
        .transpose()?;
    if let Some((regions, _)) = &glyph_report {
        restored_regions.extend(regions.iter().cloned());
    }
    validate_restore_regions(&restored_regions)?;

    let source_cue_path = resolve_spec_path(spec_dir, spec.source_cue);
    let input_menu_path = resolve_spec_path(spec_dir, spec.input_menu);
    let input_decoded_output_path = spec
        .input_decoded_output
        .map(|path| resolve_spec_path(spec_dir, path));
    let output_decoded_output_path = spec
        .output_decoded_output
        .map(|path| resolve_spec_path(spec_dir, path));
    let output_menu_path = resolve_spec_path(spec_dir, spec.output_menu);
    let report_path = output_menu_path.with_extension("menu-variant.json");
    prepare_outputs([&output_menu_path, &report_path], config.force)?;
    if let Some(path) = &input_decoded_output_path {
        ensure!(
            path != &output_menu_path && path != &report_path,
            "MENU texture variant input decoded output collides with another owned output"
        );
        prepare_outputs([path], config.force)?;
    }
    if let Some(path) = &output_decoded_output_path {
        ensure!(
            path != &output_menu_path && path != &report_path,
            "MENU texture variant output decoded file collides with another owned output"
        );
        ensure!(
            input_decoded_output_path.as_ref() != Some(path),
            "MENU texture variant decoded input and output files collide"
        );
        prepare_outputs([path], config.force)?;
    }

    let source_cue = CueSheet::parse(&source_cue_path)?;
    ensure!(
        sha256_file(&source_cue.image_path)? == spec.source_bin_sha256,
        "MENU texture variant source BIN identity changed"
    );
    let (_, source_menu) = rebuild::read_record(&source_cue.image_path, MENU_PATH)?;
    let source_decoded = decompress(&source_menu, false)?;
    let input_menu = std::fs::read(&input_menu_path)
        .with_context(|| format!("failed to read {}", input_menu_path.display()))?;
    ensure!(
        input_menu.len() == source_menu.len()
            && sha256_bytes(&input_menu) == spec.input_menu_sha256,
        "MENU texture variant input identity changed"
    );
    let input_compression = profile_menu_stream(&input_menu)?;
    let input_decoded = decompress(&input_menu, true)?;
    ensure!(
        input_decoded.len() == source_decoded.len(),
        "MENU texture variant decoded size changed"
    );
    if let Some(path) = &input_decoded_output_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &input_decoded)?;
    }

    let mut output_decoded = input_decoded.clone();
    for region in &restored_regions {
        let source_pixels = match region.representation {
            MenuTextureRepresentation::Indexed4bppWithClut => {
                read_indexed_cell_in_prefix(&source_decoded, region.tim_offset, region.cell)?
            }
            MenuTextureRepresentation::Indexed4bppWithoutClut => {
                read_indexed_cell_without_clut_in_prefix(
                    &source_decoded,
                    region.tim_offset,
                    region.cell,
                )?
            }
        };
        match region.representation {
            MenuTextureRepresentation::Indexed4bppWithClut => {
                write_indexed_cell_in_prefix(
                    &mut output_decoded,
                    region.tim_offset,
                    region.cell,
                    &source_pixels,
                )?;
            }
            MenuTextureRepresentation::Indexed4bppWithoutClut => {
                write_indexed_cell_without_clut_in_prefix(
                    &mut output_decoded,
                    region.tim_offset,
                    region.cell,
                    &source_pixels,
                )?;
            }
        }
    }
    let decoded_changed_byte_ranges = difference_ranges(&input_decoded, &output_decoded);
    ensure!(
        !decoded_changed_byte_ranges.is_empty(),
        "MENU texture variant restored no changed bytes"
    );
    if let Some(path) = &output_decoded_output_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &output_decoded)?;
    }
    let (mut output_menu, source_compression, output_compression) =
        compress_menu_with_source_limits(&output_decoded, &source_menu)?;
    ensure!(
        output_menu.len() <= source_menu.len(),
        "MENU texture variant exceeds its source record"
    );
    output_menu.resize(source_menu.len(), 0);
    ensure!(
        decompress(&output_menu, true)? == output_decoded,
        "MENU texture variant compression roundtrip changed decoded bytes"
    );
    if let Some(parent) = output_menu_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_menu_path, &output_menu)?;

    let report = MenuTextureVariantBuildReport {
        kind: "Justice Gakuen 2 non-release MENU texture variant".to_string(),
        variant_id: spec.variant_id,
        spec_path: path_string(&config.spec),
        spec_sha256: sha256_bytes(&spec_bytes),
        source_bin_sha256: spec.source_bin_sha256,
        source_menu_sha256: sha256_bytes(&source_menu),
        source_stream_byte_count: source_compression.stream_byte_count,
        source_control_blocks_crossing_input_pages: source_compression
            .control_blocks_crossing_input_pages,
        input_menu_path: path_string(&input_menu_path),
        input_menu_sha256: spec.input_menu_sha256,
        input_stream_byte_count: input_compression.stream_byte_count,
        input_control_blocks_crossing_input_pages: input_compression
            .control_blocks_crossing_input_pages,
        input_decoded_output_path: input_decoded_output_path.as_deref().map(path_string),
        input_decoded_sha256: input_decoded_output_path
            .as_ref()
            .map(|_| sha256_bytes(&input_decoded)),
        output_decoded_output_path: output_decoded_output_path.as_deref().map(path_string),
        output_decoded_sha256: output_decoded_output_path
            .as_ref()
            .map(|_| sha256_bytes(&output_decoded)),
        output_menu_path: path_string(&output_menu_path),
        output_menu_sha256: sha256_bytes(&output_menu),
        output_stream_byte_count: output_compression.stream_byte_count,
        output_control_blocks_crossing_input_pages: output_compression
            .control_blocks_crossing_input_pages,
        restored_region_count: restored_regions.len(),
        restored_roles: restored_regions
            .into_iter()
            .map(|region| region.role)
            .collect(),
        glyph_report: glyph_report.map(|(_, build)| build),
        decoded_changed_byte_ranges,
        compression_roundtrip_verified: true,
        release_eligible: false,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    std::fs::write(report_path, report_bytes)?;
    Ok(report)
}

#[derive(Debug, Deserialize)]
pub(super) struct MenuGlyphBuildReport {
    pub(super) glyphs: Vec<MenuGlyphBuildCell>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MenuGlyphBuildCell {
    pub(super) role: String,
    pub(super) code: String,
    pub(super) cell: crate::tim::Cell,
}

fn load_glyph_report_regions(
    spec_dir: &Path,
    input: &MenuGlyphReportRestoreSpec,
) -> Result<(Vec<MenuTextureRestoreRegion>, MenuGlyphReportRestoreBuild)> {
    let path = resolve_spec_path(spec_dir, input.path.clone());
    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        sha256_bytes(&bytes) == input.sha256,
        "MENU glyph restore report identity changed"
    );
    let report: MenuGlyphBuildReport = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let regions = select_glyph_report_regions(&report, input)?;
    let build = MenuGlyphReportRestoreBuild {
        path: path_string(&path),
        sha256: input.sha256.clone(),
        requested_roles: input.roles.clone(),
        requested_code_ranges: input.code_ranges.clone(),
        restored_region_count: regions.len(),
    };
    Ok((regions, build))
}

pub(super) fn select_glyph_report_regions(
    report: &MenuGlyphBuildReport,
    input: &MenuGlyphReportRestoreSpec,
) -> Result<Vec<MenuTextureRestoreRegion>> {
    ensure!(
        input.roles.iter().all(|role| !role.is_empty()),
        "MENU glyph restore report contains an empty role"
    );
    let requested_roles = input.roles.iter().collect::<BTreeSet<_>>();
    ensure!(
        requested_roles.len() == input.roles.len(),
        "MENU glyph restore report repeats a requested role"
    );
    if !requested_roles.is_empty() {
        let available_roles = report
            .glyphs
            .iter()
            .map(|glyph| &glyph.role)
            .collect::<BTreeSet<_>>();
        let missing = requested_roles
            .difference(&available_roles)
            .map(|role| role.as_str())
            .collect::<Vec<_>>();
        ensure!(
            missing.is_empty(),
            "MENU glyph restore report has no glyphs for roles: {}",
            missing.join(",")
        );
    }
    for range in &input.code_ranges {
        ensure!(
            range[0] < range[1],
            "MENU glyph restore report contains an empty code range"
        );
    }
    let regions = report
        .glyphs
        .iter()
        .map(|glyph| {
            let code = parse_glyph_code(&glyph.code)?;
            Ok((glyph, code))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(glyph, code)| {
            (requested_roles.is_empty() || requested_roles.contains(&glyph.role))
                && (input.code_ranges.is_empty()
                    || input
                        .code_ranges
                        .iter()
                        .any(|range| (range[0]..range[1]).contains(code)))
        })
        .map(|(glyph, _)| MenuTextureRestoreRegion {
            role: format!("{} glyph {}", glyph.role, glyph.code),
            tim_offset: input.tim_offset,
            cell: glyph.cell,
            representation: input.representation,
        })
        .collect::<Vec<_>>();
    ensure!(
        !regions.is_empty(),
        "MENU glyph restore report selected no glyphs"
    );
    Ok(regions)
}

fn parse_glyph_code(code: &str) -> Result<u16> {
    let digits = code
        .strip_prefix("0x")
        .context("MENU glyph restore report code is not hexadecimal")?;
    u16::from_str_radix(digits, 16).context("MENU glyph restore report code is invalid")
}

pub(super) fn validate_restore_regions(regions: &[MenuTextureRestoreRegion]) -> Result<()> {
    ensure!(
        !regions.is_empty(),
        "MENU texture variant has no restore regions"
    );
    let mut identities = BTreeSet::new();
    for region in regions {
        ensure!(
            !region.role.is_empty(),
            "MENU texture restore role is empty"
        );
        ensure!(
            region.cell.width > 0 && region.cell.height > 0,
            "MENU texture restore cell is empty"
        );
        ensure!(
            identities.insert((
                region.tim_offset,
                region.cell.x,
                region.cell.y,
                region.cell.width,
                region.cell.height,
            )),
            "MENU texture variant repeats a restore region"
        );
    }
    Ok(())
}

fn prepare_outputs<'a>(paths: impl IntoIterator<Item = &'a PathBuf>, force: bool) -> Result<()> {
    for path in paths {
        if path.exists() && !force {
            bail!("MENU texture variant output exists; pass --force to replace owned outputs");
        }
        if path.exists() {
            std::fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn resolve_spec_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
