//! Composes writers that contribute to the same pair of decoded records.

use anyhow::{Result, ensure};

use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::write_scope::{changed_ranges_are_within, merge_byte_ranges};

struct PlannedCandidate {
    owner: String,
    candidate: Vec<u8>,
    claims: Vec<DecodedDataClaim>,
}

#[derive(Debug)]
pub(crate) struct PairedDecodedRecords {
    pub(crate) first: Vec<u8>,
    pub(crate) second: Vec<u8>,
}

pub(crate) struct PairedDecodedRecordPlan {
    first_target: String,
    first_source: Vec<u8>,
    first_candidates: Vec<PlannedCandidate>,
    second_target: String,
    second_source: Vec<u8>,
    second_candidates: Vec<PlannedCandidate>,
    paired_writer_count: usize,
}

impl PairedDecodedRecordPlan {
    pub(crate) fn new(
        first_target: impl Into<String>,
        first_source: Vec<u8>,
        second_target: impl Into<String>,
        second_source: Vec<u8>,
    ) -> Result<Self> {
        let first_target = first_target.into();
        let second_target = second_target.into();
        ensure!(
            !first_target.trim().is_empty() && !second_target.trim().is_empty(),
            "paired decoded record target is empty"
        );
        ensure!(
            !first_source.is_empty() && !second_source.is_empty(),
            "paired decoded record source is empty"
        );
        Ok(Self {
            first_target,
            first_source,
            first_candidates: Vec::new(),
            second_target,
            second_source,
            second_candidates: Vec::new(),
            paired_writer_count: 0,
        })
    }

    pub(crate) fn register_first_candidate(
        &mut self,
        owner: &str,
        candidate: Vec<u8>,
        claims: Vec<DecodedDataClaim>,
    ) -> Result<()> {
        let candidate = prepare_candidate(
            &self.first_target,
            &self.first_source,
            owner,
            candidate,
            claims,
        )?;
        self.first_candidates.push(candidate);
        Ok(())
    }

    pub(crate) fn register_second_candidate(
        &mut self,
        owner: &str,
        candidate: Vec<u8>,
        claims: Vec<DecodedDataClaim>,
    ) -> Result<()> {
        let candidate = prepare_candidate(
            &self.second_target,
            &self.second_source,
            owner,
            candidate,
            claims,
        )?;
        self.second_candidates.push(candidate);
        Ok(())
    }

    pub(crate) fn register_writer<FirstWriter, SecondWriter>(
        &mut self,
        owner: &str,
        first_writer: FirstWriter,
        second_writer: SecondWriter,
    ) -> Result<()>
    where
        FirstWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
        SecondWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
    {
        ensure!(!owner.trim().is_empty(), "paired record writer is unnamed");

        let mut first_candidate = self.first_source.clone();
        let first_ranges = first_writer(&mut first_candidate)?;
        let first_claims = claims_from_ranges(owner, &self.first_target, "first", first_ranges);
        let first_candidate = prepare_candidate(
            &self.first_target,
            &self.first_source,
            owner,
            first_candidate,
            first_claims,
        )?;

        let mut second_candidate = self.second_source.clone();
        let second_ranges = second_writer(&mut second_candidate)?;
        let second_claims = claims_from_ranges(owner, &self.second_target, "second", second_ranges);
        let second_candidate = prepare_candidate(
            &self.second_target,
            &self.second_source,
            owner,
            second_candidate,
            second_claims,
        )?;

        // Add the pair only after both candidates have been validated. A failed
        // second surface must not leave a half-registered writer behind.
        self.first_candidates.push(first_candidate);
        self.second_candidates.push(second_candidate);
        self.paired_writer_count += 1;
        Ok(())
    }

    pub(crate) fn compose(self) -> Result<PairedDecodedRecords> {
        ensure!(
            self.paired_writer_count > 0,
            "paired decoded record plan has no paired writer"
        );
        let first = compose_record(
            &self.first_target,
            &self.first_source,
            &self.first_candidates,
        )?;
        let second = compose_record(
            &self.second_target,
            &self.second_source,
            &self.second_candidates,
        )?;
        Ok(PairedDecodedRecords { first, second })
    }
}

fn claims_from_ranges(
    owner: &str,
    target: &str,
    surface: &str,
    ranges: Vec<[usize; 2]>,
) -> Vec<DecodedDataClaim> {
    DecodedDataClaim::from_ranges(
        &format!("paired:{surface}:{owner}"),
        &format!("apply {owner} to {target}"),
        merge_byte_ranges(ranges),
    )
}

fn prepare_candidate(
    target: &str,
    source: &[u8],
    owner: &str,
    candidate: Vec<u8>,
    mut claims: Vec<DecodedDataClaim>,
) -> Result<PlannedCandidate> {
    ensure!(!owner.trim().is_empty(), "{target} candidate is unnamed");
    ensure!(
        candidate.len() == source.len(),
        "{owner} changed {target} length from {} to {} bytes",
        source.len(),
        candidate.len()
    );
    ensure!(!claims.is_empty(), "{owner} claimed no {target} ranges");
    for claim in &claims {
        ensure!(
            !claim.id.trim().is_empty()
                && !claim.purpose.trim().is_empty()
                && claim.range.start < claim.range.end
                && claim.range.end <= source.len(),
            "{owner} claimed an invalid {target} range"
        );
    }

    let changed = difference_ranges(source, &candidate);
    let allowed = claims
        .iter()
        .map(|claim| [claim.range.start, claim.range.end])
        .collect::<Vec<_>>();
    ensure!(!changed.is_empty(), "{owner} changed no {target} bytes");
    ensure!(
        changed_ranges_are_within(&changed, &allowed),
        "{owner} changed {target} bytes outside its claimed ranges"
    );

    claims.retain(|claim| source[claim.range.clone()] != candidate[claim.range.clone()]);
    ensure!(
        !claims.is_empty(),
        "{owner} has no effective {target} claims"
    );
    Ok(PlannedCandidate {
        owner: owner.to_string(),
        candidate,
        claims,
    })
}

fn compose_record(target: &str, source: &[u8], candidates: &[PlannedCandidate]) -> Result<Vec<u8>> {
    let source_sha256 = sha256_bytes(source);
    let mut plan = DecodedRecordWritePlan::new(target, source, &source_sha256)?;
    for candidate in candidates {
        plan.register_data_candidate(
            &candidate.owner,
            &source_sha256,
            &candidate.candidate,
            &candidate.claims,
        )?;
    }
    plan.apply(None)
}

#[cfg(test)]
#[path = "paired_decoded_record_plan_tests.rs"]
mod tests;
