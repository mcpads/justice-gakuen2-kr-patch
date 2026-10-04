use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};

#[path = "menu_source_workspace/model.rs"]
mod model;
#[path = "menu_source_workspace/writer.rs"]
mod writer;

pub use model::{
    MenuSourceAdmission, MenuSourceCandidate, MenuSourceConsumerEvidence,
    MenuSourceLoadedWordReference, MenuSourceNonTextEvidence, MenuSourceShard, MenuSourceShardRef,
    MenuSourceToken, MenuSourceTokenKind, MenuSourceWorkspaceConfig, MenuSourceWorkspaceManifest,
};

use crate::menu_audit::{StringCandidate, collect_menu_code_audit};
use crate::menu_glyph_audit::{MenuGlyphCellAudit, collect_menu_glyph_audit};
use crate::pipeline::sha256_bytes;
use writer::{
    MAX_MENU_SOURCE_SHARD_BYTES, MAX_MENU_SOURCE_SHARD_ENTRIES, MenuSourceShardIdentity,
    prepare_workspace_root, write_json, write_overlay_shards,
};

pub fn initialize_menu_source_workspace(
    config: &MenuSourceWorkspaceConfig,
) -> Result<MenuSourceWorkspaceManifest> {
    let code_audit = collect_menu_code_audit(&config.cue, config.address_flow_state_budget)?;
    let glyph_audit = collect_menu_glyph_audit(&config.cue, &config.dialogue_codebook)?;
    ensure!(
        code_audit.source_bin_sha256 == glyph_audit.source_bin_sha256,
        "menu code and glyph analyses use different source media"
    );
    let menu_code_analysis_sha256 = analysis_sha256(&code_audit)?;
    let menu_glyph_analysis_sha256 = analysis_sha256(&glyph_audit)?;
    let glyphs = glyph_audit
        .cells
        .iter()
        .map(|cell| (cell.code.as_str(), cell))
        .collect::<BTreeMap<_, _>>();

    prepare_workspace_root(&config.output, config.force)?;
    let identity = MenuSourceShardIdentity {
        source_bin_sha256: &code_audit.source_bin_sha256,
        dialogue_codebook_sha256: &glyph_audit.dialogue_codebook_sha256,
        menu_code_analysis_sha256: &menu_code_analysis_sha256,
        menu_glyph_analysis_sha256: &menu_glyph_analysis_sha256,
    };
    let mut shards = Vec::new();
    let mut fully_exact_decoded_string_count = 0usize;
    let mut partially_exact_decoded_string_count = 0usize;
    let mut unresolved_string_count = 0usize;
    let mut consumer_confirmed_string_count = 0usize;
    let mut consumer_confirmed_reference_count = 0usize;
    let mut confirmed_non_text_candidate_count = 0usize;
    let mut unclassified_candidate_count = 0usize;
    let mut unclassified_entrypoint_reachable_direct_pointer_load_candidate_count = 0usize;
    let mut entrypoint_reachable_static_reference_string_count = 0usize;
    let mut entrypoint_reachable_address_materialization_reference_count = 0usize;
    let mut entrypoint_reachable_memory_access_reference_count = 0usize;
    let mut entrypoint_reachable_loaded_word_reference_count = 0usize;
    let mut entrypoint_reachable_direct_pointer_load_string_count = 0usize;
    let mut entrypoint_reachable_direct_pointer_load_reference_count = 0usize;
    let mut exact_decoded_glyph_occurrence_count = 0usize;
    let mut unresolved_glyph_occurrence_count = 0usize;
    for overlay in code_audit.overlays {
        let mut entries = Vec::with_capacity(overlay.strings.len());
        for candidate in overlay.strings {
            let entry = source_candidate(&overlay.path, candidate, &glyphs)?;
            consumer_confirmed_string_count += usize::from(entry.consumer_confirmed);
            consumer_confirmed_reference_count += entry.consumer_reference_count;
            confirmed_non_text_candidate_count += usize::from(!entry.non_text_evidence.is_empty());
            unclassified_candidate_count +=
                usize::from(entry.admission == MenuSourceAdmission::StaticReferenceCandidate);
            let has_entrypoint_reachable_reference = !entry
                .entrypoint_reachable_address_materialization_references
                .is_empty()
                || !entry
                    .entrypoint_reachable_memory_access_references
                    .is_empty()
                || entry
                    .loaded_word_references
                    .iter()
                    .any(|reference| !reference.entrypoint_reachable_seed_offsets.is_empty());
            entrypoint_reachable_static_reference_string_count +=
                usize::from(has_entrypoint_reachable_reference);
            entrypoint_reachable_address_materialization_reference_count += entry
                .entrypoint_reachable_address_materialization_references
                .len();
            entrypoint_reachable_memory_access_reference_count +=
                entry.entrypoint_reachable_memory_access_references.len();
            entrypoint_reachable_loaded_word_reference_count += entry
                .loaded_word_references
                .iter()
                .map(|reference| reference.entrypoint_reachable_seed_offsets.len())
                .sum::<usize>();
            entrypoint_reachable_direct_pointer_load_string_count +=
                usize::from(entry.loaded_word_references.iter().any(|reference| {
                    !reference
                        .entrypoint_reachable_direct_pointer_load_seed_offsets
                        .is_empty()
                }));
            unclassified_entrypoint_reachable_direct_pointer_load_candidate_count += usize::from(
                entry.admission == MenuSourceAdmission::StaticReferenceCandidate
                    && entry.loaded_word_references.iter().any(|reference| {
                        !reference
                            .entrypoint_reachable_direct_pointer_load_seed_offsets
                            .is_empty()
                    }),
            );
            entrypoint_reachable_direct_pointer_load_reference_count += entry
                .loaded_word_references
                .iter()
                .map(|reference| {
                    reference
                        .entrypoint_reachable_direct_pointer_load_seed_offsets
                        .len()
                })
                .sum::<usize>();
            exact_decoded_glyph_occurrence_count += entry.exact_decoded_glyph_count;
            unresolved_glyph_occurrence_count += entry.unresolved_glyph_count;
            match (
                entry.exact_decoded_glyph_count,
                entry.unresolved_glyph_count,
            ) {
                (_, 0) => fully_exact_decoded_string_count += 1,
                (0, _) => unresolved_string_count += 1,
                _ => partially_exact_decoded_string_count += 1,
            }
            entries.push(entry);
        }
        shards.extend(write_overlay_shards(
            &config.output,
            &overlay.path,
            entries,
            &identity,
        )?);
    }
    let candidate_string_count = fully_exact_decoded_string_count
        + partially_exact_decoded_string_count
        + unresolved_string_count;
    ensure!(
        candidate_string_count == code_audit.candidate_string_count,
        "menu source candidate denominator changed while sharding"
    );
    let manifest = MenuSourceWorkspaceManifest {
        kind: "Justice Gakuen 2 sharded menu source candidate workspace".to_string(),
        source_bin_sha256: code_audit.source_bin_sha256,
        dialogue_codebook_sha256: glyph_audit.dialogue_codebook_sha256,
        menu_decoded_sha256: glyph_audit.menu_decoded_sha256,
        menu_code_analysis_sha256,
        menu_glyph_analysis_sha256,
        address_flow_state_budget: config.address_flow_state_budget,
        scope: "static_reference_candidate_classification".to_string(),
        overlay_count: code_audit.candidate_overlay_count,
        candidate_string_count,
        fully_exact_decoded_string_count,
        partially_exact_decoded_string_count,
        unresolved_string_count,
        exact_decoded_glyph_occurrence_count,
        unresolved_glyph_occurrence_count,
        entrypoint_reachable_static_reference_string_count,
        entrypoint_reachable_address_materialization_reference_count,
        entrypoint_reachable_memory_access_reference_count,
        entrypoint_reachable_loaded_word_reference_count,
        entrypoint_reachable_direct_pointer_load_string_count,
        entrypoint_reachable_direct_pointer_load_reference_count,
        consumer_confirmed_string_count,
        consumer_confirmed_reference_count,
        confirmed_non_text_candidate_count,
        unclassified_candidate_count,
        unclassified_entrypoint_reachable_direct_pointer_load_candidate_count,
        translation_target_count: None,
        acquisition_complete: false,
        max_entries_per_shard: MAX_MENU_SOURCE_SHARD_ENTRIES,
        max_bytes_per_shard: MAX_MENU_SOURCE_SHARD_BYTES,
        shard_count: shards.len(),
        shards,
        limitations: vec![
            "Only source-bound renderer tables with an established consumer contract are marked consumer-confirmed; source-bound non-text structures are classified separately, and all other entries remain static reference candidates.".to_string(),
            "Entrypoint-reachable local address-flow references narrow development analysis but do not establish a renderer consumer contract or a translation denominator.".to_string(),
            "Exact text is inherited only from byte-identical original-media dialogue pixels; unresolved glyphs stay explicit and no translation prose is generated.".to_string(),
            "Budget-exhausted address-flow seeds, unknown or external calls, runtime-generated strings, and baked graphical text remain outside this candidate workspace.".to_string(),
        ],
    };
    write_json(&config.output, Path::new("manifest.json"), &manifest)?;
    Ok(manifest)
}

