use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

fn layout() -> RosterGlyphLayout {
    RosterGlyphLayout {
        origin: 0x800af000,
        capacity: 512,
        names: 0x801f5814,
        hangul: 0x800b6000,
        ascii: 0x800b6400,
        uploader: 0x800b6800,
        sprite: 0x800b6a00,
    }
}

#[test]
fn roster_glyph_keeps_all_record_cells_distinct_and_never_rewrites_names() {
    let l = layout();
    let code = build_roster_glyph(&l).unwrap();
    let off = |v: u32| (v & 0x1fffffff) as usize;
    let words: Vec<u16> = (0..68)
        .map(|i| if i % 2 == 0 { 0x8000 + i } else { 0x4020 + i })
        .collect();
    for slot in 0..68 {
        for (materialized, uploaded) in [(true, true), (false, true), (true, false)] {
            let mut memory = vec![0x55; 0x200000];
            for (i, word) in words.iter().enumerate() {
                let address = off(l.names) + i / 4 * 40 + i % 4 * 2;
                memory[address..address + 2].copy_from_slice(&word.to_le_bytes());
            }
            let before = memory.clone();
            let mut r = std::array::from_fn(|i| i as u32 * 73);
            r[0] = 0;
            r[4] = words[slot].into();
            r[5] = 0x801d0000;
            r[6] = 0x01200180;
            r[7] = 0x801e1000;
            r[29] = 0x801fe000;
            r[31] = 0x800a2200;
            let saved = r;
            let renderer = if slot % 2 == 0 { l.hangul } else { l.ascii };
            let mut calls = Vec::new();
            execute_with_callbacks(
                &code,
                l.origin,
                &mut r,
                &mut memory,
                None,
                &mut |pc, r, m| {
                    if ![renderer, l.uploader, l.sprite].contains(&pc) {
                        return false;
                    }
                    calls.push(pc);
                    let result = if pc == renderer {
                        assert_eq!(r[4], u32::from(words[slot] & 0x3fff));
                        u32::from(materialized)
                    } else if pc == l.uploader {
                        assert_eq!(r[4], slot as u32);
                        if uploaded { 0 } else { u32::MAX }
                    } else {
                        assert_eq!(&r[4..8], &[slot as u32, saved[5], saved[6], saved[7]]);
                        saved[5]
                    };
                    for reg in (2..16).chain([24, 25]) {
                        r[reg] = 0xdead0000 + reg as u32;
                    }
                    let sp = off(r[29]);
                    m[sp..sp + 16].fill(0xa5);
                    r[2] = result;
                    true
                },
            );
            let expected = if !materialized {
                vec![renderer]
            } else if !uploaded {
                vec![renderer, l.uploader]
            } else {
                vec![renderer, l.uploader, l.sprite]
            };
            assert_eq!(calls, expected);
            assert_eq!(
                r[2],
                if materialized && uploaded {
                    saved[5]
                } else {
                    0
                }
            );
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], saved[reg]);
            }
            let stack = off(saved[29]);
            assert_eq!(&memory[..stack - 40], &before[..stack - 40]);
            assert_eq!(&memory[stack..], &before[stack..]);
        }
    }
}

#[test]
fn roster_glyph_shares_duplicate_words_and_rejects_unbound_or_unsupported_words() {
    let l = layout();
    let code = build_roster_glyph(&l).unwrap();
    for word in [
        0u16, 0x100, 0x3001, 0x061e, 0xc000, 0xffff, 0x4080, 0x8000, 0xbfff, 0x4020,
    ] {
        let mut memory = vec![0; 0x200000];
        for i in [0, 67] {
            let p = (l.names & 0x1fffffff) as usize + i / 4 * 40 + i % 4 * 2;
            memory[p..p + 2].copy_from_slice(&0x8000u16.to_le_bytes());
        }
        let mut r = [0; 32];
        r[4] = word.into();
        r[5] = 0x801d0000;
        r[29] = 0x801fe000;
        r[31] = 0x800a2200;
        let mut calls = Vec::new();
        execute_with_callbacks(
            &code,
            l.origin,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, _| {
                if ![l.hangul, l.ascii, l.uploader, l.sprite].contains(&pc) {
                    return false;
                }
                calls.push(pc);
                assert_eq!(word, 0x8000);
                assert_eq!(r[4], 0);
                r[2] = if pc == l.hangul {
                    1
                } else if pc == l.sprite {
                    r[5]
                } else {
                    0
                };
                true
            },
        );
        assert_eq!(
            calls,
            if word == 0x8000 {
                vec![l.hangul, l.uploader, l.sprite]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn roster_glyph_rejects_overlapping_or_unaligned_placements() {
    for bad in [
        RosterGlyphLayout {
            names: 0x800af004,
            ..layout()
        },
        RosterGlyphLayout {
            names: 0x801f5815,
            ..layout()
        },
        RosterGlyphLayout {
            sprite: 0x800af004,
            ..layout()
        },
        RosterGlyphLayout {
            hangul: 0x801f5814,
            ..layout()
        },
        RosterGlyphLayout {
            capacity: 16,
            ..layout()
        },
    ] {
        assert!(build_roster_glyph(&bad).is_err());
    }
}
