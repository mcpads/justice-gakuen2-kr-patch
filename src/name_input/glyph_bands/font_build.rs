use super::*;
use crate::font::rasterize_menu_glyphs;
use crate::pipeline::sha256_bytes;
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NameGlyphBandBuildReport {
    pub kind: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub supported_syllable_count: usize,
    pub crop: [usize; 4],
    pub pack_bytes: usize,
    pub pack_sha256: String,
    pub storage_capacity_bytes: usize,
    pub storage_bytes_remaining: usize,
    pub exact_fill_roundtrip_verified: bool,
}

pub struct NameGlyphBandBuild {
    pub pack: NameGlyphBandPack,
    pub report: NameGlyphBandBuildReport,
}

pub fn build_name_glyph_band_pack(
    font: &Path,
    font_px: f32,
    capacity: usize,
) -> Result<NameGlyphBandBuild> {
    let repertoire: String = crate::name_input::load_name_input_hangul()?
        .into_iter()
        .collect();
    let reference = rasterize_menu_glyphs(font, &repertoire, font_px, 3, 13)?;
    let pack = NameGlyphBandPack::build(&reference, capacity)?;
    let report = NameGlyphBandBuildReport {
        kind: "lossless horizontal bitmap-band name glyphs".into(),
        font_name: reference.font_name,
        font_sha256: reference.font_sha256,
        font_px,
        supported_syllable_count: pack.glyph_count(),
        crop: pack.crop(),
        pack_bytes: pack.bytes().len(),
        pack_sha256: sha256_bytes(pack.bytes()),
        storage_capacity_bytes: capacity,
        storage_bytes_remaining: capacity - pack.bytes().len(),
        exact_fill_roundtrip_verified: true,
    };
    Ok(NameGlyphBandBuild { pack, report })
}
