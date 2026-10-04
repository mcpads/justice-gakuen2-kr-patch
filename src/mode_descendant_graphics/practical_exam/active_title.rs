use std::ops::Range;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, encode, verify_placed_program};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use crate::write_scope::changed_ranges_are_within;

use super::glyph_atlas::PracticalExamActiveTitleCachePlan;

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const BUILDER_OFFSET: usize = 0x5008;
const BUILDER_SIZE: usize = 0x0eec;
const BUILDER_RUNTIME_ADDRESS: u32 = OVERLAY_RUNTIME_BASE + BUILDER_OFFSET as u32;
const SOURCE_BUILDER_SHA256: &str =
    "96b3c0138a1933322e444e716bef447e49e12c9105c5337a9dae5e4aee23a32f";
const PRACTICAL_EXAM_TIM_OFFSET: usize = 0x3c800;
pub(super) const ACTIVE_TITLE_START_SLOT: usize = 4;

const SAFE_RESULT_CACHE_BANDS: [Cell; 2] = [
    Cell {
        x: 192,
        y: 208,
        width: 64,
        height: 16,
    },
    Cell {
        x: 32,
        y: 240,
        width: 32,
        height: 16,
    },
];

#[derive(Debug, Clone)]
pub(super) struct PracticalExamActiveTitleBuilderReport {
    pub(super) allowed_range: Range<usize>,
    pub(super) data_ranges: Vec<Range<usize>>,
    pub(super) runtime_address: u32,
    pub(super) installed_instructions: Vec<Instruction>,
    pub(super) typed_readback_verified: bool,
}

pub(in crate::mode_descendant_graphics) struct PracticalExamActiveTitleTextureBuild {
    pub(in crate::mode_descendant_graphics) decoded: Vec<u8>,
    pub(in crate::mode_descendant_graphics) claims: Vec<DecodedDataClaim>,
    pub(in crate::mode_descendant_graphics) cache_cells: Vec<Cell>,
}

