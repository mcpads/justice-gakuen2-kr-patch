use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::pipeline::{sha256_bytes, sha256_file};

use super::model::{
    MenuRuntimeProbeBuildConfig, MenuRuntimeProbeBuildReport, MenuRuntimeProbeBuildSpec,
};

const SPEC_KIND: &str = "justice_gakuen2_menu_runtime_probe";
const MENU_PATH: &str = "DAT2/MENU.BIZ";

pub fn build_menu_runtime_probe(
    config: &MenuRuntimeProbeBuildConfig,
) -> Result<MenuRuntimeProbeBuildReport> {
    let spec_bytes = std::fs::read(&config.spec)
        .with_context(|| format!("failed to read {}", config.spec.display()))?;
    let spec: MenuRuntimeProbeBuildSpec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("failed to parse {}", config.spec.display()))?;
    ensure!(
        spec.kind == SPEC_KIND,
        "unsupported menu runtime probe kind"
    );
    validate_output_stem(&spec.output_stem)?;

    let spec_dir = config.spec.parent().unwrap_or_else(|| Path::new("."));
    let base_cue_path = resolve_spec_path(spec_dir, spec.base_cue);
    let replacement_menu_path = resolve_spec_path(spec_dir, spec.replacement_menu);
    let output_dir = resolve_spec_path(spec_dir, spec.output_dir);
    let base_cue = CueSheet::parse(&base_cue_path)?;
    ensure!(
        sha256_file(&base_cue.image_path)? == spec.base_bin_sha256,
        "menu runtime probe base BIN identity changed"
    );
    let replacement_menu = std::fs::read(&replacement_menu_path)
        .with_context(|| format!("failed to read {}", replacement_menu_path.display()))?;
    ensure!(
        sha256_bytes(&replacement_menu) == spec.replacement_menu_sha256,
        "menu runtime probe replacement MENU identity changed"
    );

    std::fs::create_dir_all(&output_dir)?;
    let output_bin = output_dir.join(format!("{}.bin", spec.output_stem));
    let output_cue = output_dir.join(format!("{}.cue", spec.output_stem));
    let output_report = output_dir.join(format!("{}.json", spec.output_stem));
    prepare_outputs([&output_bin, &output_cue, &output_report], config.force)?;

    let rebuild = rebuild::copy_and_replace_record(
        &base_cue.image_path,
        &output_bin,
        MENU_PATH,
        &replacement_menu,
    )?;
    let output_cue_text = base_cue.rewritten_for(&output_cue, &output_bin)?;
    std::fs::write(&output_cue, output_cue_text)?;
    let (_, readback) = rebuild::read_record(&output_bin, MENU_PATH)?;
    ensure!(
        readback == replacement_menu,
        "menu runtime probe readback changed the replacement MENU"
    );

    let report = MenuRuntimeProbeBuildReport {
        kind: "Justice Gakuen 2 non-release MENU consumer runtime probe".to_string(),
        probe_id: spec.probe_id,
        spec_path: path_string(&config.spec),
        spec_sha256: sha256_bytes(&spec_bytes),
        base_cue_path: path_string(&base_cue_path),
        base_bin_path: path_string(&base_cue.image_path),
        base_bin_sha256: spec.base_bin_sha256,
        replacement_record_path: MENU_PATH.to_string(),
        replacement_menu_path: path_string(&replacement_menu_path),
        replacement_menu_sha256: spec.replacement_menu_sha256,
        replacement_menu_size: replacement_menu.len(),
        changed_lbas: rebuild.changed_lbas,
        output_cue: path_string(&output_cue),
        output_bin: path_string(&output_bin),
        output_bin_sha256: sha256_file(&output_bin)?,
        readback_matches_replacement: true,
        release_eligible: false,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    std::fs::write(output_report, report_bytes)?;
    Ok(report)
}

pub(super) fn validate_output_stem(stem: &str) -> Result<()> {
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("menu runtime probe output_stem must use lowercase ASCII words and hyphens");
    }
    Ok(())
}

fn prepare_outputs<'a>(paths: impl IntoIterator<Item = &'a PathBuf>, force: bool) -> Result<()> {
    for path in paths {
        if path.exists() && !force {
            bail!("menu runtime probe output exists; pass --force to replace owned outputs");
        }
        if path.exists() {
            std::fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn resolve_spec_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
