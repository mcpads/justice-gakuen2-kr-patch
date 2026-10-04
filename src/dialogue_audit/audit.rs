use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};

use super::atlas::{ParsedDialogueAtlas, parse_dialogue_atlas};
use super::format::{hex_address, hex_code, hex_offset};
use super::model::{
    DialogueAssetAudit, DialogueAssetFontAudit, DialogueAuditConfig, DialogueAuditManifest,
    DialogueBankAudit, DialogueMessageAudit, DialogueTokenAudit,
};
use super::parser::{
    DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE, SELECTOR_SLOT_COUNT, SELECTOR_TABLE_OFFSET,
    parse_dialogue_banks,
};
use super::sources::load_dialogue_runtime_image_sources;
use super::summary::DialogueSummary;
use super::tokens::{DialogueTokenKind, tokenize_dialogue_message};

pub fn audit_dialogue_assets(config: &DialogueAuditConfig) -> Result<DialogueAuditManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let sources = load_dialogue_runtime_image_sources(&cue.image_path)?;
    let mut global_entries = BTreeSet::new();
    let mut assets = Vec::with_capacity(sources.len());
    let mut bank_count = 0usize;
    let mut entry_count = 0usize;
    let mut message_byte_count = 0usize;
    let mut summary = DialogueSummary::new();

    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        summary.record_atlas(&source.path, &atlas)?;
        let parsed_banks = parse_dialogue_banks(&decoded)
            .with_context(|| format!("failed to parse {} dialogue banks", source.path))?;

        let mut asset_entries = BTreeSet::new();
        let mut asset_entry_count = 0usize;
        let mut asset_message_byte_count = 0usize;
        let mut asset_fixed_glyph_occurrence_count = 0usize;
        let mut asset_runtime_extension_glyph_occurrence_count = 0usize;
        let mut asset_fixed_glyph_codes = BTreeSet::new();
        let mut asset_runtime_extension_glyph_codes = BTreeSet::new();
        let mut banks = Vec::with_capacity(parsed_banks.len());

        for bank in parsed_banks {
            let mut bank_entries = BTreeSet::new();
            let mut messages = Vec::with_capacity(bank.messages.len());
            for (index, message) in bank.messages.into_iter().enumerate() {
                let raw_words: Vec<_> = message
                    .data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect();
                let tokenized = tokenize_dialogue_message(
                    &raw_words,
                    atlas.fixed_cell_count,
                    atlas.addressable_slot_count,
                )
                .with_context(|| {
                    format!(
                        "failed to tokenize {} bank {} entry {index}",
                        source.path, bank.selector_index
                    )
                })?;
                summary.record_message(
                    &source.path,
                    &tokenized.tokens,
                    tokenized.alignment_padding_word_count,
                )?;
                record_asset_glyph_usage(
                    &tokenized.tokens,
                    &mut asset_fixed_glyph_occurrence_count,
                    &mut asset_runtime_extension_glyph_occurrence_count,
                    &mut asset_fixed_glyph_codes,
                    &mut asset_runtime_extension_glyph_codes,
                );

                let sha256 = sha256_bytes(&message.data);
                global_entries.insert(sha256.clone());
                asset_entries.insert(sha256.clone());
                bank_entries.insert(sha256.clone());
                asset_entry_count += 1;
                asset_message_byte_count += message.data.len();
                messages.push(DialogueMessageAudit {
                    index,
                    decoded_offset: hex_offset(message.decoded_offset),
                    runtime_address: hex_address(
                        DECODED_RUNTIME_BASE + message.decoded_offset as u32,
                    ),
                    byte_length: message.data.len(),
                    sha256,
                    raw_codes: raw_words.into_iter().map(hex_code).collect(),
                    alignment_padding_word_count: tokenized.alignment_padding_word_count,
                    tokens: tokenized
                        .tokens
                        .into_iter()
                        .map(|token| DialogueTokenAudit {
                            kind: token.kind.name().to_string(),
                            code: hex_code(token.code),
                            arguments: token.arguments.into_iter().map(hex_code).collect(),
                        })
                        .collect(),
                });
            }

            banks.push(DialogueBankAudit {
                selector_index: bank.selector_index,
                message_data_start: hex_offset(bank.message_data_start),
                message_data_end: hex_offset(bank.message_data_end),
                pointer_table_offset: hex_offset(bank.pointer_table_offset),
                pointer_table_runtime_address: hex_address(
                    DECODED_RUNTIME_BASE + bank.pointer_table_offset as u32,
                ),
                pointer_table_end: hex_offset(bank.pointer_table_end),
                entry_count: messages.len(),
                message_byte_count: bank.message_data_end - bank.message_data_start,
                unique_entry_count: bank_entries.len(),
                entries: messages,
            });
        }

        bank_count += banks.len();
        entry_count += asset_entry_count;
        message_byte_count += asset_message_byte_count;
        assets.push(DialogueAssetAudit {
            path: source.path,
            extent_lba: source.extent_lba,
            stored_size: source.data.len(),
            stored_sha256: sha256_bytes(&source.data),
            decoded_size: decoded.len(),
            decoded_sha256: sha256_bytes(&decoded),
            bank_count: banks.len(),
            entry_count: asset_entry_count,
            message_byte_count: asset_message_byte_count,
            unique_entry_count: asset_entries.len(),
            font: asset_font_audit(
                &atlas,
                asset_fixed_glyph_codes.len(),
                asset_runtime_extension_glyph_codes.len(),
                asset_fixed_glyph_occurrence_count,
                asset_runtime_extension_glyph_occurrence_count,
            ),
            banks,
        });
    }

    let summary = summary.finish(assets.len())?;
    let manifest = DialogueAuditManifest {
        kind: "Justice Gakuen 2 dialogue runtime-image pointer-table population audit"
            .to_string(),
        implementation: "independent Rust original-media analysis".to_string(),
        source_bin_sha256,
        storage_format: "confirmed 16-bit LZ stream containing a fixed-size runtime image"
            .to_string(),
        decoded_runtime_base: hex_address(DECODED_RUNTIME_BASE),
        decoded_image_size: DECODED_IMAGE_SIZE,
        selector_table_offset: hex_offset(SELECTOR_TABLE_OFFSET),
        selector_slot_count: SELECTOR_SLOT_COUNT,
        asset_count: assets.len(),
        bank_count,
        entry_count,
        message_byte_count,
        unique_entry_count: global_entries.len(),
        token_count: summary.token_count,
        glyph_occurrence_count: summary.glyph_occurrence_count,
        fixed_glyph_occurrence_count: summary.fixed_glyph_occurrence_count,
        runtime_extension_glyph_occurrence_count: summary
            .runtime_extension_glyph_occurrence_count,
        alignment_padding_word_count: summary.alignment_padding_word_count,
        font: summary.font,
        token_codes: summary.token_codes,
        assets,
        limitations: vec![
            "This structural audit intentionally retains glyph codes without decoding them; the separate source-corpus command joins the reviewed codebook."
                .to_string(),
            "Identical raw entries are counted separately by source coordinate and additionally summarized by SHA-256; translation approval must preserve every coordinate or an explicit shared-entry relation."
                .to_string(),
            "The manifest records static source facts. MGK04 and MGG04T decoded-image-to-RAM identities and their visible consumer paths are separate emucap evidence."
                .to_string(),
        ],
    };

    write_manifest(config, &manifest)?;
    Ok(manifest)
}

