use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeCheck, MachineCodeVerifier, RegionKind, WriteIntent,
};
use psx_r3000a::{Instruction, PROFILE_ID, Register, encode_le_bytes};

use super::{PsxMachineCodeSources, verify_r3000a_load_delays};

const SOURCE_ID: &str = "test/canonical-reassembly";
const ORIGIN: u32 = 0x8001_0000;

#[test]
fn decoded_reassembly_uses_canonical_instructions_not_replacement_bytes() {
    let instructions = vec![
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 7,
        },
        Instruction::J { target: ORIGIN },
    ];
    let bytes = encode_le_bytes(&instructions, ORIGIN).unwrap();
    let mut verifier = PsxMachineCodeSources::default();
    let provenance = verifier.register(SOURCE_ID, ORIGIN, instructions).unwrap();
    assert_eq!(provenance.isa_profile_id, PROFILE_ID);
    let region = ImageRegion {
        id: "code".to_string(),
        range: 0..bytes.len(),
        kind: RegionKind::MachineCode,
        reason: "verify canonical R3000A reassembly".to_string(),
    };
    let write = ExpectedWrite {
        id: "write".to_string(),
        owner: "test".to_string(),
        purpose: "verify canonical R3000A reassembly".to_string(),
        offset: 0,
        expected_original: vec![0; bytes.len()],
        replacement: bytes.clone(),
        intent: WriteIntent::MachineCode(provenance.clone()),
    };
    let baseline = vec![0; bytes.len()];
    let check = MachineCodeCheck {
        region: &region,
        write: &write,
        provenance: &provenance,
        baseline: &baseline,
    };
    let decoded = verifier.disassemble(&check).unwrap();

    let unrelated_replacement = ExpectedWrite {
        replacement: vec![0xff; bytes.len()],
        ..write
    };
    let changed_check = MachineCodeCheck {
        region: &region,
        write: &unrelated_replacement,
        provenance: &provenance,
        baseline: &baseline,
    };
    assert_eq!(
        verifier.assemble_decoded(&changed_check, &decoded).unwrap(),
        bytes
    );

    let mut altered = decoded;
    altered[0].canonical.replace_range(..8, "ffffffff");
    assert!(verifier.assemble_decoded(&changed_check, &altered).is_err());
}

#[test]
fn project_machine_code_rejects_an_immediate_r3000a_load_use() {
    let instructions = vec![
        Instruction::Lhu {
            rt: Register::T0,
            base: Register::A0,
            offset: 0,
        },
        Instruction::Addu {
            rd: Register::V0,
            rs: Register::T0,
            rt: Register::ZERO,
        },
    ];

    let error = verify_r3000a_load_delays(&instructions, ORIGIN, "test program")
        .unwrap_err()
        .to_string();
    assert!(error.contains("R3000A load-delay instruction"));

    let mut verifier = PsxMachineCodeSources::default();
    assert!(
        verifier
            .register("test/load-delay-hazard", ORIGIN, instructions)
            .is_err()
    );
}

#[test]
fn independent_instruction_satisfies_the_r3000a_load_delay() {
    let instructions = vec![
        Instruction::Lhu {
            rt: Register::T0,
            base: Register::A0,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 2,
        },
        Instruction::Addu {
            rd: Register::V0,
            rs: Register::T0,
            rt: Register::ZERO,
        },
    ];

    verify_r3000a_load_delays(&instructions, ORIGIN, "test program").unwrap();
}
