//! Source-bound CPU technique rows; wording is shared with EDIT command sheets.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    Cell, read_indexed_cell_without_clut_in_prefix,
    write_indexed_cell_without_clut_in_prefix_with_report,
};

use super::catalog::ModeDescendantRecordSpec;
use super::edit_move_names::load_move_names;
use super::edit_technique_archive::{ARCHIVE, MEMBER_SIZE, PATH};
use super::model::ModeDescendantGraphicsBuildConfig;
use super::record_compositor::ModeDescendantRecordDraft;
use super::source::ModeDescendantSourceRecord;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    source_path: String,
    move_names_path: std::path::PathBuf,
    members: Vec<Member>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    member_index: usize,
    styles: Vec<Vec<u8>>,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    row_index: u8,
    source_text: String,
    source_region_sha256: String,
    command_sheet_occurrence_id: String,
}

#[derive(Debug, Serialize)]
pub struct TechniqueNameBuildReport {
    pub source_path: String,
    pub manifest_sha256: String,
    pub move_name_manifest_sha256: String,
    pub command_sheet_manifest_sha256: String,
    pub member_count: usize,
    pub authored_row_count: usize,
    pub translation_status: &'static str,
    pub pass_loader_and_renderer_verified: bool,
    pub selector_style_tables_verified: bool,
    pub changes_confined_to_name_cells: bool,
    pub rows: Vec<TechniqueRowBuildReport>,
}

#[derive(Debug, Serialize)]
pub struct TechniqueRowBuildReport {
    member_index: usize,
    row_index: u8,
    source_text: String,
    korean_text: String,
    cell: Cell,
    font_px: f32,
    measured_advance_px: f32,
    fill_index: u8,
}

fn selected_rows(pass: &[u8], member: usize) -> Result<Vec<Vec<u8>>> {
    let offset = 0xf08 + member * 4;
    let pointer = u32::from_le_bytes(
        pass.get(offset..offset + 4)
            .context("CPU technique selector pointer missing")?
            .try_into()?,
    );
    let start = pointer
        .checked_sub(0x8017_a000)
        .context("CPU technique pointer outside PASS")? as usize;
    let records = pass
        .get(start..start + 44)
        .context("CPU technique style records missing")?;
    records
        .as_chunks::<11>()
        .0
        .iter()
        .map(|record| {
            let count = usize::from(record[0]);
            ensure!(
                count <= 10,
                "CPU technique style exceeds native saved-value slots"
            );
            let rows = &record[1..1 + count];
            ensure!(
                rows.iter().all(|row| *row < 10)
                    && rows.iter().collect::<BTreeSet<_>>().len() == rows.len(),
                "CPU technique style has an invalid or duplicate saved-value row"
            );
            Ok(rows.to_vec())
        })
        .collect()
}

fn validate_member(member: &Member, native_styles: &[Vec<u8>]) -> Result<()> {
    ensure!(
        member.styles == native_styles,
        "CPU technique selector/style mapping changed"
    );
    let selected = native_styles
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let authored = member
        .rows
        .iter()
        .map(|row| row.row_index)
        .collect::<BTreeSet<_>>();
    ensure!(
        authored.len() == member.rows.len() && authored == selected,
        "CPU technique authored rows must cover exactly the native selected rows"
    );
    Ok(())
}

