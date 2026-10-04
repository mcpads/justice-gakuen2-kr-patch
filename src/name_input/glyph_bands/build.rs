use super::*;
use crate::font::{RasterizedMenuGlyph, RasterizedMenuGlyphSet};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
struct Plan {
    start: usize,
    end: usize,
    masks: Vec<Vec<u8>>,
    bits: usize,
    dictionary: BTreeSet<Vec<u8>>,
    cost: usize,
}

impl NameGlyphBandPack {
    pub fn build(reference: &RasterizedMenuGlyphSet, capacity: usize) -> Result<Self> {
        ensure!(!reference.glyphs.is_empty(), "empty band glyph repertoire");
        let mut glyphs: Vec<&RasterizedMenuGlyph> = reference.glyphs.iter().collect();
        glyphs.sort_by_key(|g| g.character);
        let mut previous = None;
        let mut bounds = [20, 20, 0, 0];
        for glyph in &glyphs {
            ensure!(
                ('\u{ac00}'..='\u{d7a3}').contains(&glyph.character)
                    && previous != Some(glyph.character)
                    && glyph.pixels.len() == 400
                    && glyph.pixels.iter().all(|p| [0, 3, 13].contains(p))
                    && glyph.pixels.contains(&13),
                "invalid, duplicate or blank band glyph"
            );
            previous = Some(glyph.character);
            for (i, &pixel) in glyph.pixels.iter().enumerate() {
                if pixel == 13 {
                    bounds[0] = bounds[0].min(i % 20);
                    bounds[1] = bounds[1].min(i / 20);
                    bounds[2] = bounds[2].max(i % 20 + 1);
                    bounds[3] = bounds[3].max(i / 20 + 1);
                }
            }
        }
        let [x, y, right, bottom] = bounds;
        let width = right - x;
        let height = bottom - y;
        let mut best: Vec<(usize, Vec<Plan>)> = vec![(usize::MAX, Vec::new()); height + 1];
        best[0].0 = 0;
        for end in 1..=height {
            for start in 0..end {
                let stride = ((end - start) * width).div_ceil(8);
                let masks: Vec<_> = glyphs
                    .iter()
                    .map(|g| {
                        let mut mask = vec![0; stride];
                        for row in start..end {
                            for col in 0..width {
                                if g.pixels[(y + row) * 20 + x + col] == 13 {
                                    let bit = (row - start) * width + col;
                                    mask[bit / 8] |= 1 << (bit % 8);
                                }
                            }
                        }
                        mask
                    })
                    .collect();
                let dictionary: BTreeSet<_> = masks.iter().cloned().collect();
                let bits = index_bits(dictionary.len());
                let cost =
                    DESCRIPTOR + dictionary.len() * stride + (glyphs.len() * bits).div_ceil(8);
                if best[start].0 + cost < best[end].0 {
                    let mut chain = best[start].1.clone();
                    chain.push(Plan {
                        start,
                        end,
                        masks,
                        bits,
                        dictionary,
                        cost,
                    });
                    best[end] = (best[start].0 + cost, chain);
                }
            }
        }
        let plans = &best[height].1;
        let size = HEADER + MEMBERSHIP + RANKS + plans.iter().map(|p| p.cost).sum::<usize>();
        ensure!(
            size <= capacity && size <= u16::MAX as usize,
            "lossless band pack needs {size} bytes, capacity is {capacity}"
        );
        let mut bytes = vec![0; HEADER + plans.len() * DESCRIPTOR];
        bytes[..4].copy_from_slice(b"JGBD");
        bytes[4..8].copy_from_slice(&[x as u8, y as u8, width as u8, height as u8]);
        put_word(&mut bytes, 8, glyphs.len())?;
        bytes[10] = plans.len() as u8;
        let membership = bytes.len();
        bytes.resize(membership + MEMBERSHIP, 0);
        for glyph in &glyphs {
            let code = glyph.character as usize - 0xac00;
            bytes[membership + code / 8] |= 1 << (code % 8);
        }
        let ranks = bytes.len();
        let mut rank = 0;
        for start in (0..SYLLABLES).step_by(32) {
            bytes.extend_from_slice(&(rank as u16).to_le_bytes());
            rank += bytes[membership + start / 8..membership + (start / 8 + 4).min(MEMBERSHIP)]
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum::<usize>();
        }
        put_word(&mut bytes, 12, membership)?;
        put_word(&mut bytes, 14, ranks)?;
        for (i, plan) in plans.iter().enumerate() {
            let dictionary = bytes.len();
            let lookup: BTreeMap<_, _> = plan
                .dictionary
                .iter()
                .enumerate()
                .map(|(i, m)| (m, i))
                .collect();
            for mask in &plan.dictionary {
                bytes.extend_from_slice(mask);
            }
            let indices = bytes.len();
            bytes.resize(indices + (glyphs.len() * plan.bits).div_ceil(8), 0);
            for (glyph, mask) in plan.masks.iter().enumerate() {
                let index = lookup[mask];
                for bit in 0..plan.bits {
                    let pos = glyph * plan.bits + bit;
                    bytes[indices + pos / 8] |= (((index >> bit) & 1) as u8) << (pos % 8);
                }
            }
            let descriptor = HEADER + i * DESCRIPTOR;
            bytes[descriptor..descriptor + 4].copy_from_slice(&[
                plan.start as u8,
                plan.end as u8,
                plan.bits as u8,
                plan.masks[0].len() as u8,
            ]);
            put_word(&mut bytes, descriptor + 4, dictionary)?;
            put_word(&mut bytes, descriptor + 6, indices)?;
        }
        ensure!(
            bytes.len() == size,
            "band pack estimate differs from encoding"
        );
        let pack = Self::parse(bytes)?;
        for glyph in glyphs {
            let fill = pack.render_fill(glyph.character)?;
            for (i, &pixel) in glyph.pixels.iter().enumerate() {
                ensure!(
                    ((fill[i / 2] >> ((i % 2) * 4)) & 15) == if pixel == 13 { 13 } else { 0 },
                    "band fill differs from reference for {}",
                    glyph.character
                );
            }
        }
        Ok(pack)
    }
}
fn put_word(bytes: &mut [u8], offset: usize, value: usize) -> Result<()> {
    bytes[offset..offset + 2].copy_from_slice(&u16::try_from(value)?.to_le_bytes());
    Ok(())
}
