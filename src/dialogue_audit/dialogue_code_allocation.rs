use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::name_input::{
    NAME_GLYPH_CODE_COUNT, NAME_GLYPH_CODE_START, load_name_input_keyboard,
    plan_name_glyph_consumer_layout,
};
use crate::source_disc::SupportedSourceDisc;

use super::atlas::parse_dialogue_atlas;
use super::codebook::{
    load_resolved_dialogue_glyphs, mgk_source_glyph_inventory, runtime_source_glyph_inventory,
};
use super::corpus_model::DialogueCorpusAsset;
use super::dialogue_code_allocation_model::{
    DialogueCharacterCodeAssignment, DialogueCodeAllocationAsset, DialogueCodeAllocationConfig,
    DialogueCodeAllocationReport,
};
use super::dialogue_development_input::{
    classify_runtime_character_demand, collect_asset_dialogue_demand,
};
use super::dialogue_fixed_code_consumers::{
    DialogueFixedCodeConsumerInputs, audit_fixed_code_consumers,
};
use super::dialogue_translation_input::load_dialogue_translation_inputs_from_source;
use super::format::hex_code;
use super::parser::parse_dialogue_banks;
use super::script_source::load_mgame_from_source;
use super::selector_consumer_spec::validate_adopted_selector_consumers;
use super::selector_translation_model::DialogueSelectorTranslationAuditConfig;
use super::sources::{load_dialogue_runtime_image_sources, load_mgk_dialogue_sources};
use super::tokens::{DialogueTokenKind, tokenize_dialogue_message};
use super::translation_model::DialogueTranslationAuditConfig;
use super::translation_workspace_source::TranslationWorkspaceSource;

const ADDRESSABLE_CODE_COUNT: usize = 1_001;
const ASSET_LOCAL_CODE_COUNT: usize = ADDRESSABLE_CODE_COUNT - NAME_GLYPH_CODE_COUNT;
const FIRST_SELECTOR_TRANSLATION_BANK: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AllocatedCode {
    pub(super) code: u16,
    pub(super) source_glyph_reused: bool,
}

pub fn build_dialogue_code_allocation(
    config: &DialogueCodeAllocationConfig,
) -> Result<DialogueCodeAllocationReport> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    let primary_config = DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    };
    let selector_config = DialogueSelectorTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.selector_translation.clone(),
        output: config.selector_translation_audit_output.clone(),
    };
    let (primary_input, selector_input) =
        load_dialogue_translation_inputs_from_source(&primary_config, &selector_config, &source)?;
    build_dialogue_code_allocation_from_translation_inputs(
        config,
        &primary_input,
        &selector_input,
        &source,
    )
}

