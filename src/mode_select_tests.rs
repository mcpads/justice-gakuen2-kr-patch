use crate::tim::Cell;

use super::assets::detail_title_region;
use super::build::{description_line_cells, difference_ranges};

#[test]
fn detail_titles_own_the_full_shared_atlas_slot() {
    let mut overlay = vec![0; 0x0514];
    let descriptor_offset = 0x0508 + 6;
    for (index, value) in [0x02c0_u16, 0x0078, 0x0070].into_iter().enumerate() {
        let offset = descriptor_offset + index * 2;
        overlay[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    assert_eq!(
        detail_title_region(&overlay, 1).unwrap(),
        Cell {
            x: 888,
            y: 112,
            width: 112,
            height: 24,
        }
    );
}

#[test]
fn description_lines_are_centered_and_keep_horizontal_padding() {
    let cells = description_line_cells(
        Cell {
            x: 0,
            y: 0,
            width: 176,
            height: 112,
        },
        5,
        19,
    )
    .unwrap();

    assert_eq!(cells.len(), 5);
    assert_eq!(cells[0].x, 4);
    assert_eq!(cells[0].width, 168);
    assert_eq!(cells[0].y, 8);
    assert_eq!(cells[4].y + cells[4].height, 103);
}

#[test]
fn description_layout_rejects_text_beyond_the_panel() {
    assert!(
        description_line_cells(
            Cell {
                x: 0,
                y: 0,
                width: 176,
                height: 112,
            },
            7,
            18,
        )
        .is_err()
    );
}

#[test]
fn changed_byte_ranges_preserve_disjoint_edits() {
    let before = [0, 0, 0, 0, 0, 0, 0];
    let after = [0, 1, 1, 0, 2, 0, 3];

    assert_eq!(difference_ranges(&before, &after), [[1, 3], [4, 5], [6, 7]]);
}

#[test]
fn title_alignment_centers_ink_in_both_directions_without_clipping() {
    let mut pixels = vec![0; 40];
    pixels[8] = 14;
    pixels[31] = 3;
    super::build::align_mode_title_in_panel(&mut pixels, 20, 7).unwrap();
    assert_eq!(pixels[2], 14);
    assert_eq!(pixels[25], 3);
    super::build::align_mode_title_in_panel(&mut pixels, 20, 23).unwrap();
    assert_eq!(pixels[10], 14);
    assert_eq!(pixels[33], 3);
    assert_eq!(pixels.iter().filter(|&&p| p != 0).count(), 2);
    let before = pixels.clone();
    assert!(super::build::align_mode_title_in_panel(&mut pixels, 20, 0).is_err());
    assert_eq!(pixels, before);
}
