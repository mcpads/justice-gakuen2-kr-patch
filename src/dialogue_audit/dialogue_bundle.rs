use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::container_plan::{ContainerMemberSpec, ContainerPlan, MemberContribution};
use crate::disc::rebuild;
use crate::pipeline::sha256_bytes;

const BUNDLE_ALIGNMENT: usize = 0x800;
const BUNDLE_OWNER: &str = "dialogue bundle compositor";
const BUNDLE_PURPOSE: &str = "compose rebuilt dialogue images into one BZZ record";
const BUNDLE_MEMBER_SUFFIXES: [&str; 5] = ["T", "O", "P", "G", "J"];
const DIALOGUE_BUNDLE_PATHS: [&str; 5] = [
    "DAT2/MGG04.BZZ",
    "DAT2/MGG06.BZZ",
    "DAT2/MGG08.BZZ",
    "DAT2/MGG09.BZZ",
    "DAT2/MGG11.BZZ",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DialogueBundleSlot {
    pub offset: usize,
    pub byte_count: usize,
}

#[derive(Debug)]
pub(super) struct DialogueBundleMemberBuild {
    pub source_path: String,
    pub bundle_offset: usize,
    pub byte_count: usize,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub input_present: bool,
    pub owner: Option<String>,
    pub lineage_id: Option<String>,
    pub changed_byte_count: usize,
}

#[derive(Debug)]
pub(super) struct DialogueBundleBuild {
    pub path: String,
    pub source_byte_count: usize,
    pub owner: String,
    pub purpose: String,
    pub source_sha256: String,
    pub rebuilt_sha256: String,
    pub changed_byte_count: usize,
    pub data: Vec<u8>,
    pub members: Vec<DialogueBundleMemberBuild>,
}

pub(super) struct DialogueBundleMemberCandidate<'a> {
    pub source_path: &'a str,
    pub data: &'a [u8],
    pub source_stored_sha256: &'a str,
    pub candidate_stored_sha256: &'a str,
    pub candidate_decoded_sha256: &'a str,
    pub compression_roundtrip_verified: bool,
}

struct DialogueBundleMemberBinding {
    source_path: String,
    writable_range: std::ops::Range<usize>,
    slot_range: std::ops::Range<usize>,
    source_sha256: String,
}

