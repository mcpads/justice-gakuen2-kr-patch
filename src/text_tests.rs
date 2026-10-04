use super::*;

#[test]
fn shared_menu_ascii_map_matches_the_source_atlas_rows() {
    assert_eq!(common_menu_ascii_glyph_code('1'), Some(0x0000));
    assert_eq!(common_menu_ascii_glyph_code('0'), Some(0x0009));
    assert_eq!(common_menu_ascii_glyph_code('A'), Some(0x000a));
    assert_eq!(common_menu_ascii_glyph_code('B'), Some(0x000b));
    assert_eq!(common_menu_ascii_glyph_code('C'), Some(0x0010));
    assert_eq!(common_menu_ascii_glyph_code('N'), Some(0x001b));
    assert_eq!(common_menu_ascii_glyph_code('O'), Some(0x0020));
    assert_eq!(common_menu_ascii_glyph_code('Z'), Some(0x002b));
    assert_eq!(common_menu_ascii_glyph_code('a'), Some(0x0030));
    assert_eq!(common_menu_ascii_glyph_code('l'), Some(0x003b));
    assert_eq!(common_menu_ascii_glyph_code('m'), Some(0x0040));
    assert_eq!(common_menu_ascii_glyph_code('x'), Some(0x004b));
    assert_eq!(common_menu_ascii_glyph_code('y'), Some(0x0050));
    assert_eq!(common_menu_ascii_glyph_code('z'), Some(0x0051));
    assert_eq!(common_menu_ascii_glyph_code('@'), Some(0x0052));
    assert_eq!(common_menu_ascii_glyph_code('?'), Some(0x0055));
    assert_eq!(common_menu_ascii_glyph_code('+'), Some(0x005a));
    assert_eq!(common_menu_ascii_glyph_code('!'), Some(0x0063));
    assert_eq!(common_menu_ascii_glyph_code('.'), None);
    assert_eq!(common_menu_ascii_glyph_code('"'), None);
    assert_eq!(common_menu_ascii_glyph_code('\''), None);
    assert_eq!(common_menu_ascii_glyph_code('_'), None);
    assert_eq!(common_menu_ascii_glyph_code(':'), None);
}

#[test]
fn maps_runtime_codes_to_twenty_pixel_atlas_cells() {
    assert_eq!(atlas_position(0x0010).unwrap().x, 0);
    assert_eq!(atlas_position(0x0010).unwrap().y, 20);
    assert_eq!(atlas_position(0x0206).unwrap().x, 632);
    assert_eq!(atlas_position(0x0218).unwrap().x, 672);
    assert_eq!(atlas_position(0x0218).unwrap().y, 20);
    assert_eq!(atlas_position(0x0300).unwrap().x, 768);
}

#[test]
fn wraps_high_rows_and_columns_like_the_byte_sized_renderer_uvs() {
    assert_eq!(atlas_position(0x000d).unwrap().x, 4);
    assert_eq!(atlas_position(0x00d0).unwrap().y, 4);
    assert_eq!(atlas_position(0x02fd).unwrap().x, 516);
    assert_eq!(atlas_position(0x02fd).unwrap().y, 44);
}

#[test]
fn reads_and_replaces_a_bounded_code() {
    let mut data = vec![3, 0, 0x18, 0x02, 0x19, 0x02, 0x95, 0x01];
    assert_eq!(
        read_length_prefixed_codes(&data, 0).unwrap(),
        [0x0218, 0x0219, 0x0195]
    );
    replace_code(&mut data, 2, 0x0218, 0x0029).unwrap();
    assert_eq!(read_length_prefixed_codes(&data, 0).unwrap()[0], 0x0029);
}
