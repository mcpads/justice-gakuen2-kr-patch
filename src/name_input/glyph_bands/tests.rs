use super::*;
use crate::font::{RasterizedMenuGlyphSet, rasterize_menu_glyphs};

fn reference(chars: &str, size: f32) -> RasterizedMenuGlyphSet {
    let font =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
    rasterize_menu_glyphs(&font, chars, size, 3, 13).unwrap()
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn exact_band_pack_restores_supported_fill_within_existing_storage() {
    let repertoire: String = crate::name_input::load_ks_x_1001_hangul()
        .unwrap()
        .into_iter()
        .collect();
    for size in [11.0, 12.0] {
        let reference = reference(&repertoire, size);
        let pack =
            NameGlyphBandPack::build(&reference, crate::name_input::NAME_GLYPH_PACK_STORAGE_BYTES)
                .unwrap();
        let reread = NameGlyphBandPack::parse(pack.bytes().to_vec()).unwrap();
        assert_eq!(reread.glyph_count(), reference.glyphs.len());
        for scalar in 0xac00..=0xd7a3 {
            let c = char::from_u32(scalar).unwrap();
            assert_eq!(reread.supports(c), repertoire.contains(c));
        }
        for glyph in &reference.glyphs {
            let mut expected = [0; 200];
            for (i, &pixel) in glyph.pixels.iter().enumerate() {
                if pixel == 13 {
                    expected[i / 2] |= 13 << ((i % 2) * 4);
                }
            }
            assert_eq!(
                reread.render_fill(glyph.character).unwrap(),
                expected,
                "{} at {size}px",
                glyph.character
            );
        }
        assert!(NameGlyphBandPack::build(&reference, pack.bytes().len() - 1).is_err());
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn sparse_membership_and_single_dictionary_entry_do_not_alias_unsupported_names() {
    let pack = NameGlyphBandPack::build(&reference("힣", 12.0), 19000).unwrap();
    assert!(pack.supports('힣'));
    for c in ['가', 'A', '\u{abff}', '\u{d7a4}'] {
        assert!(!pack.supports(c));
        assert!(pack.render_fill(c).is_err());
    }
    assert!(pack.bands.iter().all(|b| b.bits == 0));
    NameGlyphBandPack::parse(pack.bytes().to_vec()).unwrap();
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn malformed_band_data_is_rejected_before_pixel_lookup() {
    let pack = NameGlyphBandPack::build(&reference("가각힣", 12.0), 19000).unwrap();
    let bytes = pack.bytes();
    for end in 0..bytes.len() {
        assert!(
            NameGlyphBandPack::parse(bytes[..end].to_vec()).is_err(),
            "truncation at {end}"
        );
    }
    for (offset, value) in [
        (0, 0),
        (11, 1),
        (6, 0),
        (HEADER, 1),
        (12, 0),
        (pack.ranks, 1),
        (pack.ranks - 1, 0xff),
    ] {
        let mut bad = bytes.to_vec();
        bad[offset] = value;
        assert!(
            NameGlyphBandPack::parse(bad).is_err(),
            "mutation at {offset}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(NameGlyphBandPack::parse(trailing).is_err());
    let band = pack
        .bands
        .iter()
        .find(|b| (b.indices - b.dictionary) / b.stride == 3)
        .unwrap();
    let mut invalid_index = bytes.to_vec();
    invalid_index[band.indices] |= 3;
    assert!(NameGlyphBandPack::parse(invalid_index).is_err());
    let mut padding = bytes.to_vec();
    padding[band.indices] |= 0x80;
    assert!(NameGlyphBandPack::parse(padding).is_err());
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn band_builder_rejects_ambiguous_or_unrenderable_repertoires() {
    let mut duplicate = reference("가", 12.0);
    duplicate
        .glyphs
        .push(reference("가", 12.0).glyphs.pop().unwrap());
    assert!(NameGlyphBandPack::build(&duplicate, 19000).is_err());
    for invalid in [0, 1, 2] {
        let mut glyphs = reference("가", 12.0);
        match invalid {
            0 => glyphs.glyphs[0].pixels.fill(0),
            1 => glyphs.glyphs[0].character = 'A',
            _ => glyphs.glyphs[0].pixels[0] = 7,
        }
        assert!(NameGlyphBandPack::build(&glyphs, 19000).is_err());
    }
    let mut empty = reference("가", 12.0);
    empty.glyphs.clear();
    assert!(NameGlyphBandPack::build(&empty, 19000).is_err());
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn emitted_band_materializer_matches_every_glyph_and_preserves_strided_neighbors() {
    use crate::name_input::runtime_test_machine::execute_with_reader;
    let repertoire: String = crate::name_input::load_ks_x_1001_hangul()
        .unwrap()
        .into_iter()
        .collect();
    let origin = 0x80010000;
    let reader = 0x80020000;
    for size in [11.0, 12.0] {
        let pack = NameGlyphBandPack::build(&reference(&repertoire, size), 19000).unwrap();
        for stride in [10, 11, 384] {
            let program = pack.build_fill_program(origin, reader, stride).unwrap();
            for c in repertoire
                .chars()
                .chain(['A', '힣', '\u{abff}', '\u{d7a4}'])
            {
                let mut memory = vec![0x55; 0x4000];
                let mut expected = memory.clone();
                let mut r = std::array::from_fn(|i| i as u32 * 13);
                r[0] = 0;
                r[4] = (c as u32).wrapping_sub(0xac00);
                r[5] = 0x202;
                r[29] = 0x3000;
                r[31] = 0x800f0000;
                let before = r;
                if pack.supports(c) {
                    let fill = pack.render_fill(c).unwrap();
                    for row in 0..20 {
                        expected[0x202 + row * stride..0x202 + row * stride + 10]
                            .copy_from_slice(&fill[row * 10..row * 10 + 10]);
                    }
                }
                execute_with_reader(
                    &program,
                    origin,
                    &mut r,
                    &mut memory,
                    Some((reader, pack.bytes())),
                );
                assert_eq!(
                    memory[..0x2fc0],
                    expected[..0x2fc0],
                    "{c} size {size} stride {stride}"
                );
                assert_eq!(memory[0x3000..], expected[0x3000..]);
                assert_eq!(r[2], if pack.supports(c) { 0x202 } else { 0 });
                for register in (16..=23).chain([29, 31]) {
                    assert_eq!(r[register], before[register], "register {register}");
                }
            }
        }
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn emitted_single_mask_uses_no_index_bits_and_validates_placement() {
    use crate::name_input::runtime_test_machine::execute_with_reader;
    let pack = NameGlyphBandPack::build(&reference("힣", 12.0), 19000).unwrap();
    let origin = 0x80010000;
    let reader = 0x80020000;
    let bytes = pack.build_fill_program(origin, reader, 11).unwrap();
    let mut memory = vec![0; 0x4000];
    let mut r = [0; 32];
    r[4] = 11171;
    r[5] = 0x202;
    r[29] = 0x3000;
    r[31] = 0x800f0000;
    execute_with_reader(
        &bytes,
        origin,
        &mut r,
        &mut memory,
        Some((reader, pack.bytes())),
    );
    let fill = pack.render_fill('힣').unwrap();
    for row in 0..20 {
        assert_eq!(
            memory[0x202 + row * 11..0x202 + row * 11 + 10],
            fill[row * 10..row * 10 + 10]
        );
    }
    assert!(pack.build_fill_program(origin + 1, reader, 10).is_err());
    assert!(pack.build_fill_program(origin, reader + 1, 10).is_err());
    assert!(pack.build_fill_program(origin, reader, 9).is_err());
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn emitted_band_renderer_matches_full_font_pixels_with_shared_outline_code() {
    use crate::name_input::runtime_test_machine::execute_with_reader;
    use crate::name_input::{
        SHARED_NAME_OUTLINE_RUNTIME_ORIGIN, build_shared_name_outline_runtime_program,
    };
    let repertoire: String = crate::name_input::load_ks_x_1001_hangul()
        .unwrap()
        .into_iter()
        .collect();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let origin = 0x80010000;
    let reader = 0x80020000;
    for size in [11.0, 12.0] {
        let reference = reference(&repertoire, size);
        let pack = NameGlyphBandPack::build(&reference, 19000).unwrap();
        for stride in [10, 384] {
            let mut program = pack
                .build_render_program(origin, reader, stride, &outline)
                .unwrap();
            let offset = (SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - origin) as usize;
            assert!(program.len() <= offset);
            program.resize(offset, 0);
            program.extend_from_slice(&outline.bytes);
            for glyph in &reference.glyphs {
                let mut memory = vec![0x55; 0x4000];
                let mut expected = memory.clone();
                let mut r = [0; 32];
                r[4] = glyph.character as u32 - 0xac00;
                r[5] = 0x202;
                r[29] = 0x3000;
                r[31] = 0x800f0000;
                for row in 0..20 {
                    for col in 0..10 {
                        expected[0x202 + row * stride + col] = glyph.pixels[row * 20 + col * 2]
                            | glyph.pixels[row * 20 + col * 2 + 1] << 4;
                    }
                }
                execute_with_reader(
                    &program,
                    origin,
                    &mut r,
                    &mut memory,
                    Some((reader, pack.bytes())),
                );
                assert_eq!(
                    memory[..0x2fc0],
                    expected[..0x2fc0],
                    "{} at {size}px stride {stride}",
                    glyph.character
                );
                assert_eq!(memory[0x3000..], expected[0x3000..]);
                assert_eq!(r[2], 0x202);
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn name_entry_band_caller_reads_scattered_storage_and_preserves_other_cells() {
    use crate::name_input::redisplay_runtime::NAME_GLYPH_CACHE_TABLE_ADDRESS;
    use crate::name_input::runtime_test_machine::execute;
    use crate::name_input::*;
    use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address};
    let keyboard = load_name_input_keyboard(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let cells = (0..228)
        .map(|i| crate::tim::Cell {
            x: i / 84 * 256 + i % 84 % 12 * 20,
            y: i % 84 / 12 * 20,
            width: 20,
            height: 20,
        })
        .collect::<Vec<_>>();
    let atlas = plan_name_input_runtime_atlas(&cells, &keyboard).unwrap();
    let repertoire: String = load_ks_x_1001_hangul().unwrap().into_iter().collect();
    let reference = reference(&repertoire, 12.0);
    let pack = NameGlyphBandPack::build(&reference, NAME_GLYPH_PACK_STORAGE_BYTES).unwrap();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let hud = 0x80011000;
    let runtime = build_name_input_band_runtime_program(&atlas, &pack, &outline, hud).unwrap();
    let origin = SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - 8;
    let mut a = Assembler::new();
    a.emit(J {
        target: runtime.glyph_fill_materializer_address.unwrap(),
    })
    .emit(psx_r3000a::Instruction::nop());
    let mut program = a.assemble(origin).unwrap().bytes().to_vec();
    program.extend_from_slice(&outline.bytes);
    assert!(program.len() <= (hud - origin) as usize);
    program.resize((hud - origin) as usize, 0);
    let mut a = Assembler::new();
    a.emit_all(load_address(R::T0, 0x801ff000))
        .emit(Sw {
            rt: R::A0,
            base: R::T0,
            offset: 0,
        })
        .emit(Sw {
            rt: R::A1,
            base: R::T0,
            offset: 4,
        })
        .emit(Sw {
            rt: R::A2,
            base: R::T0,
            offset: 8,
        })
        // The existing HUD store returns a success flag, not the glyph pointer.
        .emit(Ori {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 1,
        })
        .emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
    program.extend_from_slice(a.assemble(hud).unwrap().bytes());
    program.resize((NAME_INPUT_RUNTIME_ORIGIN - origin) as usize, 0);
    program.extend_from_slice(&runtime.bytes);
    let mut payload = pack.bytes().to_vec();
    payload.resize(18800, 0);
    payload.extend_from_slice(&atlas.lookup_table_bytes);
    payload.resize(19000, 0);
    let mut baseline = vec![0x55; 0x200000];
    for (i, cell) in payload.as_chunks::<200>().0.iter().enumerate() {
        let base = 0xe92e0 + atlas.pack_storage_cell_base_byte_offsets[i] as usize;
        for row in 0..20 {
            baseline[base + row * 384..base + row * 384 + 10]
                .copy_from_slice(&cell[row * 10..row * 10 + 10]);
        }
    }
    for (slot, offset) in atlas.cache_cell_base_byte_offsets.iter().enumerate() {
        let base = (NAME_GLYPH_CACHE_TABLE_ADDRESS & 0x1fff_ffff) as usize + slot * 4;
        baseline[base..base + 2].copy_from_slice(&offset.to_le_bytes());
    }
    for slot in 0..=16 {
        for c in ['가', '각', '쐐', '힝', '힣'] {
            let mut memory = baseline.clone();
            let mut expected = baseline.clone();
            let supported = slot < 16 && pack.supports(c);
            let pointer = if supported {
                0x800e92e0 + u32::from(atlas.cache_cell_base_byte_offsets[slot])
            } else {
                0
            };
            if supported {
                let glyph = reference.glyphs.iter().find(|g| g.character == c).unwrap();
                let base = (pointer & 0x1fff_ffff) as usize;
                for row in 0..20 {
                    for col in 0..10 {
                        expected[base + row * 384 + col] = glyph.pixels[row * 20 + col * 2]
                            | glyph.pixels[row * 20 + col * 2 + 1] << 4;
                    }
                }
                expected[0x1ff000..0x1ff004].copy_from_slice(&(slot as u32).to_le_bytes());
                expected[0x1ff004..0x1ff008].copy_from_slice(&pointer.to_le_bytes());
                expected[0x1ff008..0x1ff00c].copy_from_slice(&384u32.to_le_bytes());
            }
            let mut r = std::array::from_fn(|i| i as u32 * 13);
            r[0] = 0;
            r[4] = c as u32 - 0xac00;
            r[5] = slot as u32;
            r[29] = 0x801fe000;
            r[31] = 0x800f0000;
            let before = r;
            execute(&program, origin, &mut r, &mut memory);
            assert_eq!(r[2], u32::from(supported), "{c} slot {slot}");
            for reg in (16..24).chain([28, 29, 30, 31]) {
                assert_eq!(r[reg], before[reg]);
            }
            // The adapter and renderer own 32 + 64 bytes below the incoming SP.
            memory[0x1fdfa0..0x1fe000].copy_from_slice(&expected[0x1fdfa0..0x1fe000]);
            assert!(
                memory == expected,
                "memory changed outside target glyph, stack or HUD record: {c} slot {slot}"
            );
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn dialogue_band_caller_fits_with_existing_consumers_and_reload_support() {
    use crate::name_input::*;
    let keyboard = load_name_input_keyboard(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let repertoire: String = load_ks_x_1001_hangul().unwrap().into_iter().collect();
    let pack =
        NameGlyphBandPack::build(&reference(&repertoire, 12.0), NAME_GLYPH_PACK_STORAGE_BYTES)
            .unwrap();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let hud_layout = plan_nickname_hud_glyph_layout_for_crop(
        pack.crop(),
        NicknameHudGlyphStyle {
            scale_percent: 115,
            vertical_shift_px: -1,
        },
    )
    .unwrap();
    let program =
        build_name_dialogue_band_runtime_program(&layout, &pack, &outline, &hud_layout).unwrap();
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &program.instructions,
        NAME_DIALOGUE_RUNTIME_ORIGIN + program.instruction_offset as u32,
        "band dialogue",
    )
    .unwrap();
    assert!(program.report.component_resolver_address.is_none());
    assert!(program.report.glyph_pack_coordinate_list_offset.is_none());

    use crate::name_input::runtime_test_machine::execute;
    use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address};
    let address = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let hud = address(&program.report.nickname_hud_store_address);
    let origin = hud - 8;
    let mut a = Assembler::new();
    a.emit(J {
        target: address(&program.report.glyph_materializer_address),
    })
    .emit(psx_r3000a::Instruction::nop());
    a.emit_all(load_address(R::T0, 0x801ff000))
        .emit(Sw {
            rt: R::A0,
            base: R::T0,
            offset: 0,
        })
        .emit(Sw {
            rt: R::A1,
            base: R::T0,
            offset: 4,
        })
        .emit(Sw {
            rt: R::A2,
            base: R::T0,
            offset: 8,
        })
        .emit(Jr { rs: R::RA })
        .emit(Ori {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 1,
        });
    let mut code = a.assemble(origin).unwrap().bytes().to_vec();
    assert!(code.len() <= (SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - origin) as usize);
    code.resize((SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - origin) as usize, 0);
    code.extend_from_slice(&outline.bytes);
    code.resize((NAME_DIALOGUE_RUNTIME_ORIGIN - origin) as usize, 0);
    code.extend_from_slice(&program.bytes);
    let mut payload = pack.bytes().to_vec();
    payload.resize(NAME_GLYPH_PACK_STORAGE_BYTES, 0);
    let cache_start = layout.cache_codes_for_field(NameField::FamilyName).unwrap()[0] as usize;
    let atlas_pointer =
        (address(&program.report.current_atlas_pointer_address) & 0x1fff_ffff) as usize;
    let reference = reference("가각쐐힝", 12.0);
    // Each run supplies a different current atlas. A hard-coded previous pointer
    // would read sentinel bytes or modify the wrong cache.
    for atlas_base in [0x140000usize, 0x180000] {
        let mut baseline = vec![0x55; 0x200000];
        baseline[atlas_pointer..atlas_pointer + 4]
            .copy_from_slice(&(0x80000000u32 + atlas_base as u32).to_le_bytes());
        for cell in &layout.pack_cells {
            let start = atlas_base + cell.code as usize * 200;
            baseline[start..start + 200].copy_from_slice(
                &payload[cell.pack_cell_index * 200..cell.pack_cell_index * 200 + 200],
            );
        }
        for slot in 0..=16 {
            for c in ['가', '각', '쐐', '힝', '힣'] {
                let supported = slot < 16 && pack.supports(c);
                let mut memory = baseline.clone();
                let mut expected = baseline.clone();
                if supported {
                    let base = atlas_base + (cache_start + slot) * 200;
                    let glyph = reference.glyphs.iter().find(|g| g.character == c).unwrap();
                    for pixel in 0..200 {
                        expected[base + pixel] =
                            glyph.pixels[pixel * 2] | glyph.pixels[pixel * 2 + 1] << 4;
                    }
                    expected[0x1ff000..0x1ff004].copy_from_slice(&(slot as u32).to_le_bytes());
                    expected[0x1ff004..0x1ff008]
                        .copy_from_slice(&(0x80000000u32 + base as u32).to_le_bytes());
                    expected[0x1ff008..0x1ff00c].copy_from_slice(&10u32.to_le_bytes());
                }
                let mut r = std::array::from_fn(|i| i as u32 * 13);
                r[0] = 0;
                r[4] = c as u32 - 0xac00;
                r[5] = slot as u32;
                r[29] = 0x801fe000;
                r[31] = 0x800f0000;
                let before = r;
                execute(&code, origin, &mut r, &mut memory);
                assert_eq!(r[2], u32::from(supported));
                for reg in (16..24).chain([28, 29, 30, 31]) {
                    assert_eq!(r[reg], before[reg]);
                }
                memory[0x1fdfa0..0x1fe000].copy_from_slice(&expected[0x1fdfa0..0x1fe000]);
                assert!(
                    memory == expected,
                    "dialogue cache or neighboring memory mismatch: {c} slot {slot} atlas {atlas_base:x}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn selected_band_validation_matches_repertoire_without_mutating_names_or_pixels() {
    use crate::name_input::runtime_test_machine::execute;
    use crate::name_input::*;
    use psx_r3000a::{Assembler, Instruction};
    let keyboard = load_name_input_keyboard(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let cells = (0..228)
        .map(|i| crate::tim::Cell {
            x: i / 84 * 256 + i % 84 % 12 * 20,
            y: i % 84 / 12 * 20,
            width: 20,
            height: 20,
        })
        .collect::<Vec<_>>();
    let atlas = plan_name_input_runtime_atlas(&cells, &keyboard).unwrap();
    let repertoire: String = load_ks_x_1001_hangul().unwrap().into_iter().collect();
    let pack =
        NameGlyphBandPack::build(&reference(&repertoire, 12.0), NAME_GLYPH_PACK_STORAGE_BYTES)
            .unwrap();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let runtime = build_name_input_band_runtime_program(
        &atlas,
        &pack,
        &outline,
        NICKNAME_HUD_GLYPH_STORE_ORIGIN,
    )
    .unwrap();
    let redisplay = build_name_input_band_redisplay_runtime_program(
        &atlas,
        runtime.glyph_fill_materializer_address.unwrap(),
        &pack,
    )
    .unwrap();
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &redisplay.instructions,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
        "band name admission",
    )
    .unwrap();
    let targets = redisplay
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::Jal { target }
                if *target >= redisplay.selected_key_handler_address
                    && *target < redisplay.compound_final_backspace_address =>
            {
                Some(*target)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        targets.len(),
        1,
        "selected key paths must use the same admission validator"
    );
    let origin = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN - 8;
    let mut a = Assembler::new();
    a.emit(Instruction::J {
        target: *targets.first().unwrap(),
    })
    .emit(Instruction::nop());
    let mut code = a.assemble(origin).unwrap().bytes().to_vec();
    code.extend_from_slice(&redisplay.bytes);
    code.resize((NAME_INPUT_RUNTIME_ORIGIN - origin) as usize, 0);
    code.extend_from_slice(&runtime.bytes);
    let mut payload = pack.bytes().to_vec();
    payload.resize(18800, 0);
    payload.extend_from_slice(&atlas.lookup_table_bytes);
    payload.resize(NAME_GLYPH_PACK_STORAGE_BYTES, 0);
    let mut memory = vec![0x55; 0x200000];
    for (i, cell) in payload.as_chunks::<200>().0.iter().enumerate() {
        let base = 0xe92e0 + atlas.pack_storage_cell_base_byte_offsets[i] as usize;
        for row in 0..20 {
            memory[base + row * 384..base + row * 384 + 10]
                .copy_from_slice(&cell[row * 10..row * 10 + 10]);
        }
    }
    let expected = memory.clone();
    for syllable in (0..=11172u32).chain([65535, u32::MAX]) {
        let mut r = std::array::from_fn(|i| i as u32 * 13);
        r[0] = 0;
        r[4] = syllable;
        r[29] = 0x801fe000;
        r[31] = 0x800f0000;
        let before = r;
        execute(&code, origin, &mut r, &mut memory);
        let supported = syllable
            .checked_add(0xac00)
            .and_then(char::from_u32)
            .is_some_and(|c| pack.supports(c));
        assert_eq!(
            r[2],
            if supported { 0 } else { 65535 },
            "syllable {syllable:x}"
        );
        for reg in [
            9, 11, 12, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 28, 29, 30,
        ] {
            assert_eq!(r[reg], before[reg]);
        }
        assert_eq!(
            r[31], before[25],
            "restore outer selected-key return address"
        );
    }
    memory[0x1fdfe0..0x1fe000].copy_from_slice(&expected[0x1fdfe0..0x1fe000]);
    assert!(
        memory == expected,
        "admission modified name records or pixels"
    );
}
