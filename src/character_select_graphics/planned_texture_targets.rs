//! Resolves physical TIMs selected by rendered character-select allocations.

use std::collections::BTreeSet;

use super::model::CharacterSelectSourceInkCleanupAllocation;
use super::render::{RenderedFixedStrip, RenderedGlyph};

pub(super) fn planned_tim_offsets(
    source_path: &str,
    rendered_glyphs: &[RenderedGlyph],
    rendered_fixed_strips: &[RenderedFixedStrip],
    source_ink_cleanups: &[CharacterSelectSourceInkCleanupAllocation],
) -> BTreeSet<usize> {
    rendered_glyphs
        .iter()
        .filter(|glyph| {
            glyph
                .allocation
                .surface
                .targets_record(source_path, glyph.allocation.tim_offset)
        })
        .map(|glyph| glyph.allocation.tim_offset)
        .chain(rendered_fixed_strips.iter().filter_map(|strip| {
            strip
                .allocation
                .surface
                .targets_record(source_path, strip.allocation.tim_offset)
                .then_some(strip.allocation.tim_offset)
        }))
        .chain(source_ink_cleanups.iter().filter_map(|cleanup| {
            cleanup
                .surface
                .targets_record(source_path, cleanup.tim_offset)
                .then_some(cleanup.tim_offset)
        }))
        .collect()
}
