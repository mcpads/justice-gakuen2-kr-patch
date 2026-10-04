use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::text::read_length_prefixed_codes;

use super::description_assets::{audit_description_assets, load_description_assets};
use super::menu_textures::{detect_four_bit_tim_regions, inspect_options_menu_textures};
use super::model::{
    OptionsAssetAuditConfig, OptionsAssetAuditReport, OptionsAssetReference, OptionsAuthoredUnit,
    OptionsDevelopmentStatus, OptionsFontRole, OptionsManifest, OptionsReleaseStatus,
    OptionsTranslationUnit,
};
use super::source::{MENU_PATH, OVERLAY_PATH, OVERLAY_RUNTIME_BASE, load_options_source};
use super::source_catalog::catalog_newopt_pointer_records;

const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 options-screen translation manifest";
const TRANSLATION_KIND: &str = "Justice Gakuen 2 options-screen translation unit";
pub(super) const REQUIRED_OPTIONS_BUILD_IDS: [&str; 133] = [
    "heading",
    "navigation_help",
    "return_to_mode_menu",
    "cpu_difficulty",
    "attack_power",
    "timer_speed",
    "cpu_match_count_suffix",
    "versus_match_count_suffix",
    "cpu_round_character_change",
    "continue_character_change",
    "vibration",
    "sound_mode",
    "shortcut",
    "game_system_change",
    "key_control_change",
    "match",
    "mono",
    "stereo",
    "disabled",
    "enabled",
    "none",
    "available",
    "stop",
    "defaults",
    "exit",
    "key_control_heading",
    "game_system_heading",
    "player_one",
    "player_two",
    "no_assignment",
    "weak_punch",
    "strong_punch",
    "weak_kick",
    "strong_kick",
    "complete_burn_attack_1",
    "complete_burn_attack_2",
    "complete_burn_attack_3",
    "complete_burn_attack_4",
    "axis_move",
    "throw",
    "two_platoon",
    "breakfall",
    "spirit_gauge",
    "guard",
    "back_dash",
    "back_jump",
    "normal",
    "automatic",
    "records_heading",
    "records_load",
    "records_save",
    "records_automatic",
    "records_no",
    "records_yes",
    "records_load_confirmation",
    "records_save_confirmation",
    "records_horizontal_help",
    "records_vertical_help",
    "records_await_input",
    "records_save_complete",
    "records_load_complete",
    "records_data_error",
    "records_memory_card_not_inserted",
    "records_memory_card_corrupt",
    "records_data_file_missing",
    "records_memory_card_full",
    "records_memory_card_unformatted",
    "records_initialize_confirmation",
    "records_save_failed",
    "records_memory_card_check",
    "records_saving",
    "records_keep_memory_card_inserted",
    "records_overwrite_confirmation",
    "return_to_options",
    "sound_test_heading",
    "bgm_test",
    "sound_effect_test",
    "voice_test",
    "xa_test",
    "bonus_heading",
    "movies",
    "gallery_collection",
    "gallery_batsu",
    "gallery_hinata",
    "gallery_kyosuke",
    "gallery_shoma",
    "gallery_natsu",
    "gallery_roberto",
    "gallery_roy",
    "gallery_tiffany",
    "gallery_bowman",
    "gallery_edge",
    "gallery_akira",
    "gallery_gan",
    "gallery_hideo",
    "gallery_kyoko",
    "gallery_raizo",
    "gallery_hyo",
    "gallery_akira_alternate",
    "gallery_sakura",
    "gallery_daigo",
    "gallery_hayato",
    "initialize_failed",
    "conditional",
    "landing_only",
    "gauge_start_five",
    "gauge_start_nine",
    "gauge_start_zero",
    "gauge_maximum_fixed",
    "gauge_zero_fixed",
    "manual",
    "small",
    "play",
    "save_block_required",
    "spectator_mode",
    "ending",
    "staff_roll",
    "automatic_load_skipped",
    "load_failed",
    "ending_good",
    "ending_bad",
    "edited_character",
    "no_data",
    "button_prefix",
    "school_song",
    "school_taiyo",
    "school_gorin",
    "school_pacific",
    "school_gedo",
    "school_justice",
    "diary_title",
    "memory_suffix",
    "initialize_complete",
];

