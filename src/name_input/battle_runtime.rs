//! Compose the battle-phase name suppliers, using the bounded, pre-sampled legacy atlas.
use super::*;
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub const BATTLE_NAME_PACK_ADDRESS: u32 = 0x800da000;
pub const BATTLE_NAME_SCRATCH_ADDRESS: u32 = 0x800deb00;
pub const BATTLE_NAME_BUFFER_ADDRESS: u32 = 0x800dec00;
pub const BATTLE_NAME_STORED_CAPACITY: usize = 8308;

pub struct BattleNameRuntimeProgram {
    pub fits_native_record: bool,
    pub decoded: Vec<u8>,
    pub encoded: Vec<u8>,
    pub entry_address: u32,
    pub dispatch_address: u32,
    pub hangul_address: u32,
    pub ascii_address: u32,
    pub legacy_address: u32,
    pub code_ranges: Vec<[usize; 2]>,
}

/// Atlas coordinates and font bytes must be bound to the product's MA_ENT by
/// the installer. This constructs no disc write or sprite/allocation admission.
pub fn build_battle_name_runtime(
    source: &[u8],
    atlas: &NameInputRuntimeAtlasLayout,
    pack: &NameGlyphBandPack,
    ascii: &SelectorAsciiGlyphs,
    direct_glyphs: &[(u16, u8)],
) -> Result<BattleNameRuntimeProgram> {
    ensure!(
        source.len() == BATTLE_NAME_NATIVE_SUPPLIER_BYTES
            && source[..8] == [16, 0, 0, 0, 0, 0, 0, 0],
        "native battle supplier shape changed"
    );
    ensure!(
        atlas.font_atlas_row_bytes == 384
            && atlas.glyph_cell_width == 20
            && atlas.glyph_cell_height == 20
            && atlas.pack_storage_cell_base_byte_offsets.len() == NAME_GLYPH_PACK_CELL_COUNT,
        "unsupported battle font cell layout"
    );
    ensure!(
        atlas.lookup_table_bytes
            == atlas
                .pack_storage_cell_base_byte_offsets
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        "battle pack coordinates disagree"
    );
    let mut occupied = std::collections::BTreeSet::new();
    for &base in &atlas.pack_storage_cell_base_byte_offsets {
        for y in 0..20 {
            for x in 0..10 {
                let offset = usize::from(base) + y * 384 + x;
                ensure!(
                    usize::from(base) % 384 + 10 <= 384
                        && offset < 98304
                        && occupied.insert(offset),
                    "battle font cells overlap or leave MA_ENT pixels"
                );
            }
        }
    }
    ensure!(
        pack.bytes().len() <= NAME_GLYPH_PACK_STORAGE_BYTES,
        "battle name pack exceeds owned storage"
    );
    let outline = build_shared_name_outline_runtime_program()?;
    let stored_legacy = super::battle_legacy_glyph::pack_battle_legacy_glyphs(source)?;
    let entry = BATTLE_NAME_LOAD_DESTINATION + stored_legacy.len() as u32;
    let provisional = emit_entry(
        entry,
        0x80170004,
        0x80170008,
        0x8017000c,
        0x80170010,
        direct_glyphs,
    )?;
    let dispatch_address = entry + provisional.len() as u32;
    let dispatch =
        build_battle_glyph_dispatch(dispatch_address, 512, 0x80170014, 0x80170018, 0x8017001c)?;
    let hangul_origin = dispatch_address + dispatch.len() as u32;
    let hangul = build_selector_materializer(
        &SelectorMaterializerLayout {
            origin: hangul_origin,
            byte_capacity: 2048,
            pack_address: BATTLE_NAME_PACK_ADDRESS,
            scratch_address: BATTLE_NAME_SCRATCH_ADDRESS,
        },
        pack,
        &outline,
    )?;
    ensure!(
        hangul.glyph_rectangle == [2, 1, 16, 16],
        "battle Hangul crop must preserve the admitted full glyph"
    );
    let [ax, ay, aw, ah] = ascii.crop;
    ensure!(
        ax >= 3 && ay >= 2 && ax + aw <= 17 && ay + ah <= 16,
        "battle ASCII outline exceeds the complete 16x16 glyph rectangle"
    );
    let ascii_address = hangul_origin + hangul.bytes.len() as u32;
    let ascii_code = ascii.build_render_program(
        ascii_address,
        1024,
        0x80170000,
        BATTLE_NAME_SCRATCH_ADDRESS,
        &outline,
    )?;
    let legacy_address = ascii_address + ascii_code.len() as u32;
    let legacy = build_battle_legacy_glyph(
        legacy_address,
        512,
        BATTLE_NAME_LOAD_DESTINATION,
        BATTLE_NAME_SCRATCH_ADDRESS,
    )?;
    let executable_size = (legacy_address - entry) as usize + legacy.len();
    let coordinates = entry + executable_size as u32;
    let table_len = atlas.lookup_table_bytes.len().next_multiple_of(4);
    let palette_map = coordinates + table_len as u32;
    let rectangles = palette_map + 256;
    let ascii_data = rectangles + 32;
    let entry_code = emit_entry(
        entry,
        coordinates,
        dispatch_address,
        palette_map,
        rectangles,
        direct_glyphs,
    )?;
    ensure!(
        entry_code.len() == provisional.len(),
        "battle entry relocation changed length"
    );
    let dispatch_code = build_battle_glyph_dispatch(
        dispatch_address,
        dispatch.len(),
        hangul.entry_address,
        ascii_address,
        legacy_address,
    )?;
    let ascii_code = ascii.build_render_program(
        ascii_address,
        ascii_code.len(),
        ascii_data,
        BATTLE_NAME_SCRATCH_ADDRESS,
        &outline,
    )?;
    let mut decoded = stored_legacy;
    let mut code_ranges = Vec::new();
    for bytes in [
        &entry_code,
        &dispatch_code,
        &hangul.bytes,
        &ascii_code,
        &legacy,
    ] {
        let start = decoded.len();
        decoded.extend_from_slice(bytes);
        code_ranges.push([start, decoded.len()]);
    }
    ensure!(
        decoded.len() == BATTLE_NAME_LEGACY_STORAGE_BYTES + executable_size,
        "battle executable placement changed"
    );
    decoded.extend_from_slice(&atlas.lookup_table_bytes);
    decoded.resize(decoded.len().next_multiple_of(4), 0);
    let nibble = |p: u8| match p {
        3 => 2,
        13 => 15,
        _ => 0,
    };
    decoded.extend((0..=255u8).map(|b| nibble(b & 15) | (nibble(b >> 4) << 4)));
    for rectangle in [
        [768u16, 320, 16, 16],
        [768, 336, 16, 16],
        [928, 496, 16, 16],
        [944, 496, 16, 16],
    ] {
        for v in rectangle {
            decoded.extend(v.to_le_bytes());
        }
    }
    ensure!(
        BATTLE_NAME_LOAD_DESTINATION + decoded.len() as u32 == ascii_data,
        "battle data placement changed"
    );
    decoded.extend_from_slice(&ascii.bytes);
    decoded.resize(decoded.len().next_multiple_of(4), 0);
    ensure!(
        BATTLE_NAME_LOAD_DESTINATION + decoded.len() as u32 <= BATTLE_NAME_PACK_ADDRESS,
        "battle payload overlaps its pack copy"
    );
    let encoded = crate::compression::compress(&decoded, 0)?;
    ensure!(
        crate::compression::decompress(&encoded, false)? == decoded,
        "battle supplier failed compression roundtrip"
    );
    ensure!(
        encoded.len() <= BATTLE_NAME_STORED_CAPACITY,
        "battle supplier exceeds fixed native record capacity: {} > {}",
        encoded.len(),
        BATTLE_NAME_STORED_CAPACITY
    );
    Ok(BattleNameRuntimeProgram {
        fits_native_record: encoded.len() <= BATTLE_NAME_STORED_CAPACITY,
        decoded,
        encoded,
        entry_address: entry,
        dispatch_address,
        hangul_address: hangul.entry_address,
        ascii_address,
        legacy_address,
        code_ranges,
    })
}

