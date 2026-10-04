use anyhow::{Context, Result, ensure};

use super::MODERN_HANGUL_START;
use super::format::{
    BASE_MEMBERSHIP_BYTES, BASE_RANK_PREFIX_BYTES, FILL_PALETTE_INDEX, FINAL_COUNT,
    FINAL_MEMBERSHIP_BYTES, FINAL_RANK_PREFIX_BYTES, HEADER_BYTES, MEDIAL_COUNT,
    MODERN_HANGUL_COUNT, OUTLINE_PALETTE_INDEX, PackHeader, REPERTOIRE_MEMBERSHIP_BYTES,
    has_membership, membership_count, membership_rank, prefixed_membership_rank_u8,
    prefixed_membership_rank_u16,
};

const CELL_WIDTH: usize = 20;
const CELL_HEIGHT: usize = 20;

pub struct NameGlyphPackDecoder<'a> {
    bytes: &'a [u8],
    header: PackHeader,
    occupied_coordinates: Vec<usize>,
    repertoire_membership: &'a [u8],
    no_final_membership: &'a [u8],
    final_bearing_membership: &'a [u8],
    final_membership: &'a [u8],
    no_final_rank_prefix: &'a [u8],
    final_bearing_rank_prefix: &'a [u8],
    final_rank_prefix: &'a [u8],
    component_masks: &'a [u8],
}

impl<'a> NameGlyphPackDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        let header = PackHeader::decode(bytes)?;
        ensure!(
            header.crop_x + header.crop_width <= CELL_WIDTH
                && header.crop_y + header.crop_height <= CELL_HEIGHT,
            "name glyph pack crop exceeds its 20x20 cell"
        );
        ensure!(
            header.bytes_per_mask == header.occupied_coordinate_count.div_ceil(8),
            "name glyph pack component width disagrees with occupied coordinates"
        );
        ensure!(
            header.total_bytes == bytes.len(),
            "name glyph pack length disagrees with its header"
        );

        let coordinate_membership_end = HEADER_BYTES + header.coordinate_membership_bytes();
        let coordinate_membership = bytes
            .get(HEADER_BYTES..coordinate_membership_end)
            .context("truncated name glyph coordinate membership")?;
        let occupied_coordinates = (0..header.crop_width * header.crop_height)
            .filter(|coordinate| has_membership(coordinate_membership, *coordinate))
            .collect::<Vec<_>>();
        ensure!(
            occupied_coordinates.len() == header.occupied_coordinate_count,
            "name glyph occupied-coordinate count changed"
        );

        let repertoire_end = coordinate_membership_end + REPERTOIRE_MEMBERSHIP_BYTES;
        let no_final_end = repertoire_end + BASE_MEMBERSHIP_BYTES;
        let final_bearing_end = no_final_end + BASE_MEMBERSHIP_BYTES;
        let final_end = final_bearing_end + FINAL_MEMBERSHIP_BYTES;
        let no_final_rank_end = final_end + BASE_RANK_PREFIX_BYTES;
        let final_bearing_rank_end = no_final_rank_end + BASE_RANK_PREFIX_BYTES;
        let final_rank_end = final_bearing_rank_end + FINAL_RANK_PREFIX_BYTES;
        let repertoire_membership = bytes
            .get(coordinate_membership_end..repertoire_end)
            .context("truncated name glyph repertoire membership")?;
        let no_final_membership = bytes
            .get(repertoire_end..no_final_end)
            .context("truncated no-final name glyph membership")?;
        let final_bearing_membership = bytes
            .get(no_final_end..final_bearing_end)
            .context("truncated final-bearing name glyph membership")?;
        let final_membership = bytes
            .get(final_bearing_end..final_end)
            .context("truncated final name glyph membership")?;
        let no_final_rank_prefix = bytes
            .get(final_end..no_final_rank_end)
            .context("truncated no-final rank prefix")?;
        let final_bearing_rank_prefix = bytes
            .get(no_final_rank_end..final_bearing_rank_end)
            .context("truncated final-bearing rank prefix")?;
        let final_rank_prefix = bytes
            .get(final_bearing_rank_end..final_rank_end)
            .context("truncated final rank prefix")?;
        ensure!(
            membership_count(repertoire_membership) == header.supported_syllable_count,
            "name glyph repertoire population disagrees with its header"
        );
        ensure!(
            membership_count(no_final_membership) == header.no_final_base_count
                && membership_count(final_bearing_membership) == header.final_bearing_base_count
                && membership_count(final_membership) == header.final_component_count,
            "name glyph component populations disagree with their header"
        );
        validate_rank_prefix_u16(no_final_membership, no_final_rank_prefix)?;
        validate_rank_prefix_u16(final_bearing_membership, final_bearing_rank_prefix)?;
        validate_rank_prefix_u8(final_membership, final_rank_prefix)?;
        let component_masks = bytes
            .get(final_rank_end..)
            .context("truncated name glyph component section")?;
        let component_count = header.no_final_base_count
            + header.final_bearing_base_count
            + header.final_component_count;
        ensure!(
            component_masks.len() == component_count * header.bytes_per_mask,
            "name glyph component section length changed"
        );
        Ok(Self {
            bytes,
            header,
            occupied_coordinates,
            repertoire_membership,
            no_final_membership,
            final_bearing_membership,
            final_membership,
            no_final_rank_prefix,
            final_bearing_rank_prefix,
            final_rank_prefix,
            component_masks,
        })
    }

    pub(super) fn crop_bounds(&self) -> [usize; 4] {
        [
            self.header.crop_x,
            self.header.crop_y,
            self.header.crop_width,
            self.header.crop_height,
        ]
    }

    pub(crate) fn repertoire_membership_offset(&self) -> usize {
        HEADER_BYTES + self.header.coordinate_membership_bytes()
    }

    pub fn supports(&self, character: char) -> bool {
        syllable_index(character).is_some_and(|index| {
            index < MODERN_HANGUL_COUNT && has_membership(self.repertoire_membership, index)
        })
    }

    pub fn render(&self, character: char) -> Result<Vec<u8>> {
        let index = syllable_index(character)
            .with_context(|| format!("character {character:?} is not modern Hangul"))?;
        ensure!(
            index < MODERN_HANGUL_COUNT && has_membership(self.repertoire_membership, index),
            "Hangul character {character:?} is outside the name glyph repertoire"
        );
        let initial = index / (MEDIAL_COUNT * FINAL_COUNT);
        let medial = index / FINAL_COUNT % MEDIAL_COUNT;
        let final_consonant = index % FINAL_COUNT;
        let base_key = initial * MEDIAL_COUNT + medial;
        let fill_mask = if final_consonant == 0 {
            ensure!(
                has_membership(self.no_final_membership, base_key),
                "supported no-final name glyph lacks a base component"
            );
            self.component_mask(prefixed_membership_rank_u16(
                self.no_final_membership,
                self.no_final_rank_prefix,
                base_key,
            ))
            .to_vec()
        } else {
            ensure!(
                has_membership(self.final_bearing_membership, base_key),
                "supported final-bearing name glyph lacks a base component"
            );
            let final_key = medial * (FINAL_COUNT - 1) + final_consonant - 1;
            ensure!(
                has_membership(self.final_membership, final_key),
                "supported final-bearing name glyph lacks a final component"
            );
            let base_index = self.header.no_final_base_count
                + prefixed_membership_rank_u16(
                    self.final_bearing_membership,
                    self.final_bearing_rank_prefix,
                    base_key,
                );
            let final_index = self.header.no_final_base_count
                + self.header.final_bearing_base_count
                + prefixed_membership_rank_u8(
                    self.final_membership,
                    self.final_rank_prefix,
                    final_key,
                );
            let mut mask = self.component_mask(base_index).to_vec();
            for (target, final_component) in mask.iter_mut().zip(self.component_mask(final_index)) {
                *target |= final_component;
            }
            mask
        };
        if !self.occupied_coordinates.len().is_multiple_of(8) {
            let used_bits = self.occupied_coordinates.len() % 8;
            let padding_mask = !((1_u8 << used_bits) - 1);
            ensure!(
                fill_mask
                    .last()
                    .is_some_and(|last| last & padding_mask == 0),
                "name glyph component uses a padding bit"
            );
        }
        let mut fill = vec![false; CELL_WIDTH * CELL_HEIGHT];
        for (bit, coordinate) in self.occupied_coordinates.iter().copied().enumerate() {
            if fill_mask[bit / 8] & (1 << (bit % 8)) == 0 {
                continue;
            }
            let x = self.header.crop_x + coordinate % self.header.crop_width;
            let y = self.header.crop_y + coordinate / self.header.crop_width;
            fill[y * CELL_WIDTH + x] = true;
        }
        ensure!(
            fill.iter().any(|pixel| *pixel),
            "name glyph component is blank"
        );
        let outline = dilate(&fill);
        Ok((0..CELL_WIDTH * CELL_HEIGHT)
            .map(|pixel| {
                if fill[pixel] {
                    FILL_PALETTE_INDEX
                } else if outline[pixel] {
                    OUTLINE_PALETTE_INDEX
                } else {
                    0
                }
            })
            .collect())
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    fn component_mask(&self, index: usize) -> &'a [u8] {
        let start = index * self.header.bytes_per_mask;
        &self.component_masks[start..start + self.header.bytes_per_mask]
    }
}

