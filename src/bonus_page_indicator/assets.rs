use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::model::{BonusPageIndicatorAsset, DevelopmentStatus, ReleaseStatus};
use super::overlay::{
    CONSUMER_RUNTIME_ADDRESS, CURRENT_PAGE_LOOKUP_OFFSET, SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
    SUFFIX_TABLE_OFFSET, TOTAL_PAGE_LOOKUP_OFFSET,
};
use super::source::{
    BONUS_PAGE_INDICATOR_INVENTORY_PATH, BONUS_PAGE_INDICATOR_OVERLAY_PATH, GLYPH_TIM_OFFSET,
    INVENTORY_SOURCE_DECODED_SHA256, INVENTORY_SOURCE_STORED_SHA256, OVERLAY_SOURCE_SHA256,
    SOURCE_SUFFIX_GLYPH_CODE, SOURCE_SUFFIX_GLYPH_PACKED_SHA256,
};

const ASSET_KIND: &str = "justice_gakuen2_bonus_page_indicator_unit";
const ASSET_ID: &str = "card_viewer_page_suffix";
const RUNTIME_CONSUMER: &str = "bonus_inventory_card_viewer_page_indicator";
const SOURCE_TEXT: &str = "ページ";

pub(super) struct LoadedBonusPageIndicatorAsset {
    pub(super) sha256: String,
    pub(super) asset: BonusPageIndicatorAsset,
}

pub(super) fn load_asset(path: &Path) -> Result<LoadedBonusPageIndicatorAsset> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let asset: BonusPageIndicatorAsset = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_asset(&asset)?;
    Ok(LoadedBonusPageIndicatorAsset {
        sha256: sha256_bytes(&bytes),
        asset,
    })
}

pub(super) fn validate_asset(asset: &BonusPageIndicatorAsset) -> Result<()> {
    ensure!(
        asset.kind == ASSET_KIND && asset.id == ASSET_ID,
        "bonus page-indicator asset identity changed"
    );
    ensure!(
        asset.source.inventory_path == BONUS_PAGE_INDICATOR_INVENTORY_PATH
            && asset.source.inventory_stored_sha256 == INVENTORY_SOURCE_STORED_SHA256
            && asset.source.inventory_decoded_sha256 == INVENTORY_SOURCE_DECODED_SHA256
            && parse_hex_usize(&asset.source.glyph_tim_offset)? == GLYPH_TIM_OFFSET
            && parse_hex_u16(&asset.source.suffix_glyph_code)? == SOURCE_SUFFIX_GLYPH_CODE
            && asset.source.suffix_glyph_packed_sha256 == SOURCE_SUFFIX_GLYPH_PACKED_SHA256,
        "bonus page-indicator glyph source binding changed"
    );
    ensure!(
        asset.source.overlay_path == BONUS_PAGE_INDICATOR_OVERLAY_PATH
            && asset.source.overlay_sha256 == OVERLAY_SOURCE_SHA256
            && parse_hex_u32(&asset.source.consumer_runtime_address)? == CONSUMER_RUNTIME_ADDRESS
            && parse_hex_usize(&asset.source.suffix_table_offset)? == SUFFIX_TABLE_OFFSET
            && parse_hex_usize(&asset.source.current_page_lookup_offset)?
                == CURRENT_PAGE_LOOKUP_OFFSET
            && parse_hex_usize(&asset.source.total_page_lookup_offset)? == TOTAL_PAGE_LOOKUP_OFFSET
            && parse_hex_usize(&asset.source.suffix_loop_bound_instruction_offset)?
                == SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
        "bonus page-indicator overlay source binding changed"
    );
    ensure!(
        asset.runtime_consumer == RUNTIME_CONSUMER && asset.source_text == SOURCE_TEXT,
        "bonus page-indicator consumer meaning changed"
    );
    match asset.development_status {
        DevelopmentStatus::Authored => ensure!(
            asset.korean_text.as_deref().is_some_and(|text| {
                let mut characters = text.chars();
                characters
                    .next()
                    .is_some_and(|character| ('가'..='힣').contains(&character))
                    && characters.next().is_none()
            }) && asset.release_status != ReleaseStatus::Untranslated,
            "authored page-indicator suffix must be exactly one Korean glyph"
        ),
        DevelopmentStatus::Untranslated => ensure!(
            asset.korean_text.is_none() && asset.release_status == ReleaseStatus::Untranslated,
            "untranslated page-indicator suffix contains translated state"
        ),
    }
    ensure!(
        asset.development_status == DevelopmentStatus::Authored,
        "page-indicator build has no authored Korean suffix"
    );
    Ok(())
}

fn parse_hex_usize(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("page-indicator offset is not hexadecimal")?,
        16,
    )
    .context("invalid page-indicator offset")
}

fn parse_hex_u32(value: &str) -> Result<u32> {
    u32::try_from(parse_hex_usize(value)?).context("page-indicator address exceeds u32")
}

fn parse_hex_u16(value: &str) -> Result<u16> {
    u16::try_from(parse_hex_usize(value)?).context("page-indicator glyph code exceeds u16")
}
