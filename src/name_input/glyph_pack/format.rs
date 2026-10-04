use anyhow::{Result, ensure};

pub(super) const MAGIC: [u8; 4] = *b"JGNM";
pub(super) const HEADER_BYTES: usize = 24;
pub(super) const MODERN_HANGUL_COUNT: usize = 11_172;
pub(super) const INITIAL_COUNT: usize = 19;
pub(super) const MEDIAL_COUNT: usize = 21;
pub(super) const FINAL_COUNT: usize = 28;
pub(super) const BASE_KEY_COUNT: usize = INITIAL_COUNT * MEDIAL_COUNT;
pub(super) const FINAL_KEY_COUNT: usize = MEDIAL_COUNT * (FINAL_COUNT - 1);
pub(super) const REPERTOIRE_MEMBERSHIP_BYTES: usize = MODERN_HANGUL_COUNT.div_ceil(8);
pub(super) const BASE_MEMBERSHIP_BYTES: usize = BASE_KEY_COUNT.div_ceil(8);
pub(super) const FINAL_MEMBERSHIP_BYTES: usize = FINAL_KEY_COUNT.div_ceil(8);
pub(super) const BASE_RANK_PREFIX_BYTES: usize = BASE_MEMBERSHIP_BYTES * 2;
pub(super) const FINAL_RANK_PREFIX_BYTES: usize = FINAL_MEMBERSHIP_BYTES;
pub(super) const OUTLINE_PALETTE_INDEX: u8 = 3;
pub(super) const FILL_PALETTE_INDEX: u8 = 13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PackHeader {
    pub(super) crop_x: usize,
    pub(super) crop_y: usize,
    pub(super) crop_width: usize,
    pub(super) crop_height: usize,
    pub(super) occupied_coordinate_count: usize,
    pub(super) bytes_per_mask: usize,
    pub(super) supported_syllable_count: usize,
    pub(super) no_final_base_count: usize,
    pub(super) final_bearing_base_count: usize,
    pub(super) final_component_count: usize,
    pub(super) total_bytes: usize,
}

impl PackHeader {
    pub(super) fn encode(self) -> Result<[u8; HEADER_BYTES]> {
        let mut output = [0_u8; HEADER_BYTES];
        output[0..4].copy_from_slice(&MAGIC);
        output[4] = u8::try_from(self.crop_x)?;
        output[5] = u8::try_from(self.crop_y)?;
        output[6] = u8::try_from(self.crop_width)?;
        output[7] = u8::try_from(self.crop_height)?;
        write_u16(&mut output, 8, self.occupied_coordinate_count)?;
        write_u16(&mut output, 10, self.bytes_per_mask)?;
        write_u16(&mut output, 12, self.supported_syllable_count)?;
        write_u16(&mut output, 14, self.no_final_base_count)?;
        write_u16(&mut output, 16, self.final_bearing_base_count)?;
        write_u16(&mut output, 18, self.final_component_count)?;
        write_u16(&mut output, 20, self.total_bytes)?;
        output[22] = OUTLINE_PALETTE_INDEX;
        output[23] = FILL_PALETTE_INDEX;
        Ok(output)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() >= HEADER_BYTES,
            "truncated name glyph pack header"
        );
        ensure!(bytes[0..4] == MAGIC, "unknown name glyph pack magic");
        ensure!(
            bytes[22] == OUTLINE_PALETTE_INDEX && bytes[23] == FILL_PALETTE_INDEX,
            "name glyph pack palette indices changed"
        );
        Ok(Self {
            crop_x: usize::from(bytes[4]),
            crop_y: usize::from(bytes[5]),
            crop_width: usize::from(bytes[6]),
            crop_height: usize::from(bytes[7]),
            occupied_coordinate_count: read_u16(bytes, 8),
            bytes_per_mask: read_u16(bytes, 10),
            supported_syllable_count: read_u16(bytes, 12),
            no_final_base_count: read_u16(bytes, 14),
            final_bearing_base_count: read_u16(bytes, 16),
            final_component_count: read_u16(bytes, 18),
            total_bytes: read_u16(bytes, 20),
        })
    }

    pub(super) fn coordinate_membership_bytes(self) -> usize {
        (self.crop_width * self.crop_height).div_ceil(8)
    }
}

pub(super) fn set_membership(membership: &mut [u8], index: usize) {
    membership[index / 8] |= 1 << (index % 8);
}

pub(super) fn has_membership(membership: &[u8], index: usize) -> bool {
    membership[index / 8] & (1 << (index % 8)) != 0
}

pub(super) fn membership_count(membership: &[u8]) -> usize {
    membership
        .iter()
        .map(|value| value.count_ones() as usize)
        .sum()
}

pub(super) fn membership_rank(membership: &[u8], index: usize) -> usize {
    membership[..index / 8]
        .iter()
        .map(|value| value.count_ones() as usize)
        .sum::<usize>()
        + (membership[index / 8] & ((1 << (index % 8)) - 1)).count_ones() as usize
}

pub(super) fn rank_prefix_u16(membership: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(membership.len() * 2);
    let mut rank = 0_u16;
    for byte in membership {
        output.extend_from_slice(&rank.to_le_bytes());
        rank += u16::try_from(byte.count_ones()).expect("one byte population fits u16");
    }
    output
}

pub(super) fn rank_prefix_u8(membership: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(membership.len());
    let mut rank = 0_u8;
    for byte in membership {
        output.push(rank);
        rank = rank
            .checked_add(u8::try_from(byte.count_ones()).expect("one byte population fits u8"))
            .expect("rank prefix exceeds one byte");
    }
    output
}

pub(super) fn prefixed_membership_rank_u16(
    membership: &[u8],
    prefixes: &[u8],
    index: usize,
) -> usize {
    let byte_index = index / 8;
    let prefix_offset = byte_index * 2;
    usize::from(u16::from_le_bytes([
        prefixes[prefix_offset],
        prefixes[prefix_offset + 1],
    ])) + (membership[byte_index] & ((1 << (index % 8)) - 1)).count_ones() as usize
}

pub(super) fn prefixed_membership_rank_u8(
    membership: &[u8],
    prefixes: &[u8],
    index: usize,
) -> usize {
    let byte_index = index / 8;
    usize::from(prefixes[byte_index])
        + (membership[byte_index] & ((1 << (index % 8)) - 1)).count_ones() as usize
}

fn write_u16(output: &mut [u8], offset: usize, value: usize) -> Result<()> {
    output[offset..offset + 2].copy_from_slice(&u16::try_from(value)?.to_le_bytes());
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}
