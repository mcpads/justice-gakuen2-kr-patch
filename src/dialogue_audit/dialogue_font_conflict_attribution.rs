use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::corpus_model::{DialogueCorpusAsset, DialogueCorpusToken};
use super::dialogue_code_allocation_model::DialogueCodeAllocationAsset;

const FIRST_SELECTOR_TRANSLATION_BANK: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConflictGlyphOccurrence {
    pub(super) semantic_source_sha256: String,
    pub(super) coordinate_id: String,
    pub(super) code: u16,
    pub(super) source_character: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AttributedConflictCode {
    pub(super) code: u16,
    pub(super) replacement_character: String,
    pub(super) preserved_source_character: String,
    pub(super) preserved_coordinate_count: usize,
    pub(super) preserved_glyph_occurrence_count: usize,
    pub(super) required_semantic_hashes: Vec<String>,
}

pub(super) fn conflict_targets(
    asset: &DialogueCodeAllocationAsset,
) -> Result<BTreeMap<u16, String>> {
    let mut targets = BTreeMap::new();
    for assignment in &asset.assignments {
        if !assignment.target_source_glyph_protected || !assignment.requires_glyph_install {
            continue;
        }
        ensure!(
            targets
                .insert(
                    parse_hex_code(&assignment.code)?,
                    assignment.character.clone()
                )
                .is_none(),
            "dialogue allocation assigns one protected code more than once"
        );
    }
    Ok(targets)
}

pub(super) fn collect_conflict_occurrences(
    asset: &DialogueCorpusAsset,
    targets: &BTreeMap<u16, String>,
    authored_semantic_hashes: &BTreeSet<String>,
    runtime_insertion_semantic_hashes: &BTreeSet<String>,
    primary_referenced_coordinates: &BTreeSet<String>,
) -> Result<Vec<ConflictGlyphOccurrence>> {
    let mut result = Vec::new();
    for bank in &asset.banks {
        for entry in &bank.entries {
            if authored_semantic_hashes.contains(&entry.semantic_source_sha256)
                || runtime_insertion_semantic_hashes.contains(&entry.semantic_source_sha256)
                || (bank.selector_index < FIRST_SELECTOR_TRANSLATION_BANK
                    && !primary_referenced_coordinates.contains(&entry.coordinate_id))
            {
                continue;
            }
            for token in &entry.tokens {
                let DialogueCorpusToken::Glyph { code, text, .. } = token else {
                    continue;
                };
                let code = parse_hex_code(code)?;
                if targets.contains_key(&code) {
                    result.push(ConflictGlyphOccurrence {
                        semantic_source_sha256: entry.semantic_source_sha256.clone(),
                        coordinate_id: entry.coordinate_id.clone(),
                        code,
                        source_character: text.clone(),
                    });
                }
            }
        }
    }
    Ok(result)
}

pub(super) fn attribute_conflict_occurrences(
    targets: &BTreeMap<u16, String>,
    occurrences: &[ConflictGlyphOccurrence],
) -> Result<Vec<AttributedConflictCode>> {
    let mut groups_by_code = BTreeMap::<u16, BTreeSet<String>>::new();
    let mut coordinates_by_code = BTreeMap::<u16, BTreeSet<String>>::new();
    let mut source_characters_by_code = BTreeMap::<u16, BTreeSet<String>>::new();
    let mut occurrence_count_by_code = BTreeMap::<u16, usize>::new();
    for occurrence in occurrences {
        ensure!(
            targets.contains_key(&occurrence.code),
            "font-conflict occurrence does not target a conflicting code"
        );
        groups_by_code
            .entry(occurrence.code)
            .or_default()
            .insert(occurrence.semantic_source_sha256.clone());
        coordinates_by_code
            .entry(occurrence.code)
            .or_default()
            .insert(occurrence.coordinate_id.clone());
        source_characters_by_code
            .entry(occurrence.code)
            .or_default()
            .insert(occurrence.source_character.clone());
        *occurrence_count_by_code.entry(occurrence.code).or_default() += 1;
    }
    let mut result = Vec::new();
    for (&code, replacement_character) in targets {
        let groups = groups_by_code
            .remove(&code)
            .context("protected font-conflict code has no preserved semantic owner")?;
        let coordinates = coordinates_by_code
            .remove(&code)
            .context("protected font-conflict code has no preserved coordinate")?;
        let source_characters = source_characters_by_code
            .remove(&code)
            .context("protected font-conflict code has no source character")?;
        ensure!(
            source_characters.len() == 1,
            "one fixed source code resolves to multiple source characters"
        );
        result.push(AttributedConflictCode {
            code,
            replacement_character: replacement_character.clone(),
            preserved_source_character: source_characters
                .into_iter()
                .next()
                .context("source character set became empty")?,
            preserved_coordinate_count: coordinates.len(),
            preserved_glyph_occurrence_count: occurrence_count_by_code
                .remove(&code)
                .context("protected font-conflict code has no glyph occurrence")?,
            required_semantic_hashes: groups.into_iter().collect(),
        });
    }
    ensure!(
        groups_by_code.is_empty()
            && coordinates_by_code.is_empty()
            && source_characters_by_code.is_empty()
            && occurrence_count_by_code.is_empty(),
        "font-conflict occurrences contain an unreported code"
    );
    Ok(result)
}

fn parse_hex_code(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .context("dialogue code lacks 0x prefix")?;
    Ok(u16::from_str_radix(digits, 16)?)
}
