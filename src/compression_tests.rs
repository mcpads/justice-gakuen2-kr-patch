use super::*;

#[test]
fn roundtrips_deterministically() {
    let input = b"ABABABABABABABAB0000000000000000";
    let first = compress(input, 2).unwrap();
    let second = compress(input, 2).unwrap();
    assert_eq!(first, second);
    assert_eq!(decompress(&first, false).unwrap(), input);
}

#[test]
fn optimized_match_search_preserves_nearest_longest_match_selection() {
    let mut state = 0x82a9_5b17_u32;
    let mut words = Vec::with_capacity(4_096);
    for index in 0..4_096 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let word = if index >= 257 && index % 11 < 7 {
            words[index - 257]
        } else if index >= 43 && index % 17 < 5 {
            words[index - 43]
        } else {
            (state as u16) & 0x003f
        };
        words.push(word);
    }

    let mut positions = WordPositionTable::default();
    for index in 0..words.len() {
        for maximum in [2, 7, 31, 127, 1_073] {
            assert_eq!(
                find_longest_match(&words, &mut positions, index, maximum),
                reference_longest_match(&words, index, maximum),
                "match selection changed at word {index} with maximum {maximum}"
            );
        }
        positions.add(words[index], index);
    }
}

#[test]
fn encoder_reserves_zero_distance_for_terminator() {
    let encoded = compress(&vec![0; 4096], 16).unwrap();
    let mut offset = 0usize;
    let mut terminated = false;
    while !terminated {
        let mut control = read_u16(&encoded, &mut offset).unwrap();
        for _ in 0..16 {
            if control & 0x8000 != 0 {
                let token = read_u16(&encoded, &mut offset).unwrap();
                let mut length = token >> 11;
                let distance = token & 0x07ff;
                if length == 0 {
                    length = read_u16(&encoded, &mut offset).unwrap();
                }
                if distance == 0 {
                    assert_eq!(length, 0);
                    terminated = true;
                    break;
                }
            } else {
                read_u16(&encoded, &mut offset).unwrap();
            }
            control <<= 1;
        }
    }
    assert_eq!(offset, encoded.len());
}

#[test]
fn encoder_respects_a_streaming_consumers_match_limit() {
    let mut input = vec![0u8; 80_000];
    input.extend_from_slice(b"SCRIPT-DATA");
    if !input.len().is_multiple_of(2) {
        input.push(0);
    }

    let encoded = compress_with_maximum_match_words(&input, 16, 1_073).unwrap();

    assert_eq!(decompress(&encoded, false).unwrap(), input);
    assert_eq!(maximum_encoded_match_words(&encoded), 1_073);
}

#[test]
fn terminal_control_block_remains_available_for_page_placement_tuning() {
    let mut emitter = Emitter::new(Some(0x800));
    emitter.emit(&[0x1234], false, 1).unwrap();
    for _ in 0..4 {
        emitter.emit(&[1, 1], true, 1).unwrap();
    }
    for _ in 5..(59 * 16 + 15) {
        emitter.emit(&[0x1234], false, 1).unwrap();
    }
    emitter.emit(&[0, 0], true, 0).unwrap();
    let candidate = emitter.finish().unwrap();

    assert_eq!(candidate.len(), 0x802);
    assert_eq!(decompress(&candidate, false).unwrap().len(), 959 * 2);
}

