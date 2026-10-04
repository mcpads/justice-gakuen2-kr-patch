use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register, encode};

use crate::pipeline::difference_ranges;

use super::consumer::{
    CHANGED_INSTRUCTION_OFFSETS, OVERLAY_RUNTIME_BASE, TEXTURE_PAGE_INSTRUCTION_OFFSET,
    validate_output_stock_label_consumer, validate_stock_label_consumer,
};
use super::ownership::validate_allocated_glyph_ownership;

#[derive(Debug)]
pub(super) struct PatchedStockLabelOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
}

pub(super) fn patch_stock_label_overlay(source: &[u8]) -> Result<PatchedStockLabelOverlay> {
    validate_stock_label_consumer(source)?;
    validate_allocated_glyph_ownership(source)?;

    let mut bytes = source.to_vec();
    let output_instructions = [
        (
            0x66e0,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            0x66e8,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            TEXTURE_PAGE_INSTRUCTION_OFFSET,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x02c0,
            },
        ),
    ];
    for (offset, instruction) in output_instructions {
        let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32)?;
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    let expected_write_ranges = CHANGED_INSTRUCTION_OFFSETS
        .map(|offset| [offset, offset + 4])
        .to_vec();
    let changed_byte_ranges = difference_ranges(source, &bytes);
    ensure!(
        !changed_byte_ranges.is_empty()
            && changed_byte_ranges.iter().all(|[start, end]| {
                expected_write_ranges
                    .iter()
                    .any(|[allowed_start, allowed_end]| {
                        allowed_start <= start && end <= allowed_end
                    })
            }),
        "stock-label overlay changed bytes outside its typed selector instructions"
    );
    validate_output_stock_label_consumer(&bytes)?;

    Ok(PatchedStockLabelOverlay {
        bytes,
        expected_write_ranges,
        changed_byte_ranges,
    })
}

pub(super) fn apply_stock_label_overlay(overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
    let patched = patch_stock_label_overlay(overlay)?;
    ensure!(
        patched.bytes.len() == overlay.len(),
        "stock-label overlay patch changed record length"
    );
    overlay.copy_from_slice(&patched.bytes);
    Ok(patched.expected_write_ranges)
}
