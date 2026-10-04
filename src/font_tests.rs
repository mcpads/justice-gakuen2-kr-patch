use super::preview::scale_nearest;
use super::raster::{boundary_has_pixel, dilate};

#[test]
fn one_pixel_dilation_expands_all_neighbors() {
    let mut source = vec![false; 25];
    source[2 * 5 + 2] = true;
    let dilated = dilate(&source, 5, 5, 1);
    assert_eq!(dilated.iter().filter(|pixel| **pixel).count(), 9);
    assert!(!boundary_has_pixel(&dilated, 5, 5));
}

#[test]
fn nearest_scaling_preserves_blocks() {
    assert_eq!(scale_nearest(&[0, 1], 2, 1, 2), [0, 0, 1, 1, 0, 0, 1, 1]);
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn gothic_dialogue_preserves_bottom_strokes_with_outline() {
    let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Lv2_Gothic/TTF/NEXON Lv2 Gothic.ttf");
    let glyphs = super::rasterize_menu_glyphs(&font, "강공홍흉읽읊", 17.0, 3, 14).unwrap();
    assert!(
        glyphs
            .glyphs
            .iter()
            .all(|glyph| glyph.fit.fits_with_one_pixel_outline)
    );
}