pub(super) fn build_dialogue_bundles(
    source_bin: &Path,
    rebuilt_assets: &[DialogueBundleMemberCandidate<'_>],
) -> Result<Vec<DialogueBundleBuild>> {
    let mut replacements = BTreeMap::new();
    for asset in rebuilt_assets {
        ensure!(
            replacements.insert(asset.source_path, asset).is_none(),
            "duplicate dialogue bundle candidate {}",
            asset.source_path
        );
    }
    let mut bundles = Vec::with_capacity(DIALOGUE_BUNDLE_PATHS.len());
    for bundle_path in DIALOGUE_BUNDLE_PATHS {
        if let Some(bundle) = build_dialogue_bundle(source_bin, bundle_path, &replacements)? {
            bundles.push(bundle);
        }
    }
    Ok(bundles)
}

fn build_dialogue_bundle(
    source_bin: &Path,
    bundle_path: &str,
    replacements: &BTreeMap<&str, &DialogueBundleMemberCandidate<'_>>,
) -> Result<Option<DialogueBundleBuild>> {
    let (_, source_bundle) = rebuild::read_record(source_bin, bundle_path)?;
    let slots = parse_dialogue_bundle_slots(&source_bundle)
        .with_context(|| format!("failed to parse {bundle_path}"))?;
    ensure!(
        slots.len() == BUNDLE_MEMBER_SUFFIXES.len(),
        "{bundle_path} contains {} members instead of {}",
        slots.len(),
        BUNDLE_MEMBER_SUFFIXES.len()
    );

    let stem = bundle_path
        .strip_prefix("DAT2/")
        .and_then(|path| path.strip_suffix(".BZZ"))
        .context("dialogue bundle path does not match DAT2/*.BZZ")?;
    let member_paths = BUNDLE_MEMBER_SUFFIXES
        .iter()
        .map(|suffix| format!("DAT2/{stem}{suffix}.BIZ"))
        .collect::<Vec<_>>();
    let mut source_members = BTreeMap::new();
    for member_path in &member_paths {
        let (_, data) = rebuild::read_record(source_bin, member_path)?;
        source_members.insert(member_path.clone(), data);
    }

    let mut unused_members = member_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut bindings = Vec::with_capacity(slots.len());
    for (slot_index, slot) in slots.iter().enumerate() {
        let source_member = &source_bundle[slot.offset..slot.offset + slot.byte_count];
        let matching_paths = unused_members
            .iter()
            .filter(|path| {
                source_members
                    .get(*path)
                    .is_some_and(|candidate| candidate.as_slice() == source_member)
            })
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            matching_paths.len() == 1,
            "{bundle_path} member at 0x{:x} matched {} source BIZ records",
            slot.offset,
            matching_paths.len()
        );
        let member_path = matching_paths[0].clone();
        unused_members.remove(&member_path);
        let writable_range = slot.offset..slot.offset + slot.byte_count;
        let slot_end = slots
            .get(slot_index + 1)
            .map(|next| next.offset)
            .unwrap_or(source_bundle.len());
        ensure!(
            writable_range.end <= slot_end,
            "{bundle_path} member {member_path} exceeds its fixed slot"
        );
        bindings.push(DialogueBundleMemberBinding {
            source_path: member_path,
            writable_range,
            slot_range: slot.offset..slot_end,
            source_sha256: sha256_bytes(source_member),
        });
    }
    ensure!(
        unused_members.is_empty(),
        "{bundle_path} did not bind every source BIZ member"
    );

    let source_sha256 = sha256_bytes(&source_bundle);
    let specs = bindings
        .iter()
        .map(|binding| ContainerMemberSpec {
            id: binding.source_path.clone(),
            writable_range: binding.writable_range.clone(),
            slot_range: binding.slot_range.clone(),
            slot_start_alignment: BUNDLE_ALIGNMENT,
            expected_source_sha256: binding.source_sha256.clone(),
        })
        .collect();
    let mut plan = ContainerPlan::new(
        bundle_path,
        BUNDLE_OWNER,
        BUNDLE_PURPOSE,
        &source_bundle,
        &source_sha256,
        specs,
    )?;
    for binding in &bindings {
        let Some(candidate) = replacements.get(binding.source_path.as_str()) else {
            continue;
        };
        ensure!(
            candidate.compression_roundtrip_verified,
            "rebuilt {} has no stored-object compression readback",
            binding.source_path
        );
        ensure!(
            candidate.source_stored_sha256 == binding.source_sha256,
            "rebuilt {} consumed another stored source",
            binding.source_path
        );
        ensure!(
            sha256_bytes(candidate.data) == candidate.candidate_stored_sha256,
            "rebuilt {} stored candidate identity changed",
            binding.source_path
        );
        ensure!(
            candidate.data.len() == binding.writable_range.len(),
            "rebuilt {} changed size from {} to {} bytes",
            binding.source_path,
            binding.writable_range.len(),
            candidate.data.len()
        );
        let owner = format!("dialogue image producer: {}", binding.source_path);
        let purpose = format!("install rebuilt dialogue image {}", binding.source_path);
        let lineage_id = format!(
            "dialogue-font-stored-result:{}:{}:{}",
            binding.source_path,
            candidate.candidate_decoded_sha256,
            candidate.candidate_stored_sha256
        );
        plan.register_member(MemberContribution {
            member_id: &binding.source_path,
            owner: &owner,
            purpose: &purpose,
            consumed_source_sha256: &binding.source_sha256,
            candidate: candidate.data,
            lineage_id: &lineage_id,
        })?;
    }
    let Some(output) = plan.seal()? else {
        return Ok(None);
    };
    ensure!(
        output.owner == BUNDLE_OWNER && output.purpose == BUNDLE_PURPOSE,
        "{bundle_path} container ownership changed"
    );
    ensure!(
        output.report.source_sha256 == source_sha256
            && output.report.candidate_sha256 == sha256_bytes(&output.data),
        "{bundle_path} container identity report changed"
    );
    ensure!(
        parse_dialogue_bundle_slots(&output.data)? == slots,
        "{bundle_path} member table changed after composition"
    );
    ensure!(
        output.report.members.len() == bindings.len(),
        "{bundle_path} container member report is incomplete"
    );
    let mut members = Vec::with_capacity(bindings.len());
    for (binding, report) in bindings.iter().zip(&output.report.members) {
        ensure!(
            report.id == binding.source_path
                && report.writable_range == binding.writable_range
                && report.slot_range == binding.slot_range
                && report.source_sha256 == binding.source_sha256,
            "{bundle_path} container member report changed for {}",
            binding.source_path
        );
        let candidate = replacements.get(binding.source_path.as_str()).copied();
        let replacement = candidate
            .map(|candidate| candidate.data)
            .unwrap_or_else(|| &source_bundle[binding.writable_range.clone()]);
        ensure!(
            report.candidate_sha256 == sha256_bytes(replacement),
            "{bundle_path} container candidate changed for {}",
            binding.source_path
        );
        let source_member = &source_bundle[binding.writable_range.clone()];
        if replacement == source_member {
            ensure!(
                report.owner.is_none()
                    && report.lineage_id.is_none()
                    && report.changed_byte_count == 0,
                "{bundle_path} unchanged member acquired a producer for {}",
                binding.source_path
            );
        } else {
            ensure!(
                report.owner.is_some()
                    && report.lineage_id.is_some()
                    && report.changed_byte_count > 0,
                "{bundle_path} changed member lost its producer lineage for {}",
                binding.source_path
            );
        }
        members.push(DialogueBundleMemberBuild {
            source_path: binding.source_path.clone(),
            bundle_offset: binding.writable_range.start,
            byte_count: binding.writable_range.len(),
            source_sha256: report.source_sha256.clone(),
            replacement_sha256: report.candidate_sha256.clone(),
            input_present: candidate.is_some(),
            owner: report.owner.clone(),
            lineage_id: report.lineage_id.clone(),
            changed_byte_count: report.changed_byte_count,
        });
    }
    ensure!(
        output.report.changed_byte_count
            == output
                .report
                .members
                .iter()
                .map(|member| member.changed_byte_count)
                .sum::<usize>(),
        "{bundle_path} outer diff count differs from its member reports"
    );

    Ok(Some(DialogueBundleBuild {
        path: output.path,
        source_byte_count: source_bundle.len(),
        owner: output.owner,
        purpose: output.purpose,
        source_sha256: output.report.source_sha256,
        rebuilt_sha256: output.report.candidate_sha256,
        changed_byte_count: output.report.changed_byte_count,
        data: output.data,
        members,
    }))
}

