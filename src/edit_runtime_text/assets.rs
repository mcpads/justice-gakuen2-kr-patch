use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes};
use crate::text::read_length_prefixed_codes;
use crate::tim::parse_4bpp_without_clut_prefix;

use super::model::{
    DevelopmentStatus, EditRuntimeDirectGlyphTranslation, EditRuntimeFontManifest,
    EditRuntimeTextManifest, EditRuntimeTextTranslation, PassFixedPresentationTextEntry,
    PassFixedPresentationTextManifest, ReleaseStatus,
};
use super::source::{
    EDIT_SHARED_UI_DECODED_SHA256, EDIT_SHARED_UI_PATH, EDIT_SHARED_UI_STORED_SHA256,
    EditRuntimeTextSource, OVERLAY_PATH, OVERLAY_RUNTIME_BASE, OVERLAY_SHA256,
};

const MANIFEST_PATH: &str = "edit/runtime-text/manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 EDIT runtime text manifest";
const TRANSLATION_KIND: &str = "Justice Gakuen 2 EDIT runtime text unit";
const DIRECT_GLYPH_TRANSLATION_KIND: &str = "Justice Gakuen 2 EDIT runtime direct glyph unit";
const PASS_FIXED_PRESENTATION_TEXT_KIND: &str =
    "Justice Gakuen 2 PASS fixed-presentation semantic text";
const RENDERER_GLYPH_ADVANCE_PX: i16 = super::button_layout::SOURCE_GLYPH_ADVANCE;
const RUNTIME_FONT_TIM_OFFSET: usize = 0x21000;
const RUNTIME_FONT_TIM_SHA256: &str =
    "45b8198fde1bbe5db78c964f55259c1be5abe497f281503510ab6f3cbd6657d6";
const RUNTIME_FONT_CODES: [u16; 12] = [
    0xff20, 0xff21, 0xff22, 0xff23, 0xff24, 0xff25, 0xff26, 0xff27, 0xff28, 0xff29, 0xff2a, 0xff2b,
];
const EXPECTED_IDS: [&str; 55] = [
    "character_registration",
    "load",
    "register",
    "password_input",
    "exit",
    "not_loaded",
    "loading_progress",
    "memory_card_subject",
    "memory_card_not_inserted",
    "memory_card_uninitialized",
    "memory_card_damaged",
    "press_any_button",
    "load_confirmation",
    "load_confirmation_choices",
    "health_gauge",
    "spirit_gauge",
    "attack_power",
    "defense_power",
    "spirit",
    "friendly_character",
    "poor_match_character",
    "edit_data_absent",
    "mode_save_data_subject",
    "save_data_absent",
    "cpu_edit",
    "status_tab",
    "command_tab",
    "password_display",
    "unregister",
    "back",
    "decision_legend",
    "select_character_subject",
    "select_prompt",
    "cpu_edit_mode_prefix",
    "edit_character_performance_object",
    "configure_action",
    "edit_character_status_subject",
    "confirm_action",
    "command_list_subject",
    "edit_character_status_action",
    "edit_character_data_object",
    "password_display_action",
    "registered_character_subject",
    "delete_data_action",
    "return_to_main_command",
    "proceed_confirmation",
    "name_label",
    "school",
    "sex_label",
    "friendly_character_profile",
    "status_rating_very_low",
    "status_rating_low",
    "status_rating_normal",
    "status_rating_high",
    "status_rating_very_high",
];
const EXPECTED_DIRECT_GLYPH_IDS: [&str; 1] = ["status_overview_spirit_gauge_unit"];
const EXPECTED_PASS_FIXED_PRESENTATION_TEXT_IDS: [&str; 16] = [
    "pass_cpu_skill_usage",
    "pass_cpu_password_input",
    "pass_cpu_password_input_action",
    "pass_cpu_male",
    "pass_cpu_female",
    "pass_password_submit_instruction",
    "pass_password_cancel_instruction",
    "pass_password_decision",
    "pass_invalid_password",
    "pass_password_title",
    "pass_registration_choices",
    "pass_character_acceptance_controls",
    "pass_registration_complete",
    "pass_register_another_choices",
    "pass_cpu_registration_confirmation",
    "pass_register_another_question",
];
const FIRST_STABLE_ACTION_IDS: [&str; 5] = [
    "character_registration",
    "load",
    "register",
    "password_input",
    "exit",
];

