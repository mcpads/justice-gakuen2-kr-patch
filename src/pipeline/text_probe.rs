use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use super::{BASELINE_BIN_SHA256, difference_ranges, sha256_bytes, sha256_file};
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::text::{AtlasPosition, atlas_position, read_length_prefixed_codes, replace_code};

const TARGET_PATH: &str = "DAT1/NEWOPT.BIN";
const ORIGINAL_TARGET_SHA256: &str =
    "221f6eb284509304c4bf3c68c68eb944378f93026864bf33cf66a04aeca95187";
const ATTACK_LABEL_OFFSET: usize = 0x03c8;
const ATTACK_FIRST_CODE_OFFSET: usize = 0x03ca;
const ORIGINAL_CODE: u16 = 0x0218;
const REPLACEMENT_CODE: u16 = 0x0029;
const OUTPUT_STEM: &str = "justice-gakuen2-rust-text-code-probe";

#[derive(Debug, Clone)]
pub struct TextCodeProbeConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct CodeEditMetadata {
    pub string: String,
    pub glyph: String,
    pub replacement_glyph: String,
    pub string_offset: String,
    pub code_offset: String,
    pub original_code: String,
    pub replacement_code: String,
    pub original_atlas_position: AtlasPosition,
    pub replacement_atlas_position: AtlasPosition,
    pub expected_write_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
pub struct TextCodeProbeManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub output_bin_sha256: String,
    pub target_path: String,
    pub target_extent_lba: u32,
    pub target_sector_count: usize,
    pub changed_lbas: Vec<u32>,
    pub target_original_sha256: String,
    pub target_output_sha256: String,
    pub target_changed_byte_ranges: Vec<[usize; 2]>,
    pub edc_ecc_verified: bool,
    pub edit: CodeEditMetadata,
}

pub fn build_text_code_probe(config: &TextCodeProbeConfig) -> Result<TextCodeProbeManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_sha256}"
    );

    std::fs::create_dir_all(&config.output_dir)?;
    let output_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin"));
    let output_cue = config.output_dir.join(format!("{OUTPUT_STEM}.cue"));
    let output_manifest = config.output_dir.join(format!("{OUTPUT_STEM}.json"));
    let temporary_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin.tmp"));
    let outputs = [&output_bin, &output_cue, &output_manifest, &temporary_bin];
    if !config.force && outputs.iter().any(|path| path.exists()) {
        bail!("Rust text-code probe output exists; pass --force to replace it");
    }
    if config.force {
        for path in outputs {
            if path.exists() {
                std::fs::remove_file(path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }

    let result = build_to_temporary(
        &cue,
        &source_sha256,
        &temporary_bin,
        &output_bin,
        &output_cue,
        &output_manifest,
    );
    if result.is_err() && temporary_bin.exists() {
        let _ = std::fs::remove_file(&temporary_bin);
    }
    result
}

fn build_to_temporary(
    cue: &CueSheet,
    source_sha256: &str,
    temporary_bin: &Path,
    output_bin: &Path,
    output_cue: &Path,
    output_manifest: &Path,
) -> Result<TextCodeProbeManifest> {
    let (record, original) = rebuild::read_record(&cue.image_path, TARGET_PATH)?;
    let original_sha256 = sha256_bytes(&original);
    ensure!(
        original_sha256 == ORIGINAL_TARGET_SHA256,
        "NEWOPT.BIN identity changed: {original_sha256}"
    );
    ensure!(
        read_length_prefixed_codes(&original, ATTACK_LABEL_OFFSET)? == [0x0218, 0x0219, 0x0195],
        "expected 攻撃力 glyph-code sequence changed"
    );

    let mut changed = original.clone();
    replace_code(
        &mut changed,
        ATTACK_FIRST_CODE_OFFSET,
        ORIGINAL_CODE,
        REPLACEMENT_CODE,
    )?;
    let changed_ranges = difference_ranges(&original, &changed);
    let expected_write_ranges = vec![[ATTACK_FIRST_CODE_OFFSET, ATTACK_FIRST_CODE_OFFSET + 2]];
    ensure!(
        changed_ranges == expected_write_ranges,
        "NEWOPT.BIN edit escaped its Expected Write boundary"
    );

    let rebuilt =
        rebuild::copy_and_replace_record(&cue.image_path, temporary_bin, TARGET_PATH, &changed)?;
    ensure!(
        rebuilt.record == record,
        "target record changed between read and rebuild"
    );
    let (changed_lbas, output_sha256) =
        rebuild::compare_and_hash_images(&cue.image_path, temporary_bin)?;
    ensure!(!changed_lbas.is_empty(), "rebuilt image changed no sectors");
    ensure!(
        changed_lbas
            .iter()
            .all(|lba| rebuilt.target_lbas.contains(lba)),
        "rebuilt image changed a sector outside NEWOPT.BIN"
    );

    let manifest = TextCodeProbeManifest {
        kind: "controlled NEWOPT.BIN string-to-atlas glyph-code probe".to_string(),
        implementation: "independent Rust production pipeline".to_string(),
        source_bin_sha256: source_sha256.to_string(),
        output_bin_sha256: output_sha256,
        target_path: TARGET_PATH.to_string(),
        target_extent_lba: rebuilt.record.extent_lba,
        target_sector_count: rebuilt.target_lbas.len(),
        changed_lbas,
        target_original_sha256: original_sha256,
        target_output_sha256: sha256_bytes(&changed),
        target_changed_byte_ranges: changed_ranges,
        edc_ecc_verified: true,
        edit: CodeEditMetadata {
            string: "攻撃力".to_string(),
            glyph: "攻".to_string(),
            replacement_glyph: "X".to_string(),
            string_offset: format!("0x{ATTACK_LABEL_OFFSET:04x}"),
            code_offset: format!("0x{ATTACK_FIRST_CODE_OFFSET:04x}"),
            original_code: format!("0x{ORIGINAL_CODE:04x}"),
            replacement_code: format!("0x{REPLACEMENT_CODE:04x}"),
            original_atlas_position: atlas_position(ORIGINAL_CODE)?,
            replacement_atlas_position: atlas_position(REPLACEMENT_CODE)?,
            expected_write_ranges,
        },
    };

    std::fs::rename(temporary_bin, output_bin)?;
    std::fs::write(output_cue, cue.rewritten_for(output_cue, output_bin)?)?;
    std::fs::write(
        output_manifest,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(manifest)
}
