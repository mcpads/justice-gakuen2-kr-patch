use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::container_plan::{ContainerMemberSpec, ContainerPlan, MemberContribution};
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::paged_compression::{
    PagedCompressionProfile, compress_on_source_final_page_with_preserved_prefix,
    source_paged_compression_profile,
};
use crate::pipeline::{map_ordered_parallel, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;
use crate::tzz::{TzzMember, parse_tzz};

use super::embedded_catalogue;
use super::fixed_presentation::{build_fixed_presentation_contribution, family_owns_consumer};
use super::model::{
    DiarySceneFixedPresentationFamily, DiarySceneFontSources, DiarySceneRuntimeBundleBinding,
    DiarySceneRuntimeBundleBuild, DiarySceneRuntimeBundleReport, DiarySceneRuntimeMemberReport,
};

const TZZ_MEMBER_ALIGNMENT: usize = 0x800;
const RUNTIME_COMPRESSION_INPUT_PAGE_BYTES: usize = 0x800;
const RUNTIME_CONTAINER_OWNER: &str = "diary-scene runtime-bundle compositor";
const RUNTIME_CONTAINER_PURPOSE: &str = "compose propagated diary-scene runtime members";

struct RuntimeMemberPatch {
    bundle_index: usize,
    stored: Vec<u8>,
    report: DiarySceneRuntimeMemberReport,
}

pub(super) struct RuntimeMemberEncoding {
    pub(super) bytes: Vec<u8>,
    source_profile: PagedCompressionProfile,
    rebuilt_profile: PagedCompressionProfile,
    source_compression_prefix_preservation_required: bool,
    pub(super) compression_requirement_satisfied: bool,
    preserved_source_compressed_prefix_byte_count: usize,
    preserved_source_decoded_prefix_byte_count: usize,
}

pub(super) fn build_runtime_bundles(
    source_disc: &SupportedSourceDisc,
    bindings: &[DiarySceneRuntimeBundleBinding],
    source_catalogue_members: &[Vec<u8>],
    patched_catalogue_members: &[Vec<u8>],
    fixed_presentations: &[DiarySceneFixedPresentationFamily],
    fonts: &DiarySceneFontSources,
) -> Result<Vec<DiarySceneRuntimeBundleBuild>> {
    let changed_catalogue_indices = source_catalogue_members
        .iter()
        .zip(patched_catalogue_members)
        .enumerate()
        .filter_map(|(index, (source, patched))| (source != patched).then_some(index))
        .collect::<BTreeSet<_>>();

    let mut prepared_bundles = Vec::new();
    let mut member_tasks = Vec::new();
    for binding in bindings {
        let mut changed_member_indices =
            member_indices_with_producers(binding, &changed_catalogue_indices, fixed_presentations);
        let (_, source_bundle) = source_disc.read_record(&binding.path)?;
        ensure!(
            sha256_bytes(&source_bundle) == binding.archive_sha256,
            "diary scene runtime bundle {} source hash changed",
            binding.path
        );
        let source_slots = parse_tzz(&source_bundle)
            .with_context(|| format!("failed to parse runtime bundle {}", binding.path))?;
        ensure!(
            source_slots.len() == binding.member_count,
            "diary scene runtime bundle {} member count changed",
            binding.path
        );
        for slot in &source_slots {
            if changed_member_indices.contains(&slot.index) {
                continue;
            }
            let decoded = decompress(&source_bundle[slot.compressed_range()], false)?;
            if !embedded_catalogue::replacements(
                &decoded,
                source_catalogue_members,
                patched_catalogue_members,
            )?
            .is_empty()
            {
                changed_member_indices.push(slot.index);
            }
        }
        if changed_member_indices.is_empty() {
            continue;
        }
        changed_member_indices.sort_unstable();
        let bundle_index = prepared_bundles.len();
        member_tasks.extend(
            changed_member_indices
                .iter()
                .map(|member_index| [bundle_index, *member_index]),
        );
        prepared_bundles.push((binding, source_bundle, source_slots));
    }

    let member_patches = map_ordered_parallel(&member_tasks, |task| {
        let bundle_index = task[0];
        let member_index = task[1];
        let (binding, source_bundle, source_slots) = &prepared_bundles[bundle_index];
        let member_binding = &binding.members[member_index];
        let source_slot = &source_slots[member_binding.index];
        ensure!(
            source_slot.offset == member_binding.offset
                && source_slot.compressed_size == member_binding.compressed_size,
            "diary scene runtime member {} geometry changed in {}",
            member_binding.index,
            binding.path
        );
        let source_compressed = &source_bundle[source_slot.compressed_range()];
        ensure!(
            sha256_bytes(source_compressed) == member_binding.compressed_sha256,
            "diary scene runtime member {} changed in {}",
            member_binding.index,
            binding.path
        );
        let source_decoded = decompress(source_compressed, false)?;
        ensure!(
            source_decoded.len() == member_binding.decoded_size
                && sha256_bytes(&source_decoded) == member_binding.decoded_sha256,
            "diary scene runtime member {} decoded hash changed in {}",
            member_binding.index,
            binding.path
        );

        let changed_indices = member_binding
            .catalogue_member_indices
            .iter()
            .copied()
            .filter(|index| changed_catalogue_indices.contains(index))
            .collect::<Vec<_>>();
        let catalogue_index = changed_indices
            .first()
            .copied()
            .or_else(|| member_binding.catalogue_member_indices.first().copied())
            .context("diary scene runtime member lost its catalogue owner")?;
        let source_prefix = &source_catalogue_members[catalogue_index];
        let patched_prefix = &patched_catalogue_members[catalogue_index];
        ensure!(
            source_decoded.starts_with(source_prefix),
            "diary scene runtime member {} in {} does not start with catalogue member {}",
            member_binding.index,
            binding.path,
            catalogue_index
        );
        let target = format!("{}:member:{}", binding.path, member_binding.index);
        let mut decoded_plan =
            DecodedRecordWritePlan::new(&target, &source_decoded, &member_binding.decoded_sha256)?;
        let mut expected_decoded = source_decoded.clone();
        if !changed_indices.is_empty() {
            ensure!(
                source_prefix.len() == patched_prefix.len(),
                "diary scene runtime prefix changed extent"
            );
            ensure!(
                changed_indices.iter().all(|index| {
                    source_catalogue_members[*index] == *source_prefix
                        && patched_catalogue_members[*index] == *patched_prefix
                }),
                "ambiguous diary scene runtime owners produce different patched prefixes"
            );
            let mut prefix_candidate = source_decoded.clone();
            prefix_candidate[..patched_prefix.len()].copy_from_slice(patched_prefix);
            ensure!(
                prefix_candidate[source_prefix.len()..] == source_decoded[source_prefix.len()..],
                "diary scene runtime member {} prefix producer changed trailing decoded data in {}",
                member_binding.index,
                binding.path
            );
            let claims = DecodedDataClaim::from_effective_ranges(
                &format!(
                    "diary-scene-runtime-prefix:{}:{}",
                    binding.path, member_binding.index
                ),
                "propagate one translated diary-scene catalogue prefix",
                &source_decoded,
                &prefix_candidate,
                [[0, patched_prefix.len()]],
            )?;
            let owner =
                format!("diary-scene runtime-prefix producer: catalogue member {catalogue_index}");
            decoded_plan.register_data_candidate(
                &owner,
                &member_binding.decoded_sha256,
                &prefix_candidate,
                &claims,
            )?;
            apply_claimed_ranges(&mut expected_decoded, &prefix_candidate, &claims);
        }

        let embedded_replacements = embedded_catalogue::replacements(
            &source_decoded,
            source_catalogue_members,
            patched_catalogue_members,
        )?;
        for &(offset, index) in &embedded_replacements {
            let replacement = &patched_catalogue_members[index];
            let end = offset + replacement.len();
            let mut candidate = source_decoded.clone();
            candidate[offset..end].copy_from_slice(replacement);
            let claims = DecodedDataClaim::from_effective_ranges(
                &format!("diary-scene-embedded:{index}:{offset}"),
                "propagate a complete source-matched embedded catalogue image",
                &source_decoded,
                &candidate,
                [[offset, end]],
            )?;
            decoded_plan.register_data_candidate(
                &format!("diary-scene embedded catalogue {index} at {offset:#x}"),
                &member_binding.decoded_sha256,
                &candidate,
                &claims,
            )?;
            apply_claimed_ranges(&mut expected_decoded, &candidate, &claims);
        }

        let mut fixed_presentation_family_ids = Vec::new();
        let mut translated_fixed_presentation_region_ids = Vec::new();
        let mut preserved_fixed_presentation_region_ids = Vec::new();
        for family in fixed_presentations
            .iter()
            .filter(|family| family_owns_consumer(family, &binding.path, member_binding.index))
        {
            let contribution =
                build_fixed_presentation_contribution(&source_decoded, family, fonts)?;
            let owner = format!(
                "diary-scene fixed-presentation producer: {}",
                contribution.family_id
            );
            decoded_plan.register_data_candidate(
                &owner,
                &member_binding.decoded_sha256,
                &contribution.candidate,
                &contribution.claims,
            )?;
            apply_claimed_ranges(
                &mut expected_decoded,
                &contribution.candidate,
                &contribution.claims,
            );
            fixed_presentation_family_ids.push(contribution.family_id);
            translated_fixed_presentation_region_ids.extend(contribution.translated_region_ids);
            preserved_fixed_presentation_region_ids.extend(contribution.preserved_region_ids);
        }
        ensure!(
            !changed_indices.is_empty()
                || !fixed_presentation_family_ids.is_empty()
                || !embedded_replacements.is_empty(),
            "diary scene runtime member has no effective producer"
        );
        let patched_decoded = decoded_plan.apply(None)?;
        ensure!(
            patched_decoded == expected_decoded,
            "diary scene runtime member {} plan omitted a decoded contribution in {}",
            member_binding.index,
            binding.path
        );

        let encoding = encode_runtime_member(source_compressed, &source_decoded, &patched_decoded)
            .with_context(|| {
                format!(
                    "failed to encode diary scene runtime member {} in {}",
                    member_binding.index, binding.path
                )
            })?;
        let reencoded = encoding.bytes;
        ensure!(
            reencoded.len() <= source_slot.compressed_size,
            "rebuilt diary scene runtime member {} exceeds {} bytes in {}",
            member_binding.index,
            source_slot.compressed_size,
            binding.path
        );
        let mut stored = vec![0; source_slot.compressed_size];
        stored[..reencoded.len()].copy_from_slice(&reencoded);
        ensure!(
            decompress(&stored, true)? == patched_decoded,
            "patched diary scene runtime member {} in {} decoded incorrectly",
            member_binding.index,
            binding.path
        );
        let patched_compressed_sha256 = sha256_bytes(&stored);
        Ok(RuntimeMemberPatch {
            bundle_index,
            stored,
            report: DiarySceneRuntimeMemberReport {
                index: member_binding.index,
                catalogue_member_indices: member_binding.catalogue_member_indices.clone(),
                embedded_catalogue_offsets: embedded_replacements
                    .iter()
                    .map(|(offset, _)| *offset)
                    .collect(),
                offset: source_slot.offset,
                slot_byte_count: source_slot.slot_size,
                source_compressed_size: source_slot.compressed_size,
                rebuilt_compressed_size: reencoded.len(),
                source_compression_maximum_match_words: encoding.source_profile.maximum_match_words,
                rebuilt_compression_maximum_match_words: encoding
                    .rebuilt_profile
                    .maximum_match_words,
                source_compression_maximum_control_block_output_words: encoding
                    .source_profile
                    .maximum_control_block_output_words,
                rebuilt_compression_maximum_control_block_output_words: encoding
                    .rebuilt_profile
                    .maximum_control_block_output_words,
                source_compression_control_blocks_crossing_input_pages: encoding
                    .source_profile
                    .control_blocks_crossing_input_pages,
                rebuilt_compression_control_blocks_crossing_input_pages: encoding
                    .rebuilt_profile
                    .control_blocks_crossing_input_pages,
                source_compression_final_input_page: compression_final_input_page(
                    encoding.source_profile,
                ),
                rebuilt_compression_final_input_page: compression_final_input_page(
                    encoding.rebuilt_profile,
                ),
                source_compression_prefix_preservation_required: encoding
                    .source_compression_prefix_preservation_required,
                compression_requirement_satisfied: encoding.compression_requirement_satisfied,
                source_compressed_sha256: member_binding.compressed_sha256.clone(),
                source_decoded_sha256: member_binding.decoded_sha256.clone(),
                patched_compressed_sha256,
                patched_decoded_sha256: sha256_bytes(&patched_decoded),
                catalogue_prefix_byte_count: source_prefix.len(),
                preserved_source_compressed_prefix_byte_count: encoding
                    .preserved_source_compressed_prefix_byte_count,
                preserved_source_decoded_prefix_byte_count: encoding
                    .preserved_source_decoded_prefix_byte_count,
                changed: true,
                trailing_decoded_bytes_preserved: fixed_presentation_family_ids.is_empty()
                    && embedded_replacements.is_empty(),
                unclaimed_decoded_bytes_preserved: true,
                fixed_presentation_family_ids,
                translated_fixed_presentation_region_ids,
                preserved_fixed_presentation_region_ids,
            },
        })
    })?;

    compose_runtime_bundles(prepared_bundles, member_patches)
}

pub(super) fn encode_runtime_member(
    source_compressed: &[u8],
    source_decoded: &[u8],
    patched_decoded: &[u8],
) -> Result<RuntimeMemberEncoding> {
    ensure!(
        source_decoded.len() == patched_decoded.len(),
        "rebuilt diary scene member changed its decoded extent"
    );
    let source_profile = source_paged_compression_profile(source_compressed)?;
    let unchanged_decoded_prefix_byte_count = source_decoded
        .iter()
        .zip(patched_decoded)
        .take_while(|(source, patched)| source == patched)
        .count()
        & !1;
    let (bytes, prefix) = compress_on_source_final_page_with_preserved_prefix(
        patched_decoded,
        source_compressed,
        source_profile,
        unchanged_decoded_prefix_byte_count,
    )?;
    let preserved_source_compressed_prefix_byte_count = prefix.encoded_byte_count;
    let preserved_source_decoded_prefix_byte_count = prefix.decoded_byte_count;
    let rebuilt_profile = source_paged_compression_profile(&bytes)?;
    // The native loader continues through the source-sized slot after an early
    // terminator. Padding would then decode over the last output block, including
    // embedded portraits, regardless of which presentation producer changed it.
    let compression_requirement_satisfied = compression_final_input_page(rebuilt_profile)
        == compression_final_input_page(source_profile)
        && rebuilt_profile.maximum_match_words <= source_profile.maximum_match_words
        && rebuilt_profile.maximum_control_block_output_words
            <= source_profile.maximum_control_block_output_words
        && preserved_source_compressed_prefix_byte_count > 0
        && preserved_source_decoded_prefix_byte_count > 0
        && bytes[..preserved_source_compressed_prefix_byte_count]
            == source_compressed[..preserved_source_compressed_prefix_byte_count];
    ensure!(
        compression_requirement_satisfied,
        "rebuilt diary scene runtime stream exceeds its source compression contract"
    );
    Ok(RuntimeMemberEncoding {
        bytes,
        source_profile,
        rebuilt_profile,
        source_compression_prefix_preservation_required: true,
        compression_requirement_satisfied,
        preserved_source_compressed_prefix_byte_count,
        preserved_source_decoded_prefix_byte_count,
    })
}

fn compression_final_input_page(profile: PagedCompressionProfile) -> usize {
    (profile.stream_byte_count - 1) / RUNTIME_COMPRESSION_INPUT_PAGE_BYTES
}

fn member_indices_with_producers(
    binding: &DiarySceneRuntimeBundleBinding,
    changed_catalogue_indices: &BTreeSet<usize>,
    fixed_presentations: &[DiarySceneFixedPresentationFamily],
) -> Vec<usize> {
    binding
        .members
        .iter()
        .enumerate()
        .filter_map(|(member_index, member)| {
            (member
                .catalogue_member_indices
                .iter()
                .any(|index| changed_catalogue_indices.contains(index))
                || fixed_presentations
                    .iter()
                    .any(|family| family_owns_consumer(family, &binding.path, member.index)))
            .then_some(member_index)
        })
        .collect()
}

fn apply_claimed_ranges(output: &mut [u8], candidate: &[u8], claims: &[DecodedDataClaim]) {
    for claim in claims {
        output[claim.range.clone()].copy_from_slice(&candidate[claim.range.clone()]);
    }
}

fn compose_runtime_bundles(
    prepared_bundles: Vec<(&DiarySceneRuntimeBundleBinding, Vec<u8>, Vec<TzzMember>)>,
    member_patches: Vec<RuntimeMemberPatch>,
) -> Result<Vec<DiarySceneRuntimeBundleBuild>> {
    let mut builds = Vec::with_capacity(prepared_bundles.len());
    let mut next_member_patch = 0;
    for (bundle_index, (binding, source_bundle, source_slots)) in
        prepared_bundles.into_iter().enumerate()
    {
        let specs = source_slots
            .iter()
            .zip(&binding.members)
            .map(|(slot, member)| {
                ensure!(
                    slot.index == member.index
                        && slot.offset == member.offset
                        && slot.compressed_size == member.compressed_size,
                    "diary scene runtime member {} geometry changed in {}",
                    member.index,
                    binding.path
                );
                Ok(ContainerMemberSpec {
                    id: runtime_member_id(member.index),
                    writable_range: slot.compressed_range(),
                    slot_range: slot.slot_range(),
                    slot_start_alignment: TZZ_MEMBER_ALIGNMENT,
                    expected_source_sha256: member.compressed_sha256.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut plan = ContainerPlan::new(
            &binding.path,
            RUNTIME_CONTAINER_OWNER,
            RUNTIME_CONTAINER_PURPOSE,
            &source_bundle,
            &binding.archive_sha256,
            specs,
        )?;
        let mut member_reports = Vec::new();
        while let Some(member_patch) = member_patches.get(next_member_patch)
            && member_patch.bundle_index == bundle_index
        {
            let member_id = runtime_member_id(member_patch.report.index);
            let owner = format!(
                "diary-scene runtime-member producer: {}:{}",
                binding.path, member_patch.report.index
            );
            let purpose = format!(
                "store propagated diary-scene runtime member {}",
                member_patch.report.index
            );
            let lineage_id = format!(
                "diary-scene-runtime-stored-result:{}:{}:{}:{}",
                binding.path,
                member_patch.report.index,
                member_patch.report.patched_decoded_sha256,
                member_patch.report.patched_compressed_sha256
            );
            plan.register_member(MemberContribution {
                member_id: &member_id,
                owner: &owner,
                purpose: &purpose,
                consumed_source_sha256: &member_patch.report.source_compressed_sha256,
                candidate: &member_patch.stored,
                lineage_id: &lineage_id,
            })?;
            member_reports.push(member_patch.report.clone());
            next_member_patch += 1;
        }
        let output = plan.seal()?.with_context(|| {
            format!(
                "diary scene runtime bundle {} changed no bytes",
                binding.path
            )
        })?;
        ensure!(
            output.path == binding.path
                && output.owner == RUNTIME_CONTAINER_OWNER
                && output.purpose == RUNTIME_CONTAINER_PURPOSE
                && output.report.source_sha256 == binding.archive_sha256
                && output.report.candidate_sha256 == sha256_bytes(&output.data),
            "diary scene runtime bundle {} container identity changed",
            binding.path
        );
        ensure!(
            output
                .report
                .members
                .iter()
                .filter(|member| member.changed_byte_count > 0)
                .count()
                == member_reports.len(),
            "diary scene runtime bundle {} contribution count changed",
            binding.path
        );
        ensure!(
            parse_tzz(&output.data)? == source_slots,
            "diary scene runtime bundle {} member table changed",
            binding.path
        );
        let report = DiarySceneRuntimeBundleReport {
            path: binding.path.clone(),
            source_size: source_bundle.len(),
            source_sha256: binding.archive_sha256.clone(),
            patched_sha256: sha256_bytes(&output.data),
            changed: true,
            members: member_reports,
            source_members_start_with_catalogue_member: true,
            changes_confined_to_owned_members: true,
        };
        builds.push(DiarySceneRuntimeBundleBuild {
            path: binding.path.clone(),
            data: output.data,
            report,
        });
    }
    ensure!(
        next_member_patch == member_patches.len(),
        "diary scene runtime member patches were not all assembled"
    );
    Ok(builds)
}

fn runtime_member_id(index: usize) -> String {
    format!("runtime-member-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diary_scene::model::DiarySceneFixedPresentationConsumer;

    fn binding() -> DiarySceneRuntimeBundleBinding {
        DiarySceneRuntimeBundleBinding {
            path: "DAT2/MGBGK04.BZZ".into(),
            archive_sha256: "archive".into(),
            member_count: 1,
            members: vec![super::super::model::DiarySceneRuntimeMemberBinding {
                index: 2,
                offset: 0,
                compressed_size: 1,
                compressed_sha256: "compressed".into(),
                decoded_size: 1,
                decoded_sha256: "decoded".into(),
                catalogue_member_indices: vec![37],
            }],
        }
    }

    fn fixed_presentation(path: &str, member_index: usize) -> DiarySceneFixedPresentationFamily {
        DiarySceneFixedPresentationFamily {
            kind: "justice_gakuen2_diary_scene_fixed_presentation".into(),
            id: "exam-feedback".into(),
            source_decoded_sha256: "decoded".into(),
            tim_offset: 0,
            source_tim_sha256: "tim".into(),
            team_up_names_path: None,
            team_up_names_sha256: None,
            transparent_index: 0,
            consumers: vec![DiarySceneFixedPresentationConsumer {
                path: path.into(),
                member_index,
            }],
            translated_regions: Vec::new(),
            preserved_regions: Vec::new(),
            preserved_remainder: None,
        }
    }

    #[test]
    fn fixed_presentation_schedules_member_when_catalogue_prefix_is_unchanged() {
        let binding = binding();

        assert!(
            member_indices_with_producers(&binding, &BTreeSet::new(), &[]).is_empty(),
            "an unchanged member without another producer should stay untouched"
        );
        assert_eq!(
            member_indices_with_producers(
                &binding,
                &BTreeSet::new(),
                &[fixed_presentation("DAT2/MGBGK04.BZZ", 2)],
            ),
            [0]
        );
        assert!(
            member_indices_with_producers(
                &binding,
                &BTreeSet::new(),
                &[fixed_presentation("DAT2/MGBGK05.BZZ", 1)],
            )
            .is_empty(),
            "a family must not schedule a different consumer"
        );
    }

    #[test]
    fn every_runtime_member_preserves_the_source_terminator_page() {
        let (source_compressed, source_decoded, patched_decoded, naive) =
            compression_page_fixture();
        let source_profile = source_paged_compression_profile(&source_compressed).unwrap();

        assert!(
            compression_final_input_page(source_paged_compression_profile(&naive).unwrap())
                < compression_final_input_page(source_profile),
            "the fixture must reproduce an early-terminating naive stream"
        );

        let rebuilt =
            encode_runtime_member(&source_compressed, &source_decoded, &patched_decoded).unwrap();

        assert_eq!(
            compression_final_input_page(rebuilt.rebuilt_profile),
            compression_final_input_page(rebuilt.source_profile)
        );
        assert!(
            rebuilt.bytes[..rebuilt.preserved_source_compressed_prefix_byte_count]
                == source_compressed[..rebuilt.preserved_source_compressed_prefix_byte_count]
        );
        assert_eq!(decompress(&rebuilt.bytes, false).unwrap(), patched_decoded);
        assert!(rebuilt.source_compression_prefix_preservation_required);
        assert!(rebuilt.preserved_source_compressed_prefix_byte_count > 0);
        assert!(rebuilt.preserved_source_decoded_prefix_byte_count > 0);
        assert!(rebuilt.compression_requirement_satisfied);
    }

    fn compression_page_fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut source_decoded = Vec::new();
        let mut state = 0x1234_5678_u32;
        for index in 0..0x1800 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let word = if index % 97 < 4 {
                0x5a5a
            } else {
                (state >> 16) as u16
            };
            source_decoded.extend_from_slice(&word.to_le_bytes());
        }
        let source_compressed = crate::compression::compress(&source_decoded, 0).unwrap();
        let mut patched_decoded = source_decoded.clone();
        patched_decoded[0x1000..].fill(0);
        let naive = crate::compression::compress(&patched_decoded, 0).unwrap();
        (source_compressed, source_decoded, patched_decoded, naive)
    }
}
