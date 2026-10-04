use super::*;

fn synthetic_tim() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x10u32.to_le_bytes());
    data.extend_from_slice(&0x08u32.to_le_bytes());
    data.extend_from_slice(&44u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&140u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 128]);
    data
}

fn synthetic_tim_without_clut() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x10u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&20u32.to_le_bytes());
    data.extend_from_slice(&832u16.to_le_bytes());
    data.extend_from_slice(&256u16.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&[0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f]);
    data
}

fn synthetic_8bpp_tim() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x10u32.to_le_bytes());
    data.extend_from_slice(&0x09u32.to_le_bytes());
    data.extend_from_slice(&524u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&483u16.to_le_bytes());
    data.extend_from_slice(&256u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 512]);
    data.extend_from_slice(&28u32.to_le_bytes());
    data.extend_from_slice(&832u16.to_le_bytes());
    data.extend_from_slice(&256u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend(0u8..16);
    data
}

#[test]
fn parses_valid_tim() {
    let data = synthetic_tim();
    let tim = parse_4bpp(&data).unwrap();
    assert_eq!(tim.pixel_width(), 16);
    assert_eq!(tim.image_height, 16);
}

#[test]
fn embedded_tim_prefix_reports_its_own_boundary() {
    let mut data = synthetic_tim();
    let tim_size = data.len();
    data.extend_from_slice(&[0xaa, 0xbb]);

    let tim = parse_4bpp_prefix(&data).unwrap();
    assert_eq!(tim.total_size, tim_size);
    assert!(parse_4bpp(&data).is_err());
}

#[test]
fn indexed_glyph_install_is_confined_to_its_cell() {
    let mut data = synthetic_tim();
    let pixels = vec![14u8; 16];
    let metadata = install_indexed_glyph(
        &mut data,
        Cell {
            x: 4,
            y: 4,
            width: 4,
            height: 4,
        },
        &pixels,
        "synthetic glyph",
    )
    .unwrap();
    assert_eq!(metadata.palette_histogram[14], 16);
    assert_eq!(metadata.changed_decoded_byte_count, 8);
}

#[test]
fn embedded_indexed_glyph_install_reports_asset_relative_offsets() {
    let prefix = [0x55; 12];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_tim());
    data.extend_from_slice(&[0xaa, 0xbb]);
    let pixels = vec![13u8; 16];

    let metadata = install_indexed_glyph_in_prefix(
        &mut data,
        prefix.len(),
        Cell {
            x: 4,
            y: 4,
            width: 4,
            height: 4,
        },
        &pixels,
        "embedded glyph",
    )
    .unwrap();

    assert_eq!(metadata.pixel_data_offset, prefix.len() + 64);
    assert!(
        metadata
            .allowed_decoded_byte_ranges
            .iter()
            .all(|[start, end]| *start >= prefix.len() + 64
                && *end <= prefix.len() + synthetic_tim().len())
    );
    assert_eq!(&data[..prefix.len()], &prefix);
    assert_eq!(&data[data.len() - 2..], &[0xaa, 0xbb]);
}

#[test]
fn embedded_indexed_cell_read_preserves_low_then_high_nibble_order() {
    let prefix = [0x55; 12];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_tim());
    let tim = parse_4bpp_prefix(&data[prefix.len()..]).unwrap();
    data[prefix.len() + tim.pixel_offset] = 0x21;

    let pixels = read_indexed_cell_in_prefix(
        &data,
        prefix.len(),
        Cell {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        },
    )
    .unwrap();

    assert_eq!(pixels, [1, 2]);
}

#[test]
fn full_indexed_image_preserves_low_then_high_nibble_order() {
    let mut data = synthetic_tim();
    let tim = parse_4bpp_prefix(&data).unwrap();
    data[tim.pixel_offset] = 0x21;
    data[tim.pixel_offset + 1] = 0x43;

    let image = read_4bpp_indexed_image_in_prefix(&data, 0).unwrap();

    assert_eq!(image.width, 16);
    assert_eq!(image.height, 16);
    assert_eq!(&image.pixels[..4], &[1, 2, 3, 4]);
}

#[test]
fn rgba_decode_uses_playstation_bgr555_channel_order() {
    let mut data = synthetic_tim();
    data[20..22].copy_from_slice(&0u16.to_le_bytes());
    data[22..24].copy_from_slice(&0x001fu16.to_le_bytes());
    data[24..26].copy_from_slice(&0x03e0u16.to_le_bytes());
    data[26..28].copy_from_slice(&0x7c00u16.to_le_bytes());
    let tim = parse_4bpp_prefix(&data).unwrap();
    data[tim.pixel_offset] = 0x21;
    data[tim.pixel_offset + 1] = 0x03;

    let image = decode_4bpp_rgba_in_prefix(&data, 0, 0).unwrap();

    assert_eq!(
        &image.pixels[..16],
        &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0,]
    );
}

#[test]
fn palette_read_preserves_playstation_color_words() {
    let mut data = synthetic_tim();
    data[20..22].copy_from_slice(&0x001fu16.to_le_bytes());
    data[22..24].copy_from_slice(&0x03e0u16.to_le_bytes());

    let palette = read_4bpp_palette_words_in_prefix(&data, 0, 0).unwrap();

    assert_eq!(&palette[..2], &[0x001f, 0x03e0]);
}

#[test]
fn rgba_decode_rejects_a_palette_outside_the_clut() {
    let error = decode_4bpp_rgba_in_prefix(&synthetic_tim(), 0, 1).unwrap_err();
    assert!(error.to_string().contains("palette index"));
}