pub fn audit_options_assets(config: &OptionsAssetAuditConfig) -> Result<OptionsAssetAuditReport> {
    let source = load_options_source(&config.cue)?;

    let loaded = load_assets(
        &config.assets,
        &source.source_bin_sha256,
        &source.overlay,
        &source.menu_stored,
        &source.menu_decoded,
    )?;
    let source_catalog = catalog_newopt_pointer_records(&source.overlay)?;
    let descriptions = load_description_assets(&config.assets, &source)?;
    validate_complete_source_population(&loaded.units, &source_catalog)?;
    let tracked_offsets = loaded
        .units
        .iter()
        .map(|unit| parse_hex_usize(&unit.source_offset, "source offset"))
        .collect::<Result<BTreeSet<_>>>()?;
    let catalog_offsets = source_catalog
        .records
        .iter()
        .map(|record| record.source_offset)
        .collect::<BTreeSet<_>>();
    ensure!(
        tracked_offsets.is_subset(&catalog_offsets),
        "tracked options unit is outside the NEWOPT pointer-backed source population"
    );
    let untracked_source_record_offsets = catalog_offsets
        .difference(&tracked_offsets)
        .map(|offset| format!("0x{offset:04x}"))
        .collect::<Vec<_>>();
    let authored_unit_count = loaded
        .units
        .iter()
        .filter(|unit| unit.development_status == OptionsDevelopmentStatus::Authored)
        .count();
    let untranslated_unit_count = loaded.units.len() - authored_unit_count;
    let release_approved_unit_count = loaded
        .units
        .iter()
        .filter(|unit| unit.release_status == OptionsReleaseStatus::Approved)
        .count();
    let required_korean_characters = loaded
        .units
        .iter()
        .filter_map(|unit| unit.korean_text.as_deref())
        .flat_map(str::chars)
        .filter(|character| ('가'..='힣').contains(character))
        .collect::<BTreeSet<_>>();
    let development_asset_input_available = development_build_is_authored(&loaded.units);
    let menu_texture_regions = inspect_options_menu_textures(&source.menu_decoded)?;
    let detected_four_bit_tim_regions = detect_four_bit_tim_regions(&source.menu_decoded);
    let report = OptionsAssetAuditReport {
        kind: "Justice Gakuen 2 options-screen translation asset audit".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        overlay_path: OVERLAY_PATH.to_string(),
        overlay_sha256: sha256_bytes(&source.overlay),
        menu_path: MENU_PATH.to_string(),
        menu_stored_sha256: sha256_bytes(&source.menu_stored),
        menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        manifest_sha256: loaded.manifest_sha256,
        unit_count: loaded.units.len(),
        untranslated_unit_count,
        authored_unit_count,
        release_approved_unit_count,
        source_records_match: true,
        source_pointer_bindings_match: true,
        menu_texture_regions,
        detected_four_bit_tim_regions,
        development_asset_input_available,
        release_candidate_input_eligible: release_approved_unit_count == loaded.units.len(),
        newopt_pointer_slot_count: source_catalog.pointer_slot_count,
        newopt_unique_source_record_count: source_catalog.records.len(),
        tracked_source_record_count: tracked_offsets.len(),
        untracked_source_record_count: untracked_source_record_offsets.len(),
        untracked_source_record_offsets,
        required_korean_character_count: required_korean_characters.len(),
        required_korean_characters: required_korean_characters.into_iter().collect(),
        descriptions: audit_description_assets(&descriptions),
    };
    if let Some(parent) = config.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

pub(super) struct LoadedOptionsAssets {
    pub(super) manifest_sha256: String,
    pub(super) references: Vec<OptionsAssetReference>,
    pub(super) units: Vec<OptionsTranslationUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source_bin_sha256: &str,
    overlay: &[u8],
    menu_stored: &[u8],
    menu_decoded: &[u8],
) -> Result<LoadedOptionsAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: OptionsManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown options manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source_bin_sha256,
        "options manifest source BIN changed"
    );
    ensure!(
        manifest.overlay_path == OVERLAY_PATH && manifest.overlay_sha256 == sha256_bytes(overlay),
        "options manifest NEWOPT.BIN identity changed"
    );
    ensure!(
        manifest.menu_path == MENU_PATH
            && manifest.menu_stored_sha256 == sha256_bytes(menu_stored)
            && manifest.menu_decoded_sha256 == sha256_bytes(menu_decoded),
        "options manifest MENU.BIZ identity changed"
    );

    let manifest_ids = manifest
        .units
        .iter()
        .map(|reference| reference.id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        manifest.units.len() == manifest_ids.len(),
        "duplicate options id"
    );

    let mut source_offsets = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut units = Vec::with_capacity(manifest.units.len());
    for reference in &manifest.units {
        validate_relative_file(&reference.file)?;
        ensure!(
            files.insert(reference.file.clone()),
            "duplicate options file"
        );
        let path = root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: OptionsTranslationUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == TRANSLATION_KIND && unit.id == reference.id,
            "options unit identity differs from its manifest reference"
        );
        validate_translation_state(&unit)?;
        let source_offset = parse_hex_usize(&unit.source_offset, "source offset")?;
        ensure!(
            source_offsets.insert(source_offset),
            "duplicate options source offset"
        );
        validate_source_binding(overlay, source_offset, &unit)?;
        units.push(unit);
    }
    Ok(LoadedOptionsAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        references: manifest.units,
        units,
    })
}

