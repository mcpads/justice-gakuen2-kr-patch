use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::embedded_tim::{decode_embedded_tim_preview, detect_embedded_tim_images};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::source_disc::SupportedSourceDisc;

use super::model::{
    CharacterSelectAuxiliaryRecordAudit, CharacterSelectGraphicsAuditConfig,
    CharacterSelectGraphicsAuditReport, CharacterSelectRecordAudit,
    CharacterSelectRuntimeEvidenceAudit, CharacterSelectRuntimeEvidenceSpec,
    CharacterSelectRuntimeOverlayAudit,
};
use super::preview::write_tim_preview;
use super::source::{
    load_character_select_auxiliary_source_from_disc, load_character_select_source_from_disc,
};
use super::texture_targets::SHARED_ATLAS_OFFSET;

const REPORT_FILE: &str = "character-select-graphics-audit.json";
const PS1_RAM_BYTE_COUNT: usize = 2 * 1024 * 1024;
const OVERLAY_RAM_OFFSET: usize = 0x0a_2000;

pub fn audit_character_select_graphics(
    config: &CharacterSelectGraphicsAuditConfig,
) -> Result<CharacterSelectGraphicsAuditReport> {
    prepare_output(&config.output_dir, config.force)?;
    let source_disc = SupportedSourceDisc::open(&config.cue)?;
    let (source_bin_sha256, source_records) = load_character_select_source_from_disc(&source_disc)?;
    let auxiliary_source_records = load_character_select_auxiliary_source_from_disc(&source_disc)?;
    let runtime_evidence = load_runtime_evidence(config.runtime_evidence_spec.as_deref())?;
    let mut records = Vec::with_capacity(source_records.len());
    let mut tim_count = 0usize;

    for source in source_records {
        let record_id = source
            .path
            .rsplit_once('/')
            .map(|(_, file)| file.trim_end_matches(".BIZ").to_ascii_lowercase())
            .context("character-select source path lost its file name")?;
        let mut tims = detect_embedded_tim_images(&source.decoded);
        for (index, tim) in tims.iter_mut().enumerate() {
            let preview_file = format!(
                "{record_id}-tim-{index:02}-{:05x}-{}bpp.png",
                tim.offset, tim.bits_per_pixel
            );
            let preview_path = config.output_dir.join(&preview_file);
            let rgba = decode_embedded_tim_preview(&source.decoded, tim)?;
            write_tim_preview(&preview_path, &rgba)?;
            tim.preview_file = preview_file;
            tim.preview_sha256 = sha256_file(&preview_path)?;
        }
        tim_count += tims.len();
        let runtime_overlay = runtime_evidence
            .as_ref()
            .map(|evidence| compare_runtime_overlay(&source.overlay, &evidence.ram))
            .transpose()?;
        records.push(CharacterSelectRecordAudit {
            source_path: source.path.to_string(),
            source_extent_lba: source.extent_lba,
            source_stored_size: source.stored.len(),
            source_stored_sha256: sha256_bytes(&source.stored),
            source_decoded_size: source.decoded.len(),
            source_decoded_sha256: sha256_bytes(&source.decoded),
            overlay_path: source.overlay_path.to_string(),
            overlay_extent_lba: source.overlay_extent_lba,
            overlay_size: source.overlay.len(),
            overlay_sha256: sha256_bytes(&source.overlay),
            runtime_overlay,
            tim_count: tims.len(),
            tims,
        });
    }

    let mut auxiliary_records = Vec::with_capacity(auxiliary_source_records.len());
    let mut auxiliary_tim_count = 0usize;
    for source in auxiliary_source_records {
        let record_id = source
            .path
            .rsplit_once('/')
            .map(|(_, file)| {
                file.trim_end_matches(".BIZ")
                    .trim_end_matches(".TIZ")
                    .to_ascii_lowercase()
            })
            .context("character-select auxiliary source path lost its file name")?;
        let mut tims = detect_embedded_tim_images(&source.decoded);
        for (index, tim) in tims.iter_mut().enumerate() {
            let preview_file = format!(
                "{record_id}-tim-{index:02}-{:05x}-{}bpp.png",
                tim.offset, tim.bits_per_pixel
            );
            let preview_path = config.output_dir.join(&preview_file);
            let rgba = decode_embedded_tim_preview(&source.decoded, tim)?;
            write_tim_preview(&preview_path, &rgba)?;
            tim.preview_file = preview_file;
            tim.preview_sha256 = sha256_file(&preview_path)?;
        }
        auxiliary_tim_count += tims.len();
        auxiliary_records.push(CharacterSelectAuxiliaryRecordAudit {
            role: source.role.to_string(),
            source_path: source.path.to_string(),
            source_extent_lba: source.extent_lba,
            source_stored_size: source.stored.len(),
            source_stored_sha256: sha256_bytes(&source.stored),
            source_decoded_size: source.decoded.len(),
            source_decoded_sha256: sha256_bytes(&source.decoded),
            tim_count: tims.len(),
            tims,
        });
    }

    let shared_atlas_sha256 = records
        .first()
        .and_then(|record| {
            record
                .tims
                .iter()
                .find(|tim| tim.offset == SHARED_ATLAS_OFFSET)
        })
        .map(|tim| tim.source_tim_sha256.clone())
        .context("SELP1 shared 0x17800 atlas disappeared")?;
    let shared_atlas_identical_across_records = records.iter().all(|record| {
        record.tims.iter().any(|tim| {
            tim.offset == SHARED_ATLAS_OFFSET
                && tim.bits_per_pixel == 4
                && tim.pixel_width == 1024
                && tim.pixel_height == 256
                && tim.source_tim_sha256 == shared_atlas_sha256
        })
    });
    anyhow::ensure!(
        shared_atlas_identical_across_records,
        "SELP character-select records no longer share the 0x17800 atlas"
    );

    let runtime_evidence = runtime_evidence.map(|evidence| {
        let matching_overlay_paths = records
            .iter()
            .filter_map(|record| {
                record
                    .runtime_overlay
                    .as_ref()
                    .filter(|runtime| runtime.source_matches_resident)
                    .map(|_| record.overlay_path.clone())
            })
            .collect::<Vec<_>>();
        CharacterSelectRuntimeEvidenceAudit {
            spec_path: path_string(&evidence.spec_path),
            spec_sha256: evidence.spec_sha256,
            ram_dump_path: path_string(&evidence.ram_path),
            ram_dump_sha256: sha256_bytes(&evidence.ram),
            frame_path: path_string(&evidence.frame_path),
            frame_sha256: sha256_bytes(&evidence.frame),
            overlay_ram_offset: OVERLAY_RAM_OFFSET,
            exact_source_overlay_match_count: matching_overlay_paths.len(),
            matching_overlay_path: (matching_overlay_paths.len() == 1)
                .then(|| matching_overlay_paths[0].clone()),
        }
    });

    let report = CharacterSelectGraphicsAuditReport {
        kind: "justice_gakuen2_character_select_graphics_audit".to_string(),
        source_bin_sha256,
        record_count: records.len(),
        tim_count,
        auxiliary_record_count: auxiliary_records.len(),
        auxiliary_tim_count,
        shared_atlas_offset: SHARED_ATLAS_OFFSET,
        shared_atlas_sha256,
        shared_atlas_identical_across_records,
        runtime_evidence,
        records,
        auxiliary_records,
    };
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    std::fs::write(config.output_dir.join(REPORT_FILE), bytes)?;
    Ok(report)
}

