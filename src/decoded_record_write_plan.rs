//! Composes independently produced changes against one immutable decoded record.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeVerifier, RegionKind, WriteIntent, WritePlan,
};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::write_scope::{byte_ranges_overlap, changed_ranges_are_within, merge_byte_ranges};

pub(crate) struct CandidateWriteClaim {
    pub(crate) id: String,
    pub(crate) purpose: String,
    pub(crate) range: Range<usize>,
    pub(crate) intent: WriteIntent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedDataClaim {
    pub(crate) id: String,
    pub(crate) purpose: String,
    pub(crate) range: Range<usize>,
}

impl DecodedDataClaim {
    pub(crate) fn from_ranges(
        id_prefix: &str,
        purpose: &str,
        ranges: impl IntoIterator<Item = [usize; 2]>,
    ) -> Vec<Self> {
        ranges
            .into_iter()
            .enumerate()
            .map(|(part, [start, end])| Self {
                id: format!("{id_prefix}:part-{part}"),
                purpose: purpose.to_string(),
                range: start..end,
            })
            .collect()
    }

    pub(crate) fn from_effective_ranges(
        id_prefix: &str,
        purpose: &str,
        source: &[u8],
        candidate: &[u8],
        ranges: impl IntoIterator<Item = [usize; 2]>,
    ) -> Result<Vec<Self>> {
        ensure!(
            source.len() == candidate.len(),
            "decoded claim candidate changed extent from {} to {} bytes",
            source.len(),
            candidate.len()
        );
        let ranges = ranges.into_iter().collect::<Vec<_>>();
        ensure!(
            ranges
                .iter()
                .all(|[start, end]| start < end && *end <= source.len()),
            "decoded claim range is invalid or outside its source"
        );
        let mut claims = Self::from_ranges(id_prefix, purpose, merge_byte_ranges(ranges));
        claims.retain(|claim| source[claim.range.clone()] != candidate[claim.range.clone()]);
        Ok(claims)
    }
}

impl From<&DecodedDataClaim> for CandidateWriteClaim {
    fn from(claim: &DecodedDataClaim) -> Self {
        Self {
            id: claim.id.clone(),
            purpose: claim.purpose.clone(),
            range: claim.range.clone(),
            intent: WriteIntent::Data,
        }
    }
}

pub(crate) struct CandidateRecordWrite<'a> {
    pub(crate) owner: &'a str,
    pub(crate) source_sha256: &'a str,
    pub(crate) candidate: &'a [u8],
    pub(crate) claims: Vec<CandidateWriteClaim>,
}

pub(crate) struct DecodedRecordWritePlan<'a> {
    target: &'a str,
    source: &'a [u8],
    source_sha256: String,
    claims: Vec<PlannedClaim>,
    claimed_ranges: BTreeMap<usize, (usize, String)>,
    claim_ids: BTreeSet<String>,
}

struct PlannedClaim {
    id: String,
    owner: String,
    purpose: String,
    range: Range<usize>,
    replacement: Vec<u8>,
    intent: WriteIntent,
}

impl<'a> DecodedRecordWritePlan<'a> {
    pub(crate) fn new(
        target: &'a str,
        source: &'a [u8],
        expected_source_sha256: &str,
    ) -> Result<Self> {
        ensure!(!target.trim().is_empty(), "decoded write target is empty");
        let actual = sha256_bytes(source);
        ensure!(
            actual == expected_source_sha256,
            "decoded write target {target} source identity changed: found {actual}"
        );
        Ok(Self {
            target,
            source,
            source_sha256: actual,
            claims: Vec::new(),
            claimed_ranges: BTreeMap::new(),
            claim_ids: BTreeSet::new(),
        })
    }

