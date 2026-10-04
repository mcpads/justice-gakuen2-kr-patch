//! MGTIT's saved-row consumer reads the canonical nickname and owns twenty cells.
use crate::compression::decompress;
use crate::dialogue_audit::{NAME_ENTRY_FONT_DECODED_SIZE, NAME_ENTRY_FONT_TIM_OFFSET};
use crate::name_input::{
    NameGlyphBandPack, NameGlyphMaterializationBundle, NameInputRuntimeAtlasLayout,
    build_shared_name_outline_runtime_program, emit_name_glyph_pack_copy,
};
use crate::pipeline::sha256_bytes;
use crate::tim::parse_4bpp_prefix;
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, decode, encode, load_address};
use serde::{Deserialize, Serialize};

const BASE: u32 = 0x800a_2000;
const END: usize = 0xbce8;
const PACK: u32 = 0x800b_0000;
const TAGS: u32 = PACK + 19000;
const SCRATCH: u32 = TAGS + 40;
const DIRECT_PIXELS: u32 = 0x800b_4b40;
const TRANSIENT: u32 = 0x800c_0000;
const LOADER: u32 = 0x8001_5414;
const NATIVE_INIT: u32 = 0x800a_6ae4;
const UPLOAD: u32 = 0x8006_2630;
const SYNC: u32 = 0x8006_2384;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinueNameRuntimeReport {
    pub program_address: String,
    pub initializer_address: String,
    pub resolver_address: String,
    pub byte_count: usize,
    pub sha256: String,
    pub pack_sha256: String,
    pub pack_byte_count: usize,
    pub pack_address: String,
    pub scratch_address: String,
    #[serde(default)]
    pub direct_glyph_count: usize,
    #[serde(default)]
    pub direct_pixels_sha256: String,
    pub cache_rects: Vec<[u16; 4]>,
}

fn nop() -> psx_r3000a::Instruction {
    psx_r3000a::Instruction::nop()
}

fn mov(a: &mut Assembler, d: R, s: R) {
    a.emit(Addu {
        rd: d,
        rs: s,
        rt: R::ZERO,
    });
}
fn imm(a: &mut Assembler, d: R, v: u16) {
    a.emit(Ori {
        rt: d,
        rs: R::ZERO,
        immediate: v,
    });
}
fn call(a: &mut Assembler, target: u32) {
    a.emit(Jal { target }).emit(nop());
}
fn finish(a: &Assembler, origin: u32) -> Result<Vec<u8>> {
    let p = a.assemble(origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &psx_r3000a::verify_placed_program(p.bytes(), origin)?,
        origin,
        "Continue name runtime",
    )?;
    Ok(p.bytes().to_vec())
}

// Two rows leave the contextual title cells at columns ten and eleven intact.
fn cache_rect(index: usize) -> [u16; 4] {
    [
        960 + (index % 10) as u16 * 5,
        200 + (index / 10) as u16 * 20,
        5,
        20,
    ]
}

pub(crate) fn collect_menu_requests() -> Vec<crate::menu_atlas_plan::MenuAtlasRequest> {
    (0..20)
        .map(|index| crate::menu_atlas_plan::MenuAtlasRequest {
            key: format!("continue:cache:{index:03}"),
            contexts: std::collections::BTreeSet::from(["mgtit".to_owned()]),
            // The native resolver computes this two-row geometry. Declare its fixed
            // footprint centrally; changing it also requires changing the resolver.
            candidates: vec![0x3a0 + (index / 10) * 16 + index % 10],
            width: 20,
            height: 20,
        })
        .collect()
}

