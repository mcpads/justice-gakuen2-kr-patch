use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, ensure};

#[path = "bonus_shop_text_source/asset_audit.rs"]
mod asset_audit;
#[path = "bonus_shop_text_source/asset_sync.rs"]
mod asset_sync;
#[path = "bonus_shop_text_source/build.rs"]
mod build;
#[path = "bonus_shop_text_source/card_results.rs"]
mod card_results;
#[path = "bonus_shop_text_source/delegated_records.rs"]
mod delegated_records;
#[path = "bonus_shop_text_source/glyph_allocation.rs"]
mod glyph_allocation;
#[path = "bonus_shop_text_source/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_shop_text_source/model.rs"]
mod model;
#[path = "bonus_shop_text_source/output.rs"]
mod output;
#[path = "bonus_shop_text_source/overlay_plan.rs"]
mod overlay_plan;
#[path = "bonus_shop_text_source/parser.rs"]
mod parser;
#[path = "bonus_shop_text_source/preview.rs"]
mod preview;
#[path = "bonus_shop_text_source/runtime_glyphs.rs"]
mod runtime_glyphs;
#[path = "bonus_shop_text_source/writer.rs"]
mod writer;

pub use asset_audit::audit_bonus_shop_text_assets;
pub use asset_sync::sync_bonus_shop_text_assets;
pub(crate) use build::build_bonus_shop_text_from_source;
pub use build::{
    BONUS_SHOP_TEXT_BUILD_MANIFEST_FILE, BONUS_SHOP_TEXT_OVERLAY_OUTPUT_FILE, build_bonus_shop_text,
};
pub use model::{
    BonusShopTextAssetAuditConfig, BonusShopTextAssetAuditReport, BonusShopTextAssetSyncConfig,
    BonusShopTextAssetSyncReport, BonusShopTextBuild, BonusShopTextBuildConfig,
    BonusShopTextBuildReport, BonusShopTextFontSource, BonusShopTextFontSources,
    BonusShopTextSourceConfig, BonusShopTextSourceManifest,
};

use crate::bonus_shop_source::{
    BonusShopSource, GLYPH_TIM_OFFSET, GLYPH_TIM_SHA256, OVERLAY_PATH, OVERLAY_RUNTIME_BASE,
    OVERLAY_SOURCE_SHA256, SHOP_UI_PATH, SHOP_UI_SOURCE_DECODED_SHA256, load_source,
};
use crate::dialogue_audit::codebook::load_verified_dialogue_pixel_texts;
use crate::pipeline::sha256_file;
use model::{BonusShopTextSourceTableIndex, BonusShopTextSourceTableReport, ShopTextRole};
use parser::{ParsedShopTextTable, parse_shop_text_tables};
use writer::{prepare_output, write_json, write_source_units};

struct CollectedBonusShopTextSource {
    source: BonusShopSource,
    dialogue_codebook_sha256: String,
    tables: Vec<ParsedShopTextTable>,
}

pub fn initialize_bonus_shop_text_source(
    config: &BonusShopTextSourceConfig,
) -> Result<BonusShopTextSourceManifest> {
    let collected = collect_bonus_shop_text_source(&config.cue, &config.dialogue_codebook)?;
    let source = collected.source;
    let dialogue_codebook_sha256 = collected.dialogue_codebook_sha256;
    let tables = collected.tables;
    prepare_output(&config.output, config.force)?;

    write_source_workspace(&config.output, source, dialogue_codebook_sha256, tables)
}

fn collect_bonus_shop_text_source(
    cue: &Path,
    dialogue_codebook: &Path,
) -> Result<CollectedBonusShopTextSource> {
    let source = load_source(cue)?;
    let (dialogue_codebook_sha256, verified_pixel_texts) = load_verified_dialogue_pixel_texts(
        &source.source_image_path,
        dialogue_codebook,
        &source.source_bin_sha256,
    )?;
    ensure!(
        dialogue_codebook_sha256 == sha256_file(dialogue_codebook)?,
        "dialogue codebook identity changed while acquiring KOUBAI source text"
    );
    let tables = parse_shop_text_tables(
        &source.overlay,
        &source.shop_ui_decoded,
        &verified_pixel_texts,
    )?;
    validate_complete_table_set(&tables)?;
    Ok(CollectedBonusShopTextSource {
        source,
        dialogue_codebook_sha256,
        tables,
    })
}