pub(super) struct LoadedEditRuntimeTextAssets {
    pub(super) manifest_sha256: String,
    pub(super) renderer_glyph_advance_px: i16,
    pub(super) runtime_font: EditRuntimeFontManifest,
    pub(super) entries: Vec<EditRuntimeTextTranslation>,
    pub(super) direct_glyph_entries: Vec<EditRuntimeDirectGlyphTranslation>,
    pub(super) pass_fixed_presentation_text_sha256: String,
    pub(super) pass_fixed_presentation_text_entries: Vec<PassFixedPresentationTextEntry>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &EditRuntimeTextSource,
) -> Result<LoadedEditRuntimeTextAssets> {
    let manifest_path = root.join(MANIFEST_PATH);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: EditRuntimeTextManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown EDIT runtime text manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == BASELINE_BIN_SHA256
            && manifest.source_bin_sha256 == source.source_bin_sha256,
        "EDIT runtime text manifest source BIN changed"
    );
    ensure!(
        manifest.overlay_path == OVERLAY_PATH && manifest.overlay_sha256 == OVERLAY_SHA256,
        "EDIT runtime text manifest KANRI.BIN identity changed"
    );
    validate_runtime_font(&manifest.runtime_font, source)?;
    ensure!(
        manifest.renderer_glyph_advance_px == RENDERER_GLYPH_ADVANCE_PX,
        "EDIT runtime text renderer advance changed"
    );
    ensure!(
        manifest.entries.len() == EXPECTED_IDS.len(),
        "EDIT runtime text manifest must keep every observed entry"
    );
    ensure!(
        manifest.direct_glyph_entries.len() == EXPECTED_DIRECT_GLYPH_IDS.len(),
        "EDIT runtime text manifest must keep every observed direct-glyph entry"
    );

    let manifest_root = manifest_path
        .parent()
        .context("EDIT runtime text manifest has no parent")?;
    let mut ids = BTreeSet::new();
    let mut record_ranges = Vec::new();
    let mut placement_ranges = Vec::new();
    let mut entries = Vec::with_capacity(manifest.entries.len());
    for (index, reference) in manifest.entries.into_iter().enumerate() {
        ensure!(
            reference.id == EXPECTED_IDS[index] && ids.insert(reference.id.clone()),
            "EDIT runtime text entry order or identity changed"
        );
        validate_relative_file(&reference.file)?;
        let path = manifest_root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let entry: EditRuntimeTextTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            entry.kind == TRANSLATION_KIND && entry.id == reference.id,
            "EDIT runtime text translation identity changed"
        );
        validate_entry(&entry, source)?;
        let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        record_ranges.push([source_offset, source_offset + entry.source_record_size]);
        let placement_offset = parse_hex_usize(&entry.placement_record_offset, "placement offset")?;
        placement_ranges.push([placement_offset, placement_offset + 12]);
        entries.push(entry);
    }
    record_ranges.sort();
    placement_ranges.sort();
    ensure!(
        record_ranges
            .windows(2)
            .all(|pair| pair[0][1] <= pair[1][0])
            && placement_ranges
                .windows(2)
                .all(|pair| pair[0][1] <= pair[1][0]),
        "EDIT runtime text records overlap"
    );
    validate_first_stable_action_coverage(&entries)?;
    validate_contextual_code_ownership(&source.overlay, &entries)?;
    let mut direct_glyph_entries = Vec::with_capacity(manifest.direct_glyph_entries.len());
    for (index, reference) in manifest.direct_glyph_entries.into_iter().enumerate() {
        ensure!(
            reference.id == EXPECTED_DIRECT_GLYPH_IDS[index] && ids.insert(reference.id.clone()),
            "EDIT runtime direct-glyph entry order or identity changed"
        );
        validate_relative_file(&reference.file)?;
        let path = manifest_root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let entry: EditRuntimeDirectGlyphTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            entry.kind == DIRECT_GLYPH_TRANSLATION_KIND && entry.id == reference.id,
            "EDIT runtime direct-glyph translation identity changed"
        );
        validate_direct_glyph_entry(&entry)?;
        direct_glyph_entries.push(entry);
    }
    validate_relative_file(&manifest.pass_fixed_presentation_text)?;
    let pass_fixed_presentation_text_path =
        manifest_root.join(&manifest.pass_fixed_presentation_text);
    let pass_fixed_presentation_text_bytes = std::fs::read(&pass_fixed_presentation_text_path)
        .with_context(|| {
            format!(
                "failed to read {}",
                pass_fixed_presentation_text_path.display()
            )
        })?;
    let pass_fixed_presentation_text: PassFixedPresentationTextManifest =
        serde_json::from_slice(&pass_fixed_presentation_text_bytes).with_context(|| {
            format!(
                "failed to parse {}",
                pass_fixed_presentation_text_path.display()
            )
        })?;
    ensure!(
        pass_fixed_presentation_text.kind == PASS_FIXED_PRESENTATION_TEXT_KIND
            && pass_fixed_presentation_text.entries.len()
                == EXPECTED_PASS_FIXED_PRESENTATION_TEXT_IDS.len(),
        "PASS fixed-presentation semantic text inventory changed"
    );
    for (index, entry) in pass_fixed_presentation_text.entries.iter().enumerate() {
        ensure!(
            entry.id == EXPECTED_PASS_FIXED_PRESENTATION_TEXT_IDS[index]
                && ids.insert(entry.id.clone())
                && !entry.korean_text.is_empty()
                && entry.development_status == DevelopmentStatus::Authored
                && entry.release_status != ReleaseStatus::Untranslated,
            "PASS fixed-presentation semantic text entry changed identity, text, or review state"
        );
    }
    Ok(LoadedEditRuntimeTextAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        renderer_glyph_advance_px: manifest.renderer_glyph_advance_px,
        runtime_font: manifest.runtime_font,
        entries,
        direct_glyph_entries,
        pass_fixed_presentation_text_sha256: sha256_bytes(&pass_fixed_presentation_text_bytes),
        pass_fixed_presentation_text_entries: pass_fixed_presentation_text.entries,
    })
}

