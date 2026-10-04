use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::name_input::{NAME_GLYPH_CODE_COUNT, NAME_GLYPH_CODE_START, NameGlyphConsumerLayout};

use super::corpus_model::DialogueSourceCorpus;
use super::dialogue_code_allocation_model::{
    DialogueCharacterCodeAssignment, DialogueCodeAllocationAsset,
};
use super::dialogue_fixed_code_consumers_model::{
    DialogueFixedCodeConsumerAssetAudit, DialogueFixedCodeConsumerAuditReport,
};
use super::translation_model::DialogueTranslationControl;

const FIRST_SELECTOR_TRANSLATION_BANK: usize = 2;
const RENDERER_DECIMAL_CODES: [(char, u16); 10] = [
    ('0', 0x000a),
    ('1', 0x0001),
    ('2', 0x0002),
    ('3', 0x0003),
    ('4', 0x0004),
    ('5', 0x0005),
    ('6', 0x0006),
    ('7', 0x0007),
    ('8', 0x0008),
    ('9', 0x0009),
];

pub(super) struct DialogueFixedCodeConsumerInputs<'a> {
    pub(super) corpus: &'a DialogueSourceCorpus,
    pub(super) primary_referenced_coordinates: &'a BTreeSet<String>,
    pub(super) authored_semantic_hashes: &'a BTreeSet<String>,
    pub(super) runtime_insertion_semantic_hashes: &'a BTreeSet<String>,
    pub(super) controls_by_semantic_hash: &'a BTreeMap<String, Vec<DialogueTranslationControl>>,
    pub(super) selector_translation_population_complete: bool,
    pub(super) primary_translation_population_complete: bool,
    pub(super) expected_source_coordinate_count: usize,
    pub(super) expected_primary_referenced_coordinate_count: usize,
    pub(super) name_glyph_layout: &'a NameGlyphConsumerLayout,
    pub(super) allocation_assets: &'a [DialogueCodeAllocationAsset],
}