pub(super) fn select_authored_options_build_units(
    units: &[OptionsTranslationUnit],
) -> Result<Vec<OptionsAuthoredUnit>> {
    let by_id = units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<std::collections::BTreeMap<_, _>>();
    REQUIRED_OPTIONS_BUILD_IDS
        .iter()
        .map(|id| {
            let unit = by_id
                .get(id)
                .with_context(|| format!("missing options build unit {id}"))?;
            ensure!(
                unit.development_status == OptionsDevelopmentStatus::Authored,
                "options build unit {id} is not authored"
            );
            let font_role = unit
                .font_role
                .with_context(|| format!("options build unit {id} has no font role"))?;
            ensure!(
                font_role == expected_font_role(id)?,
                "options {id} font role changed"
            );
            Ok(OptionsAuthoredUnit {
                id: unit.id.clone(),
                source_offset: unit.source_offset.clone(),
                pointer_offsets: unit.pointer_offsets.clone(),
                source_codes: unit.source_codes.clone(),
                source_text: unit
                    .source_text
                    .clone()
                    .with_context(|| format!("options build unit {id} has no source text"))?,
                korean_text: unit
                    .korean_text
                    .clone()
                    .with_context(|| format!("options build unit {id} has no Korean text"))?,
                font_role,
                release_status: unit.release_status,
            })
        })
        .collect()
}

fn development_build_is_authored(units: &[OptionsTranslationUnit]) -> bool {
    select_authored_options_build_units(units).is_ok()
}

pub(super) fn validate_translation_state(unit: &OptionsTranslationUnit) -> Result<()> {
    match unit.development_status {
        OptionsDevelopmentStatus::Untranslated => ensure!(
            unit.source_text.is_none()
                && unit.korean_text.is_none()
                && unit.font_role.is_none()
                && unit.release_status == OptionsReleaseStatus::Untranslated,
            "untranslated options {} contains authored fields",
            unit.id
        ),
        OptionsDevelopmentStatus::Authored => {
            let source_text = unit
                .source_text
                .as_deref()
                .with_context(|| format!("options {} lacks source text", unit.id))?;
            let korean_text = unit
                .korean_text
                .as_deref()
                .with_context(|| format!("options {} lacks Korean text", unit.id))?;
            ensure!(
                !source_text.trim().is_empty()
                    && !korean_text.trim().is_empty()
                    && unit.font_role.is_some()
                    && unit.release_status != OptionsReleaseStatus::Untranslated,
                "authored options {} has incomplete translation fields",
                unit.id
            );
        }
    }
    Ok(())
}

