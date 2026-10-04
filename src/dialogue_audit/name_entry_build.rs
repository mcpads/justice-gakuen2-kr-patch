use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::name_input::{NAME_GLYPH_CODE_COUNT, NAME_GLYPH_CODE_START};
use crate::pipeline::sha256_bytes;

use super::format::hex_code;
use super::name_entry::{
    OVERLAY_PATH, OVERLAY_SHA256, PADDING_CODE, PAGE_SPECS, SELECTABLE_CODE_SEQUENCE_OFFSET,
    analyze_name_entry_overlay, load_supported_name_entry_overlay,
};
use super::name_entry_build_model::{
    DialogueNameEntryBuildConfig, DialogueNameEntryBuildReport,
    DialogueNameEntryCandidateAssignment, DialogueNameEntryCandidatePageReport,
};
use super::name_entry_font_build::{NAME_ENTRY_FONT_OUTPUT_FILE, build_name_entry_font_asset};

const CANDIDATE_KIND: &str = "Justice Gakuen 2 non-release development name-entry candidate page";
const CANDIDATE_FILES: [&str; 3] = ["family-name.json", "given-name.json", "supplemental.json"];
pub(super) const GLOBAL_NAME_CODE_START: u16 = NAME_GLYPH_CODE_START;
pub(super) const GLOBAL_NAME_CODE_COUNT: usize = NAME_GLYPH_CODE_COUNT;
pub(super) const LOCALIZED_NAME_CODE_COUNT: usize = 218;
const PRESERVED_DECIMAL_CHARACTERS: &str = "0123456789";
const PRESERVED_DECIMAL_CODES: [u16; 10] = [
    0x000a, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009,
];
const PATCHED_OVERLAY_FILE: &str = "MGENT.BIN";
const BUILD_MANIFEST_FILE: &str = "dialogue-name-entry-build.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueNameEntryCandidatePage {
    pub(super) kind: String,
    pub(super) source_overlay_sha256: String,
    pub(super) source_page: String,
    pub(super) selection_basis: String,
    #[serde(default)]
    pub(super) preserved_source_characters: String,
    pub(super) characters: String,
}

#[derive(Debug, Clone)]
pub(super) struct DialogueNameCandidateSet {
    pub(super) manifest_sha256: String,
    pub(super) pages: Vec<DialogueNameEntryCandidatePageReport>,
    pub(super) assignments: Vec<DialogueNameEntryCandidateAssignment>,
}

pub fn build_dialogue_name_entry_image(
    config: &DialogueNameEntryBuildConfig,
) -> Result<DialogueNameEntryBuildReport> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let (source_overlay, source_bin_sha256, source_overlay_sha256) =
        load_supported_name_entry_overlay(&config.cue)?;
    let candidates = load_dialogue_name_candidates(&config.candidates)?;
    let patched_overlay = patch_name_entry_overlay(&source_overlay, &candidates)?;
    let patched_overlay_sha256 = sha256_bytes(&patched_overlay);
    let (patched_font, font) = build_name_entry_font_asset(
        &config.cue,
        &source_overlay,
        &candidates.assignments,
        &config.graphics,
        &config.candidate_font,
        config.candidate_font_px,
        &config.fixed_graphics_font,
    )?;

    let reparsed = analyze_name_entry_overlay(
        &patched_overlay,
        source_bin_sha256.clone(),
        patched_overlay_sha256.clone(),
    )?;
    ensure!(
        reparsed.source_selectable_cell_count == GLOBAL_NAME_CODE_COUNT,
        "localized name-entry candidate population changed"
    );
    ensure!(
        reparsed.source_unique_selectable_code_count == GLOBAL_NAME_CODE_COUNT,
        "localized name-entry codes are not unique"
    );

    std::fs::write(
        config.output_dir.join(PATCHED_OVERLAY_FILE),
        &patched_overlay,
    )?;
    std::fs::write(
        config.output_dir.join(NAME_ENTRY_FONT_OUTPUT_FILE),
        &patched_font,
    )?;
    let report = DialogueNameEntryBuildReport {
        kind: "Justice Gakuen 2 non-release development name-entry build".to_string(),
        source_bin_sha256,
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_sha256,
        candidate_manifest_sha256: candidates.manifest_sha256,
        development_mapping_complete: true,
        release_candidate_mapping_eligible: false,
        global_name_code_start: hex_code(GLOBAL_NAME_CODE_START),
        global_name_code_end: hex_code(
            GLOBAL_NAME_CODE_START + u16::try_from(GLOBAL_NAME_CODE_COUNT - 1)?,
        ),
        candidate_count: candidates.assignments.len(),
        localized_candidate_count: candidates
            .assignments
            .iter()
            .filter(|assignment| !assignment.preserve_source_glyph)
            .count(),
        preserved_source_candidate_count: candidates
            .assignments
            .iter()
            .filter(|assignment| assignment.preserve_source_glyph)
            .count(),
        pages: candidates.pages,
        assignments: candidates.assignments,
        patched_overlay_file: PATCHED_OVERLAY_FILE.to_string(),
        patched_overlay_sha256,
        page_and_sequence_codes_match: true,
        source_padding_preserved: true,
        source_routines_preserved: true,
        font,
    };
    write_report(&config.output_dir.join(BUILD_MANIFEST_FILE), &report)?;
    Ok(report)
}

