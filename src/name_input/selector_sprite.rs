//! Emit the selector's native six-word draw-mode/sprite packet shape.
use super::selector_loader::ram_range;
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorSpriteLayout {
    pub origin: u32,
    pub byte_capacity: usize,
    pub x_words: u16,
    pub y: u16,
    pub slot_count: u16,
    pub glyph_rectangle: [usize; 4],
    pub clut_x_words: u16,
    pub clut_y: u16,
}

/// A0=cache slot, A1=28-byte packet, A2=packed signed screen XY, A3=OT entry.
/// Returns the packet pointer or zero for an invalid slot without writes. The
/// caller owns aligned, disjoint packet/OT buffers. OT linking retains its high
/// byte as native AddPrim does. Pixel data must already be uploaded.
pub fn build_selector_sprite(layout: &SelectorSpriteLayout) -> Result<Vec<u8>> {
    ram_range(layout.origin, layout.byte_capacity)?;
    let [x, y, w, h] = layout.glyph_rectangle;
    ensure!(
        x < 20 && y < 20 && w > 0 && h > 0 && w <= 20 - x && h <= 20 - y,
        "selector sprite crop leaves its complete cell"
    );
    ensure!(
        layout.origin.is_multiple_of(4)
            && layout.x_words.is_multiple_of(64)
            && layout.slot_count > 0
            && layout.slot_count <= 144
            && u32::from(layout.x_words) + u32::from(layout.slot_count.min(12)) * 5 <= 1024
            && layout.y < 512
            && layout.y % 256 + layout.slot_count.div_ceil(12) * 20 <= 256
            && layout.clut_x_words.is_multiple_of(16)
            && layout.clut_x_words <= 1008
            && layout.clut_y < 512,
        "selector sprite has invalid code, texture-page or palette geometry"
    );
    let tpage = u32::from(layout.x_words / 64) | u32::from(layout.y / 256) << 4 | 0x200;
    let clut = u32::from(layout.clut_x_words / 16) | u32::from(layout.clut_y) << 6;
    let uv_high = (u32::from(layout.y % 256) + y as u32) << 8 | clut << 16;
    let mut a = Assembler::new();
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A0,
        immediate: layout.slot_count as i16,
    })
    .bne(R::T0, R::ZERO, "write_selector_sprite")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addu {
        rd: R::V0,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .emit(Jr { rs: R::RA })
    .emit(psx_r3000a::Instruction::nop())
    .label("write_selector_sprite");
    super::selector_upload::emit_selector_cache_row(&mut a, layout.slot_count);
    a.emit(Lw {
        rt: R::T0,
        base: R::A3,
        offset: 0,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::A0,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::A0,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T1,
        shift: 2,
    })
    .emit(Addiu {
        rt: R::T1,
        rs: R::T1,
        immediate: x as i16,
    })
    .emit(Sll {
        rd: R::T3,
        rt: R::T3,
        shift: 8,
    })
    .emit_all(load_address(R::T2, uv_high))
    .emit(Addu {
        rd: R::T2,
        rs: R::T2,
        rt: R::T3,
    })
    .emit(Or {
        rd: R::T1,
        rs: R::T1,
        rt: R::T2,
    })
    .emit(Sw {
        rt: R::T1,
        base: R::A1,
        offset: 20,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T0,
        shift: 8,
    })
    .emit(Srl {
        rd: R::T1,
        rt: R::T1,
        shift: 8,
    })
    .emit(Lui {
        rt: R::T2,
        immediate: 0x0600,
    })
    .emit(Or {
        rd: R::T1,
        rs: R::T1,
        rt: R::T2,
    })
    .emit(Sw {
        rt: R::T1,
        base: R::A1,
        offset: 0,
    })
    .emit_all(load_address(R::T1, 0xe1000000 | tpage))
    .emit(Sw {
        rt: R::T1,
        base: R::A1,
        offset: 4,
    })
    .emit(Sw {
        rt: R::ZERO,
        base: R::A1,
        offset: 8,
    })
    .emit_all(load_address(R::T1, 0x64808080))
    .emit(Sw {
        rt: R::T1,
        base: R::A1,
        offset: 12,
    })
    .emit(Sw {
        rt: R::A2,
        base: R::A1,
        offset: 16,
    })
    .emit_all(load_address(R::T1, (h as u32) << 16 | w as u32))
    .emit(Sw {
        rt: R::T1,
        base: R::A1,
        offset: 24,
    })
    .emit(Srl {
        rd: R::T0,
        rt: R::T0,
        shift: 24,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 24,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::A1,
        shift: 8,
    })
    .emit(Srl {
        rd: R::T1,
        rt: R::T1,
        shift: 8,
    })
    .emit(Or {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Sw {
        rt: R::T0,
        base: R::A3,
        offset: 0,
    })
    .emit(Jr { rs: R::RA })
    .emit(Addu {
        rd: R::V0,
        rs: R::A1,
        rt: R::ZERO,
    });
    let bytes = a.assemble(layout.origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= layout.byte_capacity,
        "selector sprite exceeds code capacity"
    );
    let instructions = verify_placed_program(&bytes, layout.origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        layout.origin,
        "selector sprite",
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute;
    #[test]
    fn full_width_glyph_packets_link_without_damaging_other_slots() {
        let layout = SelectorSpriteLayout {
            origin: 0x800a8800,
            byte_capacity: 256,
            x_words: 512,
            y: 0,
            slot_count: 16,
            glyph_rectangle: [3, 3, 13, 13],
            clut_x_words: 0,
            clut_y: 482,
        };
        for count in [12u16, 16, 24, 68, 144] {
            let layout = SelectorSpriteLayout {
                slot_count: count,
                ..layout.clone()
            };
            let bytes = build_selector_sprite(&layout).unwrap();
            for slot in (0..=u32::from(count)).chain([u32::MAX]) {
                let mut memory = vec![0x55; 0x4000];
                memory[0x100..0x104].copy_from_slice(&0x7affffffu32.to_le_bytes());
                let before = memory.clone();
                let mut r = std::array::from_fn(|i| i as u32 * 37);
                r[0] = 0;
                r[4] = slot;
                r[5] = 0x80000200;
                r[6] = 0x0049fffc;
                r[7] = 0x80000100;
                r[29] = 0x80003000;
                r[31] = 0x800a2bb0;
                let saved = r;
                execute(&bytes, layout.origin, &mut r, &mut memory);
                let mut expected = before;
                if slot < u32::from(count) {
                    let words = [
                        0x06ffffff,
                        0xe1000208,
                        0,
                        0x64808080,
                        0x0049fffc,
                        0x78800300 + (slot % 12) * 20 + 3 + (slot / 12) * 20 * 256,
                        0x000d000d,
                    ];
                    for (i, w) in words.iter().enumerate() {
                        expected[0x200 + i * 4..0x204 + i * 4].copy_from_slice(&w.to_le_bytes());
                    }
                    expected[0x100..0x104].copy_from_slice(&0x7a000200u32.to_le_bytes());
                    assert_eq!(r[2], saved[5]);
                } else {
                    assert_eq!(r[2], 0);
                }
                assert_eq!(memory, expected);
                for reg in (16..24).chain([28, 29, 30, 31]) {
                    assert_eq!(r[reg], saved[reg]);
                }
                if slot < u32::from(count) {
                    r[4] = 11;
                    r[5] = 0x80000238;
                    execute(&bytes, layout.origin, &mut r, &mut memory);
                    let read = |off| u32::from_le_bytes(memory[off..off + 4].try_into().unwrap());
                    assert_eq!(read(0x100), 0x7a000238);
                    assert_eq!(read(0x238), 0x06000200);
                    assert_eq!(read(0x200), 0x06ffffff);
                }
            }
        }
        for bad in [
            SelectorSpriteLayout {
                glyph_rectangle: [usize::MAX, 3, 13, 13],
                ..layout.clone()
            },
            SelectorSpriteLayout {
                glyph_rectangle: [3, 3, 18, 13],
                ..layout.clone()
            },
            SelectorSpriteLayout {
                glyph_rectangle: [3, 3, 13, 0],
                ..layout.clone()
            },
            SelectorSpriteLayout {
                clut_x_words: 1009,
                ..layout.clone()
            },
            SelectorSpriteLayout {
                clut_y: 512,
                ..layout.clone()
            },
            SelectorSpriteLayout {
                slot_count: 145,
                ..layout.clone()
            },
            SelectorSpriteLayout {
                x_words: 513,
                ..layout.clone()
            },
            SelectorSpriteLayout {
                y: 250,
                ..layout.clone()
            },
            SelectorSpriteLayout {
                byte_capacity: 4,
                ..layout
            },
        ] {
            assert!(build_selector_sprite(&bad).is_err());
        }
    }
}
