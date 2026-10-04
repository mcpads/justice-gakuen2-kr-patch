use crate::tim::{Cell, IndexedImage};

use super::assets::ensure_tight_non_clear_bounds;
use super::audit::{read_resident_4bpp_pixels, verify_exact_texture_residency};

const VRAM_BYTES: usize = 1024 * 512 * 2;

#[test]
fn runtime_texture_read_follows_ps1_word_nibble_order() {
    let mut gpu = vec![0u8; VRAM_BYTES];
    let word_offset = (3 * 1024 + 5) * 2;
    gpu[word_offset..word_offset + 2].copy_from_slice(&0x4321u16.to_le_bytes());

    assert_eq!(
        read_resident_4bpp_pixels(&gpu, 5, 3, 4, 1).unwrap(),
        [1, 2, 3, 4]
    );
}

#[test]
fn runtime_texture_read_detects_one_changed_vram_pixel() {
    let mut gpu = vec![0u8; VRAM_BYTES];
    let first = read_resident_4bpp_pixels(&gpu, 0, 0, 8, 2).unwrap();
    gpu[0] = 0x10;
    let changed = read_resident_4bpp_pixels(&gpu, 0, 0, 8, 2).unwrap();

    let error = verify_exact_texture_residency(&first, &changed).unwrap_err();

    assert!(error.to_string().contains("exact MA_TIT title texture"));
}

#[test]
fn source_boundary_requires_ink_on_every_outer_edge() {
    let tight = IndexedImage {
        width: 4,
        height: 3,
        pixels: vec![1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1],
    };
    let padded = IndexedImage {
        width: 4,
        height: 4,
        pixels: vec![0; 16],
    };
    let cell = Cell {
        x: 0,
        y: 0,
        width: 4,
        height: 3,
    };

    ensure_tight_non_clear_bounds(&tight, cell, 0).unwrap();
    assert!(ensure_tight_non_clear_bounds(&padded, cell, 0).is_err());
}
