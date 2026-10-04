use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::psx_static_analysis::control_flow::{FlowTarget, flow_successors};
use crate::source_disc::LoadedImage;

pub(super) fn ensure_instruction(
    image: &LoadedImage,
    offset: usize,
    expected: Instruction,
    role: &str,
) -> Result<()> {
    let actual = decode_at(image, offset)?;
    ensure!(
        actual == expected,
        "{} selector-state {role} changed at +0x{offset:x}: expected {expected:?}, found {actual:?}",
        image.path
    );
    Ok(())
}

pub(super) fn ensure_register_is_preserved_between(
    image: &LoadedImage,
    start: usize,
    end: usize,
    register: Register,
    role: &str,
) -> Result<()> {
    for offset in (start..end).step_by(4) {
        let instruction = decode_at(image, offset)?;
        ensure!(
            instruction.written_gpr() != Some(register),
            "{} selector-state {role} register changes at +0x{offset:x}",
            image.path
        );
    }
    Ok(())
}

pub(super) fn ensure_no_other_reachable_direct_entry(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    expected_transfer_offset: usize,
    target_runtime_address: u32,
    role: &str,
) -> Result<()> {
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[expected_transfer_offset],
        target_runtime_address,
        role,
    )
}

pub(super) fn ensure_only_reachable_direct_entries(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    expected_transfer_offsets: &[usize],
    target_runtime_address: u32,
    role: &str,
) -> Result<()> {
    for &offset in reachable_instruction_offsets {
        let instruction = decode_at(image, offset)?;
        if matches!(
            flow_successors(&instruction, runtime_address(image, offset)?).target,
            Some(FlowTarget::Direct(target)) if target == target_runtime_address
        ) {
            ensure!(
                expected_transfer_offsets.contains(&offset),
                "{} selector-state {role} has another reachable direct entry at +0x{offset:x}",
                image.path
            );
        }
    }
    Ok(())
}

pub(super) fn ensure_direct_call_offsets(
    image: &LoadedImage,
    target_runtime_address: u32,
    expected_call_offsets: &[usize],
    role: &str,
) -> Result<()> {
    let actual_call_offsets = (0..image.data.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            decode_at(image, *offset).ok()
                == Some(Instruction::Jal {
                    target: target_runtime_address,
                })
        })
        .collect::<BTreeSet<_>>();
    let expected_call_offset_set = expected_call_offsets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        expected_call_offset_set.len() == expected_call_offsets.len(),
        "{} selector-state {role} repeats an expected direct caller offset",
        image.path
    );
    ensure!(
        actual_call_offsets == expected_call_offset_set,
        "{} selector-state {role} direct caller census changed: expected {expected_call_offset_set:?}, found {actual_call_offsets:?}",
        image.path
    );
    Ok(())
}

pub(super) fn decode_at(image: &LoadedImage, offset: usize) -> Result<Instruction> {
    let end = offset
        .checked_add(4)
        .context("selector-state instruction end overflow")?;
    let bytes = image
        .data
        .get(offset..end)
        .with_context(|| format!("{} selector-state instruction is truncated", image.path))?;
    let pc = runtime_address(image, offset)?;
    decode(u32::from_le_bytes(bytes.try_into()?), pc)
        .with_context(|| format!("failed to decode {} instruction at {pc:#010x}", image.path))
}

pub(super) fn runtime_address(image: &LoadedImage, offset: usize) -> Result<u32> {
    image
        .runtime_base
        .context("selector-state image lacks a runtime base")?
        .checked_add(u32::try_from(offset)?)
        .context("selector-state instruction address overflow")
}

#[cfg(test)]
mod tests {
    use psx_r3000a::{Instruction, encode};

    use super::*;

    const RUNTIME_BASE: u32 = 0x800a_2000;
    const TARGET: u32 = 0x800a_3000;

    #[test]
    fn complete_direct_call_census_rejects_an_unprofiled_caller() {
        let instructions = [
            Instruction::Jal { target: TARGET },
            Instruction::nop(),
            Instruction::Jal {
                target: 0x800a_4000,
            },
            Instruction::Jal { target: TARGET },
        ];
        let data = instructions
            .iter()
            .enumerate()
            .flat_map(|(index, instruction)| {
                encode(
                    instruction,
                    RUNTIME_BASE + u32::try_from(index * 4).unwrap(),
                )
                .unwrap()
                .to_le_bytes()
            })
            .collect();
        let image = LoadedImage {
            path: "DAT1/SYNTH.BIN".to_string(),
            data,
            runtime_base: Some(RUNTIME_BASE),
            entrypoints: Vec::new(),
        };

        ensure_direct_call_offsets(&image, TARGET, &[0, 12], "synthetic copy").unwrap();
        let error = ensure_direct_call_offsets(&image, TARGET, &[0], "synthetic copy")
            .unwrap_err()
            .to_string();

        assert!(error.contains("direct caller census changed"));
        assert!(error.contains("12"));
    }
}
