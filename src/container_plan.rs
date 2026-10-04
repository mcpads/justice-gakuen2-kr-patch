//! Seals member contributions into one immutable-source container output.

use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::pipeline::{difference_ranges, sha256_bytes};

pub(crate) struct ContainerMemberSpec {
    pub(crate) id: String,
    pub(crate) writable_range: Range<usize>,
    pub(crate) slot_range: Range<usize>,
    pub(crate) slot_start_alignment: usize,
    pub(crate) expected_source_sha256: String,
}

pub(crate) struct MemberContribution<'a> {
    pub(crate) member_id: &'a str,
    pub(crate) owner: &'a str,
    pub(crate) purpose: &'a str,
    pub(crate) consumed_source_sha256: &'a str,
    pub(crate) candidate: &'a [u8],
    pub(crate) lineage_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainerMemberReport {
    pub(crate) id: String,
    pub(crate) writable_range: Range<usize>,
    pub(crate) slot_range: Range<usize>,
    pub(crate) owner: Option<String>,
    pub(crate) lineage_id: Option<String>,
    pub(crate) source_sha256: String,
    pub(crate) candidate_sha256: String,
    pub(crate) changed_byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainerReport {
    pub(crate) source_sha256: String,
    pub(crate) candidate_sha256: String,
    pub(crate) changed_byte_count: usize,
    pub(crate) members: Vec<ContainerMemberReport>,
}

pub(crate) struct ContainerOutput {
    pub(crate) owner: String,
    pub(crate) path: String,
    pub(crate) purpose: String,
    pub(crate) data: Vec<u8>,
    pub(crate) report: ContainerReport,
}

struct RegisteredMemberContribution {
    owner: String,
    purpose: String,
    candidate: Vec<u8>,
    lineage_id: String,
    changed_ranges: Vec<Range<usize>>,
}

pub(crate) struct ContainerPlan<'a> {
    path: String,
    outer_owner: String,
    outer_purpose: String,
    source: &'a [u8],
    source_sha256: String,
    members: Vec<ContainerMemberSpec>,
    member_indices: BTreeMap<String, usize>,
    contributions: Vec<Option<RegisteredMemberContribution>>,
}

impl<'a> ContainerPlan<'a> {
    pub(crate) fn new(
        path: impl Into<String>,
        outer_owner: impl Into<String>,
        outer_purpose: impl Into<String>,
        source: &'a [u8],
        expected_source_sha256: &str,
        members: Vec<ContainerMemberSpec>,
    ) -> Result<Self> {
        let path = path.into();
        let outer_owner = outer_owner.into();
        let outer_purpose = outer_purpose.into();
        ensure!(
            !path.trim().is_empty()
                && !outer_owner.trim().is_empty()
                && !outer_purpose.trim().is_empty(),
            "container path or outer ownership is empty"
        );
        ensure!(
            !members.is_empty(),
            "container {path} has no member specification"
        );
        let source_sha256 = sha256_bytes(source);
        ensure!(
            source_sha256 == expected_source_sha256,
            "container {path} source identity changed: found {source_sha256}"
        );

        let mut member_indices = BTreeMap::new();
        for (index, member) in members.iter().enumerate() {
            ensure!(
                !member.id.trim().is_empty()
                    && valid_range(&member.writable_range, source.len())
                    && valid_range(&member.slot_range, source.len())
                    && contains(&member.slot_range, &member.writable_range),
                "container {path} has an invalid member specification"
            );
            ensure!(
                member.slot_start_alignment != 0
                    && member
                        .slot_range
                        .start
                        .is_multiple_of(member.slot_start_alignment),
                "container {path} member {} slot is not aligned to 0x{:x}",
                member.id,
                member.slot_start_alignment
            );
            ensure!(
                member_indices.insert(member.id.clone(), index).is_none(),
                "container {path} has duplicate member {}",
                member.id
            );
            let source_member = source
                .get(member.writable_range.clone())
                .context("container member is outside its immutable source")?;
            ensure!(
                sha256_bytes(source_member) == member.expected_source_sha256,
                "container {path} member {} source identity changed",
                member.id
            );
            for previous in members.iter().take(index) {
                ensure!(
                    !ranges_overlap(&member.slot_range, &previous.slot_range),
                    "container {path} member slots {} and {} overlap",
                    previous.id,
                    member.id
                );
            }
        }

        let contributions = (0..members.len()).map(|_| None).collect();
        Ok(Self {
            path,
            outer_owner,
            outer_purpose,
            source,
            source_sha256,
            members,
            member_indices,
            contributions,
        })
    }