#[test]
fn seeded_control_block_preserves_a_runtime_catalog_prefix() {
    let mut words = (0_u16..64).collect::<Vec<_>>();
    words[2..8].fill(2);
    let input = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    let mut prefix = Vec::new();
    prefix.extend_from_slice(&0x1000_u16.to_le_bytes());
    for word in &words[..3] {
        prefix.extend_from_slice(&word.to_le_bytes());
    }
    prefix.extend_from_slice(&((5_u16 << 11) | 1).to_le_bytes());
    for word in &words[8..20] {
        prefix.extend_from_slice(&word.to_le_bytes());
    }

    let encoded = compress_with_seeded_control_block(
        &input,
        &prefix,
        CompressionLimits {
            maximum_match_words: 64,
            maximum_control_block_output_words: 80,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap();

    assert!(encoded.starts_with(&prefix));
    assert_eq!(decompress(&encoded, false).unwrap(), input);
}

#[test]
fn seeded_control_blocks_preserve_the_unchanged_source_stream_prefix() {
    let source = (0_u16..1024).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    let source_encoded = compress(&source, 0).unwrap();
    let mut patched = source.clone();
    patched[1536..].fill(0);
    let prefix = complete_control_block_prefix(&source_encoded, 1536).unwrap();

    let encoded = compress_with_seeded_control_blocks(
        &patched,
        &source_encoded[..prefix.encoded_byte_count],
        CompressionLimits {
            maximum_match_words: 1024,
            maximum_control_block_output_words: 1024,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
    .unwrap();

    assert!(prefix.decoded_byte_count <= 1536);
    assert!(encoded.starts_with(&source_encoded[..prefix.encoded_byte_count]));
    assert_eq!(decompress(&encoded, false).unwrap(), patched);
}

#[test]
fn rejects_trailing_bytes_unless_allowed() {
    let mut encoded = compress(b"ABCD", 0).unwrap();
    encoded.extend_from_slice(&[0, 0]);
    assert!(decompress(&encoded, false).is_err());
    assert_eq!(decompress(&encoded, true).unwrap(), b"ABCD");
}

fn maximum_encoded_match_words(encoded: &[u8]) -> usize {
    let mut offset = 0usize;
    let mut maximum = 0usize;
    loop {
        let mut control = read_u16(encoded, &mut offset).unwrap();
        for _ in 0..16 {
            if control & 0x8000 != 0 {
                let token = read_u16(encoded, &mut offset).unwrap();
                let mut length = usize::from(token >> 11);
                let distance = token & 0x07ff;
                if length == 0 {
                    length = usize::from(read_u16(encoded, &mut offset).unwrap());
                }
                if distance == 0 && length == 0 {
                    return maximum;
                }
                maximum = maximum.max(length);
            } else {
                read_u16(encoded, &mut offset).unwrap();
            }
            control <<= 1;
        }
    }
}

fn reference_longest_match(
    words: &[u16],
    index: usize,
    maximum_match_words: usize,
) -> (usize, usize) {
    let minimum = index.saturating_sub(MAX_DISTANCE);
    let maximum = maximum_match_words.min(words.len() - index);
    let mut best = (0, 0);
    for candidate in (minimum..index).rev() {
        if words[candidate] != words[index] {
            continue;
        }
        let distance = index - candidate;
        let mut length = 1;
        while length < maximum && words[index + length] == words[index + length - distance] {
            length += 1;
        }
        if length > best.0 {
            best = (length, distance);
            if length == maximum {
                break;
            }
        }
    }
    best
}

#[test]
fn decoder_page_work_is_bounded_across_block_boundaries_and_seeded_prefixes() {
    let mut input = (0..1800_u16).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    input.extend(vec![0x11; 80_000]);
    input.extend_from_slice(b"FOLLOWING-SCRIPT");
    let mut limits = CompressionLimits {
        maximum_match_words: 1085,
        maximum_control_block_output_words: 1100,
        input_page_bytes: None,
        single_word_token_prefix_words: 0,
        input_page_output_limit: None,
    };
    let unbounded = compress_with_limits(&input, 16, limits).unwrap();
    assert!(
        crate::paged_compression::profile_paged_compression(&unbounded)
            .unwrap()
            .maximum_input_page_output_words
            > 2824
    );
    limits.input_page_output_limit = Some(InputPageOutputLimit {
        input_page_bytes: 0x800,
        maximum_output_words: 2824,
    });
    let prefix = complete_control_block_prefix(&unbounded, 3500).unwrap();
    for encoded in [
        compress_with_limits(&input, 16, limits).unwrap(),
        compress_with_seeded_control_blocks(
            &input,
            &unbounded[..prefix.encoded_byte_count],
            limits,
        )
        .unwrap(),
    ] {
        assert_eq!(decompress(&encoded, false).unwrap(), input);
        let profile = crate::paged_compression::profile_paged_compression(&encoded).unwrap();
        assert!(profile.maximum_input_page_output_words <= 2824);
        assert!(profile.maximum_match_words <= 1085);
        assert!(profile.maximum_control_block_output_words <= 1100);
    }
}
