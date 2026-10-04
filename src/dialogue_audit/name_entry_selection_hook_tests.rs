use super::name_entry_selection_hook::{HOOK_START_OFFSET, install_name_entry_selection_hook};
use crate::name_input::{NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN, editing_runtime_tests::Editor};
#[path = "name_entry_selection_source_fixture.rs"]
mod source_fixture;
fn source() -> Vec<u8> {
    let mut bytes = vec![0x5a; 0x9000];
    for (i, word) in source_fixture::WORDS.iter().enumerate() {
        bytes[HOOK_START_OFFSET + i * 4..HOOK_START_OFFSET + i * 4 + 4]
            .copy_from_slice(&word.to_le_bytes());
    }
    bytes
}
#[test]
fn selection_bridge_is_source_bound_and_only_replaces_its_native_function() {
    let source = source();
    let mut patched = source.clone();
    let report = install_name_entry_selection_hook(
        &mut patched,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + 0x200,
    )
    .unwrap();
    assert_eq!(report.overwritten_byte_count, 0x110);
    assert_eq!(&source[..HOOK_START_OFFSET], &patched[..HOOK_START_OFFSET]);
    assert_eq!(
        &source[HOOK_START_OFFSET + 0x110..],
        &patched[HOOK_START_OFFSET + 0x110..]
    );
    patched = source.clone();
    patched[HOOK_START_OFFSET + 4] ^= 1;
    assert!(
        install_name_entry_selection_hook(
            &mut patched,
            NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + 0x200
        )
        .is_err()
    );
    assert!(install_name_entry_selection_hook(&mut source.clone(), 0x80000000).is_err());
}
#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn reverse_medial_helper_keeps_the_record_pointer_and_reverses_each_compound() {
    let mut editor = Editor::new();
    const OBJECT: usize = 0x1c0000;
    for field in 0..3 {
        for slot in [0, if field == 2 { 3 } else { 5 }] {
            for (initial, medial, expected) in [
                ('ㄱ', 'ㅘ', '고'),
                ('ㄱ', 'ㅙ', '과'),
                ('ㄱ', 'ㅚ', '고'),
                ('ㄱ', 'ㅝ', '구'),
                ('ㄱ', 'ㅞ', '궈'),
                ('ㄱ', 'ㅟ', '구'),
                ('ㄱ', 'ㅢ', '그'),
            ] {
                editor.reset(field, slot);
                editor.select(
                    crate::name_input::HANGUL_CONSONANT_KEYS
                        .iter()
                        .position(|&c| c == initial)
                        .unwrap(),
                );
                editor.select(
                    19 + crate::name_input::HANGUL_VOWEL_KEYS
                        .iter()
                        .position(|&c| c == medial)
                        .unwrap(),
                );
                let before = editor.memory[OBJECT..OBJECT + 0x50].to_vec();
                let pointer = editor.pointer();
                let mut r = [0; 32];
                r[10] = pointer as u32 | 0x80000000;
                r[11] = u16::from_le_bytes(editor.memory[pointer..pointer + 2].try_into().unwrap())
                    as u32;
                r[31] = 0x80182c08;
                editor.call(editor.runtime.compound_final_backspace_address, r);
                let mut wanted = before;
                let offset = pointer - OBJECT;
                wanted[offset..offset + 2].copy_from_slice(
                    &(0x8000u16 | (expected as u32 - 0xac00) as u16).to_le_bytes(),
                );
                assert_eq!(
                    &editor.memory[OBJECT..OBJECT + 0x50],
                    wanted,
                    "{medial} -> {expected}, field {field} slot {slot}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn installed_selection_bridge_preserves_fields_cursor_and_calling_convention() {
    use crate::name_input::{NAME_GLYPH_CODE_START, runtime_test_machine::execute_with_callbacks};
    const OBJECT: usize = 0x1c0000;
    const ORIGIN: u32 = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN - 8;
    let mut e = Editor::new();
    // Native page lookup, with distinct selected cells on each page.
    for page in 0..3usize {
        let table = 0x1d0000 + page * 256;
        e.memory[0x17ad08 + page * 4..0x17ad0c + page * 4]
            .copy_from_slice(&(0x80000000 | table as u32).to_le_bytes());
        let key = [0, 40, 92][page];
        e.memory[table + 14..table + 16]
            .copy_from_slice(&(NAME_GLYPH_CODE_START + key).to_le_bytes());
    }
    let jump =
        psx_r3000a::encode(&psx_r3000a::Instruction::J { target: 0x80181194 }, ORIGIN).unwrap();
    e.code[..4].copy_from_slice(&jump.to_le_bytes());
    e.code[4..8].fill(0);
    for field in 0..3u8 {
        let last = if field == 2 { 3 } else { 5 };
        for slot in [0, last] {
            for page in 0..3u8 {
                e.reset(field, slot);
                e.memory[OBJECT + 9] = page;
                e.memory[OBJECT + 12] = 7;
                let pointer = e.pointer();
                let mut expected = e.memory[OBJECT..OBJECT + 0x50].to_vec();
                let code = [
                    0xc000,
                    NAME_GLYPH_CODE_START + 40,
                    NAME_GLYPH_CODE_START + 92,
                ][page as usize];
                expected[pointer - OBJECT..pointer - OBJECT + 2]
                    .copy_from_slice(&code.to_le_bytes());
                if page != 0 {
                    if slot < last {
                        expected[10] += 1;
                    } else {
                        expected[12] = 96;
                    }
                }
                let mut r = [0; 32];
                r[4] = OBJECT as u32 | 0x80000000;
                r[16] = 0x12345678;
                r[17] = 0x23456789;
                r[29] = 0x801fe000;
                r[31] = 0x800f0000;
                execute_with_callbacks(
                    &e.code,
                    ORIGIN,
                    &mut r,
                    &mut e.memory,
                    None,
                    &mut |pc, regs, memory| {
                        if pc != 0x80182c08 {
                            return false;
                        }
                        assert!(matches!(regs[4], 0x91 | 0x92));
                        let sp = (regs[29] & 0x1fffffff) as usize;
                        memory[sp..sp + 16].fill(0xa5);
                        for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                            regs[reg] = 0xdead0000 + reg as u32;
                        }
                        true
                    },
                );
                assert_eq!(
                    &e.memory[OBJECT..OBJECT + 0x50],
                    expected,
                    "field {field}, slot {slot}, page {page}"
                );
                assert_eq!(
                    [r[16], r[17], r[29], r[31]],
                    [0x12345678, 0x23456789, 0x801fe000, 0x800f0000]
                );
            }
        }
    }
}
