use super::{PagedCompressionProfile, compress_page_safe_image, profile_paged_compression};
use crate::compression::{CompressionLimits, compress_with_limits, decompress};

#[test]
fn long_zero_arena_preserves_the_following_script_data() {
    let decoded = long_zero_arena();

    let source_profile = PagedCompressionProfile::from_source_contract(1_073, 2_146, 0, 0x7000);
    let compressed = compress_page_safe_image(&decoded, source_profile).unwrap();
    let rebuilt_profile = profile_paged_compression(&compressed).unwrap();

    assert_eq!(decompress(&compressed, false).unwrap(), decoded);
    assert_eq!(rebuilt_profile.control_blocks_crossing_input_pages, 0);
    assert_eq!(
        (rebuilt_profile.stream_byte_count - 1) / 0x800,
        (source_profile.stream_byte_count - 1) / 0x800
    );
}

#[test]
fn narrow_final_page_preserves_the_decoded_tail() {
    let decoded = long_zero_arena();
    // A source slot may end only a few bytes into its final input page. The
    // terminator must reach that page without truncating the following script.
    for final_page_bytes in [4, 18] {
        let source_profile = PagedCompressionProfile::from_source_contract(
            1_073,
            2_146,
            0,
            0x4000 + final_page_bytes,
        );
        let compressed = super::compress_on_source_final_page_inner(
            &decoded,
            source_profile,
            source_profile.stream_byte_count,
            None,
            false,
            0,
            None,
        )
        .unwrap();
        assert!(compressed.len() > 0x4000 && compressed.len() <= 0x4000 + final_page_bytes);
        assert_eq!(decompress(&compressed, false).unwrap(), decoded);
    }
}

#[test]
fn page_safe_compression_selects_the_shorter_valid_candidate() {
    let decoded = long_zero_arena();
    let source_profile = PagedCompressionProfile::from_source_contract(1_073, 2_146, 0, 0x7000);
    let valid_direct_candidate = compress_with_limits(
        &decoded,
        16,
        CompressionLimits {
            maximum_match_words: source_profile.maximum_match_words,
            maximum_control_block_output_words: 51,
            input_page_bytes: Some(0x800),
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap();
    let selected = compress_page_safe_image(&decoded, source_profile).unwrap();
    let source_final_page = (source_profile.stream_byte_count - 1) / 0x800;

    for candidate in [&valid_direct_candidate, &selected] {
        let profile = profile_paged_compression(candidate).unwrap();

        assert_eq!(decompress(candidate, false).unwrap(), decoded);
        assert_eq!(profile.control_blocks_crossing_input_pages, 0);
        assert_eq!((profile.stream_byte_count - 1) / 0x800, source_final_page);
        assert!(profile.stream_byte_count <= source_profile.stream_byte_count);
    }
    assert!(selected.len() < valid_direct_candidate.len());
}

fn long_zero_arena() -> Vec<u8> {
    let mut decoded = vec![0u8; 80_000];
    decoded.extend_from_slice(b"SCRIPT-DATA");
    if !decoded.len().is_multiple_of(2) {
        decoded.push(0);
    }
    decoded
}

#[test]
fn equivalent_extended_matches_fill_a_narrow_final_page_without_new_output() {
    let input = vec![0x11; 12_000];
    let limits = CompressionLimits {
        maximum_match_words: 7,
        maximum_control_block_output_words: 112,
        input_page_bytes: None,
        single_word_token_prefix_words: 0,
        input_page_output_limit: None,
    };
    let encoded = compress_with_limits(&input, 16, limits).unwrap();
    let selection = super::SourceFinalPageSelection {
        source_final_page: encoded.len() + 8,
        output_slot_capacity: encoded.len() + 16,
        reject_control_blocks_crossing_input_pages: false,
        single_word_prefix_floor_words: 16,
        input_page_output_limit: Some(crate::compression::InputPageOutputLimit {
            input_page_bytes: 0x800,
            maximum_output_words: 10000,
        }),
    };
    let padded = super::extend_nearby_short_matches(&encoded, selection)
        .unwrap()
        .unwrap();
    assert_eq!(padded.len(), encoded.len() + 10);
    assert_eq!(decompress(&padded, false).unwrap(), input);
    assert_eq!(
        profile_paged_compression(&padded)
            .unwrap()
            .maximum_match_words,
        7
    );
    assert_eq!(&padded[..34], &encoded[..34]);
}
