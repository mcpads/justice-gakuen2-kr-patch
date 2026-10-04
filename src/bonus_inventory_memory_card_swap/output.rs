use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::{sha256_file, write_pretty_json_and_hash};

use super::model::{
    BonusInventoryMemoryCardSwapBuildReport, BonusInventoryMemoryCardSwapFontSource,
};

pub const BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE: &str =
    "bonus-inventory-memory-card-swap-koubai2.bin";
pub const BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE: &str =
    "bonus-inventory-memory-card-swap-build.json";

pub(super) fn validate_font(font: &BonusInventoryMemoryCardSwapFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus memory-card-swap font settings are invalid"
    );
    Ok(())
}

pub(super) fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE,
        BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus memory-card-swap output exists; pass --force to replace it");
        }
    }
    Ok(())
}

pub(super) fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusInventoryMemoryCardSwapBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let report_path = output_dir.join(BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&report_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "bonus memory-card-swap overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
