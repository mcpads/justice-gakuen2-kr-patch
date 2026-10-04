use anyhow::{Result, bail, ensure};

use crate::compression::{
    CompleteControlBlockPrefix, CompressionLimits, InputPageOutputLimit,
    complete_control_block_prefix, compress_with_limits, compress_with_seeded_control_blocks,
};

#[cfg(test)]
#[path = "paged_compression_tests.rs"]
mod tests;

const PAGED_STREAM_LITERAL_PREFIX_WORDS: usize = 16;
const PAGED_STREAM_INPUT_PAGE_BYTES: usize = 0x800;
const MINIMUM_CONTROL_BLOCK_OUTPUT_WORDS: usize = 16;

#[derive(Clone, Copy)]
struct SourceFinalPageSelection {
    source_final_page: usize,
    output_slot_capacity: usize,
    reject_control_blocks_crossing_input_pages: bool,
    single_word_prefix_floor_words: usize,
    input_page_output_limit: Option<InputPageOutputLimit>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct PagedCompressionProfile {
    pub(crate) maximum_match_words: usize,
    pub(crate) maximum_control_block_output_words: usize,
    pub(crate) control_blocks_crossing_input_pages: usize,
    pub(crate) stream_byte_count: usize,
    #[serde(default)]
    pub(crate) maximum_input_page_output_words: usize,
}

impl PagedCompressionProfile {
    pub(crate) fn from_source_contract(
        maximum_match_words: usize,
        maximum_control_block_output_words: usize,
        control_blocks_crossing_input_pages: usize,
        stream_byte_count: usize,
    ) -> Self {
        Self {
            maximum_match_words,
            maximum_control_block_output_words,
            control_blocks_crossing_input_pages,
            stream_byte_count,
            maximum_input_page_output_words: 0,
        }
    }
}

pub(crate) fn source_paged_compression_profile(stored: &[u8]) -> Result<PagedCompressionProfile> {
    profile_paged_compression(stored)
}

pub(crate) fn compress_page_safe_image(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
) -> Result<Vec<u8>> {
    compress_page_safe_image_inner(
        decoded,
        source_profile,
        source_profile.stream_byte_count,
        None,
    )
}

pub(crate) fn compress_page_safe_image_in_slot(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
    output_slot_capacity: usize,
) -> Result<Vec<u8>> {
    compress_page_safe_image_inner(decoded, source_profile, output_slot_capacity, None)
}

pub(crate) fn compress_page_safe_image_with_seeded_control_block(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
    encoded_control_block: &[u8],
) -> Result<Vec<u8>> {
    compress_page_safe_image_inner(
        decoded,
        source_profile,
        source_profile.stream_byte_count,
        Some(encoded_control_block),
    )
}

pub(crate) fn compress_page_safe_image_with_seeded_control_block_in_slot(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
    encoded_control_block: &[u8],
    output_slot_capacity: usize,
) -> Result<Vec<u8>> {
    compress_page_safe_image_inner(
        decoded,
        source_profile,
        output_slot_capacity,
        Some(encoded_control_block),
    )
}

pub(crate) fn compress_on_source_final_page_with_preserved_prefix(
    decoded: &[u8],
    source_encoded: &[u8],
    source_profile: PagedCompressionProfile,
    unchanged_decoded_prefix_byte_count: usize,
) -> Result<(Vec<u8>, CompleteControlBlockPrefix)> {
    let prefix =
        complete_control_block_prefix(source_encoded, unchanged_decoded_prefix_byte_count)?;
    ensure!(
        prefix.encoded_byte_count > 0 && prefix.decoded_byte_count > 0,
        "source stream has no complete control block inside its unchanged decoded prefix"
    );
    let compressed = compress_on_source_final_page_inner(
        decoded,
        source_profile,
        source_profile.stream_byte_count,
        Some(&source_encoded[..prefix.encoded_byte_count]),
        false,
        prefix.decoded_byte_count / 2,
        None,
    )?;
    ensure!(
        compressed.starts_with(&source_encoded[..prefix.encoded_byte_count]),
        "rebuilt stream did not preserve its source control-block prefix"
    );
    Ok((compressed, prefix))
}

pub(crate) fn compress_with_source_page_work_limit(
    decoded: &[u8],
    source_encoded: &[u8],
    unchanged_decoded_prefix_byte_count: usize,
) -> Result<Vec<u8>> {
    let profile = source_paged_compression_profile(source_encoded)?;
    let prefix =
        complete_control_block_prefix(source_encoded, unchanged_decoded_prefix_byte_count)?;
    ensure!(
        prefix.encoded_byte_count > 0,
        "source has no preserved control block"
    );
    let compressed = compress_on_source_final_page_inner(
        decoded,
        profile,
        profile.stream_byte_count,
        Some(&source_encoded[..prefix.encoded_byte_count]),
        false,
        prefix.decoded_byte_count / 2,
        Some(InputPageOutputLimit {
            input_page_bytes: PAGED_STREAM_INPUT_PAGE_BYTES,
            maximum_output_words: profile.maximum_input_page_output_words,
        }),
    )?;
    ensure!(
        profile_paged_compression(&compressed)?.maximum_input_page_output_words
            <= profile.maximum_input_page_output_words,
        "rebuilt decoder input-page expansion exceeds source work budget"
    );
    Ok(compressed)
}

fn compress_page_safe_image_inner(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
    output_slot_capacity: usize,
    encoded_control_block: Option<&[u8]>,
) -> Result<Vec<u8>> {
    compress_on_source_final_page_inner(
        decoded,
        source_profile,
        output_slot_capacity,
        encoded_control_block,
        true,
        0,
        None,
    )
}

fn compress_on_source_final_page_inner(
    decoded: &[u8],
    source_profile: PagedCompressionProfile,
    output_slot_capacity: usize,
    encoded_control_blocks: Option<&[u8]>,
    align_control_blocks_to_input_pages: bool,
    single_word_prefix_floor_words: usize,
    input_page_output_limit: Option<InputPageOutputLimit>,
) -> Result<Vec<u8>> {
    ensure!(
        output_slot_capacity >= source_profile.stream_byte_count,
        "paged stream output slot is smaller than its source stream"
    );
    let source_final_page = (source_profile.stream_byte_count - 1) / PAGED_STREAM_INPUT_PAGE_BYTES
        * PAGED_STREAM_INPUT_PAGE_BYTES;
    // The source maximum is already the admitted decoder contract. Searching for
    // the smallest usable maximum recompressed every image O(log n) times without
    // strengthening that contract. Start with the most capable admitted encoder;
    // later passes only add padding pressure when the terminator lands too early.
    let selected_match_limit = source_profile.maximum_match_words;
    let compress_candidate = |maximum_control_block_output_words,
                              single_word_token_prefix_words| {
        let limits = CompressionLimits {
            maximum_match_words: selected_match_limit,
            maximum_control_block_output_words,
            input_page_bytes: align_control_blocks_to_input_pages
                .then_some(PAGED_STREAM_INPUT_PAGE_BYTES),
            single_word_token_prefix_words,
            input_page_output_limit,
        };
        if let Some(encoded_control_blocks) = encoded_control_blocks {
            compress_with_seeded_control_blocks(decoded, encoded_control_blocks, limits)
        } else {
            compress_with_limits(decoded, PAGED_STREAM_LITERAL_PREFIX_WORDS, limits)
        }
    };
    let selection = SourceFinalPageSelection {
        source_final_page,
        output_slot_capacity,
        reject_control_blocks_crossing_input_pages: align_control_blocks_to_input_pages,
        single_word_prefix_floor_words,
        input_page_output_limit,
    };
    let compressed = select_source_page_candidate(
        decoded.len() / 2,
        source_profile.maximum_control_block_output_words,
        selection,
        &compress_candidate,
    )?;
    ensure!(
        compressed.len() > source_final_page,
        "rebuilt stream terminator at 0x{:x} is in an earlier input page than source page 0x{source_final_page:x}",
        compressed.len()
    );
    ensure!(
        compressed.len() <= output_slot_capacity,
        "rebuilt paged stream exceeds its output slot"
    );
    let rebuilt_profile = profile_paged_compression(&compressed)?;
    ensure!(
        rebuilt_profile.maximum_match_words <= source_profile.maximum_match_words,
        "rebuilt stream match length exceeds its source profile"
    );
    ensure!(
        rebuilt_profile.maximum_control_block_output_words
            <= source_profile.maximum_control_block_output_words,
        "rebuilt stream control-block expansion exceeds its source profile"
    );
    if align_control_blocks_to_input_pages {
        ensure!(
            rebuilt_profile.control_blocks_crossing_input_pages == 0,
            "rebuilt stream has {} control block(s) crossing 0x{PAGED_STREAM_INPUT_PAGE_BYTES:x}-byte input pages",
            rebuilt_profile.control_blocks_crossing_input_pages
        );
    }
    Ok(compressed)
}

fn select_source_page_candidate(
    decoded_word_count: usize,
    maximum_control_block_output_words: usize,
    selection: SourceFinalPageSelection,
    compress_candidate: &impl Fn(usize, usize) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let compressed = compress_candidate(maximum_control_block_output_words, 0)?;
    ensure!(
        compressed.len() <= selection.output_slot_capacity,
        "paged stream cannot fit within its output slot"
    );
    if let Some(padded) = extend_nearby_short_matches(&compressed, selection)? {
        return Ok(padded);
    }
    if compressed.len() > selection.source_final_page {
        return page_safe_candidate(
            maximum_control_block_output_words,
            0,
            compressed,
            selection,
            compress_candidate,
        )?
        .ok_or_else(|| {
            anyhow::anyhow!("paged stream terminator cannot avoid an input-page boundary")
        });
    }

    let mut lower_block_output_limit = MINIMUM_CONTROL_BLOCK_OUTPUT_WORDS;
    let mut upper_block_output_limit = maximum_control_block_output_words;
    let mut smallest_early_block_output_limit = None;
    while lower_block_output_limit <= upper_block_output_limit {
        let block_output_limit =
            lower_block_output_limit + (upper_block_output_limit - lower_block_output_limit) / 2;
        let candidate = compress_candidate(block_output_limit, 0)?;
        if let Some(padded) = extend_nearby_short_matches(&candidate, selection)? {
            return Ok(padded);
        }
        if candidate.len() <= selection.source_final_page {
            smallest_early_block_output_limit = Some(block_output_limit);
            if block_output_limit == MINIMUM_CONTROL_BLOCK_OUTPUT_WORDS {
                break;
            }
            upper_block_output_limit = block_output_limit - 1;
        } else {
            lower_block_output_limit = block_output_limit + 1;
        }
    }
    let smallest_early_block_output_limit = smallest_early_block_output_limit.ok_or_else(|| {
        anyhow::anyhow!(
            "paged stream cannot find a control-block limit ending before the source input page"
        )
    })?;

    // There are two boundary-adjacent ways to reach the source final page:
    // tighten the block limit once, or retain the less padded block limit and
    // delay its first multi-word match. Keep whichever valid stream is shorter.
    let direct_page_candidate =
        if smallest_early_block_output_limit > MINIMUM_CONTROL_BLOCK_OUTPUT_WORDS {
            let block_output_limit = smallest_early_block_output_limit - 1;
            let candidate = compress_candidate(block_output_limit, 0)?;
            page_safe_candidate(
                block_output_limit,
                0,
                candidate,
                selection,
                compress_candidate,
            )?
        } else {
            None
        };

    let mut lower_single_word_prefix =
        PAGED_STREAM_LITERAL_PREFIX_WORDS.max(selection.single_word_prefix_floor_words);
    let mut upper_single_word_prefix = decoded_word_count;
    let mut smallest_prefix_candidate = None;
    while lower_single_word_prefix <= upper_single_word_prefix {
        let single_word_prefix =
            lower_single_word_prefix + (upper_single_word_prefix - lower_single_word_prefix) / 2;
        let candidate = compress_candidate(smallest_early_block_output_limit, single_word_prefix)?;
        if let Some(padded) = extend_nearby_short_matches(&candidate, selection)? {
            return Ok(padded);
        }
        if candidate.len() <= selection.source_final_page {
            lower_single_word_prefix = single_word_prefix + 1;
        } else if candidate.len() <= selection.output_slot_capacity {
            smallest_prefix_candidate = Some((single_word_prefix, candidate));
            if single_word_prefix == PAGED_STREAM_LITERAL_PREFIX_WORDS {
                break;
            }
            upper_single_word_prefix = single_word_prefix - 1;
        } else {
            if single_word_prefix == PAGED_STREAM_LITERAL_PREFIX_WORDS {
                break;
            }
            upper_single_word_prefix = single_word_prefix - 1;
        }
    }
    let prefixed_page_candidate =
        if let Some((single_word_prefix, candidate)) = smallest_prefix_candidate {
            page_safe_candidate(
                smallest_early_block_output_limit,
                single_word_prefix,
                candidate,
                selection,
                compress_candidate,
            )?
        } else {
            None
        };

    if let Some(candidate) = [direct_page_candidate, prefixed_page_candidate]
        .into_iter()
        .flatten()
        .min_by_key(Vec::len)
    {
        return Ok(candidate);
    }

    // A narrow final page may have no encodable length at the one block limit
    // chosen above. Vary prefix pressure independently across the remaining
    // source-admitted block limits; never widen the slot or move termination.
    for block_limit in
        (smallest_early_block_output_limit..=maximum_control_block_output_words).rev()
    {
        let mut lower_prefix =
            PAGED_STREAM_LITERAL_PREFIX_WORDS.max(selection.single_word_prefix_floor_words);
        let mut upper_prefix = decoded_word_count;
        while lower_prefix <= upper_prefix {
            let prefix = lower_prefix + (upper_prefix - lower_prefix) / 2;
            let candidate = compress_candidate(block_limit, prefix)?;
            if let Some(padded) = extend_nearby_short_matches(&candidate, selection)? {
                return Ok(padded);
            }
            if candidate.len() <= selection.source_final_page {
                lower_prefix = prefix + 1;
            } else {
                if let Some(candidate) = page_safe_candidate(
                    block_limit,
                    prefix,
                    candidate,
                    selection,
                    compress_candidate,
                )? {
                    return Ok(candidate);
                }
                if prefix == 0 {
                    break;
                }
                upper_prefix = prefix - 1;
            }
        }
    }
    bail!("paged stream cannot place its terminator within the source final-page window")
}

fn extend_nearby_short_matches(
    encoded: &[u8],
    selection: SourceFinalPageSelection,
) -> Result<Option<Vec<u8>>> {
    let Some(limit) = selection.input_page_output_limit else {
        return Ok(None);
    };
    let desired = selection.source_final_page + 2;
    if encoded.len() >= desired
        || desired > selection.output_slot_capacity
        || desired - encoded.len() > 512
    {
        return Ok(None);
    }
    let needed = (desired - encoded.len()) / 2;
    let mut offsets = Vec::new();
    let mut input = 0;
    let mut decoded_words = 0;
    'stream: loop {
        let mut control = read_u16(encoded, &mut input)?;
        for _ in 0..16 {
            let offset = input;
            let token = read_u16(encoded, &mut input)?;
            if control & 0x8000 != 0 {
                let mut length = usize::from(token >> 11);
                if length == 0 {
                    length = usize::from(read_u16(encoded, &mut input)?);
                } else if decoded_words >= selection.single_word_prefix_floor_words {
                    offsets.push(offset);
                }
                if token & 0x7ff == 0 && length == 0 {
                    break 'stream;
                }
                decoded_words += length;
            } else {
                decoded_words += 1;
            }
            control <<= 1;
        }
    }
    if offsets.len() < needed {
        return Ok(None);
    }
    let mut padded = Vec::with_capacity(desired);
    let mut copied = 0;
    for &offset in &offsets[offsets.len() - needed..] {
        padded.extend_from_slice(&encoded[copied..offset]);
        let token = u16::from_le_bytes([encoded[offset], encoded[offset + 1]]);
        padded.extend_from_slice(&(token & 0x7ff).to_le_bytes());
        padded.extend_from_slice(&(token >> 11).to_le_bytes());
        copied = offset + 2;
    }
    padded.extend_from_slice(&encoded[copied..]);
    let profile = profile_paged_compression(&padded)?;
    if profile.maximum_input_page_output_words > limit.maximum_output_words
        || (selection.reject_control_blocks_crossing_input_pages
            && profile.control_blocks_crossing_input_pages != 0)
    {
        return Ok(None);
    }
    Ok(Some(padded))
}