pub(super) fn parse_dialogue_bundle_slots(data: &[u8]) -> Result<Vec<DialogueBundleSlot>> {
    ensure!(data.len() >= 8, "dialogue bundle header is truncated");
    let mut slots: Vec<DialogueBundleSlot> = Vec::new();
    let mut cursor = 0usize;
    loop {
        ensure!(
            cursor + 8 <= data.len(),
            "dialogue bundle header has no terminator"
        );
        let offset = read_u32(data, cursor)? as usize;
        let byte_count = read_u32(data, cursor + 4)? as usize;
        cursor += 8;
        if offset == 0 && byte_count == 0 {
            break;
        }
        ensure!(
            offset != 0 && byte_count != 0,
            "dialogue bundle has a partial null member"
        );
        ensure!(
            offset.is_multiple_of(BUNDLE_ALIGNMENT),
            "dialogue bundle member offset 0x{offset:x} is not 0x800-aligned"
        );
        let end = offset
            .checked_add(byte_count)
            .context("dialogue bundle member range overflow")?;
        ensure!(end <= data.len(), "dialogue bundle member is out of range");
        if let Some(previous) = slots.last() {
            ensure!(
                offset >= previous.offset + previous.byte_count,
                "dialogue bundle members overlap or are out of order"
            );
        }
        slots.push(DialogueBundleSlot { offset, byte_count });
    }
    ensure!(!slots.is_empty(), "dialogue bundle has no members");
    ensure!(
        cursor <= slots[0].offset,
        "dialogue bundle member overlaps its header"
    );
    Ok(slots)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .context("dialogue bundle u32 is truncated")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}
