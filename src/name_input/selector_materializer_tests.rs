use super::*;
use crate::font::rasterize_menu_glyphs;
use crate::name_input::{build_shared_name_outline_runtime_program, runtime_test_machine::execute};
use psx_r3000a::Instruction;

fn layout() -> SelectorMaterializerLayout {
    SelectorMaterializerLayout {
        origin: 0x800a9000,
        byte_capacity: 2016,
        pack_address: 0x800b0000,
        scratch_address: 0x800b4a38,
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn selector_materializer_restores_complete_glyph_and_preserves_neighbors() {
    let font =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
    let reference =
        rasterize_menu_glyphs(&font, "가각값깎꼭똠뷁쀍잠체력방어근성힣", 12.0, 3, 13).unwrap();
    let pack = NameGlyphBandPack::build(&reference, 19000).unwrap();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let layout = layout();
    let p = build_selector_materializer(&layout, &pack, &outline).unwrap();
    let origin = 0x80010000;
    let mut code = vec![0; (layout.origin - origin) as usize + p.bytes.len()];
    let mut entry = Assembler::new();
    entry
        .emit(Instruction::J {
            target: p.entry_address,
        })
        .emit(Instruction::nop());
    code[..8].copy_from_slice(entry.assemble(origin).unwrap().bytes());
    let at = (outline.outline_pixel_address - origin) as usize;
    code[at..at + outline.bytes.len()].copy_from_slice(&outline.bytes);
    code[(layout.origin - origin) as usize..].copy_from_slice(&p.bytes);
    let off = |address: u32| (address & 0x1fffffff) as usize;
    let mut memory = vec![0x55; 0x200000];
    let data = off(layout.pack_address);
    memory[data..data + pack.bytes().len()].copy_from_slice(pack.bytes());
    for glyph in &reference.glyphs {
        let before = memory.clone();
        let mut r = std::array::from_fn(|i| i as u32 * 37);
        r[0] = 0;
        r[4] = glyph.character as u32 - 0xac00;
        r[29] = 0x801fe000;
        r[31] = 0x800f0000;
        let saved = r;
        execute(&code, origin, &mut r, &mut memory);
        assert_eq!(r[2], layout.scratch_address);
        for reg in (16..24).chain([28, 29, 30, 31]) {
            assert_eq!(r[reg], saved[reg]);
        }
        let pixels: Vec<_> = glyph
            .pixels
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| p[0] | p[1] << 4)
            .collect();
        let scratch = off(layout.scratch_address);
        assert_eq!(
            &memory[scratch..scratch + 200],
            pixels,
            "{}",
            glyph.character
        );
        let [x, y, w, h] = p.glyph_rectangle;
        for row in 0..20 {
            for col in 0..20 {
                if col < x || col >= x + w || row < y || row >= y + h {
                    assert_eq!(glyph.pixels[row * 20 + col], 0);
                }
            }
        }
        let mut expected = before;
        expected[scratch..scratch + 200].copy_from_slice(&pixels);
        let stack = off(saved[29]);
        expected[stack - 64..stack].copy_from_slice(&memory[stack - 64..stack]);
        assert_eq!(memory, expected);
    }
    for invalid in [0xffffffff, 0x2ba4, ('나' as u32) - 0xac00] {
        let before = memory.clone();
        let mut r = [0; 32];
        r[4] = invalid;
        r[29] = 0x801fe000;
        r[31] = 0x800f0000;
        execute(&code, origin, &mut r, &mut memory);
        assert_eq!(r[2], 0);
        let stack = off(r[29]);
        let mut expected = before;
        expected[stack - 64..stack].copy_from_slice(&memory[stack - 64..stack]);
        assert_eq!(
            memory, expected,
            "unsupported code modified pixels or other memory"
        );
    }
    for bad in [
        SelectorMaterializerLayout {
            scratch_address: layout.pack_address,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            pack_address: layout.origin,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            origin: outline.outline_pixel_address,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            scratch_address: 0x801ffffc,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            byte_capacity: 4,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            origin: layout.origin + 1,
            ..layout.clone()
        },
        SelectorMaterializerLayout {
            byte_capacity: usize::MAX,
            ..layout
        },
    ] {
        assert!(build_selector_materializer(&bad, &pack, &outline).is_err());
    }
}
