//! Source-backed VRAM residency validation.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::parse_4bpp_prefix;

use super::super::assets::validate_siken20_indexed_result_consumer_path;
use super::super::model::PracticalResultAssets;
use super::super::projection_model::*;
use super::source_evidence::{
    checked_end, parse_hex, parse_hex_u32, source_for_path, validate_declared_overlay_base,
    validate_span,
};

pub(super) fn validate_residencies<'a>(
    assets: &'a PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
) -> Result<BTreeMap<&'a str, &'a PracticalResultVramResidency>> {
    ensure!(
        !assets.vram_residency_catalog.residencies.is_empty(),
        "practical-result residency catalog is empty"
    );
    let mut by_id = BTreeMap::new();
    for residency in &assets.vram_residency_catalog.residencies {
        ensure!(
            !residency.id.is_empty()
                && by_id.insert(residency.id.as_str(), residency).is_none()
                && residency.bpp == 4
                && residency.pixel_width == 256
                && residency.pixel_height == 256
                && !residency.unresolved_reason.trim().is_empty(),
            "invalid or duplicate practical-result residency {}",
            residency.id
        );
        let source = source_for_path(sources, &residency.source_path)?;
        let tim_offset = parse_hex(&residency.tim_offset, "residency TIM offset")?;
        let tim_end = checked_end(tim_offset, residency.source_tim_size, "residency TIM")?;
        let tim_bytes = source
            .decoded
            .get(tim_offset..tim_end)
            .with_context(|| format!("residency {} TIM is truncated", residency.id))?;
        ensure!(
            sha256_bytes(tim_bytes) == residency.source_tim_sha256,
            "residency {} source TIM changed",
            residency.id
        );
        let tim = parse_4bpp_prefix(tim_bytes)?;
        ensure!(
            tim.total_size == residency.source_tim_size
                && usize::from(tim.image_x) == residency.image_vram_word_x
                && usize::from(tim.image_y) == residency.image_vram_y
                && tim.pixel_width() == residency.pixel_width
                && tim.image_height == residency.pixel_height
                && usize::from(residency.texture_page) * 64 == residency.image_vram_word_x
                && usize::from(residency.texture_bank) * 256 == residency.image_vram_y,
            "residency {} TIM/page/external-bank identity changed",
            residency.id
        );

        match &residency.evidence {
            PracticalResultVramResidencyEvidence::StaticConfirmedLoadAndUploadIntent {
                producer_overlay_path,
                runtime_base,
                loader_span_offset,
                loader_span_size,
                loader_span_sha256,
                record_load_buffer_setup_offset,
                indirect_load_call_offset,
                catalog_index_setup_offset,
                catalog_index,
                main_catalog_source_path,
                main_catalog_source_sha256,
                main_catalog_entry_offset,
                main_catalog_entry_size,
                main_catalog_entry_sha256,
                record_load_buffer_ram_address,
                unconditional_tim_pointer_setup_offset,
                unconditional_tim_postprocess_call_offset,
            } => {
                validate_residency_loader_evidence(
                    residency,
                    sources,
                    ResidencyLoaderEvidence {
                        producer_overlay_path,
                        runtime_base,
                        loader_span_offset,
                        loader_span_size: *loader_span_size,
                        loader_span_sha256,
                        required_offsets: &[
                            ("record load-buffer setup", record_load_buffer_setup_offset),
                            ("indirect load call", indirect_load_call_offset),
                            ("catalog-index setup", catalog_index_setup_offset),
                            ("TIM pointer setup", unconditional_tim_pointer_setup_offset),
                            (
                                "TIM postprocess call",
                                unconditional_tim_postprocess_call_offset,
                            ),
                        ],
                        catalog_index,
                        main_catalog_source_path,
                        main_catalog_source_sha256,
                        main_catalog_entry_offset,
                        main_catalog_entry_size: *main_catalog_entry_size,
                        main_catalog_entry_sha256,
                        record_load_buffer_ram_address,
                    },
                )?;
            }
            PracticalResultVramResidencyEvidence::StaticConfirmedIndexedMemberLoadAndUploadIntent {
                producer_overlay_path,
                runtime_base,
                loader_span_offset,
                loader_span_size,
                loader_span_sha256,
                record_load_buffer_setup_offset,
                indirect_load_call_offset,
                catalog_index_setup_offset,
                catalog_index,
                selected_member_index,
                selected_when_state_byte_equals_two,
                member_index_setup_offset,
                main_catalog_source_path,
                main_catalog_source_sha256,
                main_catalog_entry_offset,
                main_catalog_entry_size,
                main_catalog_entry_sha256,
                record_load_buffer_ram_address,
                first_tim_pointer_setup_offset,
                first_tim_postprocess_call_offset,
            } => {
                validate_residency_loader_evidence(
                    residency,
                    sources,
                    ResidencyLoaderEvidence {
                        producer_overlay_path,
                        runtime_base,
                        loader_span_offset,
                        loader_span_size: *loader_span_size,
                        loader_span_sha256,
                        required_offsets: &[
                            ("record load-buffer setup", record_load_buffer_setup_offset),
                            ("indirect load call", indirect_load_call_offset),
                            ("catalog-index setup", catalog_index_setup_offset),
                            ("member-index setup", member_index_setup_offset),
                            ("first TIM pointer setup", first_tim_pointer_setup_offset),
                            (
                                "first TIM postprocess call",
                                first_tim_postprocess_call_offset,
                            ),
                        ],
                        catalog_index,
                        main_catalog_source_path,
                        main_catalog_source_sha256,
                        main_catalog_entry_offset,
                        main_catalog_entry_size: *main_catalog_entry_size,
                        main_catalog_entry_sha256,
                        record_load_buffer_ram_address,
                    },
                )?;
                ensure!(
                    producer_overlay_path == "DAT1/SIKEN2.BIN"
                        && catalog_index == "0x02b5"
                        && *selected_member_index < 2
                        && *selected_when_state_byte_equals_two == (*selected_member_index == 0)
                        && residency.tim_offset
                            == if *selected_member_index == 0 {
                                "0x00000"
                            } else {
                                "0x6d000"
                            }
                        && record_load_buffer_setup_offset
                            == if *selected_member_index == 0 {
                                "0x36a0"
                            } else {
                                "0x36c0"
                            }
                        && member_index_setup_offset
                            == if *selected_member_index == 0 {
                                "0x36bc"
                            } else {
                                "0x36d8"
                            }
                        && indirect_load_call_offset == "0x36dc"
                        && catalog_index_setup_offset == "0x369c"
                        && first_tim_pointer_setup_offset == "0x36ec"
                        && first_tim_postprocess_call_offset == "0x36f8",
                    "residency {} has an unsupported indexed-member selection or upload declaration",
                    residency.id
                );
                let producer = source_for_path(sources, producer_overlay_path)?;
                validate_siken20_indexed_result_consumer_path(
                    &producer.decoded,
                    residency.id.as_str(),
                )?;
            }
        }
    }
    Ok(by_id)
}

