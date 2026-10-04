use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::pipeline::{sha256_bytes, sha256_file};

use super::dialogue_development_runtime_model::{
    DialogueDevelopmentRuntimeAuditConfig, DialogueDevelopmentRuntimeAuditReport,
};
use super::dialogue_disc_build_model::{
    DialogueDiscAssetReadback, DialogueDiscBuildReport, DialogueDiscBundleMemberReadback,
    DialogueDiscBundleReadback,
};
use super::parser::{DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE};

const PSX_RAM_BASE: u32 = 0x8000_0000;
const PSX_RAM_SIZE: usize = 0x20_0000;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug)]
pub(super) struct ValidatedRuntimeImageChain<'a> {
    pub(super) asset: &'a DialogueDiscAssetReadback,
    pub(super) bundle: &'a DialogueDiscBundleReadback,
    pub(super) member: &'a DialogueDiscBundleMemberReadback,
    pub(super) resident_sha256: String,
}

pub fn audit_dialogue_development_runtime(
    config: &DialogueDevelopmentRuntimeAuditConfig,
) -> Result<DialogueDevelopmentRuntimeAuditReport> {
    validate_emucap_identity(config)?;

    let disc_report_bytes = std::fs::read(&config.disc_report)
        .with_context(|| format!("failed to read {}", config.disc_report.display()))?;
    let disc_report: DialogueDiscBuildReport = serde_json::from_slice(&disc_report_bytes)
        .with_context(|| format!("failed to parse {}", config.disc_report.display()))?;
    ensure!(
        disc_report.all_records_read_back
            && (disc_report.changes_confined_to_replaced_records
                || disc_report.changes_confined_to_declared_records_and_metadata)
            && disc_report.edc_ecc_verified
            && disc_report.fixed_code_consumer_ownership_complete,
        "dialogue development disc report is not readback-complete"
    );

    let disc_bin_sha256 = sha256_file(&config.disc_bin)
        .with_context(|| format!("failed to hash {}", config.disc_bin.display()))?;
    ensure!(
        disc_bin_sha256 == disc_report.output_bin_sha256,
        "development BIN SHA-256 does not match its disc report"
    );

    let runtime_ram = std::fs::read(&config.runtime_ram)
        .with_context(|| format!("failed to read {}", config.runtime_ram.display()))?;
    ensure!(
        runtime_ram.len() == PSX_RAM_SIZE,
        "unexpected emucap RAM size: 0x{:x}",
        runtime_ram.len()
    );
    let resident_offset = usize::try_from(DECODED_RUNTIME_BASE - PSX_RAM_BASE)?;
    let resident_end = resident_offset + DECODED_IMAGE_SIZE;
    let resident = runtime_ram
        .get(resident_offset..resident_end)
        .context("emucap RAM lacks the complete dialogue runtime image")?;
    let chain = validate_runtime_image_chain(
        &disc_report.dialogue_assets,
        &disc_report.dialogue_bundles,
        &config.source_path,
        resident,
    )?;

    let runtime_frame = std::fs::read(&config.runtime_frame)
        .with_context(|| format!("failed to read {}", config.runtime_frame.display()))?;
    ensure!(
        runtime_frame.starts_with(PNG_SIGNATURE),
        "emucap runtime frame is not a PNG"
    );

    let report = DialogueDevelopmentRuntimeAuditReport {
        kind: "Justice Gakuen 2 dialogue development runtime audit".to_string(),
        source_path: config.source_path.clone(),
        launch_id: config.launch_id.clone(),
        emulator_build: config.emulator_build.clone(),
        capability_revision: config.capability_revision.clone(),
        disc_report_path: config.disc_report.display().to_string(),
        disc_report_sha256: sha256_bytes(&disc_report_bytes),
        disc_bin_path: config.disc_bin.display().to_string(),
        disc_bin_sha256,
        individual_record_extent_lba: chain.asset.extent_lba,
        individual_record_stored_sha256: chain.asset.readback_stored_sha256.clone(),
        decoded_asset_sha256: chain.asset.readback_decoded_sha256.clone(),
        bundle_path: chain.bundle.path.clone(),
        bundle_extent_lba: chain.bundle.extent_lba,
        bundle_readback_sha256: chain.bundle.readback_sha256.clone(),
        bundle_member_offset: chain.member.bundle_offset,
        bundle_member_byte_count: chain.member.byte_count,
        bundle_member_readback_sha256: chain.member.readback_replacement_sha256.clone(),
        runtime_ram_path: config.runtime_ram.display().to_string(),
        runtime_ram_sha256: sha256_bytes(&runtime_ram),
        runtime_ram_byte_count: runtime_ram.len(),
        resident_runtime_start: format!("0x{DECODED_RUNTIME_BASE:08x}"),
        resident_runtime_end: format!(
            "0x{:08x}",
            DECODED_RUNTIME_BASE + u32::try_from(DECODED_IMAGE_SIZE)?
        ),
        resident_runtime_sha256: chain.resident_sha256,
        runtime_frame_path: config.runtime_frame.display().to_string(),
        runtime_frame_sha256: sha256_bytes(&runtime_frame),
        runtime_frame_png_verified: true,
        disc_readback_chain_verified: true,
        runtime_residency_verified: true,
    };
    write_json(&config.output, &report)?;
    Ok(report)
}