fn initializer(
    origin: u32,
    coordinates: u32,
    pixels: u32,
    direct_table: u32,
    direct_count: u16,
) -> Result<Vec<u8>> {
    let mut a = Assembler::new();
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -48,
    });
    for (i, r) in [R::RA, R::S0, R::S1, R::S2, R::S3].iter().enumerate() {
        a.emit(Sw {
            rt: *r,
            base: R::SP,
            offset: 44 - i as i16 * 4,
        });
    }
    a.emit_all(load_address(R::A0, TRANSIENT));
    imm(&mut a, R::A1, 718);
    call(&mut a, LOADER);
    emit_name_glyph_pack_copy(&mut a, coordinates, pixels, PACK);
    // Preserve direct keyboard glyphs before the native title loader reuses memory.
    a.emit_all(load_address(R::S0, direct_table))
        .emit_all(load_address(R::S1, DIRECT_PIXELS));
    imm(&mut a, R::S2, direct_count);
    a.beq(R::S2, R::ZERO, "direct_copy_done")
        .emit(nop())
        .label("copy_direct_cell")
        .emit(Lhu {
            rt: R::T0,
            base: R::S0,
            offset: 4,
        })
        .emit_all(load_address(R::T1, pixels))
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::T1,
        });
    imm(&mut a, R::T2, 20);
    a.label("copy_direct_row");
    imm(&mut a, R::T3, 10);
    a.label("copy_direct_pixel")
        .emit(Lbu {
            rt: R::T4,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 1,
        })
        .emit(Sb {
            rt: R::T4,
            base: R::S1,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::S1,
            rs: R::S1,
            immediate: 1,
        })
        .emit(Addiu {
            rt: R::T3,
            rs: R::T3,
            immediate: -1,
        })
        .bne(R::T3, R::ZERO, "copy_direct_pixel")
        .emit(nop())
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 374,
        })
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: -1,
        })
        .bne(R::T2, R::ZERO, "copy_direct_row")
        .emit(nop())
        .emit(Addiu {
            rt: R::S0,
            rs: R::S0,
            immediate: 6,
        })
        .emit(Addiu {
            rt: R::S2,
            rs: R::S2,
            immediate: -1,
        })
        .bne(R::S2, R::ZERO, "copy_direct_cell")
        .emit(nop())
        .label("direct_copy_done");
    a.emit_all(load_address(R::T0, TAGS));
    imm(&mut a, R::T1, 20);
    imm(&mut a, R::T2, 0xffff);
    a.label("clear_continue_tags")
        .emit(Sh {
            rt: R::T2,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 2,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: -1,
        })
        .bne(R::T1, R::ZERO, "clear_continue_tags")
        .emit(nop());
    // Native title loading follows the transient MA_ENT read, restoring native scratch data.
    call(&mut a, NATIVE_INIT);
    for (i, r) in [R::RA, R::S0, R::S1, R::S2, R::S3].iter().enumerate() {
        a.emit(Lw {
            rt: *r,
            base: R::SP,
            offset: 44 - i as i16 * 4,
        });
    }
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 48,
    })
    .emit(Jr { rs: R::RA })
    .emit(nop());
    finish(&a, origin)
}

