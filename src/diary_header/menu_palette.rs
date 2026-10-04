use anyhow::{Result, ensure};

use super::model::{DiaryHeaderFontStyle, DiaryHeaderIndexedRendering};

// Native white, selected yellow and cyan heading palettes run from bright
// index 1 to dark index 15. Increasing coverage is not increasing brightness.
pub(super) fn validate(style: &DiaryHeaderFontStyle, source_tim: &[u8]) -> Result<()> {
    let DiaryHeaderIndexedRendering::Outlined {
        fill_index,
        outline_index,
    } = style.rendering
    else {
        anyhow::bail!("Diary menu lettering requires separate bright fill and dark outline");
    };
    ensure!(
        fill_index < 16 && outline_index < 16,
        "invalid menu palette index"
    );
    for palette in [4usize, 5, 6] {
        let color = |index: u8| -> Result<[u16; 3]> {
            let offset = 20 + (palette * 16 + usize::from(index)) * 2;
            let bytes = source_tim
                .get(offset..offset + 2)
                .ok_or_else(|| anyhow::anyhow!("truncated menu palette"))?;
            let word = u16::from_le_bytes(bytes.try_into()?);
            Ok([word & 31, (word >> 5) & 31, (word >> 10) & 31])
        };
        let fill = color(fill_index)?;
        let outline = color(outline_index)?;
        ensure!(
            fill.iter().copied().max().unwrap() >= 30
                && outline.iter().all(|channel| *channel <= 2),
            "Diary menu palette {palette} must have bright fill and dark outline"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diary_header::model::DiaryHeaderFontRole;
    use crate::diary_header::{assets::load_diary_header_assets, build::rasterize_entry};

    #[test]
    #[ignore = "requires assets/ and the original disc in roms/"]
    fn menu_fill_is_bright_in_each_native_state_and_outlines_fit() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let spec = crate::development_build_spec::load_development_build_spec(
            &root.join("assets/build/development.json"),
        )
        .unwrap();
        let style = &spec.fonts.diary_header.action_label;
        let source = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (_, stored) = source.read_record("DAT2/MGCOCK.TIZ").unwrap();
        let tim = crate::compression::decompress(&stored, false).unwrap();
        validate(style, &tim).unwrap();
        let mut reversed = style.clone();
        reversed.rendering = DiaryHeaderIndexedRendering::Outlined {
            fill_index: 15,
            outline_index: 1,
        };
        assert!(validate(&reversed, &tim).is_err());
        reversed.rendering = DiaryHeaderIndexedRendering::CoverageRamp {
            first_ink_index: 1,
            last_ink_index: 15,
        };
        assert!(validate(&reversed, &tim).is_err());
        let rasterizer = crate::font::IndexedTextRasterizer::load(&style.path).unwrap();
        let (_, entries) = load_diary_header_assets(&root.join("assets/diary/header")).unwrap();
        for mut entry in entries
            .into_iter()
            .filter(|e| e.font_role == DiaryHeaderFontRole::ActionLabel)
        {
            let width = entry.cell.width;
            let height = entry.cell.height;
            let size = entry.font_px.unwrap_or(style.font_px);
            let shift = entry
                .vertical_shift_px
                .unwrap_or(style.glyph_layout.vertical_shift_px);
            let raster =
                rasterize_entry(&rasterizer, &entry, style, &entry.layout, size, shift).unwrap();
            entry.cell.width += 4;
            entry.cell.height += 4;
            let padded =
                rasterize_entry(&rasterizer, &entry, style, &entry.layout, size, shift).unwrap();
            for y in 0..height + 4 {
                for x in 0..width + 4 {
                    let expected = if (2..width + 2).contains(&x) && (2..height + 2).contains(&y) {
                        raster.pixels[(y - 2) * width + x - 2]
                    } else {
                        0
                    };
                    assert_eq!(
                        padded.pixels[y * (width + 4) + x],
                        expected,
                        "{} outline clipped at ({x},{y})",
                        entry.id
                    );
                }
            }
        }
    }
}