fn validate_rank_prefix_u16(membership: &[u8], prefixes: &[u8]) -> Result<()> {
    ensure!(
        prefixes.len() == membership.len() * 2,
        "name glyph two-byte rank prefix length changed"
    );
    for index in 0..membership.len() * 8 {
        ensure!(
            prefixed_membership_rank_u16(membership, prefixes, index)
                == membership_rank(membership, index),
            "name glyph two-byte rank prefix changed at bit {index}"
        );
    }
    Ok(())
}

fn validate_rank_prefix_u8(membership: &[u8], prefixes: &[u8]) -> Result<()> {
    ensure!(
        prefixes.len() == membership.len(),
        "name glyph one-byte rank prefix length changed"
    );
    for index in 0..membership.len() * 8 {
        ensure!(
            prefixed_membership_rank_u8(membership, prefixes, index)
                == membership_rank(membership, index),
            "name glyph one-byte rank prefix changed at bit {index}"
        );
    }
    Ok(())
}

fn syllable_index(character: char) -> Option<usize> {
    u32::from(character)
        .checked_sub(MODERN_HANGUL_START)
        .and_then(|index| usize::try_from(index).ok())
}

fn dilate(fill: &[bool]) -> Vec<bool> {
    let mut output = fill.to_vec();
    for y in 0..CELL_HEIGHT {
        for x in 0..CELL_WIDTH {
            if !fill[y * CELL_WIDTH + x] {
                continue;
            }
            for dy in -1_i32..=1 {
                for dx in -1_i32..=1 {
                    let next_x = x as i32 + dx;
                    let next_y = y as i32 + dy;
                    if next_x >= 0
                        && next_y >= 0
                        && next_x < CELL_WIDTH as i32
                        && next_y < CELL_HEIGHT as i32
                    {
                        output[next_y as usize * CELL_WIDTH + next_x as usize] = true;
                    }
                }
            }
        }
    }
    output
}
