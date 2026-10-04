//! Upload one selector scratch glyph through the game's native GPU services.
use super::selector_loader::{overlaps, ram_range};
use crate::contextual_texture_upload::{
    VRAM_UPLOAD_ROUTINE_ADDRESS, VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorUploadLayout {
    pub origin: u32,
    pub byte_capacity: usize,
    pub scratch_address: u32,
    pub x_words: u16,
    pub y: u16,
    pub slot_count: u16,
}

/// A0 selects a 20x20 cell in a 4-bit texture-page cache. Invalid slots
/// return -1 without calls or writes. Otherwise return LoadImage's result after
/// DrawSync(0), retaining the scratch buffer until queued transfers complete.
/// VRAM ownership and palette/packet construction are separate installer duties.
pub fn build_selector_upload(layout: &SelectorUploadLayout) -> Result<Vec<u8>> {
    let code = ram_range(layout.origin, layout.byte_capacity)?;
    let scratch = ram_range(layout.scratch_address, 200)?;
    ensure!(
        layout.origin.is_multiple_of(4) && layout.scratch_address.is_multiple_of(4),
        "unaligned selector upload code or scratch"
    );
    ensure!(
        layout.slot_count > 0
            && layout.slot_count <= 144
            && layout.x_words.is_multiple_of(64)
            && u32::from(layout.x_words) + u32::from(layout.slot_count.min(12)) * 5 <= 1024
            && layout.y < 512
            && layout.y % 256 + layout.slot_count.div_ceil(12) * 20 <= 256,
        "selector cells leave VRAM or cross their texture page"
    );
    let upload = ram_range(VRAM_UPLOAD_ROUTINE_ADDRESS, 4)?;
    let sync = ram_range(VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS, 4)?;
    ensure!(
        !overlaps(&code, &scratch)
            && [&upload, &sync]
                .iter()
                .all(|r| !overlaps(r, &code) && !overlaps(r, &scratch)),
        "selector upload overwrites a native service or scratch"
    );
    let mut a = Assembler::new();
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A0,
        immediate: layout.slot_count as i16,
    })
    .bne(R::T0, R::ZERO, "upload_selector_cell")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::V0,
        rs: R::ZERO,
        immediate: -1,
    })
    .emit(Jr { rs: R::RA })
    .emit(psx_r3000a::Instruction::nop())
    .label("upload_selector_cell");
    emit_selector_cache_row(&mut a, layout.slot_count);
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -32,
    })
    .emit(Sw {
        rt: R::RA,
        base: R::SP,
        offset: 28,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::A0,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::A0,
    })
    .emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: layout.x_words as i16,
    })
    .emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 16,
    })
    .emit(Ori {
        rt: R::T0,
        rs: R::ZERO,
        immediate: layout.y,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T3,
    })
    .emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 18,
    })
    .emit_all(load_address(R::T0, 0x00140005))
    .emit(Sw {
        rt: R::T0,
        base: R::SP,
        offset: 20,
    })
    .emit_all(load_address(R::A1, layout.scratch_address))
    .emit(Jal {
        target: VRAM_UPLOAD_ROUTINE_ADDRESS,
    })
    .emit(Addiu {
        rt: R::A0,
        rs: R::SP,
        immediate: 16,
    })
    .emit(Sw {
        rt: R::V0,
        base: R::SP,
        offset: 24,
    })
    .emit(Jal {
        target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
    })
    .emit(Addu {
        rd: R::A0,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .emit(Lw {
        rt: R::V0,
        base: R::SP,
        offset: 24,
    })
    .emit(Lw {
        rt: R::RA,
        base: R::SP,
        offset: 28,
    })
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 32,
    })
    .emit(Jr { rs: R::RA })
    .emit(psx_r3000a::Instruction::nop());
    let bytes = a.assemble(layout.origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= layout.byte_capacity,
        "selector uploader exceeds code capacity"
    );
    let instructions = verify_placed_program(&bytes, layout.origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        layout.origin,
        "selector glyph upload",
    )?;
    Ok(bytes)
}