    pub(crate) fn register_candidate(&mut self, candidate: CandidateRecordWrite<'_>) -> Result<()> {
        ensure!(
            candidate.candidate.len() == self.source.len(),
            "{} candidate {} changed decoded extent",
            self.target,
            candidate.owner
        );
        ensure!(
            candidate.source_sha256 == self.source_sha256,
            "{} candidate {} consumed a different source identity",
            self.target,
            candidate.owner
        );
        ensure!(
            !candidate.owner.trim().is_empty() && !candidate.claims.is_empty(),
            "{} has an unnamed or empty candidate writer",
            self.target
        );
        let changed = difference_ranges(self.source, candidate.candidate);
        let allowed = candidate
            .claims
            .iter()
            .map(|claim| [claim.range.start, claim.range.end])
            .collect::<Vec<_>>();
        ensure!(
            !changed.is_empty() && changed_ranges_are_within(&changed, &allowed),
            "{} candidate {} changed bytes outside its claims",
            self.target,
            candidate.owner
        );

        let mut claimed_ranges = self.claimed_ranges.clone();
        let mut claim_ids = self.claim_ids.clone();
        let mut claims = Vec::with_capacity(candidate.claims.len());
        for claim in candidate.claims {
            ensure!(
                !claim.id.trim().is_empty()
                    && !claim.purpose.trim().is_empty()
                    && claim.range.start < claim.range.end
                    && claim.range.end <= self.source.len(),
                "{} candidate {} has an invalid claim",
                self.target,
                candidate.owner
            );
            ensure!(
                changed.iter().any(|range| {
                    byte_ranges_overlap([claim.range.start, claim.range.end], *range)
                }),
                "{} candidate {} claim {} changes no bytes",
                self.target,
                candidate.owner,
                claim.id
            );
            ensure!(
                claim_ids.insert(claim.id.clone()),
                "{} has duplicate candidate claim {}",
                self.target,
                claim.id
            );
            register_non_overlapping_range(self.target, &mut claimed_ranges, &claim)?;

            let replacement = candidate
                .candidate
                .get(claim.range.clone())
                .context("decoded record claim is outside its candidate")?
                .to_vec();
            claims.push(PlannedClaim {
                id: claim.id,
                owner: candidate.owner.to_string(),
                purpose: claim.purpose,
                range: claim.range,
                replacement,
                intent: claim.intent,
            });
        }
        self.claimed_ranges = claimed_ranges;
        self.claim_ids = claim_ids;
        self.claims.extend(claims);
        Ok(())
    }

    pub(crate) fn register_data_candidate(
        &mut self,
        owner: &str,
        source_sha256: &str,
        candidate: &[u8],
        claims: &[DecodedDataClaim],
    ) -> Result<()> {
        self.register_candidate(CandidateRecordWrite {
            owner,
            source_sha256,
            candidate,
            claims: claims.iter().map(CandidateWriteClaim::from).collect(),
        })
    }

    pub(crate) fn apply(
        self,
        machine_code_verifier: Option<&dyn MachineCodeVerifier>,
    ) -> Result<Vec<u8>> {
        let mut output = self.source.to_vec();
        for claim in &self.claims {
            let source = self
                .source
                .get(claim.range.clone())
                .context("decoded record claim is outside its immutable source")?;
            let local_range = 0..claim.range.len();
            let plan = WritePlan::new()
                .region(ImageRegion {
                    id: format!("{}:region", claim.id),
                    range: local_range.clone(),
                    kind: region_kind(&claim.intent),
                    reason: claim.purpose.clone(),
                })
                .write(ExpectedWrite {
                    id: claim.id.clone(),
                    owner: claim.owner.clone(),
                    purpose: claim.purpose.clone(),
                    offset: 0,
                    expected_original: source.to_vec(),
                    replacement: claim.replacement.clone(),
                    intent: claim.intent.clone(),
                });
            let replacement = plan.apply(source, machine_code_verifier).with_context(|| {
                format!(
                    "failed to verify decoded write {} for {}",
                    claim.id, self.target
                )
            })?;
            output[claim.range.clone()].copy_from_slice(&replacement);
        }
        let changed = difference_ranges(self.source, &output);
        let allowed = self
            .claims
            .iter()
            .map(|claim| [claim.range.start, claim.range.end])
            .collect::<Vec<_>>();
        ensure!(
            changed_ranges_are_within(&changed, &allowed),
            "failed to audit final decoded write plan for {}",
            self.target
        );
        Ok(output)
    }
}

fn register_non_overlapping_range(
    target: &str,
    claimed_ranges: &mut BTreeMap<usize, (usize, String)>,
    claim: &CandidateWriteClaim,
) -> Result<()> {
    if let Some((_, (previous_end, previous_id))) =
        claimed_ranges.range(..=claim.range.start).next_back()
    {
        ensure!(
            *previous_end <= claim.range.start,
            "{target} candidate claims overlap: {previous_id} and {}",
            claim.id
        );
    }
    if let Some((next_start, (_, next_id))) = claimed_ranges.range(claim.range.start..).next() {
        ensure!(
            *next_start >= claim.range.end,
            "{target} candidate claims overlap: {next_id} and {}",
            claim.id
        );
    }
    claimed_ranges.insert(claim.range.start, (claim.range.end, claim.id.clone()));
    Ok(())
}

fn region_kind(intent: &WriteIntent) -> RegionKind {
    match intent {
        WriteIntent::Data => RegionKind::Data,
        WriteIntent::Metadata => RegionKind::Metadata,
        WriteIntent::MachineCode(_) => RegionKind::MachineCode,
    }
}

#[cfg(test)]
#[path = "decoded_record_write_plan_tests.rs"]
mod tests;
