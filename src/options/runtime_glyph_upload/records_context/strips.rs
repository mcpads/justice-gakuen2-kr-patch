//! Fit large raw uploads into existing fragmented padding without a decoder.
use super::PreparedGlyph;
use anyhow::{Result, ensure};

pub(super) fn split_large_uploads(glyphs: &[PreparedGlyph]) -> Result<Vec<PreparedGlyph>> {
    let mut uploads = Vec::new();
    for glyph in glyphs {
        if glyph.payload.len() <= 200 {
            uploads.push(glyph.clone());
            continue;
        }
        let row_bytes = glyph.cell.width / 2;
        ensure!(
            row_bytes > 0
                && row_bytes <= 100
                && row_bytes.is_multiple_of(4)
                && glyph.payload.len() == row_bytes * glyph.cell.height,
            "large Records upload cannot form word-aligned row strips"
        );
        let rows = 100 / row_bytes;
        for (index, payload) in glyph.payload.chunks(rows * row_bytes).enumerate() {
            let mut strip = glyph.clone();
            strip.cell.y += index * rows;
            strip.cell.height = payload.len() / row_bytes;
            strip.payload = payload.to_vec();
            uploads.push(strip);
        }
    }
    Ok(uploads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tim::Cell;

    fn glyph(code: u16, side: usize, source: bool) -> PreparedGlyph {
        PreparedGlyph {
            role: "test".into(),
            character: None,
            code,
            cell: Cell {
                x: 40,
                y: 40,
                width: side,
                height: side,
            },
            source_preservation: source,
            payload: (0..side * side / 2)
                .map(|n| (n + usize::from(source)) as u8)
                .collect(),
        }
    }

    #[test]
    fn strips_reconstruct_every_entry_and_restore_pixel_at_the_same_destination() -> Result<()> {
        for source in [false, true] {
            let original = glyph(0x100, 40, source);
            let strips = split_large_uploads(std::slice::from_ref(&original))?;
            let mut observed = vec![None; original.payload.len()];
            for strip in strips {
                assert_eq!(strip.cell.x, original.cell.x);
                assert_eq!(strip.cell.width, original.cell.width);
                assert_eq!(strip.code, original.code);
                assert_eq!(strip.source_preservation, source);
                assert!(strip.payload.len() <= 100 && strip.payload.len().is_multiple_of(4));
                let start = (strip.cell.y - original.cell.y) * original.cell.width / 2;
                for (at, byte) in strip.payload.iter().enumerate() {
                    assert!(observed[start + at].replace(*byte).is_none());
                }
            }
            assert_eq!(
                observed,
                original
                    .payload
                    .iter()
                    .copied()
                    .map(Some)
                    .collect::<Vec<_>>()
            );
        }
        let small = glyph(0x101, 20, false);
        let strips = split_large_uploads(std::slice::from_ref(&small))?;
        assert_eq!(strips.len(), 1);
        assert_eq!(strips[0].cell, small.cell);
        assert_eq!(strips[0].payload, small.payload);
        Ok(())
    }

    #[test]
    fn english_records_population_fits_existing_padding_after_strip_allocation() -> Result<()> {
        let mut glyphs = (0..71)
            .map(|code| glyph(code, 20, false))
            .collect::<Vec<_>>();
        glyphs.extend([glyph(71, 40, false), glyph(72, 40, false)]);
        let original = glyphs.iter().map(|g| g.payload.len()).collect::<Vec<_>>();
        assert!(super::super::storage::allocate_storage(&original, &original).is_err());
        let strips = split_large_uploads(&glyphs)?;
        let sizes = strips.iter().map(|g| g.payload.len()).collect::<Vec<_>>();
        let layout = super::super::storage::allocate_storage(&sizes, &sizes)?;
        assert_eq!(sizes.iter().sum::<usize>(), original.iter().sum::<usize>());
        let mut ranges = layout
            .entry_payload_offsets
            .iter()
            .chain(&layout.exit_restore_payload_offsets)
            .zip(sizes.iter().chain(&sizes))
            .map(|(start, size)| [*start, *start + size])
            .collect::<Vec<_>>();
        ranges.sort_unstable();
        assert!(ranges.windows(2).all(|p| p[0][1] <= p[1][0]));
        Ok(())
    }
}
