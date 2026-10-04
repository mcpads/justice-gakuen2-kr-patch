//! Shared source identity, runtime-address, hash-span, and pointer primitives.

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::source_disc::dat1_runtime_base;

#[derive(Default)]
pub(super) struct ValidationCounts {
    pub(super) hashed_spans: usize,
    pub(super) runtime_addresses: usize,
    pub(super) pointer_aliases: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn address_and_span(
    data: &[u8],
    base: u32,
    offset: &str,
    address: &str,
    size: usize,
    expected_sha256: &str,
    role: &str,
    counts: &mut ValidationCounts,
) -> Result<usize> {
    let offset = address_only(data, base, offset, address, role, counts)?;
    validate_span(data, offset, size, expected_sha256, role)?;
    counts.hashed_spans += 1;
    Ok(offset)
}

pub(super) fn address_only(
    data: &[u8],
    base: u32,
    offset: &str,
    address: &str,
    role: &str,
    counts: &mut ValidationCounts,
) -> Result<usize> {
    let offset = parse_hex(offset, role)?;
    ensure!(
        checked_end(offset, 4, role)? <= data.len(),
        "{role} escapes its overlay"
    );
    let expected = base
        .checked_add(u32::try_from(offset)?)
        .context("runtime address overflow")?;
    ensure!(
        parse_hex_u32(address, role)? == expected,
        "{role} runtime address is not base + offset"
    );
    counts.runtime_addresses += 1;
    Ok(offset)
}

pub(super) fn validate_span(
    data: &[u8],
    offset: usize,
    size: usize,
    expected_sha256: &str,
    role: &str,
) -> Result<()> {
    ensure!(size > 0, "{role} has an empty span");
    let end = checked_end(offset, size, role)?;
    let bytes = data
        .get(offset..end)
        .with_context(|| format!("{role} escapes its source"))?;
    ensure!(
        sha256_bytes(bytes) == expected_sha256,
        "{role} bytes changed"
    );
    Ok(())
}

pub(super) fn validate_pointer_aliases(
    overlay: &[u8],
    table_offset: usize,
    aliases: &[usize],
    expected_pointer: u32,
    role: &str,
) -> Result<()> {
    ensure!(!aliases.is_empty(), "{role} has no pointer aliases");
    for &alias in aliases {
        let offset = table_offset
            .checked_add(alias.checked_mul(4).context("pointer index overflow")?)
            .context("pointer offset overflow")?;
        ensure_pointer_value(overlay, offset, expected_pointer, role)?;
    }
    Ok(())
}

pub(super) fn ensure_pointer_value(
    data: &[u8],
    offset: usize,
    expected: u32,
    role: &str,
) -> Result<()> {
    let end = checked_end(offset, 4, role)?;
    let bytes: [u8; 4] = data
        .get(offset..end)
        .with_context(|| format!("{role} pointer is truncated"))?
        .try_into()
        .expect("four-byte slice");
    ensure!(
        u32::from_le_bytes(bytes) == expected,
        "{role} pointer names another target"
    );
    Ok(())
}

pub(super) fn validate_offset_list(data: &[u8], offsets: &[String], role: &str) -> Result<()> {
    ensure!(!offsets.is_empty(), "{role} offset list is empty");
    let parsed = offsets
        .iter()
        .map(|offset| parse_hex(offset, role))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        parsed.windows(2).all(|pair| pair[0] < pair[1])
            && parsed
                .iter()
                .all(|offset| offset % 4 == 0 && offset + 4 <= data.len()),
        "{role} offsets are unsorted, unaligned, or outside the overlay"
    );
    Ok(())
}

pub(super) fn validate_offset_range(data: &[u8], start: &str, end: &str, role: &str) -> Result<()> {
    let start = parse_hex(start, role)?;
    let end = parse_hex(end, role)?;
    ensure!(
        start % 4 == 0 && end % 4 == 0 && start <= end && end + 4 <= data.len(),
        "{role} range is invalid or outside the overlay"
    );
    Ok(())
}

pub(super) fn ensure_strictly_increasing(values: &[usize], role: &str) -> Result<()> {
    ensure!(
        !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1]),
        "{role} are empty, duplicated, or unsorted"
    );
    Ok(())
}

pub(super) fn source_for_path<'a>(
    sources: &'a [ModeDescendantSourceRecord],
    path: &str,
) -> Result<&'a ModeDescendantSourceRecord> {
    sources
        .iter()
        .find(|source| source.path == path)
        .with_context(|| format!("practical-result projection source {path} was not loaded"))
}

pub(super) fn validate_declared_overlay_base(
    path: &str,
    declared: &str,
    role: &str,
) -> Result<u32> {
    let declared = parse_hex_u32(declared, role)?;
    let expected = dat1_runtime_base(path)?
        .with_context(|| format!("{role} source {path} has no runtime-base authority"))?;
    ensure!(
        declared == expected,
        "{role} runtime base differs from central overlay authority for {path}"
    );
    Ok(declared)
}

pub(super) fn checked_end(offset: usize, size: usize, role: &str) -> Result<usize> {
    offset
        .checked_add(size)
        .with_context(|| format!("{role} range overflow"))
}

pub(super) fn parse_hex(value: &str, role: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .with_context(|| format!("{role} is not hexadecimal"))?,
        16,
    )
    .with_context(|| format!("invalid {role}"))
}

pub(super) fn parse_hex_u32(value: &str, role: &str) -> Result<u32> {
    u32::from_str_radix(
        value
            .strip_prefix("0x")
            .with_context(|| format!("{role} is not hexadecimal"))?,
        16,
    )
    .with_context(|| format!("invalid {role}"))
}