fn page_safe_candidate(
    block_output_limit: usize,
    single_word_prefix: usize,
    candidate: Vec<u8>,
    selection: SourceFinalPageSelection,
    compress_candidate: &impl Fn(usize, usize) -> Result<Vec<u8>>,
) -> Result<Option<Vec<u8>>> {
    if candidate.len() <= selection.source_final_page
        || candidate.len() > selection.output_slot_capacity
    {
        return Ok(None);
    }
    if !selection.reject_control_blocks_crossing_input_pages
        || profile_paged_compression(&candidate)?.control_blocks_crossing_input_pages == 0
    {
        return Ok(Some(candidate));
    }

    let first_prefix = single_word_prefix
        .max(PAGED_STREAM_LITERAL_PREFIX_WORDS)
        .max(selection.single_word_prefix_floor_words)
        .saturating_add(1);
    for repaired_prefix in first_prefix..first_prefix.saturating_add(256) {
        let repaired = compress_candidate(block_output_limit, repaired_prefix)?;
        if repaired.len() > selection.output_slot_capacity {
            break;
        }
        if repaired.len() > selection.source_final_page
            && profile_paged_compression(&repaired)?.control_blocks_crossing_input_pages == 0
        {
            return Ok(Some(repaired));
        }
    }
    Ok(None)
}

