use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::{sha256_file, write_pretty_json_and_hash};

use super::model::{BonusJBankReturnLabelBuildReport, BonusJBankReturnLabelFontSource};

pub const BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE: &str =
    "bonus-j-bank-return-label-koubai2.bin";
pub const BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE: &str =
    "bonus-j-bank-return-label-build.json";

pub(super) fn validate_font(font: &BonusJBankReturnLabelFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "J-BANK return-label font settings are invalid"
    );
    Ok(())
}

pub(super) fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE,
        BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("J-BANK return-label output exists; pass --force to replace it");
        }
    }
    Ok(())
}

pub(super) fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusJBankReturnLabelBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let report_path = output_dir.join(BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&report_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "J-BANK return-label overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