fn validate_emucap_identity(config: &DialogueDevelopmentRuntimeAuditConfig) -> Result<()> {
    ensure!(
        config.launch_id.starts_with("launch-") && config.launch_id.len() > "launch-".len(),
        "emucap launch ID must begin with launch-"
    );
    ensure!(
        !config.emulator_build.trim().is_empty(),
        "emulator build identity must not be empty"
    );
    let revision = config
        .capability_revision
        .strip_prefix("sha256:")
        .context("emucap capability revision must use sha256:<digest>")?;
    ensure!(
        revision.len() == 64 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "emucap capability revision has an invalid SHA-256 digest"
    );
    Ok(())
}

pub(super) fn validate_runtime_image_chain<'a>(
    assets: &'a [DialogueDiscAssetReadback],
    bundles: &'a [DialogueDiscBundleReadback],
    source_path: &str,
    resident: &[u8],
) -> Result<ValidatedRuntimeImageChain<'a>> {
    let matching_assets: Vec<_> = assets
        .iter()
        .filter(|asset| asset.path == source_path)
        .collect();
    ensure!(
        matching_assets.len() == 1,
        "disc report must contain exactly one individual record for {source_path}"
    );
    let asset = matching_assets[0];
    ensure!(
        asset.readback_verified
            && asset.expected_stored_sha256 == asset.readback_stored_sha256
            && asset.expected_decoded_sha256 == asset.readback_decoded_sha256,
        "individual dialogue record readback is not verified for {source_path}"
    );

    let matching_members: Vec<_> = bundles
        .iter()
        .flat_map(|bundle| {
            bundle
                .members
                .iter()
                .filter(move |member| member.source_path == source_path)
                .map(move |member| (bundle, member))
        })
        .collect();
    ensure!(
        matching_members.len() == 1,
        "disc report must contain exactly one bundle member for {source_path}"
    );
    let (bundle, member) = matching_members[0];
    ensure!(
        bundle.readback_verified && bundle.expected_rebuilt_sha256 == bundle.readback_sha256,
        "dialogue bundle readback is not verified for {}",
        bundle.path
    );
    ensure!(
        member.readback_verified
            && member.expected_replacement_sha256 == member.readback_replacement_sha256,
        "dialogue bundle member readback is not verified for {source_path}"
    );
    ensure!(
        asset.readback_stored_sha256 == member.readback_replacement_sha256,
        "individual dialogue record and loaded bundle member differ for {source_path}"
    );

    let resident_sha256 = sha256_bytes(resident);
    ensure!(
        resident_sha256 == asset.readback_decoded_sha256,
        "emucap RAM dialogue image differs from disc readback for {source_path}"
    );
    Ok(ValidatedRuntimeImageChain {
        asset,
        bundle,
        member,
        resident_sha256,
    })
}

fn write_json(path: &Path, report: &DialogueDevelopmentRuntimeAuditReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