struct LoadedRuntimeEvidence {
    spec_path: std::path::PathBuf,
    spec_sha256: String,
    ram_path: std::path::PathBuf,
    ram: Vec<u8>,
    frame_path: std::path::PathBuf,
    frame: Vec<u8>,
}

fn load_runtime_evidence(spec_path: Option<&Path>) -> Result<Option<LoadedRuntimeEvidence>> {
    let Some(spec_path) = spec_path else {
        return Ok(None);
    };
    let spec_bytes = std::fs::read(spec_path)
        .with_context(|| format!("failed to read {}", spec_path.display()))?;
    let spec: CharacterSelectRuntimeEvidenceSpec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("failed to parse {}", spec_path.display()))?;
    let base = spec_path.parent().unwrap_or_else(|| Path::new("."));
    let ram_path = resolve_spec_path(base, spec.ram_dump);
    let frame_path = resolve_spec_path(base, spec.frame);
    let ram = std::fs::read(&ram_path)
        .with_context(|| format!("failed to read {}", ram_path.display()))?;
    anyhow::ensure!(
        ram.len() == PS1_RAM_BYTE_COUNT,
        "runtime RAM dump is not a complete 2 MiB PS1 RAM image"
    );
    let frame = std::fs::read(&frame_path)
        .with_context(|| format!("failed to read {}", frame_path.display()))?;
    anyhow::ensure!(
        frame.starts_with(b"\x89PNG\r\n\x1a\n"),
        "paired runtime frame is not a PNG"
    );
    Ok(Some(LoadedRuntimeEvidence {
        spec_path: spec_path.to_path_buf(),
        spec_sha256: sha256_bytes(&spec_bytes),
        ram_path,
        ram,
        frame_path,
        frame,
    }))
}

fn resolve_spec_path(base: &Path, path: std::path::PathBuf) -> std::path::PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

pub(super) fn compare_runtime_overlay(
    source_overlay: &[u8],
    ram: &[u8],
) -> Result<CharacterSelectRuntimeOverlayAudit> {
    let end = OVERLAY_RAM_OFFSET
        .checked_add(source_overlay.len())
        .context("character-select runtime overlay range overflow")?;
    let resident = ram
        .get(OVERLAY_RAM_OFFSET..end)
        .context("character-select runtime overlay is outside the RAM dump")?;
    let matching_byte_count = source_overlay
        .iter()
        .zip(resident)
        .filter(|(source, runtime)| source == runtime)
        .count();
    let matching_prefix_length = source_overlay
        .iter()
        .zip(resident)
        .take_while(|(source, runtime)| source == runtime)
        .count();
    Ok(CharacterSelectRuntimeOverlayAudit {
        resident_sha256: sha256_bytes(resident),
        matching_byte_count,
        matching_prefix_length,
        source_matches_resident: source_overlay == resident,
    })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn prepare_output(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() && !force {
        bail!("character-select graphics audit output exists; pass --force to replace it");
    }
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}
