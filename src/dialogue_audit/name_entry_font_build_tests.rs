use super::name_entry_font_build::glyph_cell_from_triplet;

#[test]
fn glyph_triplets_map_three_texture_pages_without_crossing_boundaries() {
    let first = glyph_cell_from_triplet(0x0c, 0, 0).unwrap();
    let last_hiragana = glyph_cell_from_triplet(0x0c, 11, 6).unwrap();
    let first_katakana = glyph_cell_from_triplet(0x0d, 0, 0).unwrap();
    let last_alphanumeric = glyph_cell_from_triplet(0x0e, 11, 6).unwrap();

    assert_eq!((first.cell.x, first.cell.y), (0, 0));
    assert_eq!((last_hiragana.cell.x, last_hiragana.cell.y), (220, 120));
    assert_eq!((first_katakana.cell.x, first_katakana.cell.y), (256, 0));
    assert_eq!(
        (last_alphanumeric.cell.x, last_alphanumeric.cell.y),
        (732, 120)
    );
}

#[test]
fn glyph_triplets_reject_unknown_pages_and_out_of_grid_cells() {
    assert!(glyph_cell_from_triplet(0x0b, 0, 0).is_err());
    assert!(glyph_cell_from_triplet(0x0f, 0, 0).is_err());
    assert!(glyph_cell_from_triplet(0x0c, 12, 0).is_err());
    assert!(glyph_cell_from_triplet(0x0c, 0, 7).is_err());
}