pub(super) fn build(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    disc: &SupportedSourceDisc,
    source: &ModeDescendantSourceRecord,
    spec: &'static ModeDescendantRecordSpec,
    path: &Path,
    command_path: &Path,
) -> Result<(ModeDescendantRecordDraft, TechniqueNameBuildReport)> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "justice_gakuen2_edit_technique_names" && manifest.source_path == PATH,
        "unsupported CPU technique manifest"
    );
    ensure!(
        source.path == PATH
            && spec.source_path == PATH
            && source.decoded.len() == ARCHIVE.members.len() * MEMBER_SIZE,
        "CPU technique build source changed"
    );
    ensure!(
        manifest.members.len() == ARCHIVE.members.len()
            && manifest
                .members
                .iter()
                .map(|m| m.member_index)
                .collect::<BTreeSet<_>>()
                == (0..ARCHIVE.members.len()).collect(),
        "CPU technique member coverage changed"
    );
    let parent = path
        .parent()
        .context("CPU technique manifest has no parent")?;
    ensure!(
        !manifest.move_names_path.is_absolute()
            && manifest
                .move_names_path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "CPU technique wording path leaves its manifest directory"
    );
    let command_bytes = std::fs::read(command_path)?;
    let commands: CommandManifest = serde_json::from_slice(&command_bytes)?;
    let wording = load_move_names(&parent.join(&manifest.move_names_path))?;
    let (_, pass) = disc.read_record("DAT1/PASS.BIN")?;
    ensure!(
        sha256_bytes(&pass) == "f160d227922e5928eda71a1d332fa6c463a4ba57826d35ca72d3c6e0581b90d4",
        "CPU technique PASS source identity changed"
    );
    // These immutable source windows bind the 0x4b loader, selector/style lookup,
    // 200x20 row sprites and the native ten-slot read/write mapping.
    validate_consumers(&pass)?;
    let (_, shared_stored) = disc.read_record("DAT2/EDITMOJI.TIZ")?;
    let shared = crate::compression::decompress(&shared_stored, false)?;
    ensure!(
        shared.get(20 + 128 * 2..20 + 144 * 2) == Some(PALETTE.as_slice()),
        "CPU technique source CLUT at (128,481) changed"
    );
    let font = &config.fonts.edit_compact_label;
    let rasterizer = rasterizers.for_font(&font.path)?;
    let mut patched = source.decoded.clone();
    let mut allowed = Vec::new();
    let mut rows = Vec::new();
    for member in &manifest.members {
        validate_member(member, &selected_rows(&pass, member.member_index)?)?;
        let tim_offset = member.member_index * MEMBER_SIZE;
        for row in &member.rows {
            ensure!(
                !row.command_sheet_occurrence_id.trim().is_empty(),
                "CPU technique row lacks source comparison identity"
            );
            ensure!(
                commands.occurrences.iter().any(|other| other.id
                    == row.command_sheet_occurrence_id
                    && other.member_index == member.member_index
                    && other.role == "move_name"
                    && other.move_name_source_text.as_deref() == Some(row.source_text.as_str())),
                "CPU technique row lost its reviewed command-sheet correspondence"
            );
            let cell = Cell {
                x: 0,
                y: usize::from(row.row_index) * 20,
                width: 200,
                height: 20,
            };
            let source_pixels =
                read_indexed_cell_without_clut_in_prefix(&source.decoded, tim_offset, cell)?;
            ensure!(
                sha256_bytes(&source_pixels) == row.source_region_sha256,
                "CPU technique member {} row {} source cell changed",
                member.member_index,
                row.row_index
            );
            let korean_text = wording
                .entries
                .get(&row.source_text)
                .with_context(|| {
                    format!(
                        "CPU technique has no shared translation: {}",
                        row.source_text
                    )
                })?
                .join(" ");
            let raster = rasterizer.rasterize(
                &korean_text,
                cell.width,
                cell.height,
                font.font_px,
                0.0,
                0,
                None,
                15,
                HorizontalTextAlignment::Center,
            )?;
            let write = write_indexed_cell_without_clut_in_prefix_with_report(
                &mut patched,
                tim_offset,
                cell,
                &raster.pixels,
            )?;
            ensure!(
                write.changed_byte_count > 0,
                "CPU technique row changed no pixels"
            );
            allowed.extend(write.allowed_ranges);
            rows.push(TechniqueRowBuildReport {
                member_index: member.member_index,
                row_index: row.row_index,
                source_text: row.source_text.clone(),
                korean_text,
                cell,
                font_px: font.font_px,
                measured_advance_px: raster.measured_advance_px,
                fill_index: 15,
            });
        }
    }
    let claims = DecodedDataClaim::from_effective_ranges(
        "mode-descendant:edit-technique-names",
        "render source-bound CPU technique names using shared EDIT wording",
        &source.decoded,
        &patched,
        allowed,
    )?;
    let report = TechniqueNameBuildReport {
        source_path: PATH.to_string(),
        manifest_sha256: sha256_bytes(&bytes),
        move_name_manifest_sha256: wording.sha256,
        command_sheet_manifest_sha256: sha256_bytes(&command_bytes),
        member_count: manifest.members.len(),
        authored_row_count: rows.len(),
        translation_status: "draft",
        pass_loader_and_renderer_verified: true,
        selector_style_tables_verified: true,
        changes_confined_to_name_cells: true,
        rows,
    };
    Ok((
        ModeDescendantRecordDraft {
            spec,
            decoded: patched,
            decoded_write_claims: claims,
        },
        report,
    ))
}

