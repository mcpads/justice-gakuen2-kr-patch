use super::{extract_wrapped_menu_cell, physical_cell_position};
use crate::menu_atlas::{MENU_ATLAS_HEIGHT, MENU_ATLAS_PAGE_WIDTH};

#[test]
fn physical_cell_position_wraps_uv_inside_the_selected_texture_page() {
    assert_eq!(physical_cell_position(0x03ff), (3, 15, 15, 44, 44));
}

#[test]
fn extracted_cell_wraps_both_axes_without_crossing_texture_pages() {
    let row_bytes = 1024 / 2;
    let mut image = vec![0u8; row_bytes * MENU_ATLAS_HEIGHT];
    for y in 0..MENU_ATLAS_HEIGHT {
        for x in 0..MENU_ATLAS_PAGE_WIDTH {
            let color = if y < 4 {
                3
            } else if x < 4 {
                2
            } else {
                1
            };
            set_pixel(&mut image, row_bytes, x, y, color);
        }
    }

    let pixels = extract_wrapped_menu_cell(&image, row_bytes, 0x00cc).unwrap();
    assert_eq!(
        &pixels[..10],
        &[0x11; 8].into_iter().chain([0x22; 2]).collect::<Vec<_>>()
    );
    assert_eq!(&pixels[16 * 10..17 * 10], &[0x33; 10]);
}

fn set_pixel(image: &mut [u8], row_bytes: usize, x: usize, y: usize, color: u8) {
    let offset = y * row_bytes + x / 2;
    let shift = 4 * (x & 1);
    image[offset] = (image[offset] & !(0x0f << shift)) | (color << shift);
}
