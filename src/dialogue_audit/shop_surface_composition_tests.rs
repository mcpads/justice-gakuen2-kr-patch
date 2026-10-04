use crate::compression::{CompressionLimits, compress_with_limits, decompress};
use crate::decoded_record_write_plan::DecodedDataClaim;

use super::ShopSurfaceComposition;

#[test]
fn shop_wrapper_composes_and_reencodes_the_record_pair() {
    let source_decoded = source_fixture(0x5678, 8_192);
    let source_stored = source_stream(&source_decoded);
    let mut fixed_decoded = source_decoded.clone();
    fixed_decoded[10] ^= 1;
    let expected_fixed = fixed_decoded[10];
    let expected_dynamic = source_decoded[20] ^ 1;
    let mut composition = ShopSurfaceComposition::from_verified_sources(
        source_stored,
        fixed_decoded,
        vec![data_claim("shop-fixed", 10, 11)],
        "DAT1/KOUBAI.BIN".to_string(),
        vec![0; 8],
    )
    .unwrap();

    composition
        .apply_surface_writer(
            "shop exit confirmation",
            |shop_ui| {
                shop_ui[20] ^= 1;
                Ok(vec![[20, 21]])
            },
            |overlay| {
                overlay[3] = 9;
                Ok(vec![[3, 4]])
            },
        )
        .unwrap();

    let built = composition.finish().unwrap();
    assert_eq!(built.shop_ui_decoded[10], expected_fixed);
    assert_eq!(built.shop_ui_decoded[20], expected_dynamic);
    assert_eq!(built.overlay[3], 9);
    assert_eq!(
        decompress(&built.shop_ui_stored, true).unwrap(),
        built.shop_ui_decoded
    );
}

fn data_claim(id: &str, start: usize, end: usize) -> DecodedDataClaim {
    DecodedDataClaim {
        id: id.to_string(),
        purpose: "render fixture fixed UI".to_string(),
        range: start..end,
    }
}

fn source_stream(decoded: &[u8]) -> Vec<u8> {
    compress_with_limits(
        decoded,
        512,
        CompressionLimits {
            maximum_match_words: 64,
            maximum_control_block_output_words: 80,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap()
}

fn source_fixture(seed: u32, byte_count: usize) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = Vec::with_capacity(byte_count);
    while bytes.len() < byte_count {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        bytes.extend_from_slice(&((state as u16) & 0x00ff).to_le_bytes());
    }
    bytes.truncate(byte_count);
    let repeated = bytes[5_000..5_512].to_vec();
    bytes[6_000..6_512].copy_from_slice(&repeated);
    bytes
}