fn emit_entry(
    origin: u32,
    coordinates: u32,
    dispatch: u32,
    palette: u32,
    rectangles: u32,
    direct_glyphs: &[(u16, u8)],
) -> Result<Vec<u8>> {
    let mut a = Assembler::new();
    macro_rules! nop {
        () => {
            a.emit(psx_r3000a::Instruction::nop());
        };
    }
    macro_rules! mov {
        ($d:expr,$s:expr) => {
            a.emit(Addu {
                rd: $d,
                rs: $s,
                rt: R::ZERO,
            });
        };
    }
    macro_rules! imm {
        ($d:expr,$v:expr) => {
            a.emit(Addiu {
                rt: $d,
                rs: R::ZERO,
                immediate: $v,
            });
        };
    }
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -72,
    });
    for (i, r) in [
        R::S0,
        R::S1,
        R::S2,
        R::S3,
        R::S4,
        R::S5,
        R::S6,
        R::S7,
        R::RA,
    ]
    .into_iter()
    .enumerate()
    {
        a.emit(Sw {
            rt: r,
            base: R::SP,
            offset: 32 + i as i16 * 4,
        });
    }
    super::emit_name_glyph_pack_copy(
        &mut a,
        coordinates,
        BATTLE_NAME_FONT_PIXELS,
        BATTLE_NAME_PACK_ADDRESS,
    );
    imm!(R::S0, 0);
    a.emit_all(load_address(R::S4, BATTLE_NAME_BUFFER_ADDRESS));
    a.label("slot")
        .emit_all(load_address(R::T0, 0x801f6494))
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::S0,
        })
        .emit(Lbu {
            rt: R::T1,
            base: R::T0,
            offset: 0,
        });
    imm!(R::T2, 30);
    a.bne(R::T1, R::T2, "next_slot");
    nop!();
    mov!(R::T0, R::S4);
    imm!(R::T1, 128);
    a.label("clear_name")
        .emit(Sw {
            rt: R::ZERO,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: -1,
        })
        .bne(R::T1, R::ZERO, "clear_name")
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 4,
        });
    a.emit_all(load_address(R::T0, 0x801f64fc))
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::S0,
        })
        .emit(Lbu {
            rt: R::S1,
            base: R::T0,
            offset: 0,
        });
    nop!();
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::S1,
        immediate: 17,
    })
    .beq(R::T0, R::ZERO, "upload");
    nop!();
    a.emit(Sll {
        rd: R::T0,
        rt: R::S1,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::S1,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 3,
    })
    .emit_all(load_address(R::S2, 0x801f5814))
    .emit(Addu {
        rd: R::S2,
        rs: R::S2,
        rt: R::T0,
    });
    imm!(R::S3, 0);
    a.label("glyph").emit(Lhu {
        rt: R::A0,
        base: R::S2,
        offset: 0,
    });
    emit_diary_battle_word(&mut a, direct_glyphs)?;
    imm!(R::T0, 0x3001);
    a.beq(R::A0, R::T0, "upload");
    mov!(R::A1, R::S1);
    a.emit(Jal { target: dispatch });
    nop!();
    a.beq(R::V0, R::ZERO, "next_glyph");
    mov!(R::S5, R::V1);
    a.emit(Addiu {
        rt: R::T2,
        rs: R::V0,
        immediate: 11,
    })
    .emit(Sll {
        rd: R::T3,
        rt: R::S3,
        shift: 3,
    })
    .emit(Addu {
        rd: R::T3,
        rs: R::T3,
        rt: R::S4,
    })
    .emit_all(load_address(R::T9, palette));
    imm!(R::T4, 16);
    a.label("row");
    imm!(R::T5, 8);
    a.label("byte")
        .emit(Lbu {
            rt: R::T6,
            base: R::T2,
            offset: 0,
        })
        .bne(R::S5, R::ZERO, "store_byte");
    nop!();
    a.emit(Addu {
        rd: R::T7,
        rs: R::T9,
        rt: R::T6,
    })
    .emit(Lbu {
        rt: R::T6,
        base: R::T7,
        offset: 0,
    });
    a.label("store_byte")
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: 1,
        })
        .emit(Sb {
            rt: R::T6,
            base: R::T3,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T5,
            rs: R::T5,
            immediate: -1,
        })
        .bne(R::T5, R::ZERO, "byte")
        .emit(Addiu {
            rt: R::T3,
            rs: R::T3,
            immediate: 1,
        });
    a.emit(Addiu {
        rt: R::T2,
        rs: R::T2,
        immediate: 2,
    })
    .emit(Addiu {
        rt: R::T4,
        rs: R::T4,
        immediate: -1,
    })
    .bne(R::T4, R::ZERO, "row")
    .emit(Addiu {
        rt: R::T3,
        rs: R::T3,
        immediate: 24,
    });
    a.label("next_glyph")
        .emit(Addiu {
            rt: R::S3,
            rs: R::S3,
            immediate: 1,
        })
        .emit(Sltiu {
            rt: R::T0,
            rs: R::S3,
            immediate: 4,
        })
        .bne(R::T0, R::ZERO, "glyph")
        .emit(Addiu {
            rt: R::S2,
            rs: R::S2,
            immediate: 2,
        });
    a.label("upload")
        .emit(Sll {
            rd: R::T0,
            rt: R::S0,
            shift: 3,
        })
        .emit_all(load_address(R::T1, rectangles))
        .emit(Addu {
            rd: R::T1,
            rs: R::T1,
            rt: R::T0,
        })
        .emit(Lw {
            rt: R::T2,
            base: R::T1,
            offset: 0,
        })
        .emit(Lw {
            rt: R::T3,
            base: R::T1,
            offset: 4,
        })
        .emit(Sw {
            rt: R::T2,
            base: R::SP,
            offset: 16,
        })
        .emit(Sw {
            rt: R::T3,
            base: R::SP,
            offset: 20,
        })
        .emit(Addiu {
            rt: R::A0,
            rs: R::SP,
            immediate: 16,
        });
    mov!(R::A1, R::S4);
    a.emit(Jal { target: 0x80062630 });
    nop!();
    a.emit(Jal { target: 0x80062384 });
    imm!(R::A0, 0);
    a.label("next_slot")
        .emit(Addiu {
            rt: R::S0,
            rs: R::S0,
            immediate: 1,
        })
        .emit(Sltiu {
            rt: R::T0,
            rs: R::S0,
            immediate: 4,
        })
        .bne(R::T0, R::ZERO, "slot");
    nop!();
    for (i, r) in [
        R::S0,
        R::S1,
        R::S2,
        R::S3,
        R::S4,
        R::S5,
        R::S6,
        R::S7,
        R::RA,
    ]
    .into_iter()
    .enumerate()
    {
        a.emit(Lw {
            rt: r,
            base: R::SP,
            offset: 32 + i as i16 * 4,
        });
    }
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 72,
    })
    .emit(Jr { rs: R::RA });
    nop!();
    let bytes = a.assemble(origin)?.bytes().to_vec();
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, origin)?,
        origin,
        "battle name generator",
    )?;
    Ok(bytes)
}

