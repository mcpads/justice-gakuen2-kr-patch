use anyhow::{Result, bail, ensure};

const MAX_DISTANCE: usize = 0x07ff;
const MAX_EXTENDED_LENGTH: usize = 0xffff;
const TOKENS_PER_CONTROL_BLOCK: usize = 16;

const WORD_VALUE_COUNT: usize = 1_usize << u16::BITS;

#[derive(Default)]
struct WordPositionList {
    indices: Vec<usize>,
    first_in_window: usize,
}

struct WordPositionTable {
    by_word: Box<[WordPositionList]>,
}

impl Default for WordPositionTable {
    fn default() -> Self {
        Self {
            by_word: std::iter::repeat_with(WordPositionList::default)
                .take(WORD_VALUE_COUNT)
                .collect(),
        }
    }
}

impl WordPositionTable {
    fn add(&mut self, word: u16, index: usize) {
        self.by_word[usize::from(word)].indices.push(index);
    }

    fn candidates_in_window(&mut self, word: u16, minimum: usize) -> &[usize] {
        let positions = &mut self.by_word[usize::from(word)];
        while positions
            .indices
            .get(positions.first_in_window)
            .is_some_and(|candidate| *candidate < minimum)
        {
            positions.first_in_window += 1;
        }
        &positions.indices[positions.first_in_window..]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompressionLimits {
    pub maximum_match_words: usize,
    pub maximum_control_block_output_words: usize,
    pub input_page_bytes: Option<usize>,
    pub single_word_token_prefix_words: usize,
    pub input_page_output_limit: Option<InputPageOutputLimit>,
}

/// Bound the work performed by one native decoder invocation. The decoder
/// finishes a whole control block before checking the input-page boundary.
#[derive(Debug, Clone, Copy)]
pub struct InputPageOutputLimit {
    pub input_page_bytes: usize,
    pub maximum_output_words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompleteControlBlockPrefix {
    pub(crate) encoded_byte_count: usize,
    pub(crate) decoded_byte_count: usize,
}

pub fn decompress(data: &[u8], allow_trailing: bool) -> Result<Vec<u8>> {
    let mut input_offset = 0usize;
    let mut output = Vec::new();

    loop {
        let mut control = read_u16(data, &mut input_offset)?;
        for _ in 0..16 {
            if control & 0x8000 != 0 {
                let token = read_u16(data, &mut input_offset)?;
                let mut length = usize::from(token >> 11);
                let distance = usize::from(token & 0x07ff);
                if length == 0 {
                    length = usize::from(read_u16(data, &mut input_offset)?);
                }

                if distance == 0 {
                    if length == 0 {
                        ensure!(
                            allow_trailing || input_offset == data.len(),
                            "{} trailing byte(s) after terminator",
                            data.len() - input_offset
                        );
                        return Ok(output);
                    }
                    // Legacy retail data expects a zero-filled destination.
                    // New streams never use this RAM-dependent token.
                    output.resize(output.len() + length * 2, 0);
                } else {
                    let distance_bytes = distance * 2;
                    ensure!(
                        distance_bytes <= output.len(),
                        "match distance {distance} halfwords exceeds output at input offset 0x{input_offset:x}"
                    );
                    for _ in 0..length {
                        let source = output.len() - distance_bytes;
                        let pair = [output[source], output[source + 1]];
                        output.extend_from_slice(&pair);
                    }
                }
            } else {
                let literal = read_u16(data, &mut input_offset)?;
                output.extend_from_slice(&literal.to_le_bytes());
            }
            control <<= 1;
        }
    }
}

pub fn compress(data: &[u8], literal_prefix_words: usize) -> Result<Vec<u8>> {
    compress_with_limits(
        data,
        literal_prefix_words,
        CompressionLimits {
            maximum_match_words: MAX_EXTENDED_LENGTH,
            maximum_control_block_output_words: MAX_EXTENDED_LENGTH * TOKENS_PER_CONTROL_BLOCK,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
}

pub fn compress_with_maximum_match_words(
    data: &[u8],
    literal_prefix_words: usize,
    maximum_match_words: usize,
) -> Result<Vec<u8>> {
    compress_with_limits(
        data,
        literal_prefix_words,
        CompressionLimits {
            maximum_match_words,
            maximum_control_block_output_words: MAX_EXTENDED_LENGTH * TOKENS_PER_CONTROL_BLOCK,
            input_page_bytes: None,
            single_word_token_prefix_words: 0,
            input_page_output_limit: None,
        },
    )
}

pub fn compress_with_limits(
    data: &[u8],
    literal_prefix_words: usize,
    limits: CompressionLimits,
) -> Result<Vec<u8>> {
    let words = validated_words(data, literal_prefix_words, limits)?;
    compress_words(
        &words,
        literal_prefix_words,
        0,
        WordPositionTable::default(),
        Emitter::new(limits.input_page_bytes),
        limits,
    )
}

pub fn compress_with_seeded_control_block(
    data: &[u8],
    encoded_control_block: &[u8],
    limits: CompressionLimits,
) -> Result<Vec<u8>> {
    compress_with_seeded_control_blocks(data, encoded_control_block, limits)
}

pub fn compress_with_seeded_control_blocks(
    data: &[u8],
    encoded_control_blocks: &[u8],
    limits: CompressionLimits,
) -> Result<Vec<u8>> {
    let decoded = decode_complete_control_blocks(encoded_control_blocks, limits.input_page_bytes)?;
    let decoded_prefix = decoded.bytes;
    let decoded_prefix_words = decoded_prefix.len() / 2;
    let words = validated_words(data, decoded_prefix_words, limits)?;
    ensure!(
        data.starts_with(&decoded_prefix),
        "seeded compression control blocks do not decode to the input prefix"
    );
    let mut positions = WordPositionTable::default();
    for (index, &word) in words.iter().enumerate().take(decoded_prefix_words) {
        positions.add(word, index);
    }
    compress_words(
        &words,
        decoded_prefix_words,
        decoded_prefix_words,
        positions,
        Emitter::with_seeded_control_blocks(
            limits.input_page_bytes,
            encoded_control_blocks.to_vec(),
            decoded.control_blocks_in_input_page,
            decoded.extended_tokens_in_input_page,
        )?,
        limits,
    )
}

pub(crate) fn complete_control_block_prefix(
    encoded: &[u8],
    maximum_decoded_byte_count: usize,
) -> Result<CompleteControlBlockPrefix> {
    ensure!(
        maximum_decoded_byte_count.is_multiple_of(2),
        "compression prefix extent must be even"
    );
    let mut input_offset = 0usize;
    let mut output = Vec::new();
    let mut prefix = CompleteControlBlockPrefix {
        encoded_byte_count: 0,
        decoded_byte_count: 0,
    };
    while input_offset < encoded.len() {
        let output_byte_count = output.len();
        let block = decode_control_block(encoded, &mut input_offset, &mut output)?;
        if block.contains_terminator || output.len() > maximum_decoded_byte_count {
            output.truncate(output_byte_count);
            break;
        }
        prefix = CompleteControlBlockPrefix {
            encoded_byte_count: input_offset,
            decoded_byte_count: output.len(),
        };
    }
    Ok(prefix)
}

fn validated_words(
    data: &[u8],
    literal_prefix_words: usize,
    limits: CompressionLimits,
) -> Result<Vec<u16>> {
    ensure!(data.len().is_multiple_of(2), "input size must be even");
    let words: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    ensure!(
        literal_prefix_words <= words.len(),
        "literal prefix exceeds input word count"
    );
    ensure!(
        (2..=MAX_EXTENDED_LENGTH).contains(&limits.maximum_match_words),
        "maximum match length must be between 2 and {MAX_EXTENDED_LENGTH} halfwords"
    );
    ensure!(
        limits.maximum_control_block_output_words >= TOKENS_PER_CONTROL_BLOCK,
        "maximum control-block output must accommodate {TOKENS_PER_CONTROL_BLOCK} literal halfwords"
    );
    if let Some(input_page_bytes) = limits.input_page_bytes {
        ensure!(
            input_page_bytes >= 2 + TOKENS_PER_CONTROL_BLOCK * 2
                && input_page_bytes.is_multiple_of(2),
            "input page must fit one control block and have an even byte count"
        );
    }
    Ok(words)
}

fn compress_words(
    words: &[u16],
    literal_prefix_words: usize,
    mut index: usize,
    mut positions: WordPositionTable,
    mut emitter: Emitter,
    limits: CompressionLimits,
) -> Result<Vec<u8>> {
    let mut page_budget = limits
        .input_page_output_limit
        .map(|limit| InputPageBudget::from_prefix(limit, &emitter.output))
        .transpose()?;
    while index < words.len() {
        if let Some(budget) = &mut page_budget {
            budget.start_token(&emitter);
        }
        if index < literal_prefix_words {
            emitter.emit(&[words[index]], false, 1)?;
            if let Some(budget) = &mut page_budget {
                budget.output_words += 1;
            }
            positions.add(words[index], index);
            index += 1;
            continue;
        }

        let remaining_tokens = TOKENS_PER_CONTROL_BLOCK - emitter.token_count() - 1;
        let maximum_current_output = limits
            .maximum_control_block_output_words
            .checked_sub(emitter.decoded_word_count() + remaining_tokens)
            .unwrap_or(1);
        let maximum_match_words = limits
            .maximum_match_words
            .min(maximum_current_output)
            .min(if emitter.can_emit_extended_token()? {
                MAX_EXTENDED_LENGTH
            } else {
                31
            })
            .min(page_budget.as_ref().map_or(MAX_EXTENDED_LENGTH, |budget| {
                budget.match_allowance(&emitter)
            }))
            .min(if index < limits.single_word_token_prefix_words {
                1
            } else {
                MAX_EXTENDED_LENGTH
            });
        let (length, distance) = if maximum_match_words >= 1 {
            find_longest_match(words, &mut positions, index, maximum_match_words)
        } else {
            (0, 0)
        };
        let consumed = if length == 1 && emitter.must_emit_extended_token()? {
            emitter.emit(&[distance as u16, 1], true, 1)?;
            1
        } else if length >= 2 {
            if length <= 31 {
                if emitter.must_emit_extended_token()? {
                    emitter.emit(&[distance as u16, length as u16], true, length)?;
                } else {
                    emitter.emit(&[((length as u16) << 11) | distance as u16], true, length)?;
                }
            } else {
                emitter.emit(&[distance as u16, length as u16], true, length)?;
            }
            length
        } else {
            emitter.emit(&[words[index]], false, 1)?;
            1
        };

        if let Some(budget) = &mut page_budget {
            budget.output_words += consumed;
        }
        for (consumed_index, &word) in words.iter().enumerate().skip(index).take(consumed) {
            positions.add(word, consumed_index);
        }
        index += consumed;
    }

    emitter.emit(&[0, 0], true, 0)?;
    emitter.finish()
}

struct InputPageBudget {
    limit: InputPageOutputLimit,
    page: usize,
    output_words: usize,
}

impl InputPageBudget {
    fn from_prefix(limit: InputPageOutputLimit, encoded: &[u8]) -> Result<Self> {
        ensure!(
            limit.input_page_bytes >= 34 && limit.input_page_bytes.is_multiple_of(2),
            "decoder input page must fit a control block"
        );
        ensure!(
            limit.maximum_output_words >= limit.input_page_bytes / 2 + 16,
            "decoder page budget must accommodate literal tokens and block lookahead"
        );
        let mut budget = Self {
            limit,
            page: 0,
            output_words: 0,
        };
        let mut cursor = 0;
        let mut decoded = Vec::new();
        while cursor < encoded.len() {
            let page = cursor / limit.input_page_bytes;
            if page != budget.page {
                budget.page = page;
                budget.output_words = 0;
            }
            let before = decoded.len();
            decode_control_block(encoded, &mut cursor, &mut decoded)?;
            budget.output_words += (decoded.len() - before) / 2;
        }
        Ok(budget)
    }

    fn start_token(&mut self, emitter: &Emitter) {
        if emitter.control_offset.is_none() {
            let page = emitter.output.len() / self.limit.input_page_bytes;
            if page != self.page {
                self.page = page;
                self.output_words = 0;
            }
        }
    }

    fn match_allowance(&self, emitter: &Emitter) -> usize {
        // Reserve enough single-word tokens to reach the page boundary and
        // finish its final control block. Control words only reduce this need.
        let page_end = (self.page + 1) * self.limit.input_page_bytes;
        let remaining_literal_words =
            page_end.saturating_sub(emitter.output.len()).div_ceil(2) + 15;
        self.limit
            .maximum_output_words
            .saturating_sub(self.output_words + remaining_literal_words)
            .max(1)
    }
}

struct DecodedControlBlockPrefix {
    bytes: Vec<u8>,
    control_blocks_in_input_page: usize,
    extended_tokens_in_input_page: usize,
}

struct DecodedControlBlock {
    contains_terminator: bool,
    extended_token_count: usize,
}

fn decode_complete_control_blocks(
    encoded: &[u8],
    input_page_bytes: Option<usize>,
) -> Result<DecodedControlBlockPrefix> {
    ensure!(
        !encoded.is_empty(),
        "seeded compression prefix must contain a complete control block"
    );
    let mut input_offset = 0usize;
    let mut output = Vec::new();
    let mut control_blocks_in_input_page = 0usize;
    let mut extended_tokens_in_input_page = 0usize;
    while input_offset < encoded.len() {
        let control_block_start = input_offset;
        let block = decode_control_block(encoded, &mut input_offset, &mut output)?;
        ensure!(
            !block.contains_terminator,
            "seeded compression prefix contains a terminator"
        );
        if let Some(page_bytes) = input_page_bytes {
            ensure!(
                control_block_start / page_bytes == (input_offset - 1) / page_bytes,
                "seeded compression control block crosses an input-page boundary"
            );
            control_blocks_in_input_page += 1;
            extended_tokens_in_input_page += block.extended_token_count;
            if input_offset.is_multiple_of(page_bytes) {
                control_blocks_in_input_page = 0;
                extended_tokens_in_input_page = 0;
            }
        }
    }
    Ok(DecodedControlBlockPrefix {
        bytes: output,
        control_blocks_in_input_page,
        extended_tokens_in_input_page,
    })
}

fn decode_control_block(
    encoded: &[u8],
    input_offset: &mut usize,
    output: &mut Vec<u8>,
) -> Result<DecodedControlBlock> {
    let mut control = read_u16(encoded, input_offset)?;
    let mut extended_token_count = 0usize;
    for _ in 0..TOKENS_PER_CONTROL_BLOCK {
        if control & 0x8000 == 0 {
            let word = read_u16(encoded, input_offset)?;
            output.extend_from_slice(&word.to_le_bytes());
        } else {
            let token = read_u16(encoded, input_offset)?;
            let distance = usize::from(token & 0x07ff);
            let mut length = usize::from(token >> 11);
            if length == 0 {
                length = usize::from(read_u16(encoded, input_offset)?);
                extended_token_count += 1;
            }
            if distance == 0 && length == 0 {
                return Ok(DecodedControlBlock {
                    contains_terminator: true,
                    extended_token_count,
                });
            }
            if distance == 0 {
                output.resize(output.len() + length * 2, 0);
            } else {
                let distance_bytes = distance * 2;
                ensure!(
                    distance_bytes <= output.len(),
                    "seeded compression match exceeds its decoded prefix"
                );
                for _ in 0..length {
                    let source = output.len() - distance_bytes;
                    let pair = [output[source], output[source + 1]];
                    output.extend_from_slice(&pair);
                }
            }
        }
        control <<= 1;
    }
    Ok(DecodedControlBlock {
        contains_terminator: false,
        extended_token_count,
    })
}

fn read_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        bail!("truncated halfword at input offset 0x{:x}", *offset);
    }
    let value = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(value)
}

fn find_longest_match(
    words: &[u16],
    positions: &mut WordPositionTable,
    index: usize,
    maximum_match_words: usize,
) -> (usize, usize) {
    let minimum = index.saturating_sub(MAX_DISTANCE);
    let candidates = positions.candidates_in_window(words[index], minimum);

    let mut best_length = 0usize;
    let mut best_distance = 0usize;
    let maximum = maximum_match_words.min(words.len() - index);
    for &candidate in candidates.iter().rev() {
        let distance = index - candidate;
        if best_length != 0 {
            let next_offset = best_length;
            if words[index + next_offset] != words[index + next_offset - distance] {
                continue;
            }
            let middle_offset = best_length / 2;
            if words[index + middle_offset] != words[index + middle_offset - distance] {
                continue;
            }
        }
        let mut length = 1usize;
        while length < maximum && words[index + length] == words[index + length - distance] {
            length += 1;
        }
        if length > best_length {
            best_length = length;
            best_distance = distance;
            if length == maximum {
                break;
            }
        }
    }
    (best_length, best_distance)
}

struct Emitter {
    output: Vec<u8>,
    control_offset: Option<usize>,
    control: u16,
    token_count: usize,
    decoded_word_count: usize,
    control_has_terminator: bool,
    input_page_bytes: Option<usize>,
    control_blocks_in_input_page: usize,
    extended_tokens_in_input_page: usize,
}

impl Emitter {
    fn new(input_page_bytes: Option<usize>) -> Self {
        Self {
            output: Vec::new(),
            control_offset: None,
            control: 0,
            token_count: 0,
            decoded_word_count: 0,
            control_has_terminator: false,
            input_page_bytes,
            control_blocks_in_input_page: 0,
            extended_tokens_in_input_page: 0,
        }
    }

    fn with_seeded_control_blocks(
        input_page_bytes: Option<usize>,
        output: Vec<u8>,
        control_blocks_in_input_page: usize,
        extended_tokens_in_input_page: usize,
    ) -> Result<Self> {
        let emitter = Self {
            output,
            control_offset: None,
            control: 0,
            token_count: 0,
            decoded_word_count: 0,
            control_has_terminator: false,
            input_page_bytes,
            control_blocks_in_input_page,
            extended_tokens_in_input_page,
        };
        if let Some((blocks_per_page, required_extended_tokens)) = emitter.input_page_schedule()? {
            ensure!(
                emitter.control_blocks_in_input_page < blocks_per_page,
                "seeded compression prefix exhausts an incomplete input page"
            );
            ensure!(
                emitter.extended_tokens_in_input_page <= required_extended_tokens,
                "seeded compression prefix exceeds its input-page extended-token schedule"
            );
        }
        Ok(emitter)
    }

    fn emit(&mut self, encoded_words: &[u16], is_match: bool, decoded_words: usize) -> Result<()> {
        if self.control_offset.is_none() {
            self.validate_control_block_start()?;
            self.control_offset = Some(self.output.len());
            self.output.extend_from_slice(&[0, 0]);
            self.control = 0;
            self.token_count = 0;
            self.decoded_word_count = 0;
            self.control_has_terminator = false;
        }
        self.control_has_terminator |= is_match && decoded_words == 0;
        if is_match {
            self.control |= 1 << (15 - self.token_count);
        }
        for word in encoded_words {
            self.output.extend_from_slice(&word.to_le_bytes());
        }
        self.extended_tokens_in_input_page +=
            usize::from(encoded_words.len() == 2 && decoded_words != 0);
        self.token_count += 1;
        self.decoded_word_count += decoded_words;
        if self.token_count == 16 {
            self.flush_control()?;
        }
        Ok(())
    }

    fn token_count(&self) -> usize {
        if self.control_offset.is_some() {
            self.token_count
        } else {
            0
        }
    }

    fn decoded_word_count(&self) -> usize {
        if self.control_offset.is_some() {
            self.decoded_word_count
        } else {
            0
        }
    }

    fn can_emit_extended_token(&self) -> Result<bool> {
        let Some((_, required_extended_tokens)) = self.input_page_schedule()? else {
            return Ok(true);
        };
        Ok(self.extended_tokens_in_input_page < required_extended_tokens)
    }

    fn must_emit_extended_token(&self) -> Result<bool> {
        let Some((_, required_extended_tokens)) = self.input_page_schedule()? else {
            return Ok(false);
        };
        Ok(self.extended_tokens_in_input_page < required_extended_tokens)
    }

    fn input_page_schedule(&self) -> Result<Option<(usize, usize)>> {
        let Some(page_bytes) = self.input_page_bytes else {
            return Ok(None);
        };
        let minimum_block_bytes = 2 + TOKENS_PER_CONTROL_BLOCK * 2;
        let blocks_per_page = page_bytes / minimum_block_bytes;
        let remaining_bytes = page_bytes - blocks_per_page * minimum_block_bytes;
        ensure!(
            blocks_per_page != 0 && remaining_bytes.is_multiple_of(2),
            "input page cannot be tiled by control blocks"
        );
        let required_extended_tokens = remaining_bytes / 2;
        ensure!(
            required_extended_tokens <= blocks_per_page * TOKENS_PER_CONTROL_BLOCK,
            "input page requires more extended tokens than its control blocks can hold"
        );
        Ok(Some((blocks_per_page, required_extended_tokens)))
    }

    fn validate_control_block_start(&self) -> Result<()> {
        let Some((blocks_per_page, _)) = self.input_page_schedule()? else {
            return Ok(());
        };
        ensure!(
            self.control_blocks_in_input_page < blocks_per_page,
            "input-page control-block schedule overflowed"
        );
        Ok(())
    }

    fn flush_control(&mut self) -> Result<()> {
        if let Some(offset) = self.control_offset.take() {
            if self.token_count == TOKENS_PER_CONTROL_BLOCK
                && !self.control_has_terminator
                && let Some(page_bytes) = self.input_page_bytes
            {
                ensure!(
                    offset / page_bytes == (self.output.len() - 1) / page_bytes,
                    "control block 0x{offset:x}..0x{:x} crosses an input-page boundary with {} block(s) and {} extended token(s) scheduled in the page",
                    self.output.len(),
                    self.control_blocks_in_input_page,
                    self.extended_tokens_in_input_page,
                );
            }
            self.output[offset..offset + 2].copy_from_slice(&self.control.to_le_bytes());
            if self.token_count == TOKENS_PER_CONTROL_BLOCK && !self.control_has_terminator {
                self.control_blocks_in_input_page += 1;
                if let Some((blocks_per_page, required_extended_tokens)) =
                    self.input_page_schedule()?
                    && self.control_blocks_in_input_page == blocks_per_page
                {
                    ensure!(
                        self.extended_tokens_in_input_page == required_extended_tokens,
                        "input page has {} extended token(s), expected {required_extended_tokens}",
                        self.extended_tokens_in_input_page
                    );
                    ensure!(
                        self.output
                            .len()
                            .is_multiple_of(self.input_page_bytes.unwrap()),
                        "input-page schedule did not end on its boundary"
                    );
                    self.control_blocks_in_input_page = 0;
                    self.extended_tokens_in_input_page = 0;
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>> {
        self.flush_control()?;
        Ok(self.output)
    }
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
