use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;
const ORIGIN: u32 = 0x800d8300;
const HANGUL: u32 = 0x800d9000;
const ASCII: u32 = 0x800d9400;
const LEGACY: u32 = 0x800d9800;
const SCRATCH: u32 = 0x800deb00;

fn expected(word: u32) -> Option<(u32, u32, u32)> {
    if word & 0xc000 == 0x8000 {
        Some((HANGUL, word & 0x3fff, 0))
    } else if word & 0xff80 == 0x4000 {
        Some((ASCII, word & 0x7f, 0))
    } else if word < 176 {
        Some((LEGACY, if word == 0xa8 { 0x54 } else { word }, 1))
    } else {
        None
    }
}

#[test]
fn every_stored_word_routes_with_bounded_indexes_or_is_rejected_without_supplier_access() {
    let bytes = build_battle_glyph_dispatch(ORIGIN, 512, HANGUL, ASCII, LEGACY).unwrap();
    let mut memory = vec![0x55; 0x200000];
    let before = memory.clone();
    for word in 0..=u16::MAX {
        run(
            &bytes,
            u32::from(word),
            8,
            expected(u32::from(word)),
            &mut memory,
        );
    }
    let mut expected = before;
    expected[0x1fdfe8..0x1fe000].copy_from_slice(&memory[0x1fdfe8..0x1fe000]);
    assert_eq!(memory, expected, "word dispatch wrote outside its stack");
}

#[test]
fn only_temporary_record_can_resolve_four_cache_aliases_and_resolution_is_not_recursive() {
    let bytes = build_battle_glyph_dispatch(ORIGIN, 512, HANGUL, ASCII, LEGACY).unwrap();
    let mut memory = vec![0x55; 0x200000];
    for (i, word) in [0x8118u16, 0x4041, 0xa8, 0x339].into_iter().enumerate() {
        memory[0x1f1896 + i * 2..0x1f1898 + i * 2].copy_from_slice(&word.to_le_bytes());
        for record in 0..=255 {
            let target = if record == 16 {
                expected(u32::from(word))
            } else {
                None
            };
            run(&bytes, 0x339 + i as u32, record, target, &mut memory);
        }
    }
    for record in [17, 255, u32::MAX] {
        run(&bytes, 0x8000, record, None, &mut memory);
    }
}

fn run(bytes: &[u8], word: u32, record: u32, expected: Option<(u32, u32, u32)>, memory: &mut [u8]) {
    let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
    r[0] = 0;
    r[4] = word;
    r[5] = record;
    r[29] = 0x801fe000;
    r[31] = 0x80027f24;
    let saved = r;
    let mut calls = 0;
    execute_with_callbacks(bytes, ORIGIN, &mut r, memory, None, &mut |pc, r, m| {
        if ![HANGUL, ASCII, LEGACY].contains(&pc) {
            return false;
        }
        calls += 1;
        let (target, arg, _) = expected.expect("rejected word reached a supplier");
        assert_eq!((pc, r[4]), (target, arg), "word {word:x}, record {record}");
        for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
            r[reg] = 0xdead0000 + reg as u32;
        }
        let sp = (r[29] & 0x1fffffff) as usize;
        m[sp..sp + 16].fill(0xcc);
        // Membership and ASCII character availability are the materializers'
        // responsibility. Also exercise their zero-return path through this ABI.
        r[2] = if (target == HANGUL && arg >= 11172) || (target == ASCII && arg < 32) {
            0
        } else {
            SCRATCH
        };
        true
    });
    assert_eq!(calls, usize::from(expected.is_some()));
    let result = expected.map_or(0, |(target, arg, _)| {
        if (target == HANGUL && arg >= 11172) || (target == ASCII && arg < 32) {
            0
        } else {
            SCRATCH
        }
    });
    assert_eq!(r[2], result);
    assert_eq!(r[3], expected.map_or(0, |v| v.2));
    for reg in (16..24).chain([28, 29, 30, 31]) {
        assert_eq!(r[reg], saved[reg]);
    }
}
