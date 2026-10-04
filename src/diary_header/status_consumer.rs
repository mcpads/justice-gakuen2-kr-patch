use anyhow::{Context, Result, ensure};

use super::model::{DiaryHeaderEntry, DiaryHeaderFontStyle, DiaryHeaderIndexedRendering};
use crate::font::RasterizedIndexedText;
use crate::tim::Cell;

// Both original MGAME status coordinate tables select these 48x16 sprites.
// Native packet readback confirms that the upper atlas row starts at V=32,
// not V=24; the unused eight rows above it are not rendered.
const COORDINATES: [u8; 12] = [0, 48, 48, 48, 96, 48, 144, 48, 80, 32, 128, 32];
const LABELS: [&str; 6] = [
    "attack",
    "defense",
    "health",
    "guts",
    "potential",
    "intellect",
];

pub(super) fn validate_status_labels(source: &[u8], entries: &[DiaryHeaderEntry]) -> Result<()> {
    for offset in [0x2254, 0x316c] {
        ensure!(
            source.get(offset..offset + COORDINATES.len()) == Some(COORDINATES.as_slice()),
            "native status-label coordinate table changed at MGAME +{offset:#x}"
        );
    }
    validate_entries(entries)
}

fn validate_entries(entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (id, uv) in LABELS.into_iter().zip(COORDINATES.as_chunks::<2>().0) {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("missing native status label {id}"))?;
        ensure!(
            entry.cell
                == Cell {
                    x: usize::from(uv[0]),
                    y: usize::from(uv[1]),
                    width: 48,
                    height: 16
                },
            "status label {id} differs from its native sprite cell"
        );
    }
    Ok(())
}

pub(super) fn validate_vertical_outline(
    entry: &DiaryHeaderEntry,
    style: &DiaryHeaderFontStyle,
    raster: &RasterizedIndexedText,
) -> Result<()> {
    if !LABELS.contains(&entry.id.as_str()) {
        return Ok(());
    }
    if let DiaryHeaderIndexedRendering::Outlined { fill_index, .. } = style.rendering {
        let width = entry.cell.width;
        let last_row = (entry.cell.height - 1) * width;
        ensure!(
            !raster.pixels[..width].contains(&fill_index)
                && !raster.pixels[last_row..].contains(&fill_index),
            "status label {} leaves no room for its vertical outline",
            entry.id
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
    fn status_labels_preserve_the_outline_rendered_on_a_taller_canvas() {
        use crate::development_build_spec::load_development_build_spec;
        use crate::diary_header::build::rasterize_entry;
        use crate::font::IndexedTextRasterizer;

        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let spec =
            load_development_build_spec(&root.join("assets/build/development.json")).unwrap();
        let style = &spec.fonts.diary_header.status_label;
        let rasterizer = IndexedTextRasterizer::load(&style.path).unwrap();
        let (_, entries) = load_diary_header_assets(&root.join("assets/diary/header")).unwrap();
        let mut rejected_previous_shift = false;
        for mut entry in entries
            .into_iter()
            .filter(|entry| LABELS.contains(&entry.id.as_str()))
        {
            let size = entry.font_px.unwrap_or(style.font_px);
            let shift = entry
                .vertical_shift_px
                .unwrap_or(style.glyph_layout.vertical_shift_px);
            let raster =
                rasterize_entry(&rasterizer, &entry, style, &entry.layout, size, shift).unwrap();
            validate_vertical_outline(&entry, style, &raster).unwrap();
            let old = rasterize_entry(&rasterizer, &entry, style, &entry.layout, size, -2).unwrap();
            rejected_previous_shift |= validate_vertical_outline(&entry, style, &old).is_err();

            let width = entry.cell.width;
            let height = entry.cell.height;
            entry.cell.height += 4;
            let padded =
                rasterize_entry(&rasterizer, &entry, style, &entry.layout, size, shift).unwrap();
            assert!(
                padded.pixels[..2 * width].iter().all(|pixel| *pixel == 0),
                "{} top outline clipped",
                entry.id
            );
            assert!(
                padded.pixels[(height + 2) * width..]
                    .iter()
                    .all(|pixel| *pixel == 0),
                "{} bottom outline clipped",
                entry.id
            );
            assert_eq!(
                raster.pixels,
                padded.pixels[2 * width..(height + 2) * width],
                "{} differs from padded rendering",
                entry.id
            );
        }
        assert!(rejected_previous_shift);
    }

    #[test]
    #[ignore = "requires assets/"]
    fn status_labels_reject_the_unconsumed_upper_atlas_rows() {
        let (_, mut entries) = load_diary_header_assets(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/diary/header"),
        )
        .unwrap();
        validate_entries(&entries).unwrap();
        for id in ["potential", "intellect"] {
            let entry = entries.iter_mut().find(|entry| entry.id == id).unwrap();
            let original = entry.cell;
            entry.cell.y = 24;
            entry.cell.height = 24;
            assert!(validate_entries(&entries).is_err());
            entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .cell = original;
        }
    }
}