pub(super) fn validate_complete_source_population(
    units: &[OptionsTranslationUnit],
    source_catalog: &super::source_catalog::NewoptSourceCatalog,
) -> Result<()> {
    ensure!(
        source_catalog.pointer_slot_count == 148 && source_catalog.records.len() == 135,
        "NEWOPT pointer-backed source population changed"
    );
    let by_offset = units
        .iter()
        .map(|unit| Ok((parse_hex_usize(&unit.source_offset, "source offset")?, unit)))
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    ensure!(
        by_offset.len() == source_catalog.records.len(),
        "options assets do not cover all 135 unique NEWOPT source records"
    );
    for record in &source_catalog.records {
        let unit = by_offset.get(&record.source_offset).with_context(|| {
            format!(
                "missing options source asset for NEWOPT +0x{:04x}",
                record.source_offset
            )
        })?;
        let pointer_offsets = unit
            .pointer_offsets
            .iter()
            .map(|offset| parse_hex_usize(offset, "pointer offset"))
            .collect::<Result<BTreeSet<_>>>()?;
        ensure!(
            pointer_offsets == record.pointer_offsets.iter().copied().collect(),
            "options {} does not preserve every pointer reference",
            unit.id
        );
    }
    Ok(())
}

fn expected_font_role(id: &str) -> Result<OptionsFontRole> {
    Ok(match id {
        "sound_test_heading" | "bonus_heading" => OptionsFontRole::Heading,
        "return_to_options"
        | "bgm_test"
        | "sound_effect_test"
        | "voice_test"
        | "xa_test"
        | "movies"
        | "gallery_collection"
        | "gallery_batsu"
        | "gallery_hinata"
        | "gallery_kyosuke"
        | "gallery_shoma"
        | "gallery_natsu"
        | "gallery_roberto"
        | "gallery_roy"
        | "gallery_tiffany"
        | "gallery_bowman"
        | "gallery_edge"
        | "gallery_akira"
        | "gallery_gan"
        | "gallery_hideo"
        | "gallery_kyoko"
        | "gallery_raizo"
        | "gallery_hyo"
        | "gallery_akira_alternate"
        | "gallery_sakura"
        | "gallery_daigo"
        | "gallery_hayato"
        | "initialize_failed"
        | "conditional"
        | "landing_only"
        | "gauge_start_five"
        | "gauge_start_nine"
        | "gauge_start_zero"
        | "gauge_maximum_fixed"
        | "gauge_zero_fixed"
        | "manual"
        | "small"
        | "play"
        | "save_block_required"
        | "spectator_mode"
        | "ending"
        | "staff_roll"
        | "automatic_load_skipped"
        | "load_failed"
        | "ending_good"
        | "ending_bad"
        | "edited_character"
        | "no_data"
        | "button_prefix"
        | "school_song"
        | "school_taiyo"
        | "school_gorin"
        | "school_pacific"
        | "school_gedo"
        | "school_justice"
        | "diary_title"
        | "memory_suffix"
        | "initialize_complete" => OptionsFontRole::Label,
        "heading" | "key_control_heading" | "game_system_heading" => OptionsFontRole::Heading,
        "records_heading" => OptionsFontRole::RecordsMainHeading,
        "records_load" | "records_save" | "records_automatic" => OptionsFontRole::RecordsMainItem,
        "records_no"
        | "records_yes"
        | "records_load_confirmation"
        | "records_save_confirmation"
        | "records_initialize_confirmation"
        | "records_overwrite_confirmation" => OptionsFontRole::RecordsPrompt,
        "records_memory_card_check" => OptionsFontRole::RecordsStatusHeading,
        "records_await_input"
        | "records_save_complete"
        | "records_load_complete"
        | "records_data_error"
        | "records_memory_card_not_inserted"
        | "records_memory_card_corrupt"
        | "records_data_file_missing"
        | "records_memory_card_full"
        | "records_memory_card_unformatted"
        | "records_save_failed"
        | "records_saving"
        | "records_keep_memory_card_inserted" => OptionsFontRole::RecordsStatusMessage,
        "navigation_help"
        | "return_to_mode_menu"
        | "records_horizontal_help"
        | "records_vertical_help" => OptionsFontRole::Help,
        "cpu_difficulty"
        | "attack_power"
        | "timer_speed"
        | "cpu_match_count_suffix"
        | "versus_match_count_suffix"
        | "cpu_round_character_change"
        | "continue_character_change"
        | "vibration"
        | "sound_mode"
        | "shortcut"
        | "game_system_change"
        | "key_control_change"
        | "player_one"
        | "player_two"
        | "breakfall"
        | "spirit_gauge"
        | "guard"
        | "back_dash"
        | "back_jump" => OptionsFontRole::Label,
        "match"
        | "mono"
        | "stereo"
        | "disabled"
        | "enabled"
        | "none"
        | "available"
        | "stop"
        | "no_assignment"
        | "weak_punch"
        | "strong_punch"
        | "weak_kick"
        | "strong_kick"
        | "complete_burn_attack_1"
        | "complete_burn_attack_2"
        | "complete_burn_attack_3"
        | "complete_burn_attack_4"
        | "axis_move"
        | "throw"
        | "two_platoon"
        | "normal"
        | "automatic" => OptionsFontRole::Value,
        "defaults" | "exit" => OptionsFontRole::Action,
        _ => anyhow::bail!("unknown options unit id {id}"),
    })
}

