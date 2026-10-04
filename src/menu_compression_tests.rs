use crate::compression::{CompressionLimits, compress_with_limits, decompress};

use super::{compress_menu_with_source_limits, profile_menu_stream};

#[test]
fn rebuilt_stream_stays_within_source_expansion_limits() {
    let source_decoded = repeated_fixture(0x1234, 8_192);
    let mut source_stored = compress_with_limits(
        &source_decoded,
        16,
        CompressionLimits {
            maximum_match_words: 64,
            maximum_control_block_output_words: 80,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap();
    source_stored.resize(16_384, 0);
    let source_profile = profile_menu_stream(&source_stored).unwrap();
    let mut rebuilt_decoded = source_decoded;
    rebuilt_decoded[2_048..4_096].fill(0x5a);

    let (rebuilt_stored, admitted_source, rebuilt_profile) =
        compress_menu_with_source_limits(&rebuilt_decoded, &source_stored).unwrap();

    assert_eq!(admitted_source, source_profile);
    assert!(rebuilt_profile.maximum_match_words <= source_profile.maximum_match_words);
    assert!(
        rebuilt_profile.maximum_control_block_output_words
            <= source_profile.maximum_control_block_output_words
    );
    assert_eq!(rebuilt_profile.control_blocks_crossing_input_pages, 0);
    assert_eq!(
        (rebuilt_profile.stream_byte_count - 1) / 0x800,
        (source_profile.stream_byte_count - 1) / 0x800
    );
    assert_eq!(decompress(&rebuilt_stored, false).unwrap(), rebuilt_decoded);
}

#[test]
fn stream_profile_stops_at_the_terminator_before_record_padding() {
    let decoded = repeated_fixture(0x4321, 256);
    let mut stored = compress_with_limits(
        &decoded,
        16,
        CompressionLimits {
            maximum_match_words: 32,
            maximum_control_block_output_words: 48,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap();
    let stream_byte_count = stored.len();
    stored.resize(stream_byte_count + 128, 0);

    assert_eq!(
        profile_menu_stream(&stored).unwrap().stream_byte_count,
        stream_byte_count
    );
}

fn repeated_fixture(word: u16, byte_count: usize) -> Vec<u8> {
    let pair = word.to_le_bytes();
    (0..byte_count)
        .map(|index| pair[index % pair.len()])
        .collect()
}
