//! Lossless horizontal bitmap bands for the supported name-glyph repertoire.
//!
//! Serialized offsets are relative to the pack. Membership and rank prefixes
//! map a Hangul syllable to a dense glyph index; each band then selects one
//! exact fill mask. Outlines are generated after materialization.
//! Halfword fields are little-endian byte sequences and may be unaligned;
//! a runtime reader must not assume they can be loaded with LHU.
use anyhow::{Result, ensure};

#[path = "glyph_bands/build.rs"]
mod build;
#[path = "glyph_bands/cache_runtime.rs"]
mod cache_runtime;
#[path = "glyph_bands/runtime.rs"]
mod runtime;
#[cfg(test)]
#[path = "glyph_bands/tests.rs"]
mod tests;

const HEADER: usize = 16;
const DESCRIPTOR: usize = 8;
const SYLLABLES: usize = 11172;
const MEMBERSHIP: usize = SYLLABLES.div_ceil(8);
const RANKS: usize = SYLLABLES.div_ceil(32) * 2;

#[derive(Clone, Debug)]
struct Band {
    start: usize,
    end: usize,
    bits: usize,
    stride: usize,
    dictionary: usize,
    indices: usize,
}

#[derive(Clone, Debug)]
pub struct NameGlyphBandPack {
    bytes: Vec<u8>,
    crop: [usize; 4],
    glyph_count: usize,
    membership: usize,
    ranks: usize,
    bands: Vec<Band>,
}