pub(super) fn install_active_title_builder(
    source_overlay: &[u8],
) -> Result<(Vec<u8>, PracticalExamActiveTitleBuilderReport)> {
    let allowed_range = BUILDER_OFFSET..BUILDER_OFFSET + BUILDER_SIZE;
    let source = source_overlay
        .get(allowed_range.clone())
        .context("1999 practical-exam active-title builder is truncated")?;
    ensure!(
        sha256_bytes(source) == SOURCE_BUILDER_SHA256,
        "1999 practical-exam active-title builder source changed"
    );
    ensure!(
        verify_placed_program(source, BUILDER_RUNTIME_ADDRESS)?.len() == BUILDER_SIZE / 4,
        "1999 practical-exam active-title source is not a complete R3000A block"
    );

    // Active gameplay uses its own primitive bank, ordering bucket and uploaded
    // instruction-member texture. The menu renderer is not callable here.
    let mut bytes = source.to_vec();
    for (offset, register, old, new) in [
        (0x527c, Register::A0, 128, 0),
        (0x528c, Register::A1, 500, 503),
        (0x52b0, Register::V0, 24, 16),
        (0x52c0, Register::V0, 70, 62),
        (0x53ec, Register::V0, 15, 16),
        (0x53f4, Register::V0, 110, 94),
        (0x5b54, Register::A2, 768, 896),
        (0x5c14, Register::A1, 500, 503),
        (0x5c44, Register::V0, 8, 16),
        (0x5d28, Register::S6, 384, 400),
        (0x5df4, Register::A0, 112, 0),
        (0x5e04, Register::A1, 500, 503),
        (0x5e2c, Register::V0, 24, 16),
    ] {
        let address = OVERLAY_RUNTIME_BASE + offset as u32;
        let relative = offset - BUILDER_OFFSET;
        let expected = Instruction::Addiu {
            rt: register,
            rs: Register::ZERO,
            immediate: old,
        };
        ensure!(
            bytes[relative..relative + 4] == encode(&expected, address)?.to_le_bytes(),
            "term gameplay HUD source instruction changed at {address:08x}"
        );
        let replacement = Instruction::Addiu {
            rt: register,
            rs: Register::ZERO,
            immediate: new,
        };
        bytes[relative..relative + 4]
            .copy_from_slice(&encode(&replacement, address)?.to_le_bytes());
    }
    let advance_address = OVERLAY_RUNTIME_BASE + 0x5e9c;
    let relative = 0x5e9c - BUILDER_OFFSET;
    let old_advance = Instruction::Addiu {
        rt: Register::S6,
        rs: Register::S6,
        immediate: 24,
    };
    ensure!(
        bytes[relative..relative + 4] == encode(&old_advance, advance_address)?.to_le_bytes(),
        "attempt counter decimal advance changed"
    );
    let new_advance = Instruction::Addiu {
        rt: Register::S6,
        rs: Register::S6,
        immediate: 16,
    };
    bytes[relative..relative + 4]
        .copy_from_slice(&encode(&new_advance, advance_address)?.to_le_bytes());
    let instructions = verify_placed_program(&bytes, BUILDER_RUNTIME_ADDRESS)?;

    let mut patched = source_overlay.to_vec();
    patched[allowed_range.clone()].copy_from_slice(&bytes);
    let data_ranges = vec![0x003c..0x00b4, 0x0a28..0x0a40];
    ensure!(
        source_overlay[0x0a00..0x0a14]
            == [
                160, 48, 184, 48, 208, 48, 232, 48, 160, 64, 184, 64, 208, 64, 232, 64, 160, 80,
                184, 80
            ],
        "term gameplay decimal selector table changed"
    );
    ensure!(
        source_overlay[0x0a28..0x0a40]
            == [
                12, 8, 0, 184, 96, 24, 16, 0, 12, 8, 0, 184, 112, 24, 16, 20, 12, 7, 0, 208, 112,
                24, 16, 60
            ],
        "term gameplay prefix descriptors changed"
    );
    // The UV table at 0xa00 also supplies the timer. Keep every entry and
    // texture page unchanged; both gameplay providers regenerate these cells.
    for (index, screen_offset) in [0, 12, 48].into_iter().enumerate() {
        let offset = 0x0a28 + index * 8;
        patched[offset + 1..offset + 3].copy_from_slice(&[0, 3]);
        patched[offset + 5] = 16;
        patched[offset + 7] = screen_offset;
    }
    ensure!(
        sha256_bytes(&source_overlay[0x3c..0xb4])
            == "0956a6d6d7783f1f45a3e8b43580f8270bc058970d942554d0a9b66258765c10",
        "attempt counter's three native rows changed"
    );
    use crate::practical_instruction_graphics::counter_layout::{LABEL_CELLS, label_index};
    for row in 0..3 {
        for fragment in 0..4 {
            let offset = 0x3c + (row * 4 + fragment) * 10;
            let cell = LABEL_CELLS[label_index(row, fragment)];
            patched[offset + 2..offset + 4]
                .copy_from_slice(&(174_u16 + row as u16 * 16).to_le_bytes());
            patched[offset + 4..offset + 6].copy_from_slice(&(cell.x as u16).to_le_bytes());
            patched[offset + 6..offset + 8].copy_from_slice(&(cell.y as u16).to_le_bytes());
            patched[offset + 8..offset + 10].copy_from_slice(&0_u16.to_le_bytes());
        }
    }
    let mut owned = vec![[allowed_range.start, allowed_range.end]];
    owned.extend(data_ranges.iter().map(|range| [range.start, range.end]));
    ensure!(
        changed_ranges_are_within(&difference_ranges(source_overlay, &patched), &owned,),
        "active-title builder escaped its source-owned block"
    );
    Ok((
        patched,
        PracticalExamActiveTitleBuilderReport {
            allowed_range,
            data_ranges,
            runtime_address: BUILDER_RUNTIME_ADDRESS,
            installed_instructions: instructions,
            typed_readback_verified: true,
        },
    ))
}