pub(super) fn load_dialogue_name_candidates(root: &Path) -> Result<DialogueNameCandidateSet> {
    let mut raw_pages = Vec::new();
    let mut manifest_bytes = Vec::new();
    for file in CANDIDATE_FILES {
        let path = root.join(file);
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to read name-entry candidates {}", path.display()))?;
        manifest_bytes.extend_from_slice(file.as_bytes());
        manifest_bytes.push(0);
        manifest_bytes.extend_from_slice(&bytes);
        let page: DialogueNameEntryCandidatePage = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse name-entry candidates {}", path.display()))?;
        raw_pages.push((file.to_string(), page));
    }
    validate_candidate_pages(raw_pages, sha256_bytes(&manifest_bytes))
}

pub(super) fn validate_candidate_pages(
    raw_pages: Vec<(String, DialogueNameEntryCandidatePage)>,
    manifest_sha256: String,
) -> Result<DialogueNameCandidateSet> {
    ensure!(
        raw_pages.len() == PAGE_SPECS.len(),
        "name-entry candidate page count changed"
    );
    let mut seen = BTreeSet::new();
    let mut assignments = Vec::with_capacity(GLOBAL_NAME_CODE_COUNT);
    let mut localized_assignment_count = 0usize;
    let mut pages = Vec::with_capacity(PAGE_SPECS.len());

    for ((candidate_file, page), (expected_name, _, cell_count, padding_count)) in
        raw_pages.into_iter().zip(PAGE_SPECS)
    {
        ensure!(
            page.kind == CANDIDATE_KIND,
            "unknown name-entry candidate kind"
        );
        ensure!(
            page.source_overlay_sha256 == OVERLAY_SHA256,
            "name-entry candidate source overlay changed"
        );
        ensure!(
            page.source_page == expected_name,
            "name-entry candidate page order changed"
        );
        ensure!(
            !page.selection_basis.trim().is_empty(),
            "name-entry candidate selection basis is empty"
        );
        let preserved_source_characters =
            page.preserved_source_characters.chars().collect::<Vec<_>>();
        let characters = page.characters.chars().collect::<Vec<_>>();
        let expected_count = cell_count - padding_count;
        ensure!(
            preserved_source_characters.len() + characters.len() == expected_count,
            "{} name-entry candidate count changed",
            page.source_page
        );
        if page.source_page == "alphanumeric" {
            ensure!(
                page.preserved_source_characters == PRESERVED_DECIMAL_CHARACTERS,
                "alphanumeric page must preserve the source decimal prefix"
            );
        } else {
            ensure!(
                preserved_source_characters.is_empty(),
                "only the alphanumeric page may preserve source candidates"
            );
        }
        let first_assignment_index = assignments.len();
        for (page_position, character) in preserved_source_characters.iter().copied().enumerate() {
            ensure!(
                seen.insert(character),
                "name-entry candidate {character:?} is duplicated"
            );
            assignments.push(DialogueNameEntryCandidateAssignment {
                source_page: page.source_page.clone(),
                page_position,
                character: character.to_string(),
                code: hex_code(PRESERVED_DECIMAL_CODES[page_position]),
                preserve_source_glyph: true,
            });
        }
        for (index, character) in characters.iter().copied().enumerate() {
            ensure!(
                ('\u{ac00}'..='\u{d7a3}').contains(&character),
                "name-entry candidate {character:?} is not a precomposed Hangul syllable"
            );
            ensure!(
                seen.insert(character),
                "name-entry candidate {character:?} is duplicated"
            );
            let code = GLOBAL_NAME_CODE_START + u16::try_from(localized_assignment_count)?;
            assignments.push(DialogueNameEntryCandidateAssignment {
                source_page: page.source_page.clone(),
                page_position: preserved_source_characters.len() + index,
                character: character.to_string(),
                code: hex_code(code),
                preserve_source_glyph: false,
            });
            localized_assignment_count += 1;
        }
        let last_assignment_index = assignments.len() - 1;
        pages.push(DialogueNameEntryCandidatePageReport {
            candidate_file,
            source_page: page.source_page,
            selection_basis: page.selection_basis,
            candidate_count: expected_count,
            localized_candidate_count: characters.len(),
            preserved_source_candidate_count: preserved_source_characters.len(),
            first_code: assignments[first_assignment_index].code.clone(),
            last_code: assignments[last_assignment_index].code.clone(),
            characters: page.characters,
        });
    }
    ensure!(
        assignments.len() == GLOBAL_NAME_CODE_COUNT,
        "global name-entry candidate population changed"
    );
    ensure!(
        localized_assignment_count == LOCALIZED_NAME_CODE_COUNT,
        "localized name-entry candidate population changed"
    );
    ensure!(
        assignments
            .iter()
            .rev()
            .find(|assignment| !assignment.preserve_source_glyph)
            .is_some_and(|assignment| assignment.code == "0x03de"),
        "localized name-entry code range changed"
    );
    Ok(DialogueNameCandidateSet {
        manifest_sha256,
        pages,
        assignments,
    })
}