    pub(crate) fn register_member(&mut self, contribution: MemberContribution<'_>) -> Result<()> {
        ensure!(
            !contribution.owner.trim().is_empty()
                && !contribution.purpose.trim().is_empty()
                && !contribution.lineage_id.trim().is_empty(),
            "container {} has an unnamed member contribution",
            self.path
        );
        let index = *self
            .member_indices
            .get(contribution.member_id)
            .with_context(|| {
                format!(
                    "container {} has no member {}",
                    self.path, contribution.member_id
                )
            })?;
        let member = &self.members[index];
        ensure!(
            contribution.consumed_source_sha256 == member.expected_source_sha256,
            "container {} member {} contribution consumed the wrong source",
            self.path,
            member.id
        );
        ensure!(
            contribution.candidate.len() == member.writable_range.len(),
            "container {} member {} candidate changed extent from {} to {} bytes",
            self.path,
            member.id,
            member.writable_range.len(),
            contribution.candidate.len()
        );
        ensure!(
            self.contributions[index].is_none(),
            "container {} member {} has more than one owner",
            self.path,
            member.id
        );
        let source_member = &self.source[member.writable_range.clone()];
        let changed_ranges = difference_ranges(source_member, contribution.candidate)
            .into_iter()
            .map(|[start, end]| start..end)
            .collect::<Vec<_>>();
        ensure!(
            !changed_ranges.is_empty(),
            "container {} member {} contribution changes no bytes",
            self.path,
            member.id
        );
        self.contributions[index] = Some(RegisteredMemberContribution {
            owner: contribution.owner.to_string(),
            purpose: contribution.purpose.to_string(),
            candidate: contribution.candidate.to_vec(),
            lineage_id: contribution.lineage_id.to_string(),
            changed_ranges,
        });
        Ok(())
    }

    pub(crate) fn seal(self) -> Result<Option<ContainerOutput>> {
        if self.contributions.iter().all(Option::is_none) {
            return Ok(None);
        }
        let mut write_plan = WritePlan::new();
        for (index, range) in protected_ranges(self.source.len(), &self.members)
            .into_iter()
            .enumerate()
        {
            write_plan.regions.push(ImageRegion {
                id: format!("{}:protected:{index}", self.path),
                range,
                kind: RegionKind::Protected,
                reason: "container header, metadata, gap, slot padding, or trailer".to_string(),
            });
        }
        for member in &self.members {
            write_plan.regions.push(ImageRegion {
                id: format!("{}:member:{}", self.path, member.id),
                range: member.writable_range.clone(),
                kind: RegionKind::Data,
                reason: format!("stored bytes for container member {}", member.id),
            });
        }

        let mut planned_changed_ranges = Vec::new();
        for (member, contribution) in self.members.iter().zip(&self.contributions) {
            let Some(contribution) = contribution else {
                continue;
            };
            write_plan.writes.push(ExpectedWrite {
                id: format!("{}:member-write:{}", self.path, member.id),
                owner: contribution.owner.clone(),
                purpose: contribution.purpose.clone(),
                offset: member.writable_range.start,
                expected_original: self.source[member.writable_range.clone()].to_vec(),
                replacement: contribution.candidate.clone(),
                intent: WriteIntent::Data,
            });
            for relative_range in &contribution.changed_ranges {
                planned_changed_ranges.push(
                    member.writable_range.start + relative_range.start
                        ..member.writable_range.start + relative_range.end,
                );
            }
        }
        let planned_changed_ranges = merge_ranges(planned_changed_ranges);
        let data = write_plan
            .apply(self.source, None)
            .with_context(|| format!("failed to seal container {}", self.path))?;
        let actual_changed_ranges = difference_ranges(self.source, &data)
            .into_iter()
            .map(|[start, end]| start..end)
            .collect::<Vec<_>>();
        ensure!(
            actual_changed_ranges == planned_changed_ranges,
            "container {} final diff differs from its member contributions",
            self.path
        );

        let members = self
            .members
            .into_iter()
            .zip(self.contributions)
            .map(|(member, contribution)| {
                let source_member = &self.source[member.writable_range.clone()];
                let candidate_member = &data[member.writable_range.clone()];
                let (owner, lineage_id, changed_byte_count) = contribution
                    .map(|contribution| {
                        (
                            Some(contribution.owner),
                            Some(contribution.lineage_id),
                            contribution.changed_ranges.iter().map(Range::len).sum(),
                        )
                    })
                    .unwrap_or((None, None, 0));
                ContainerMemberReport {
                    id: member.id,
                    writable_range: member.writable_range,
                    slot_range: member.slot_range,
                    owner,
                    lineage_id,
                    source_sha256: sha256_bytes(source_member),
                    candidate_sha256: sha256_bytes(candidate_member),
                    changed_byte_count,
                }
            })
            .collect::<Vec<_>>();
        let report = ContainerReport {
            source_sha256: self.source_sha256,
            candidate_sha256: sha256_bytes(&data),
            changed_byte_count: actual_changed_ranges.iter().map(Range::len).sum(),
            members,
        };
        Ok(Some(ContainerOutput {
            owner: self.outer_owner,
            path: self.path,
            purpose: self.outer_purpose,
            data,
            report,
        }))
    }
}

fn protected_ranges(byte_count: usize, members: &[ContainerMemberSpec]) -> Vec<Range<usize>> {
    let mut writable = members
        .iter()
        .map(|member| member.writable_range.clone())
        .collect::<Vec<_>>();
    writable.sort_by_key(|range| range.start);
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    for range in writable {
        if cursor < range.start {
            ranges.push(cursor..range.start);
        }
        cursor = range.end;
    }
    if cursor < byte_count {
        ranges.push(cursor..byte_count);
    }
    ranges
}

fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(previous) = merged.last_mut()
            && range.start == previous.end
        {
            previous.end = range.end;
        } else {
            merged.push(range);
        }
    }
    merged
}

fn valid_range(range: &Range<usize>, byte_count: usize) -> bool {
    range.start < range.end && range.end <= byte_count
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
#[path = "container_plan_tests.rs"]
mod tests;