pub(super) fn audit_fixed_code_consumers(
    inputs: DialogueFixedCodeConsumerInputs<'_>,
) -> Result<DialogueFixedCodeConsumerAuditReport> {
    let DialogueFixedCodeConsumerInputs {
        corpus,
        primary_referenced_coordinates,
        authored_semantic_hashes,
        runtime_insertion_semantic_hashes,
        controls_by_semantic_hash,
        selector_translation_population_complete,
        primary_translation_population_complete,
        expected_source_coordinate_count,
        expected_primary_referenced_coordinate_count,
        name_glyph_layout,
        allocation_assets,
    } = inputs;
    let name_glyph_layout_complete = name_glyph_layout_covers_reserved_range(name_glyph_layout)?;
    let mut assets = Vec::with_capacity(corpus.assets.len());
    let mut primary_referenced_coordinate_rewrite_count = 0usize;
    let mut selector_stored_coordinate_count = 0usize;
    let mut selector_authored_coordinate_rewrite_count = 0usize;
    let mut runtime_insertion_coordinate_rewrite_count = 0usize;
    let mut preserved_primary_untranslated_coordinate_count = 0usize;
    let mut preserved_primary_unreferenced_coordinate_count = 0usize;
    let mut preserved_selector_coordinate_count = 0usize;
    let mut protected_source_glyph_code_count = 0usize;
    let mut protected_source_glyph_overwrite_count = 0usize;
    let mut source_coordinate_count = 0usize;
    let mut player_name_control_coordinate_count = 0usize;
    let mut school_name_control_coordinate_count = 0usize;
    let mut decimal_control_coordinate_count = 0usize;

    for corpus_asset in &corpus.assets {
        let allocation_asset = allocation_assets
            .iter()
            .find(|asset| asset.source_path == corpus_asset.source_path)
            .with_context(|| {
                format!(
                    "{} disappeared from the dialogue code allocation",
                    corpus_asset.source_path
                )
            })?;
        let coordinates = corpus_asset
            .banks
            .iter()
            .flat_map(|bank| {
                bank.entries
                    .iter()
                    .map(move |entry| StoredDialogueCoordinate {
                        selector_index: bank.selector_index,
                        coordinate_id: entry.coordinate_id.as_str(),
                        semantic_source_sha256: entry.semantic_source_sha256.as_str(),
                    })
            })
            .collect::<Vec<_>>();
        let ownership = classify_stored_coordinates(
            &coordinates,
            primary_referenced_coordinates,
            authored_semantic_hashes,
            runtime_insertion_semantic_hashes,
            !primary_translation_population_complete,
        )?;
        let controls = count_runtime_text_controls(
            &coordinates,
            authored_semantic_hashes,
            controls_by_semantic_hash,
        )?;
        let runtime_decimal_codes_preserve_source_glyphs = !controls.decimal_output_present
            || runtime_decimal_codes_remain_source_glyphs(&allocation_asset.assignments)?;
        let asset_protected_source_glyph_overwrite_count = allocation_asset
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.target_source_glyph_protected && assignment.requires_glyph_install
            })
            .count();
        let preserved_source_glyph_protection_complete =
            asset_protected_source_glyph_overwrite_count == 0;

        source_coordinate_count += ownership.stored_coordinate_count;
        primary_referenced_coordinate_rewrite_count +=
            ownership.primary_referenced_coordinate_rewrite_count;
        selector_stored_coordinate_count += ownership.selector_stored_coordinate_count;
        selector_authored_coordinate_rewrite_count +=
            ownership.selector_authored_coordinate_rewrite_count;
        runtime_insertion_coordinate_rewrite_count +=
            ownership.runtime_insertion_coordinate_rewrite_count;
        preserved_primary_untranslated_coordinate_count +=
            ownership.preserved_primary_untranslated_coordinate_count;
        preserved_primary_unreferenced_coordinate_count +=
            ownership.preserved_primary_unreferenced_coordinate_count;
        preserved_selector_coordinate_count += ownership.preserved_selector_coordinate_count;
        protected_source_glyph_code_count += allocation_asset.protected_source_glyph_code_count;
        protected_source_glyph_overwrite_count += asset_protected_source_glyph_overwrite_count;
        player_name_control_coordinate_count += controls.player_name_coordinate_count;
        school_name_control_coordinate_count += controls.school_name_coordinate_count;
        decimal_control_coordinate_count += controls.decimal_coordinate_count;

        assets.push(DialogueFixedCodeConsumerAssetAudit {
            source_path: corpus_asset.source_path.clone(),
            stored_coordinate_count: ownership.stored_coordinate_count,
            primary_referenced_coordinate_count: ownership.primary_referenced_coordinate_count,
            primary_referenced_coordinate_rewrite_count: ownership
                .primary_referenced_coordinate_rewrite_count,
            authored_coordinate_rewrite_count: ownership.authored_coordinate_rewrite_count,
            selector_authored_coordinate_rewrite_count: ownership
                .selector_authored_coordinate_rewrite_count,
            runtime_insertion_coordinate_rewrite_count: ownership
                .runtime_insertion_coordinate_rewrite_count,
            preserved_primary_untranslated_coordinate_count: ownership
                .preserved_primary_untranslated_coordinate_count,
            preserved_primary_unreferenced_coordinate_count: ownership
                .preserved_primary_unreferenced_coordinate_count,
            preserved_selector_coordinate_count: ownership.preserved_selector_coordinate_count,
            protected_source_glyph_code_count: allocation_asset.protected_source_glyph_code_count,
            protected_source_glyph_overwrite_count: asset_protected_source_glyph_overwrite_count,
            preserved_source_glyph_protection_complete,
            decimal_control_coordinate_count: controls.decimal_coordinate_count,
            runtime_decimal_codes_preserve_source_glyphs,
        });
    }

    let unresolved_runtime_sources = allocation_assets
        .iter()
        .flat_map(|asset| {
            asset
                .unresolved_runtime_sources
                .iter()
                .map(|source| format!("{}:{source}", asset.source_path))
        })
        .collect::<Vec<_>>();
    let runtime_sources_resolved = unresolved_runtime_sources.is_empty();
    let runtime_decimal_codes_preserve_source_glyphs = assets
        .iter()
        .all(|asset| asset.runtime_decimal_codes_preserve_source_glyphs);
    let preserved_source_glyph_protection_complete = assets
        .iter()
        .all(|asset| asset.preserved_source_glyph_protection_complete);
    let primary_script_inventory_complete =
        primary_referenced_coordinates.len() == expected_primary_referenced_coordinate_count;
    let expected_runtime_insertion_coordinate_count = corpus
        .assets
        .iter()
        .filter(|asset| {
            asset
                .banks
                .iter()
                .any(|bank| bank.selector_index == FIRST_SELECTOR_TRANSLATION_BANK)
        })
        .count()
        * super::runtime_insertion_messages::runtime_insertion_message_count_per_asset();
    let source_coordinate_partition_complete = source_coordinate_count
        == expected_source_coordinate_count
        && source_coordinate_count
            == assets
                .iter()
                .map(|asset| {
                    asset.authored_coordinate_rewrite_count
                        + asset.runtime_insertion_coordinate_rewrite_count
                        + asset.preserved_primary_untranslated_coordinate_count
                        + asset.preserved_primary_unreferenced_coordinate_count
                        + asset.preserved_selector_coordinate_count
                })
                .sum::<usize>();
    let fixed_code_consumer_ownership_complete = primary_script_inventory_complete
        && primary_translation_population_complete
        && primary_referenced_coordinate_rewrite_count
            == expected_primary_referenced_coordinate_count
        && preserved_primary_untranslated_coordinate_count == 0
        && selector_translation_population_complete
        && selector_stored_coordinate_count
            == selector_authored_coordinate_rewrite_count
                + runtime_insertion_coordinate_rewrite_count
        && runtime_insertion_coordinate_rewrite_count
            == expected_runtime_insertion_coordinate_count
        && preserved_selector_coordinate_count == 0
        && source_coordinate_partition_complete
        && name_glyph_layout_complete
        && runtime_sources_resolved
        && runtime_decimal_codes_preserve_source_glyphs;

    Ok(DialogueFixedCodeConsumerAuditReport {
        primary_script_inventory_complete,
        primary_translation_population_complete,
        primary_referenced_coordinate_count: primary_referenced_coordinates.len(),
        primary_referenced_coordinate_rewrite_count,
        selector_translation_population_complete,
        selector_stored_coordinate_count,
        selector_authored_coordinate_rewrite_count,
        runtime_insertion_coordinate_rewrite_count,
        preserved_primary_untranslated_coordinate_count,
        preserved_primary_unreferenced_coordinate_count,
        preserved_selector_coordinate_count,
        protected_source_glyph_code_count,
        protected_source_glyph_overwrite_count,
        preserved_source_glyph_protection_complete,
        source_coordinate_count,
        source_coordinate_partition_complete,
        name_glyph_layout_complete,
        name_glyph_active_code_count: name_glyph_layout.active_glyphs.len(),
        name_glyph_cache_code_count: name_glyph_layout.cache_slots.len(),
        name_glyph_pack_code_count: name_glyph_layout.pack_cells.len(),
        player_name_control_coordinate_count,
        school_name_control_coordinate_count,
        decimal_control_coordinate_count,
        runtime_sources_resolved,
        unresolved_runtime_sources,
        runtime_decimal_codes_preserve_source_glyphs,
        fixed_code_consumer_ownership_complete,
        assets,
    })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StoredDialogueCoordinate<'a> {
    pub(super) selector_index: usize,
    pub(super) coordinate_id: &'a str,
    pub(super) semantic_source_sha256: &'a str,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct StoredCoordinateOwnershipCounts {
    pub(super) stored_coordinate_count: usize,
    pub(super) primary_referenced_coordinate_count: usize,
    pub(super) primary_referenced_coordinate_rewrite_count: usize,
    pub(super) authored_coordinate_rewrite_count: usize,
    pub(super) selector_stored_coordinate_count: usize,
    pub(super) selector_authored_coordinate_rewrite_count: usize,
    pub(super) runtime_insertion_coordinate_rewrite_count: usize,
    pub(super) preserved_primary_untranslated_coordinate_count: usize,
    pub(super) preserved_primary_unreferenced_coordinate_count: usize,
    pub(super) preserved_selector_coordinate_count: usize,
}

