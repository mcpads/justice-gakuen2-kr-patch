use anyhow::{Context, Result, ensure};

use super::model::DiaryHeaderEntry;
use crate::pipeline::sha256_bytes;
use crate::tim::Cell;

// MGAME reads the native club index at 0x801f185b, then copies the two
// bytes of table[index] into a 64x20 sprite's u/v fields. See the source
// consumer evidence; the adjacent atlas rows do not have a 24-pixel pitch.
const CLUB_CELLS: [(&str, usize, usize); 9] = [
    ("baseball-club", 192, 48),
    ("volleyball-club", 192, 108),
    ("soccer-club", 192, 88),
    ("sumo-club", 192, 68),
    ("art-club", 128, 64),
    ("wind-music-club", 128, 104),
    ("drama-club", 64, 84),
    ("chemistry-club", 64, 64),
    ("go-home-club", 128, 84),
];

pub(super) fn validate_club_labels(source: &[u8], entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (start, end, expected) in [
        (
            0x1c74,
            0x1c86,
            "fa6b712afb3c7f95e8cbae8c1f894890564a095c557b45dd27787e8bcd6e52ad",
        ),
        (
            0x14000,
            0x141f8,
            "ba156ef4736c8c77aa61ba8d36ecc4092cee9e77ba4b5731dcacd75392ad8389",
        ),
    ] {
        let bytes = source
            .get(start..end)
            .context("truncated native club-label consumer")?;
        ensure!(
            sha256_bytes(bytes) == expected,
            "native club-label consumer changed"
        );
    }
    validate_entries(entries)
}

fn validate_entries(entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (id, x, y) in CLUB_CELLS {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("missing native club label {id}"))?;
        ensure!(
            entry.cell
                == Cell {
                    x,
                    y,
                    width: 64,
                    height: 20
                },
            "club label {id} differs from its native sprite cell"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diary_header::assets::load_diary_header_assets;
    use std::path::Path;

    #[test]
    #[ignore = "requires assets/"]
    fn club_labels_preserve_full_ink_inside_native_height() {
        use crate::development_build_spec::load_development_build_spec;
        use crate::diary_header::build::rasterize_entry;
        use crate::font::IndexedTextRasterizer;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let spec =
            load_development_build_spec(&root.join("assets/build/development.json")).unwrap();
        let style = &spec.fonts.diary_header.club_label;
        let rasterizer = IndexedTextRasterizer::load(&style.path).unwrap();
        let (_, entries) = load_diary_header_assets(&root.join("assets/diary/header")).unwrap();
        for mut entry in entries
            .into_iter()
            .filter(|e| CLUB_CELLS.iter().any(|(id, _, _)| *id == e.id))
        {
            let raster = rasterize_entry(
                &rasterizer,
                &entry,
                style,
                &entry.layout,
                style.font_px,
                style.glyph_layout.vertical_shift_px,
            )
            .unwrap();
            let width = entry.cell.width;
            let height = entry.cell.height;
            entry.cell.height += 4;
            let padded = rasterize_entry(
                &rasterizer,
                &entry,
                style,
                &entry.layout,
                style.font_px,
                style.glyph_layout.vertical_shift_px,
            )
            .unwrap();
            assert!(
                padded.pixels[..2 * width].iter().all(|p| *p == 0),
                "{} top clipped",
                entry.id
            );
            assert!(
                padded.pixels[(height + 2) * width..]
                    .iter()
                    .all(|p| *p == 0),
                "{} bottom clipped",
                entry.id
            );
            assert_eq!(
                raster.pixels,
                padded.pixels[2 * width..(height + 2) * width],
                "{} ink lost",
                entry.id
            );
            eprintln!("{} {:?}", entry.id, raster.ink_bounds);
        }
    }

    #[test]
    #[ignore = "requires assets/"]
    fn club_labels_fit_native_sprites_without_overwriting_the_next_row() {
        let (_, mut entries) = load_diary_header_assets(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/diary/header")
                .as_path(),
        )
        .unwrap();
        validate_entries(&entries).unwrap();
        entries
            .iter_mut()
            .find(|entry| entry.id == "baseball-club")
            .unwrap()
            .cell
            .height = 24;
        assert!(validate_entries(&entries).is_err());
    }
}
