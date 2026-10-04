use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::compression::{compress, decompress};
use crate::cue::CueSheet;
use crate::disc::rebuild;
pub(crate) use crate::file_digest::sha256_file;
use crate::source_disc::profile::{MENU_RECORD, SOURCE_BIN_SHA256};
use crate::tim::{EditMetadata, overlay_options_heading_o};
pub(crate) use crate::write_scope::difference_ranges;

pub mod hangul_probe;
mod ordered_parallel;
#[cfg(test)]
#[path = "pipeline/ordered_parallel_tests.rs"]
mod ordered_parallel_tests;
pub mod text_probe;
pub use hangul_probe::{HangulProbeConfig, HangulProbeManifest, build_hangul_probe};
pub(crate) use ordered_parallel::map_ordered_parallel;
pub use text_probe::{TextCodeProbeConfig, TextCodeProbeManifest, build_text_code_probe};

pub(crate) const BASELINE_BIN_SHA256: &str = SOURCE_BIN_SHA256;
pub(crate) const ORIGINAL_MENU_DECODED_SHA256: &str = MENU_RECORD.decoded_sha256;
const TARGET_PATH: &str = "DAT2/MENU.BIZ";
pub(crate) const EMBEDDED_MOJI2_TIM_SIZE: usize = 131_520;
const OUTPUT_STEM: &str = "justice-gakuen2-rust-glyph-probe";

#[derive(Debug, Clone)]
pub struct GlyphProbeConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct GlyphProbeManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub output_bin_sha256: String,
    pub target_path: String,
    pub target_extent_lba: u32,
    pub target_sector_count: usize,
    pub changed_lbas: Vec<u32>,
    pub stored_size: usize,
    pub reencoded_size: usize,
    pub padding_size: usize,
    pub catalog_first_word: String,
    pub decoded_original_sha256: String,
    pub decoded_output_sha256: String,
    pub decoded_changed_byte_count: usize,
    pub decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub edc_ecc_verified: bool,
    pub edit: EditMetadata,
}

pub fn build_glyph_probe(config: &GlyphProbeConfig) -> Result<GlyphProbeManifest> {
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
        bail!("Rust probe output exists; pass --force to replace it");
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
) -> Result<GlyphProbeManifest> {
    let (record, original_stored) = rebuild::read_record(&cue.image_path, TARGET_PATH)?;

    let original_decoded = decompress(&original_stored, false)?;
    let original_decoded_sha256 = sha256_bytes(&original_decoded);
    ensure!(
        original_decoded_sha256 == ORIGINAL_MENU_DECODED_SHA256,
        "MENU.BIZ decoded identity changed: {original_decoded_sha256}"
    );
    ensure!(
        original_decoded.len() > EMBEDDED_MOJI2_TIM_SIZE,
        "MENU.BIZ lacks the embedded MOJI2 TIM prefix"
    );

    let mut decoded = original_decoded.clone();
    let edit = overlay_options_heading_o(&mut decoded[..EMBEDDED_MOJI2_TIM_SIZE])?;
    let difference_ranges = difference_ranges(&original_decoded, &decoded);
    ensure!(
        difference_ranges.iter().all(|[start, end]| edit
            .allowed_decoded_byte_ranges
            .iter()
            .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)),
        "decoded edit escaped its Expected Write boundary"
    );
    let changed_byte_count: usize = difference_ranges.iter().map(|[a, b]| b - a).sum();
    ensure!(
        changed_byte_count == edit.changed_decoded_byte_count,
        "edit metadata changed-byte count drifted"
    );

    let reencoded = compress(&decoded, 16)?;
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "Rust LZ encode/decode roundtrip failed"
    );
    ensure!(
        reencoded.len() <= original_stored.len(),
        "re-encoded MENU.BIZ exceeds its original extent"
    );
    ensure!(
        reencoded[..4] == original_stored[..4],
        "re-encoded MENU.BIZ changes the catalog first word"
    );
    let mut padded = reencoded.clone();
    padded.resize(original_stored.len(), 0);

    let rebuild =
        rebuild::copy_and_replace_record(&cue.image_path, temporary_bin, TARGET_PATH, &padded)?;
    ensure!(
        rebuild.record == record,
        "target record changed between read and rebuild"
    );
    let (_, rebuilt_stored) = rebuild::read_record(temporary_bin, TARGET_PATH)?;
    ensure!(
        decompress(&rebuilt_stored, true)? == decoded,
        "rebuilt ISO record decodes to unexpected content"
    );

    let start_lba = rebuild.record.extent_lba;
    let sector_count = rebuild.target_lbas.len();
    let (changed_lbas, output_sha256) =
        rebuild::compare_and_hash_images(&cue.image_path, temporary_bin)?;
    ensure!(!changed_lbas.is_empty(), "rebuilt image changed no sectors");
    ensure!(
        changed_lbas
            .iter()
            .all(|lba| rebuild.target_lbas.contains(lba)),
        "rebuilt image changed a sector outside MENU.BIZ"
    );

    let manifest = GlyphProbeManifest {
        kind: "controlled MENU.BIZ embedded-MOJI2 single-cell glyph probe".to_string(),
        implementation: "independent Rust production pipeline".to_string(),
        source_bin_sha256: source_sha256.to_string(),
        output_bin_sha256: output_sha256,
        target_path: TARGET_PATH.to_string(),
        target_extent_lba: start_lba,
        target_sector_count: sector_count,
        changed_lbas,
        stored_size: original_stored.len(),
        reencoded_size: reencoded.len(),
        padding_size: padded.len() - reencoded.len(),
        catalog_first_word: hex_lower(&original_stored[..4]),
        decoded_original_sha256: original_decoded_sha256,
        decoded_output_sha256: sha256_bytes(&decoded),
        decoded_changed_byte_count: changed_byte_count,
        decoded_changed_byte_ranges: difference_ranges,
        edc_ecc_verified: true,
        edit,
    };

    std::fs::rename(temporary_bin, output_bin)?;
    std::fs::write(output_cue, cue.rewritten_for(output_cue, output_bin)?)?;
    std::fs::write(
        output_manifest,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(manifest)
}

pub(crate) fn sha256_bytes(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

pub(crate) fn write_pretty_json_and_hash<T: Serialize>(
    path: &Path,
    value: &T,
    trailing_newline: bool,
) -> Result<String> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    if trailing_newline {
        bytes.push(b'\n');
    }
    let sha256 = sha256_bytes(&bytes);
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(sha256)
}

pub(crate) fn hex_lower(data: &[u8]) -> String {
    let mut output = String::with_capacity(data.len() * 2);
    for byte in data {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