fn validate_consumers(pass: &[u8]) -> Result<()> {
    for &(start, end, hash) in CONSUMER_WINDOWS {
        ensure!(
            pass.get(start..end)
                .is_some_and(|bytes| sha256_bytes(bytes) == hash),
            "CPU technique consumer at PASS +0x{start:x} changed"
        );
    }
    let loader_sites = pass
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, word)| (*word == 0x2405_004bu32.to_le_bytes()).then_some(index * 4))
        .collect::<Vec<_>>();
    ensure!(
        loader_sites == [0x57f0, 0x5b48],
        "CPU technique PASS loader population changed"
    );
    Ok(())
}

const PALETTE: [u8; 32] = [
    0, 0, 96, 252, 123, 239, 57, 231, 214, 218, 148, 210, 115, 206, 49, 198, 239, 189, 206, 185,
    140, 177, 107, 173, 41, 165, 231, 156, 198, 152, 132, 144,
];
const CONSUMER_WINDOWS: &[(usize, usize, &str)] = &[
    (
        0x57d0,
        0x5840,
        "0e63d0a894d19dea878574a8d69c404c644e01cf7056a2773625a1952fd7ba46",
    ),
    (
        0x5b40,
        0x5b9c,
        "fb61b4e3545bd3329bad4af8ade47cc54dcf71eb9c97d98aff6d4ded2b64a1f7",
    ),
    (
        0x4d40,
        0x507c,
        "fd58d664362a30e33b0d9f4f1b21f9b845273befd0e94f5487940c0de6064d95",
    ),
    (
        0x50b0,
        0x51d4,
        "f90cec425dae1a7804c82178b99d8570ad79fb226f4f1f28cfdf7d8f68a71dd9",
    ),
    (
        0x984,
        0xf88,
        "b09eaa8a1fe9c4a61884bed00cf46cc9a041845d5b120dd44219c482530c8eb1",
    ),
];

// Read only the semantic correspondence; the command-sheet builder owns its full plan.
#[derive(Deserialize)]
struct CommandManifest {
    occurrences: Vec<CommandOccurrence>,
}
#[derive(Deserialize)]
struct CommandOccurrence {
    id: String,
    member_index: usize,
    role: String,
    move_name_source_text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: u8) -> Row {
        Row {
            row_index: index,
            source_text: String::new(),
            source_region_sha256: String::new(),
            command_sheet_occurrence_id: String::new(),
        }
    }

    // Name writes must cover exactly the rows the native style table can consume.
    #[test]
    fn rejects_missing_unselected_and_duplicate_authored_rows() {
        let styles = vec![vec![], vec![0, 3], vec![0, 1, 3]];
        let mut member = Member {
            member_index: 0,
            styles: styles.clone(),
            rows: vec![row(3), row(0), row(1)],
        };
        validate_member(&member, &styles).unwrap();
        member.rows.pop();
        assert!(validate_member(&member, &styles).is_err());
        member.rows.push(row(2));
        assert!(validate_member(&member, &styles).is_err());
        member.rows = vec![row(0), row(1), row(3), row(3)];
        assert!(validate_member(&member, &styles).is_err());
        member.rows.pop();
        member.styles[1].reverse();
        assert!(validate_member(&member, &styles).is_err());
    }

    // Malformed row selection must fail before indexing the ten native saved-value slots.
    #[test]
    fn rejects_invalid_saved_value_indices_and_truncated_style_tables() {
        let mut pass = vec![0; 0xf0c];
        pass[0xf08..0xf0c].copy_from_slice(&0x8017_a100u32.to_le_bytes());
        pass[0x100..0x103].copy_from_slice(&[2, 0, 3]);
        assert_eq!(selected_rows(&pass, 0).unwrap()[0], [0, 3]);
        for bad in [0, 10, 255] {
            pass[0x102] = bad;
            assert!(selected_rows(&pass, 0).is_err());
        }
        pass[0x100] = 11;
        assert!(selected_rows(&pass, 0).is_err());
        pass[0xf08..0xf0c].copy_from_slice(&0x8017_af00u32.to_le_bytes());
        assert!(selected_rows(&pass, 0).is_err());
    }
}
