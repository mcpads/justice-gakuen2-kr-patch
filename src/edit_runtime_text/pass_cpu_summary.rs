//! CPU summary uses KANRI's name renderer through its exported entry. Share
//! the same fixed unit alias as the separate status screen, retaining digits.
use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;

pub(super) const UNIT_OFFSET: usize = 0x6278;
const BASE: u32 = 0x8017_a000;
const CONSUMER_HASH: &str = "8aa10654587f59b5e62281bf3fb15a85d84a3e50af487881485d47f8010b2699";

pub(super) fn install(source: &[u8], kanri: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        sha256_bytes(
            source
                .get(0x624c..0x62a8)
                .context("truncated CPU summary consumer")?
        ) == CONSUMER_HASH,
        "PASS CPU summary digit lookup or KANRI renderer call changed"
    );
    ensure!(
        kanri.get(4..8) == Some(0x800a_3f74u32.to_le_bytes().as_slice()),
        "PASS summary no longer calls the KANRI name renderer"
    );
    let instruction = Instruction::Addiu {
        rt: Register::V0,
        rs: Register::ZERO,
        immediate: super::kanri_name_runtime::KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE as i16,
    };
    let origin = BASE + UNIT_OFFSET as u32;
    let mut candidate = source.to_vec();
    candidate[UNIT_OFFSET..UNIT_OFFSET + 4]
        .copy_from_slice(&psx_r3000a::encode(&instruction, origin)?.to_le_bytes());
    let mut sources = PsxMachineCodeSources::default();
    let provenance = sources.register("pass-cpu-summary-unit", origin, vec![instruction])?;
    let hash = sha256_bytes(source);
    let mut plan = DecodedRecordWritePlan::new("DAT1/PASS.BIN", source, &hash)?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "CPU summary spirit-gauge unit",
        source_sha256: &hash,
        candidate: &candidate,
        claims: vec![CandidateWriteClaim {
            id: "pass-cpu-summary-unit".into(),
            purpose: "use the shared Korean unit without changing the numeric value or layout"
                .into(),
            range: UNIT_OFFSET..UNIT_OFFSET + 4,
            intent: WriteIntent::MachineCode(provenance),
        }],
    })?;
    plan.apply(Some(&sources))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the private original disc; emits no ROM"]
    fn source_cpu_summary_preserves_digit_lookup_and_calls_shared_unit_alias() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let disc = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (_, pass) = disc.read_record(super::super::source::PASS_PATH).unwrap();
        let (_, kanri) = disc
            .read_record(super::super::source::OVERLAY_PATH)
            .unwrap();
        let output = install(&pass, &kanri).unwrap();
        assert_eq!(&output[..UNIT_OFFSET], &pass[..UNIT_OFFSET]);
        assert_eq!(&output[UNIT_OFFSET + 4..], &pass[UNIT_OFFSET + 4..]);
        let code = u16::from_le_bytes(output[UNIT_OFFSET..UNIT_OFFSET + 2].try_into().unwrap());
        assert_eq!(
            code,
            super::super::kanri_name_runtime::KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE
        );
        let mut drifted = pass.clone();
        drifted[0x6290] ^= 1;
        assert!(install(&drifted, &kanri).is_err());
        let mut wrong_renderer = kanri;
        wrong_renderer[4] ^= 1;
        assert!(install(&pass, &wrong_renderer).is_err());
    }
}
