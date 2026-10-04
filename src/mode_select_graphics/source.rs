use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::mode_select::source::{MENU_PATH, ModeSelectSource, load_source};
use crate::pipeline::sha256_file;

use super::model::{ModeSelectGraphicsAuditConfig, ModeSelectGraphicsAuditSpec};

const SPEC_KIND: &str = "justice_gakuen2_mode_select_graphics_audit";

pub(super) struct LoadedAuditInputs {
    pub(super) spec_bytes: Vec<u8>,
    pub(super) output_dir: PathBuf,
    pub(super) source: ModeSelectSource,
    pub(super) observed: Option<ObservedMenu>,
}

pub(super) struct ObservedMenu {
    pub(super) cue_path: PathBuf,
    pub(super) bin_sha256: String,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
}

pub(super) fn load_audit_inputs(
    config: &ModeSelectGraphicsAuditConfig,
) -> Result<LoadedAuditInputs> {
    let spec_bytes = std::fs::read(&config.spec)
        .with_context(|| format!("failed to read {}", config.spec.display()))?;
    let spec: ModeSelectGraphicsAuditSpec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("failed to parse {}", config.spec.display()))?;
    ensure!(
        spec.kind == SPEC_KIND,
        "unsupported MODE SELECT graphics audit kind"
    );
    let spec_dir = config.spec.parent().unwrap_or_else(|| Path::new("."));
    let source_cue = resolve_spec_path(spec_dir, spec.source_cue);
    let observed_cue = spec
        .observed_cue
        .map(|path| resolve_spec_path(spec_dir, path));
    let output_dir = resolve_spec_path(spec_dir, spec.output_dir);
    prepare_output(&output_dir, config.force)?;

    Ok(LoadedAuditInputs {
        spec_bytes,
        output_dir,
        source: load_source(&source_cue)?,
        observed: observed_cue
            .as_deref()
            .map(load_observed_menu)
            .transpose()?,
    })
}

fn load_observed_menu(cue_path: &Path) -> Result<ObservedMenu> {
    let cue = CueSheet::parse(cue_path)?;
    let bin_sha256 = sha256_file(&cue.image_path)?;
    let (_, stored) = rebuild::read_record(&cue.image_path, MENU_PATH)?;
    let decoded = decompress(&stored, true)?;
    Ok(ObservedMenu {
        cue_path: cue_path.to_path_buf(),
        bin_sha256,
        stored,
        decoded,
    })
}

fn resolve_spec_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn prepare_output(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() && !force {
        bail!("MODE SELECT graphics audit output exists; pass --force to replace it");
    }
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
