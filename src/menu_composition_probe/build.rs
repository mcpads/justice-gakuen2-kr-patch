use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::menu_composition::{DecodedMenuChange, compose_disjoint_menu_changes};
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};

use super::model::{
    MenuCompositionContributorBuild, MenuCompositionContributorSpec,
    MenuCompositionProbeBuildConfig, MenuCompositionProbeBuildReport,
    MenuCompositionProbeBuildSpec, MenuContributorEncoding,
};

const SPEC_KIND: &str = "justice_gakuen2_menu_composition_probe";
const MENU_PATH: &str = "DAT2/MENU.BIZ";

pub fn build_menu_composition_probe(
    config: &MenuCompositionProbeBuildConfig,
) -> Result<MenuCompositionProbeBuildReport> {
    let spec_bytes = std::fs::read(&config.spec)
        .with_context(|| format!("failed to read {}", config.spec.display()))?;
    let spec: MenuCompositionProbeBuildSpec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("failed to parse {}", config.spec.display()))?;
    ensure!(
        spec.kind == SPEC_KIND,
        "unsupported MENU composition probe kind"
    );
    ensure!(
        spec.source_bin_sha256 == BASELINE_BIN_SHA256,
        "MENU composition probe source identity is not the supported baseline"
    );
    validate_contributors(&spec.contributors)?;

    let spec_dir = config.spec.parent().unwrap_or_else(|| Path::new("."));
    let source_cue_path = resolve_spec_path(spec_dir, spec.source_cue);
    let output_menu_path = resolve_spec_path(spec_dir, spec.output_menu);
    let output_report_path = output_menu_path.with_extension("menu-composition.json");
    let contributor_paths = spec
        .contributors
        .iter()
        .map(|contributor| resolve_spec_path(spec_dir, contributor.path.clone()))
        .collect::<Vec<_>>();
    ensure!(
        contributor_paths
            .iter()
            .all(|path| path != &output_menu_path),
        "MENU composition output collides with a contributor"
    );
    prepare_outputs([&output_menu_path, &output_report_path], config.force)?;

    let source_cue = CueSheet::parse(&source_cue_path)?;
    ensure!(
        sha256_file(&source_cue.image_path)? == spec.source_bin_sha256,
        "MENU composition probe source BIN identity changed"
    );
    let (_, source_menu) = rebuild::read_record(&source_cue.image_path, MENU_PATH)?;
    let source_decoded = decompress(&source_menu, false)?;

    let mut contributor_decoded = Vec::with_capacity(spec.contributors.len());
    for (contributor, path) in spec.contributors.iter().zip(&contributor_paths) {
        let input =
            std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        ensure!(
            sha256_bytes(&input) == contributor.sha256,
            "MENU composition contributor {} identity changed",
            contributor.role
        );
        let decoded = match contributor.encoding {
            MenuContributorEncoding::CompressedRecord => {
                ensure!(
                    input.len() == source_menu.len(),
                    "MENU composition contributor {} compressed record size changed",
                    contributor.role
                );
                decompress(&input, true)?
            }
            MenuContributorEncoding::DecodedImage => input,
        };
        ensure!(
            decoded.len() == source_decoded.len(),
            "MENU composition contributor {} decoded size changed",
            contributor.role
        );
        contributor_decoded.push(decoded);
    }

    let changes = spec
        .contributors
        .iter()
        .zip(&contributor_decoded)
        .map(|(contributor, decoded)| DecodedMenuChange {
            owner: &contributor.role,
            decoded,
        })
        .collect::<Vec<_>>();
    let composed = compose_disjoint_menu_changes(&source_decoded, &changes)?;
    let (mut output_menu, source_compression, output_compression) =
        compress_menu_with_source_limits(&composed.decoded, &source_menu)?;
    ensure!(
        output_menu.len() <= source_menu.len(),
        "MENU composition probe exceeds its source record"
    );
    output_menu.resize(source_menu.len(), 0);
    ensure!(
        decompress(&output_menu, true)? == composed.decoded,
        "MENU composition probe compression roundtrip changed decoded bytes"
    );
    if let Some(parent) = output_menu_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_menu_path, &output_menu)?;

    let contributors = spec
        .contributors
        .into_iter()
        .zip(contributor_paths)
        .zip(contributor_decoded)
        .zip(composed.contributions)
        .map(
            |(((contributor, path), decoded), contribution)| MenuCompositionContributorBuild {
                role: contributor.role,
                path: path_string(&path),
                input_sha256: contributor.sha256,
                encoding: match contributor.encoding {
                    MenuContributorEncoding::CompressedRecord => "compressed_record",
                    MenuContributorEncoding::DecodedImage => "decoded_image",
                }
                .to_string(),
                decoded_sha256: sha256_bytes(&decoded),
                changed_byte_ranges: contribution.changed_byte_ranges,
            },
        )
        .collect();
    let report = MenuCompositionProbeBuildReport {
        kind: "Justice Gakuen 2 non-release MENU composition probe".to_string(),
        probe_id: spec.probe_id,
        spec_path: path_string(&config.spec),
        spec_sha256: sha256_bytes(&spec_bytes),
        source_bin_sha256: spec.source_bin_sha256,
        source_menu_sha256: sha256_bytes(&source_menu),
        source_decoded_sha256: sha256_bytes(&source_decoded),
        source_stream_byte_count: source_compression.stream_byte_count,
        source_control_blocks_crossing_input_pages: source_compression
            .control_blocks_crossing_input_pages,
        output_menu_path: path_string(&output_menu_path),
        output_menu_sha256: sha256_bytes(&output_menu),
        output_decoded_sha256: sha256_bytes(&composed.decoded),
        output_stream_byte_count: output_compression.stream_byte_count,
        output_control_blocks_crossing_input_pages: output_compression
            .control_blocks_crossing_input_pages,
        changed_byte_ranges: composed.changed_byte_ranges,
        contributors,
        compression_roundtrip_verified: true,
        release_eligible: false,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    std::fs::write(output_report_path, report_bytes)?;
    Ok(report)
}

pub(super) fn validate_contributors(contributors: &[MenuCompositionContributorSpec]) -> Result<()> {
    ensure!(
        !contributors.is_empty(),
        "MENU composition probe has no contributors"
    );
    let mut roles = BTreeSet::new();
    for contributor in contributors {
        ensure!(
            !contributor.role.is_empty(),
            "MENU composition contributor role is empty"
        );
        ensure!(
            roles.insert(&contributor.role),
            "MENU composition contributor role repeats: {}",
            contributor.role
        );
    }
    Ok(())
}

fn prepare_outputs<'a>(paths: impl IntoIterator<Item = &'a PathBuf>, force: bool) -> Result<()> {
    for path in paths {
        if path.exists() && !force {
            bail!("MENU composition probe output exists; pass --force to replace owned outputs");
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
