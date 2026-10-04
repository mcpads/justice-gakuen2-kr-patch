use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use super::build::build_diary_header;
use super::model::{
    DiaryHeaderBuildConfig, DiaryHeaderDiscBuildConfig, DiaryHeaderDiscBuildReport,
};
use super::source::DIARY_HEADER_PATH;
use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;

const OUTPUT_STEM: &str = "justice-gakuen2-diary-header-development";
const ASSET_OUTPUT_DIRECTORY: &str = "diary-header";

pub fn build_diary_header_disc(
    config: &DiaryHeaderDiscBuildConfig,
) -> Result<DiaryHeaderDiscBuildReport> {
    prepare_outputs(&config.output_dir, config.force)?;
    let cue = CueSheet::parse(&config.cue)?;
    let header = build_diary_header(&DiaryHeaderBuildConfig {
        cue: config.cue.clone(),
        assets: config.assets.clone(),
        fonts: config.fonts.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        output_dir: config.output_dir.join(ASSET_OUTPUT_DIRECTORY),
        force: config.force,
    })?;

    let output_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin"));
    let output_cue = config.output_dir.join(format!("{OUTPUT_STEM}.cue"));
    let output_report = config.output_dir.join(format!("{OUTPUT_STEM}.json"));
    let temporary_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin.tmp"));
    let (_, mgame_source) = rebuild::read_record(&cue.image_path, "DAT1/MGAME.BIN")?;
    let mgame_sha = crate::pipeline::sha256_bytes(&mgame_source);
    let mut plan = crate::decoded_record_write_plan::DecodedRecordWritePlan::new(
        "DAT1/MGAME.BIN",
        &mgame_source,
        &mgame_sha,
    )?;
    let mut machine = crate::psx_machine_code_sources::PsxMachineCodeSources::default();
    super::action_spacing::register(&mgame_source, &mgame_sha, &mut plan, &mut machine)?;
    let mgame = plan.apply(Some(&machine))?;
    let rebuild_result = rebuild::copy_and_replace_records(
        &cue.image_path,
        &temporary_bin,
        &[
            rebuild::FixedRecordReplacement {
                path: DIARY_HEADER_PATH,
                data: &header.stored,
            },
            rebuild::FixedRecordReplacement {
                path: "DAT1/MGAME.BIN",
                data: &mgame,
            },
        ],
    );
    let rebuilt = match rebuild_result {
        Ok(rebuilt) => rebuilt,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary_bin);
            return Err(error);
        }
    };

    ensure!(
        rebuild::read_record(&temporary_bin, "DAT1/MGAME.BIN")?.1 == mgame,
        "diary header action spacing readback changed"
    );
    let (_, readback) = rebuild::read_record(&temporary_bin, DIARY_HEADER_PATH)?;
    ensure!(
        readback == header.stored,
        "diary header development disc readback changed stored bytes"
    );
    ensure!(
        decompress(&readback, true)? == header.decoded,
        "diary header development disc readback changed decoded bytes"
    );
    let (changed_lbas, output_bin_sha256) =
        rebuild::compare_and_hash_images(&cue.image_path, &temporary_bin)?;
    let changes_confined_to_diary_header = !changed_lbas.is_empty()
        && changed_lbas.iter().all(|lba| {
            rebuilt
                .iter()
                .any(|record| record.target_lbas.contains(lba))
        });
    ensure!(
        changes_confined_to_diary_header,
        "diary header development disc changed a sector outside MGCOCK.TIZ and its MGAME renderer"
    );
    ensure!(
        changed_lbas.iter().any(|lba| rebuilt
            .iter()
            .any(|record| record.target_lbas.contains(lba))),
        "diary header development disc did not change MGCOCK.TIZ"
    );

    std::fs::rename(&temporary_bin, &output_bin)?;
    std::fs::write(&output_cue, cue.rewritten_for(&output_cue, &output_bin)?)?;
    let report = DiaryHeaderDiscBuildReport {
        kind: "Justice Gakuen 2 non-release diary-header development disc build".to_string(),
        output_bin_sha256,
        output_cue: output_cue
            .file_name()
            .and_then(|name| name.to_str())
            .context("diary header output CUE name is not UTF-8")?
            .to_string(),
        diary_header: header.report,
        changed_lbas,
        changes_confined_to_diary_header,
        readback_verified: true,
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
            bail!("diary header development output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}
