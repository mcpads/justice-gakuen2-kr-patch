use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register, decode, encode};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::psx_machine_code_sources::PsxMachineCodeSources;

use super::model::{
    CharacterSelectConsumerPlan, CharacterSelectConsumerReferenceKind,
    CharacterSelectDescriptorBuild, CharacterSelectFontRole, CharacterSelectGlyphAllocation,
    CharacterSelectLocalizedSource, CharacterSelectOverlayBuild, CharacterSelectRouteOccurrence,
    CharacterSelectSourceGuardBuild, CharacterSelectTextureSurface,
};
use super::route_census::{bound_occurrence, consumer_target, producer_target};
use super::source::character_select_overlay_output_file;
use super::texture_targets::SHARED_ATLAS_OFFSET;

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const FOUR_BPP_TEXTURE_PAGE_WORD_WIDTH: u16 = 64;

#[path = "consumer/descriptor.rs"]
mod descriptor;
#[path = "consumer/map.rs"]
mod map;
#[path = "consumer/source_glyph_references.rs"]
mod source_glyph_references;
#[path = "consumer/stage_selector.rs"]
mod stage_selector;

use descriptor::encode_descriptor;
use map::{DescriptorEncoding, DescriptorSpec, OVERLAYS, SltiPatch};
pub(super) use source_glyph_references::{
    SourceGlyphReference, reclaimable_stage_label_source_cells,
    retained_source_referenced_20px_cells,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CharacterSelectDynamicTextBinding {
    pub(super) source_ui_id: &'static str,
    pub(super) font_role: CharacterSelectFontRole,
    pub(super) surface: CharacterSelectTextureSurface,
}

pub(super) fn dynamic_text_bindings() -> Result<Vec<CharacterSelectDynamicTextBinding>> {
    let mut bindings = BTreeMap::new();
    for descriptor in OVERLAYS.iter().flat_map(|overlay| overlay.descriptors) {
        let binding = CharacterSelectDynamicTextBinding {
            source_ui_id: descriptor.source_ui_id,
            font_role: descriptor.font_role,
            surface: descriptor.surface,
        };
        if let Some(existing) = bindings.insert(descriptor.source_ui_id, binding) {
            ensure!(
                existing == binding,
                "character-select source UI {} has conflicting dynamic consumers",
                descriptor.source_ui_id
            );
        }
    }
    for source_ui_id in stage_selector::source_ui_ids() {
        let binding = CharacterSelectDynamicTextBinding {
            source_ui_id,
            font_role: CharacterSelectFontRole::StageLabel,
            surface: CharacterSelectTextureSurface::StageLabelAtlas,
        };
        ensure!(
            bindings.insert(source_ui_id, binding).is_none(),
            "character-select source UI {source_ui_id} repeats a stage-label consumer"
        );
    }
    Ok(bindings.into_values().collect())
}

pub(super) fn dynamic_route_occurrences(
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Vec<CharacterSelectRouteOccurrence> {
    let mut occurrences = OVERLAYS
        .iter()
        .flat_map(|overlay| {
            overlay
                .descriptors
                .iter()
                .filter(|&descriptor| {
                    localized_sources
                        .iter()
                        .any(|source| source.source_ui_id == descriptor.source_ui_id)
                })
                .map(|descriptor| {
                    bound_occurrence(
                        descriptor.route_occurrence_id,
                        "selp_overlay_descriptor",
                        [descriptor.source_ui_id],
                        vec![producer_target(
                            overlay.texture_source_path,
                            Some(SHARED_ATLAS_OFFSET),
                            Some(descriptor.surface),
                            [format!(
                                "{}-dynamic-glyph-allocations",
                                descriptor.source_ui_id
                            )],
                        )],
                        vec![consumer_target(
                            overlay.texture_source_path,
                            overlay.source_path,
                            CharacterSelectConsumerReferenceKind::DescriptorAndPointer,
                            [descriptor.offset, descriptor.pointer_offset],
                            [descriptor.route_occurrence_id],
                        )],
                    )
                })
        })
        .collect::<Vec<_>>();
    occurrences.extend(stage_selector::route_occurrences(localized_sources));
    occurrences
}

pub(super) fn rewrite_character_select_overlay(
    source_path: &str,
    source: &[u8],
    allocations: &[CharacterSelectGlyphAllocation],
    consumer_plan: &CharacterSelectConsumerPlan,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<(Vec<u8>, CharacterSelectOverlayBuild)> {
    let spec = OVERLAYS
        .iter()
        .find(|spec| spec.source_path == source_path)
        .with_context(|| format!("unsupported character-select overlay {source_path}"))?;
    let allocation_by_glyph = allocations
        .iter()
        .map(|allocation| {
            (
                (
                    allocation.surface,
                    allocation.font_role,
                    allocation.character,
                ),
                allocation,
            )
        })
        .collect::<BTreeMap<_, _>>();
    ensure!(
        allocation_by_glyph.len() == allocations.len(),
        "character-select allocation map contains duplicate glyphs"
    );

    let source_guards = spec
        .source_guards
        .iter()
        .filter(|guard| source_binding_is_active(guard.activation_source_ui_id, localized_sources))
        .map(|guard| {
            let end = guard
                .offset
                .checked_add(guard.expected.len())
                .context("character-select source guard range overflow")?;
            let actual = source.get(guard.offset..end).with_context(|| {
                format!("{source_path} source guard for {} is truncated", guard.role)
            })?;
            ensure!(
                actual == guard.expected,
                "{source_path} source guard changed for {} at +0x{:04x}",
                guard.role,
                guard.offset
            );
            Ok(CharacterSelectSourceGuardBuild {
                role: guard.role.to_string(),
                byte_range: [guard.offset, end],
                source_sha256: sha256_bytes(actual),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut patched = source.to_vec();
    let mut expected_write_ranges = Vec::new();
    let mut machine_code_offsets = BTreeSet::new();
    let mut descriptors = Vec::new();
    for descriptor in spec.descriptors {
        let Some(entry) = localized_sources
            .iter()
            .find(|entry| entry.source_ui_id == descriptor.source_ui_id)
        else {
            continue;
        };
        validate_pointer(source_path, source, descriptor)?;
        let source_descriptor = source
            .get(descriptor.offset..descriptor.offset + descriptor.expected.len())
            .with_context(|| {
                format!(
                    "{source_path} descriptor for {} is truncated",
                    descriptor.source_ui_id
                )
            })?;
        ensure!(
            source_descriptor == descriptor.expected,
            "{source_path} source descriptor changed for {} at +0x{:04x}",
            descriptor.source_ui_id,
            descriptor.offset
        );
        let encoded = encode_descriptor(entry, descriptor, &allocation_by_glyph)?;
        patched[descriptor.offset..descriptor.offset + encoded.len()].copy_from_slice(&encoded);
        expected_write_ranges.push([
            descriptor.offset,
            descriptor.offset + descriptor.expected.len(),
        ]);
        let texture_page_indices = entry
            .korean_text
            .chars()
            .filter(|character| descriptor.encoding.retains_spaces() || !character.is_whitespace())
            .filter_map(|character| {
                allocation_by_glyph
                    .get(&(descriptor.surface, descriptor.font_role, character))
                    .map(|allocation| allocation.texture_page_index)
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        ensure!(
            !texture_page_indices.is_empty(),
            "character-select descriptor contains no allocated glyph"
        );
        let logical_character_count = entry.korean_text.chars().count();
        let encoded_glyph_count = match descriptor.encoding {
            DescriptorEncoding::DirectUv
            | DescriptorEncoding::DirectLabelGrid
            | DescriptorEncoding::CountedLabelGrid => entry
                .korean_text
                .chars()
                .filter(|character| !character.is_whitespace())
                .count(),
            DescriptorEncoding::ParticipantRuns => entry
                .korean_text
                .chars()
                .filter(|character| !character.is_whitespace() && *character != '?')
                .count(),
            DescriptorEncoding::ColumnRow
            | DescriptorEncoding::DirectURow
            | DescriptorEncoding::SelectionHelp => logical_character_count,
        };
        descriptors.push(CharacterSelectDescriptorBuild {
            route_occurrence_id: descriptor.route_occurrence_id.to_string(),
            source_ui_id: descriptor.source_ui_id.to_string(),
            translation_id: entry.translation_id.clone(),
            descriptor_offset: descriptor.offset,
            logical_character_count,
            encoded_glyph_count,
            omitted_separator_count: logical_character_count - encoded_glyph_count,
            texture_page_indices,
        });
    }
    stage_selector::rewrite(
        source_path,
        source,
        &mut patched,
        allocations,
        localized_sources,
        &mut expected_write_ranges,
        &mut descriptors,
    )?;
    validate_heading_pages(allocations, consumer_plan)?;
    for binding in spec.texture_pages {
        let Some(entry) = localized_sources
            .iter()
            .find(|entry| entry.source_ui_id == binding.source_ui_id)
        else {
            continue;
        };
        let descriptor = spec
            .descriptors
            .iter()
            .find(|descriptor| descriptor.source_ui_id == binding.source_ui_id)
            .context("texture-page consumer has no descriptor owner")?;
        let pages = entry
            .korean_text
            .chars()
            .filter(|character| descriptor.encoding.retains_spaces() || !character.is_whitespace())
            .map(|character| {
                allocation_by_glyph
                    .get(&(descriptor.surface, descriptor.font_role, character))
                    .map(|allocation| allocation.texture_page_index)
                    .with_context(|| {
                        format!(
                            "texture-page consumer {} has no glyph {character:?}",
                            binding.source_ui_id
                        )
                    })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        ensure!(
            pages.len() == 1,
            "texture-page consumer {} must use exactly one allocated page",
            binding.source_ui_id
        );
        let page = *pages
            .first()
            .context("texture-page consumer has no glyphs")?;
        let word_x = consumer_plan
            .shared_atlas_vram_word_x
            .checked_add(u16::from(page) * FOUR_BPP_TEXTURE_PAGE_WORD_WIDTH)
            .context("texture-page consumer VRAM x overflow")?;
        let offset = binding.offset;
        patch_addiu_a2(
            source_path,
            &mut patched,
            offset,
            binding.source_word_x,
            i16::try_from(word_x).context("texture-page consumer VRAM x exceeds immediate")?,
        )?;
        expected_write_ranges.push([offset, offset + 4]);
        machine_code_offsets.insert(offset);
    }
    for patch in spec.addiu_patches {
        if !source_binding_is_active(patch.activation_source_ui_id, localized_sources) {
            continue;
        }
        patch_addiu(
            source_path,
            &mut patched,
            patch.offset,
            patch.rt,
            (patch.rs, patch.replacement_rs),
            patch.expected_immediate,
            patch.replacement_immediate,
        )?;
        expected_write_ranges.push([patch.offset, patch.offset + 4]);
        machine_code_offsets.insert(patch.offset);
    }
    for patch in spec.slot_count_patches {
        if !source_binding_is_active(patch.activation_source_ui_id, localized_sources) {
            continue;
        }
        patch_slti(source_path, &mut patched, patch)?;
        expected_write_ranges.push([patch.offset, patch.offset + 4]);
        machine_code_offsets.insert(patch.offset);
    }
    if localized_sources
        .iter()
        .any(|entry| entry.source_ui_id == "battle_ready_label")
    {
        for offset in super::ready_layout::rewrite_sprite_width(source_path, source, &mut patched)?
        {
            expected_write_ranges.push([offset, offset + 4]);
            machine_code_offsets.insert(offset);
        }
    }
    expected_write_ranges.sort_unstable();
    let source_sha256 = sha256_bytes(source);
    let mut machine_code_sources = PsxMachineCodeSources::default();
    let mut claims = Vec::with_capacity(expected_write_ranges.len());
    for (index, [start, end]) in expected_write_ranges.iter().copied().enumerate() {
        let source_range = source
            .get(start..end)
            .context("character-select overlay claim exceeds its source")?;
        let candidate_range = patched
            .get(start..end)
            .context("character-select overlay claim exceeds its candidate")?;
        if source_range == candidate_range {
            continue;
        }
        let (id, purpose, intent) = if machine_code_offsets.contains(&start) {
            ensure!(
                end - start == 4,
                "{source_path} machine-code claim at +0x{start:04x} is not one instruction"
            );
            let id = format!("character-select-overlay:{source_path}:instruction:{start:04x}");
            let runtime_address = OVERLAY_RUNTIME_BASE
                .checked_add(u32::try_from(start)?)
                .context("character-select instruction address overflow")?;
            let provenance = machine_code_sources.register(
                &id,
                runtime_address,
                vec![decode_instruction(&patched, start)?],
            )?;
            (
                id,
                "install one typed character-select consumer instruction".to_string(),
                WriteIntent::MachineCode(provenance),
            )
        } else {
            (
                format!("character-select-overlay:{source_path}:data:{index}"),
                "install one character-select consumer descriptor".to_string(),
                WriteIntent::Data,
            )
        };
        claims.push(CandidateWriteClaim {
            id,
            purpose,
            range: start..end,
            intent,
        });
    }
    let mut write_plan = DecodedRecordWritePlan::new(source_path, source, &source_sha256)?;
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "character-select overlay consumer producer",
        source_sha256: &source_sha256,
        candidate: &patched,
        claims,
    })?;
    let planned = write_plan.apply(Some(&machine_code_sources))?;
    ensure!(
        planned == patched,
        "{source_path} overlay plan omitted consumer bytes"
    );
    let changed_byte_ranges = difference_ranges(source, &planned);
    ensure!(
        !changed_byte_ranges.is_empty(),
        "{source_path} consumer rewrite changed no bytes"
    );
    let report = CharacterSelectOverlayBuild {
        source_path: source_path.to_string(),
        output_file: character_select_overlay_output_file(source_path)?.to_string(),
        source_sha256,
        patched_sha256: sha256_bytes(&planned),
        source_size: source.len(),
        expected_write_ranges,
        changed_byte_ranges,
        source_guards,
        descriptors,
    };
    Ok((planned, report))
}

fn source_binding_is_active(
    activation_source_ui_id: Option<&str>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> bool {
    activation_source_ui_id.is_none_or(|source_ui_id| {
        localized_sources
            .iter()
            .any(|source| source.source_ui_id == source_ui_id)
    })
}

fn validate_heading_pages(
    allocations: &[CharacterSelectGlyphAllocation],
    consumer_plan: &CharacterSelectConsumerPlan,
) -> Result<()> {
    ensure!(
        allocations
            .iter()
            .filter(|allocation| allocation.font_role == CharacterSelectFontRole::SelectHeading)
            .all(|allocation| {
                allocation.texture_page_index == consumer_plan.select_heading_texture_page_index
            }),
        "character-select select-heading allocations disagree with the consumer plan"
    );
    Ok(())
}

fn validate_pointer(source_path: &str, source: &[u8], spec: &DescriptorSpec) -> Result<()> {
    let expected = Instruction::Addiu {
        rt: spec.pointer_register,
        rs: spec.pointer_register,
        immediate: spec.pointer_immediate,
    };
    let actual = decode_instruction(source, spec.pointer_offset)?;
    ensure!(
        actual == expected,
        "{source_path} descriptor pointer changed for {} at +0x{:04x}: expected {expected:?}, found {actual:?}",
        spec.source_ui_id,
        spec.pointer_offset
    );
    Ok(())
}

fn patch_addiu_a2(
    source_path: &str,
    target: &mut [u8],
    offset: usize,
    expected_immediate: i16,
    replacement_immediate: i16,
) -> Result<()> {
    patch_addiu(
        source_path,
        target,
        offset,
        Register::A2,
        (Register::ZERO, Register::ZERO),
        expected_immediate,
        replacement_immediate,
    )
}

fn patch_addiu(
    source_path: &str,
    target: &mut [u8],
    offset: usize,
    rt: Register,
    source_registers: (Register, Register),
    expected_immediate: i16,
    replacement_immediate: i16,
) -> Result<()> {
    let expected = Instruction::Addiu {
        rt,
        rs: source_registers.0,
        immediate: expected_immediate,
    };
    let actual = decode_instruction(target, offset)?;
    ensure!(
        actual == expected,
        "{source_path} texture-page builder changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
    );
    let replacement = Instruction::Addiu {
        rt,
        rs: source_registers.1,
        immediate: replacement_immediate,
    };
    let word = encode(&replacement, OVERLAY_RUNTIME_BASE + offset as u32)?;
    target[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    ensure!(
        decode_instruction(target, offset)? == replacement,
        "{source_path} texture-page replacement failed at +0x{offset:04x}"
    );
    Ok(())
}

fn patch_slti(source_path: &str, target: &mut [u8], patch: &SltiPatch) -> Result<()> {
    let expected = Instruction::Slti {
        rt: patch.rt,
        rs: patch.rs,
        immediate: patch.expected_immediate,
    };
    let actual = decode_instruction(target, patch.offset)?;
    ensure!(
        actual == expected,
        "{source_path} instruction changed at +0x{:04x}: expected {expected:?}, found {actual:?}",
        patch.offset
    );
    let replacement = Instruction::Slti {
        rt: patch.rt,
        rs: patch.rs,
        immediate: patch.replacement_immediate,
    };
    let word = encode(
        &replacement,
        OVERLAY_RUNTIME_BASE
            .checked_add(u32::try_from(patch.offset)?)
            .context("character-select instruction address overflow")?,
    )?;
    target[patch.offset..patch.offset + 4].copy_from_slice(&word.to_le_bytes());
    Ok(())
}

fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated character-select instruction at +0x{offset:04x}"))?;
    decode(
        u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
        OVERLAY_RUNTIME_BASE + offset as u32,
    )
    .with_context(|| format!("failed to decode character-select instruction at +0x{offset:04x}"))
}
