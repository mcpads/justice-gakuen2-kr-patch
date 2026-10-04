#[path = "font/indexed_text.rs"]
mod indexed_text;
#[path = "font/model.rs"]
mod model;
#[path = "font/preview.rs"]
mod preview;
#[path = "font/raster.rs"]
mod raster;

pub(crate) use indexed_text::centered_glyph_ink_spans;
pub use indexed_text::{
    IndexedTextRasterizer, IndexedTextRasterizers, rasterize_indexed_text,
    rasterize_indexed_text_with_coverage_ramp, rasterize_shifted_indexed_text_with_coverage_ramp,
};
pub use model::{
    GlyphFit, HorizontalTextAlignment, MenuFontPreviewConfig, MenuFontPreviewManifest,
    RasterizedIndexedText, RasterizedMenuGlyph, RasterizedMenuGlyphSet, SizeFit,
};
pub use preview::build_menu_font_preview;
pub use raster::rasterize_menu_glyphs;

#[cfg(test)]
#[path = "font_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "font/supported_fonts_tests.rs"]
mod supported_fonts_tests;

#[cfg(test)]
#[path = "font/indexed_text_tests.rs"]
mod indexed_text_tests;

#[path = "font/indexed_transform.rs"]
mod indexed_transform;
pub(crate) use indexed_transform::rotate_indexed_raster;
