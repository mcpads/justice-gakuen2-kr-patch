use super::*;

use crate::name_input::runtime_test_machine::execute;

#[test]
fn emitted_corrections_match_fill_toggles_and_preserve_neighboring_pixels_and_memory() {
    for crop in [[4, 4, 10, 10], [0, 0, 10, 10], [10, 10, 10, 10]] {
        let origin = 0x80010000;
        let program = build_name_fill_correction_program(origin, crop).unwrap();
        let stream = vec![
            0, 0, 5, 0, 1, 9, 10, 99, 0, 1, 3, 5, 50, 98, 0xa3, 0x2b, 1, 0,
        ];
        NameGlyphFillCorrections::parse(stream.clone(), crop).unwrap();
        for code in [0, 1, 256, 11000, 11171, 11172] {
            let mut memory = vec![0x55; 0x1000];
            memory[0x301..0x301 + stream.len()].copy_from_slice(&stream);
            for (i, b) in memory[0x800..0x8c8].iter_mut().enumerate() {
                *b = [0, 0x0d, 0xd0, 0xdd][i % 4];
            }
            let mut expected = memory.clone();
            let coordinates: &[usize] = match code {
                0 => &[0, 1, 9, 10, 99],
                256 => &[5, 50, 98],
                11171 => &[0],
                _ => &[],
            };
            for &c in coordinates {
                let x = crop[0] + c % crop[2];
                let y = crop[1] + c / crop[2];
                expected[0x800 + y * 10 + x / 2] ^= 13 << ((x % 2) * 4);
            }
            let mut r = std::array::from_fn(|i| i as u32 * 7);
            r[0] = 0;
            r[4] = code;
            r[5] = 0x800;
            r[6] = 0x301;
            r[7] = 0x301 + stream.len() as u32;
            r[31] = 0x800f0000;
            let before = r;
            execute(&program, origin, &mut r, &mut memory);
            assert_eq!(memory, expected, "syllable {code}");
            assert_eq!(r[2], u32::from(!coordinates.is_empty()));
            for register in [
                1, 3, 4, 5, 7, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            ] {
                assert_eq!(r[register], before[register], "register {register}");
            }
        }
    }
}

#[test]
fn empty_stream_returns_without_reading_or_writing_memory() {
    let origin = 0x80010000;
    let program = build_name_fill_correction_program(origin, [4, 4, 10, 10]).unwrap();
    let mut r = [0; 32];
    r[5] = u32::MAX;
    r[6] = u32::MAX;
    r[7] = u32::MAX;
    r[31] = 0x800f0000;
    execute(&program, origin, &mut r, &mut []);
    assert_eq!(r[2], 0);
    assert!(build_name_fill_correction_program(origin + 1, [4, 4, 10, 10]).is_err());
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn emitted_corrections_restore_every_supported_syllable_to_direct_font_fill() {
    use crate::font::rasterize_menu_glyphs;
    use crate::name_input::{
        NAME_GLYPH_PACK_STORAGE_BYTES, NameGlyphPackDecoder, build_name_glyph_pack,
        load_ks_x_1001_hangul,
    };
    let font =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
    let repertoire: String = load_ks_x_1001_hangul().unwrap().into_iter().collect();
    for size in [11.0, 12.0] {
        let pack = build_name_glyph_pack(&font, size, NAME_GLYPH_PACK_STORAGE_BYTES).unwrap();
        let decoder = NameGlyphPackDecoder::new(&pack.bytes).unwrap();
        let reference = rasterize_menu_glyphs(&font, &repertoire, size, 3, 13).unwrap();
        let corrections = NameGlyphFillCorrections::build(&pack, &reference).unwrap();
        let origin = 0x80010000;
        let program = build_name_fill_correction_program(origin, pack.report.crop).unwrap();
        let mut memory = vec![0; 0x9000];
        memory[0x101..0x101 + corrections.bytes().len()].copy_from_slice(corrections.bytes());
        for glyph in &reference.glyphs {
            let base = decoder.render(glyph.character).unwrap();
            let mut expected = [0u8; 200];
            memory[0x8000..0x80c8].fill(0);
            for pixel in 0..400 {
                let shift = (pixel % 2) * 4;
                if base[pixel] == 13 {
                    memory[0x8000 + pixel / 2] |= 13 << shift;
                }
                if glyph.pixels[pixel] == 13 {
                    expected[pixel / 2] |= 13 << shift;
                }
            }
            let mut r = [0; 32];
            r[4] = glyph.character as u32 - 0xac00;
            r[5] = 0x8000;
            r[6] = 0x101;
            r[7] = 0x101 + corrections.bytes().len() as u32;
            r[31] = 0x800f0000;
            execute(&program, origin, &mut r, &mut memory);
            assert_eq!(
                memory[0x8000..0x80c8],
                expected,
                "{} at {size}px",
                glyph.character
            );
        }
    }
}
