use std::ops::Range;

pub(crate) mod bounded_jump_tables;
pub(crate) mod control_flow;
pub(crate) mod memory_access;
pub(crate) mod reachable_control_flow;
pub(crate) mod value_flow;
#[cfg(test)]
mod value_flow_tests;

/// The byte ranges that may be decoded or admitted as executable instructions.
///
/// The complete loaded image remains a separate input to the analysis so data
/// tables may live outside these ranges without being mistaken for code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutableDomain {
    image_len: usize,
    instruction_ranges: Vec<Range<usize>>,
}

impl ExecutableDomain {
    pub(crate) fn full_image(image_len: usize) -> Self {
        let instruction_end = image_len - image_len % 4;
        let instruction_ranges = (instruction_end > 0)
            .then_some(0..instruction_end)
            .into_iter();
        Self::from_instruction_ranges(image_len, instruction_ranges)
            .expect("full-image executable range is aligned and bounded")
    }

    pub(crate) fn from_instruction_ranges(
        image_len: usize,
        instruction_ranges: impl IntoIterator<Item = Range<usize>>,
    ) -> Option<Self> {
        let mut instruction_ranges = instruction_ranges.into_iter().collect::<Vec<_>>();
        instruction_ranges.sort_by_key(|range| (range.start, range.end));
        if instruction_ranges.iter().any(|range| {
            range.start >= range.end
                || !range.start.is_multiple_of(4)
                || !range.end.is_multiple_of(4)
                || range.end > image_len
        }) || instruction_ranges
            .windows(2)
            .any(|ranges| ranges[0].end > ranges[1].start)
        {
            return None;
        }
        Some(Self {
            image_len,
            instruction_ranges,
        })
    }

    pub(crate) fn from_instruction_offsets(
        image_len: usize,
        instruction_offsets: impl IntoIterator<Item = usize>,
    ) -> Option<Self> {
        let mut instruction_offsets = instruction_offsets.into_iter().collect::<Vec<_>>();
        instruction_offsets.sort_unstable();
        instruction_offsets.dedup();
        if instruction_offsets
            .iter()
            .any(|offset| !offset.is_multiple_of(4) || offset.saturating_add(4) > image_len)
        {
            return None;
        }
        let mut ranges = Vec::<Range<usize>>::new();
        for offset in instruction_offsets {
            if let Some(last) = ranges.last_mut()
                && last.end == offset
            {
                last.end += 4;
            } else {
                ranges.push(offset..offset + 4);
            }
        }
        Self::from_instruction_ranges(image_len, ranges)
    }

    pub(crate) fn instruction_ranges(&self) -> &[Range<usize>] {
        &self.instruction_ranges
    }

    pub(crate) fn contains_instruction_offset(&self, offset: usize) -> bool {
        offset.is_multiple_of(4)
            && offset.checked_add(4).is_some_and(|end| {
                self.instruction_ranges
                    .iter()
                    .any(|range| range.start <= offset && end <= range.end)
            })
    }

    pub(crate) fn instruction_offset(&self, instruction_base: u32, pc: u32) -> Option<usize> {
        self.image_instruction_offset(instruction_base, pc)
            .filter(|&offset| self.contains_instruction_offset(offset))
    }

    pub(crate) fn image_instruction_offset(&self, instruction_base: u32, pc: u32) -> Option<usize> {
        pc.checked_sub(instruction_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| offset.is_multiple_of(4) && offset + 4 <= self.image_len)
    }

    pub(crate) fn matches_image_len(&self, image_len: usize) -> bool {
        self.image_len == image_len
    }
}
