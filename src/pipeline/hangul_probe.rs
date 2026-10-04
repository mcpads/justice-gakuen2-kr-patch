use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

#[path = "hangul_probe/model.rs"]
mod model;

pub use model::{
    HangulProbeConfig, HangulProbeManifest, InstalledHangulGlyph, MenuAssetChange, TextAssetChange,
};

use super::{
    BASELINE_BIN_SHA256, EMBEDDED_MOJI2_TIM_SIZE, ORIGINAL_MENU_DECODED_SHA256, difference_ranges,
    hex_lower, sha256_bytes, sha256_file,
};
use crate::compression::{compress, decompress};
use crate::cue::CueSheet;
use crate::disc::rebuild::{self, FixedRecordReplacement};
use crate::font::rasterize_menu_glyphs;
use crate::menu_audit::audit_provisional_codes;
use crate::text::{atlas_position, read_length_prefixed_codes, replace_code};
use crate::tim::{Cell, install_indexed_glyph};

const MENU_PATH: &str = "DAT2/MENU.BIZ";
const NEWOPT_PATH: &str = "DAT1/NEWOPT.BIN";
const ORIGINAL_NEWOPT_SHA256: &str =
    "221f6eb284509304c4bf3c68c68eb944378f93026864bf33cf66a04aeca95187";
const ATTACK_LABEL_OFFSET: usize = 0x03c8;
const ATTACK_CODE_OFFSET: usize = 0x03ca;
const ORIGINAL_CODES: [u16; 3] = [0x0218, 0x0219, 0x0195];
const PROVISIONAL_CODES: [u16; 3] = [0x0047, 0x004a, 0x0069];
const HANGUL_LABEL: &str = "공격력";
const FONT_PX: f32 = 14.0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;
const OUTPUT_STEM: &str = "justice-gakuen2-rust-hangul-probe";