pub(super) fn validate_source_binding(
    overlay: &[u8],
    source_offset: usize,
    unit: &OptionsTranslationUnit,
) -> Result<()> {
    let source_codes = unit
        .source_codes
        .iter()
        .map(|code| parse_hex_u16(code, "source code"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        read_length_prefixed_codes(overlay, source_offset)? == source_codes,
        "options {} source glyph codes changed",
        unit.id
    );
    ensure!(
        !unit.pointer_offsets.is_empty(),
        "options {} has no source pointer binding",
        unit.id
    );
    let expected_pointer = OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(source_offset)?)
        .context("options source pointer overflow")?;
    let mut pointer_offsets = BTreeSet::new();
    for pointer_offset in &unit.pointer_offsets {
        let pointer_offset = parse_hex_usize(pointer_offset, "pointer offset")?;
        ensure!(
            pointer_offsets.insert(pointer_offset),
            "options {} repeats a source pointer offset",
            unit.id
        );
        let bytes = overlay
            .get(pointer_offset..pointer_offset + 4)
            .context("options source pointer is out of bounds")?;
        ensure!(
            u32::from_le_bytes(bytes.try_into().unwrap()) == expected_pointer,
            "options {} source pointer changed at +0x{pointer_offset:04x}",
            unit.id
        );
    }
    Ok(())
}

pub(super) fn parse_hex_usize(value: &str, role: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .with_context(|| format!("options {role} lacks 0x prefix"))?,
        16,
    )
    .with_context(|| format!("options {role} is not hexadecimal"))
}

pub(super) fn parse_hex_u16(value: &str, role: &str) -> Result<u16> {
    u16::try_from(parse_hex_usize(value, role)?)
        .with_context(|| format!("options {role} exceeds u16"))
}

fn validate_relative_file(file: &str) -> Result<()> {
    let path = Path::new(file);
    ensure!(
        !file.is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "options manifest contains an unsafe file path"
    );
    Ok(())
}