// MGAME/SIKEN producers recopy the display companion after KANRI closes.
// Resolve only temporary record 16 by its exact character position, using the
// same active keyboard mapping as the name-entry font. Registered records keep
// their own stored words and never borrow the current Diary profile.
fn emit_diary_battle_word(a: &mut Assembler, direct: &[(u16, u8)]) -> Result<()> {
    a.emit(Addiu {
        rt: R::T0,
        rs: R::ZERO,
        immediate: 16,
    })
    .bne(R::S1, R::T0, "diary_word_done")
    .emit(Sll {
        rd: R::T0,
        rt: R::S3,
        shift: 1,
    })
    .emit_all(load_address(R::T1, 0x801f1896))
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::T0,
    })
    .emit(Lhu {
        rt: R::A2,
        base: R::T1,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::T0,
        rs: R::ZERO,
        immediate: 0x3001,
    })
    .beq(R::A2, R::T0, "diary_word_canonical")
    .emit(Andi {
        rt: R::T0,
        rs: R::A2,
        immediate: 0xc000,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x8000,
    })
    .beq(R::T0, R::T1, "diary_word_canonical")
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x4000,
    })
    .beq(R::T0, R::T1, "diary_word_canonical")
    .emit(psx_r3000a::Instruction::nop());
    let mut keys = direct.to_vec();
    keys.sort_unstable();
    ensure!(
        !keys.is_empty() && keys.windows(2).all(|w| w[0].0 != w[1].0),
        "invalid battle direct-key mapping"
    );
    let mut runs: Vec<(u16, u16, u8)> = Vec::new();
    for (code, ch) in keys {
        ensure!(
            code < 0x4000 && ch.is_ascii(),
            "battle direct key leaves its domain"
        );
        if let Some((start, count, first)) = runs.last_mut() {
            if code == *start + *count && u16::from(ch) == u16::from(*first) + *count {
                *count += 1;
                continue;
            }
        }
        runs.push((code, 1, ch));
    }
    for (i, (start, count, first)) in runs.into_iter().enumerate() {
        let next = format!("diary_direct_next_{i}");
        a.emit(Addiu {
            rt: R::T0,
            rs: R::A2,
            immediate: -(start as i16),
        })
        .emit(Sltiu {
            rt: R::T1,
            rs: R::T0,
            immediate: count as i16,
        })
        .beq(R::T1, R::ZERO, &next)
        .emit(psx_r3000a::Instruction::nop())
        .emit(Addiu {
            rt: R::A0,
            rs: R::T0,
            immediate: 0x4000 | i16::from(first),
        })
        .beq(R::ZERO, R::ZERO, "diary_word_done")
        .emit(psx_r3000a::Instruction::nop())
        .label(&next);
    }
    a.beq(R::ZERO, R::ZERO, "diary_word_done")
        .emit(psx_r3000a::Instruction::nop())
        .label("diary_word_canonical")
        .emit(Addu {
            rd: R::A0,
            rs: R::A2,
            rt: R::ZERO,
        })
        .label("diary_word_done");
    Ok(())
}

#[cfg(test)]
#[path = "battle_runtime_tests.rs"]
mod tests;