pub(super) fn classify_stored_coordinates(
    coordinates: &[StoredDialogueCoordinate<'_>],
    primary_referenced_coordinates: &BTreeSet<String>,
    authored_semantic_hashes: &BTreeSet<String>,
    runtime_insertion_semantic_hashes: &BTreeSet<String>,
    allow_preserved_primary_references: bool,
) -> Result<StoredCoordinateOwnershipCounts> {
    let mut counts = StoredCoordinateOwnershipCounts::default();
    for coordinate in coordinates {
        let is_primary_reference =
            primary_referenced_coordinates.contains(coordinate.coordinate_id);
        let is_authored = authored_semantic_hashes.contains(coordinate.semantic_source_sha256);
        let is_runtime_insertion =
            runtime_insertion_semantic_hashes.contains(coordinate.semantic_source_sha256);
        ensure!(
            !(is_authored && is_runtime_insertion),
            "one dialogue coordinate has two rewrite owners"
        );

        counts.stored_coordinate_count += 1;
        if coordinate.selector_index >= FIRST_SELECTOR_TRANSLATION_BANK {
            counts.selector_stored_coordinate_count += 1;
        }
        if is_primary_reference {
            counts.primary_referenced_coordinate_count += 1;
        }
        if is_authored {
            counts.authored_coordinate_rewrite_count += 1;
            if is_primary_reference {
                counts.primary_referenced_coordinate_rewrite_count += 1;
            }
            if coordinate.selector_index >= FIRST_SELECTOR_TRANSLATION_BANK {
                counts.selector_authored_coordinate_rewrite_count += 1;
            }
            continue;
        }
        if is_runtime_insertion {
            ensure!(
                coordinate.selector_index >= FIRST_SELECTOR_TRANSLATION_BANK,
                "runtime insertion rewrite appears in a primary script bank"
            );
            counts.runtime_insertion_coordinate_rewrite_count += 1;
            continue;
        }
        if coordinate.selector_index >= FIRST_SELECTOR_TRANSLATION_BANK {
            counts.preserved_selector_coordinate_count += 1;
        } else {
            if is_primary_reference {
                ensure!(
                    allow_preserved_primary_references,
                    "execution-referenced primary dialogue lacks a rewrite owner"
                );
                counts.preserved_primary_untranslated_coordinate_count += 1;
            } else {
                counts.preserved_primary_unreferenced_coordinate_count += 1;
            }
        }
    }
    Ok(counts)
}

pub(super) fn runtime_decimal_codes_remain_source_glyphs(
    assignments: &[DialogueCharacterCodeAssignment],
) -> Result<bool> {
    for (character, expected_code) in RENDERER_DECIMAL_CODES {
        let Some(assignment) = assignments
            .iter()
            .find(|assignment| assignment.character == character.to_string())
        else {
            return Ok(false);
        };
        if !assignment.source_glyph_reused || parse_hex_code(&assignment.code)? != expected_code {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, Default)]
struct RuntimeTextControlCounts {
    player_name_coordinate_count: usize,
    school_name_coordinate_count: usize,
    decimal_coordinate_count: usize,
    decimal_output_present: bool,
}

fn count_runtime_text_controls(
    coordinates: &[StoredDialogueCoordinate<'_>],
    authored_semantic_hashes: &BTreeSet<String>,
    controls_by_semantic_hash: &BTreeMap<String, Vec<DialogueTranslationControl>>,
) -> Result<RuntimeTextControlCounts> {
    let mut result = RuntimeTextControlCounts::default();
    for coordinate in coordinates {
        if !authored_semantic_hashes.contains(coordinate.semantic_source_sha256) {
            continue;
        }
        let controls = controls_by_semantic_hash
            .get(coordinate.semantic_source_sha256)
            .context("authored dialogue lacks control ownership evidence")?;
        let mut has_player_name = false;
        let mut has_school_name = false;
        let mut has_decimal = false;
        for control in controls {
            match control.semantic_name.as_str() {
                "protagonist_family_name"
                | "protagonist_given_name"
                | "protagonist_nickname"
                | "relationship_name_plain"
                | "relationship_name_kun_kanji"
                | "relationship_name_kun_katakana"
                | "relationship_family_name_kun_katakana"
                | "relationship_name_san"
                | "relationship_name_san_or_chan" => has_player_name = true,
                "current_school_name" => has_school_name = true,
                "current_month" | "current_day" | "comparison_month" | "comparison_day" => {
                    has_decimal = true;
                }
                _ => {}
            }
        }
        result.player_name_coordinate_count += usize::from(has_player_name);
        result.school_name_coordinate_count += usize::from(has_school_name);
        result.decimal_coordinate_count += usize::from(has_decimal);
        result.decimal_output_present |= has_decimal;
    }
    Ok(result)
}

fn name_glyph_layout_covers_reserved_range(layout: &NameGlyphConsumerLayout) -> Result<bool> {
    if layout.code_start != NAME_GLYPH_CODE_START || layout.code_count != NAME_GLYPH_CODE_COUNT {
        return Ok(false);
    }
    let codes = layout
        .active_glyphs
        .iter()
        .map(|cell| cell.code)
        .chain(layout.cache_slots.iter().map(|slot| slot.cache_code))
        .chain(layout.pack_cells.iter().map(|cell| cell.code))
        .collect::<BTreeSet<_>>();
    let expected = (0..NAME_GLYPH_CODE_COUNT)
        .map(|index| {
            NAME_GLYPH_CODE_START
                .checked_add(u16::try_from(index)?)
                .context("shared name glyph code range overflow")
        })
        .collect::<Result<BTreeSet<_>>>()?;
    Ok(codes == expected)
}

fn parse_hex_code(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .context("dialogue code lacks 0x prefix")?;
    Ok(u16::from_str_radix(digits, 16)?)
}