fn source_candidate(
    overlay_path: &str,
    candidate: StringCandidate,
    glyphs: &BTreeMap<&str, &MenuGlyphCellAudit>,
) -> Result<MenuSourceCandidate> {
    let mut tokens = Vec::with_capacity(candidate.raw_codes.len());
    for raw_code in &candidate.raw_codes {
        tokens.push(source_token(raw_code, glyphs)?);
    }
    let exact_decoded_glyph_count = tokens
        .iter()
        .filter(|token| {
            token.kind == MenuSourceTokenKind::Glyph && token.exact_dialogue_pixel_text.is_some()
        })
        .count();
    let unresolved_glyph_count = tokens
        .iter()
        .filter(|token| {
            token.kind == MenuSourceTokenKind::Glyph && token.exact_dialogue_pixel_text.is_none()
        })
        .count();
    let fully_exact_text = (unresolved_glyph_count == 0).then(|| {
        tokens
            .iter()
            .filter_map(|token| token.exact_dialogue_pixel_text.as_deref())
            .collect::<String>()
    });
    let newopt_consumer_reference_count =
        newopt_placement_reference_count(overlay_path, &candidate.pointer_offsets)?;
    let consumer_reference_count = candidate
        .consumer_reference_count
        .checked_add(newopt_consumer_reference_count)
        .context("menu consumer reference count overflow")?;
    let consumer_confirmed = consumer_reference_count > 0;
    ensure!(
        !consumer_confirmed || candidate.non_text_evidence.is_empty(),
        "menu candidate cannot be both a renderer consumer and confirmed non-text"
    );
    let mut consumer_evidence = candidate
        .consumer_evidence
        .iter()
        .copied()
        .map(MenuSourceConsumerEvidence::from)
        .collect::<Vec<_>>();
    if newopt_consumer_reference_count > 0 {
        consumer_evidence.push(MenuSourceConsumerEvidence::NewoptPlacementPointerTable);
    }
    let non_text_evidence = candidate
        .non_text_evidence
        .iter()
        .copied()
        .map(MenuSourceNonTextEvidence::from)
        .collect::<Vec<_>>();
    let loaded_word_references = compact_loaded_word_references(&candidate)?;
    Ok(MenuSourceCandidate {
        candidate_id: format!("{overlay_path}@{}", candidate.target_offset),
        target_offset: candidate.target_offset,
        admission: if consumer_confirmed {
            MenuSourceAdmission::FormatConfirmedRendererConsumer
        } else if !non_text_evidence.is_empty() {
            MenuSourceAdmission::ConfirmedNonTextStructure
        } else {
            MenuSourceAdmission::StaticReferenceCandidate
        },
        consumer_confirmed,
        consumer_evidence,
        consumer_reference_count,
        non_text_evidence,
        fully_exact_text,
        exact_decoded_glyph_count,
        unresolved_glyph_count,
        tokens,
        pointer_offsets: candidate.pointer_offsets,
        address_materialization_offsets: candidate.address_materialization_offsets,
        memory_access_offsets: candidate.memory_access_offsets,
        loaded_word_references,
        entrypoint_reachable_address_materialization_references: candidate
            .entrypoint_reachable_address_materialization_references,
        entrypoint_reachable_memory_access_references: candidate
            .entrypoint_reachable_memory_access_references,
        shared_references: candidate.shared_references,
    })
}