impl NameGlyphBandPack {
    pub(crate) fn membership_byte_offset(&self) -> usize {
        self.membership
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            bytes.len() >= HEADER && bytes.len() <= u16::MAX as usize,
            "invalid band pack length"
        );
        ensure!(
            &bytes[..4] == b"JGBD" && bytes[11] == 0,
            "unknown band pack format"
        );
        let crop = [
            bytes[4] as usize,
            bytes[5] as usize,
            bytes[6] as usize,
            bytes[7] as usize,
        ];
        let [x, y, width, height] = crop;
        ensure!(
            width > 0 && height > 0 && x + width <= 20 && y + height <= 20,
            "invalid band crop"
        );
        let glyph_count = word(&bytes, 8);
        let count = usize::from(bytes[10]);
        ensure!(
            glyph_count > 0 && glyph_count <= SYLLABLES && count > 0 && count <= height,
            "invalid band population"
        );
        let membership = HEADER + count * DESCRIPTOR;
        let ranks = membership + MEMBERSHIP;
        let mut cursor = ranks + RANKS;
        ensure!(
            word(&bytes, 12) == membership && word(&bytes, 14) == ranks && cursor <= bytes.len(),
            "invalid band lookup ranges"
        );
        ensure!(
            bytes[ranks - 1] & 0xf0 == 0,
            "membership includes non-Hangul scalars"
        );
        let mut rank = 0;
        for start in (0..SYLLABLES).step_by(32) {
            ensure!(
                word(&bytes, ranks + start / 32 * 2) == rank,
                "incorrect band rank prefix"
            );
            rank += bytes[membership + start / 8..membership + (start / 8 + 4).min(MEMBERSHIP)]
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum::<usize>();
        }
        ensure!(rank == glyph_count, "band membership count differs");
        let mut bands = Vec::new();
        let mut previous_end = 0;
        for i in 0..count {
            let offset = HEADER + i * DESCRIPTOR;
            let band = Band {
                start: bytes[offset] as usize,
                end: bytes[offset + 1] as usize,
                bits: bytes[offset + 2] as usize,
                stride: bytes[offset + 3] as usize,
                dictionary: word(&bytes, offset + 4),
                indices: word(&bytes, offset + 6),
            };
            ensure!(
                band.start == previous_end && band.end > band.start && band.end <= height,
                "band rows overlap or leave a gap"
            );
            let pixels = (band.end - band.start) * width;
            ensure!(
                band.stride == pixels.div_ceil(8) && band.bits <= 14,
                "invalid band mask or index width"
            );
            ensure!(
                band.dictionary == cursor && band.indices > cursor && band.indices <= bytes.len(),
                "invalid band dictionary range"
            );
            let dictionary = &bytes[cursor..band.indices];
            ensure!(
                dictionary.len().is_multiple_of(band.stride),
                "partial band dictionary mask"
            );
            let unique = dictionary.len() / band.stride;
            ensure!(
                unique <= glyph_count && band.bits == index_bits(unique),
                "band dictionary count disagrees with index width"
            );
            let mut previous: Option<&[u8]> = None;
            for mask in dictionary.chunks_exact(band.stride) {
                ensure!(
                    previous.is_none_or(|p| p < mask),
                    "band dictionary is not sorted and unique"
                );
                ensure!(padding_is_zero(mask, pixels), "nonzero band mask padding");
                previous = Some(mask);
            }
            cursor = band.indices + (glyph_count * band.bits).div_ceil(8);
            ensure!(cursor <= bytes.len(), "truncated band indices");
            let indices = &bytes[band.indices..cursor];
            ensure!(
                padding_is_zero(indices, glyph_count * band.bits),
                "nonzero band index padding"
            );
            for glyph in 0..glyph_count {
                ensure!(
                    read_bits(indices, glyph * band.bits, band.bits) < unique,
                    "band index leaves dictionary"
                );
            }
            previous_end = band.end;
            bands.push(band);
        }
        ensure!(
            previous_end == height && cursor == bytes.len(),
            "band rows or trailing bytes are unaccounted for"
        );
        Ok(Self {
            bytes,
            crop,
            glyph_count,
            membership,
            ranks,
            bands,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn crop(&self) -> [usize; 4] {
        self.crop
    }
    pub fn glyph_count(&self) -> usize {
        self.glyph_count
    }

    pub(crate) fn repertoire_membership_offset(&self) -> usize {
        self.membership
    }

    pub fn supports(&self, character: char) -> bool {
        let Some(code) = (character as u32).checked_sub(0xac00) else {
            return false;
        };
        (code as usize) < SYLLABLES
            && self.bytes[self.membership + code as usize / 8] & (1 << (code % 8)) != 0
    }

    pub fn render_fill(&self, character: char) -> Result<[u8; 200]> {
        ensure!(
            self.supports(character),
            "unsupported band glyph {character}"
        );
        let code = character as usize - 0xac00;
        let mut rank = word(&self.bytes, self.ranks + code / 32 * 2);
        for scalar in code / 32 * 32..code {
            rank += usize::from((self.bytes[self.membership + scalar / 8] >> (scalar % 8)) & 1);
        }
        let [x, y, width, _] = self.crop;
        let mut output = [0; 200];
        for band in &self.bands {
            let index = read_bits(&self.bytes[band.indices..], rank * band.bits, band.bits);
            let start = band.dictionary + index * band.stride;
            let mask = &self.bytes[start..start + band.stride];
            for bit in 0..(band.end - band.start) * width {
                if mask[bit / 8] & (1 << (bit % 8)) != 0 {
                    let pixel = (y + band.start + bit / width) * 20 + x + bit % width;
                    output[pixel / 2] |= 13 << ((pixel % 2) * 4);
                }
            }
        }
        Ok(output)
    }
}

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}
fn index_bits(unique: usize) -> usize {
    (usize::BITS - (unique - 1).leading_zeros()) as usize
}
fn padding_is_zero(bytes: &[u8], bits: usize) -> bool {
    bits.is_multiple_of(8) || bytes.last().is_some_and(|b| b >> (bits % 8) == 0)
}
fn read_bits(bytes: &[u8], start: usize, count: usize) -> usize {
    (0..count).fold(0, |v, bit| {
        v | (((bytes[(start + bit) / 8] >> ((start + bit) % 8)) & 1) as usize) << bit
    })
}

#[path = "glyph_bands/font_build.rs"]
mod font_build;
pub use font_build::{NameGlyphBandBuild, NameGlyphBandBuildReport, build_name_glyph_band_pack};
