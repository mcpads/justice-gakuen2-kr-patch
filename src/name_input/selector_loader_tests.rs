use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

fn fixture() -> (SelectorNameLoaderLayout, NameInputRuntimeAtlasLayout) {
    let offsets: Vec<_> = (0..NAME_GLYPH_PACK_CELL_COUNT)
        .map(|i| ((i / 19) * 20 * 384 + (i % 19) * 10) as u16)
        .collect();
    // Probe addresses are not an admitted product allocation.
    let layout = SelectorNameLoaderLayout {
        origin: 0x800a8000,
        byte_capacity: 1216,
        native_loader_address: 0x80015414,
        font: NameAssetLoad {
            catalog_index: 718,
            destination: 0x800c0000,
            decoded_byte_count: 210944,
        },
        textures: NameAssetLoad {
            catalog_index: 53,
            destination: 0x800d4000,
            decoded_byte_count: 328192,
        },
        font_pixel_address: 0x800d92e0,
        pack_destination: 0x800b0000,
        font_copies: Vec::new(),
        restore_selector_service_table: false,
        restored_zero_ranges: Vec::new(),
    };
    let atlas = NameInputRuntimeAtlasLayout {
        font_atlas_row_bytes: 384,
        glyph_cell_width: 20,
        glyph_cell_height: 20,
        lookup_table_bytes: offsets.iter().flat_map(|v| v.to_le_bytes()).collect(),
        pack_storage_cell_base_byte_offsets: offsets,
        cache_cell_base_byte_offsets: Vec::new(),
    };
    (layout, atlas)
}

#[test]
fn loader_preserves_the_pack_when_selector_reload_overwrites_font_pixels() {
    for restore in [false, true] {
        let (mut layout, atlas) = fixture();
        layout.restore_selector_service_table = restore;
        let p = build_selector_name_loader(&layout, &atlas).unwrap();
        let offset = |a: u32| (a & 0x1fffffff) as usize;
        let mut memory = vec![0x55; 0x200000];
        memory[offset(layout.origin)..offset(layout.origin) + p.bytes.len()]
            .copy_from_slice(&p.bytes);
        memory[0x1f6360..0x1f6364].copy_from_slice(&0x80089a7cu32.to_le_bytes());
        memory[0x89bcc..0x89bd0].copy_from_slice(&0x80015fccu32.to_le_bytes());
        let before = memory.clone();
        let mut font = vec![0xa5; layout.font.decoded_byte_count];
        let pixels = usize::try_from(layout.font_pixel_address - layout.font.destination).unwrap();
        let mut expected_pack = Vec::new();
        for (cell, &base) in atlas.pack_storage_cell_base_byte_offsets.iter().enumerate() {
            for row in 0..20 {
                for x in 0..10 {
                    let value = (cell * 37 + row * 11 + x) as u8;
                    font[pixels + usize::from(base) + row * 384 + x] = value;
                    expected_pack.push(value);
                }
            }
        }
        let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
        r[0] = 0;
        r[29] = 0x801fe000;
        r[31] = 0x800a2bc0;
        let saved = r;
        let mut calls = 0;
        execute_with_callbacks(
            &p.bytes,
            layout.origin,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, m| {
                if pc != layout.native_loader_address {
                    return false;
                }
                calls += 1;
                assert_eq!(m[0x1f64e9], 0, "motion cache invalidated before font load");
                if calls == 1 {
                    assert_eq!((r[4], r[5]), (layout.font.destination, 718));
                    m[offset(r[4])..offset(r[4]) + font.len()].copy_from_slice(&font);
                } else {
                    assert_eq!(calls, 2);
                    assert_eq!((r[4], r[5]), (layout.textures.destination, 53));
                    // Model the native selector resource overwriting the transient font.
                    m[offset(r[4])..offset(r[4]) + layout.textures.decoded_byte_count].fill(0x77);
                }
                for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                    r[reg] = 0xdead0000 + reg as u32;
                }
                let sp = offset(r[29]);
                m[sp..sp + 16].fill(0xcc);
                r[2] = if calls == 1 { 109762 } else { 221538 };
                true
            },
        );
        assert_eq!(calls, 2);
        assert_eq!(r[2], if restore { 0x80015fcc } else { 221538 });
        if restore {
            assert_eq!(r[4], 0x800d0000);
        }
        for reg in 16..24 {
            assert_eq!(r[reg], saved[reg]);
        }
        for reg in [28, 29, 30, 31] {
            assert_eq!(r[reg], saved[reg]);
        }
        assert_eq!(
            &memory[offset(layout.pack_destination)
                ..offset(layout.pack_destination) + NAME_GLYPH_PACK_STORAGE_BYTES],
            expected_pack
        );
        let mut expected = before;
        expected[0x1f64e9] = 0;
        expected[offset(layout.font.destination)..offset(layout.font.destination) + font.len()]
            .copy_from_slice(&font);
        expected[offset(layout.textures.destination)
            ..offset(layout.textures.destination) + layout.textures.decoded_byte_count]
            .fill(0x77);
        expected[offset(layout.pack_destination)
            ..offset(layout.pack_destination) + expected_pack.len()]
            .copy_from_slice(&expected_pack);
        let stack = offset(saved[29]);
        expected[stack - 40..stack].copy_from_slice(&memory[stack - 40..stack]);
        assert_eq!(
            memory, expected,
            "loader changed memory outside its loads, copied pack, cache flag and owned stack"
        );
    }
}

