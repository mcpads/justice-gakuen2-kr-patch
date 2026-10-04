use anyhow::{Context, Result, ensure};

#[path = "runtime_image_audit/model.rs"]
mod model;

pub use model::{RuntimeImageAuditConfig, RuntimeImageAuditReport, RuntimeImageRecordAudit};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::dialogue_audit::DECODED_IMAGE_SIZE;
use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};

const SELECTOR_TABLE_OFFSET: usize = 0x31000;
const SELECTOR_TABLE_PREFIX_SIZE: usize = 16;

pub fn audit_runtime_images(config: &RuntimeImageAuditConfig) -> Result<RuntimeImageAuditReport> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let mut track = RawTrack::open(&cue.image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let files = iso.files()?;
    let iso_file_count = files.len();
    let mut biz_records = files
        .into_iter()
        .filter(|(path, _)| path.ends_with(".BIZ"))
        .collect::<Vec<_>>();
    biz_records.sort_by(|left, right| left.0.cmp(&right.0));
    let stored_biz_record_count = biz_records.len();
    let mut records = Vec::with_capacity(stored_biz_record_count);

    for (path, record) in biz_records {
        let stored = iso
            .read_record(&record)
            .with_context(|| format!("failed to read {path}"))?;
        let stored_sha256 = sha256_bytes(&stored);
        let audit = match decompress(&stored, true) {
            Ok(decoded) => {
                let selector_table_prefix = selector_table_prefix(&decoded);
                RuntimeImageRecordAudit {
                    path,
                    extent_lba: record.extent_lba,
                    stored_size: stored.len(),
                    stored_sha256,
                    decode_succeeded: true,
                    decode_error: None,
                    decoded_size: Some(decoded.len()),
                    decoded_sha256: Some(sha256_bytes(&decoded)),
                    selector_table_prefix_offset: selector_table_prefix
                        .as_ref()
                        .map(|_| format!("0x{SELECTOR_TABLE_OFFSET:05x}")),
                    selector_table_prefix,
                    matches_dialogue_runtime_image_size: decoded.len() == DECODED_IMAGE_SIZE,
                }
            }
            Err(error) => RuntimeImageRecordAudit {
                path,
                extent_lba: record.extent_lba,
                stored_size: stored.len(),
                stored_sha256,
                decode_succeeded: false,
                decode_error: Some(error.to_string()),
                decoded_size: None,
                decoded_sha256: None,
                selector_table_prefix_offset: None,
                selector_table_prefix: None,
                matches_dialogue_runtime_image_size: false,
            },
        };
        records.push(audit);
    }

    let decoded_biz_record_count = records
        .iter()
        .filter(|record| record.decode_succeeded)
        .count();
    let selector_signature_record_count = records
        .iter()
        .filter(|record| record.selector_table_prefix.is_some())
        .count();
    let dialogue_runtime_image_paths = records
        .iter()
        .filter(|record| record.matches_dialogue_runtime_image_size)
        .map(|record| record.path.as_str())
        .collect::<Vec<_>>();
    let dialogue_runtime_image_record_count = dialogue_runtime_image_paths.len();
    let dialogue_runtime_image_path_manifest_sha256 =
        path_manifest_sha256(&dialogue_runtime_image_paths);
    let report = RuntimeImageAuditReport {
        kind: "runtime_image_audit".to_string(),
        source_bin_sha256,
        iso_file_count,
        stored_biz_record_count,
        decoded_biz_record_count,
        selector_signature_record_count,
        dialogue_runtime_image_record_count,
        dialogue_runtime_image_path_manifest_sha256,
        records,
    };
    write_report(config, &report)?;
    Ok(report)
}

fn write_report(config: &RuntimeImageAuditConfig, report: &RuntimeImageAuditReport) -> Result<()> {
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(&config.output, bytes)
        .with_context(|| format!("failed to write {}", config.output.display()))
}

fn selector_table_prefix(decoded: &[u8]) -> Option<String> {
    let end = SELECTOR_TABLE_OFFSET.checked_add(SELECTOR_TABLE_PREFIX_SIZE)?;
    decoded.get(SELECTOR_TABLE_OFFSET..end).map(hex_bytes)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn path_manifest_sha256(paths: &[&str]) -> String {
    let mut bytes = paths.join("\n").into_bytes();
    bytes.push(b'\n');
    sha256_bytes(&bytes)
}

#[cfg(test)]
#[path = "runtime_image_audit_tests.rs"]
mod tests;
