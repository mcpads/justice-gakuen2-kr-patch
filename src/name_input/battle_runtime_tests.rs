use super::*;
use crate::{
    font::rasterize_menu_glyphs, name_input::runtime_test_machine::execute_with_callbacks_limit,
};

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn complete_generator_uploads_four_independent_records_with_full_glyphs_and_syncs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let hangul = rasterize_menu_glyphs(
        &root.join("../fonts/galmuri/Galmuri14.ttf"),
        "괘궤읽각",
        15.,
        3,
        13,
    )
    .unwrap();
    let pack = NameGlyphBandPack::build(&hangul, 19000).unwrap();
    let mut ascii_reference = rasterize_menu_glyphs(
        &root.join("../fonts/neodgm/neodgm.ttf"),
        &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
        16.,
        3,
        13,
    )
    .unwrap();
    for glyph in &mut ascii_reference.glyphs {
        assert!(glyph.pixels[..20].iter().all(|&p| p == 0));
        glyph.pixels.copy_within(20..400, 0);
        glyph.pixels[380..].fill(0);
    }
    let ascii = SelectorAsciiGlyphs::build(&ascii_reference, 4096).unwrap();
    let offsets: Vec<u16> = (0..NAME_GLYPH_PACK_CELL_COUNT)
        .map(|i| ((i / 19) * 20 * 384 + i % 19 * 10) as u16)
        .collect();
    let atlas = NameInputRuntimeAtlasLayout {
        font_atlas_row_bytes: 384,
        glyph_cell_width: 20,
        glyph_cell_height: 20,
        lookup_table_bytes: offsets.iter().flat_map(|v| v.to_le_bytes()).collect(),
        pack_storage_cell_base_byte_offsets: offsets,
        cache_cell_base_byte_offsets: vec![],
    };
    let mut source = vec![0; 16916];
    source[..8].copy_from_slice(&[16, 0, 0, 0, 0, 0, 0, 0]);
    source[8..12].copy_from_slice(&16908u32.to_le_bytes());
    source[16..20].copy_from_slice(&[6, 0, 128, 5]);
    for (i, b) in source[20..].iter_mut().enumerate() {
        *b = (i * 13 + i / 96 * 7) as u8;
    }
    let keyboard = load_name_input_keyboard(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let direct = layout
        .active_glyphs
        .iter()
        .filter_map(|g| {
            let c = g.character.chars().next()?;
            c.is_ascii().then_some((g.code, c as u8))
        })
        .collect::<Vec<_>>();
    let p = build_battle_name_runtime(&source, &atlas, &pack, &ascii, &direct).unwrap();
    let origin = 0x80010000;
    let mut code = vec![0; (BATTLE_NAME_LOAD_DESTINATION - origin) as usize + p.decoded.len()];
    let mut jump = Assembler::new();
    jump.emit(J {
        target: p.entry_address,
    })
    .emit(psx_r3000a::Instruction::nop());
    code[..8].copy_from_slice(jump.assemble(origin).unwrap().bytes());
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let o = (outline.outline_pixel_address - origin) as usize;
    code[o..o + outline.bytes.len()].copy_from_slice(&outline.bytes);
    code[(BATTLE_NAME_LOAD_DESTINATION - origin) as usize..].copy_from_slice(&p.decoded);
    let off = |a: u32| (a & 0x1fffffff) as usize;
    let mut m = vec![0x55; 0x200000];
    m[off(BATTLE_NAME_LOAD_DESTINATION)..off(BATTLE_NAME_LOAD_DESTINATION) + p.decoded.len()]
        .copy_from_slice(&p.decoded);
    let mut stored = vec![0; 19000];
    stored[..pack.bytes().len()].copy_from_slice(pack.bytes());
    for (cell, &base) in atlas.pack_storage_cell_base_byte_offsets.iter().enumerate() {
        for row in 0..20 {
            let at = off(BATTLE_NAME_FONT_PIXELS) + usize::from(base) + row * 384;
            m[at..at + 10]
                .copy_from_slice(&stored[cell * 200 + row * 10..cell * 200 + row * 10 + 10]);
        }
    }
    let tag = |c: char| 0x8000 | ((c as u32 - 0xac00) as u16);
    let words = [
        [tag('괘'), tag('궤'), tag('읽'), tag('궤')],
        [0x4041, 0x407a, 0x4030, 0x405f],
        [0x339, 0x33a, 0x33b, 0x33c],
        [1, 0xa8, 0x3001, 0xdead],
    ];
    let indexes = [8u8, 7, 16, 0];
    m[0x1f6494..0x1f6498].fill(30);
    m[0x1f64fc..0x1f6500].copy_from_slice(&indexes);
    for (i, &idx) in indexes.iter().enumerate() {
        for (j, &word) in words[i].iter().enumerate() {
            let at = 0x1f5814 + usize::from(idx) * 40 + j * 2;
            m[at..at + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
    for (i, word) in [tag('각'), 0x0359, 0x03b8, 0x03ab].iter().enumerate() {
        m[0x1f1896 + i * 2..0x1f1898 + i * 2].copy_from_slice(&word.to_le_bytes());
    }
    let palette = |p: u8| match p {
        13 => 15,
        3 => 2,
        _ => 0,
    };
    let mut expected = vec![vec![0u8; 512]; 4];
    for (slot, text) in [(0, "괘궤읽궤"), (1, "Az0_"), (2, "각A_0")] {
        for (column, c) in text.chars().enumerate() {
            let g = hangul
                .glyphs
                .iter()
                .chain(&ascii_reference.glyphs)
                .find(|g| g.character == c)
                .unwrap();
            for y in 0..16 {
                for x in 0..16 {
                    let v = palette(g.pixels[(y + 1) * 20 + x + 2]);
                    expected[slot][y * 32 + column * 8 + x / 2] |= v << (4 * (x % 2));
                }
            }
        }
    }
    for (column, glyph) in [1, 0x54].into_iter().enumerate() {
        for y in 0..16 {
            for x in 0..16 {
                let sx = x * 3 / 2;
                let b = source[20 + glyph * 96 + y / 2 * 12 + sx / 2];
                let v = (b >> (4 * (sx % 2))) & 15;
                expected[3][y * 32 + column * 8 + x / 2] |= v << (4 * (x % 2));
            }
        }
    }
    let before = m.clone();
    let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
    r[0] = 0;
    r[29] = 0x801fe000;
    r[31] = 0x80027f24;
    let saved = r;
    let mut calls = 0;
    execute_with_callbacks_limit(
        &code,
        origin,
        &mut r,
        &mut m,
        None,
        1_000_000,
        &mut |pc, r, m| {
            if pc != 0x80062630 && pc != 0x80062384 {
                return false;
            }
            let slot = calls / 2;
            if calls % 2 == 0 {
                assert_eq!(pc, 0x80062630);
                assert!(slot < 4);
                let at = off(r[4]);
                let rect: Vec<u16> = m[at..at + 8]
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes(p.try_into().unwrap()))
                    .collect();
                assert_eq!(
                    rect,
                    [
                        [768, 320, 16, 16],
                        [768, 336, 16, 16],
                        [928, 496, 16, 16],
                        [944, 496, 16, 16]
                    ][slot]
                );
                assert_eq!(r[5], BATTLE_NAME_BUFFER_ADDRESS);
                assert_eq!(
                    &m[off(r[5])..off(r[5]) + 512],
                    expected[slot],
                    "slot {slot}"
                );
            } else {
                assert_eq!(pc, 0x80062384);
                assert_eq!(r[4], 0);
            }
            calls += 1;
            for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                r[reg] = 0xdead0000 + reg as u32;
            }
            let sp = off(r[29]);
            m[sp..sp + 16].fill(0xcc);
            true
        },
    );
    assert_eq!(calls, 8);
    for reg in (16..24).chain([28, 29, 30, 31]) {
        assert_eq!(r[reg], saved[reg]);
    }
    let mut allowed = before;
    for (at, len) in [
        (off(BATTLE_NAME_PACK_ADDRESS), 19000),
        (off(BATTLE_NAME_SCRATCH_ADDRESS), 200),
        (off(BATTLE_NAME_BUFFER_ADDRESS), 512),
        (off(saved[29]) - 256, 256),
    ] {
        allowed[at..at + len].copy_from_slice(&m[at..at + len]);
    }
    assert_eq!(
        m, allowed,
        "battle generator wrote outside pack, scratch, buffer or stack"
    );
}

#[test]
#[ignore = "requires assets/"]
fn diary_word_resolves_all_keyboard_keys_by_slot_without_borrowing_registered_names() {
    let keyboard = load_name_input_keyboard(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let direct = layout
        .active_glyphs
        .iter()
        .filter_map(|g| {
            let c = g.character.chars().next()?;
            c.is_ascii().then_some((g.code, c as u8))
        })
        .collect::<Vec<_>>();
    let mut a = Assembler::new();
    emit_diary_battle_word(&mut a, &direct).unwrap();
    a.emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
    let origin = 0x80010000;
    let code = a.assemble(origin).unwrap();
    let cases = direct
        .iter()
        .map(|&(code, character)| (code, 0x4000 | u16::from(character)))
        .chain([
            (0x8118, 0x8118),
            (0x405f, 0x405f),
            (0x3001, 0x3001),
            (0x061e, 0x57),
            (0xc000, 0x57),
        ])
        .collect::<Vec<_>>();
    let mut m = vec![0x55; 0x200000];
    for (raw, expected) in cases {
        for slot in 0..4 {
            m[0x1f1896 + slot * 2..0x1f1898 + slot * 2].copy_from_slice(&raw.to_le_bytes());
            for record in 0..17 {
                let before = m.clone();
                let mut r = [0u32; 32];
                r[4] = 0x57;
                r[17] = record;
                r[19] = slot as u32;
                r[31] = 0x80027f24;
                execute_with_callbacks_limit(
                    code.bytes(),
                    origin,
                    &mut r,
                    &mut m,
                    None,
                    1000,
                    &mut |_, _, _| false,
                );
                assert_eq!(
                    r[4],
                    if record == 16 {
                        u32::from(expected)
                    } else {
                        0x57
                    }
                );
                assert_eq!(m, before, "word resolution must not rewrite any record");
            }
        }
    }
}
