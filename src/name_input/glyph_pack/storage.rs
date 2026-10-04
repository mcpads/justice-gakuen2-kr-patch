use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::super::{NameGlyphPackCell, NameGlyphPackStoragePlan};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, install_indexed_glyph_in_prefix, read_indexed_cell_in_prefix};

const PIXELS_PER_BYTE: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameGlyphPackStorageImage {
    pub pack_byte_count: usize,
    pub cells: Vec<NameGlyphPackCellPayload>,
    pub runtime_lookup: Option<NameGlyphPackRuntimeLookupPayload>,
    pub auxiliary_payloads: Vec<NameGlyphPackRuntimeLookupPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameGlyphPackCellPayload {
    pub storage: NameGlyphPackCell,
    pub storage_byte_range: [usize; 2],
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameGlyphPackRuntimeLookupPayload {
    pub storage_byte_range: [usize; 2],
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameGlyphPackRuntimeLookupInstallReport {
    pub first_storage_cell_index: usize,
    pub storage_cell_count: usize,
    pub storage_byte_range: [usize; 2],
    pub byte_count: usize,
    pub sha256: String,
    pub readback_verified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameGlyphPackTimInstallReport {
    pub payload_sha256: String,
    pub payload_byte_count: usize,
    pub storage_cell_count: usize,
    pub changed_decoded_byte_count: usize,
    pub tim_readback_sha256: String,
    pub tim_readback_verified: bool,
}

pub fn encode_name_glyph_pack_cells(
    pack: &[u8],
    storage: &NameGlyphPackStoragePlan,
) -> Result<NameGlyphPackStorageImage> {
    validate_storage_plan(storage)?;
    ensure!(!pack.is_empty(), "name glyph pack is empty");
    ensure!(
        pack.len() <= storage.byte_capacity,
        "name glyph pack exceeds its hidden-cell storage"
    );

    let mut padded = vec![0u8; storage.byte_capacity];
    padded[..pack.len()].copy_from_slice(pack);
    let cells = storage
        .cells
        .iter()
        .cloned()
        .enumerate()
        .map(|(cell_index, cell)| {
            let start = cell_index * storage.bytes_per_cell;
            let end = start + storage.bytes_per_cell;
            NameGlyphPackCellPayload {
                storage: cell,
                storage_byte_range: [start, end],
                pixels: bytes_to_pixels(&padded[start..end]),
            }
        })
        .collect();

    Ok(NameGlyphPackStorageImage {
        pack_byte_count: pack.len(),
        cells,
        runtime_lookup: None,
        auxiliary_payloads: Vec::new(),
    })
}

pub fn install_name_glyph_pack_runtime_lookup(
    image: &mut NameGlyphPackStorageImage,
    lookup: &[u8],
) -> Result<NameGlyphPackRuntimeLookupInstallReport> {
    ensure!(
        image.runtime_lookup.is_none(),
        "name glyph pack runtime lookup is already installed"
    );
    ensure!(
        !lookup.is_empty(),
        "name glyph pack runtime lookup is empty"
    );
    let bytes_per_cell = image
        .cells
        .first()
        .context("name glyph pack storage has no cells")?
        .storage_byte_range[1];
    ensure!(bytes_per_cell > 0, "name glyph pack storage cell is empty");
    let storage_cell_count = lookup.len().div_ceil(bytes_per_cell);
    let first_storage_cell_index = image
        .cells
        .len()
        .checked_sub(storage_cell_count)
        .context("name glyph pack runtime lookup exceeds the trailing storage cells")?;
    let storage_byte_start = first_storage_cell_index * bytes_per_cell;
    let storage_byte_range = [storage_byte_start, storage_byte_start + lookup.len()];
    ensure!(
        image.pack_byte_count <= storage_byte_start,
        "name glyph pack runtime lookup overlaps the component pack"
    );
    let mut remaining = lookup;
    for cell in &mut image.cells[first_storage_cell_index..] {
        let mut cell_bytes = pixels_to_bytes(&cell.pixels);
        ensure!(
            cell_bytes.iter().all(|byte| *byte == 0),
            "name glyph pack runtime lookup tail cell is not reserved padding"
        );
        let byte_count = remaining.len().min(bytes_per_cell);
        cell_bytes[..byte_count].copy_from_slice(&remaining[..byte_count]);
        cell.pixels = bytes_to_pixels(&cell_bytes);
        remaining = &remaining[byte_count..];
    }
    ensure!(
        remaining.is_empty(),
        "name glyph pack runtime lookup write was truncated"
    );
    image.runtime_lookup = Some(NameGlyphPackRuntimeLookupPayload {
        storage_byte_range,
        bytes: lookup.to_vec(),
    });
    read_name_glyph_pack_cells(image)?;
    Ok(NameGlyphPackRuntimeLookupInstallReport {
        first_storage_cell_index,
        storage_cell_count,
        storage_byte_range,
        byte_count: lookup.len(),
        sha256: sha256_bytes(lookup),
        readback_verified: true,
    })
}

/// Add explicitly owned data in padding without changing the decoded Hangul pack.
pub fn install_name_glyph_pack_auxiliary(
    image: &mut NameGlyphPackStorageImage,
    start: usize,
    payload: &[u8],
) -> Result<()> {
    read_name_glyph_pack_cells(image)?;
    let end = start
        .checked_add(payload.len())
        .context("auxiliary glyph data overflow")?;
    ensure!(
        !payload.is_empty()
            && start >= image.pack_byte_count
            && end
                <= image
                    .cells
                    .last()
                    .context("empty storage")?
                    .storage_byte_range[1]
            && image
                .runtime_lookup
                .iter()
                .chain(image.auxiliary_payloads.iter())
                .all(|p| end <= p.storage_byte_range[0] || p.storage_byte_range[1] <= start),
        "auxiliary glyph data overlaps pack, metadata or storage boundary"
    );
    for cell in &mut image.cells {
        let [a, z] = cell.storage_byte_range;
        let left = start.max(a);
        let right = end.min(z);
        if left < right {
            let mut bytes = pixels_to_bytes(&cell.pixels);
            bytes[left - a..right - a].copy_from_slice(&payload[left - start..right - start]);
            cell.pixels = bytes_to_pixels(&bytes);
        }
    }
    image
        .auxiliary_payloads
        .push(NameGlyphPackRuntimeLookupPayload {
            storage_byte_range: [start, end],
            bytes: payload.to_vec(),
        });
    read_name_glyph_pack_cells(image)?;
    Ok(())
}

pub fn read_name_glyph_pack_cells(image: &NameGlyphPackStorageImage) -> Result<Vec<u8>> {
    ensure!(!image.cells.is_empty(), "name glyph pack storage is empty");
    let mut bytes = Vec::new();
    for cell in &image.cells {
        ensure!(
            cell.pixels.len().is_multiple_of(PIXELS_PER_BYTE),
            "name glyph pack cell has an odd pixel count"
        );
        ensure!(
            cell.pixels.iter().all(|pixel| *pixel < 16),
            "name glyph pack cell contains a non-4-bpp pixel"
        );
        let cell_byte_count = cell.pixels.len() / PIXELS_PER_BYTE;
        ensure!(
            cell.storage_byte_range == [bytes.len(), bytes.len() + cell_byte_count],
            "name glyph pack cell ranges are not contiguous"
        );
        bytes.extend(pixels_to_bytes(&cell.pixels));
    }
    ensure!(
        image.pack_byte_count <= bytes.len(),
        "name glyph pack length exceeds the stored cell payload"
    );
    let mut padding = bytes[image.pack_byte_count..].to_vec();
    let mut occupied = Vec::new();
    for runtime_lookup in image
        .runtime_lookup
        .iter()
        .chain(image.auxiliary_payloads.iter())
    {
        ensure!(
            runtime_lookup.storage_byte_range[0] >= image.pack_byte_count
                && runtime_lookup.storage_byte_range[0] < runtime_lookup.storage_byte_range[1]
                && occupied
                    .iter()
                    .all(|&(a, z)| runtime_lookup.storage_byte_range[1] <= a
                        || z <= runtime_lookup.storage_byte_range[0])
                && runtime_lookup.storage_byte_range[1] <= bytes.len()
                && bytes
                    [runtime_lookup.storage_byte_range[0]..runtime_lookup.storage_byte_range[1]]
                    == runtime_lookup.bytes,
            "name glyph pack runtime lookup payload changed"
        );
        occupied.push((
            runtime_lookup.storage_byte_range[0],
            runtime_lookup.storage_byte_range[1],
        ));
        let start = runtime_lookup.storage_byte_range[0] - image.pack_byte_count;
        let end = runtime_lookup.storage_byte_range[1] - image.pack_byte_count;
        padding[start..end].fill(0);
    }
    ensure!(
        padding.iter().all(|byte| *byte == 0),
        "name glyph pack padding is not zero"
    );
    bytes.truncate(image.pack_byte_count);
    Ok(bytes)
}

pub fn install_name_glyph_pack_cells_in_tim(
    decoded: &mut [u8],
    tim_offset: usize,
    layout_cells: &[Cell],
    image: &NameGlyphPackStorageImage,
) -> Result<NameGlyphPackTimInstallReport> {
    let source = decoded.to_vec();
    for payload in &image.cells {
        let cell = *layout_cells
            .get(payload.storage.atlas_layout_record_index)
            .ok_or_else(|| anyhow::anyhow!("name glyph pack layout index is out of range"))?;
        ensure!(
            cell.width * cell.height == payload.pixels.len(),
            "name glyph pack cell does not fill its physical layout cell"
        );
        install_indexed_glyph_in_prefix(
            decoded,
            tim_offset,
            cell,
            &payload.pixels,
            "Korean name glyph component pack storage",
        )?;
    }

    let readback_cells = image
        .cells
        .iter()
        .map(|payload| {
            let cell = layout_cells[payload.storage.atlas_layout_record_index];
            Ok(NameGlyphPackCellPayload {
                storage: payload.storage.clone(),
                storage_byte_range: payload.storage_byte_range,
                pixels: read_indexed_cell_in_prefix(decoded, tim_offset, cell)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let readback = read_name_glyph_pack_cells(&NameGlyphPackStorageImage {
        pack_byte_count: image.pack_byte_count,
        cells: readback_cells,
        runtime_lookup: image.runtime_lookup.clone(),
        auxiliary_payloads: image.auxiliary_payloads.clone(),
    })?;
    let expected = read_name_glyph_pack_cells(image)?;
    ensure!(
        readback == expected,
        "name glyph pack TIM readback differs from the input pack"
    );
    let changed_decoded_byte_count = source
        .iter()
        .zip(decoded.iter())
        .filter(|(before, after)| before != after)
        .count();
    ensure!(
        changed_decoded_byte_count > 0,
        "name glyph pack TIM install changed no decoded bytes"
    );

    Ok(NameGlyphPackTimInstallReport {
        payload_sha256: sha256_bytes(&expected),
        payload_byte_count: expected.len(),
        storage_cell_count: image.cells.len(),
        changed_decoded_byte_count,
        tim_readback_sha256: sha256_bytes(&readback),
        tim_readback_verified: true,
    })
}

fn validate_storage_plan(storage: &NameGlyphPackStoragePlan) -> Result<()> {
    ensure!(
        storage.byte_capacity == storage.cell_count * storage.bytes_per_cell,
        "name glyph pack storage capacity disagrees with its cells"
    );
    ensure!(
        storage.cell_count == storage.cells.len(),
        "name glyph pack storage cell count changed"
    );
    ensure!(
        storage.bytes_per_cell > 0,
        "name glyph pack storage has empty cells"
    );
    Ok(())
}

fn bytes_to_pixels(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|byte| [byte & 0x0f, byte >> 4])
        .collect()
}

fn pixels_to_bytes(pixels: &[u8]) -> Vec<u8> {
    pixels
        .as_chunks::<PIXELS_PER_BYTE>()
        .0
        .iter()
        .map(|pair| pair[0] | (pair[1] << 4))
        .collect()
}