pub fn build_hangul_probe(config: &HangulProbeConfig) -> Result<HangulProbeManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    std::fs::create_dir_all(&config.output_dir)?;
    let output_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin"));
    let output_cue = config.output_dir.join(format!("{OUTPUT_STEM}.cue"));
    let output_manifest = config.output_dir.join(format!("{OUTPUT_STEM}.json"));
    let temporary_bin = config.output_dir.join(format!("{OUTPUT_STEM}.bin.tmp"));
    let outputs = [&output_bin, &output_cue, &output_manifest, &temporary_bin];
    if !config.force && outputs.iter().any(|path| path.exists()) {
        bail!("Rust Hangul probe output exists; pass --force to replace it");
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
        config,
        &cue,
        &source_bin_sha256,
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
    config: &HangulProbeConfig,
    cue: &CueSheet,
    source_bin_sha256: &str,
    temporary_bin: &Path,
    output_bin: &Path,
    output_cue: &Path,
    output_manifest: &Path,
) -> Result<HangulProbeManifest> {
    let provisional_code_evidence = audit_provisional_codes(&cue.image_path, &PROVISIONAL_CODES)?;
    ensure!(
        provisional_code_evidence.dat1_bin_files_scanned == 61
            && provisional_code_evidence.dat1_address_flow_seed_count == 26_126
            && provisional_code_evidence.dat1_address_flow_instruction_state_count == 4_517_380
            && provisional_code_evidence.dat1_address_flow_budget_exhausted_seed_count == 1_050
            && provisional_code_evidence.candidate_overlay_count == 52
            && provisional_code_evidence.candidate_string_count == 1_450
            && provisional_code_evidence.candidate_pointer_reference_count == 570
            && provisional_code_evidence.candidate_address_materialization_reference_count == 998
            && provisional_code_evidence.candidate_memory_access_reference_count == 1_093
            && provisional_code_evidence.candidate_loaded_word_reference_count == 1_057
            && provisional_code_evidence.candidate_shared_pointer_reference_count == 204
            && provisional_code_evidence.candidate_shared_address_materialization_reference_count
                == 0
            && provisional_code_evidence.candidate_shared_memory_access_reference_count == 0
            && provisional_code_evidence.candidate_shared_loaded_word_reference_count == 1
            && provisional_code_evidence.shared_reference_sources.len() == 1
            && provisional_code_evidence.shared_reference_sources[0].address_flow_seed_count
                == 9_487
            && provisional_code_evidence.shared_reference_sources[0]
                .address_flow_instruction_state_count
                == 4_103_557
            && provisional_code_evidence.shared_reference_sources[0]
                .address_flow_budget_exhausted_seed_count
                == 942
            && provisional_code_evidence.shared_reference_sources[0]
                .overlay_window_pointer_reference_count
                == 54
            && provisional_code_evidence.shared_reference_sources[0]
                .overlay_window_address_materialization_reference_count
                == 5
            && provisional_code_evidence.shared_reference_sources[0]
                .overlay_window_memory_access_reference_count
                == 20
            && provisional_code_evidence.shared_reference_sources[0]
                .overlay_window_loaded_word_reference_count
                == 9,
        "provisional code audit baseline changed"
    );
    ensure!(
        provisional_code_evidence.checks.iter().all(|check| {
            check.static_reference_overlays.is_empty()
                && check.wrapped_cell_overlap_codes.is_empty()
                && check.other_candidate_overlap_codes.is_empty()
        }),
        "provisional Hangul code is referenced or overlaps a referenced/candidate cell"
    );

    let rasterized = rasterize_menu_glyphs(
        &config.font,
        HANGUL_LABEL,
        FONT_PX,
        OUTLINE_INDEX,
        FILL_INDEX,
    )?;
    ensure!(
        rasterized.glyphs.len() == PROVISIONAL_CODES.len(),
        "Hangul glyph count changed"
    );

    let (menu_record, original_menu_stored) = rebuild::read_record(&cue.image_path, MENU_PATH)?;
    let original_menu_decoded = decompress(&original_menu_stored, false)?;
    let original_menu_decoded_sha256 = sha256_bytes(&original_menu_decoded);
    ensure!(
        original_menu_decoded_sha256 == ORIGINAL_MENU_DECODED_SHA256,
        "MENU.BIZ decoded identity changed: {original_menu_decoded_sha256}"
    );
    ensure!(
        original_menu_decoded.len() > EMBEDDED_MOJI2_TIM_SIZE,
        "MENU.BIZ lacks the embedded MOJI2 TIM prefix"
    );

    let mut changed_menu_decoded = original_menu_decoded.clone();
    let mut installed_glyphs = Vec::new();
    for (glyph, code) in rasterized.glyphs.into_iter().zip(PROVISIONAL_CODES) {
        let position = atlas_position(code)?;
        let cell = Cell {
            x: position.x,
            y: position.y,
            width: 20,
            height: 20,
        };
        let install = install_indexed_glyph(
            &mut changed_menu_decoded[..EMBEDDED_MOJI2_TIM_SIZE],
            cell,
            &glyph.pixels,
            &format!("provisional {code:#06x} glyph {:?}", glyph.character),
        )?;
        installed_glyphs.push(InstalledHangulGlyph {
            character: glyph.character,
            provisional_code: format!("0x{code:04x}"),
            atlas_position: position,
            font_fit: glyph.fit,
            install,
        });
    }
    let menu_decoded_changed_byte_ranges =
        difference_ranges(&original_menu_decoded, &changed_menu_decoded);
    ensure!(
        !menu_decoded_changed_byte_ranges.is_empty(),
        "Hangul glyph install changed no MENU.BIZ bytes"
    );
    ensure!(
        menu_decoded_changed_byte_ranges
            .iter()
            .all(|[start, end]| installed_glyphs
                .iter()
                .flat_map(|glyph| &glyph.install.allowed_decoded_byte_ranges)
                .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)),
        "Hangul glyph install escaped its Expected Write cells"
    );

    let reencoded_menu = compress(&changed_menu_decoded, 16)?;
    ensure!(
        decompress(&reencoded_menu, false)? == changed_menu_decoded,
        "Hangul MENU.BIZ encode/decode roundtrip failed"
    );
    ensure!(
        reencoded_menu.len() <= original_menu_stored.len(),
        "Hangul MENU.BIZ exceeds its original extent"
    );
    ensure!(
        reencoded_menu[..4] == original_menu_stored[..4],
        "Hangul MENU.BIZ changes the catalog first word"
    );
    let reencoded_menu_size = reencoded_menu.len();
    let mut padded_menu = reencoded_menu;
    padded_menu.resize(original_menu_stored.len(), 0);

    let (newopt_record, original_newopt) = rebuild::read_record(&cue.image_path, NEWOPT_PATH)?;
    let original_newopt_sha256 = sha256_bytes(&original_newopt);
    ensure!(
        original_newopt_sha256 == ORIGINAL_NEWOPT_SHA256,
        "NEWOPT.BIN identity changed: {original_newopt_sha256}"
    );
    ensure!(
        read_length_prefixed_codes(&original_newopt, ATTACK_LABEL_OFFSET)? == ORIGINAL_CODES,
        "expected 攻撃力 glyph-code sequence changed"
    );
    let mut changed_newopt = original_newopt.clone();
    for (index, (&original, &replacement)) in ORIGINAL_CODES
        .iter()
        .zip(PROVISIONAL_CODES.iter())
        .enumerate()
    {
        replace_code(
            &mut changed_newopt,
            ATTACK_CODE_OFFSET + index * 2,
            original,
            replacement,
        )?;
    }
    ensure!(
        read_length_prefixed_codes(&changed_newopt, ATTACK_LABEL_OFFSET)? == PROVISIONAL_CODES,
        "replacement 공격력 glyph-code sequence changed"
    );
    let newopt_changed_byte_ranges = difference_ranges(&original_newopt, &changed_newopt);
    ensure!(
        newopt_changed_byte_ranges
            == [[
                ATTACK_CODE_OFFSET,
                ATTACK_CODE_OFFSET + PROVISIONAL_CODES.len() * 2
            ]],
        "Hangul NEWOPT.BIN edit escaped its Expected Write boundary"
    );

    let replacements = [
        FixedRecordReplacement {
            path: MENU_PATH,
            data: &padded_menu,
        },
        FixedRecordReplacement {
            path: NEWOPT_PATH,
            data: &changed_newopt,
        },
    ];
    let rebuilt = rebuild::copy_and_replace_records(&cue.image_path, temporary_bin, &replacements)?;
    ensure!(
        rebuilt.len() == 2,
        "two-record rebuild returned wrong count"
    );
    ensure!(
        rebuilt[0].record == menu_record && rebuilt[1].record == newopt_record,
        "target record changed between read and rebuild"
    );

    let (_, rebuilt_menu_stored) = rebuild::read_record(temporary_bin, MENU_PATH)?;
    ensure!(
        decompress(&rebuilt_menu_stored, true)? == changed_menu_decoded,
        "rebuilt MENU.BIZ decodes to unexpected Hangul content"
    );
    let (_, rebuilt_newopt) = rebuild::read_record(temporary_bin, NEWOPT_PATH)?;
    ensure!(
        rebuilt_newopt == changed_newopt,
        "rebuilt NEWOPT.BIN differs from Hangul replacement"
    );

    let (changed_lbas, output_bin_sha256) =
        rebuild::compare_and_hash_images(&cue.image_path, temporary_bin)?;
    let allowed_lbas: Vec<_> = rebuilt
        .iter()
        .flat_map(|record| record.target_lbas.iter().copied())
        .collect();
    ensure!(!changed_lbas.is_empty(), "Hangul probe changed no sectors");
    ensure!(
        changed_lbas.iter().all(|lba| allowed_lbas.contains(lba)),
        "Hangul probe changed a sector outside MENU.BIZ or NEWOPT.BIN"
    );
    ensure!(
        changed_lbas
            .iter()
            .any(|lba| rebuilt[0].target_lbas.contains(lba))
            && changed_lbas
                .iter()
                .any(|lba| rebuilt[1].target_lbas.contains(lba)),
        "Hangul probe did not change both target records"
    );

    let font_file = config
        .font
        .file_name()
        .and_then(|name| name.to_str())
        .context("font file name is not UTF-8")?
        .to_string();
    let output_cue_name = output_cue
        .file_name()
        .and_then(|name| name.to_str())
        .context("output CUE file name is not UTF-8")?
        .to_string();
    let manifest = HangulProbeManifest {
        kind: "controlled Rust-only Maplestory Light 공격력 menu probe".to_string(),
        implementation: "independent Rust original-media font/text/disc pipeline".to_string(),
        source_bin_sha256: source_bin_sha256.to_string(),
        output_bin_sha256,
        output_cue: output_cue_name,
        changed_lbas,
        edc_ecc_verified: true,
        font_file,
        font_sha256: rasterized.font_sha256,
        font_px: FONT_PX,
        outline_palette_index: OUTLINE_INDEX,
        fill_palette_index: FILL_INDEX,
        original_label: "攻撃力".to_string(),
        replacement_label: HANGUL_LABEL.to_string(),
        allocation_status: "provisional PoC codes; not production-reclaimable slots".to_string(),
        provisional_code_evidence,
        glyphs: installed_glyphs,
        menu_asset: MenuAssetChange {
            path: MENU_PATH.to_string(),
            extent_lba: rebuilt[0].record.extent_lba,
            sector_count: rebuilt[0].target_lbas.len(),
            original_stored_sha256: sha256_bytes(&original_menu_stored),
            output_stored_sha256: sha256_bytes(&padded_menu),
            original_decoded_sha256: original_menu_decoded_sha256,
            output_decoded_sha256: sha256_bytes(&changed_menu_decoded),
            reencoded_size: reencoded_menu_size,
            padding_size: padded_menu.len() - reencoded_menu_size,
            catalog_first_word: hex_lower(&original_menu_stored[..4]),
            decoded_changed_byte_ranges: menu_decoded_changed_byte_ranges,
        },
        text_asset: TextAssetChange {
            path: NEWOPT_PATH.to_string(),
            extent_lba: rebuilt[1].record.extent_lba,
            sector_count: rebuilt[1].target_lbas.len(),
            original_sha256: original_newopt_sha256,
            output_sha256: sha256_bytes(&changed_newopt),
            label_offset: format!("0x{ATTACK_LABEL_OFFSET:04x}"),
            original_codes: ORIGINAL_CODES
                .into_iter()
                .map(|code| format!("0x{code:04x}"))
                .collect(),
            replacement_codes: PROVISIONAL_CODES
                .into_iter()
                .map(|code| format!("0x{code:04x}"))
                .collect(),
            changed_byte_ranges: newopt_changed_byte_ranges,
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
