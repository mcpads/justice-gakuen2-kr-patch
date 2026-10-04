use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::disc::rebuild::{self, FixedRecordReplacement};
use crate::menu_atlas::require_proven_shared_atlas_writes;
use crate::source_disc::MAIN_EXECUTABLE_PATH;

use super::description_source::OPTINFO_PATH;
use super::model::{OptionsBuildConfig, OptionsBuildReport};
use super::record_build::build_options_records;
use super::source::{MENU_PATH, OVERLAY_PATH};

const OUTPUT_STEM: &str = "justice-gakuen2-korean-options-development";

pub fn build_options_assets(config: &OptionsBuildConfig) -> Result<OptionsBuildReport> {
    prepare_outputs(&config.output_dir, config.force)?;
    let records = build_options_records(config)?;
    require_proven_shared_atlas_writes(
        "Options glyph",
        records
            .report
            .glyphs
            .iter()
            .filter(|glyph| glyph.global_menu_resident)
            .count(),
        records.report.allocation_proven_reclaimable,
    )?;
    let output_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin"));
    let output_cue = config.output_dir.join(format!("{OUTPUT_STEM}.cue"));
    let output_report = config.output_dir.join(format!("{OUTPUT_STEM}.json"));
    let temporary_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin.tmp"));
    let (menu_record, _) = rebuild::read_record(&records.cue.image_path, MENU_PATH)?;
    let (overlay_record, _) = rebuild::read_record(&records.cue.image_path, OVERLAY_PATH)?;
    let (optinfo_record, _) = rebuild::read_record(&records.cue.image_path, OPTINFO_PATH)?;
    let (main_executable_record, _) =
        rebuild::read_record(&records.cue.image_path, MAIN_EXECUTABLE_PATH)?;
    let replacements = [
        FixedRecordReplacement {
            path: MENU_PATH,
            data: &records.menu_stored,
        },
        FixedRecordReplacement {
            path: OVERLAY_PATH,
            data: &records.overlay,
        },
        FixedRecordReplacement {
            path: OPTINFO_PATH,
            data: &records.optinfo_stored,
        },
        FixedRecordReplacement {
            path: MAIN_EXECUTABLE_PATH,
            data: &records.main_executable,
        },
    ];
    let build_result =
        rebuild::copy_and_replace_records(&records.cue.image_path, &temporary_bin, &replacements);
    let rebuilt = match build_result {
        Ok(rebuilt) => rebuilt,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary_bin);
            return Err(error);
        }
    };
    ensure!(
        rebuilt.len() == 4
            && rebuilt[0].record == menu_record
            && rebuilt[1].record == overlay_record
            && rebuilt[2].record == optinfo_record
            && rebuilt[3].record == main_executable_record,
        "options disc rebuild changed its target records"
    );
    let (_, readback_menu) = rebuild::read_record(&temporary_bin, MENU_PATH)?;
    let (_, readback_overlay) = rebuild::read_record(&temporary_bin, OVERLAY_PATH)?;
    let (_, readback_optinfo) = rebuild::read_record(&temporary_bin, OPTINFO_PATH)?;
    let (_, readback_main_executable) = rebuild::read_record(&temporary_bin, MAIN_EXECUTABLE_PATH)?;
    ensure!(
        decompress(&readback_menu, true)? == records.menu_decoded,
        "options disc readback changed MENU.BIZ"
    );
    ensure!(
        readback_overlay == records.overlay,
        "options disc readback changed NEWOPT.BIN"
    );
    ensure!(
        decompress(&readback_optinfo, true)? == records.optinfo_decoded,
        "options disc readback changed OPTINFO.TIZ"
    );
    ensure!(
        readback_main_executable == records.main_executable,
        "options disc readback changed records-main SLPS bytes"
    );
    let (changed_lbas, output_bin_sha256) =
        rebuild::compare_and_hash_images(&records.cue.image_path, &temporary_bin)?;
    let allowed_lbas = rebuilt
        .iter()
        .flat_map(|record| record.target_lbas.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        !changed_lbas.is_empty() && changed_lbas.iter().all(|lba| allowed_lbas.contains(lba)),
        "options disc build changed a sector outside MENU.BIZ, NEWOPT.BIN, OPTINFO.TIZ, or SLPS_021.20"
    );
    ensure!(
        rebuilt.iter().all(|record| changed_lbas
            .iter()
            .any(|lba| record.target_lbas.contains(lba))),
        "options disc build did not change all four target records"
    );

    std::fs::rename(&temporary_bin, &output_bin)?;
    std::fs::write(
        &output_cue,
        records.cue.rewritten_for(&output_cue, &output_bin)?,
    )?;
    let report = OptionsBuildReport {
        kind: "Justice Gakuen 2 non-release development options-screen build".to_string(),
        output_bin_sha256,
        output_cue: output_cue
            .file_name()
            .and_then(|name| name.to_str())
            .context("options output CUE name is not UTF-8")?
            .to_string(),
        records: records.report,
        changed_lbas,
        edc_ecc_verified: true,
    };
    std::fs::write(
        &output_report,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(report)
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for extension in ["bin", "cue", "json", "bin.tmp"] {
        let path = output_dir.join(format!("{OUTPUT_STEM}.{extension}"));
        if path.exists() && !force {
            bail!("options development output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}