fn write_source_workspace(
    output: &Path,
    source: BonusShopSource,
    dialogue_codebook_sha256: String,
    tables: Vec<ParsedShopTextTable>,
) -> Result<BonusShopTextSourceManifest> {
    let mut table_reports = Vec::with_capacity(tables.len());
    let mut total_glyph_count = 0usize;
    let mut resolved_glyph_count = 0usize;
    let mut fully_decoded_record_count = 0usize;
    let mut unresolved_record_count = 0usize;
    let mut total_pointer_interval_byte_count = 0usize;
    let mut total_trailing_interval_byte_count = 0usize;
    let mut all_trailing_interval_bytes_zero = true;
    let mut all_targets = BTreeSet::new();
    for table in &tables {
        total_glyph_count += table.total_glyph_count;
        resolved_glyph_count += table.resolved_glyph_count;
        fully_decoded_record_count += table.fully_decoded_record_count;
        unresolved_record_count += table.records.len() - table.fully_decoded_record_count;
        let pointer_interval_byte_count = table
            .records
            .iter()
            .map(|record| record.unit.source_pointer_interval_byte_count)
            .sum::<usize>();
        let trailing_interval_byte_count = table
            .records
            .iter()
            .map(|record| record.unit.trailing_interval_byte_count)
            .sum::<usize>();
        let table_trailing_intervals_are_zero = table
            .records
            .iter()
            .all(|record| record.unit.trailing_interval_all_zero);
        total_pointer_interval_byte_count += pointer_interval_byte_count;
        total_trailing_interval_byte_count += trailing_interval_byte_count;
        all_trailing_interval_bytes_zero &= table_trailing_intervals_are_zero;
        all_targets.extend(table.records.iter().map(|record| record.source_offset));

        let unit_refs = write_source_units(output, table)?;
        let contact_sheets = preview::write_contact_sheets(output, table, &source.shop_ui_decoded)?;
        let index = BonusShopTextSourceTableIndex {
            kind: "Justice Gakuen 2 bonus-shop source table index".to_string(),
            source_bin_sha256: source.source_bin_sha256.clone(),
            source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
            source_glyph_tim_sha256: GLYPH_TIM_SHA256.to_string(),
            role: table.spec.role,
            pointer_table_range: table.spec.pointer_table_range.map(hex_offset),
            record_region: table.spec.record_region.map(hex_offset),
            record_count: table.records.len(),
            fully_decoded_record_count: table.fully_decoded_record_count,
            pointer_interval_byte_count,
            trailing_interval_byte_count,
            all_trailing_interval_bytes_zero: table_trailing_intervals_are_zero,
            total_glyph_count: table.total_glyph_count,
            resolved_glyph_count: table.resolved_glyph_count,
            unresolved_glyph_count: table.total_glyph_count - table.resolved_glyph_count,
            units: unit_refs,
            contact_sheets,
        };
        let index_path = format!("{}/index.json", table.spec.role.slug());
        let (index_sha256, _) = write_json(output, Path::new(&index_path), &index)?;
        table_reports.push(BonusShopTextSourceTableReport {
            role: table.spec.role,
            pointer_table_range: table.spec.pointer_table_range.map(hex_offset),
            record_region: table.spec.record_region.map(hex_offset),
            record_count: table.records.len(),
            unique_record_target_count: table
                .records
                .iter()
                .map(|record| record.source_offset)
                .collect::<BTreeSet<_>>()
                .len(),
            pointer_interval_byte_count,
            trailing_interval_byte_count,
            all_trailing_interval_bytes_zero: table_trailing_intervals_are_zero,
            index_path,
            index_sha256,
        });
    }

    let total_record_count = tables
        .iter()
        .map(|table| table.records.len())
        .sum::<usize>();
    let acquisition_complete = total_record_count == 186
        && table_reports.iter().all(|table| {
            table.record_count == table.unique_record_target_count
                && table.record_count == table.role.expected_record_count()
        });
    ensure!(
        acquisition_complete,
        "KOUBAI source acquisition did not cover the complete 83 + 83 + 20 pointer set"
    );
    let manifest = BonusShopTextSourceManifest {
        kind: "Justice Gakuen 2 complete sharded bonus-shop source text workspace".to_string(),
        implementation: "Rust source-bound KOUBAI pointer-table parser and 4-bpp contact-sheet renderer"
            .to_string(),
        source_bin_sha256: source.source_bin_sha256,
        dialogue_codebook_sha256,
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        source_shop_ui_path: SHOP_UI_PATH.to_string(),
        source_shop_ui_decoded_sha256: SHOP_UI_SOURCE_DECODED_SHA256.to_string(),
        source_glyph_tim_offset: hex_offset(GLYPH_TIM_OFFSET),
        source_glyph_tim_sha256: GLYPH_TIM_SHA256.to_string(),
        overlay_runtime_base: format!("0x{OVERLAY_RUNTIME_BASE:08x}"),
        table_count: tables.len(),
        total_record_count,
        unique_record_target_count: all_targets.len(),
        total_pointer_interval_byte_count,
        total_trailing_interval_byte_count,
        all_trailing_interval_bytes_zero,
        total_glyph_count,
        resolved_glyph_count,
        unresolved_glyph_count: total_glyph_count - resolved_glyph_count,
        fully_decoded_record_count,
        unresolved_record_count,
        acquisition_complete,
        tables: table_reports,
        limitations: vec![
            "Acquisition completeness covers every pointer entry in the three source-bound KOUBAI tables; it does not claim that every screen selecting those entries has been exercised at runtime."
                .to_string(),
            "Exact source characters are inherited only from byte-identical verified dialogue glyph pixels; unresolved glyphs remain explicit and visible in the contact sheets."
                .to_string(),
            "No Korean translation text is generated or inferred by this command."
                .to_string(),
            "Pointer intervals and their trailing bytes are measured as source evidence only; zero-filled trailing bytes are not declared reclaimable translation capacity without a separate consumer and ownership proof."
                .to_string(),
        ],
    };
    write_json(output, Path::new("manifest.json"), &manifest)?;
    Ok(manifest)
}

fn validate_complete_table_set(tables: &[ParsedShopTextTable]) -> Result<()> {
    ensure!(tables.len() == 3, "KOUBAI source table count changed");
    let actual = tables
        .iter()
        .map(|table| (table.spec.role, table.records.len()))
        .collect::<BTreeMap<_, _>>();
    let expected = ShopTextRole::ALL
        .into_iter()
        .map(|role| (role, role.expected_record_count()))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        actual == expected,
        "KOUBAI source table cardinalities changed"
    );
    Ok(())
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}

#[cfg(test)]
#[path = "bonus_shop_text_source/asset_audit_tests.rs"]
mod asset_audit_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source/asset_sync_tests.rs"]
mod asset_sync_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source/glyph_allocation_tests.rs"]
mod glyph_allocation_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source/overlay_plan_tests.rs"]
mod overlay_plan_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source/runtime_glyphs_tests.rs"]
mod runtime_glyphs_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_shop_text_source_tests.rs"]
mod tests;
