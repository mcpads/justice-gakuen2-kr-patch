use anyhow::Result;

use crate::mode_select::source::{MENU_PATH, MODE_SELECT_OVERLAY_PATH};
use crate::pipeline::sha256_bytes;

use super::inventory::{collect_tim_inventory, summarize_tim_inventory};
use super::model::{
    ModeSelectGraphicsAuditConfig, ModeSelectGraphicsAuditReport, ModeSelectObservedMenuAudit,
};
use super::preview::write_tim_previews;
use super::source::{load_audit_inputs, path_string};

const REPORT_FILE: &str = "mode-select-graphics-audit.json";

pub fn audit_mode_select_graphics(
    config: &ModeSelectGraphicsAuditConfig,
) -> Result<ModeSelectGraphicsAuditReport> {
    let inputs = load_audit_inputs(config)?;
    let mut tims = collect_tim_inventory(
        &inputs.source.menu_decoded,
        inputs
            .observed
            .as_ref()
            .map(|observed| observed.decoded.as_slice()),
    )?;
    write_tim_previews(&inputs.source.menu_decoded, &mut tims, &inputs.output_dir)?;
    let summary = summarize_tim_inventory(&tims, inputs.observed.is_some());
    let observed_menu = inputs.observed.map(|observed| {
        let exact_source_tim_match_count = tims
            .iter()
            .filter(|tim| tim.observed_matches_source == Some(true))
            .count();
        ModeSelectObservedMenuAudit {
            cue_path: path_string(&observed.cue_path),
            bin_sha256: observed.bin_sha256,
            menu_stored_sha256: sha256_bytes(&observed.stored),
            menu_decoded_sha256: sha256_bytes(&observed.decoded),
            menu_decoded_size: observed.decoded.len(),
            exact_source_tim_match_count,
            changed_source_tim_count: tims.len() - exact_source_tim_match_count,
        }
    });
    let report = ModeSelectGraphicsAuditReport {
        kind: "Justice Gakuen 2 source-bound MODE SELECT graphics audit".to_string(),
        spec_path: path_string(&config.spec),
        spec_sha256: sha256_bytes(&inputs.spec_bytes),
        source_bin_sha256: inputs.source.source_bin_sha256,
        source_menu_path: MENU_PATH.to_string(),
        source_menu_stored_sha256: sha256_bytes(&inputs.source.menu_stored),
        source_menu_decoded_sha256: sha256_bytes(&inputs.source.menu_decoded),
        source_menu_decoded_size: inputs.source.menu_decoded.len(),
        source_overlay_path: MODE_SELECT_OVERLAY_PATH.to_string(),
        source_overlay_sha256: sha256_bytes(&inputs.source.overlay),
        observed_menu,
        embedded_tim_count: summary.embedded_tim_count,
        classified_tim_count: summary.classified_tim_count,
        unclassified_tim_count: summary.unclassified_tim_count,
        mode_artwork_count: summary.mode_artwork_count,
        mode_preview_count: summary.mode_preview_count,
        observed_matching_mode_preview_count: summary.observed_matching_mode_preview_count,
        tims,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    std::fs::write(inputs.output_dir.join(REPORT_FILE), report_bytes)?;
    Ok(report)
}
