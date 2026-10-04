//! Raise idle text modulation in both Training menus, preserving selection logic.
use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register, encode_bytes};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;

pub(super) const IDLE_TEXT_SITES: [usize; 5] = [0x1498, 0x159c, 0x1608, 0x1660, 0x16b8];
pub(super) const IDLE_BRIGHTNESS: i16 = 0x68;

pub(super) fn apply(source: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        source
            .get(0x1410..0x1934)
            .is_some_and(|code| sha256_bytes(code)
                == "c0336b640021735018bf42425841d0ae2b6ea381f1de13d87976412139fae4b7"),
        "Training pause/settings modulation consumer changed"
    );
    let hash = sha256_bytes(source);
    let mut candidate = source.to_vec();
    let mut claims = Vec::new();
    let mut machine = PsxMachineCodeSources::default();
    for offset in IDLE_TEXT_SITES {
        let address = 0x800a2000 + offset as u32;
        let instruction = |immediate| Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate,
        };
        let expected = encode_bytes(&instruction(0x40), address)?;
        ensure!(
            source[offset..offset + 4] == expected,
            "Training idle brightness source changed"
        );
        let replacement = instruction(IDLE_BRIGHTNESS);
        candidate[offset..offset + 4].copy_from_slice(&encode_bytes(&replacement, address)?);
        let id = format!("training-idle-contrast:{offset:x}");
        let provenance = machine.register(id.clone(), address, vec![replacement])?;
        claims.push(CandidateWriteClaim {
            id,
            purpose: "keep idle text legible while retaining brighter selection".into(),
            range: offset..offset + 4,
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    let mut plan = DecodedRecordWritePlan::new("DAT1/TRAIN.BIN idle contrast", source, &hash)?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "Training menu contrast",
        source_sha256: &hash,
        candidate: &candidate,
        claims,
    })?;
    plan.apply(Some(&machine))
}
