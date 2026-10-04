use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;

use super::atlas::parse_dialogue_atlas;
use super::codebook_model::ResolvedDialogueGlyph;
use super::format::{hex_code, hex_offset};
use super::parser::parse_dialogue_banks;
use super::review_model::DialogueCodebookReviewContext;
use super::runtime_insertions::dialogue_control_spec;
use super::sources::load_dialogue_runtime_image_sources;
use super::tokens::{DialogueTokenKind, ParsedDialogueToken, tokenize_dialogue_message};

pub(super) const MAXIMUM_CONTEXTS_PER_GLYPH: usize = 8;
pub(super) const CONTEXT_TOKEN_RADIUS: usize = 24;

pub(super) fn collect_review_contexts(
    image_path: &Path,
    resolved_glyphs: &BTreeMap<String, ResolvedDialogueGlyph>,
    unresolved_pixel_hashes: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<DialogueCodebookReviewContext>>> {
    let mut candidates = BTreeMap::<String, Vec<DialogueCodebookReviewContext>>::new();

    for source in load_dialogue_runtime_image_sources(image_path)? {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        for bank in parse_dialogue_banks(&decoded)? {
            for (entry_index, message) in bank.messages.into_iter().enumerate() {
                let raw_words = message
                    .data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>();
                let tokenized = tokenize_dialogue_message(
                    &raw_words,
                    atlas.fixed_cell_count,
                    atlas.addressable_slot_count,
                )?;
                let targets = target_positions(
                    &tokenized.tokens,
                    &atlas.fixed_cell_sha256,
                    unresolved_pixel_hashes,
                )?;
                for (pixel_sha256, positions) in targets {
                    let (target_token_index, target_code) = positions[0];
                    candidates.entry(pixel_sha256.clone()).or_default().push(
                        DialogueCodebookReviewContext {
                            coordinate_id: format!(
                                "{}#bank-{}-entry-{entry_index:04}",
                                source.path, bank.selector_index
                            ),
                            source_asset: source.path.clone(),
                            bank_index: bank.selector_index,
                            entry_index,
                            decoded_offset: hex_offset(message.decoded_offset),
                            target_code: hex_code(target_code),
                            target_token_index,
                            target_occurrence_count: positions.len(),
                            context_markup: render_context_markup(
                                &tokenized.tokens,
                                &atlas.fixed_cell_sha256,
                                resolved_glyphs,
                                &pixel_sha256,
                                target_token_index,
                            )?,
                        },
                    );
                }
            }
        }
    }

    let mut bounded = BTreeMap::new();
    for pixel_sha256 in unresolved_pixel_hashes {
        let contexts = candidates
            .remove(pixel_sha256)
            .with_context(|| format!("unresolved pixel {pixel_sha256} has no message context"))?;
        bounded.insert(pixel_sha256.clone(), select_diverse_contexts(contexts));
    }
    ensure!(
        candidates.is_empty(),
        "review contexts contain an unexpected unresolved pixel"
    );
    Ok(bounded)
}

fn target_positions(
    tokens: &[ParsedDialogueToken],
    fixed_cell_sha256: &[String],
    unresolved_pixel_hashes: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<(usize, u16)>>> {
    let mut targets = BTreeMap::<String, Vec<(usize, u16)>>::new();
    for (token_index, token) in tokens.iter().enumerate() {
        if token.kind != DialogueTokenKind::FixedGlyph {
            continue;
        }
        let pixel_sha256 = fixed_cell_sha256
            .get(usize::from(token.code))
            .context("fixed dialogue token has no source pixel")?;
        if unresolved_pixel_hashes.contains(pixel_sha256) {
            targets
                .entry(pixel_sha256.clone())
                .or_default()
                .push((token_index, token.code));
        }
    }
    Ok(targets)
}

fn select_diverse_contexts(
    contexts: Vec<DialogueCodebookReviewContext>,
) -> Vec<DialogueCodebookReviewContext> {
    let mut selected = Vec::new();
    let mut selected_coordinates = BTreeSet::new();
    let mut selected_assets = BTreeSet::new();

    for context in &contexts {
        if selected.len() == MAXIMUM_CONTEXTS_PER_GLYPH {
            break;
        }
        if selected_assets.insert(context.source_asset.clone()) {
            selected_coordinates.insert(context.coordinate_id.clone());
            selected.push(context.clone());
        }
    }
    for context in contexts {
        if selected.len() == MAXIMUM_CONTEXTS_PER_GLYPH {
            break;
        }
        if selected_coordinates.insert(context.coordinate_id.clone()) {
            selected.push(context);
        }
    }
    selected
}

fn render_context_markup(
    tokens: &[ParsedDialogueToken],
    fixed_cell_sha256: &[String],
    resolved_glyphs: &BTreeMap<String, ResolvedDialogueGlyph>,
    target_pixel_sha256: &str,
    target_token_index: usize,
) -> Result<String> {
    let start = target_token_index.saturating_sub(CONTEXT_TOKEN_RADIUS);
    let end = tokens
        .len()
        .min(target_token_index + CONTEXT_TOKEN_RADIUS + 1);
    let mut output = String::new();
    if start != 0 {
        output.push_str("[…]");
    }
    for token in &tokens[start..end] {
        match token.kind {
            DialogueTokenKind::FixedGlyph => {
                let pixel_sha256 = fixed_cell_sha256
                    .get(usize::from(token.code))
                    .context("fixed dialogue token has no source pixel")?;
                if pixel_sha256 == target_pixel_sha256 {
                    output.push_str("⟦TARGET⟧");
                } else if let Some(glyph) = resolved_glyphs.get(pixel_sha256) {
                    output.push_str(&glyph.text);
                } else {
                    output.push_str("⟦?");
                    output.push_str(&pixel_sha256[..8]);
                    output.push('⟧');
                }
            }
            DialogueTokenKind::LineBreak => output.push('\n'),
            DialogueTokenKind::MessageEnd => output.push_str("{#message_end}"),
            DialogueTokenKind::RuntimeExtensionGlyph => {
                output.push_str("{#runtime_extension:");
                output.push_str(&hex_code(token.code));
                output.push('}');
            }
            _ => {
                let spec = dialogue_control_spec(token.code)
                    .context("dialogue review context has an unknown control")?;
                output.push_str("{#");
                output.push_str(spec.semantic_name);
                for argument in &token.arguments {
                    output.push(':');
                    output.push_str(&hex_code(*argument));
                }
                output.push('}');
            }
        }
    }
    if end != tokens.len() {
        output.push_str("[…]");
    }
    Ok(output)
}

#[cfg(test)]
#[path = "review_context_tests.rs"]
mod tests;
