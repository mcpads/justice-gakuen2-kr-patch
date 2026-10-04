use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::name_input::NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN;
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;

pub(in crate::dialogue_audit) const STORAGE_OFFSET: usize = 0x00dc;
pub(in crate::dialogue_audit) const TABLE_ADDRESS: u32 =
    NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN + STORAGE_OFFSET as u32;
pub(in crate::dialogue_audit) const DATA_BYTES: usize = 20;

pub(in crate::dialogue_audit) fn control_data() -> Vec<u8> {
    // Native field order is nickname, family, given. Every entry is a control
    // followed by a terminator, never a pointer to tagged character words.
    // Retain the reserved extent without borrowing the adjacent repair helper.
    [0x3001_2002u32, 0x3001_2000, 0x3001_2001, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileNameControlsReport {
    pub table_address: String,
    pub data_sha256: String,
    pub native_geometry_records_preserved: bool,
    pub family_and_given_use_tagged_name_consumer: bool,
    pub nickname_uses_tagged_name_consumer: bool,
}

pub(super) fn register_profile_name_controls(
    source: &[u8],
    source_sha256: &str,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine: &mut PsxMachineCodeSources,
) -> Result<ProfileNameControlsReport> {
    ensure!(
        sha256_bytes(&source[0x13d34..0x13d78])
            == "b246ebd0d0c68ff7683f97ee8258a182c2594f81d79dfbc1270a32e5968cb780",
        "profile name texture-loader binding changed"
    );
    ensure!(
        sha256_bytes(&source[0x1c44..0x1c74])
            == "fca5375a5004c508ecac1bd0fc362a23696d0d008c065e882714c93827125ca0",
        "profile name records or geometry changed"
    );
    let sites = [
        (
            0x800b5d48,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
            Instruction::Lui {
                rt: Register::AT,
                immediate: ((TABLE_ADDRESS + 0x8000) >> 16) as u16,
            },
        ),
        (
            0x800b5d50,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::AT,
                offset: 0x3c50,
            },
            // AT already includes field * 4. Pass the inline control record,
            // not the word stored there, to the native text dispatcher.
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::AT,
                immediate: TABLE_ADDRESS as i16,
            },
        ),
    ];
    let mut candidate = source.to_vec();
    let mut claims = Vec::new();
    for (address, original, replacement) in sites {
        let offset = (address - 0x800a2000) as usize;
        ensure!(
            decode(
                u32::from_le_bytes(source[offset..offset + 4].try_into()?),
                address
            )? == original,
            "profile name loader instruction changed"
        );
        if original == replacement {
            continue;
        }
        candidate[offset..offset + 4]
            .copy_from_slice(&encode(&replacement, address)?.to_le_bytes());
        let id = format!("mgame:profile-name:control-table:{address:08x}");
        let provenance = machine.register(id.clone(), address, vec![replacement])?;
        claims.push(CandidateWriteClaim {
            id,
            purpose: "route full profile names through native name controls".to_string(),
            range: offset..offset + 4,
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: "profile name texture controls",
        source_sha256,
        candidate: &candidate,
        claims,
    })?;
    Ok(ProfileNameControlsReport {
        table_address: format!("0x{TABLE_ADDRESS:08x}"),
        data_sha256: sha256_bytes(&control_data()),
        native_geometry_records_preserved: true,
        family_and_given_use_tagged_name_consumer: true,
        nickname_uses_tagged_name_consumer: true,
    })
}
