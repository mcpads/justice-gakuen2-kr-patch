#[path = "title_notice/assets.rs"]
mod assets;
#[path = "title_notice/build.rs"]
mod build;
#[path = "title_notice/consumer.rs"]
mod consumer;
#[path = "title_notice/model.rs"]
mod model;
#[path = "title_notice/source.rs"]
mod source;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::source_disc::SupportedSourceDisc;

pub(crate) use build::build_title_notice_from_source;
pub(crate) use model::{TitleNoticeBuild, TitleNoticeBuildConfig, TitleNoticeFontStyle};

pub(crate) fn load_source_bound_title_notice_characters(
    assets_root: &Path,
    source_disc: &SupportedSourceDisc,
) -> Result<BTreeSet<char>> {
    let source = source::load_title_notice_source_from_disc(source_disc)?;
    let assets = assets::load_assets(assets_root, &source)?;
    Ok(assets
        .entries
        .iter()
        .flat_map(|entry| entry.korean_text.chars())
        .filter(|character| !character.is_whitespace())
        .collect())
}

#[cfg(test)]
#[path = "title_notice_tests.rs"]
mod tests;

pub(crate) fn collect_menu_requests(
    assets_root: &Path,
    source_disc: &SupportedSourceDisc,
) -> Result<Vec<crate::menu_atlas_plan::MenuAtlasRequest>> {
    let requested = load_source_bound_title_notice_characters(assets_root, source_disc)?;
    Ok(requested
        .into_iter()
        .map(|character| crate::menu_atlas_plan::MenuAtlasRequest {
            key: format!("title_notice:{:04x}", character as u32),
            contexts: BTreeSet::from(["mgtit".to_owned()]),
            candidates: crate::menu_glyph_slots::OPTIONS_SMALL_GLYPH_CODE_CANDIDATES
                .into_iter()
                .filter(|code| {
                    crate::menu_glyph_slots::TITLE_NOTICE_VISIBLE_GLYPH_CODE_RANGE.contains(code)
                })
                .collect(),
            width: 20,
            height: 20,
        })
        .collect())
}
