//! Palette-source and CLUT binding validation.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::mode_descendant_graphics::model::ModeDescendantStorageKind;
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::{parse_4bpp_prefix, read_4bpp_palette_words_in_prefix};
use crate::tzz::parse_tzz;

use super::super::model::PracticalResultAssets;
use super::super::projection_model::*;
use super::source_evidence::{parse_hex, source_for_path};

pub(super) struct ValidatedClut<'a> {
    pub(super) binding: &'a PracticalResultClutBinding,
}

pub(super) fn validate_cluts<'a>(
    assets: &'a PracticalResultAssets,
    residencies: &BTreeMap<&str, &PracticalResultVramResidency>,
    sources: &[ModeDescendantSourceRecord],
) -> Result<BTreeMap<&'a str, ValidatedClut<'a>>> {
    ensure!(
        !assets.clut_binding_catalog.bindings.is_empty(),
        "practical-result CLUT catalog is empty"
    );
    let mut by_id = BTreeMap::new();
    for binding in &assets.clut_binding_catalog.bindings {
        ensure!(
            !binding.id.is_empty() && !by_id.contains_key(binding.id.as_str()),
            "duplicate or empty practical-result CLUT binding {}",
            binding.id
        );
        match &binding.evidence {
            PracticalResultClutBindingEvidence::StaticConfirmed {
                source_path,
                tim_offset: tim_offset_text,
                bpp,
                palette_index,
                palette_sha256: expected_hash,
                ..
            } => {
                ensure!(
                    *bpp == 4
                        && residencies.values().any(|residency| {
                            residency.source_path == source_path.as_str()
                                && residency.tim_offset == tim_offset_text.as_str()
                                && residency.bpp == *bpp
                        }),
                    "resolved CLUT {} is not bound to a residency TIM",
                    binding.id
                );
                let source = source_for_path(sources, source_path)?;
                let tim_offset = parse_hex(tim_offset_text, "CLUT TIM offset")?;
                let tim = parse_4bpp_prefix(
                    source
                        .decoded
                        .get(tim_offset..)
                        .context("CLUT TIM offset escapes its source")?,
                )?;
                let linear_color = palette_index
                    .checked_mul(16)
                    .context("CLUT palette offset overflow")?;
                ensure!(
                    usize::from(tim.clut_x) + linear_color % tim.clut_width == binding.clut_vram_x
                        && usize::from(tim.clut_y) + linear_color / tim.clut_width
                            == binding.clut_vram_y,
                    "CLUT {} VRAM coordinates differ from its TIM palette",
                    binding.id
                );
                let palette_bytes =
                    read_4bpp_palette_words_in_prefix(&source.decoded, tim_offset, *palette_index)?
                        .into_iter()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>();
                ensure!(
                    sha256_bytes(&palette_bytes) == expected_hash.as_str(),
                    "CLUT {} palette bytes changed",
                    binding.id
                );
            }
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                source_path,
                member_count,
                member_decoded_size,
                member_palette_sha256s,
                unique_palette_count,
                palette_family_sha256,
            } => validate_external_producer_family(
                binding,
                source_for_path(sources, source_path)?,
                *member_count,
                *member_decoded_size,
                member_palette_sha256s,
                *unique_palette_count,
                palette_family_sha256,
            )?,
            PracticalResultClutBindingEvidence::Unresolved { unresolved_reason } => ensure!(
                !unresolved_reason.trim().is_empty(),
                "unresolved CLUT {} lacks a reason",
                binding.id
            ),
        }
        by_id.insert(binding.id.as_str(), ValidatedClut { binding });
    }
    Ok(by_id)
}

fn validate_external_producer_family(
    binding: &PracticalResultClutBinding,
    source: &ModeDescendantSourceRecord,
    member_count: usize,
    member_decoded_size: usize,
    member_palette_sha256s: &[String],
    unique_palette_count: usize,
    palette_family_sha256: &str,
) -> Result<()> {
    ensure!(
        source.storage_kind == ModeDescendantStorageKind::TzzCompressedMembers,
        "external CLUT producer {} is not a TZZ member family",
        source.path
    );
    let members = parse_tzz(&source.stored).context("failed to parse external CLUT producer")?;
    ensure!(
        member_count > 0
            && members.len() == member_count
            && member_palette_sha256s.len() == member_count
            && member_decoded_size > 0,
        "external CLUT producer {} member denominator changed",
        binding.id
    );

    let mut palette_family = Vec::with_capacity(member_count * 32);
    let mut palette_hashes = BTreeSet::new();
    for (member, expected_hash) in members.iter().zip(member_palette_sha256s) {
        let decoded = decompress(&source.stored[member.compressed_range()], true)
            .with_context(|| format!("failed to decode external CLUT member {}", member.index))?;
        let tim = parse_4bpp_prefix(&decoded)
            .with_context(|| format!("external CLUT member {} has no primary TIM", member.index))?;
        ensure!(
            decoded.len() == member_decoded_size
                && usize::from(tim.clut_x) == binding.clut_vram_x
                && usize::from(tim.clut_y) == binding.clut_vram_y
                && tim.clut_width * tim.clut_height == 16,
            "external CLUT member {} geometry changed",
            member.index
        );
        let palette_bytes = read_4bpp_palette_words_in_prefix(&decoded, 0, 0)?
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let palette_sha256 = sha256_bytes(&palette_bytes);
        ensure!(
            &palette_sha256 == expected_hash,
            "external CLUT member {} palette changed",
            member.index
        );
        palette_hashes.insert(palette_sha256);
        palette_family.extend(palette_bytes);
    }
    ensure!(
        palette_hashes.len() == unique_palette_count
            && sha256_bytes(&palette_family) == palette_family_sha256,
        "external CLUT producer {} family identity changed",
        binding.id
    );
    Ok(())
}
