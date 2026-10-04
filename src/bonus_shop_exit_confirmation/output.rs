use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::{sha256_file, write_pretty_json_and_hash};

use super::model::{BonusShopExitConfirmationBuildReport, BonusShopExitConfirmationFontSource};

pub const BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE: &str =
    "bonus-shop-exit-confirmation-koubai.bin";
pub const BONUS_SHOP_EXIT_CONFIRMATION_BUILD_MANIFEST_FILE: &str =
    "bonus-shop-exit-confirmation-build.json";

pub(super) fn validate_font(font: &BonusShopExitConfirmationFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "shop exit-confirmation font settings are invalid"
    );
    Ok(())
}

pub(super) fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE,
        BONUS_SHOP_EXIT_CONFIRMATION_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("shop exit-confirmation output exists; pass --force to replace it");
        }
    }
    Ok(())
}

pub(super) fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusShopExitConfirmationBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let report_path = output_dir.join(BONUS_SHOP_EXIT_CONFIRMATION_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&report_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "shop exit-confirmation overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
