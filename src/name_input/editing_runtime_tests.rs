use super::runtime_test_machine::execute;
use super::*;
use psx_r3000a::{Assembler, Instruction};

const OBJECT: usize = 0x1c0000;
const ORIGIN: u32 = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN - 8;

pub(crate) struct Editor {
    pub(crate) code: Vec<u8>,
    pub(crate) memory: Vec<u8>,
    pub(crate) runtime: NameInputRedisplayRuntimeProgram,
}

impl Editor {
    pub(crate) fn new() -> Self {
        let atlas = super::redisplay_runtime_tests::fixture_atlas();
        let repertoire: String = load_name_input_hangul().unwrap().into_iter().collect();
        let font =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
        let glyphs = crate::font::rasterize_menu_glyphs(&font, &repertoire, 12.0, 3, 13).unwrap();
        let pack = NameGlyphBandPack::build(&glyphs, NAME_GLYPH_PACK_STORAGE_BYTES).unwrap();
        let outline = build_shared_name_outline_runtime_program().unwrap();
        let materializer = build_name_input_band_runtime_program(
            &atlas,
            &pack,
            &outline,
            NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        )
        .unwrap();
        let runtime = build_name_input_band_redisplay_runtime_program(
            &atlas,
            materializer.glyph_fill_materializer_address.unwrap(),
            &pack,
        )
        .unwrap();
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            &runtime.instructions,
            NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
            "name editing",
        )
        .unwrap();
        let mut code = vec![0; 8];
        code.extend_from_slice(&runtime.bytes);
        code.resize((NAME_INPUT_RUNTIME_ORIGIN - ORIGIN) as usize, 0);
        code.extend_from_slice(&materializer.bytes);
        let bridge_origin = 0x80181194;
        let bridge = crate::dialogue_audit::build_name_entry_selection_replacement(
            runtime.selected_key_handler_address,
        )
        .unwrap();
        code.resize((bridge_origin - ORIGIN) as usize, 0);
        for (i, instruction) in bridge.iter().enumerate() {
            code.extend_from_slice(
                &psx_r3000a::encode(instruction, bridge_origin + i as u32 * 4)
                    .unwrap()
                    .to_le_bytes(),
            );
        }