struct ResidencyLoaderEvidence<'a> {
    producer_overlay_path: &'a str,
    runtime_base: &'a str,
    loader_span_offset: &'a str,
    loader_span_size: usize,
    loader_span_sha256: &'a str,
    required_offsets: &'a [(&'a str, &'a str)],
    catalog_index: &'a str,
    main_catalog_source_path: &'a str,
    main_catalog_source_sha256: &'a str,
    main_catalog_entry_offset: &'a str,
    main_catalog_entry_size: usize,
    main_catalog_entry_sha256: &'a str,
    record_load_buffer_ram_address: &'a str,
}

fn validate_residency_loader_evidence(
    residency: &PracticalResultVramResidency,
    sources: &[ModeDescendantSourceRecord],
    evidence: ResidencyLoaderEvidence<'_>,
) -> Result<()> {
    let producer = source_for_path(sources, evidence.producer_overlay_path)?;
    let loader_offset = parse_hex(evidence.loader_span_offset, "loader span offset")?;
    validate_span(
        &producer.decoded,
        loader_offset,
        evidence.loader_span_size,
        evidence.loader_span_sha256,
        "residency loader span",
    )?;
    let loader_end = checked_end(loader_offset, evidence.loader_span_size, "loader span")?;
    for (role, offset) in evidence.required_offsets {
        let offset = parse_hex(offset, role)?;
        ensure!(
            offset >= loader_offset
                && offset % 4 == 0
                && checked_end(offset, 4, role)? <= loader_end,
            "residency {} {role} escapes the declared loader span",
            residency.id
        );
    }
    parse_hex(evidence.catalog_index, "residency catalog index")?;
    validate_declared_overlay_base(
        evidence.producer_overlay_path,
        evidence.runtime_base,
        "residency producer",
    )?;
    parse_hex_u32(
        evidence.record_load_buffer_ram_address,
        "residency record load-buffer RAM address",
    )?;

    let main = source_for_path(sources, evidence.main_catalog_source_path)?;
    ensure!(
        sha256_bytes(&main.decoded) == evidence.main_catalog_source_sha256,
        "residency {} main executable identity changed",
        residency.id
    );
    validate_span(
        &main.decoded,
        parse_hex(
            evidence.main_catalog_entry_offset,
            "main catalog entry offset",
        )?,
        evidence.main_catalog_entry_size,
        evidence.main_catalog_entry_sha256,
        "residency main catalog entry",
    )
}
