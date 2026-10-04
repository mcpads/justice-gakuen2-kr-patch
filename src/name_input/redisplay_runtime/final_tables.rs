pub(super) const INVALID_FINAL_INDEX: u8 = u8::MAX;
pub(super) const COMPOUND_FINAL_ENTRY_COUNT: usize = 11;

const SIMPLE_FINAL_BY_INITIAL: [u8; 19] = [
    1,
    2,
    4,
    7,
    INVALID_FINAL_INDEX,
    8,
    16,
    17,
    INVALID_FINAL_INDEX,
    19,
    20,
    21,
    22,
    INVALID_FINAL_INDEX,
    23,
    24,
    25,
    26,
    27,
];

// Each entry packs first-final[4:0], second-initial[9:5], and result-final[14:10].
// The trailing zero is a fail-closed scan sentinel.
const COMPOUND_FINALS: [(u8, u8, u8); COMPOUND_FINAL_ENTRY_COUNT] = [
    (1, 9, 3),
    (4, 12, 5),
    (4, 18, 6),
    (8, 0, 9),
    (8, 6, 10),
    (8, 7, 11),
    (8, 9, 12),
    (8, 16, 13),
    (8, 17, 14),
    (8, 18, 15),
    (17, 9, 18),
];

pub(super) fn simple_final_table() -> [u8; 19] {
    SIMPLE_FINAL_BY_INITIAL
}

pub(super) fn compound_final_table() -> [u8; (COMPOUND_FINAL_ENTRY_COUNT + 1) * 2] {
    let mut bytes = [0_u8; (COMPOUND_FINAL_ENTRY_COUNT + 1) * 2];
    for (index, (first_final, second_initial, result_final)) in
        COMPOUND_FINALS.into_iter().enumerate()
    {
        let entry = u16::from(first_final)
            | (u16::from(second_initial) << 5)
            | (u16::from(result_final) << 10);
        bytes[index * 2..index * 2 + 2].copy_from_slice(&entry.to_le_bytes());
    }
    bytes
}