        let mut memory = vec![0; 0x200000];
        let address = (NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN & 0x1fffffff) as usize;
        memory[address..address + runtime.bytes.len()].copy_from_slice(&runtime.bytes);
        let materializer_address = (NAME_INPUT_RUNTIME_ORIGIN & 0x1fffffff) as usize;
        memory[materializer_address..materializer_address + materializer.bytes.len()]
            .copy_from_slice(&materializer.bytes);
        let mut payload = pack.bytes().to_vec();
        payload.resize(18800, 0);
        payload.extend_from_slice(&atlas.lookup_table_bytes);
        payload.resize(NAME_GLYPH_PACK_STORAGE_BYTES, 0);
        for (i, cell) in payload.as_chunks::<200>().0.iter().enumerate() {
            let base = 0xe92e0 + atlas.pack_storage_cell_base_byte_offsets[i] as usize;
            for row in 0..20 {
                memory[base + row * 384..base + row * 384 + 10]
                    .copy_from_slice(&cell[row * 10..row * 10 + 10]);
            }
        }
        let mut editor = Self {
            code,
            memory,
            runtime,
        };
        editor.reset(0, 0);
        editor
    }
    pub(crate) fn reset(&mut self, field: u8, slot: u8) {
        self.memory[OBJECT..OBJECT + 0x50].fill(0);
        self.memory[OBJECT + 15] = field;
        self.memory[OBJECT + 10] = slot;
        for field in 0..3 {
            for slot in 0..6 {
                let address = OBJECT + 0x12 + field * 16 + slot * 2;
                self.memory[address..address + 2].copy_from_slice(&EMPTY_NAME_SLOT.to_le_bytes());
            }
        }
    }
    pub(crate) fn pointer(&self) -> usize {
        OBJECT
            + 0x12
            + self.memory[OBJECT + 15] as usize * 16
            + self.memory[OBJECT + 10] as usize * 2
    }
    pub(crate) fn call(&mut self, target: u32, mut registers: [u32; 32]) -> [u32; 32] {
        let mut a = Assembler::new();
        a.emit(Instruction::J { target }).emit(Instruction::nop());
        self.code[..8].copy_from_slice(a.assemble(ORIGIN).unwrap().bytes());
        registers[29] = 0x801fe000;
        if registers[31] == 0 {
            registers[31] = 0x800f0000;
        }
        execute(&self.code, ORIGIN, &mut registers, &mut self.memory);
        assert_eq!(registers[29], 0x801fe000);
        registers
    }
    pub(crate) fn select(&mut self, key: usize) {
        let mut r = [0; 32];
        r[2] = self.pointer() as u32 | 0x80000000;
        r[3] = NAME_GLYPH_CODE_START as u32 + key as u32;
        r[16] = OBJECT as u32 | 0x80000000;
        let r = self.call(self.runtime.selected_key_handler_address, r);
        assert_eq!(r[2], 0, "Hangul must not invoke native auto-advance");
    }
    pub(crate) fn words(&self) -> Vec<u16> {
        let base = OBJECT + 0x12 + self.memory[OBJECT + 15] as usize * 16;
        self.memory[base..base + 12]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect()
    }
}
fn syllable(c: char) -> u16 {
    HANGUL_NAME_TAG | (c as u32 - 0xac00) as u16
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn consecutive_name_syllables_do_not_drop_an_inadmissible_final() {
    let mut e = Editor::new();
    let keys: Vec<char> = HANGUL_CONSONANT_KEYS
        .into_iter()
        .chain(HANGUL_VOWEL_KEYS)
        .collect();
    for field in 0..3 {
        for (input, expected) in [
            ("ㄱㅣㅁㅎㅏㄴㅡㄹ", "김하늘"),
            ("ㅂㅏㄱㅅㅓㅈㅜㄴ", "박서준"),
            ("ㅂㅗㅁㅂㅣㅊㄴㅜㄹㅣ", "봄빛누리"),
            ("ㄲㅗㅊㅂㅣㅊ", "꽃빛"),
        ] {
            e.reset(field, 0);
            for c in input.chars() {
                e.select(keys.iter().position(|&key| key == c).unwrap());
            }
            let words: Vec<_> = expected.chars().map(syllable).collect();
            assert_eq!(
                &e.words()[..words.len()],
                words,
                "{expected}, field {field}"
            );
        }
        for prefix in ["ㅅㅓ", "ㄱㅏㄱ"] {
            e.reset(field, if field == 2 { 3 } else { 5 });
            for c in prefix.chars() {
                e.select(keys.iter().position(|&key| key == c).unwrap());
            }
            let before = e.memory[OBJECT..OBJECT + 0x50].to_vec();
            e.select(if prefix == "ㅅㅓ" { 12 } else { 9 });
            assert_eq!(&e.memory[OBJECT..OBJECT + 0x50], before);
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn native_emitted_editing_keeps_initial_medial_final_in_the_current_slot() {
    let mut e = Editor::new();
    for (key, word) in [(0, 0xc000), (19, syllable('가')), (0, syllable('각'))] {
        e.select(key);
        assert_eq!(e.words()[0], word);
        assert_eq!(e.memory[OBJECT + 10], 0);
        assert_eq!(e.words()[1], EMPTY_NAME_SLOT);
    }
    e.select(19);
    assert_eq!(&e.words()[..2], &[syllable('가'), syllable('가')]);
    assert_eq!(e.memory[OBJECT + 10], 1);
    e.select(4); // ㄸ cannot be a final: begin the next initial.
    assert_eq!(e.words()[2], 0xc004);
    assert_eq!(e.memory[OBJECT + 10], 2);
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn native_emitted_editing_handles_compound_finals_and_last_slot_without_overflow() {
    let mut e = Editor::new();
    for field in 0..3 {
        let last = if field == 2 { 3 } else { 5 };
        e.reset(field, last);
        for key in [0, 19, 0] {
            e.select(key);
        }
        let before = e.memory[OBJECT..OBJECT + 0x50].to_vec();
        e.select(19); // Splitting needs another slot; reject atomically.
        assert_eq!(&e.memory[OBJECT..OBJECT + 0x50], before);
        e.select(4);
        assert_eq!(&e.memory[OBJECT..OBJECT + 0x50], before);
    }
    e.reset(0, 0);
    for key in [0, 19, 0, 9] {
        e.select(key);
    } // ㄱㅏㄱㅅ -> 갃 (unsupported)
    assert_eq!(e.words()[0], syllable('각'));
    e.reset(0, 0);
    for key in [0, 19, 5, 0] {
        e.select(key);
    } // ㄱㅏㄹㄱ -> 갉
    assert_eq!(e.words()[0], syllable('갉'));
    e.select(19);
    assert_eq!(&e.words()[..2], &[syllable('갈'), syllable('가')]);
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn unfinished_initials_resolve_to_their_visible_keyboard_glyphs() {
    let mut e = Editor::new();
    for initial in 0..19 {
        let mut r = [0; 32];
        r[4] = 0xc000 | initial;
        r[5] = 0x801c8118;
        r[6] = 0;
        let r = e.call(e.runtime.tagged_code_resolver_address, r);
        assert_eq!(r[2], NAME_GLYPH_CODE_START as u32 + initial);
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn every_supported_syllable_can_be_composed_including_the_last_name_slot() {
    let mut e = Editor::new();
    let simple = [
        1u8, 2, 4, 7, 255, 8, 16, 17, 255, 19, 20, 21, 22, 255, 23, 24, 25, 26, 27,
    ];
    let compound = [
        (1u16, 9u16, 3u16),
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
    let repertoire = load_name_input_hangul().unwrap();
    for &c in &repertoire {
        let scalar = c as usize - 0xac00;
        let initial = scalar / 588;
        let medial = scalar / 28 % 21;
        let final_index = scalar % 28;
        let mut keys = vec![initial, medial + 19];
        if final_index != 0 {
            if let Some(key) = simple.iter().position(|&f| f as usize == final_index) {
                keys.push(key);
            } else {
                let entry = compound
                    .iter()
                    .map(|&(first, next, result)| first | (next << 5) | (result << 10))
                    .find(|entry| (*entry >> 10) as usize == final_index)
                    .unwrap();
                keys.push(simple.iter().position(|&f| f as u16 == entry & 31).unwrap());
                keys.push(((entry >> 5) & 31) as usize);
            }
        }
        for (field, slot) in [(0, 0), (0, 5), (1, 5), (2, 3)] {
            e.reset(field, slot);
            for &key in &keys {
                e.select(key);
            }
            assert_eq!(
                e.words()[slot as usize],
                syllable(c),
                "{c}, field {field}, slot {slot}"
            );
            assert_eq!(e.memory[OBJECT + 10], slot);
            // Once a final is split, the retained old syllable must still be supported.
            if slot == 0 && final_index != 0 {
                e.select(19);
                let words = e.words();
                for word in words.into_iter().filter(|w| *w != EMPTY_NAME_SLOT) {
                    assert!(
                        repertoire
                            .contains(&char::from_u32(0xac00 + (word & 0x3fff) as u32).unwrap())
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn redisplay_uploads_only_changed_syllables_and_keeps_field_caches_distinct() {
    use psx_r3000a::{Instruction::*, Register as R};
    let mut e = Editor::new();
    // The decoder and uploader are independently exercised above; these stubs
    // count calls so a cache hit proves neither expensive operation was entered.
    for (target, counter) in [
        (
            u32::from_str_radix(&e.runtime.report.materializer_address[2..], 16).unwrap(),
            0x190000u32,
        ),
        (e.runtime.cache_cell_uploader_address, 0x190004u32),
    ] {
        let mut a = Assembler::new();
        a.emit_all(psx_r3000a::load_address(R::T0, counter | 0x80000000))
            .emit(Lw {
                rt: R::T1,
                base: R::T0,
                offset: 0,
            })
            .emit(psx_r3000a::Instruction::nop())
            .emit(Addiu {
                rt: R::T1,
                rs: R::T1,
                immediate: 1,
            })
            .emit(Sw {
                rt: R::T1,
                base: R::T0,
                offset: 0,
            })
            .emit(Jr { rs: R::RA })
            .emit(Ori {
                rt: R::V0,
                rs: R::ZERO,
                immediate: 1,
            });
        let bytes = a.assemble(target).unwrap().bytes().to_vec();
        let offset = (target - ORIGIN) as usize;
        e.code[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }
    for (field_buffer, slot, c, expected_calls) in [
        (0x801c8118, 0, '가', 1),
        (0x801c8118, 0, '가', 1),
        (0x801c8118, 0, '각', 2),
        (0x801c8118, 0, '각', 2),
        (0x801c82d8, 0, '각', 3),
        (0x801c8498, 3, '각', 4),
        (0x801c8118, 0, '각', 4),
    ] {
        let mut r = [0; 32];
        r[4] = syllable(c) as u32;
        r[5] = field_buffer;
        r[6] = slot;
        let r = e.call(e.runtime.tagged_code_resolver_address, r);
        assert_ne!(r[2], 65535);
        for offset in [0x190000, 0x190004] {
            assert_eq!(
                u32::from_le_bytes(e.memory[offset..offset + 4].try_into().unwrap()),
                expected_calls
            );
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn selected_medials_compose_or_replace_in_place_even_in_the_last_slot() {
    let mut editor = Editor::new();
    let keys: Vec<_> = HANGUL_CONSONANT_KEYS
        .into_iter()
        .chain(HANGUL_VOWEL_KEYS)
        .collect();
    for field in 0..3 {
        for slot in [0, if field == 2 { 3 } else { 5 }] {
            for (input, expected) in [
                ("ㄱㅗㅏ", '과'),
                ("ㄱㅗㅐ", '괘'),
                ("ㄱㅗㅣ", '괴'),
                ("ㄱㅜㅓ", '궈'),
                ("ㄱㅜㅔ", '궤'),
                ("ㄱㅜㅣ", '귀'),
                ("ㄱㅡㅣ", '긔'),
                ("ㄱㅘㅣ", '괘'),
                ("ㄱㅝㅣ", '궤'),
                ("ㄱㅗㅘ", '과'),
                ("ㄱㅏㅓ", '거'),
            ] {
                editor.reset(field, slot);
                let before = editor.memory[OBJECT..OBJECT + 0x50].to_vec();
                for c in input.chars() {
                    editor.select(keys.iter().position(|&k| k == c).unwrap());
                }
                let mut wanted = before;
                let offset = 0x12 + field as usize * 16 + slot as usize * 2;
                wanted[offset..offset + 2].copy_from_slice(&syllable(expected).to_le_bytes());
                assert_eq!(
                    &editor.memory[OBJECT..OBJECT + 0x50],
                    wanted,
                    "{input} -> {expected}, field {field}, slot {slot}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn unfinished_initial_replacement_and_direct_key_rejection_preserve_all_fields() {
    let mut e = Editor::new();
    for field in 0..3 {
        for slot in [0, if field == 2 { 3 } else { 5 }] {
            e.reset(field, slot);
            e.select(0);
            e.select(2);
            assert_eq!(e.words()[slot as usize], 0xc002);
            let before = e.memory[OBJECT..OBJECT + 0x50].to_vec();
            for key in [40, 92] {
                let mut r = [0; 32];
                r[2] = e.pointer() as u32 | 0x80000000;
                r[3] = NAME_GLYPH_CODE_START as u32 + key;
                r[16] = OBJECT as u32 | 0x80000000;
                let r = e.call(e.runtime.selected_key_handler_address, r);
                assert_eq!(r[2], 0);
                assert_eq!(&e.memory[OBJECT..OBJECT + 0x50], before);
            }
            e.select(19);
            assert_eq!(e.words()[slot as usize], syllable('나'));
            assert_eq!(e.memory[OBJECT + 10], slot);
        }
    }
}
