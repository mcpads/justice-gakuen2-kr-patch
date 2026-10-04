use psx_r3000a::{Assembler, Instruction, Register};

use crate::source_disc::{LoadedImage, LoadedImageEntrypoint};

use super::name_companion_reference_audit::scan_loaded_source;

const RUNTIME_BASE: u32 = 0x800a_2000;

#[test]
fn separates_static_reads_and_writes_inside_the_companion_buffer() {
    let source = loaded_source(&[
        Instruction::Lui {
            rt: Register::T0,
            immediate: 0x801f,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x1886,
        },
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::T0,
            offset: 4,
        },
        Instruction::Sh {
            rt: Register::V0,
            base: Register::T0,
            offset: 6,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);

    let scan = scan_loaded_source(&source, 64).unwrap();

    assert_eq!(scan.candidates.len(), 2);
    assert_eq!(scan.candidates[0].operation, "read");
    assert_eq!(scan.candidates[0].access_runtime_addresses, ["0x801f188a"]);
    assert_eq!(scan.candidates[1].operation, "write");
    assert_eq!(scan.candidates[1].access_runtime_addresses, ["0x801f188c"]);
}

#[test]
fn ignores_memory_accesses_outside_the_companion_buffer() {
    let source = loaded_source(&[
        Instruction::Lui {
            rt: Register::T0,
            immediate: 0x801f,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x1896,
        },
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);

    let scan = scan_loaded_source(&source, 64).unwrap();

    assert!(scan.candidates.is_empty());
}

#[test]
fn ignores_companion_accesses_after_an_entrypoint_return() {
    let source = loaded_source(&[
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::T0,
            immediate: 0x801f,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x1886,
        },
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        },
    ]);

    let scan = scan_loaded_source(&source, 64).unwrap();

    assert!(scan.candidates.is_empty());
    assert_eq!(scan.seed_count, 0);
}

fn loaded_source(instructions: &[Instruction]) -> LoadedImage {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler.emit(instruction.clone());
    }
    let program = assembler.assemble(RUNTIME_BASE).unwrap();
    LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data: program.bytes().to_vec(),
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![LoadedImageEntrypoint {
            role: "primary",
            source_reference_kind: "fixture",
            source_reference_offset: 0,
            runtime_address: RUNTIME_BASE,
        }],
    }
}