pub(super) fn build_dialogue_code_allocation_from_translation_inputs(
    config: &DialogueCodeAllocationConfig,
    primary_input: &super::dialogue_translation_input::PrimaryDialogueTranslationInput,
    selector_input: &super::dialogue_translation_input::SelectorDialogueTranslationInput,
    source: &SupportedSourceDisc,
) -> Result<DialogueCodeAllocationReport> {
    let translation = &primary_input.report;
    let development_authored_group_count = translation
        .semantic_group_count
        .checked_sub(translation.untranslated_group_count)
        .context("dialogue authored group count underflow")?;
    ensure!(
        development_authored_group_count != 0
            && (!config.input_policy.requires_complete_scope()
                || translation.development_translation_input_available),
        "dialogue code allocation input policy requires unavailable primary text"
    );
    ensure!(
        primary_input.authored_segments.len() == development_authored_group_count,
        "dialogue authored input count changed"
    );
    let mut translated_segments = primary_input.authored_segments.clone();
    let selector_translation = &selector_input.report;
    let authored_selector_group_count = selector_translation
        .development_authored_group_count
        .checked_sub(selector_translation.runtime_insertion_rewrite_group_count)
        .context("selector authored group count underflow")?;
    ensure!(
        !config.input_policy.requires_complete_scope()
            || selector_translation.development_full_selector_input_available,
        "dialogue code allocation input policy requires unavailable selector text"
    );
    ensure!(
        selector_input.authored_segments.len() == authored_selector_group_count,
        "selector authored input count changed"
    );
    for (semantic_hash, segments) in &selector_input.authored_segments {
        ensure!(
            translated_segments
                .insert(semantic_hash.clone(), segments.clone())
                .is_none(),
            "primary and selector translation semantic hashes overlap"
        );
    }
    let name_input_keyboard = load_name_input_keyboard(&config.name_input_keyboard)?;
    let name_glyph_layout = plan_name_glyph_consumer_layout(&name_input_keyboard)?;
    let global_name_codes = name_glyph_layout
        .active_glyphs
        .iter()
        .map(|assignment| {
            Ok((
                one_character(&assignment.character)
                    .context("shared name glyph assignment is not one character")?,
                assignment.code,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let installed_global_name_codes = global_name_codes.clone();
    ensure!(
        usize::from(NAME_GLYPH_CODE_START) == ASSET_LOCAL_CODE_COUNT,
        "global name-entry code start disagrees with the dialogue allocation boundary"
    );
    let TranslationWorkspaceSource {
        corpus,
        groups_by_owner,
        referenced_coordinates,
        source_asset_count,
        ..
    } = &primary_input.source;
    ensure!(
        *source_asset_count == selector_translation.source_asset_count,
        "primary and selector translation scopes cover different source assets"
    );
    let mut controls_by_semantic_hash = groups_by_owner
        .values()
        .flatten()
        .map(|group| (group.semantic_source_sha256.clone(), group.controls.clone()))
        .collect::<BTreeMap<_, _>>();
    let selector_groups = selector_input
        .source
        .groups_by_selector
        .values()
        .flatten()
        .collect::<Vec<_>>();
    let runtime_insertion_semantic_hashes = selector_groups
        .iter()
        .filter(|group| {
            group.development_resolution
                == super::selector_translation_model::DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
        })
        .map(|group| group.semantic_source_sha256.clone())
        .collect::<BTreeSet<_>>();
    for group in &selector_groups {
        if translated_segments.contains_key(&group.semantic_source_sha256) {
            ensure!(
                controls_by_semantic_hash
                    .insert(group.semantic_source_sha256.clone(), group.controls.clone())
                    .is_none(),
                "primary and selector source controls overlap"
            );
        }
    }
    let mut demand_by_asset =
        collect_asset_dialogue_demand(corpus, &translated_segments, &controls_by_semantic_hash)?;

    let direct_selector_consumers =
        validate_adopted_selector_consumers(&load_mgame_from_source(source)?)?;
    let (source_pixel_hashes, usage) = match *source_asset_count {
        10 => mgk_source_glyph_inventory(source.image_path())?,
        78 => runtime_source_glyph_inventory(source.image_path())?,
        _ => anyhow::bail!("unsupported dialogue allocation source asset count"),
    };
    let (codebook_sha256, resolved_glyphs) = load_resolved_dialogue_glyphs(
        &config.codebook,
        &translation.source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    ensure!(
        codebook_sha256 == translation.codebook_sha256,
        "dialogue codebook changed during allocation"
    );

    let sources = match *source_asset_count {
        10 => load_mgk_dialogue_sources(source.image_path())?,
        78 => load_dialogue_runtime_image_sources(source.image_path())?,
        _ => unreachable!(),
    };
    let mut assets = Vec::new();
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)?;
        ensure!(
            atlas.addressable_slot_count == ADDRESSABLE_CODE_COUNT,
            "{} addressable dialogue code count changed",
            source.path
        );
        let demand = demand_by_asset
            .remove(&source.path)
            .with_context(|| format!("{} lacks translated code demand", source.path))?;
        let corpus_asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} disappeared from dialogue corpus", source.path))?;
        let protected_source_codes = preserved_fixed_source_codes(
            &decoded,
            corpus_asset,
            &translated_segments,
            &runtime_insertion_semantic_hashes,
            referenced_coordinates,
            atlas.fixed_cell_count,
            atlas.addressable_slot_count,
        )?;
        let mut runtime = classify_runtime_character_demand(&demand.controls);
        runtime
            .unresolved_sources
            .remove("localized_player_name_buffers");
        let mut required_characters = demand.static_characters.clone();
        required_characters.extend(&runtime.characters);
        let mut reusable_codes = reusable_source_codes(
            &atlas.fixed_cell_sha256,
            &resolved_glyphs
                .iter()
                .map(|(hash, glyph)| (hash.as_str(), glyph.text.as_str()))
                .collect(),
        );
        for (&character, &code) in &global_name_codes {
            reusable_codes.entry(character).or_default().push(code);
        }
        let allocated = allocate_character_codes(
            &required_characters,
            &reusable_codes,
            &protected_source_codes,
            ASSET_LOCAL_CODE_COUNT,
            ADDRESSABLE_CODE_COUNT,
        )
        .with_context(|| {
            format!(
                "failed to allocate {} required characters for {} while protecting {} source glyph codes",
                required_characters.len(),
                source.path,
                protected_source_codes.len()
            )
        })?;
        let assignments = allocated
            .iter()
            .map(|(character, allocated)| {
                let code = usize::from(allocated.code);
                let global_name_glyph_reused = installed_global_name_codes
                    .get(character)
                    .is_some_and(|global_code| *global_code == allocated.code);
                DialogueCharacterCodeAssignment {
                    character: character.to_string(),
                    code: hex_code(allocated.code),
                    source_glyph_reused: allocated.source_glyph_reused && !global_name_glyph_reused,
                    global_name_glyph_reused,
                    requires_glyph_install: !allocated.source_glyph_reused,
                    target_was_fixed_source_cell: code < atlas.fixed_cell_count,
                    target_source_glyph_protected: protected_source_codes.contains(&allocated.code),
                    target_source_cell_sha256: atlas.fixed_cell_sha256.get(code).cloned(),
                }
            })
            .collect::<Vec<_>>();
        let reusable_source_assignment_count = assignments
            .iter()
            .filter(|assignment| assignment.source_glyph_reused)
            .count();
        let global_name_assignment_reuse_count = assignments
            .iter()
            .filter(|assignment| assignment.global_name_glyph_reused)
            .count();
        let replaced_fixed_cell_count = assignments
            .iter()
            .filter(|assignment| {
                assignment.requires_glyph_install && assignment.target_was_fixed_source_cell
            })
            .count();
        let extension_cell_install_count = assignments
            .iter()
            .filter(|assignment| {
                assignment.requires_glyph_install && !assignment.target_was_fixed_source_cell
            })
            .count();
        assets.push(DialogueCodeAllocationAsset {
            source_path: source.path,
            fixed_cell_count: atlas.fixed_cell_count,
            addressable_code_count: atlas.addressable_slot_count,
            protected_source_glyph_code_count: protected_source_codes.len(),
            translated_coordinate_count: demand.translated_coordinate_count,
            static_character_count: demand.static_characters.len(),
            known_runtime_character_count: runtime.characters.len(),
            required_character_count: required_characters.len(),
            reusable_source_assignment_count,
            global_name_assignment_reuse_count,
            replaced_fixed_cell_count,
            extension_cell_install_count,
            unused_asset_local_code_count: ASSET_LOCAL_CODE_COUNT
                - assignments
                    .iter()
                    .filter(|assignment| {
                        parse_hex_code(&assignment.code)
                            .is_ok_and(|code| usize::from(code) < ASSET_LOCAL_CODE_COUNT)
                    })
                    .count(),
            unresolved_runtime_sources: runtime.unresolved_sources.into_iter().collect(),
            assignments,
        });
    }
    ensure!(
        demand_by_asset.is_empty(),
        "translation demand contains an unknown dialogue asset"
    );

    let authored_semantic_hashes = translated_segments.keys().cloned().collect::<BTreeSet<_>>();
    let fixed_code_consumers = audit_fixed_code_consumers(DialogueFixedCodeConsumerInputs {
        corpus,
        primary_referenced_coordinates: referenced_coordinates,
        authored_semantic_hashes: &authored_semantic_hashes,
        runtime_insertion_semantic_hashes: &runtime_insertion_semantic_hashes,
        controls_by_semantic_hash: &controls_by_semantic_hash,
        selector_translation_population_complete: selector_translation
            .development_full_selector_input_available,
        primary_translation_population_complete: translation
            .development_translation_input_available,
        expected_source_coordinate_count: match *source_asset_count {
            10 => 10_156,
            78 => 62_639,
            _ => unreachable!(),
        },
        expected_primary_referenced_coordinate_count: match *source_asset_count {
            10 => 7_049,
            78 => 40_026,
            _ => unreachable!(),
        },
        name_glyph_layout: &name_glyph_layout,
        allocation_assets: &assets,
    })?;
    let fixed_code_consumer_ownership_complete =
        fixed_code_consumers.fixed_code_consumer_ownership_complete;

    let report = DialogueCodeAllocationReport {
        kind: "Justice Gakuen 2 development dialogue code allocation".to_string(),
        source_bin_sha256: translation.source_bin_sha256.clone(),
        codebook_sha256,
        input_policy: config.input_policy.label().to_string(),
        semantic_group_count: translation.semantic_group_count,
        development_authored_group_count,
        selector_semantic_group_count: selector_translation.semantic_group_count,
        selector_development_authored_group_count: selector_translation
            .development_authored_group_count,
        selector_development_full_input_available: selector_translation
            .development_full_selector_input_available,
        release_candidate_selector_input_eligible: selector_translation
            .release_candidate_selector_input_eligible,
        development_build_input_available: true,
        development_translation_input_available: translation
            .development_translation_input_available,
        release_candidate_translation_input_eligible: translation
            .release_candidate_translation_input_eligible,
        addressable_code_count: ADDRESSABLE_CODE_COUNT,
        asset_local_code_count: ASSET_LOCAL_CODE_COUNT,
        global_name_code_start: hex_code(u16::try_from(ASSET_LOCAL_CODE_COUNT)?),
        global_name_code_end: hex_code(u16::try_from(ADDRESSABLE_CODE_COUNT - 1)?),
        global_name_code_count: NAME_GLYPH_CODE_COUNT,
        name_input_keyboard_sha256: name_input_keyboard.spec_sha256,
        name_glyph_layout_complete: true,
        name_glyph_layout_release_candidate_eligible: false,
        name_glyph_layout,
        direct_selector_consumers,
        fixed_code_consumers,
        fixed_code_consumer_ownership_complete,
        static_allocation_complete: true,
        assets,
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

pub(super) fn allocate_character_codes(
    required_characters: &BTreeSet<char>,
    reusable_codes: &BTreeMap<char, Vec<u16>>,
    protected_source_codes: &BTreeSet<u16>,
    asset_local_code_count: usize,
    addressable_code_count: usize,
) -> Result<BTreeMap<char, AllocatedCode>> {
    ensure!(
        asset_local_code_count <= addressable_code_count
            && required_characters.len() <= addressable_code_count,
        "dialogue character demand exceeds addressable code capacity"
    );
    let mut result = BTreeMap::new();
    let mut used_codes = BTreeSet::new();
    for character in required_characters {
        let mut candidates = reusable_codes.get(character).into_iter().flatten().copied();
        let admissible =
            |code: &u16| usize::from(*code) < addressable_code_count && !used_codes.contains(code);
        let Some(code) = candidates
            .clone()
            .find(|code| protected_source_codes.contains(code) && admissible(code))
            .or_else(|| candidates.find(admissible))
        else {
            continue;
        };
        used_codes.insert(code);
        result.insert(
            *character,
            AllocatedCode {
                code,
                source_glyph_reused: true,
            },
        );
    }
    let unprotected_free_codes = (0..asset_local_code_count)
        .map(u16::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|code| !used_codes.contains(code) && !protected_source_codes.contains(code));
    let protected_free_codes = protected_source_codes
        .iter()
        .copied()
        .filter(|code| usize::from(*code) < asset_local_code_count && !used_codes.contains(code));
    let mut free_codes = unprotected_free_codes.chain(protected_free_codes);
    for character in required_characters {
        if result.contains_key(character) {
            continue;
        }
        let code = free_codes
            .next()
            .context("asset-local dialogue code allocation exhausted")?;
        result.insert(
            *character,
            AllocatedCode {
                code,
                source_glyph_reused: false,
            },
        );
    }
    Ok(result)
}

fn preserved_fixed_source_codes(
    decoded: &[u8],
    corpus_asset: &DialogueCorpusAsset,
    authored_segments: &BTreeMap<String, Vec<String>>,
    runtime_insertion_semantic_hashes: &BTreeSet<String>,
    primary_referenced_coordinates: &BTreeSet<String>,
    fixed_cell_count: usize,
    addressable_slot_count: usize,
) -> Result<BTreeSet<u16>> {
    let banks = parse_dialogue_banks(decoded)?;
    ensure!(
        banks.len() == corpus_asset.banks.len(),
        "dialogue corpus bank count changed while protecting source glyphs"
    );
    let mut protected = BTreeSet::new();
    for (bank, corpus_bank) in banks.iter().zip(&corpus_asset.banks) {
        ensure!(
            bank.selector_index == corpus_bank.selector_index
                && bank.messages.len() == corpus_bank.entries.len(),
            "dialogue corpus population changed while protecting source glyphs"
        );
        for (message, entry) in bank.messages.iter().zip(&corpus_bank.entries) {
            if authored_segments.contains_key(&entry.semantic_source_sha256)
                || runtime_insertion_semantic_hashes.contains(&entry.semantic_source_sha256)
            {
                continue;
            }
            if bank.selector_index < FIRST_SELECTOR_TRANSLATION_BANK
                && !primary_referenced_coordinates.contains(&entry.coordinate_id)
            {
                continue;
            }
            let raw_words = message
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            protected.extend(
                tokenize_dialogue_message(&raw_words, fixed_cell_count, addressable_slot_count)?
                    .tokens
                    .into_iter()
                    .filter(|token| token.kind == DialogueTokenKind::FixedGlyph)
                    .map(|token| token.code),
            );
        }
    }
    Ok(protected)
}

fn reusable_source_codes(
    fixed_cell_sha256: &[String],
    resolved_glyphs: &BTreeMap<&str, &str>,
) -> BTreeMap<char, Vec<u16>> {
    let mut result = BTreeMap::<char, Vec<u16>>::new();
    for (code, hash) in fixed_cell_sha256.iter().enumerate() {
        let Some(character) = resolved_glyphs
            .get(hash.as_str())
            .and_then(|text| one_character(text))
        else {
            continue;
        };
        if let Ok(code) = u16::try_from(code) {
            result.entry(character).or_default().push(code);
        }
    }
    result
}

fn one_character(text: &str) -> Option<char> {
    let mut characters = text.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

fn parse_hex_code(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .context("dialogue code lacks 0x prefix")?;
    Ok(u16::from_str_radix(digits, 16)?)
}

fn write_report(path: &Path, report: &DialogueCodeAllocationReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