// A0 becomes the column (0..11); T3 is the row pixel offset.
// Keep the existing two-row program byte-identical for installed consumers.
// The enclosing routine validates the complete cache against texture-page bounds.
pub(super) fn emit_selector_cache_row(a: &mut Assembler, slots: u16) {
    if slots > 24 {
        a.emit(Addu {
            rd: R::T3,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .label("selector_cache_row_loop")
        .emit(Sltiu {
            rt: R::T4,
            rs: R::A0,
            immediate: 12,
        })
        .bne(R::T4, R::ZERO, "selector_cache_row_done")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Addiu {
            rt: R::A0,
            rs: R::A0,
            immediate: -12,
        })
        .beq(R::ZERO, R::ZERO, "selector_cache_row_loop")
        .emit(Addiu {
            rt: R::T3,
            rs: R::T3,
            immediate: 20,
        })
        .label("selector_cache_row_done");
        return;
    }
    a.emit(Addu {
        rd: R::T3,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .emit(Sltiu {
        rt: R::T4,
        rs: R::A0,
        immediate: 12,
    })
    .bne(R::T4, R::ZERO, "selector_cache_first_row")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: -12,
    })
    .emit(Addiu {
        rt: R::T3,
        rs: R::ZERO,
        immediate: 20,
    })
    .label("selector_cache_first_row");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute_with_callbacks;
    #[test]
    fn uploads_each_complete_cell_and_waits_before_reusing_scratch() {
        let layout = SelectorUploadLayout {
            origin: 0x800a8800,
            byte_capacity: 256,
            scratch_address: 0x800b4a38,
            x_words: 512,
            y: 0,
            slot_count: 16,
        };
        let off = |a: u32| (a & 0x1fffffff) as usize;
        for count in [12u16, 16, 24, 68, 144] {
            let layout = SelectorUploadLayout {
                slot_count: count,
                ..layout.clone()
            };
            let program = build_selector_upload(&layout).unwrap();
            for slot in (0..=u32::from(count)).chain([u32::MAX]) {
                for result in [0, 1, u32::MAX] {
                    let mut memory = vec![0x55; 0x200000];
                    let before = memory.clone();
                    let mut r = std::array::from_fn(|i| i as u32 * 37);
                    r[0] = 0;
                    r[4] = slot;
                    r[29] = 0x801fe000;
                    r[31] = 0x800a2bb0;
                    let saved = r;
                    let mut calls = Vec::new();
                    let mut pending = None;
                    execute_with_callbacks(
                        &program,
                        layout.origin,
                        &mut r,
                        &mut memory,
                        None,
                        &mut |pc, r, m| {
                            if ![
                                VRAM_UPLOAD_ROUTINE_ADDRESS,
                                VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
                            ]
                            .contains(&pc)
                            {
                                return false;
                            }
                            calls.push(pc);
                            if pc == VRAM_UPLOAD_ROUTINE_ADDRESS {
                                let rect = off(r[4]);
                                let values: Vec<_> = m[rect..rect + 8]
                                    .as_chunks::<2>()
                                    .0
                                    .iter()
                                    .map(|v| u16::from_le_bytes(*v))
                                    .collect();
                                assert_eq!(
                                    values,
                                    vec![
                                        512 + (slot % 12) as u16 * 5,
                                        (slot / 12) as u16 * 20,
                                        5,
                                        20
                                    ]
                                );
                                assert_eq!(r[5], layout.scratch_address);
                                pending = Some(m[off(r[5])..off(r[5]) + 200].to_vec());
                            } else {
                                assert_eq!(r[4], 0);
                                assert_eq!(
                                    &m[off(layout.scratch_address)
                                        ..off(layout.scratch_address) + 200],
                                    pending.take().unwrap()
                                );
                            }
                            for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                                r[reg] = 0xdead0000 + reg as u32;
                            }
                            let sp = off(r[29]);
                            m[sp..sp + 16].fill(0xcc);
                            r[2] = if pc == VRAM_UPLOAD_ROUTINE_ADDRESS {
                                result
                            } else {
                                0xdead
                            };
                            true
                        },
                    );
                    assert_eq!(
                        r[2],
                        if slot < u32::from(count) {
                            result
                        } else {
                            u32::MAX
                        }
                    );
                    assert!(pending.is_none());
                    assert_eq!(
                        calls,
                        if slot < u32::from(count) {
                            vec![
                                VRAM_UPLOAD_ROUTINE_ADDRESS,
                                VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
                            ]
                        } else {
                            vec![]
                        }
                    );
                    for i in (16..24).chain([28, 29, 30, 31]) {
                        assert_eq!(r[i], saved[i]);
                    }
                    let mut expected = before;
                    if slot < u32::from(count) {
                        let sp = off(saved[29]);
                        expected[sp - 32..sp].copy_from_slice(&memory[sp - 32..sp]);
                    }
                    assert_eq!(memory, expected);
                }
            }
        }
        for bad in [
            SelectorUploadLayout {
                slot_count: 0,
                ..layout.clone()
            },
            SelectorUploadLayout {
                slot_count: 145,
                ..layout.clone()
            },
            SelectorUploadLayout {
                x_words: 513,
                ..layout.clone()
            },
            SelectorUploadLayout {
                x_words: 1024,
                ..layout.clone()
            },
            SelectorUploadLayout {
                y: 250,
                ..layout.clone()
            },
            SelectorUploadLayout {
                y: 512,
                ..layout.clone()
            },
            SelectorUploadLayout {
                scratch_address: layout.origin,
                ..layout.clone()
            },
            SelectorUploadLayout {
                scratch_address: VRAM_UPLOAD_ROUTINE_ADDRESS,
                ..layout.clone()
            },
            SelectorUploadLayout {
                byte_capacity: 4,
                ..layout
            },
        ] {
            assert!(build_selector_upload(&bad).is_err());
        }
    }
}
