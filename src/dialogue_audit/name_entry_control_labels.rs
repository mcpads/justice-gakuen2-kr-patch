//! Preserve the native seven control sprites while showing an incomplete-slot cue.
//! The former nickname-only six-label path is retired; its space stays in this
//! renderer rather than extending the already packed shared name runtime.
use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_slot_navigation_guard::current_slot_validator_address;
use crate::pipeline::sha256_bytes;
use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, decode};

pub(super) const OFFSET: usize = 0x7bb8;
pub(super) const END: usize = 0x7ddc;
const SOURCE_SHA256: &str = "96f714c2ae09a568a9a15c4a09a9fba74b5a690fc6b8d791fb55a8e69f0dcc31";

pub(super) fn build_control_labels(source: &[u8]) -> Result<(Vec<u8>, Vec<Instruction>)> {
    let native = source
        .get(OFFSET..END)
        .context("control-label renderer truncated")?;
    ensure!(
        sha256_bytes(native) == SOURCE_SHA256,
        "control-label renderer source changed"
    );
    let mut a = Assembler::new();
    for offset in (OFFSET..END).step_by(4) {
        a.label(format!("native_{offset:x}"));
        if offset == 0x7bec {
            // The existing guard owns the definition of an unfinished slot.
            a.emit(Instruction::Jal {
                target: current_slot_validator_address(),
            })
            .emit(Instruction::Addu {
                rd: Register::A0,
                rs: Register::FP,
                rt: Register::ZERO,
            })
            .emit(Instruction::Sw {
                rt: Register::V0,
                base: Register::SP,
                offset: 0x14,
            })
            .emit(Instruction::Lui {
                rt: Register::S3,
                immediate: 0x801f,
            });
        }
        if matches!(
            offset,
            0x7bec
                | 0x7bf0
                | 0x7bf4
                | 0x7c0c
                | 0x7c10
                | 0x7c14
                | 0x7c18
                | 0x7c1c
                | 0x7c20
                | 0x7c24
                | 0x7c28
                | 0x7c2c
                | 0x7c30
                | 0x7c34
                | 0x7d94
                | 0x7d98
        ) {
            continue;
        }
        let instruction = decode(
            u32::from_le_bytes(source[offset..offset + 4].try_into()?),
            OVERLAY_RUNTIME_BASE + offset as u32,
        )?;
        // S3 is callee-saved and no longer needed for the packet+8 temporary.
        if matches!(
            instruction,
            Instruction::Lui {
                immediate: 0x801f,
                ..
            }
        ) {
            continue;
        }
        if offset == 0x7d24 {
            // Only the first (Hangul) label changes, regardless of active page.
            // A valid slot or any subsequent label retains its original UV.
            a.emit(Instruction::Lw {
                rt: Register::T1,
                base: Register::SP,
                offset: 0x14,
            })
            .emit(Instruction::Addiu {
                rt: Register::T0,
                rs: Register::S6,
                immediate: -1,
            })
            .emit(Instruction::Or {
                rd: Register::T1,
                rs: Register::T1,
                rt: Register::T0,
            })
            .bne(Register::T1, Register::ZERO, "keep_label_uv")
            .emit(Instruction::nop())
            .emit(Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 192,
            })
            .label("keep_label_uv");
        }
        match instruction {
            Instruction::Lw {
                rt,
                offset: displacement,
                ..
            } if matches!(displacement, 0x608c | 0x6360 | 0x6370 | 0x6090) => {
                a.emit(Instruction::Lw {
                    rt,
                    base: Register::S3,
                    offset: displacement,
                });
            }
            _ if offset == 0x7cd0 => {
                a.emit(Instruction::nop());
            }
            _ if offset == 0x7ce0 => {
                a.emit(Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::S0,
                    immediate: 8,
                });
            }
            _ if offset == 0x7d5c => {
                a.emit(Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::S0,
                    immediate: 8,
                });
            }
            _ if offset == 0x7d9c => {
                a.emit(Instruction::Sltiu {
                    rt: Register::V0,
                    rs: Register::S6,
                    immediate: 7,
                });
            }
            Instruction::Bne { rs, rt, target } => {
                a.bne(
                    rs,
                    rt,
                    format!("native_{:x}", target - OVERLAY_RUNTIME_BASE),
                );
            }
            other => {
                a.emit(other);
            }
        }
    }
    let program = a.assemble(OVERLAY_RUNTIME_BASE + OFFSET as u32)?;
    ensure!(
        program.bytes().len() <= END - OFFSET,
        "control-label renderer exceeds owned region"
    );
    let mut bytes = program.bytes().to_vec();
    let mut instructions = program.instructions().to_vec();
    while bytes.len() < END - OFFSET {
        bytes.extend_from_slice(&[0; 4]);
        instructions.push(Instruction::nop());
    }
    Ok((bytes, instructions))
}

/// Keep the authored cue and the native sprite consumer bound at build time.
pub(super) fn validate_hint_graphic(
    reports: &[super::name_entry_fixed_graphics_model::DialogueNameEntryFixedGraphicBuildReport],
) -> Result<()> {
    let cue = reports
        .iter()
        .filter(|r| r.surface_id == "candidate-controls")
        .flat_map(|r| &r.entries)
        .find(|e| e.id == "incomplete_slot_hint")
        .context("incomplete-slot cue graphic missing")?;
    ensure!(
        cue.cell.x == 704 && cue.cell.y == 200 && cue.cell.width == 48 && cue.cell.height == 24,
        "incomplete-slot cue no longer fits the native control-label sprite"
    );
    ensure!(
        cue.korean_text == "모음 입력",
        "incomplete-slot cue must explain vowel completion"
    );
    Ok(())
}
