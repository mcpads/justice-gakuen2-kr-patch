use std::ops::Range;

use anyhow::Result;
use expected_write::WriteIntent;
use psx_r3000a::Instruction;

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedDataClaim, DecodedRecordWritePlan,
};
use crate::psx_machine_code_sources::PsxMachineCodeSources;

pub(super) struct NameEntryRecordContribution<'a> {
    pub(super) owner: &'a str,
    pub(super) candidate: &'a [u8],
    pub(super) claims: Vec<NameEntryRecordClaim>,
}

pub(super) enum NameEntryRecordClaim {
    Data(DecodedDataClaim),
    MachineCode {
        id: String,
        purpose: String,
        range: Range<usize>,
        runtime_address: u32,
        instructions: Vec<Instruction>,
    },
}

pub(super) fn effective_data_claims(
    id_prefix: &str,
    purpose: &str,
    source: &[u8],
    candidate: &[u8],
    ranges: impl IntoIterator<Item = [usize; 2]>,
) -> Result<Vec<NameEntryRecordClaim>> {
    Ok(
        DecodedDataClaim::from_effective_ranges(id_prefix, purpose, source, candidate, ranges)?
            .into_iter()
            .map(NameEntryRecordClaim::Data)
            .collect(),
    )
}

pub(super) fn compose_name_entry_record(
    target: &str,
    source: &[u8],
    source_sha256: &str,
    contributions: Vec<NameEntryRecordContribution<'_>>,
) -> Result<Vec<u8>> {
    let mut plan = DecodedRecordWritePlan::new(target, source, source_sha256)?;
    let mut machine_code_sources = PsxMachineCodeSources::default();
    for contribution in contributions {
        let mut claims = Vec::with_capacity(contribution.claims.len());
        for claim in contribution.claims {
            claims.push(match claim {
                NameEntryRecordClaim::Data(claim) => CandidateWriteClaim::from(&claim),
                NameEntryRecordClaim::MachineCode {
                    id,
                    purpose,
                    range,
                    runtime_address,
                    instructions,
                } => {
                    let provenance =
                        machine_code_sources.register(id.clone(), runtime_address, instructions)?;
                    CandidateWriteClaim {
                        id,
                        purpose,
                        range,
                        intent: WriteIntent::MachineCode(provenance),
                    }
                }
            });
        }
        plan.register_candidate(CandidateRecordWrite {
            owner: contribution.owner,
            source_sha256,
            candidate: contribution.candidate,
            claims,
        })?;
    }
    plan.apply(Some(&machine_code_sources))
}
