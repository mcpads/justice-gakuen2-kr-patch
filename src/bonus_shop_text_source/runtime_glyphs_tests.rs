use psx_r3000a::encode;

use super::runtime_glyphs::{protected_runtime_glyph_codes, source_consumer_instructions};
use crate::bonus_shop_source::OVERLAY_RUNTIME_BASE;

#[test]
fn runtime_consumers_protect_digits_separator_and_heading_quotes() {
    let protected = protected_runtime_glyph_codes(&source_overlay_fixture()).unwrap();

    assert_eq!(
        protected.into_iter().collect::<Vec<_>>(),
        [
            0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x0059,
            0x0197, 0x0198,
        ]
    );
}

#[test]
fn digit_lookup_and_consumer_instruction_drift_are_rejected() {
    let mut lookup_drift = source_overlay_fixture();
    lookup_drift[0x3a14] = 0;
    assert!(protected_runtime_glyph_codes(&lookup_drift).is_err());

    let mut instruction_drift = source_overlay_fixture();
    instruction_drift[0x82b4..0x82b8].fill(0);
    assert!(protected_runtime_glyph_codes(&instruction_drift).is_err());
}

fn source_overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 0x90c0];
    overlay[0x3a14..0x3a1e].copy_from_slice(&[9, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
    for (offset, instruction) in source_consumer_instructions() {
        let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
        overlay[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    overlay
}

#[test]
fn heading_quote_page_and_uv_drift_are_rejected() {
    for offset in [0x7aa8, 0x7b70, 0x7b78, 0x7d80, 0x7e3c, 0x7e44] {
        let mut overlay = source_overlay_fixture();
        overlay[offset..offset + 4].fill(0);
        assert!(protected_runtime_glyph_codes(&overlay).is_err());
    }
}