#[test]
fn rejects_loader_layouts_that_destroy_code_pack_or_source_cells() {
    let (layout, atlas) = fixture();
    for bad in [
        SelectorNameLoaderLayout {
            pack_destination: layout.font.destination,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            origin: layout.textures.destination,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            native_loader_address: layout.font.destination,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            byte_capacity: usize::MAX,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            byte_capacity: 32,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            font_pixel_address: layout.font_pixel_address + 1,
            ..layout.clone()
        },
        SelectorNameLoaderLayout {
            font_pixel_address: 0x801ffffe,
            ..layout.clone()
        },
    ] {
        assert!(build_selector_name_loader(&bad, &atlas).is_err());
    }
    let mut bad = atlas.clone();
    bad.pack_storage_cell_base_byte_offsets[0] |= 1;
    assert!(build_selector_name_loader(&layout, &bad).is_err());
    let mut bad = atlas.clone();
    bad.pack_storage_cell_base_byte_offsets[1] = bad.pack_storage_cell_base_byte_offsets[0] + 2;
    bad.lookup_table_bytes = bad
        .pack_storage_cell_base_byte_offsets
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert!(build_selector_name_loader(&layout, &bad).is_err());
    let mut bad = atlas;
    bad.lookup_table_bytes[0] ^= 1;
    assert!(build_selector_name_loader(&layout, &bad).is_err());
}

#[test]
fn contiguous_font_suppliers_survive_destructive_texture_reload() {
    let (mut layout, atlas) = fixture();
    layout.font.decoded_byte_count = 229168;
    layout.font_copies = vec![
        SelectorDataCopy {
            source: 0x800f3800,
            destination: layout.pack_destination,
            byte_count: 16940,
        },
        SelectorDataCopy {
            source: 0x800f7a2c,
            destination: 0x800b4c00,
            byte_count: 1284,
        },
    ];
    let program = build_selector_name_loader(&layout, &atlas).unwrap();
    assert_eq!(program.bytes.len(), program.code_byte_count);
    let off = |address: u32| (address & 0x1fffffff) as usize;
    let mut memory = vec![0x55; 0x200000];
    memory[off(layout.origin)..off(layout.origin) + program.bytes.len()]
        .copy_from_slice(&program.bytes);
    let mut expected = memory.clone();
    expected[0x1f64e9] = 0;
    let font: Vec<u8> = (0..layout.font.decoded_byte_count)
        .map(|i| (i * 37 + i / 251) as u8)
        .collect();
    expected[off(layout.font.destination)..off(layout.font.destination) + font.len()]
        .copy_from_slice(&font);
    expected[off(layout.textures.destination)
        ..off(layout.textures.destination) + layout.textures.decoded_byte_count]
        .fill(0x77);
    for copy in &layout.font_copies {
        let source = (copy.source - layout.font.destination) as usize;
        expected[off(copy.destination)..off(copy.destination) + copy.byte_count]
            .copy_from_slice(&font[source..source + copy.byte_count]);
    }
    let mut registers = std::array::from_fn(|i| 0x12340000 + i as u32);
    registers[0] = 0;
    registers[29] = 0x801fe000;
    registers[31] = 0x800a2bc0;
    let saved = registers;
    let mut calls = 0;
    execute_with_callbacks(
        &program.bytes,
        layout.origin,
        &mut registers,
        &mut memory,
        None,
        &mut |pc, r, m| {
            if pc != layout.native_loader_address {
                return false;
            }
            calls += 1;
            if calls == 1 {
                assert_eq!((r[4], r[5]), (layout.font.destination, 718));
                m[off(r[4])..off(r[4]) + font.len()].copy_from_slice(&font);
            } else {
                assert_eq!(calls, 2);
                assert_eq!((r[4], r[5]), (layout.textures.destination, 53));
                m[off(r[4])..off(r[4]) + layout.textures.decoded_byte_count].fill(0x77);
            }
            for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                r[reg] = 0xdead0000 + reg as u32;
            }
            true
        },
    );
    assert_eq!(calls, 2);
    for reg in (16..24).chain([28, 29, 30, 31]) {
        assert_eq!(registers[reg], saved[reg]);
    }
    let stack = off(saved[29]);
    expected[stack - 40..stack].copy_from_slice(&memory[stack - 40..stack]);
    assert_eq!(memory, expected, "font transport changed unowned memory");
}

#[test]
fn contiguous_font_transport_rejects_invalid_ownership() {
    let (mut layout, atlas) = fixture();
    layout.font.decoded_byte_count = 229168;
    let pack = SelectorDataCopy {
        source: 0x800f3800,
        destination: layout.pack_destination,
        byte_count: 16940,
    };
    let ascii = SelectorDataCopy {
        source: 0x800f7a2c,
        destination: 0x800b4c00,
        byte_count: 1284,
    };
    for copies in [
        vec![SelectorDataCopy {
            byte_count: 0,
            ..pack.clone()
        }],
        vec![SelectorDataCopy {
            source: pack.source + 1,
            ..pack.clone()
        }],
        vec![SelectorDataCopy {
            source: 0x80100000,
            ..pack.clone()
        }],
        vec![SelectorDataCopy {
            destination: layout.pack_destination + 4,
            ..pack.clone()
        }],
        vec![SelectorDataCopy {
            byte_count: 19004,
            ..pack.clone()
        }],
        vec![
            pack.clone(),
            SelectorDataCopy {
                source: pack.source,
                ..ascii.clone()
            },
        ],
        vec![
            pack.clone(),
            SelectorDataCopy {
                destination: layout.pack_destination + 18000,
                ..ascii.clone()
            },
        ],
        vec![
            pack.clone(),
            SelectorDataCopy {
                destination: layout.origin,
                ..ascii.clone()
            },
        ],
        vec![
            pack.clone(),
            SelectorDataCopy {
                destination: layout.textures.destination,
                ..ascii.clone()
            },
        ],
        vec![pack.clone(), ascii.clone(), ascii],
    ] {
        layout.font_copies = copies;
        assert!(build_selector_name_loader(&layout, &atlas).is_err());
    }
}
