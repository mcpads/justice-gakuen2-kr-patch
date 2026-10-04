//! Repack the shared selector's name rows and retarget data-only sprite descriptors.
//! Native character indices, pointer order, renderer code and game state stay intact.

use std::cmp::Reverse;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::catalog::ModeDescendantRecordSpec;
use super::model::{
    ModeDescendantEntry, ModeDescendantGraphicsBuildConfig, ModeDescendantPlacement,
    ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface,
};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

const TIM_OFFSET: usize = 0x91800;
const TIM_SIZE: usize = 32896;
const TIM_SHA256: &str = "4cf3977edd71e5aea10993610c74cca7f78dae5ba2224975a5726df73b5abe2a";
const SPECS: [ModeDescendantRecordSpec; 2] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinSprintSelector,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT1/MGAME06.BIN",
        storage_kind: ModeDescendantStorageKind::Raw,
        output_file: "mgame06-selector.bin",
        texture_outputs: &[],
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinDanceSelector,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT1/MGAME07.BIN",
        storage_kind: ModeDescendantStorageKind::Raw,
        output_file: "mgame07-selector.bin",
        texture_outputs: &[],
    },
];
// Descriptor order is deliberately independent of display/character order.
const NAMES: [(&str, &str, u8, u8); 22] = [
    ("kyosuke", "恭介", 5, 2),
    ("batsu", "バツ", 0, 2),
    ("hinata", "ひなた", 2, 3),
    ("shoma", "将馬", 7, 2),
    ("natsu", "夏", 9, 1),
    ("roberto", "ロベルト", 12, 4),
    ("roy", "ロイ", 16, 2),
    ("tiffany", "ティファニー", 18, 6),
    ("bowman", "ボーマン", 24, 4),
    ("edge", "エッジ", 28, 3),
    ("akira", "アキラ", 31, 3),
    ("gan", "岩", 34, 1),
    ("hideo", "英雄", 36, 2),
    ("kyoko", "響子", 38, 2),
    ("raizo", "雷蔵", 40, 2),
    ("hyo", "雹", 42, 1),
    ("akira_unmasked", "あきら", 43, 3),
    ("sakura", "さくら", 48, 3),
    ("daigo", "醍醐", 51, 2),
    ("hayato", "隼人", 53, 2),
    ("ran", "ラン", 56, 2),
    ("nagare", "流", 55, 1),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    entries: Vec<Entry>,
    speed_markers: Vec<SpeedMarker>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source_text: String,
    korean_text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeedMarker {
    id: String,
    source_text: String,
    korean_text: String,
    source_region_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct SelectorNamesReport {
    pub manifest_sha256: String,
    pub font_sha256: String,
    pub source_tim_sha256: String,
    pub physical_texture_copies: usize,
    pub consumer_tables: usize,
    pub code_and_character_order_preserved: bool,
    pub names: Vec<NamePlacement>,
    pub speed_marker_copies: usize,
    pub speed_marker_font_px: f32,
    pub speed_marker_font_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct NamePlacement {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_start: u8,
    pub source_count: u8,
    pub start: u8,
    pub count: u8,
    pub cell: Cell,
}

pub(super) fn record_specs() -> impl Iterator<Item = &'static ModeDescendantRecordSpec> {
    SPECS.iter()
}

fn allocate(counts: &[u8]) -> Result<Vec<u8>> {
    let mut order: Vec<_> = (0..counts.len()).collect();
    order.sort_by_key(|&index| Reverse(counts[index]));
    let mut used = [0u8; 5];
    let mut starts = vec![0; counts.len()];
    for index in order {
        let count = counts[index];
        ensure!(
            (1..=12).contains(&count),
            "selector name has invalid glyph-cell demand"
        );
        let row = used
            .iter()
            .position(|&filled| filled + count <= 12)
            .context("Korean selector names exceed the five owned name rows")?;
        starts[index] = row as u8 * 12 + used[row];
        used[row] += count;
    }
    Ok(starts)
}

fn retarget(overlay: &mut [u8], base: usize, starts: &[u8], counts: &[u8]) -> Result<()> {
    ensure!(
        starts.len() == NAMES.len() && counts.len() == NAMES.len(),
        "selector name mapping is incomplete"
    );
    for (index, (_, _, start, count)) in NAMES.iter().enumerate() {
        let offset = base + index * 4;
        let descriptor = overlay
            .get_mut(offset..offset + 4)
            .context("missing selector name descriptor")?;
        ensure!(
            descriptor == [1, *start, *count, 0],
            "selector name descriptor changed at {offset:#x}"
        );
        ensure!(
            starts[index] < 60 && counts[index] > 0 && starts[index] % 12 + counts[index] <= 12,
            "selector name crosses its texture row"
        );
        descriptor[1] = starts[index];
        descriptor[2] = counts[index];
    }
    // Each school/gender compound is terminated and has its own native pointer.
    let schools = [
        (60, 4, 92),
        (60, 4, 94),
        (64, 4, 92),
        (64, 4, 94),
        (72, 8, 92),
        (72, 8, 94),
        (80, 4, 92),
        (84, 8, 92),
        (84, 8, 94),
    ];
    for (index, (start, count, gender)) in schools.iter().enumerate() {
        let offset = base - 72 + index * 8;
        let descriptor = overlay
            .get_mut(offset..offset + 8)
            .context("missing selector school descriptor")?;
        ensure!(
            descriptor == [2, *start, *count, *gender, 2, 0, 0, 0],
            "selector school descriptor changed"
        );
        if *start == 72 {
            descriptor[2] = 5;
        }
        if *start == 84 {
            descriptor[2] = 6;
        }
    }
    Ok(())
}

pub(super) fn apply(
    config: &ModeDescendantGraphicsBuildConfig,
    drafts: &mut Vec<ModeDescendantRecordDraft>,
    sources: &[ModeDescendantSourceRecord],
    fixed_entries: &[ModeDescendantEntry],
    path: &Path,
) -> Result<SelectorNamesReport> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "justice_gakuen2_gorin_selector_names"
            && manifest.entries.len() == NAMES.len(),
        "selector manifest must cover the native name table"
    );
    let mut counts = Vec::new();
    for (entry, (id, source, _, _)) in manifest.entries.iter().zip(NAMES) {
        ensure!(
            entry.id == id && entry.source_text == source,
            "selector name identity/order changed"
        );
        ensure!(
            !entry.korean_text.is_empty()
                && entry
                    .korean_text
                    .chars()
                    .all(|c| ('가'..='힣').contains(&c)),
            "selector name must contain Korean syllables only"
        );
        counts.push(u8::try_from(entry.korean_text.chars().count())?);
    }
    let starts = allocate(&counts)?;
    // Keep the fixed-label writer and shortened native school consumers in agreement.
    for (id, y, width) in [
        ("gorin_selector_pacific", 120, 100),
        ("gorin_selector_justice", 140, 120),
    ] {
        let entry = fixed_entries
            .iter()
            .find(|entry| entry.id == id)
            .context("missing compact selector school label")?;
        ensure!(
            matches!(&entry.placement, ModeDescendantPlacement::Fixed {bits_per_pixel:4,tim_offset,cell,..} if tim_offset == "0x91800" && *cell == Cell{x:0,y,width,height:20}),
            "selector school label and consumer disagree"
        );
    }
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&config.fonts.gorin_menu.path)?;
    let mut name_pixels = vec![0u8; 240 * 100];
    let mut placements = Vec::new();
    let mut font_sha256 = String::new();
    for (index, entry) in manifest.entries.iter().enumerate() {
        let cell = Cell {
            x: usize::from(starts[index] % 12) * 20,
            y: usize::from(starts[index] / 12) * 20,
            width: usize::from(counts[index]) * 20,
            height: 20,
        };
        let raster = rasterizer.rasterize_shifted(
            &entry.korean_text,
            cell.width,
            cell.height,
            config.fonts.gorin_menu.font_px,
            0.0,
            -3,
            0,
            Some(1),
            15,
            HorizontalTextAlignment::Center,
        )?;
        font_sha256 = raster.font_sha256;
        for row in 0..20 {
            name_pixels[(cell.y + row) * 240 + cell.x..(cell.y + row) * 240 + cell.x + cell.width]
                .copy_from_slice(&raster.pixels[row * cell.width..(row + 1) * cell.width]);
        }
        placements.push(NamePlacement {
            id: entry.id.clone(),
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            source_start: NAMES[index].2,
            source_count: NAMES[index].3,
            start: starts[index],
            count: counts[index],
            cell,
        });
    }
    for record in [
        ModeDescendantRecord::GorinTitle9,
        ModeDescendantRecord::GorinTitle10,
        ModeDescendantRecord::GorinTitle11,
        ModeDescendantRecord::GorinTitle12,
    ] {
        let draft = drafts
            .iter_mut()
            .find(|draft| draft.spec.record == record)
            .context("missing shared Gorin selector texture")?;
        let source = source_for_spec(sources, draft.spec)?;
        let tim = source
            .decoded
            .get(TIM_OFFSET..TIM_OFFSET + TIM_SIZE)
            .context("missing source selector TIM")?;
        ensure!(
            sha256_bytes(tim) == TIM_SHA256,
            "source selector TIM changed"
        );
        for (cell, pixels) in [
            (
                Cell {
                    x: 0,
                    y: 0,
                    width: 240,
                    height: 100,
                },
                name_pixels.clone(),
            ),
            (
                Cell {
                    x: 100,
                    y: 120,
                    width: 60,
                    height: 20,
                },
                vec![0; 60 * 20],
            ),
            (
                Cell {
                    x: 120,
                    y: 140,
                    width: 40,
                    height: 20,
                },
                vec![0; 40 * 20],
            ),
        ] {
            let write = write_indexed_cell_in_prefix_with_report(
                &mut draft.decoded,
                TIM_OFFSET,
                cell,
                &pixels,
            )?;
            draft
                .decoded_write_claims
                .extend(DecodedDataClaim::from_effective_ranges(
                    &format!("gorin-selector-cell-{}-{}", cell.x, cell.y),
                    "repack Korean selector names and clear unused school tails",
                    &source.decoded,
                    &draft.decoded,
                    write.allowed_ranges,
                )?);
        }
    }
    ensure!(
        manifest.speed_markers.len() == 2,
        "both native speed markers are required"
    );
    let marker_style = config
        .fonts
        .gorin_speed_marker
        .as_ref()
        .context("Gorin speed markers require their compact font role")?;
    let marker_rasterizer = rasterizers.for_font(&marker_style.path)?;
    let mut marker_font_sha256 = String::new();
    let mut marker_copies = 0;
    for draft in drafts
        .iter_mut()
        .filter(|draft| draft.spec.source_path.starts_with("DAT2/MINITTL"))
    {
        let source = source_for_spec(sources, draft.spec)?;
        for (entry, (id, japanese, x)) in manifest
            .speed_markers
            .iter()
            .zip([("slow", "遅", 208), ("fast", "速", 232)])
        {
            ensure!(
                entry.id == id && entry.source_text == japanese && !entry.korean_text.is_empty(),
                "speed marker identity changed"
            );
            let cell = Cell {
                x,
                y: 0,
                width: 20,
                height: 20,
            };
            let mut pixels = read_indexed_cell_in_prefix(&source.decoded, 0, cell)?;
            ensure!(
                sha256_bytes(&pixels) == entry.source_region_sha256,
                "speed marker source changed"
            );
            // In its native red CLUT, 1..8 is the original white/gray lettering;
            // 10..15 is the red disk and its edge. Keep the circular silhouette.
            for (offset, pixel) in pixels.iter_mut().enumerate() {
                let x = offset % 20;
                let y = offset / 20;
                if (1..=8).contains(pixel)
                    || (*pixel != 0 && (2..18).contains(&x) && (3..16).contains(&y))
                {
                    *pixel = 15;
                }
            }
            let raster = marker_rasterizer.rasterize_shifted_with_coverage_ramp(
                &entry.korean_text,
                20,
                20,
                marker_style.font_px,
                0.0,
                0,
                0,
                1,
                8,
                HorizontalTextAlignment::Center,
            )?;
            marker_font_sha256 = raster.font_sha256;
            for (pixel, ink) in pixels.iter_mut().zip(raster.pixels) {
                if ink != 0 {
                    ensure!(*pixel != 0, "speed marker ink exceeds the source disk");
                    *pixel = ink;
                }
            }
            let write =
                write_indexed_cell_in_prefix_with_report(&mut draft.decoded, 0, cell, &pixels)?;
            draft
                .decoded_write_claims
                .extend(DecodedDataClaim::from_effective_ranges(
                    &format!("gorin-speed-marker-{id}"),
                    "translate speed labels inside their source red disks",
                    &source.decoded,
                    &draft.decoded,
                    write.allowed_ranges,
                )?);
            marker_copies += 1;
        }
    }
    ensure!(
        marker_copies == 26,
        "speed marker consumer atlas set changed"
    );
    for (spec, base) in SPECS.iter().zip([0x2b0, 0x280]) {
        let source = source_for_spec(sources, spec)?;
        let mut decoded = source.decoded.clone();
        retarget(&mut decoded, base, &starts, &counts)?;
        let ranges = difference_ranges(&source.decoded, &decoded);
        let claims = DecodedDataClaim::from_effective_ranges(
            "gorin-selector-consumers",
            "retarget name coordinates/counts and compact school widths",
            &source.decoded,
            &decoded,
            ranges,
        )?;
        drafts.push(ModeDescendantRecordDraft {
            spec,
            decoded,
            decoded_write_claims: claims,
        });
    }
    Ok(SelectorNamesReport {
        manifest_sha256: sha256_bytes(&bytes),
        font_sha256,
        source_tim_sha256: TIM_SHA256.to_string(),
        physical_texture_copies: 4,
        consumer_tables: 2,
        code_and_character_order_preserved: true,
        names: placements,
        speed_marker_copies: marker_copies,
        speed_marker_font_px: marker_style.font_px,
        speed_marker_font_sha256: marker_font_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_pack_without_overlap_or_row_wrapping() {
        let counts = [
            3, 2, 3, 2, 2, 4, 2, 3, 2, 2, 3, 1, 3, 2, 3, 1, 3, 3, 3, 3, 1, 3,
        ];
        let starts = allocate(&counts).unwrap();
        let mut occupied = [false; 60];
        for (&start, &count) in starts.iter().zip(&counts) {
            assert!(start % 12 + count <= 12);
            for slot in &mut occupied[usize::from(start)..usize::from(start + count)] {
                assert!(!*slot);
                *slot = true;
            }
        }
        assert!(allocate(&[12; 6]).is_err());
        assert!(allocate(&[0]).is_err());
    }
    #[test]
    fn consumer_retargeting_preserves_pointers_and_rejects_source_drift() {
        let base = 72;
        let mut source = vec![0x55; 200];
        for (i, (_, _, s, c)) in NAMES.iter().enumerate() {
            source[base + i * 4..base + i * 4 + 4].copy_from_slice(&[1, *s, *c, 0]);
        }
        let schools = [
            (60, 4, 92),
            (60, 4, 94),
            (64, 4, 92),
            (64, 4, 94),
            (72, 8, 92),
            (72, 8, 94),
            (80, 4, 92),
            (84, 8, 92),
            (84, 8, 94),
        ];
        for (i, (s, c, g)) in schools.iter().enumerate() {
            source[i * 8..i * 8 + 8].copy_from_slice(&[2, *s, *c, *g, 2, 0, 0, 0]);
        }
        let starts = allocate(&[2; 22]).unwrap();
        let mut patched = source.clone();
        retarget(&mut patched, base, &starts, &[2; 22]).unwrap();
        for i in 0..source.len() {
            if source[i] != patched[i] {
                assert!(
                    (i < 72 && i % 8 == 2)
                        || ((72..160).contains(&i) && matches!((i - 72) % 4, 1 | 2))
                );
            }
        }
        assert_eq!(&source[160..], &patched[160..]);
        let mut bad = source.clone();
        bad[base + 3] = 1;
        assert!(retarget(&mut bad, base, &starts, &[2; 22]).is_err());
        let mut bad_starts = starts;
        bad_starts[0] = 11;
        assert!(retarget(&mut source, base, &bad_starts, &[2; 22]).is_err());
    }
}
