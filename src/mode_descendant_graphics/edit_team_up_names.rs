//! All cooperative move strips selected by KANRI from the shared secondary TIM.
//! These are fixed sprites, not requests in the shared MENU glyph arena.

use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    development_build_spec::SizedFontSource,
    font::{HorizontalTextAlignment, IndexedTextRasterizers},
    pipeline::sha256_bytes,
    tim::{
        Cell, cells_overlap, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
    },
};

const MEMBER_SIZE: usize = 0x2b800;
const TIM_OFFSET: usize = 0x13000;
const TIM_END: usize = 0x23040;
const TIM_HASH: &str = "96bc875a7708495e19c799d3be0da70a7dbea8d0844fb56516beffa0a3a1d819";
const DESCRIPTOR_HASH: &str = "7cac64451b6f4180c2c60044879fc4dc3ad1d51e08aea7412a0a94e474c3ff22";
const CONSUMER_HASH: &str = "d96291be1e5ac20c5ff82c78abae75679ec8cca1301a8ce676fc5d7190c3aafd";

#[derive(Debug)]
pub(super) struct Plan {
    sha256: String,
    manifest: Manifest,
    translations_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    source_path: String,
    translation_status: String,
    entries: Vec<Entry>,
    translations_path: std::path::PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source_text: String,
    #[serde(skip_deserializing)]
    korean_text: String,
    descriptor_offset: usize,
    cell: Cell,
    clear_cell: Cell,
    source_indexed_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    manifest_sha256: String,
    translations_sha256: String,
    source_tim_sha256: &'static str,
    consumer_sha256: &'static str,
    translation_status: String,
    unique_name_count: usize,
    authored_occurrence_count: usize,
    font_sha256: String,
    font_px: f32,
    palette_and_trailer_preserved: bool,
    runtime_verified: bool,
}

pub(super) fn load(path: &Path) -> Result<Plan> {
    let bytes = std::fs::read(path)?;
    let mut manifest: Manifest = serde_json::from_slice(&bytes)?;
    let translations = crate::team_up_names::TeamUpNames::load(
        &path
            .parent()
            .context("cooperative plan has no parent")?
            .join(&manifest.translations_path),
    )?;
    for entry in &mut manifest.entries {
        entry.korean_text = translations.translation(&entry.source_text)?.to_string();
    }
    ensure!(
        manifest.kind == "justice_gakuen2_edit_team_up_names"
            && manifest.source_path == "DAT2/EDITCM.BIZ"
            && manifest.translation_status == "draft",
        "unsupported EDIT cooperative-name manifest"
    );
    let mut descriptors = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (i, entry) in manifest.entries.iter().enumerate() {
        ensure!(
            descriptors.insert(entry.descriptor_offset)
                && names.insert(&entry.source_text)
                && !entry.source_text.trim().is_empty()
                && !entry.korean_text.trim().is_empty()
                && !entry.korean_text.contains(['\n', '\r'])
                && entry.cell.width > 0
                && entry.cell.height == 24
                && entry
                    .cell
                    .x
                    .checked_add(entry.cell.width)
                    .is_some_and(|end| end <= 512)
                && entry
                    .cell
                    .y
                    .checked_add(entry.cell.height)
                    .is_some_and(|end| end <= 256),
            "invalid or duplicate EDIT cooperative-name strip"
        );
        ensure!(
            entry.clear_cell.x == entry.cell.x
                && entry.clear_cell.y == entry.cell.y
                && entry.clear_cell.height == entry.cell.height
                && entry.clear_cell.width >= entry.cell.width
                && entry.clear_cell.width <= entry.cell.width + 1
                && entry.clear_cell.x + entry.clear_cell.width <= 512,
            "cooperative clearing range does not contain its source sprite"
        );
        for other in &manifest.entries[..i] {
            ensure!(
                !cells_overlap(entry.clear_cell, other.clear_cell),
                "overlapping EDIT cooperative-name strips"
            );
        }
    }
    ensure!(
        descriptors == (0x1038..0x1204).step_by(20).collect(),
        "EDIT cooperative-name plan must cover the complete source descriptor family"
    );
    Ok(Plan {
        sha256: sha256_bytes(&bytes),
        translations_sha256: translations.sha256,
        manifest,
    })
}

pub(super) fn validate_consumer(plan: &Plan, kanri: &[u8]) -> Result<()> {
    ensure!(
        sha256_bytes(
            kanri
                .get(0x1038..0x1264)
                .context("missing cooperative descriptors")?
        ) == DESCRIPTOR_HASH
            && sha256_bytes(
                kanri
                    .get(0x4dc0..0x4dfc)
                    .context("missing cooperative selector consumer")?
            ) == CONSUMER_HASH,
        "KANRI cooperative selector or descriptor family changed"
    );
    for entry in &plan.manifest.entries {
        let words = kanri[entry.descriptor_offset..entry.descriptor_offset + 20]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|word| u16::from_le_bytes(*word))
            .collect::<Vec<_>>();
        ensure!(
            words[0] == 0
                && matches!(words[1], 640 | 704)
                && words[2] == 256
                && words[3] == 484
                && entry.cell.x == usize::from(words[1] - 640) * 4 + usize::from(words[4])
                && entry.cell.y == usize::from(words[5])
                && entry.cell.width == usize::from(words[6])
                && entry.cell.height == usize::from(words[7]),
            "cooperative name {} leaves its source-selected sprite",
            entry.source_text
        );
    }
    Ok(())
}