fn record_asset_glyph_usage(
    tokens: &[super::tokens::ParsedDialogueToken],
    fixed_glyph_occurrence_count: &mut usize,
    runtime_extension_glyph_occurrence_count: &mut usize,
    fixed_glyph_codes: &mut BTreeSet<u16>,
    runtime_extension_glyph_codes: &mut BTreeSet<u16>,
) {
    for token in tokens {
        match token.kind {
            DialogueTokenKind::FixedGlyph => {
                *fixed_glyph_occurrence_count += 1;
                fixed_glyph_codes.insert(token.code);
            }
            DialogueTokenKind::RuntimeExtensionGlyph => {
                *runtime_extension_glyph_occurrence_count += 1;
                runtime_extension_glyph_codes.insert(token.code);
            }
            _ => {}
        }
    }
}

fn asset_font_audit(
    atlas: &ParsedDialogueAtlas,
    used_fixed_cell_count: usize,
    used_runtime_extension_slot_count: usize,
    fixed_glyph_occurrence_count: usize,
    runtime_extension_glyph_occurrence_count: usize,
) -> DialogueAssetFontAudit {
    DialogueAssetFontAudit {
        tim_size: atlas.tim_size,
        clut_vram_x: atlas.clut_vram_x,
        clut_vram_y: atlas.clut_vram_y,
        clut_color_count: atlas.clut_color_count,
        image_vram_x: atlas.image_vram_x,
        image_vram_y: atlas.image_vram_y,
        pixel_data_offset: hex_offset(atlas.pixel_data_offset),
        fixed_cell_count: atlas.fixed_cell_count,
        addressable_slot_count_before_selector: atlas.addressable_slot_count,
        runtime_extension_slot_count: atlas.addressable_slot_count - atlas.fixed_cell_count,
        used_fixed_cell_count,
        used_runtime_extension_slot_count,
        fixed_glyph_occurrence_count,
        runtime_extension_glyph_occurrence_count,
        fixed_atlas_sha256: atlas.fixed_atlas_sha256.clone(),
        source_extension_start: hex_offset(atlas.source_extension_start),
        source_extension_end: hex_offset(atlas.source_extension_end),
        source_extension_sha256: atlas.source_extension_sha256.clone(),
        source_extension_nonzero_byte_count: atlas.source_extension_nonzero_byte_count,
    }
}

fn write_manifest(config: &DialogueAuditConfig, manifest: &DialogueAuditManifest) -> Result<()> {
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(manifest)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))
}