pub(crate) fn profile_paged_compression(stored: &[u8]) -> Result<PagedCompressionProfile> {
    let mut input_offset = 0usize;
    let mut maximum_match_words = 0usize;
    let mut maximum_control_block_output_words = 0usize;
    let mut control_blocks_crossing_input_pages = 0usize;
    let mut maximum_input_page_output_words = 0usize;
    let mut input_page_output_words = 0usize;
    let mut input_page = 0usize;
    loop {
        if input_offset / PAGED_STREAM_INPUT_PAGE_BYTES != input_page {
            input_page = input_offset / PAGED_STREAM_INPUT_PAGE_BYTES;
            input_page_output_words = 0;
        }
        let control_block_start = input_offset;
        let mut control = read_u16(stored, &mut input_offset)?;
        let mut control_block_output_words = 0usize;
        for _ in 0..16 {
            if control & 0x8000 != 0 {
                let token = read_u16(stored, &mut input_offset)?;
                let mut length = usize::from(token >> 11);
                let distance = token & 0x07ff;
                if length == 0 {
                    length = usize::from(read_u16(stored, &mut input_offset)?);
                }
                if distance == 0 && length == 0 {
                    control_blocks_crossing_input_pages += usize::from(
                        control_block_start / PAGED_STREAM_INPUT_PAGE_BYTES
                            != (input_offset - 1) / PAGED_STREAM_INPUT_PAGE_BYTES,
                    );
                    maximum_control_block_output_words =
                        maximum_control_block_output_words.max(control_block_output_words);
                    maximum_input_page_output_words = maximum_input_page_output_words
                        .max(input_page_output_words + control_block_output_words);
                    return Ok(PagedCompressionProfile {
                        maximum_input_page_output_words,
                        maximum_match_words,
                        maximum_control_block_output_words,
                        control_blocks_crossing_input_pages,
                        stream_byte_count: input_offset,
                    });
                }
                maximum_match_words = maximum_match_words.max(length);
                control_block_output_words += length;
            } else {
                read_u16(stored, &mut input_offset)?;
                control_block_output_words += 1;
            }
            control <<= 1;
        }
        input_page_output_words += control_block_output_words;
        maximum_input_page_output_words =
            maximum_input_page_output_words.max(input_page_output_words);
        maximum_control_block_output_words =
            maximum_control_block_output_words.max(control_block_output_words);
        control_blocks_crossing_input_pages += usize::from(
            control_block_start / PAGED_STREAM_INPUT_PAGE_BYTES
                != (input_offset - 1) / PAGED_STREAM_INPUT_PAGE_BYTES,
        );
    }
}

fn read_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        bail!("truncated paged compression token at 0x{offset:x}");
    }
    let value = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(value)
}