#[test]
fn embedded_indexed_cell_write_preserves_pixels_outside_the_cell() {
    let prefix = [0x55; 7];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_tim());
    data.extend_from_slice(&[0xaa, 0xbb]);
    let before = data.clone();
    let cell = Cell {
        x: 3,
        y: 4,
        width: 5,
        height: 2,
    };
    let replacement = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let ranges = write_indexed_cell_in_prefix(&mut data, prefix.len(), cell, &replacement).unwrap();

    assert_eq!(
        read_indexed_cell_in_prefix(&data, prefix.len(), cell).unwrap(),
        replacement
    );
    assert_eq!(ranges.len(), cell.height);
    assert_eq!(&data[..prefix.len()], &before[..prefix.len()]);
    assert_eq!(&data[data.len() - 2..], &[0xaa, 0xbb]);
    assert_eq!(
        read_indexed_cell_in_prefix(
            &data,
            prefix.len(),
            Cell {
                x: 2,
                y: 4,
                width: 1,
                height: 2,
            },
        )
        .unwrap(),
        [0, 0]
    );
}

#[test]
fn clutless_indexed_cell_roundtrip_preserves_pixels_outside_the_cell() {
    let prefix = [0x55; 7];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_tim_without_clut());
    data.extend_from_slice(&[0xaa, 0xbb]);
    let before = data.clone();
    let cell = Cell {
        x: 1,
        y: 0,
        width: 3,
        height: 2,
    };
    let replacement = [1, 2, 3, 4, 5, 6];

    let write = write_indexed_cell_without_clut_in_prefix_with_report(
        &mut data,
        prefix.len(),
        cell,
        &replacement,
    )
    .unwrap();

    assert_eq!(write.allowed_ranges.len(), cell.height);
    assert_eq!(write.changed_byte_count, 4);
    assert_eq!(
        read_indexed_cell_without_clut_in_prefix(&data, prefix.len(), cell).unwrap(),
        replacement
    );
    assert_eq!(&data[..prefix.len()], &before[..prefix.len()]);
    assert_eq!(&data[data.len() - 2..], &[0xaa, 0xbb]);
    assert_eq!(
        read_indexed_cell_without_clut_in_prefix(
            &data,
            prefix.len(),
            Cell {
                x: 0,
                y: 0,
                width: 1,
                height: 2,
            },
        )
        .unwrap(),
        [1, 9]
    );
}

#[test]
fn eight_bpp_indexed_cell_roundtrip_preserves_pixels_outside_the_cell() {
    let mut data = synthetic_8bpp_tim();
    let before = data.clone();
    let cell = Cell {
        x: 2,
        y: 0,
        width: 4,
        height: 2,
    };
    let replacement = [255, 1, 2, 3, 4, 5, 6, 7];

    let ranges = write_8bpp_indexed_cell(&mut data, cell, &replacement).unwrap();

    assert_eq!(read_8bpp_indexed_cell(&data, cell).unwrap(), replacement);
    assert_eq!(ranges.len(), 2);
    let tim = parse_8bpp(&data).unwrap();
    assert_eq!(tim.pixel_width(), 8);
    assert_eq!(tim.image_height, 2);
    assert_eq!(
        read_8bpp_indexed_cell(
            &data,
            Cell {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }
        )
        .unwrap(),
        [0, 1, 8, 9]
    );
    assert_eq!(&data[..tim.pixel_offset], &before[..tim.pixel_offset]);
}

#[test]
fn embedded_eight_bpp_cell_reader_accepts_prefix_and_trailing_bytes() {
    let prefix = [0x55; 7];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_8bpp_tim());
    data.extend_from_slice(&[0xaa, 0xbb]);

    assert_eq!(
        read_8bpp_indexed_cell_in_prefix(
            &data,
            prefix.len(),
            Cell {
                x: 2,
                y: 0,
                width: 4,
                height: 2,
            },
        )
        .unwrap(),
        [2, 3, 4, 5, 10, 11, 12, 13]
    );
}

#[test]
fn embedded_eight_bpp_cell_write_preserves_prefix_and_trailing_bytes() {
    let prefix = [0x55; 7];
    let mut data = prefix.to_vec();
    data.extend_from_slice(&synthetic_8bpp_tim());
    data.extend_from_slice(&[0xaa, 0xbb]);
    let before = data.clone();
    let cell = Cell {
        x: 2,
        y: 0,
        width: 4,
        height: 2,
    };
    let replacement = [255, 1, 2, 3, 4, 5, 6, 7];

    let ranges =
        write_8bpp_indexed_cell_in_prefix(&mut data, prefix.len(), cell, &replacement).unwrap();

    assert_eq!(
        read_8bpp_indexed_cell_in_prefix(&data, prefix.len(), cell).unwrap(),
        replacement
    );
    assert_eq!(ranges.len(), cell.height);
    assert_eq!(&data[..prefix.len()], &before[..prefix.len()]);
    assert_eq!(&data[data.len() - 2..], &[0xaa, 0xbb]);
}

#[test]
fn embedded_eight_bpp_palette_reader_preserves_color_words() {
    let prefix = [0x55; 7];
    let mut data = prefix.to_vec();
    let mut tim = synthetic_8bpp_tim();
    tim[20..22].copy_from_slice(&0x001fu16.to_le_bytes());
    tim[20 + 255 * 2..22 + 255 * 2].copy_from_slice(&0x7fffu16.to_le_bytes());
    data.extend_from_slice(&tim);

    let palette = read_8bpp_palette_words_in_prefix(&data, prefix.len(), 0).unwrap();

    assert_eq!(palette[0], 0x001f);
    assert_eq!(palette[255], 0x7fff);
}

#[test]
fn eight_bpp_palette_reader_rejects_a_palette_outside_the_clut() {
    let error = read_8bpp_palette_words_in_prefix(&synthetic_8bpp_tim(), 0, 1).unwrap_err();
    assert!(error.to_string().contains("palette index"));
}