fn validate_direct_glyph_entry(entry: &EditRuntimeDirectGlyphTranslation) -> Result<()> {
    parse_hex_usize(
        &entry.source_instruction_offset,
        "direct-glyph source instruction offset",
    )?;
    parse_hex_u16(&entry.source_code, "direct-glyph source code")?;
    ensure!(
        !entry.source_text.is_empty()
            && entry.korean_text.chars().count() == 1
            && entry.development_status == DevelopmentStatus::Authored
            && entry.release_status != ReleaseStatus::Untranslated,
        "authored EDIT runtime direct-glyph {} lacks one source-bound output glyph or review state",
        entry.id
    );
    Ok(())
}

fn validate_first_stable_action_coverage(entries: &[EditRuntimeTextTranslation]) -> Result<()> {
    let untranslated = entries
        .iter()
        .filter(|entry| FIRST_STABLE_ACTION_IDS.contains(&entry.id.as_str()))
        .filter(|entry| entry.development_status != DevelopmentStatus::Authored)
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    ensure!(
        untranslated.is_empty(),
        "EDIT first-stable actions remain untranslated: {}",
        untranslated.join(", ")
    );
    Ok(())
}

fn validate_runtime_font(
    font: &EditRuntimeFontManifest,
    source: &EditRuntimeTextSource,
) -> Result<()> {
    ensure!(
        font.path == EDIT_SHARED_UI_PATH
            && font.stored_sha256 == EDIT_SHARED_UI_STORED_SHA256
            && font.decoded_sha256 == EDIT_SHARED_UI_DECODED_SHA256,
        "EDIT runtime font source identity changed"
    );
    let tim_offset = parse_hex_usize(&font.tim_offset, "runtime font TIM offset")?;
    let tim = parse_4bpp_without_clut_prefix(
        source
            .edit_shared_ui_decoded
            .get(tim_offset..)
            .context("EDIT runtime font TIM offset is outside EDITMOJI")?,
    )?;
    let tim_bytes = source
        .edit_shared_ui_decoded
        .get(tim_offset..tim_offset + tim.total_size)
        .context("EDIT runtime font TIM is truncated")?;
    let codes = font
        .glyph_codes
        .iter()
        .map(|code| parse_hex_u16(code, "runtime font glyph code"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        tim_offset == RUNTIME_FONT_TIM_OFFSET
            && tim.image_x == 704
            && tim.image_y == 0
            && tim.pixel_width() == 256
            && tim.image_height == 256
            && sha256_bytes(tim_bytes) == RUNTIME_FONT_TIM_SHA256
            && font.tim_sha256 == RUNTIME_FONT_TIM_SHA256
            && codes == RUNTIME_FONT_CODES,
        "EDIT runtime font page binding changed"
    );
    Ok(())
}

fn validate_contextual_code_ownership(
    overlay: &[u8],
    entries: &[EditRuntimeTextTranslation],
) -> Result<()> {
    let mut expected = Vec::new();
    for entry in entries {
        let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        for (index, code) in entry.source_codes.iter().enumerate() {
            let code = parse_hex_u16(code, "source code")?;
            if RUNTIME_FONT_CODES.contains(&code) {
                expected.push((source_offset + 2 + index * 2, code));
            }
        }
    }
    let observed = overlay
        .get(..0x1000)
        .context("KANRI string table is truncated")?
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, bytes)| {
            let code = u16::from_le_bytes([bytes[0], bytes[1]]);
            RUNTIME_FONT_CODES
                .contains(&code)
                .then_some((index * 2, code))
        })
        .collect::<Vec<_>>();
    ensure!(
        observed == expected,
        "EDIT contextual glyph codes have an unowned KANRI string-table consumer"
    );
    Ok(())
}

