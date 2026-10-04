use std::path::Path;

use anyhow::{Context, Result};

use super::model::PracticalResultAssets;
use super::source_ownership::PracticalResultSourceOwnership;
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;

#[path = "assets/catalog.rs"]
mod catalog;
#[path = "assets/indexed_member_consumer.rs"]
mod indexed_member_consumer;
#[path = "assets/source_atlas_domains.rs"]
mod source_atlas_domains;
#[path = "assets/source_pixels.rs"]
mod source_pixels;

pub(super) use indexed_member_consumer::{
    validate_siken20_indexed_result_consumer_path, validate_siken20_indexed_result_texture_lifetime,
};

pub(super) fn load_practical_result_assets(directory: &Path) -> Result<PracticalResultAssets> {
    catalog::load_practical_result_assets(directory)
}

pub(super) fn validate_practical_result_sources(
    assets: &PracticalResultAssets,
    ownership: &PracticalResultSourceOwnership,
    sources: &[ModeDescendantSourceRecord],
) -> Result<()> {
    source_pixels::validate_practical_result_sources(assets, ownership, sources)
}

fn source_for_path<'a>(
    sources: &'a [ModeDescendantSourceRecord],
    path: &str,
) -> Result<&'a ModeDescendantSourceRecord> {
    sources
        .iter()
        .find(|source| source.path == path)
        .with_context(|| format!("practical-result source {path} was not loaded"))
}

pub(super) fn parse_hex_offset(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("practical-result TIM offset is not hexadecimal")?,
        16,
    )
    .context("invalid practical-result TIM offset")
}
