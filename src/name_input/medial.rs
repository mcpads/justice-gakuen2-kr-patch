// Unicode medial indices. Both the editor model and the installed lookup use
// these transitions; an explicit vowel otherwise replaces the current medial.
pub(super) const COMBINATIONS: [(u16, u16, u16); 9] = [
    (8, 0, 9),
    (8, 1, 10),
    (8, 20, 11),
    (13, 4, 14),
    (13, 5, 15),
    (13, 20, 16),
    (18, 20, 19),
    (9, 20, 10),
    (14, 20, 15),
];

pub(super) fn select(previous: char, selected: char) -> char {
    let keys = super::keyboard::HANGUL_VOWEL_KEYS;
    COMBINATIONS
        .iter()
        .find_map(|&(old, key, next)| {
            (keys[old as usize] == previous && keys[key as usize] == selected)
                .then_some(keys[next as usize])
        })
        .unwrap_or(selected)
}

// Canonical reverse stages also apply to a directly selected compound vowel.
pub(super) const BACKSPACE: [(u16, u16); 7] = [
    (9, 8),
    (10, 9),
    (11, 8),
    (14, 13),
    (15, 14),
    (16, 13),
    (19, 18),
];
pub(super) fn backspace(medial: char) -> Option<char> {
    let keys = super::keyboard::HANGUL_VOWEL_KEYS;
    BACKSPACE
        .iter()
        .find_map(|&(old, next)| (keys[old as usize] == medial).then_some(keys[next as usize]))
}