fn validate_entry(
    entry: &EditRuntimeTextTranslation,
    source: &EditRuntimeTextSource,
) -> Result<()> {
    let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
    let source_record = source
        .overlay
        .get(source_offset..source_offset + entry.source_record_size)
        .context("EDIT runtime text source record is out of bounds")?;
    ensure!(
        sha256_bytes(source_record) == entry.source_record_sha256,
        "EDIT runtime text {} source record changed",
        entry.id
    );
    let expected_codes = entry
        .source_codes
        .iter()
        .map(|code| parse_hex_u16(code, "source code"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        read_length_prefixed_codes(&source.overlay, source_offset)? == expected_codes,
        "EDIT runtime text {} source codes changed",
        entry.id
    );
    ensure!(
        2 + expected_codes.len() * 2 <= entry.source_record_size,
        "EDIT runtime text {} source record is smaller than its codes",
        entry.id
    );

    let placement_offset = parse_hex_usize(&entry.placement_record_offset, "placement offset")?;
    let placement = source
        .overlay
        .get(placement_offset..placement_offset + 12)
        .context("EDIT runtime text placement record is out of bounds")?;
    ensure!(
        sha256_bytes(placement) == entry.source_placement_sha256,
        "EDIT runtime text {} placement record changed",
        entry.id
    );
    ensure!(
        read_i16(placement, 0)? == entry.source_x
            && read_i16(placement, 2)? == entry.source_y
            && read_u32(placement, 4)? == OVERLAY_RUNTIME_BASE + u32::try_from(source_offset)?
            && read_u32(placement, 8)? == 0,
        "EDIT runtime text {} placement consumer binding changed",
        entry.id
    );

    match entry.development_status {
        DevelopmentStatus::Authored => ensure!(
            entry
                .source_text
                .as_ref()
                .is_some_and(|text| !text.is_empty())
                && entry
                    .korean_text
                    .as_ref()
                    .is_some_and(|text| !text.is_empty())
                && entry.release_status != ReleaseStatus::Untranslated,
            "authored EDIT runtime text {} lacks source, Korean, or review state",
            entry.id
        ),
        DevelopmentStatus::Untranslated => ensure!(
            entry.korean_text.is_none() && entry.release_status == ReleaseStatus::Untranslated,
            "untranslated EDIT runtime text {} carries authored output",
            entry.id
        ),
    }
    Ok(())
}

pub(super) fn parse_hex_usize(value: &str, label: &str) -> Result<usize> {
    let value = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not hexadecimal"))?;
    usize::from_str_radix(value, 16).with_context(|| format!("invalid {label}"))
}

pub(super) fn parse_hex_u16(value: &str, label: &str) -> Result<u16> {
    let value = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not hexadecimal"))?;
    u16::from_str_radix(value, 16).with_context(|| format!("invalid {label}"))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16> {
    Ok(i16::from_le_bytes(
        data.get(offset..offset + 2)
            .context("truncated i16")?
            .try_into()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .context("truncated u32")?
            .try_into()?,
    ))
}

fn validate_relative_file(path: &str) -> Result<()> {
    let path = Path::new(path);
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "EDIT runtime text asset path must be a plain relative path"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DevelopmentStatus, EditRuntimeTextTranslation, ReleaseStatus,
        validate_first_stable_action_coverage,
    };
    use crate::edit_runtime_text::model::HorizontalAlignment;

    fn entry(id: &str, development_status: DevelopmentStatus) -> EditRuntimeTextTranslation {
        EditRuntimeTextTranslation {
            kind: String::new(),
            id: id.to_string(),
            source_offset: String::new(),
            source_record_size: 0,
            source_record_sha256: String::new(),
            source_codes: Vec::new(),
            source_text: None,
            korean_text: None,
            placement_record_offset: String::new(),
            source_x: 0,
            source_y: 0,
            source_placement_sha256: String::new(),
            horizontal_alignment: HorizontalAlignment::PreserveCenter,
            development_status,
            release_status: ReleaseStatus::Untranslated,
        }
    }

    #[test]
    fn first_stable_edit_action_cannot_disappear_as_an_untranslated_blank() {
        let entries = [
            entry("character_registration", DevelopmentStatus::Authored),
            entry("load", DevelopmentStatus::Authored),
            entry("register", DevelopmentStatus::Untranslated),
            entry("password_input", DevelopmentStatus::Authored),
            entry("exit", DevelopmentStatus::Authored),
        ];

        let error = validate_first_stable_action_coverage(&entries).unwrap_err();
        assert!(error.to_string().contains("register"));
    }
}
