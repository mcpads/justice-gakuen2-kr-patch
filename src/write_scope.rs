pub(crate) fn difference_ranges(before: &[u8], after: &[u8]) -> Vec<[usize; 2]> {
    assert_eq!(before.len(), after.len());
    let mut ranges = Vec::new();
    let mut start = None;
    for (offset, (left, right)) in before.iter().zip(after).enumerate() {
        if left != right {
            start.get_or_insert(offset);
        } else if let Some(start) = start.take() {
            ranges.push([start, offset]);
        }
    }
    if let Some(start) = start {
        ranges.push([start, before.len()]);
    }
    ranges
}

pub(crate) fn changed_ranges_are_within(changed: &[[usize; 2]], allowed: &[[usize; 2]]) -> bool {
    if changed.iter().any(|[start, end]| start > end)
        || allowed.iter().any(|[start, end]| start > end)
    {
        return false;
    }

    let allowed = allowed
        .iter()
        .copied()
        .filter(|[start, end]| start < end)
        .collect::<Vec<_>>();
    let merged = merge_byte_ranges(allowed);

    changed.iter().all(|[start, end]| {
        if start == end {
            return true;
        }
        let insertion = merged.partition_point(|[allowed_start, _]| allowed_start <= start);
        insertion != 0 && *end <= merged[insertion - 1][1]
    })
}

pub(crate) fn merge_byte_ranges(mut ranges: Vec<[usize; 2]>) -> Vec<[usize; 2]> {
    ranges.retain(|[start, end]| start < end);
    ranges.sort_unstable_by_key(|[start, end]| (*start, *end));
    let mut merged: Vec<[usize; 2]> = Vec::with_capacity(ranges.len());
    for [start, end] in ranges {
        if let Some(previous) = merged.last_mut()
            && start <= previous[1]
        {
            previous[1] = previous[1].max(end);
        } else {
            merged.push([start, end]);
        }
    }
    merged
}

pub(crate) fn byte_ranges_overlap(left: [usize; 2], right: [usize; 2]) -> bool {
    left[0] < right[1] && right[0] < left[1]
}
