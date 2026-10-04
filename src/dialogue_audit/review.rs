use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};

use super::atlas::{
    GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH, parse_dialogue_atlas,
};
use super::codebook::{
    audit_codebook_bytes, resolved_dialogue_glyphs, runtime_source_glyph_inventory,
};
use super::preview::{render_cell, write_grayscale_png};
use super::review_context::{
    CONTEXT_TOKEN_RADIUS, MAXIMUM_CONTEXTS_PER_GLYPH, collect_review_contexts,
};
use super::review_model::{
    DialogueCodebookReviewConfig, DialogueCodebookReviewManifest, DialogueCodebookReviewPage,
    DialogueCodebookReviewShard, DialogueCodebookReviewSlot,
};
use super::sources::load_dialogue_runtime_image_sources;

const REVIEW_COLUMNS: usize = 8;
const REVIEW_ROWS: usize = 8;
const REVIEW_SCALE: usize = 4;
const REVIEW_GUTTER: usize = 24;
const REVIEW_SLOTS_PER_PAGE: usize = REVIEW_COLUMNS * REVIEW_ROWS;
const MAXIMUM_REVIEW_SHARD_BYTES: usize = 512 * 1024;

pub fn build_dialogue_codebook_review(
    config: &DialogueCodebookReviewConfig,
) -> Result<DialogueCodebookReviewManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let (source_pixel_hashes, usage) = runtime_source_glyph_inventory(&cue.image_path)?;
    let codebook_bytes = std::fs::read(&config.codebook)
        .with_context(|| format!("failed to read {}", config.codebook.display()))?;
    let coverage = audit_codebook_bytes(
        &codebook_bytes,
        &source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    let unresolved_pixel_hashes = coverage
        .unresolved
        .iter()
        .map(|glyph| glyph.pixel_sha256.clone())
        .collect::<BTreeSet<_>>();
    let resolved_glyphs = resolved_dialogue_glyphs(&codebook_bytes)?;
    let mut contexts_by_pixel =
        collect_review_contexts(&cue.image_path, &resolved_glyphs, &unresolved_pixel_hashes)?;
    let cells = source_glyph_cells(&cue.image_path)?;
    ensure!(
        cells.keys().cloned().collect::<BTreeSet<_>>() == source_pixel_hashes,
        "review cell population differs from the codebook source inventory"
    );
    std::fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("failed to create {}", config.output_dir.display()))?;

    let mut pages = Vec::new();
    for (page_offset, unresolved) in coverage
        .unresolved
        .chunks(REVIEW_SLOTS_PER_PAGE)
        .enumerate()
    {
        let page_index = page_offset + 1;
        let (width, height) = review_page_dimensions();
        let mut pixels = vec![0xff; width * height];
        let mut slots = Vec::with_capacity(unresolved.len());
        for (slot_index, glyph) in unresolved.iter().enumerate() {
            let (origin_x, origin_y) = review_slot_origin(slot_index);
            render_cell(
                &cells[&glyph.pixel_sha256],
                &mut pixels,
                width,
                origin_x,
                origin_y,
                REVIEW_SCALE,
            );
            slots.push(DialogueCodebookReviewSlot {
                page_index,
                slot_index,
                pixel_sha256: glyph.pixel_sha256.clone(),
                occurrence_count: glyph.occurrence_count,
                source_references: glyph.source_references.clone(),
                contexts: contexts_by_pixel
                    .remove(&glyph.pixel_sha256)
                    .with_context(|| {
                        format!("{} review contexts disappeared", glyph.pixel_sha256)
                    })?,
            });
        }
        let filename = format!("dialogue-codebook-review-{page_index:03}.png");
        let png_path = config.output_dir.join(&filename);
        write_grayscale_png(&png_path, width, height, &pixels)?;
        let shard_filename = format!("dialogue-codebook-review-{page_index:03}.json");
        let shard = DialogueCodebookReviewShard {
            kind: "Dialogue runtime-image glyph review context shard".to_string(),
            page_index,
            slot_count: slots.len(),
            slots,
        };
        let mut shard_bytes = serde_json::to_vec_pretty(&shard)?;
        shard_bytes.push(b'\n');
        ensure!(
            shard_bytes.len() <= MAXIMUM_REVIEW_SHARD_BYTES,
            "dialogue codebook review shard {page_index} exceeds {MAXIMUM_REVIEW_SHARD_BYTES} bytes"
        );
        std::fs::write(config.output_dir.join(&shard_filename), &shard_bytes)?;
        pages.push(DialogueCodebookReviewPage {
            page_index,
            png: filename,
            png_sha256: sha256_file(&png_path)?,
            shard: shard_filename,
            shard_sha256: sha256_bytes(&shard_bytes),
            shard_byte_count: shard_bytes.len(),
            slot_count: shard.slot_count,
        });
    }
    ensure!(
        contexts_by_pixel.is_empty(),
        "review contexts remain after every unresolved pixel was sharded"
    );

    let manifest = DialogueCodebookReviewManifest {
        kind: "Unresolved dialogue runtime-image glyph review pages".to_string(),
        implementation: "independent Rust original-media review rendering".to_string(),
        source_bin_sha256,
        codebook_sha256: coverage.codebook_sha256,
        unresolved_used_source_pixel_hash_count: coverage.unresolved_used_source_pixel_hash_count,
        unresolved_glyph_occurrence_count: coverage.unresolved_glyph_occurrence_count,
        cell_width: GLYPH_CELL_WIDTH,
        cell_height: GLYPH_CELL_HEIGHT,
        scale: REVIEW_SCALE,
        gutter: REVIEW_GUTTER,
        columns: REVIEW_COLUMNS,
        slots_per_page: REVIEW_SLOTS_PER_PAGE,
        coordinate_rule: "slot_index = row * columns + column; rows and columns are zero-based"
            .to_string(),
        ordering_rule: "descending glyph occurrence count, then ascending pixel SHA-256"
            .to_string(),
        maximum_contexts_per_glyph: MAXIMUM_CONTEXTS_PER_GLYPH,
        context_token_radius: CONTEXT_TOKEN_RADIUS,
        maximum_shard_bytes: MAXIMUM_REVIEW_SHARD_BYTES,
        pages,
    };
    std::fs::write(
        config.output_dir.join("dialogue-codebook-review.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(manifest)
}

pub(super) fn review_page_dimensions() -> (usize, usize) {
    let tile_width = GLYPH_CELL_WIDTH * REVIEW_SCALE;
    let tile_height = GLYPH_CELL_HEIGHT * REVIEW_SCALE;
    (
        REVIEW_GUTTER + REVIEW_COLUMNS * (tile_width + REVIEW_GUTTER),
        REVIEW_GUTTER + REVIEW_ROWS * (tile_height + REVIEW_GUTTER),
    )
}

pub(super) fn review_slot_origin(slot_index: usize) -> (usize, usize) {
    let tile_width = GLYPH_CELL_WIDTH * REVIEW_SCALE;
    let tile_height = GLYPH_CELL_HEIGHT * REVIEW_SCALE;
    (
        REVIEW_GUTTER + (slot_index % REVIEW_COLUMNS) * (tile_width + REVIEW_GUTTER),
        REVIEW_GUTTER + (slot_index / REVIEW_COLUMNS) * (tile_height + REVIEW_GUTTER),
    )
}

fn source_glyph_cells(image_path: &std::path::Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut cells = BTreeMap::new();
    for source in load_dialogue_runtime_image_sources(image_path)? {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        for (code, pixel_sha256) in atlas.fixed_cell_sha256.iter().enumerate() {
            let start = atlas.pixel_data_offset + code * GLYPH_CELL_BYTE_COUNT;
            let cell = decoded[start..start + GLYPH_CELL_BYTE_COUNT].to_vec();
            if let Some(existing) = cells.insert(pixel_sha256.clone(), cell.clone()) {
                ensure!(
                    existing == cell,
                    "identical dialogue pixel hash has different source bytes"
                );
            }
        }
    }
    Ok(cells)
}
