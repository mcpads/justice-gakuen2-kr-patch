//! Emits the loader sequence for a selector-owned copy of the shared name pack.
//! Call-site installation and RAM ownership are separate from this constructor.
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

use super::{
    NAME_GLYPH_PACK_CELL_COUNT, NAME_GLYPH_PACK_STORAGE_BYTES, NameInputRuntimeAtlasLayout,
    SelectorDataCopy, emit_name_glyph_pack_copy,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct NameAssetLoad {
    pub catalog_index: u16,
    pub destination: u32,
    pub decoded_byte_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorNameLoaderLayout {
    pub origin: u32,
    pub byte_capacity: usize,
    pub native_loader_address: u32,
    pub font: NameAssetLoad,
    pub textures: NameAssetLoad,
    pub font_pixel_address: u32,
    pub pack_destination: u32,
    /// Contiguous suppliers copied after the font load and before texture reload.
    /// Empty retains the original scattered-cell pack transport.
    #[serde(default)]
    pub font_copies: Vec<SelectorDataCopy>,
    #[serde(default)]
    pub restore_selector_service_table: bool,
    /// Source-zero transport ranges to restore after texture reload, before
    /// returning to any native consumer. Offsets within the decoded texture.
    #[serde(default)]
    pub restored_zero_ranges: Vec<[usize; 2]>,
}

#[derive(Clone, Debug)]
pub struct SelectorNameLoaderProgram {
    pub bytes: Vec<u8>,
    pub code_byte_count: usize,
    pub coordinate_table_address: u32,
    pub instructions: Vec<Instruction>,
}

pub(super) fn ram_range(address: u32, count: usize) -> Result<std::ops::Range<u32>> {
    let end = u64::from(address)
        .checked_add(u64::try_from(count)?)
        .ok_or_else(|| anyhow::anyhow!("selector loader range length overflow"))?;
    ensure!(
        count > 0 && address >= 0x8000_0000 && end <= 0x8020_0000,
        "selector name loader range leaves cached main RAM"
    );
    Ok(address..u32::try_from(end)?)
}

pub(super) fn overlaps(a: &std::ops::Range<u32>, b: &std::ops::Range<u32>) -> bool {
    a.start < b.end && b.start < a.end
}

/// The entry must already be resident outside both native load destinations.
/// Load MA_ENT, copy all scattered pack cells, reload the selector texture image,
/// and return to the original post-load continuation before native TIM uploads.
pub fn build_selector_name_loader(
    layout: &SelectorNameLoaderLayout,
    atlas: &NameInputRuntimeAtlasLayout,
) -> Result<SelectorNameLoaderProgram> {
    let code = ram_range(layout.origin, layout.byte_capacity)?;
    let pack = ram_range(layout.pack_destination, NAME_GLYPH_PACK_STORAGE_BYTES)?;
    let font = ram_range(layout.font.destination, layout.font.decoded_byte_count)?;
    let textures = ram_range(
        layout.textures.destination,
        layout.textures.decoded_byte_count,
    )?;
    let loader = ram_range(layout.native_loader_address, 4)?;
    let mut restored = Vec::new();
    for &[start, end] in &layout.restored_zero_ranges {
        ensure!(
            start < end
                && end <= layout.textures.decoded_byte_count
                && start.is_multiple_of(4)
                && end.is_multiple_of(4),
            "invalid selector transport restoration range"
        );
        let range = start..end;
        ensure!(
            restored
                .iter()
                .all(|r: &std::ops::Range<usize>| range.start >= r.end || r.start >= range.end),
            "overlapping selector transport restoration ranges"
        );
        restored.push(range);
    }
    ensure!(
        layout.origin.is_multiple_of(4)
            && layout.native_loader_address.is_multiple_of(4)
            && layout.pack_destination.is_multiple_of(2)
            && layout.font.destination.is_multiple_of(4)
            && layout.textures.destination.is_multiple_of(4)
            && layout.font_pixel_address.is_multiple_of(2),
        "selector loader has an unaligned code or pack address"
    );
    ensure!(
        !overlaps(&code, &pack)
            && !overlaps(&code, &font)
            && !overlaps(&code, &textures)
            && !overlaps(&pack, &font)
            && !overlaps(&pack, &textures)
            && !overlaps(&loader, &code)
            && !overlaps(&loader, &pack)
            && !overlaps(&loader, &font)
            && !overlaps(&loader, &textures),
        "selector name loader would overwrite resident code or copied pack"
    );
    let table = if layout.font_copies.is_empty() {
        ensure!(
            atlas.font_atlas_row_bytes == 384
                && atlas.glyph_cell_width == 20
                && atlas.glyph_cell_height == 20
                && atlas.pack_storage_cell_base_byte_offsets.len() == NAME_GLYPH_PACK_CELL_COUNT,
            "selector loader needs the shared 20x20 packed-cell layout"
        );
        let mut seen = std::collections::BTreeSet::new();
        let mut occupied = std::collections::BTreeSet::new();
        for &offset in &atlas.pack_storage_cell_base_byte_offsets {
            ensure!(
                usize::from(offset) % 384 + 10 <= 384,
                "selector name pack cell crosses an atlas row"
            );
            for row in 0..20 {
                for column in 0..10 {
                    ensure!(
                        occupied.insert(usize::from(offset) + row * 384 + column),
                        "selector name pack cells overlap"
                    );
                }
            }
            let start = layout
                .font_pixel_address
                .checked_add(u32::from(offset))
                .ok_or_else(|| anyhow::anyhow!("selector font cell address overflow"))?;
            let end = u64::from(start) + 19 * 384 + 10;
            ensure!(
                offset.is_multiple_of(2)
                    && seen.insert(offset)
                    && start >= font.start
                    && end <= u64::from(font.end),
                "selector name pack cell is unaligned, duplicated or outside the loaded font"
            );
        }
        let table: Vec<_> = atlas
            .pack_storage_cell_base_byte_offsets
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        ensure!(
            table == atlas.lookup_table_bytes,
            "selector name pack coordinates disagree with the atlas table"
        );
        table
    } else {
        let mut sources = Vec::new();
        let mut destinations = Vec::new();
        for (index, copy) in layout.font_copies.iter().enumerate() {
            ensure!(
                copy.source.is_multiple_of(4)
                    && copy.destination.is_multiple_of(4)
                    && copy.byte_count.is_multiple_of(4),
                "selector font copy is not word aligned"
            );
            let source = ram_range(copy.source, copy.byte_count)?;
            let destination = ram_range(copy.destination, copy.byte_count)?;
            ensure!(
                source.start >= font.start && source.end <= font.end,
                "selector font copy leaves loaded font"
            );
            ensure!(
                [&code, &font, &textures, &loader]
                    .iter()
                    .all(|range| !overlaps(range, &destination)),
                "selector font copy overwrites code or native loads"
            );
            if index == 0 {
                ensure!(
                    destination.start == pack.start && destination.end <= pack.end,
                    "first selector font copy must supply the reserved pack"
                );
            } else {
                ensure!(
                    !overlaps(&destination, &pack),
                    "additional selector font copy overlaps reserved pack"
                );
            }
            ensure!(
                sources.iter().all(|range| !overlaps(range, &source))
                    && destinations
                        .iter()
                        .all(|range| !overlaps(range, &destination)),
                "selector font copies overlap"
            );
            sources.push(source);
            destinations.push(destination);
        }
        Vec::new()
    };
    let first = emit_loader(layout, layout.origin)?;
    let code_byte_count = first.len();
    let coordinate_table_address = layout
        .origin
        .checked_add(u32::try_from(code_byte_count)?)
        .ok_or_else(|| anyhow::anyhow!("selector coordinate table address overflow"))?;
    let mut bytes = emit_loader(layout, coordinate_table_address)?;
    ensure!(
        bytes.len() == code_byte_count,
        "selector coordinate relocation changed code length"
    );
    let instructions = verify_placed_program(&bytes, layout.origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        layout.origin,
        "selector name loader",
    )?;
    bytes.extend_from_slice(&table);
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    ensure!(
        bytes.len() <= layout.byte_capacity,
        "selector name loader exceeds its code/table capacity"
    );
    Ok(SelectorNameLoaderProgram {
        bytes,
        code_byte_count,
        coordinate_table_address,
        instructions,
    })
}

fn emit_loader(layout: &SelectorNameLoaderLayout, table: u32) -> Result<Vec<u8>> {
    use Instruction::*;
    let mut a = Assembler::new();
    a.emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -40,
    });
    for (r, offset) in [
        (Register::S0, 16),
        (Register::S1, 20),
        (Register::S2, 24),
        (Register::S3, 28),
        (Register::RA, 36),
    ] {
        a.emit(Sw {
            rt: r,
            base: Register::SP,
            offset,
        });
    }
    // MA_ENT and the resident glyph pack reuse COMMOT's 800aa000..800d3898
    // arena. Native Training reselect retains this cache-valid byte, so battle
    // entry would otherwise skip catalog 58 at 80028134 and consume font data
    // as motion pointers. Invalidate before the first destructive load; the
    // native battle loader restores the whole resource after selector exit.
    a.emit(Lui {
        rt: Register::T0,
        immediate: 0x801f,
    })
    .emit(Sb {
        rt: Register::ZERO,
        base: Register::T0,
        offset: 0x64e9,
    });
    a.emit_all(load_address(Register::A0, layout.font.destination))
        .emit(Jal {
            target: layout.native_loader_address,
        })
        .emit(Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: layout.font.catalog_index,
        });
    if layout.font_copies.is_empty() {
        emit_name_glyph_pack_copy(
            &mut a,
            table,
            layout.font_pixel_address,
            layout.pack_destination,
        );
    } else {
        for (index, copy) in layout.font_copies.iter().enumerate() {
            let label = format!("copy_selector_font_{index}");
            a.emit_all(load_address(Register::T0, copy.source))
                .emit_all(load_address(Register::T1, copy.destination))
                .emit_all(load_address(
                    Register::T3,
                    copy.source + copy.byte_count as u32,
                ))
                .label(&label)
                .emit(Lw {
                    rt: Register::T2,
                    base: Register::T0,
                    offset: 0,
                })
                .emit(Addiu {
                    rt: Register::T0,
                    rs: Register::T0,
                    immediate: 4,
                })
                .emit(Sw {
                    rt: Register::T2,
                    base: Register::T1,
                    offset: 0,
                })
                .emit(Addiu {
                    rt: Register::T1,
                    rs: Register::T1,
                    immediate: 4,
                })
                .bne(Register::T0, Register::T3, &label)
                .emit(Instruction::nop());
        }
    }
    a.emit_all(load_address(Register::A0, layout.textures.destination))
        .emit(Jal {
            target: layout.native_loader_address,
        })
        .emit(Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: layout.textures.catalog_index,
        });
    for (index, &[start, end]) in layout.restored_zero_ranges.iter().enumerate() {
        let label = format!("restore_selector_transport_{index}");
        a.emit_all(load_address(
            Register::T0,
            layout.textures.destination + start as u32,
        ))
        .emit_all(load_address(
            Register::T1,
            layout.textures.destination + end as u32,
        ))
        .label(&label)
        .emit(Sw {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 4,
        })
        .bne(Register::T0, Register::T1, &label)
        .emit(Instruction::nop());
    }
    for (r, offset) in [
        (Register::RA, 36),
        (Register::S0, 16),
        (Register::S1, 20),
        (Register::S2, 24),
        (Register::S3, 28),
    ] {
        a.emit(Lw {
            rt: r,
            base: Register::SP,
            offset,
        });
    }
    if layout.restore_selector_service_table {
        // Displaced PLSEL1 setup at 800a2bb0..800a2bc0, before TIM uploads.
        a.emit(Lui {
            rt: Register::V0,
            immediate: 0x801f,
        })
        .emit(Lw {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x6360,
        })
        .emit(Lui {
            rt: Register::A0,
            immediate: 0x800d,
        })
        .emit(Lw {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x150,
        });
    }
    a.emit(Jr { rs: Register::RA }).emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: 40,
    });
    Ok(a.assemble(layout.origin)?.bytes().to_vec())
}

#[cfg(test)]
#[path = "selector_loader_tests.rs"]
mod tests;