// At the replaced lookup, V1 is unsigned canonical code, S2 points just past
// this cell, S7 is its column. T0 returns the original packed atlas descriptor.
fn resolver(
    origin: u32,
    materializer: u32,
    direct_table: u32,
    direct_count: u16,
) -> Result<Vec<u8>> {
    let mut a = Assembler::new();
    a.emit(Andi {
        rt: R::T0,
        rs: R::V1,
        immediate: 0xc000,
    });
    imm(&mut a, R::T1, 0x8000);
    mov(&mut a, R::T5, R::ZERO);
    a.beq(R::T0, R::T1, "continue_dynamic").emit(nop());
    a.emit_all(load_address(R::T0, direct_table));
    imm(&mut a, R::T1, direct_count);
    a.emit_all(load_address(R::T5, DIRECT_PIXELS));
    a.beq(R::T1, R::ZERO, "continue_legacy")
        .emit(nop())
        .label("find_direct_name")
        .emit(Lhu {
            rt: R::T2,
            base: R::T0,
            offset: 0,
        })
        .emit(Lhu {
            rt: R::T3,
            base: R::T0,
            offset: 2,
        })
        .beq(R::T2, R::V1, "continue_dynamic")
        .emit(nop())
        .beq(R::T3, R::V1, "continue_dynamic")
        .emit(nop())
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 6,
        })
        .emit(Addiu {
            rt: R::T5,
            rs: R::T5,
            immediate: 200,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: -1,
        })
        .bne(R::T1, R::ZERO, "find_direct_name")
        .emit(nop())
        .label("continue_legacy");
    // Older untagged source names retain their original renderer mapping.
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::V1,
        immediate: 0x061e,
    })
    .beq(R::T0, R::ZERO, "continue_skip")
    .emit(nop())
    .emit(Sll {
        rd: R::T0,
        rt: R::V1,
        shift: 1,
    })
    .emit_all(load_address(R::T1, 0x800a_2378))
    .emit(Addu {
        rd: R::T1,
        rs: R::T1,
        rt: R::T0,
    })
    .emit(Lhu {
        rt: R::T0,
        base: R::T1,
        offset: 0,
    })
    .emit(nop())
    .emit(Andi {
        rt: R::T0,
        rs: R::T0,
        immediate: 0x0fff,
    })
    .emit(Jr { rs: R::RA })
    .emit(nop());
    a.label("continue_skip");
    imm(&mut a, R::T0, 0x0fff);
    a.emit(Jr { rs: R::RA }).emit(nop());
    a.label("continue_dynamic").emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -64,
    });
    for (i, r) in [R::RA, R::S0, R::S1, R::S2, R::S3].iter().enumerate() {
        a.emit(Sw {
            rt: *r,
            base: R::SP,
            offset: 60 - i as i16 * 4,
        });
    }
    a.emit(Sw {
        rt: R::T5,
        base: R::SP,
        offset: 24,
    });
    mov(&mut a, R::S0, R::V1);
    a.emit_all(load_address(R::T0, 0x801f_63f0))
        .emit(Lw {
            rt: R::T0,
            base: R::T0,
            offset: 0,
        })
        .emit(nop())
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 0x1900,
        })
        .emit(Subu {
            rd: R::T0,
            rs: R::S2,
            rt: R::T0,
        })
        .emit(Srl {
            rd: R::T0,
            rt: R::T0,
            shift: 9,
        })
        .emit(Sltiu {
            rt: R::T1,
            rs: R::T0,
            immediate: 5,
        })
        .beq(R::T1, R::ZERO, "continue_invalid")
        .emit(nop())
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 2,
        })
        .emit(Addu {
            rd: R::S1,
            rs: R::T0,
            rt: R::S7,
        });
    imm(&mut a, R::T1, 10);
    a.emit(Divu {
        rs: R::S1,
        rt: R::T1,
    })
    .emit(Mflo { rd: R::T2 })
    .emit(Mfhi { rd: R::T3 });
    // Descriptor 0x3a0 + quotient*16 + remainder.
    a.emit(Sll {
        rd: R::S3,
        rt: R::T2,
        shift: 4,
    })
    .emit(Addu {
        rd: R::S3,
        rs: R::S3,
        rt: R::T3,
    })
    .emit(Addiu {
        rt: R::S3,
        rs: R::S3,
        immediate: 0x03a0,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::S1,
        shift: 1,
    })
    .emit_all(load_address(R::S2, TAGS))
    .emit(Addu {
        rd: R::S2,
        rs: R::S2,
        rt: R::T0,
    })
    .emit(Lhu {
        rt: R::T0,
        base: R::S2,
        offset: 0,
    })
    .emit(nop())
    .beq(R::T0, R::S0, "continue_cached")
    .emit(nop())
    .emit(Andi {
        rt: R::A0,
        rs: R::S0,
        immediate: 0x3fff,
    })
    .emit_all(load_address(R::A1, SCRATCH));
    a.emit(Lw {
        rt: R::T0,
        base: R::SP,
        offset: 24,
    })
    .emit(nop())
    .bne(R::T0, R::ZERO, "render_direct_name")
    .emit(nop());
    call(&mut a, materializer);
    a.beq(R::V0, R::ZERO, "continue_invalid").emit(nop());
    a.beq(R::ZERO, R::ZERO, "remap_continue_palette")
        .emit(nop());
    a.label("render_direct_name")
        .emit_all(load_address(R::T1, SCRATCH));
    imm(&mut a, R::T2, 200);
    a.label("copy_direct_scratch")
        .emit(Lbu {
            rt: R::T3,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 1,
        })
        .emit(Sb {
            rt: R::T3,
            base: R::T1,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: 1,
        })
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: -1,
        })
        .bne(R::T2, R::ZERO, "copy_direct_scratch")
        .emit(nop())
        .label("remap_continue_palette");
    // Match the title CLUT's fill (14) while retaining outline (3) and zero.
    a.emit_all(load_address(R::T0, SCRATCH));
    imm(&mut a, R::T1, 200);
    a.label("continue_palette")
        .emit(Lbu {
            rt: R::T2,
            base: R::T0,
            offset: 0,
        })
        .emit(nop())
        .emit(Andi {
            rt: R::T3,
            rs: R::T2,
            immediate: 15,
        });
    imm(&mut a, R::T4, 13);
    a.bne(R::T3, R::T4, "continue_high_nibble")
        .emit(nop())
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: 1,
        });
    a.label("continue_high_nibble").emit(Andi {
        rt: R::T3,
        rs: R::T2,
        immediate: 240,
    });
    imm(&mut a, R::T4, 208);
    a.bne(R::T3, R::T4, "continue_store_pixel")
        .emit(nop())
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: 16,
        });
    a.label("continue_store_pixel")
        .emit(Sb {
            rt: R::T2,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 1,
        })
        .emit(Addiu {
            rt: R::T1,
            rs: R::T1,
            immediate: -1,
        })
        .bne(R::T1, R::ZERO, "continue_palette")
        .emit(nop());
    a.emit(Andi {
        rt: R::T0,
        rs: R::S3,
        immediate: 15,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T0,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: 960,
    })
    .emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 16,
    })
    .emit(Srl {
        rd: R::T0,
        rt: R::S3,
        shift: 4,
    })
    .emit(Andi {
        rt: R::T0,
        rs: R::T0,
        immediate: 15,
    })
    .emit(Sll {
        rd: R::T1,
        rt: R::T0,
        shift: 2,
    })
    .emit(Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 2,
    })
    .emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 18,
    });
    imm(&mut a, R::T0, 5);
    a.emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 20,
    });
    imm(&mut a, R::T0, 20);
    a.emit(Sh {
        rt: R::T0,
        base: R::SP,
        offset: 22,
    });
    a.emit(Addiu {
        rt: R::A0,
        rs: R::SP,
        immediate: 16,
    })
    .emit_all(load_address(R::A1, SCRATCH));
    call(&mut a, UPLOAD);
    mov(&mut a, R::A0, R::ZERO);
    call(&mut a, SYNC);
    a.emit(Sh {
        rt: R::S0,
        base: R::S2,
        offset: 0,
    })
    .label("continue_cached");
    mov(&mut a, R::T0, R::S3);
    a.beq(R::ZERO, R::ZERO, "continue_return")
        .emit(nop())
        .label("continue_invalid");
    imm(&mut a, R::T0, 0x0fff);
    a.label("continue_return");
    for (i, r) in [R::RA, R::S0, R::S1, R::S2, R::S3].iter().enumerate() {
        a.emit(Lw {
            rt: *r,
            base: R::SP,
            offset: 60 - i as i16 * 4,
        });
    }
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 64,
    })
    .emit(Jr { rs: R::RA })
    .emit(nop());
    finish(&a, origin)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn install(
    source: &[u8],
    output: &mut [u8],
    storage_start: usize,
    stored_font: &[u8],
    atlas: &NameInputRuntimeAtlasLayout,
    bundle: &NameGlyphMaterializationBundle,
    direct_glyphs: &[crate::name_input::NameInputDirectGlyphSource],
    occupied_rects: &[[u16; 4]],
    menu_plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<ContinueNameRuntimeReport> {
    ensure!(
        source.len() == END
            && output.len() == END
            && storage_start < END
            && storage_start.is_multiple_of(4),
        "MGTIT size changed"
    );
    ensure!(
        source[storage_start..END].iter().all(|v| *v == 0)
            && output[storage_start..END] == source[storage_start..END],
        "Continue runtime storage overlaps a writer"
    );
    let rects = (0..20)
        .map(|index| {
            let cell = menu_plan.glyph(&format!("continue:cache:{index:03}"))?.cell;
            let rect = crate::contextual_texture_upload::menu_atlas_vram_rect(cell)?;
            ensure!(
                rect == cache_rect(index),
                "Continue plan disagrees with the native cache resolver"
            );
            Ok(rect)
        })
        .collect::<Result<Vec<_>>>()?;
    for a in &rects {
        for b in occupied_rects {
            ensure!(
                a[0] + a[2] <= b[0]
                    || b[0] + b[2] <= a[0]
                    || a[1] + a[3] <= b[1]
                    || b[1] + b[3] <= a[1],
                "Continue cache overlaps a title glyph"
            );
        }
    }
    let font = decompress(stored_font, true)?;
    ensure!(
        font.len() >= NAME_ENTRY_FONT_DECODED_SIZE
            && u64::from(TRANSIENT) + font.len() as u64 <= 0x8010_0000,
        "MA_ENT extent changed"
    );
    let tim = parse_4bpp_prefix(&font[NAME_ENTRY_FONT_TIM_OFFSET..])?;
    ensure!(
        tim.row_bytes() == 384
            && atlas.font_atlas_row_bytes == 384
            && atlas.pack_storage_cell_base_byte_offsets.len() == 95
            && atlas.lookup_table_bytes.len() == 190,
        "MA_ENT pack geometry changed"
    );
    let pixels = NAME_ENTRY_FONT_TIM_OFFSET + tim.pixel_offset;
    let mut copied = Vec::new();
    for &offset in &atlas.pack_storage_cell_base_byte_offsets {
        for row in 0..20 {
            let start = pixels + usize::from(offset) + row * 384;
            copied.extend_from_slice(
                font.get(start..start + 10)
                    .ok_or_else(|| anyhow::anyhow!("MA_ENT pack coordinate outside font"))?,
            );
        }
    }
    ensure!(
        copied.get(..bundle.pack_bytes.len()) == Some(bundle.pack_bytes.as_slice()),
        "Continue pack differs from MA_ENT supplier"
    );
    let expected_lookup: Vec<_> = atlas
        .pack_storage_cell_base_byte_offsets
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    ensure!(
        expected_lookup == atlas.lookup_table_bytes,
        "MA_ENT physical lookup mismatch"
    );
    let admitted: std::collections::BTreeSet<_> = crate::name_input::LATIN_KEYS
        .bytes()
        .chain(crate::name_input::DIGIT_KEYS.bytes())
        .chain(crate::name_input::SYMBOL_KEYS.bytes())
        .collect();
    ensure!(
        direct_glyphs
            .iter()
            .map(|g| g.character)
            .collect::<std::collections::BTreeSet<_>>()
            == admitted
            && direct_glyphs.len() == admitted.len(),
        "Continue direct-key supplier does not cover the admitted keyboard"
    );
    ensure!(
        direct_glyphs
            .iter()
            .map(|g| g.legacy_code)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == direct_glyphs.len(),
        "Continue direct-key codes alias"
    );
    let mut direct_bytes = Vec::new();
    let mut direct_table_bytes = Vec::new();
    for g in direct_glyphs {
        ensure!(g.legacy_code < 0x0fff, "invalid direct legacy code");
        direct_table_bytes.extend(g.legacy_code.to_le_bytes());
        direct_table_bytes.extend((0x4000 | u16::from(g.character)).to_le_bytes());
        direct_table_bytes.extend(g.pixel_byte_offset.to_le_bytes());
        for row in 0..20 {
            let a = pixels + usize::from(g.pixel_byte_offset) + row * 384;
            direct_bytes.extend_from_slice(
                font.get(a..a + 10)
                    .ok_or_else(|| anyhow::anyhow!("direct glyph outside MA_ENT"))?,
            );
        }
    }
    ensure!(
        DIRECT_PIXELS as usize + direct_bytes.len() <= TRANSIENT as usize,
        "Continue direct glyph arena overlaps MA_ENT load"
    );
    let direct_count = u16::try_from(direct_glyphs.len())?;
    let pack = NameGlyphBandPack::parse(bundle.pack_bytes.clone())?;
    let origin = BASE + storage_start as u32;
    let mut reader = Assembler::new();
    reader
        .emit_all(load_address(R::T0, PACK))
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::A0,
        })
        .emit(Lbu {
            rt: R::V0,
            base: R::T0,
            offset: 0,
        })
        .emit(nop())
        .emit(Jr { rs: R::RA })
        .emit(nop());
    let mut bytes = finish(&reader, origin)?;
    let materializer = origin + bytes.len() as u32;
    bytes.extend(pack.build_render_program(
        materializer,
        origin,
        10,
        &build_shared_name_outline_runtime_program()?,
    )?);
    let resolver_address = origin + bytes.len() as u32;
    let resolver_size = resolver(resolver_address, materializer, 0, direct_count)?.len();
    let initializer_address = resolver_address + resolver_size as u32;
    let dummy = initializer(initializer_address, 0, 0, 0, direct_count)?;
    let coordinates = initializer_address + dummy.len() as u32;
    let direct_table = coordinates + atlas.lookup_table_bytes.len() as u32;
    bytes.extend(resolver(
        resolver_address,
        materializer,
        direct_table,
        direct_count,
    )?);
    bytes.extend(initializer(
        initializer_address,
        coordinates,
        TRANSIENT + pixels as u32,
        direct_table,
        direct_count,
    )?);
    bytes.extend(&atlas.lookup_table_bytes);
    bytes.extend(direct_table_bytes);
    ensure!(
        storage_start + bytes.len() <= END,
        "Continue runtime needs {} bytes, available {}",
        bytes.len(),
        END - storage_start
    );
    let patches = [
        (
            0x1c94,
            Jal {
                target: NATIVE_INIT,
            },
            Jal {
                target: initializer_address,
            },
        ),
        (
            0x4080,
            Addiu {
                rt: R::S2,
                rs: R::A1,
                immediate: 0x86,
            },
            Addiu {
                rt: R::S2,
                rs: R::A1,
                immediate: 0x96,
            },
        ),
        (
            0x40b4,
            Lh {
                rt: R::V1,
                base: R::S2,
                offset: 0,
            },
            Lhu {
                rt: R::V1,
                base: R::S2,
                offset: 0,
            },
        ),
        (
            0x424c,
            Slti {
                rt: R::V0,
                rs: R::S7,
                immediate: 5,
            },
            Slti {
                rt: R::V0,
                rs: R::S7,
                immediate: 4,
            },
        ),
    ];
    for (offset, expected, _) in &patches {
        ensure!(
            decode(
                u32::from_le_bytes(source[*offset..*offset + 4].try_into()?),
                BASE + *offset as u32
            )? == *expected
                && output[*offset..*offset + 4] == source[*offset..*offset + 4],
            "Continue hook changed at {offset:x}"
        );
    }
    let old = [
        Lui {
            rt: R::AT,
            immediate: 0x800a,
        },
        Addu {
            rd: R::AT,
            rs: R::AT,
            rt: R::V0,
        },
        Lh {
            rt: R::T0,
            base: R::AT,
            offset: 0x2378,
        },
        Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 0x0fff,
        },
        Andi {
            rt: R::T0,
            rs: R::T0,
            immediate: 0x0fff,
        },
    ];
    for (i, expected) in old.iter().enumerate() {
        let o = 0x40d8 + i * 4;
        ensure!(
            decode(
                u32::from_le_bytes(source[o..o + 4].try_into()?),
                BASE + o as u32
            )? == *expected
                && output[o..o + 4] == source[o..o + 4],
            "Continue lookup source changed"
        );
    }
    for (offset, _, replacement) in patches {
        output[offset..offset + 4]
            .copy_from_slice(&encode(&replacement, BASE + offset as u32)?.to_le_bytes());
    }
    for (i, op) in [
        Jal {
            target: resolver_address,
        },
        nop(),
        Addiu {
            rt: R::V0,
            rs: R::ZERO,
            immediate: 0x0fff,
        },
        nop(),
        nop(),
    ]
    .iter()
    .enumerate()
    {
        let o = 0x40d8 + i * 4;
        output[o..o + 4].copy_from_slice(&encode(op, BASE + o as u32)?.to_le_bytes());
    }
    output[storage_start..storage_start + bytes.len()].copy_from_slice(&bytes);
    Ok(ContinueNameRuntimeReport {
        program_address: format!("0x{origin:08x}"),
        initializer_address: format!("0x{initializer_address:08x}"),
        resolver_address: format!("0x{resolver_address:08x}"),
        byte_count: bytes.len(),
        sha256: sha256_bytes(&bytes),
        pack_sha256: sha256_bytes(&bundle.pack_bytes),
        pack_byte_count: bundle.pack_bytes.len(),
        pack_address: format!("0x{PACK:08x}"),
        scratch_address: format!("0x{SCRATCH:08x}"),
        direct_glyph_count: direct_glyphs.len(),
        direct_pixels_sha256: sha256_bytes(&direct_bytes),
        cache_rects: rects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute_with_callbacks;

    // Every saved row has independent glyph storage; unchanged frames must not
    // queue another upload, and a changed name must replace only its own cell.
    #[test]
    fn saved_rows_cache_by_slot_and_code_without_clobbering_caller() {
        let origin = 0x800a_d800;
        let render = 0x8001_1000;
        let program = resolver(origin, render, 0, 0).unwrap();
        let mut memory = vec![0xee; 0x20_0000];
        memory[0x1f63f0..0x1f63f4].copy_from_slice(&0x801e8000u32.to_le_bytes());
        memory[(TAGS & 0x1fff_ffff) as usize..(TAGS & 0x1fff_ffff) as usize + 40].fill(0xff);
        let mut uploads = 0;
        let mut renders = 0;
        let mut syncs = 0;
        let mut pending = None;
        for pass in 0..3 {
            for slot in 0..20 {
                let row = slot / 4;
                let col = slot % 4;
                let code = 0x8000 + if pass == 2 { 1 } else { 0 };
                let mut r = [0u32; 32];
                for (i, reg) in r.iter_mut().enumerate().take(24).skip(16) {
                    *reg = 0x12000000 + i as u32;
                }
                r[18] = 0x801e9900 + row as u32 * 0x200 + 0x96 + col as u32 * 2 + 2;
                r[23] = col as u32;
                r[3] = code;
                r[29] = 0x801df000;
                r[31] = 0x80010000;
                let before = r;
                execute_with_callbacks(
                    &program,
                    origin,
                    &mut r,
                    &mut memory,
                    None,
                    &mut |pc, r, m| {
                        let sp = (r[29] & 0x1fff_ffff) as usize;
                        if pc == render {
                            renders += 1;
                            assert_eq!(r[4], code & 0x3fff);
                            assert_eq!(r[5], SCRATCH);
                            let a = (SCRATCH & 0x1fff_ffff) as usize;
                            m[a..a + 200].fill(0xd3);
                            r[2] = SCRATCH;
                        } else if pc == UPLOAD {
                            assert!(pending.is_none());
                            uploads += 1;
                            let a = (r[4] & 0x1fff_ffff) as usize;
                            let rect: Vec<_> = m[a..a + 8]
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|x| u16::from_le_bytes(*x))
                                .collect();
                            assert_eq!(rect, cache_rect(slot));
                            let a = (r[5] & 0x1fff_ffff) as usize;
                            assert_eq!(&m[a..a + 200], &[0xe3; 200]);
                            pending = Some(m[a..a + 200].to_vec());
                        } else if pc == SYNC {
                            syncs += 1;
                            assert_eq!(r[4], 0);
                            let a = (SCRATCH & 0x1fff_ffff) as usize;
                            assert_eq!(pending.take().unwrap(), m[a..a + 200]);
                        } else {
                            return false;
                        }
                        m[sp..sp + 16].fill(0xa5);
                        for i in [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                            r[i] = 0xdead0000 + i as u32;
                        }
                        true
                    },
                );
                assert_eq!(r[8], 0x3a0 + (slot / 10) as u32 * 16 + (slot % 10) as u32);
                assert_eq!(r[16..24], before[16..24]);
                assert_eq!(r[29], before[29]);
                assert_eq!(r[31], before[31]);
                let a = (TAGS & 0x1fff_ffff) as usize + slot * 2;
                assert_eq!(
                    u16::from_le_bytes(memory[a..a + 2].try_into().unwrap()),
                    code as u16
                );
            }
            assert_eq!(uploads, if pass == 2 { 40 } else { 20 });
        }
        assert_eq!((renders, uploads, syncs), (40, 40, 40));
    }

    #[test]
    fn every_direct_key_renders_from_its_own_cell_for_legacy_and_tagged_records() {
        let keys: Vec<_> = crate::name_input::LATIN_KEYS
            .bytes()
            .chain(crate::name_input::DIGIT_KEYS.bytes())
            .chain(crate::name_input::SYMBOL_KEYS.bytes())
            .collect();
        let origin = 0x800ad200;
        let table = 0x800ac800;
        let render = 0x80011000;
        let program = resolver(origin, render, table, keys.len() as u16).unwrap();
        let mut memory = vec![0u8; 0x200000];
        memory[0x1f63f0..0x1f63f4].copy_from_slice(&0x801e8000u32.to_le_bytes());
        for (i, &key) in keys.iter().enumerate() {
            let offset = (table & 0x1fffffff) as usize + i * 6;
            memory[offset..offset + 2].copy_from_slice(&(0x300 + i as u16).to_le_bytes());
            memory[offset + 2..offset + 4]
                .copy_from_slice(&(0x4000 | u16::from(key)).to_le_bytes());
            memory[(DIRECT_PIXELS & 0x1fffffff) as usize + i * 200 + usize::from(key)] = 0xd3;
        }
        let sources = memory[(DIRECT_PIXELS & 0x1fffffff) as usize
            ..(DIRECT_PIXELS & 0x1fffffff) as usize + keys.len() * 200]
            .to_vec();
        for (i, &key) in keys.iter().enumerate() {
            for code in [0x300 + i as u16, 0x4000 | u16::from(key)] {
                let mut uploads = 0;
                let mut syncs = 0;
                for _ in 0..2 {
                    let mut r = [0u32; 32];
                    r[3] = u32::from(code);
                    r[18] = 0x801e9998;
                    r[29] = 0x801df000;
                    r[31] = 0x80010000;
                    execute_with_callbacks(
                        &program,
                        origin,
                        &mut r,
                        &mut memory,
                        None,
                        &mut |pc, r, m| {
                            assert_ne!(pc, render, "ASCII must use its rasterized direct cell");
                            if pc == UPLOAD {
                                uploads += 1;
                                let a = (r[5] & 0x1fffffff) as usize;
                                let mut expected = [0u8; 200];
                                expected[usize::from(key)] = 0xe3;
                                assert_eq!(m[a..a + 200], expected, "key {}", char::from(key));
                                true
                            } else if pc == SYNC {
                                syncs += 1;
                                true
                            } else {
                                false
                            }
                        },
                    );
                    assert_eq!(r[8], 0x3a0);
                }
                assert_eq!((uploads, syncs), (1, 1));
            }
        }
        assert_eq!(
            sources,
            memory[(DIRECT_PIXELS & 0x1fffffff) as usize
                ..(DIRECT_PIXELS & 0x1fffffff) as usize + keys.len() * 200]
        );
    }

    #[test]
    fn rejected_hangul_does_not_upload_or_publish_cache_tag() {
        let origin = 0x800a_d800;
        let render = 0x80011000;
        let mut m = vec![0; 0x20_0000];
        m[0x1f63f0..0x1f63f4].copy_from_slice(&0x801e8000u32.to_le_bytes());
        let mut r = [0; 32];
        r[3] = 0xbfff;
        r[18] = 0x801e9998;
        r[29] = 0x801df000;
        r[31] = 0x80010000;
        execute_with_callbacks(
            &resolver(origin, render, 0, 0).unwrap(),
            origin,
            &mut r,
            &mut m,
            None,
            &mut |pc, r, _| {
                assert_ne!(pc, UPLOAD);
                assert_ne!(pc, SYNC);
                if pc != render {
                    return false;
                }
                r[2] = 0;
                true
            },
        );
        assert_eq!(r[8], 0xfff);
        assert!(
            m[(TAGS & 0x1fff_ffff) as usize..(TAGS & 0x1fff_ffff) as usize + 40]
                .iter()
                .all(|v| *v == 0)
        );
    }
}