pub(in crate::mode_descendant_graphics) fn install_active_title_texture_cache(
    immutable_result_source: &[u8],
    result_base: &[u8],
    exam_texture: &[u8],
    cache: &PracticalExamActiveTitleCachePlan,
) -> Result<PracticalExamActiveTitleTextureBuild> {
    ensure!(
        immutable_result_source.len() == result_base.len() && cache.covers_all_term_titles(),
        "active-title result cache inputs are incomplete"
    );
    let mut patched = result_base.to_vec();
    let mut allowed_ranges = Vec::new();
    let mut cache_cells = Vec::with_capacity(cache.glyphs.len());
    for glyph in cache.glyphs.values() {
        ensure!(
            glyph.cell.x >= 512,
            "active-title cache glyph is outside the matched SIKEN10 page"
        );
        let result_cell = Cell {
            x: glyph.cell.x - 512,
            y: glyph.cell.y,
            width: glyph.cell.width,
            height: glyph.cell.height,
        };
        ensure!(
            read_indexed_cell_in_prefix(immutable_result_source, 0, result_cell)?
                .iter()
                .all(|pixel| *pixel == 0)
                && read_indexed_cell_in_prefix(result_base, 0, result_cell)?
                    == read_indexed_cell_in_prefix(immutable_result_source, 0, result_cell)?,
            "active-title cache target for {:?} is not untouched transparent source",
            glyph.character
        );
        ensure!(
            read_indexed_cell_in_prefix(exam_texture, PRACTICAL_EXAM_TIM_OFFSET, glyph.cell)?
                == glyph.pixels,
            "active-title cache producers disagree for {:?}",
            glyph.character
        );
        let write =
            write_indexed_cell_in_prefix_with_report(&mut patched, 0, result_cell, &glyph.pixels)?;
        ensure!(
            glyph.character == ' ' || write.changed_byte_count > 0,
            "active-title result cache changed no pixels for {:?}",
            glyph.character
        );
        allowed_ranges.extend(write.allowed_ranges);
        cache_cells.push(result_cell);
    }

    let source_pixels = crate::tim::read_4bpp_indexed_image_in_prefix(result_base, 0)?;
    let patched_pixels = crate::tim::read_4bpp_indexed_image_in_prefix(&patched, 0)?;
    for (index, (source, replacement)) in source_pixels
        .pixels
        .iter()
        .zip(&patched_pixels.pixels)
        .enumerate()
    {
        if source == replacement {
            continue;
        }
        let point = Cell {
            x: index % source_pixels.width,
            y: index / source_pixels.width,
            width: 1,
            height: 1,
        };
        ensure!(
            SAFE_RESULT_CACHE_BANDS
                .iter()
                .any(|band| crate::tim::cells_overlap(*band, point)),
            "active-title cache changed a pixel read by another result projection"
        );
    }
    let claims = DecodedDataClaim::from_effective_ranges(
        "mode-descendant:practical-1999:active-title-cache",
        "mirror term-title glyphs across the SIKEN10 and SIKEN2 producer transition",
        immutable_result_source,
        &patched,
        allowed_ranges,
    )?;
    ensure!(
        !claims.is_empty(),
        "active-title result cache produced no Expected Writes"
    );
    Ok(PracticalExamActiveTitleTextureBuild {
        decoded: patched,
        claims,
        cache_cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    #[ignore = "requires the supported original disc"]
    fn term_hud_rebinds_every_digit_without_replacing_native_primitive_scheduling() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let disc = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (_, source) = disc.read_record("DAT1/SIKEN2.BIN").unwrap();
        {
            let (patched, report) = install_active_title_builder(&source).unwrap();
            assert_eq!(&patched[0xa00..0xa14], &source[0xa00..0xa14]);
            for index in 0..3 {
                let offset = 0xa28 + index * 8;
                let descriptor = &patched[offset..offset + 8];
                assert_eq!(&descriptor[3..5], &source[offset + 3..offset + 5]);
                assert_eq!(&descriptor[..3], &[12, 0, 3]);
            }
            for offset in (BUILDER_OFFSET..BUILDER_OFFSET + BUILDER_SIZE).step_by(4) {
                if source[offset..offset + 4] == patched[offset..offset + 4] {
                    continue;
                }
                let address = OVERLAY_RUNTIME_BASE + offset as u32;
                let instruction = psx_r3000a::decode(
                    u32::from_le_bytes(patched[offset..offset + 4].try_into().unwrap()),
                    address,
                )
                .unwrap();
                assert!(matches!(
                    instruction,
                    Instruction::Addiu {
                        rs: Register::ZERO | Register::S6,
                        ..
                    }
                ));
                assert!(
                    [
                        0x527c, 0x528c, 0x52b0, 0x52c0, 0x53ec, 0x53f4, 0x5b54, 0x5c14, 0x5c44,
                        0x5d28, 0x5df4, 0x5e04, 0x5e2c, 0x5e9c
                    ]
                    .contains(&offset)
                );
            }
            assert!(report.typed_readback_verified);
        }
        for offset in [0x50bc, 0x51a4, 0x53ec, 0xa00, 0xa28, 0x3c, 0x5e9c] {
            let mut drift = source.clone();
            drift[offset] ^= 1;
            assert!(install_active_title_builder(&drift).is_err());
        }
    }
}
