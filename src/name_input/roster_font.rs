//! One font supplier for narrow roster Hangul and ASCII, appended to MA_ENT.
use super::*;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::path::Path;

const ORIGINAL_DECODED_BYTES: usize = 0x33800;
const MAXIMUM_DECODED_BYTES: usize = 0x38000;

#[derive(Clone, Debug, Serialize)]
pub struct RosterNameFontReport {
    pub hangul: NameGlyphBandBuildReport,
    pub ascii_sha256: String,
    pub ascii_crop: [usize; 4],
    pub ascii_bytes: usize,
    pub pack_offset: usize,
    pub ascii_offset: usize,
    pub decoded_byte_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RosterNameFont {
    pub pack: NameGlyphBandPack,
    pub ascii: SelectorAsciiGlyphs,
    pub report: RosterNameFontReport,
}

impl RosterNameFont {
    pub fn build(font: &Path, font_px: f32) -> Result<Self> {
        let hangul = build_name_glyph_band_pack(font, font_px, NAME_GLYPH_PACK_STORAGE_BYTES)?;
        let reference = crate::font::rasterize_menu_glyphs(
            font,
            &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
            font_px,
            3,
            13,
        )?;
        let ascii = SelectorAsciiGlyphs::build(&reference, 4096)?;
        let [hx, _, hw, _] = hangul.pack.crop();
        let [ax, _, aw, _] = ascii.crop;
        let outlined_width = (hx + hw).max(ax + aw) - hx.min(ax) + 2;
        ensure!(
            outlined_width <= super::roster_adapter::ROSTER_ADVANCE as usize
                && 3 * super::roster_adapter::ROSTER_ADVANCE as usize + outlined_width <= 56,
            "roster font outlines overlap character or column boundaries"
        );
        let ascii_offset = ORIGINAL_DECODED_BYTES + hangul.pack.bytes().len().next_multiple_of(4);
        let decoded_byte_count = ascii_offset + ascii.bytes.len().next_multiple_of(4);
        ensure!(
            decoded_byte_count <= MAXIMUM_DECODED_BYTES,
            "roster font exceeds reserved appended font load extent"
        );
        let report = RosterNameFontReport {
            hangul: hangul.report,
            ascii_sha256: crate::pipeline::sha256_bytes(&ascii.bytes),
            ascii_crop: ascii.crop,
            ascii_bytes: ascii.bytes.len(),
            pack_offset: ORIGINAL_DECODED_BYTES,
            ascii_offset,
            decoded_byte_count,
        };
        Ok(Self {
            pack: hangul.pack,
            ascii,
            report,
        })
    }

    pub fn append_to(&self, decoded: &mut Vec<u8>) -> Result<()> {
        ensure!(
            decoded.len() == ORIGINAL_DECODED_BYTES,
            "roster font append requires the original decoded MA_ENT extent"
        );
        decoded.extend_from_slice(self.pack.bytes());
        decoded.resize(self.report.ascii_offset, 0);
        decoded.extend_from_slice(&self.ascii.bytes);
        decoded.resize(self.report.decoded_byte_count, 0);
        Ok(())
    }

    pub fn loader_copies(
        &self,
        font_base: u32,
        pack: u32,
        ascii: u32,
    ) -> Result<Vec<SelectorDataCopy>> {
        super::selector_loader::ram_range(font_base, self.report.decoded_byte_count)?;
        Ok(vec![
            SelectorDataCopy {
                source: font_base + self.report.pack_offset as u32,
                destination: pack,
                byte_count: self.pack.bytes().len().next_multiple_of(4),
            },
            SelectorDataCopy {
                source: font_base + self.report.ascii_offset as u32,
                destination: ascii,
                byte_count: self.ascii.bytes.len().next_multiple_of(4),
            },
        ])
    }
}
