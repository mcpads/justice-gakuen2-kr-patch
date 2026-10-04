//! Catalog-backed sprite descriptor consumers.

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::super::super::projection_model::*;
use super::super::source_evidence::{
    ValidationCounts, address_and_span, address_only, checked_end, parse_hex,
    validate_pointer_aliases,
};

pub(super) fn validate_catalog_renderer(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultCatalogRendererEvidence,
    counts: &mut ValidationCounts,
) -> Result<()> {
    address_and_span(
        overlay,
        base,
        &evidence.renderer_offset,
        &evidence.renderer_runtime_address,
        evidence.renderer_size,
        &evidence.renderer_sha256,
        "catalog renderer",
        counts,
    )?;
    address_and_span(
        overlay,
        base,
        &evidence.pointer_table_offset,
        &evidence.pointer_table_runtime_address,
        evidence.pointer_table_size,
        &evidence.pointer_table_sha256,
        "catalog pointer table",
        counts,
    )?;
    Ok(())
}

pub(super) fn validate_digit_descriptor_table(
    overlay: &[u8],
    base: u32,
    renderer: &PracticalResultCatalogRendererEvidence,
    evidence: &PracticalResultDigitDescriptorTableEvidence,
    counts: &mut ValidationCounts,
) -> Result<()> {
    let mut bytes = Vec::new();
    ensure!(
        !evidence.raw_value_entries.is_empty()
            && evidence
                .descriptor_bytes_in_raw_value_order_size
                .is_multiple_of(evidence.raw_value_entries.len()),
        "digit descriptor ordered-byte denominator changed"
    );
    let entry_size =
        evidence.descriptor_bytes_in_raw_value_order_size / evidence.raw_value_entries.len();
    let pointer_table = parse_hex(&renderer.pointer_table_offset, "digit pointer table")?;
    for (expected_raw, entry) in evidence.raw_value_entries.iter().enumerate() {
        ensure!(
            entry.raw_value == expected_raw,
            "digit raw-value order changed"
        );
        let offset = address_only(
            overlay,
            base,
            &entry.descriptor_offset,
            &entry.descriptor_runtime_address,
            "digit descriptor",
            counts,
        )?;
        bytes.extend_from_slice(
            overlay
                .get(offset..checked_end(offset, entry_size, "digit descriptor")?)
                .context("digit descriptor is truncated")?,
        );
        validate_pointer_aliases(
            overlay,
            pointer_table,
            &[entry.descriptor_index],
            base.checked_add(u32::try_from(offset)?)
                .context("digit descriptor address overflow")?,
            "digit descriptor",
        )?;
    }
    ensure!(
        bytes.len() == evidence.descriptor_bytes_in_raw_value_order_size
            && sha256_bytes(&bytes) == evidence.descriptor_bytes_in_raw_value_order_sha256,
        "digit descriptors in raw-value order changed"
    );
    counts.hashed_spans += 1;
    Ok(())
}

pub(super) fn validate_g_selector(
    overlay: &[u8],
    base: u32,
    renderer: &PracticalResultCatalogRendererEvidence,
    evidence: &PracticalResultGDescriptorSelectorEvidence,
    counts: &mut ValidationCounts,
) -> Result<()> {
    address_and_span(
        overlay,
        base,
        &evidence.selector_table_offset,
        &evidence.selector_table_runtime_address,
        evidence.selector_table_size,
        &evidence.selector_table_sha256,
        "G selector table",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "G selector load",
            &evidence.selector_load_offset,
            &evidence.selector_load_runtime_address,
        ),
        (
            "G descriptor-table load",
            &evidence.descriptor_table_load_offset,
            &evidence.descriptor_table_load_runtime_address,
        ),
        (
            "G renderer call",
            &evidence.renderer_call_offset,
            &evidence.renderer_call_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    validate_selector_descriptor_bytes(
        overlay,
        base,
        &renderer.pointer_table_offset,
        evidence.descriptor_bytes_in_raw_selector_order_size,
        &evidence.descriptor_bytes_in_raw_selector_order_sha256,
        &evidence.raw_selector_entries,
        counts,
    )
}

pub(super) fn validate_go_selector(
    overlay: &[u8],
    base: u32,
    renderer: &PracticalResultCatalogRendererEvidence,
    evidence: &PracticalResultGoDescriptorSelectorEvidence,
    counts: &mut ValidationCounts,
) -> Result<()> {
    address_and_span(
        overlay,
        base,
        &evidence.selector_table_offset,
        &evidence.selector_table_runtime_address,
        evidence.selector_table_size,
        &evidence.selector_table_sha256,
        "GO selector table",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "GO descriptor-table load",
            &evidence.descriptor_table_load_offset,
            &evidence.descriptor_table_load_runtime_address,
        ),
        (
            "GO renderer call",
            &evidence.renderer_call_offset,
            &evidence.renderer_call_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    validate_selector_descriptor_bytes(
        overlay,
        base,
        &renderer.pointer_table_offset,
        evidence.descriptor_bytes_in_raw_selector_order_size,
        &evidence.descriptor_bytes_in_raw_selector_order_sha256,
        &evidence.raw_selector_entries,
        counts,
    )
}

pub(super) fn validate_selector_descriptor_bytes(
    overlay: &[u8],
    base: u32,
    pointer_table_offset: &str,
    total_size: usize,
    expected_hash: &str,
    entries: &[PracticalResultCatalogSelectorEntry],
    counts: &mut ValidationCounts,
) -> Result<()> {
    ensure!(
        !entries.is_empty() && total_size.is_multiple_of(entries.len()),
        "selector descriptor ordered-byte denominator changed"
    );
    let entry_size = total_size / entries.len();
    let pointer_table = parse_hex(pointer_table_offset, "selector pointer table")?;
    let mut bytes = Vec::with_capacity(total_size);
    for (expected_raw, entry) in entries.iter().enumerate() {
        ensure!(
            entry.raw_selector_value == expected_raw,
            "raw selector order changed"
        );
        let offset = address_only(
            overlay,
            base,
            &entry.descriptor_offset,
            &entry.descriptor_runtime_address,
            "selector descriptor",
            counts,
        )?;
        bytes.extend_from_slice(
            overlay
                .get(offset..checked_end(offset, entry_size, "selector descriptor")?)
                .context("selector descriptor is truncated")?,
        );
        validate_pointer_aliases(
            overlay,
            pointer_table,
            &[entry.descriptor_index],
            base.checked_add(u32::try_from(offset)?)
                .context("selector descriptor address overflow")?,
            "selector descriptor",
        )?;
    }
    ensure!(
        bytes.len() == total_size && sha256_bytes(&bytes) == expected_hash,
        "descriptors in raw-selector order changed"
    );
    counts.hashed_spans += 1;
    Ok(())
}
