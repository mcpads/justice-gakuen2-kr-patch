use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::embedded_tim::{decode_embedded_tim_preview, detect_embedded_tim_images};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::source_disc::profile::EDIT_REGISTRATION_OVERLAY_RECORD;
use crate::tim_preview::write_tim_preview;

use super::catalog::{
    PRACTICAL_1999_DESCRIPTOR_PATH, PRACTICAL_BASICS_DESCRIPTOR_PATH, practical_result_record_specs,
};
use super::edit_command_sheets::{EDIT_COMMAND_SHEETS_PATH, audit_edit_command_sheets};
use super::model::{
    ModeDescendantGraphicsAuditConfig, ModeDescendantGraphicsAuditReport,
    ModeDescendantGraphicsRecordAudit,
};
use super::practical_exam::{
    PracticalExamConsumer, audit_practical_exam_direct_texture_census,
    audit_practical_exam_secondary_descriptors,
};
use super::practical_results::PracticalResultPlan;
use super::source::{is_graphics_inventory_source, load_mode_descendant_audit_sources};
use super::texture_usage::collect_texture_usage;

const REPORT_FILE: &str = "mode-descendant-graphics-audit.json";

pub fn audit_mode_descendant_graphics(
    config: &ModeDescendantGraphicsAuditConfig,
) -> Result<ModeDescendantGraphicsAuditReport> {
    prepare_output(&config.output_dir, config.force)?;
    let (source_bin_sha256, sources) = load_mode_descendant_audit_sources(&config.cue)?;
    let practical_exam_consumers = [
        (
            PRACTICAL_BASICS_DESCRIPTOR_PATH,
            PracticalExamConsumer::BasicsReview,
        ),
        (
            PRACTICAL_1999_DESCRIPTOR_PATH,
            PracticalExamConsumer::Exam1999,
        ),
    ];
    let practical_exam_direct_texture_censuses = practical_exam_consumers
        .iter()
        .copied()
        .map(|(path, consumer)| {
            let source = sources
                .iter()
                .find(|source| source.path == path)
                .with_context(|| format!("practical-exam census source {path} was not loaded"))?;
            audit_practical_exam_direct_texture_census(&source.decoded, consumer, path)
        })
        .collect::<Result<Vec<_>>>()?;
    let practical_exam_secondary_descriptor_censuses = practical_exam_consumers
        .into_iter()
        .map(|(path, consumer)| {
            let source = sources
                .iter()
                .find(|source| source.path == path)
                .with_context(|| {
                    format!("practical-exam descriptor source {path} was not loaded")
                })?;
            audit_practical_exam_secondary_descriptors(&source.decoded, consumer, path)
        })
        .collect::<Result<Vec<_>>>()?;
    let practical_result_plan = PracticalResultPlan::load(&config.assets)?;
    let practical_results =
        practical_result_plan.assess_sources(practical_result_record_specs(), &sources)?;
    let mut records = Vec::with_capacity(sources.len());
    let mut tim_count = 0usize;
    let mut edit_command_sheets = None;
    let edit_command_sheet_consumer = sources
        .iter()
        .find(|source| source.path == EDIT_REGISTRATION_OVERLAY_RECORD.path)
        .context("mode-descendant audit did not load the EDIT command-sheet consumer")?;

    for source in sources
        .iter()
        .filter(|source| is_graphics_inventory_source(source.path))
    {
        let record_id = source
            .path
            .rsplit_once('/')
            .map(|(_, file)| {
                file.trim_end_matches(".BIZ")
                    .trim_end_matches(".TIZ")
                    .to_ascii_lowercase()
            })
            .context("mode-descendant source path lost its file name")?;
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
        if source.path == EDIT_COMMAND_SHEETS_PATH {
            edit_command_sheets = Some(audit_edit_command_sheets(
                source,
                edit_command_sheet_consumer,
                &tims,
            )?);
        }
        tim_count += tims.len();
        records.push(ModeDescendantGraphicsRecordAudit {
            role: source.role.to_string(),
            source_path: source.path.to_string(),
            storage_kind: source.storage_kind,
            source_extent_lba: source.extent_lba,
            source_stored_size: source.stored.len(),
            source_stored_sha256: sha256_bytes(&source.stored),
            source_decoded_size: source.decoded.len(),
            source_decoded_sha256: sha256_bytes(&source.decoded),
            source_allows_trailing_bytes: source.allows_trailing_bytes,
            tim_count: tims.len(),
            tims,
        });
    }

    let texture_usage = collect_texture_usage(&records)?;
    let unique_tim_count = texture_usage.len();
    let shared_tim_count = texture_usage
        .iter()
        .filter(|usage| usage.consumer_count > 1)
        .count();
    let report = ModeDescendantGraphicsAuditReport {
        kind: "justice_gakuen2_mode_descendant_graphics_audit".to_string(),
        source_bin_sha256,
        record_count: records.len(),
        tim_count,
        unique_tim_count,
        shared_tim_count,
        edit_command_sheets: edit_command_sheets
            .context("mode-descendant audit did not inspect EDIT command sheets")?,
        practical_exam_direct_texture_censuses,
        practical_exam_secondary_descriptor_censuses,
        practical_results,
        texture_usage,
        records,
    };
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    std::fs::write(config.output_dir.join(REPORT_FILE), bytes)?;
    Ok(report)
}

fn prepare_output(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() && !force {
        bail!("mode-descendant graphics audit output exists; pass --force to replace it");
    }
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}
