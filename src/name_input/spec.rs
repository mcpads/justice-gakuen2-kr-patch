use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::pipeline::sha256_bytes;

use super::cache::{plan_name_glyph_cache, plan_name_glyph_pack_storage};
use super::keyboard::{
    DIGIT_KEYS, HANGUL_CONSONANT_KEYS, HANGUL_VOWEL_KEYS, LATIN_KEYS, SYMBOL_KEYS,
};
use super::model::{NameInputKeyAssignment, NameInputKeyboardPlan, NameInputPagePlan};

const KEYBOARD_KIND: &str = "Justice Gakuen 2 Korean name-input keyboard";
const SOURCE_OVERLAY_SHA256: &str =
    "eaf243e2d5a78919055b453611ea8b785766a6dc945966a1d5c0ce166714f70a";
const PAGE_SPECS: [(&str, &str, usize); 3] = [
    ("hiragana", "hangul", 84),
    ("katakana", "latin", 82),
    ("alphanumeric", "digits_and_symbols", 62),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NameInputKeyboardDocument {
    kind: String,
    source_overlay_sha256: String,
    pages: Vec<NameInputPageDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NameInputPageDocument {
    source_page: String,
    role: String,
    label: String,
    groups: Vec<NameInputKeyGroupDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NameInputKeyGroupDocument {
    id: String,
    input: String,
    characters: String,
}

pub fn load_name_input_keyboard(path: &Path) -> Result<NameInputKeyboardPlan> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read name-input keyboard {}", path.display()))?;
    parse_name_input_keyboard(&bytes, &path.display().to_string())
}

fn parse_name_input_keyboard(bytes: &[u8], path: &str) -> Result<NameInputKeyboardPlan> {
    let document: NameInputKeyboardDocument = serde_json::from_slice(bytes)
        .with_context(|| format!("failed to parse name-input keyboard {path}"))?;
    ensure!(
        document.kind == KEYBOARD_KIND,
        "unknown name-input keyboard kind"
    );
    ensure!(
        document.source_overlay_sha256 == SOURCE_OVERLAY_SHA256,
        "name-input keyboard source overlay changed"
    );
    ensure!(
        document.pages.len() == PAGE_SPECS.len(),
        "name-input keyboard must define exactly three pages"
    );

    let mut seen_characters = BTreeSet::new();
    let mut pages = Vec::with_capacity(PAGE_SPECS.len());
    for (page, (expected_source_page, expected_role, source_selectable_capacity)) in
        document.pages.into_iter().zip(PAGE_SPECS)
    {
        ensure!(
            page.source_page == expected_source_page,
            "name-input keyboard source page order changed"
        );
        ensure!(
            page.role == expected_role,
            "name-input keyboard page role changed"
        );
        ensure!(
            !page.label.trim().is_empty(),
            "name-input keyboard page label is empty"
        );

        let mut assignments = Vec::new();
        let mut seen_groups = BTreeSet::new();
        for group in page.groups {
            ensure!(
                seen_groups.insert(group.id.clone()),
                "name-input keyboard group is duplicated"
            );
            ensure!(
                matches!(group.input.as_str(), "consonant" | "vowel" | "direct"),
                "unknown name-input key behavior"
            );
            ensure!(
                !group.characters.is_empty(),
                "name-input keyboard group is empty"
            );
            for character in group.characters.chars() {
                ensure!(
                    seen_characters.insert(character),
                    "name-input keyboard character {character:?} is duplicated"
                );
                assignments.push(NameInputKeyAssignment {
                    page_position: assignments.len(),
                    group: group.id.clone(),
                    input: group.input.clone(),
                    character: character.to_string(),
                });
            }
        }
        ensure!(
            assignments.len() <= source_selectable_capacity,
            "name-input keyboard page exceeds its source selectable capacity"
        );
        pages.push(NameInputPagePlan {
            source_page: page.source_page,
            role: page.role,
            label: page.label,
            source_selectable_capacity,
            active_key_count: assignments.len(),
            inactive_position_count: source_selectable_capacity - assignments.len(),
            assignments,
        });
    }
    validate_key_repertoire(&pages)?;
    let cache = plan_name_glyph_cache(&pages)?;
    let glyph_pack_storage = plan_name_glyph_pack_storage(&pages, &cache)?;

    Ok(NameInputKeyboardPlan {
        kind: KEYBOARD_KIND.to_string(),
        source_overlay_sha256: SOURCE_OVERLAY_SHA256.to_string(),
        spec_path: path.to_string(),
        spec_sha256: sha256_bytes(bytes),
        pages,
        cache,
        glyph_pack_storage,
    })
}

fn validate_key_repertoire(pages: &[NameInputPagePlan]) -> Result<()> {
    let hangul = &pages[0].assignments;
    let consonants = characters_for_group(hangul, "consonants", "consonant");
    let vowels = characters_for_group(hangul, "vowels", "vowel");
    ensure!(
        consonants == HANGUL_CONSONANT_KEYS.iter().collect::<String>(),
        "name-input consonant repertoire changed"
    );
    ensure!(
        vowels == HANGUL_VOWEL_KEYS.iter().collect::<String>(),
        "name-input vowel repertoire changed"
    );

    let latin = &pages[1].assignments;
    ensure!(
        latin.iter().all(|assignment| assignment.input == "direct")
            && latin
                .iter()
                .map(|assignment| assignment.character.as_str())
                .collect::<String>()
                == LATIN_KEYS,
        "name-input Latin repertoire changed"
    );

    let digits_and_symbols = &pages[2].assignments;
    ensure!(
        characters_for_group(digits_and_symbols, "digits", "direct") == DIGIT_KEYS,
        "name-input digit repertoire changed"
    );
    ensure!(
        characters_for_group(digits_and_symbols, "symbols", "direct") == SYMBOL_KEYS,
        "name-input symbol repertoire changed"
    );
    Ok(())
}

fn characters_for_group(
    assignments: &[NameInputKeyAssignment],
    group: &str,
    input: &str,
) -> String {
    assignments
        .iter()
        .filter(|assignment| assignment.group == group && assignment.input == input)
        .map(|assignment| assignment.character.as_str())
        .collect()
}

#[cfg(test)]
pub(super) fn parse_name_input_keyboard_fixture(bytes: &[u8]) -> Result<NameInputKeyboardPlan> {
    parse_name_input_keyboard(bytes, "fixture.json")
}
