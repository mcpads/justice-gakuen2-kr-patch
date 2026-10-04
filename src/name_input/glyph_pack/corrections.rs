//! Sparse fill toggles restore a compositional glyph before its outline is drawn.
use super::{NameGlyphPack, NameGlyphPackDecoder};
use crate::font::RasterizedMenuGlyphSet;
use anyhow::{Result, ensure};

pub struct NameGlyphFillCorrections {
    bytes: Vec<u8>,
    crop: [usize; 4],
    entries: Vec<(u16, std::ops::Range<usize>)>,
}

impl NameGlyphFillCorrections {
    /// Entries are sorted syllable offsets (u16 LE), a nonzero byte count,
    /// and strictly increasing crop-relative fill coordinates (u8).
    pub fn parse(bytes: Vec<u8>, crop: [usize; 4]) -> Result<Self> {
        let [x, y, width, height] = crop;
        ensure!(
            width > 0
                && height > 0
                && x < 20
                && y < 20
                && width <= 20 - x
                && height <= 20 - y
                && width * height <= 256,
            "invalid correction crop"
        );
        let mut entries = Vec::new();
        let mut cursor = 0;
        let mut previous = None;
        while cursor < bytes.len() {
            ensure!(bytes.len() - cursor >= 3, "truncated correction header");
            let syllable = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into()?);
            ensure!(
                syllable < 11172 && previous.is_none_or(|p| p < syllable),
                "invalid or unordered correction syllable"
            );
            let count = usize::from(bytes[cursor + 2]);
            let start = cursor + 3;
            ensure!(
                count > 0 && count <= bytes.len() - start,
                "truncated or empty correction entry"
            );
            let end = start + count;
            let coordinates = &bytes[start..end];
            ensure!(
                coordinates.iter().all(|&c| usize::from(c) < width * height)
                    && coordinates.windows(2).all(|p| p[0] < p[1]),
                "invalid or duplicate correction coordinate"
            );
            entries.push((syllable, start..end));
            previous = Some(syllable);
            cursor = end;
        }
        Ok(Self {
            bytes,
            crop,
            entries,
        })
    }

    pub fn build(pack: &NameGlyphPack, reference: &RasterizedMenuGlyphSet) -> Result<Self> {
        ensure!(
            reference.font_sha256 == pack.report.font_sha256,
            "correction reference font differs from pack"
        );
        let decoder = NameGlyphPackDecoder::new(&pack.bytes)?;
        Self::parse(Vec::new(), pack.report.crop)?;
        ensure!(
            decoder.crop_bounds() == pack.report.crop,
            "pack report crop differs from bytes"
        );
        let [x, y, width, height] = pack.report.crop;
        let mut bytes = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut glyphs: Vec<_> = reference.glyphs.iter().collect();
        glyphs.sort_by_key(|g| g.character);
        for glyph in &glyphs {
            ensure!(
                seen.insert(glyph.character) && glyph.pixels.len() == 400,
                "duplicate or malformed correction reference"
            );
            let generated = decoder.render(glyph.character)?;
            let mut coordinates = Vec::new();
            for (index, &pixel) in generated.iter().enumerate() {
                if (pixel == 13) != (glyph.pixels[index] == 13) {
                    let (px, py) = (index % 20, index / 20);
                    ensure!(
                        px >= x && px < x + width && py >= y && py < y + height,
                        "reference fill leaves pack crop"
                    );
                    coordinates.push(u8::try_from((py - y) * width + px - x)?);
                }
            }
            if !coordinates.is_empty() {
                bytes.extend_from_slice(
                    &u16::try_from(glyph.character as u32 - 0xac00)?.to_le_bytes(),
                );
                bytes.push(u8::try_from(coordinates.len())?);
                bytes.extend_from_slice(&coordinates);
            }
        }
        ensure!(
            (0xac00..=0xd7a3)
                .filter_map(char::from_u32)
                .filter(|&c| decoder.supports(c))
                .eq(seen.iter().copied()),
            "correction reference does not cover pack repertoire"
        );
        let correction = Self::parse(bytes, pack.report.crop)?;
        for glyph in glyphs {
            ensure!(
                correction.render(&decoder, glyph.character)? == glyph.pixels,
                "corrected glyph differs from reference raster"
            );
        }
        Ok(correction)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn render(&self, decoder: &NameGlyphPackDecoder<'_>, character: char) -> Result<Vec<u8>> {
        ensure!(
            decoder.crop_bounds() == self.crop,
            "correction and glyph crops differ"
        );
        let generated = decoder.render(character)?;
        self.apply(character, &generated)
    }

    fn apply(&self, character: char, generated: &[u8]) -> Result<Vec<u8>> {
        ensure!(
            generated.len() == 400 && ('\u{ac00}'..='\u{d7a3}').contains(&character),
            "invalid correction glyph"
        );
        let mut fill: Vec<bool> = generated.iter().map(|&p| p == 13).collect();
        let code = u16::try_from(character as u32 - 0xac00)?;
        if let Ok(index) = self.entries.binary_search_by_key(&code, |e| e.0) {
            let [x, y, width, _] = self.crop;
            for &coordinate in &self.bytes[self.entries[index].1.clone()] {
                let c = usize::from(coordinate);
                let index = (y + c / width) * 20 + x + c % width;
                fill[index] = !fill[index];
            }
        }
        Ok((0_usize..400)
            .map(|index| {
                let (x, y) = (index % 20, index / 20);
                let outline = (y.saturating_sub(1)..=(y + 1).min(19)).any(|row| {
                    (x.saturating_sub(1)..=(x + 1).min(19)).any(|col| fill[row * 20 + col])
                });
                if fill[index] {
                    13
                } else if outline {
                    3
                } else {
                    0
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggles_remove_old_ink_and_regenerate_outline_without_touching_other_glyphs() {
        let correction =
            NameGlyphFillCorrections::parse(vec![0, 0, 2, 0, 1], [4, 4, 10, 10]).unwrap();
        let mut original = vec![0; 400];
        original[84] = 13;
        original[63] = 3;
        let corrected = correction.apply('가', &original).unwrap();
        assert_eq!(corrected[84], 3);
        assert_eq!(corrected[85], 13);
        assert_eq!(corrected[63], 0);
        assert_eq!(corrected[66], 3);
        let untouched = correction.apply('각', &original).unwrap();
        assert_eq!(untouched[84], 13);
        assert_eq!(untouched[85], 3);
    }
    #[test]
    fn malformed_corrections_are_rejected_before_rendering() {
        for bytes in [
            vec![0],
            vec![0, 0, 0],
            vec![0, 0, 2, 1],
            vec![0, 0, 1, 100],
            vec![0, 0, 2, 1, 1],
            vec![1, 0, 1, 0, 0, 0, 1, 0],
            vec![0xa4, 0x2b, 1, 0],
        ] {
            assert!(NameGlyphFillCorrections::parse(bytes, [4, 4, 10, 10]).is_err());
        }
        assert!(NameGlyphFillCorrections::parse(vec![], [20, 4, 1, 1]).is_err());
        assert!(NameGlyphFillCorrections::parse(vec![], [0, 0, 20, 20]).is_err());
    }
}
