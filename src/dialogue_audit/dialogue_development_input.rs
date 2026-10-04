use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};

use super::corpus_model::DialogueSourceCorpus;
use super::translation_model::DialogueTranslationControl;

#[derive(Debug, Default)]
pub(super) struct AssetDialogueDemand {
    pub(super) translated_coordinate_count: usize,
    pub(super) static_characters: BTreeSet<char>,
    pub(super) controls: Vec<DialogueTranslationControl>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeCharacterDemand {
    pub(super) characters: BTreeSet<char>,
    pub(super) unresolved_sources: BTreeSet<String>,
}

impl RuntimeCharacterDemand {
    pub(super) fn is_complete(&self) -> bool {
        self.unresolved_sources.is_empty()
    }
}

pub(super) fn collect_asset_dialogue_demand(
    corpus: &DialogueSourceCorpus,
    translated_segments: &BTreeMap<String, Vec<String>>,
    controls_by_semantic_hash: &BTreeMap<String, Vec<DialogueTranslationControl>>,
) -> Result<BTreeMap<String, AssetDialogueDemand>> {
    let mut demand_by_asset = BTreeMap::<String, AssetDialogueDemand>::new();
    for asset in &corpus.assets {
        let demand = demand_by_asset
            .entry(asset.source_path.clone())
            .or_default();
        for entry in asset.banks.iter().flat_map(|bank| &bank.entries) {
            let Some(segments) = translated_segments.get(&entry.semantic_source_sha256) else {
                continue;
            };
            let controls = controls_by_semantic_hash
                .get(&entry.semantic_source_sha256)
                .context("authored dialogue lacks protected source controls")?;
            demand.translated_coordinate_count += 1;
            demand
                .static_characters
                .extend(segments.iter().flat_map(|segment| segment.chars()));
            demand.controls.extend(controls.iter().cloned());
        }
    }
    Ok(demand_by_asset)
}

pub(super) fn classify_runtime_character_demand(
    controls: &[DialogueTranslationControl],
) -> RuntimeCharacterDemand {
    let mut result = RuntimeCharacterDemand {
        characters: "군씨짱".chars().collect(),
        ..RuntimeCharacterDemand::default()
    };
    for control in controls {
        match control.semantic_name.as_str() {
            "protagonist_family_name"
            | "protagonist_given_name"
            | "protagonist_nickname"
            | "relationship_name_plain" => {
                result
                    .unresolved_sources
                    .insert("localized_player_name_buffers".to_string());
            }
            "relationship_name_kun_kanji"
            | "relationship_name_kun_katakana"
            | "relationship_family_name_kun_katakana" => {
                result.characters.extend("군".chars());
                result
                    .unresolved_sources
                    .insert("localized_player_name_buffers".to_string());
            }
            "relationship_name_san" => {
                result.characters.extend("씨".chars());
                result
                    .unresolved_sources
                    .insert("localized_player_name_buffers".to_string());
            }
            "relationship_name_san_or_chan" => {
                result.characters.extend("씨짱".chars());
                result
                    .unresolved_sources
                    .insert("localized_player_name_buffers".to_string());
            }
            "current_school_name" => {
                result
                    .unresolved_sources
                    .insert("localized_school_name_table".to_string());
            }
            "current_month" | "current_day" | "comparison_month" | "comparison_day" => {
                result.characters.extend("0123456789".chars());
            }
            _ => {}
        }
    }
    result
}