fn compact_loaded_word_references(
    candidate: &StringCandidate,
) -> Result<Vec<MenuSourceLoadedWordReference>> {
    let mut references = candidate
        .loaded_word_references
        .iter()
        .map(|reference| {
            (
                (
                    reference.instruction_offset.clone(),
                    reference.load_instruction_offset.clone(),
                    reference.storage_address.clone(),
                ),
                MenuSourceLoadedWordReference {
                    instruction_offset: reference.instruction_offset.clone(),
                    load_instruction_offset: reference.load_instruction_offset.clone(),
                    storage_address: reference.storage_address.clone(),
                    entrypoint_reachable_seed_offsets: Vec::new(),
                    entrypoint_reachable_direct_pointer_load_seed_offsets: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for reference in &candidate.entrypoint_reachable_loaded_word_references {
        let key = (
            reference.instruction_offset.clone(),
            reference.load_instruction_offset.clone(),
            reference.storage_address.clone(),
        );
        references
            .get_mut(&key)
            .context("reachable loaded-word chain has no static reference")?
            .entrypoint_reachable_seed_offsets
            .push(reference.seed_offset.clone());
    }
    for reference in &candidate.entrypoint_reachable_direct_pointer_load_references {
        let key = (
            reference.instruction_offset.clone(),
            reference.load_instruction_offset.clone(),
            reference.storage_address.clone(),
        );
        references
            .get_mut(&key)
            .context("direct pointer load has no static reference")?
            .entrypoint_reachable_direct_pointer_load_seed_offsets
            .push(reference.seed_offset.clone());
    }
    Ok(references.into_values().collect())
}

fn newopt_placement_reference_count(
    overlay_path: &str,
    pointer_offsets: &[String],
) -> Result<usize> {
    if overlay_path != "DAT1/NEWOPT.BIN" {
        return Ok(0);
    }
    let mut reference_count = 0usize;
    for pointer_offset in pointer_offsets {
        let offset = usize::from_str_radix(
            pointer_offset
                .strip_prefix("0x")
                .context("menu pointer offset lost its hexadecimal prefix")?,
            16,
        )
        .context("menu pointer offset is not hexadecimal")?;
        if (0x0b90..=0x0bcc).contains(&offset) && (offset - 0x0b90).is_multiple_of(4) {
            reference_count += 1;
        }
    }
    Ok(reference_count)
}

fn source_token(
    raw_code: &str,
    glyphs: &BTreeMap<&str, &MenuGlyphCellAudit>,
) -> Result<MenuSourceToken> {
    let raw_value = u16::from_str_radix(
        raw_code
            .strip_prefix("0x")
            .context("menu raw code lost its hexadecimal prefix")?,
        16,
    )
    .context("menu raw code is not hexadecimal")?;
    let normalized_value = raw_value & 0x0fff;
    let normalized_code = format!("0x{normalized_value:04x}");
    if normalized_value == 0x0fff {
        return Ok(MenuSourceToken {
            raw_code: raw_code.to_string(),
            normalized_code,
            control_nibble: (raw_value >> 12) as u8,
            kind: MenuSourceTokenKind::Skip,
            pixel_sha256: None,
            exact_dialogue_pixel_text: None,
        });
    }
    let glyph = glyphs
        .get(normalized_code.as_str())
        .context("menu code has no physical glyph audit cell")?;
    Ok(MenuSourceToken {
        raw_code: raw_code.to_string(),
        normalized_code,
        control_nibble: (raw_value >> 12) as u8,
        kind: MenuSourceTokenKind::Glyph,
        pixel_sha256: Some(glyph.pixel_sha256.clone()),
        exact_dialogue_pixel_text: glyph.exact_dialogue_pixel_text.clone(),
    })
}

fn analysis_sha256(value: &impl serde::Serialize) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

#[cfg(test)]
#[path = "menu_source_workspace_tests.rs"]
mod tests;