pub(super) fn apply(
    plan: &Plan,
    style: &SizedFontSource,
    rasterizers: &mut IndexedTextRasterizers,
    source: &[u8],
    patched: &mut [u8],
    allowed: &mut Vec<[usize; 2]>,
) -> Result<Report> {
    ensure!(
        source.len() == 32 * MEMBER_SIZE && patched.len() == source.len(),
        "EDIT cooperative archive extent changed"
    );
    let rasterizer = rasterizers.for_font(&style.path)?;
    // Resolve every label once before writing any of the identical member copies.
    let rendered = plan
        .manifest
        .entries
        .iter()
        .map(|entry| {
            rasterizer
                .rasterize(
                    &entry.korean_text,
                    entry.cell.width,
                    entry.cell.height,
                    style.font_px,
                    0.0,
                    15,
                    Some(0),
                    14,
                    HorizontalTextAlignment::Center,
                )
                .with_context(|| format!("EDIT cooperative name {} layout", entry.source_text))
        })
        .collect::<Result<Vec<_>>>()?;
    for (entry, raster) in plan.manifest.entries.iter().zip(&rendered) {
        let [left, top, right, bottom] = raster.ink_bounds;
        ensure!(
            left > 0 && top > 0 && right < entry.cell.width && bottom < entry.cell.height,
            "cooperative name {} has no clear margin around its complete outline",
            entry.source_text
        );
    }
    for member in 0..32 {
        let start = member * MEMBER_SIZE;
        let tim = start + TIM_OFFSET;
        ensure!(
            sha256_bytes(&source[tim..start + TIM_END]) == TIM_HASH
                && source[tim..start + MEMBER_SIZE] == patched[tim..start + MEMBER_SIZE],
            "EDIT cooperative member {member} source or earlier writer changed"
        );
        for (entry, raster) in plan.manifest.entries.iter().zip(&rendered) {
            ensure!(
                sha256_bytes(&read_indexed_cell_in_prefix(source, tim, entry.clear_cell)?)
                    == entry.source_indexed_sha256,
                "EDIT cooperative name {} source region changed",
                entry.source_text
            );
            let mut pixels = vec![15; entry.clear_cell.width * entry.clear_cell.height];
            for row in 0..entry.cell.height {
                pixels
                    [row * entry.clear_cell.width..row * entry.clear_cell.width + entry.cell.width]
                    .copy_from_slice(
                        &raster.pixels[row * entry.cell.width..(row + 1) * entry.cell.width],
                    );
            }
            let write =
                write_indexed_cell_in_prefix_with_report(patched, tim, entry.clear_cell, &pixels)?;
            ensure!(
                write.changed_byte_count > 0,
                "cooperative name changed no pixels"
            );
            allowed.extend(write.allowed_ranges);
        }
        ensure!(
            source[tim..tim + 0x40] == patched[tim..tim + 0x40]
                && source[start + TIM_END..start + MEMBER_SIZE]
                    == patched[start + TIM_END..start + MEMBER_SIZE],
            "EDIT cooperative palette, TIM metadata or trailer changed"
        );
    }
    Ok(Report {
        manifest_sha256: plan.sha256.clone(),
        translations_sha256: plan.translations_sha256.clone(),
        source_tim_sha256: TIM_HASH,
        consumer_sha256: CONSUMER_HASH,
        translation_status: plan.manifest.translation_status.clone(),
        unique_name_count: rendered.len(),
        authored_occurrence_count: 32 * rendered.len(),
        font_sha256: sha256_bytes(&std::fs::read(&style.path)?),
        font_px: style.font_px,
        palette_and_trailer_preserved: true,
        runtime_verified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the private original disc and selected font; emits no ROM"]
    fn source_team_up_family_erases_all_old_ink_and_preserves_neighbor_pixels() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let disc = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let mut plan =
            load(&root.join("assets/menu/mode-descendants/edit/team-up-names.json")).unwrap();
        let (_, kanri) = disc.read_record("DAT1/KANRI.BIN").unwrap();
        validate_consumer(&plan, &kanri).unwrap();
        // A shifted authoring rectangle must fail even if it fits the atlas.
        plan.manifest.entries[1].cell.x += 1;
        assert!(validate_consumer(&plan, &kanri).is_err());
        plan.manifest.entries[1].cell.x -= 1;
        let mut wrong_consumer = kanri;
        wrong_consumer[0x4dc8] ^= 1;
        assert!(validate_consumer(&plan, &wrong_consumer).is_err());

        let (_, stored) = disc.read_record("DAT2/EDITCM.BIZ").unwrap();
        let source = super::super::indexed_member_archive::decode_indexed_member_archive(
            &stored,
            &super::super::edit_command_sheets::EDIT_COMMAND_SHEET_ARCHIVE,
        )
        .unwrap();
        let spec = crate::development_build_spec::load_development_build_spec(
            &root.join("assets/build/development.json"),
        )
        .unwrap();
        let style = spec.fonts.mode_descendants.edit_team_up_name;
        let mut rasterizers = IndexedTextRasterizers::default();
        let mut patched = source.clone();
        let mut ranges = Vec::new();
        let report = apply(
            &plan,
            &style,
            &mut rasterizers,
            &source,
            &mut patched,
            &mut ranges,
        )
        .unwrap();
        assert_eq!(report.authored_occurrence_count, 736);
        let changes = crate::pipeline::difference_ranges(&source, &patched);
        assert!(crate::write_scope::changed_ranges_are_within(
            &changes, &ranges
        ));
        let mut escaped = patched.clone();
        escaped[TIM_OFFSET + 20] ^= 1; // Palette byte outside all authored pixels.
        assert!(!crate::write_scope::changed_ranges_are_within(
            &crate::pipeline::difference_ranges(&source, &escaped),
            &ranges,
        ));
        println!(
            "contiguous changes crossing adjacent authored ranges: {}",
            changes
                .iter()
                .filter(|[start, end]| !ranges
                    .iter()
                    .any(|allowed| *start >= allowed[0] && *end <= allowed[1]))
                .count()
        );
        for member in 0..32 {
            let start = member * MEMBER_SIZE;
            let tim = start + TIM_OFFSET;
            assert_eq!(&source[start..tim], &patched[start..tim]);
            assert_eq!(
                &source[start + TIM_END..start + MEMBER_SIZE],
                &patched[start + TIM_END..start + MEMBER_SIZE]
            );
            let old = crate::tim::read_4bpp_indexed_image_in_prefix(&source, tim).unwrap();
            let new = crate::tim::read_4bpp_indexed_image_in_prefix(&patched, tim).unwrap();
            for entry in &plan.manifest.entries {
                let expected = rasterizers
                    .for_font(&style.path)
                    .unwrap()
                    .rasterize(
                        &entry.korean_text,
                        entry.cell.width,
                        entry.cell.height,
                        style.font_px,
                        0.0,
                        15,
                        Some(0),
                        14,
                        HorizontalTextAlignment::Center,
                    )
                    .unwrap();
                assert_eq!(
                    read_indexed_cell_in_prefix(&patched, tim, entry.cell).unwrap(),
                    expected.pixels
                );
            }
            // Retail artwork extends one pixel beyond three draw descriptors.
            for (x, y) in [(395, 6), (98, 34), (139, 152)] {
                assert_ne!(old.pixels[y * 512 + x], 15);
                assert_eq!(
                    new.pixels[y * 512 + x],
                    15,
                    "native glyph tail {member}:{x},{y}"
                );
            }
            for y in 0..256 {
                for x in 0..512 {
                    if !plan.manifest.entries.iter().any(|e| {
                        x >= e.clear_cell.x
                            && x < e.clear_cell.x + e.clear_cell.width
                            && y >= e.clear_cell.y
                            && y < e.clear_cell.y + e.clear_cell.height
                    }) {
                        assert_eq!(old.pixels[y * 512 + x], 15, "unowned original ink {x},{y}");
                        assert_eq!(
                            old.pixels[y * 512 + x],
                            new.pixels[y * 512 + x],
                            "neighbor pixel {member}:{x},{y}"
                        );
                    }
                }
            }
        }
        // An overlapping upstream writer or a drifted source copy must fail closed.
        let mut conflicted = source.clone();
        conflicted[TIM_OFFSET + 0x40] ^= 1;
        assert!(
            apply(
                &plan,
                &style,
                &mut rasterizers,
                &source,
                &mut conflicted,
                &mut Vec::new()
            )
            .is_err()
        );
        let mut drifted = source.clone();
        drifted[31 * MEMBER_SIZE + TIM_OFFSET + 0x40] ^= 1;
        assert!(
            apply(
                &plan,
                &style,
                &mut rasterizers,
                &drifted,
                &mut drifted.clone(),
                &mut Vec::new()
            )
            .is_err()
        );

        let archive = super::super::indexed_member_archive::rebuild_indexed_member_archive(
            &stored,
            &super::super::edit_command_sheets::EDIT_COMMAND_SHEET_ARCHIVE,
            &patched,
        )
        .unwrap();
        assert_eq!(archive.physical.len(), stored.len());
        assert!(
            archive
                .members
                .iter()
                .all(|member| member.patched_stored_size <= member.slot_size)
        );

        let output = root.join("work/edit-consumer-batch/static");
        std::fs::create_dir_all(&output).unwrap();
        let tim = crate::embedded_tim::parse_embedded_tim_at(&patched, TIM_OFFSET).unwrap();
        let image = crate::embedded_tim::decode_embedded_tim_preview(&patched, &tim).unwrap();
        crate::tim_preview::write_tim_preview(&output.join("cooperative-names.png"), &image)
            .unwrap();
        std::fs::write(
            output.join("cooperative-names.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}
