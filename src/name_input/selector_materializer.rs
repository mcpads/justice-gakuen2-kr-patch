//! Selector-local scratch rendering using the shared, lossless name font.
use super::selector_loader::{overlaps, ram_range};
use super::{NameGlyphBandPack, SharedNameOutlineRuntimeProgram, emit_contiguous_pack_byte_reader};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Register, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorMaterializerLayout {
    pub origin: u32,
    pub byte_capacity: usize,
    pub pack_address: u32,
    pub scratch_address: u32,
}

#[derive(Clone, Debug)]
pub struct SelectorMaterializerProgram {
    pub bytes: Vec<u8>,
    pub entry_address: u32,
    /// Source rectangle including the complete outline, within the 20x20 cell.
    pub glyph_rectangle: [usize; 4],
}

/// A0 is an untagged Hangul offset. Return the owned 20x20 packed scratch cell,
/// or zero without touching it for an unsupported code. Upload and sprite
/// placement must use glyph_rectangle rather than clipping to the native width.
pub fn build_selector_materializer(
    layout: &SelectorMaterializerLayout,
    pack: &NameGlyphBandPack,
    outline: &SharedNameOutlineRuntimeProgram,
) -> Result<SelectorMaterializerProgram> {
    let code = ram_range(layout.origin, layout.byte_capacity)?;
    let data = ram_range(layout.pack_address, pack.bytes().len())?;
    let scratch = ram_range(layout.scratch_address, 200)?;
    let helper = ram_range(outline.outline_pixel_address, outline.bytes.len())?;
    ensure!(
        layout.origin.is_multiple_of(4)
            && layout.pack_address.is_multiple_of(2)
            && layout.scratch_address.is_multiple_of(4),
        "unaligned selector materializer layout"
    );
    let ranges = [&code, &data, &scratch, &helper];
    for (i, range) in ranges.iter().enumerate() {
        ensure!(
            ranges[i + 1..].iter().all(|other| !overlaps(range, other)),
            "selector materializer overwrites code, pack, outline or scratch"
        );
    }
    let [x, y, width, height] = pack.crop();
    ensure!(
        x > 0 && y > 0 && x + width < 20 && y + height < 20,
        "selector glyph lacks space for its complete outline"
    );
    let mut a = Assembler::new();
    emit_contiguous_pack_byte_reader(&mut a, layout.pack_address, pack.bytes().len())?;
    let entry_address = layout.origin + u32::try_from(a.assemble(layout.origin)?.bytes().len())?;
    a.emit_all(load_address(Register::A1, layout.scratch_address));
    let render_origin = layout.origin + u32::try_from(a.assemble(layout.origin)?.bytes().len())?;
    let renderer = pack.build_render_program(render_origin, layout.origin, 10, outline)?;
    a.emit_all(verify_placed_program(&renderer, render_origin)?);
    let bytes = a.assemble(layout.origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= layout.byte_capacity,
        "selector materializer exceeds code capacity"
    );
    let instructions = verify_placed_program(&bytes, layout.origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        layout.origin,
        "selector materializer",
    )?;
    Ok(SelectorMaterializerProgram {
        bytes,
        entry_address,
        glyph_rectangle: [x - 1, y - 1, width + 2, height + 2],
    })
}

#[cfg(test)]
#[path = "selector_materializer_tests.rs"]
mod tests;
