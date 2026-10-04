//! Resolves project-owned R3000A programs for Expected Write verification.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{
    DecodedInstruction, MachineCodeCheck, MachineCodeProvenance, MachineCodeVerifier,
    MachineCodeVerifierError,
};
use psx_r3000a::{
    Instruction, Location, PROFILE_ID, PsxR3000A, decode, encode_le_bytes, verify_placed_program,
};
use typed_isa_core::{AccessKind, StaticSemantics};

#[derive(Default)]
pub(crate) struct PsxMachineCodeSources {
    sources: BTreeMap<String, PsxMachineCodeSource>,
}

struct PsxMachineCodeSource {
    runtime_address: u32,
    instructions: Vec<Instruction>,
}

impl PsxMachineCodeSources {
    pub(crate) fn register(
        &mut self,
        id: impl Into<String>,
        runtime_address: u32,
        instructions: Vec<Instruction>,
    ) -> Result<MachineCodeProvenance> {
        let id = id.into();
        ensure!(
            !id.trim().is_empty() && !instructions.is_empty(),
            "R3000A assembly source is unnamed or empty"
        );
        verify_r3000a_load_delays(&instructions, runtime_address, &id)?;
        ensure!(
            self.sources
                .insert(
                    id.clone(),
                    PsxMachineCodeSource {
                        runtime_address,
                        instructions,
                    },
                )
                .is_none(),
            "duplicate R3000A assembly source {id}"
        );
        Ok(MachineCodeProvenance {
            assembly_source_id: id,
            isa_profile_id: PROFILE_ID.to_string(),
        })
    }

    fn resolve(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<&PsxMachineCodeSource, MachineCodeVerifierError> {
        if check.provenance.isa_profile_id != PROFILE_ID {
            return Err(MachineCodeVerifierError::new(format!(
                "unsupported machine-code profile {}",
                check.provenance.isa_profile_id
            )));
        }
        self.sources
            .get(&check.provenance.assembly_source_id)
            .ok_or_else(|| {
                MachineCodeVerifierError::new(format!(
                    "unknown R3000A assembly source {}",
                    check.provenance.assembly_source_id
                ))
            })
    }
}

pub(crate) fn verify_r3000a_load_delays(
    instructions: &[Instruction],
    origin: u32,
    role: &str,
) -> Result<()> {
    for (index, pair) in instructions.windows(2).enumerate() {
        let loaded_register = match pair[0] {
            Instruction::Lb { rt, .. }
            | Instruction::Lbu { rt, .. }
            | Instruction::Lh { rt, .. }
            | Instruction::Lhu { rt, .. }
            | Instruction::Lw { rt, .. }
            | Instruction::Mfc0 { rt, .. }
            | Instruction::Mfc2 { rt, .. }
            | Instruction::Cfc2 { rt, .. } => rt,
            _ => continue,
        };
        let delay_instruction_address = origin
            .checked_add(u32::try_from((index + 1) * 4)?)
            .context("R3000A load-delay instruction address overflow")?;
        let semantics = PsxR3000A::semantics(&pair[1], &delay_instruction_address)
            .with_context(|| format!("failed to inspect {role} load-delay instruction"))?;
        let reads_loaded_register = semantics.location_accesses.iter().any(|access| {
            access.kind == AccessKind::Read && access.location == Location::Gpr(loaded_register)
        });
        ensure!(
            !reads_loaded_register,
            "{role} consumes {loaded_register:?} in the R3000A load-delay instruction at \
             0x{delay_instruction_address:08x}: {:?} then {:?}",
            pair[0],
            pair[1],
        );
    }
    Ok(())
}

impl MachineCodeVerifier for PsxMachineCodeSources {
    fn assemble_source(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<Vec<u8>, MachineCodeVerifierError> {
        assemble(self.resolve(check)?)
    }

    fn disassemble(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<Vec<DecodedInstruction>, MachineCodeVerifierError> {
        let source = self.resolve(check)?;
        let decoded = verify_placed_program(&check.write.replacement, source.runtime_address)
            .map_err(|error| MachineCodeVerifierError::new(error.to_string()))?;
        Ok(decoded
            .iter()
            .zip(check.write.replacement.as_chunks::<4>().0)
            .enumerate()
            .map(|(index, (instruction, bytes))| {
                let word = u32::from_le_bytes(*bytes);
                DecodedInstruction {
                    offset: index * 4,
                    len: 4,
                    canonical: canonical_instruction(word, instruction),
                }
            })
            .collect())
    }

    fn assemble_decoded(
        &self,
        check: &MachineCodeCheck<'_>,
        decoded: &[DecodedInstruction],
    ) -> Result<Vec<u8>, MachineCodeVerifierError> {
        let source = self.resolve(check)?;
        let instructions = decoded
            .iter()
            .map(|instruction| decode_canonical(instruction, source.runtime_address))
            .collect::<Result<Vec<_>, _>>()?;
        encode_le_bytes(&instructions, source.runtime_address).map_err(machine_code_error)
    }
}

fn decode_canonical(
    decoded: &DecodedInstruction,
    origin: u32,
) -> Result<Instruction, MachineCodeVerifierError> {
    if decoded.len != 4 || !decoded.offset.is_multiple_of(4) {
        return Err(MachineCodeVerifierError::new(format!(
            "invalid R3000A instruction boundary at offset {:#x}",
            decoded.offset
        )));
    }
    let (word_hex, _) = decoded.canonical.split_once(':').ok_or_else(|| {
        MachineCodeVerifierError::new("canonical R3000A instruction has no typed identity")
    })?;
    if word_hex.len() != 8 {
        return Err(MachineCodeVerifierError::new(
            "canonical R3000A instruction word is not eight hexadecimal digits",
        ));
    }
    let word = u32::from_str_radix(word_hex, 16).map_err(machine_code_error)?;
    let offset = u32::try_from(decoded.offset).map_err(machine_code_error)?;
    let pc = origin.checked_add(offset).ok_or_else(|| {
        MachineCodeVerifierError::new("canonical R3000A instruction address overflow")
    })?;
    let instruction = decode(word, pc).map_err(machine_code_error)?;
    if canonical_instruction(word, &instruction) != decoded.canonical {
        return Err(MachineCodeVerifierError::new(format!(
            "canonical R3000A typed identity changed at offset {:#x}",
            decoded.offset
        )));
    }
    Ok(instruction)
}

fn canonical_instruction(word: u32, instruction: &Instruction) -> String {
    format!("{word:08x}:{instruction:?}")
}

fn machine_code_error(error: impl std::fmt::Display) -> MachineCodeVerifierError {
    MachineCodeVerifierError::new(error.to_string())
}

fn assemble(source: &PsxMachineCodeSource) -> Result<Vec<u8>, MachineCodeVerifierError> {
    encode_le_bytes(&source.instructions, source.runtime_address).map_err(machine_code_error)
}

#[cfg(test)]
#[path = "psx_machine_code_sources_tests.rs"]
mod tests;