pub(super) fn patch_name_entry_overlay(
    source: &[u8],
    candidates: &DialogueNameCandidateSet,
) -> Result<Vec<u8>> {
    ensure!(
        candidates.assignments.len() == GLOBAL_NAME_CODE_COUNT,
        "name-entry candidate population is incomplete"
    );
    let mut patched = source.to_vec();
    let mut assignment_index = 0usize;
    for (page_name, page_offset, cell_count, expected_padding_count) in PAGE_SPECS {
        let mut padding_count = 0usize;
        let mut page_position = 0usize;
        for cell_index in 0..cell_count {
            let offset = page_offset + cell_index * 2;
            let source_code = read_u16(source, offset)?;
            if source_code == PADDING_CODE {
                padding_count += 1;
                continue;
            }
            let assignment = candidates
                .assignments
                .get(assignment_index)
                .context("name-entry candidate assignment exhausted")?;
            ensure!(
                assignment.source_page == page_name && assignment.page_position == page_position,
                "name-entry candidate assignment order changed"
            );
            let code = parse_hex_code(&assignment.code)?;
            if assignment.preserve_source_glyph {
                ensure!(
                    source_code == code,
                    "preserved name-entry source glyph code changed"
                );
            } else {
                write_u16(&mut patched, offset, code)?;
            }
            write_u16(
                &mut patched,
                SELECTABLE_CODE_SEQUENCE_OFFSET + assignment_index * 2,
                code,
            )?;
            assignment_index += 1;
            page_position += 1;
        }
        ensure!(
            padding_count == expected_padding_count,
            "name-entry source padding population changed"
        );
    }
    ensure!(
        assignment_index == candidates.assignments.len(),
        "name-entry candidate assignments remain unused"
    );
    Ok(patched)
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .context("truncated name-entry u16")?;
    Ok(u16::from_le_bytes(bytes.try_into()?))
}

fn write_u16(data: &mut [u8], offset: usize, value: u16) -> Result<()> {
    let destination = data
        .get_mut(offset..offset + 2)
        .context("truncated name-entry u16 destination")?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn parse_hex_code(value: &str) -> Result<u16> {
    let digits = value
        .strip_prefix("0x")
        .context("name-entry code lacks 0x prefix")?;
    Ok(u16::from_str_radix(digits, 16)?)
}

pub(super) fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        ensure!(output_dir.is_dir(), "name-entry output is not a directory");
        if !force && std::fs::read_dir(output_dir)?.next().is_some() {
            bail!(
                "name-entry output is not empty; pass --force to replace owned files in {}",
                output_dir.display()
            );
        }
        return Ok(());
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}

fn write_report(path: &Path, report: &DialogueNameEntryBuildReport) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
