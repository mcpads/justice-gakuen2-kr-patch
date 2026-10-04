use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};

use crate::character_select_graphics::validate_consumer_resource_loads;
use crate::embedded_tim::{decode_embedded_tim_preview, detect_embedded_tim_images};
use crate::mode_descendant_graphics::practical_exam::{
    PracticalExamConsumer, audit_practical_exam_direct_texture_census,
};
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, IndexedImage, RgbaImage, parse_8bpp_prefix, read_4bpp_indexed_image_in_prefix,
    read_8bpp_indexed_cell_in_prefix, read_indexed_cell_without_clut_in_prefix,
};

use super::super::model::{
    PracticalResultAssets, PracticalResultStrategy, PracticalResultTextureConsumerEvidence,
};
use super::super::source_ownership::PracticalResultSourceOwnership;
use super::indexed_member_consumer::validate_siken20_indexed_result_consumer_path;
use super::{parse_hex_offset, source_for_path};

pub(super) fn validate_practical_result_sources(
    assets: &PracticalResultAssets,
    ownership: &PracticalResultSourceOwnership,
    sources: &[ModeDescendantSourceRecord],
) -> Result<()> {
    let expected_paths = assets
        .sources
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        sources
            .iter()
            .filter(|source| expected_paths.contains(source.path))
            .map(|source| source.path)
            .collect::<BTreeSet<_>>()
            == expected_paths,
        "practical-result source denominator changed"
    );
    for binding in &assets.sources {
        let source = source_for_path(sources, &binding.source_path)?;
        ensure!(
            source.storage_kind == binding.storage_kind
                && source.stored.len() == binding.source_stored_size
                && sha256_bytes(&source.stored) == binding.source_stored_sha256
                && source.decoded.len() == binding.source_decoded_size
                && sha256_bytes(&source.decoded) == binding.source_decoded_sha256,
            "practical-result source identity changed for {}",
            binding.source_path
        );
    }

    let tim_cache = PracticalResultTimCache::load(assets, sources)?;
    validate_tim_denominator(assets, &tim_cache)?;
    validate_fixed_text_consumer_occurrences(assets, sources, &tim_cache)?;
    validate_judgment_stamp_consumer_occurrences(assets, sources, &tim_cache)?;
    validate_cataloged_source_glyph_pixels(assets, &tim_cache)?;

    let mut pixels_by_physical_cell = BTreeMap::new();
    let mut physical_hash_mismatches = Vec::new();
    for (key, physical) in &ownership.physical_cells {
        let pixels =
            tim_cache.indexed_source_cell(&key.source_path, key.tim_offset, key.bpp, key.cell())?;
        let found_sha256 = sha256_bytes(&pixels);
        if found_sha256 != physical.source_indexed_sha256 {
            physical_hash_mismatches.push(format!("{}={found_sha256}", physical.region_id));
        }
        ensure!(
            pixels_by_physical_cell
                .insert(key.clone(), pixels)
                .is_none(),
            "practical-result physical source cell was decoded twice"
        );
    }
    ensure!(
        physical_hash_mismatches.is_empty(),
        "practical-result indexed source pixel preimages changed: {}",
        physical_hash_mismatches.join(", ")
    );

    let verified_physical_regions = ownership
        .physical_cells
        .values()
        .map(|physical| physical.region_id.as_str())
        .collect::<BTreeSet<_>>();
    for entry in &assets.entries {
        if entry.strategy == PracticalResultStrategy::GlyphSequence {
            let mut sequence = ownership
                .semantic_references
                .iter()
                .filter(|reference| reference.entry_id == entry.id)
                .collect::<Vec<_>>();
            sequence.sort_by_key(|reference| reference.sequence_index);
            ensure!(
                sequence.len() == entry.source_references.len(),
                "practical-result glyph sequence {} lost a semantic reference",
                entry.id
            );
            let mut sequence_pixels = Vec::new();
            for reference in sequence {
                sequence_pixels.extend_from_slice(
                    pixels_by_physical_cell
                        .get(&reference.physical_cell)
                        .context("practical-result sequence lost its physical source cell")?,
                );
            }
            let found_sequence_sha256 = sha256_bytes(&sequence_pixels);
            ensure!(
                entry.source_cell_sequence_indexed_sha256.as_deref()
                    == Some(found_sequence_sha256.as_str()),
                "practical-result indexed glyph sequence {} changed: found {found_sequence_sha256}",
                entry.id
            );
        }
        for source_reference in &entry.unresolved_source_references {
            if let (Some(cell), Some(expected_hash)) = (
                source_reference.cell,
                source_reference.source_visual_rgba_sha256.as_deref(),
            ) {
                let pixels = tim_cache.palette_0_rgba_source_cell(
                    &source_reference.source_path,
                    parse_hex_offset(&source_reference.tim_offset)?,
                    source_reference.bpp,
                    cell,
                )?;
                ensure!(
                    sha256_bytes(&pixels) == expected_hash,
                    "unresolved practical-result source pixels changed for {}",
                    source_reference.reference_id
                );
            }
        }
    }
    let mut fixed_text_target_hash_mismatches = Vec::new();
    for target in &assets.fixed_text_target_catalog.targets {
        let pixels = tim_cache.indexed_source_cell(
            &target.target_record_path,
            parse_hex_offset(&target.target_tim_offset)?,
            target.target_bpp,
            target.target_cell,
        )?;
        let found_sha256 = sha256_bytes(&pixels);
        if found_sha256 != target.expected_preimage_indexed_sha256 {
            fixed_text_target_hash_mismatches.push(format!("{}={found_sha256}", target.target_id));
        }
    }
    ensure!(
        fixed_text_target_hash_mismatches.is_empty(),
        "fixed-text target indexed preimages changed: {}",
        fixed_text_target_hash_mismatches.join(", ")
    );
    for target in &assets.judgment_stamp_target_catalog.targets {
        let pixels = tim_cache.indexed_source_cell(
            &target.target_record_path,
            parse_hex_offset(&target.target_tim_offset)?,
            target.target_bpp,
            target.target_cell,
        )?;
        let found_sha256 = sha256_bytes(&pixels);
        ensure!(
            found_sha256 == target.expected_preimage_indexed_sha256,
            "judgment-stamp target indexed preimage changed for {}: expected {}, found {}",
            target.target_id,
            target.expected_preimage_indexed_sha256,
            found_sha256
        );
    }
    for target in &assets.action_cell_target_catalog.targets {
        let pixels = tim_cache.indexed_source_cell(
            &target.target_record_path,
            parse_hex_offset(&target.target_tim_offset)?,
            target.target_bpp,
            target.target_cell,
        )?;
        let found_sha256 = sha256_bytes(&pixels);
        ensure!(
            found_sha256 == target.expected_preimage_indexed_sha256,
            "action-cell target indexed preimage changed for {}: expected {}, found {}",
            target.target_id,
            target.expected_preimage_indexed_sha256,
            found_sha256
        );
    }
    let mut candidate_hash_mismatches = Vec::new();
    for candidate in &assets.fixed_text_target_catalog.layout_candidates {
        let pixels = tim_cache.indexed_source_cell(
            &candidate.source_path,
            parse_hex_offset(&candidate.source_tim_offset)?,
            candidate.source_bpp,
            candidate.candidate_cell,
        )?;
        let found_sha256 = sha256_bytes(&pixels);
        if found_sha256 != candidate.source_indexed_sha256 {
            candidate_hash_mismatches.push(format!("{}={found_sha256}", candidate.candidate_id));
        }
    }
    ensure!(
        candidate_hash_mismatches.is_empty(),
        "fixed-text layout-candidate indexed preimages changed: {}",
        candidate_hash_mismatches.join(",")
    );
    let mut judgment_candidate_hash_mismatches = Vec::new();
    for candidate in &assets.judgment_stamp_target_catalog.layout_candidates {
        let pixels = tim_cache.indexed_source_cell(
            &candidate.source_path,
            parse_hex_offset(&candidate.source_tim_offset)?,
            candidate.source_bpp,
            candidate.candidate_cell,
        )?;
        let found_sha256 = sha256_bytes(&pixels);
        if found_sha256 != candidate.source_indexed_sha256 {
            judgment_candidate_hash_mismatches
                .push(format!("{}={found_sha256}", candidate.candidate_id));
        }
    }
    ensure!(
        judgment_candidate_hash_mismatches.is_empty(),
        "judgment-stamp layout-candidate indexed preimages changed: {}",
        judgment_candidate_hash_mismatches.join(",")
    );
    for protected in &assets.protected_content.regions {
        let pixels = tim_cache.indexed_source_cell(
            &protected.source_path,
            parse_hex_offset(&protected.tim_offset)?,
            protected.bpp,
            protected.cell,
        )?;
        ensure!(
            sha256_bytes(&pixels) == protected.source_indexed_sha256,
            "protected practical-result indexed source pixels changed for {}",
            protected.id
        );
    }
    ensure!(
        !verified_physical_regions.is_empty(),
        "practical-result source validation covered no physical regions"
    );
    Ok(())
}

fn validate_cataloged_source_glyph_pixels(
    assets: &PracticalResultAssets,
    tim_cache: &PracticalResultTimCache,
) -> Result<()> {
    for bank in &assets.source_glyph_catalog.banks {
        let tim_offset = parse_hex_offset(&bank.tim_offset)?;
        ensure!(
            tim_cache.palette_count(&bank.source_path, tim_offset, bank.bpp)? == bank.palette_count,
            "practical-result source-glyph bank {} changed palette denominator",
            bank.id
        );
        for row in &bank.rows {
            for (index, glyph) in row.glyphs.chars().enumerate() {
                let x = row
                    .start_x
                    .checked_add(
                        index
                            .checked_mul(row.cell_width)
                            .context("practical-result source-glyph row width overflow")?,
                    )
                    .context("practical-result source-glyph row x overflow")?;
                let pixels = tim_cache.indexed_source_cell(
                    &bank.source_path,
                    tim_offset,
                    bank.bpp,
                    Cell {
                        x,
                        y: row.y,
                        width: row.cell_width,
                        height: row.cell_height,
                    },
                )?;
                ensure!(
                    pixels.iter().any(|pixel| *pixel != 0),
                    "practical-result catalog maps {:?} to a blank source cell at {},{}",
                    glyph,
                    x,
                    row.y
                );
            }
        }
    }
    Ok(())
}

type TimKey = (String, usize, u8);

struct CachedPracticalResultTim {
    source_tim_sha256: String,
    palette_count: usize,
    indexed: IndexedImage,
    rgba: RgbaImage,
}

struct PracticalResultTimCache {
    images: BTreeMap<TimKey, CachedPracticalResultTim>,
}

impl PracticalResultTimCache {
    fn load(
        assets: &PracticalResultAssets,
        sources: &[ModeDescendantSourceRecord],
    ) -> Result<Self> {
        let source_paths = tim_source_paths(assets);
        let mut images = BTreeMap::new();
        for source in sources
            .iter()
            .filter(|source| source_paths.contains(source.path))
        {
            for tim in detect_embedded_tim_images(&source.decoded) {
                let key = (source.path.to_string(), tim.offset, tim.bits_per_pixel);
                let full_cell = Cell {
                    x: 0,
                    y: 0,
                    width: tim.pixel_width,
                    height: tim.pixel_height,
                };
                let indexed = match (tim.bits_per_pixel, tim.palette_count) {
                    (4, 0) => IndexedImage {
                        width: tim.pixel_width,
                        height: tim.pixel_height,
                        pixels: read_indexed_cell_without_clut_in_prefix(
                            &source.decoded,
                            tim.offset,
                            full_cell,
                        )?,
                    },
                    (4, _) => read_4bpp_indexed_image_in_prefix(&source.decoded, tim.offset)?,
                    (8, _) => IndexedImage {
                        width: tim.pixel_width,
                        height: tim.pixel_height,
                        pixels: read_8bpp_indexed_cell_in_prefix(
                            &source.decoded,
                            tim.offset,
                            full_cell,
                        )?,
                    },
                    (bits, _) => anyhow::bail!(
                        "unsupported practical-result TIM depth {bits} in {}",
                        source.path
                    ),
                };
                let cached = CachedPracticalResultTim {
                    source_tim_sha256: tim.source_tim_sha256.clone(),
                    palette_count: tim.palette_count,
                    indexed,
                    rgba: decode_embedded_tim_preview(&source.decoded, &tim)?,
                };
                ensure!(
                    images.insert(key, cached).is_none(),
                    "duplicate TIM identity in practical-result source {}",
                    source.path
                );
            }
        }
        ensure!(
            !images.is_empty(),
            "practical-result TIM cache found no source images"
        );
        Ok(Self { images })
    }

    fn image(&self, source_path: &str, tim_offset: usize, bpp: u8) -> Result<&RgbaImage> {
        self.images
            .get(&(source_path.to_string(), tim_offset, bpp))
            .map(|cached| &cached.rgba)
            .with_context(|| {
                format!(
                    "practical-result source {source_path} lost {bpp}bpp TIM at {tim_offset:#x}"
                )
            })
    }

    fn indexed_image(
        &self,
        source_path: &str,
        tim_offset: usize,
        bpp: u8,
    ) -> Result<&IndexedImage> {
        self.images
            .get(&(source_path.to_string(), tim_offset, bpp))
            .map(|cached| &cached.indexed)
            .with_context(|| {
                format!(
                    "practical-result source {source_path} lost {bpp}bpp TIM at {tim_offset:#x}"
                )
            })
    }

    fn palette_count(&self, source_path: &str, tim_offset: usize, bpp: u8) -> Result<usize> {
        self.images
            .get(&(source_path.to_string(), tim_offset, bpp))
            .map(|cached| cached.palette_count)
            .with_context(|| {
                format!(
                    "practical-result source {source_path} lost {bpp}bpp TIM at {tim_offset:#x}"
                )
            })
    }

    fn palette_0_rgba_source_cell(
        &self,
        source_path: &str,
        tim_offset: usize,
        bpp: u8,
        cell: Cell,
    ) -> Result<Vec<u8>> {
        ensure!(
            self.palette_count(source_path, tim_offset, bpp)? > 0,
            "practical-result visual guard requires a source CLUT in {source_path} at {tim_offset:#x}"
        );
        let rgba = self.image(source_path, tim_offset, bpp)?;
        ensure!(
            cell.x + cell.width <= rgba.width && cell.y + cell.height <= rgba.height,
            "practical-result source cell escapes {source_path} TIM at {tim_offset:#x}"
        );
        let mut pixels = Vec::with_capacity(cell.width * cell.height * 4);
        for y in cell.y..cell.y + cell.height {
            let start = (y * rgba.width + cell.x) * 4;
            let end = start + cell.width * 4;
            pixels.extend_from_slice(&rgba.pixels[start..end]);
        }
        Ok(pixels)
    }

    fn indexed_source_cell(
        &self,
        source_path: &str,
        tim_offset: usize,
        bpp: u8,
        cell: Cell,
    ) -> Result<Vec<u8>> {
        let indexed = self.indexed_image(source_path, tim_offset, bpp)?;
        ensure!(
            cell.x + cell.width <= indexed.width && cell.y + cell.height <= indexed.height,
            "practical-result indexed source cell escapes {source_path} TIM at {tim_offset:#x}"
        );
        let mut pixels = Vec::with_capacity(cell.width * cell.height);
        for y in cell.y..cell.y + cell.height {
            let start = y * indexed.width + cell.x;
            let end = start + cell.width;
            pixels.extend_from_slice(&indexed.pixels[start..end]);
        }
        Ok(pixels)
    }
}

fn validate_fixed_text_consumer_occurrences(
    assets: &PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
    tim_cache: &PracticalResultTimCache,
) -> Result<()> {
    for occurrence in &assets.fixed_text_consumer_occurrence_catalog.occurrences {
        let key = (
            occurrence.target_record_path.clone(),
            parse_hex_offset(&occurrence.target_tim_offset)?,
            occurrence.target_bpp,
        );
        let source_tim = tim_cache.images.get(&key).with_context(|| {
            format!(
                "fixed-text consumer occurrence {} lost its source TIM",
                occurrence.occurrence_id
            )
        })?;
        ensure!(
            source_tim.source_tim_sha256 == occurrence.source_tim_sha256,
            "fixed-text consumer occurrence {} source TIM changed: expected {}, found {}",
            occurrence.occurrence_id,
            occurrence.source_tim_sha256,
            source_tim.source_tim_sha256
        );
        match &occurrence.evidence {
            PracticalResultTextureConsumerEvidence::RuntimeObservedTexture { .. } => {}
            PracticalResultTextureConsumerEvidence::StaticBoundTexture { .. } => {
                validate_static_full_texture_fixed_text_consumer(occurrence, sources)?
            }
            PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture { .. } => {
                validate_static_indexed_member_texture_consumer(
                    occurrence.occurrence_id.as_str(),
                    &occurrence.target_record_path,
                    &occurrence.target_tim_offset,
                    occurrence.target_bpp,
                    &occurrence.evidence,
                    sources,
                )?
            }
            PracticalResultTextureConsumerEvidence::StaticBoundCell {
                consumer_source_cell,
                ..
            } => {
                let indexed = tim_cache.indexed_source_cell(
                    &occurrence.target_record_path,
                    parse_hex_offset(&occurrence.target_tim_offset)?,
                    occurrence.target_bpp,
                    *consumer_source_cell,
                )?;
                ensure!(
                    indexed.iter().any(|pixel| *pixel != 0),
                    "fixed-text consumer occurrence {} names an empty source cell",
                    occurrence.occurrence_id
                );
            }
        }
    }
    Ok(())
}

fn validate_static_indexed_member_texture_consumer(
    occurrence_id: &str,
    target_record_path: &str,
    target_tim_offset: &str,
    target_bpp: u8,
    evidence: &PracticalResultTextureConsumerEvidence,
    sources: &[ModeDescendantSourceRecord],
) -> Result<()> {
    let PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture {
        consumer_record_path,
        consumer_source_sha256,
        source_catalog_index,
        selected_member_index,
        selected_when_state_byte_equals_two,
        loader_span_offset,
        loader_span_size,
        loader_span_sha256,
        indexed_load_call_offset,
        tim_upload_call_offsets,
    } = evidence
    else {
        bail!("texture consumer occurrence {occurrence_id} has no indexed-member binding");
    };
    let member_base = selected_member_index
        .checked_mul(0x6d000)
        .context("indexed-member logical base overflow")?;
    let tim_offset = parse_hex_offset(target_tim_offset)?;
    let local_tim_offset = tim_offset
        .checked_sub(member_base)
        .context("indexed-member TIM precedes its selected member")?;
    let (expected_upload_offset, expected_bpp) = match local_tim_offset {
        0x00000 => (0x36f8, 4),
        0x08800 => (0x3714, 8),
        0x45000 | 0x58800 => (0x3764, 4),
        _ => bail!(
            "indexed-member texture occurrence {occurrence_id} names an unsupported member-local TIM"
        ),
    };
    ensure!(
        target_record_path == "DAT2/SIKEN20.BIZ"
            && target_bpp == expected_bpp
            && consumer_record_path == "DAT1/SIKEN2.BIN"
            && *source_catalog_index == 0x02b5
            && *selected_member_index < 2
            && *selected_when_state_byte_equals_two == (*selected_member_index == 0)
            && parse_hex_offset(loader_span_offset)? == 0x3680
            && *loader_span_size == 0x100
            && parse_hex_offset(indexed_load_call_offset)? == 0x36dc
            && tim_upload_call_offsets
                .iter()
                .map(|offset| parse_hex_offset(offset))
                .collect::<Result<Vec<_>>>()?
                .contains(&expected_upload_offset),
        "unsupported indexed-member texture consumer binding {occurrence_id}"
    );
    let consumer = source_for_path(sources, consumer_record_path)?;
    ensure!(
        sha256_bytes(&consumer.decoded) == consumer_source_sha256.as_str(),
        "indexed-member texture consumer {occurrence_id} source changed"
    );
    let loader_start = parse_hex_offset(loader_span_offset)?;
    let loader_end = loader_start + loader_span_size;
    let loader_span = consumer
        .decoded
        .get(loader_start..loader_end)
        .context("indexed-member fixed-text loader span is truncated")?;
    ensure!(
        sha256_bytes(loader_span) == loader_span_sha256.as_str(),
        "indexed-member texture consumer {occurrence_id} loader span changed"
    );
    validate_siken20_indexed_result_consumer_path(&consumer.decoded, occurrence_id)?;
    Ok(())
}

fn validate_static_full_texture_fixed_text_consumer(
    occurrence: &super::super::model::PracticalResultFixedTextConsumerOccurrence,
    sources: &[ModeDescendantSourceRecord],
) -> Result<()> {
    let PracticalResultTextureConsumerEvidence::StaticBoundTexture {
        consumer_record_path,
        consumer_source_sha256,
        source_catalog_index,
        renderer_entry_offset,
        full_texture_table_offset,
        full_texture_table_entry_count,
        full_texture_table_sha256,
    } = &occurrence.evidence
    else {
        bail!(
            "fixed-text consumer occurrence {} has no static binding",
            occurrence.occurrence_id
        );
    };
    ensure!(
        occurrence.target_record_path == "DAT2/SIKEN1.BIZ"
            && occurrence.target_tim_offset == "0x55000"
            && occurrence.target_bpp == 8
            && consumer_record_path == "DAT1/SIKEN.BIN"
            && *source_catalog_index == 0x2b2
            && parse_hex_offset(renderer_entry_offset)? == 0x6de8
            && parse_hex_offset(full_texture_table_offset)? == 0x003c
            && *full_texture_table_entry_count == 4,
        "unsupported static fixed-text consumer binding {}",
        occurrence.occurrence_id
    );
    let consumer = source_for_path(sources, consumer_record_path)?;
    ensure!(
        sha256_bytes(&consumer.decoded) == consumer_source_sha256.as_str(),
        "static fixed-text consumer {} source changed",
        occurrence.occurrence_id
    );
    validate_consumer_resource_loads(consumer_record_path, &consumer.decoded)?;
    audit_practical_exam_direct_texture_census(
        &consumer.decoded,
        PracticalExamConsumer::BasicsReview,
        &occurrence.target_record_path,
    )?;

    let table_offset = parse_hex_offset(full_texture_table_offset)?;
    let table_size = full_texture_table_entry_count
        .checked_mul(18)
        .context("full-texture table size overflow")?;
    let table = consumer
        .decoded
        .get(table_offset..table_offset + table_size)
        .context("static fixed-text full-texture table is truncated")?;
    ensure!(
        sha256_bytes(table) == full_texture_table_sha256.as_str(),
        "static fixed-text full-texture table changed"
    );

    let producer = source_for_path(sources, &occurrence.target_record_path)?;
    let tim_offset = parse_hex_offset(&occurrence.target_tim_offset)?;
    let tim = parse_8bpp_prefix(
        producer
            .decoded
            .get(tim_offset..)
            .context("static fixed-text source TIM is truncated")?,
    )?;
    ensure!(
        tim.pixel_width() == 512
            && tim.image_height == 480
            && tim.image_x == 512
            && tim.image_y == 0
            && tim.clut_x == 0
            && tim.clut_y == 484
            && tim.clut_width * tim.clut_height == 256,
        "static fixed-text source TIM identity changed"
    );
    Ok(())
}

fn validate_judgment_stamp_consumer_occurrences(
    assets: &PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
    tim_cache: &PracticalResultTimCache,
) -> Result<()> {
    for occurrence in &assets
        .judgment_stamp_consumer_occurrence_catalog
        .occurrences
    {
        let key = (
            occurrence.target_record_path.clone(),
            parse_hex_offset(&occurrence.target_tim_offset)?,
            occurrence.target_bpp,
        );
        let source_tim = tim_cache.images.get(&key).with_context(|| {
            format!(
                "judgment-stamp consumer occurrence {} lost its source TIM",
                occurrence.occurrence_id
            )
        })?;
        ensure!(
            source_tim.source_tim_sha256 == occurrence.source_tim_sha256,
            "judgment-stamp consumer occurrence {} source TIM changed",
            occurrence.occurrence_id
        );
        if matches!(
            &occurrence.evidence,
            PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture { .. }
        ) {
            validate_static_indexed_member_texture_consumer(
                occurrence.occurrence_id.as_str(),
                &occurrence.target_record_path,
                &occurrence.target_tim_offset,
                occurrence.target_bpp,
                &occurrence.evidence,
                sources,
            )?;
        }
    }
    Ok(())
}

fn tim_source_paths(assets: &PracticalResultAssets) -> BTreeSet<&str> {
    let mut paths = BTreeSet::new();
    paths.extend(
        assets
            .physical_region_catalog
            .regions
            .iter()
            .map(|region| region.source_path.as_str()),
    );
    paths.extend(
        assets
            .source_glyph_catalog
            .banks
            .iter()
            .map(|bank| bank.source_path.as_str()),
    );
    paths.extend(
        assets
            .fixed_text_target_catalog
            .targets
            .iter()
            .map(|target| target.target_record_path.as_str()),
    );
    paths.extend(
        assets
            .fixed_text_target_catalog
            .layout_candidates
            .iter()
            .map(|candidate| candidate.source_path.as_str()),
    );
    paths.extend(
        assets
            .fixed_text_consumer_occurrence_catalog
            .occurrences
            .iter()
            .map(|occurrence| occurrence.target_record_path.as_str()),
    );
    paths.extend(
        assets
            .judgment_stamp_target_catalog
            .targets
            .iter()
            .map(|target| target.target_record_path.as_str()),
    );
    paths.extend(
        assets
            .judgment_stamp_target_catalog
            .layout_candidates
            .iter()
            .map(|candidate| candidate.source_path.as_str()),
    );
    paths.extend(
        assets
            .judgment_stamp_consumer_occurrence_catalog
            .occurrences
            .iter()
            .map(|occurrence| occurrence.target_record_path.as_str()),
    );
    paths.extend(
        assets
            .coverage_boundary
            .existing_shared_ui_owner
            .source_paths
            .iter()
            .map(String::as_str),
    );
    paths.extend(
        assets
            .coverage_boundary
            .excluded_tim_surfaces
            .iter()
            .map(|tim| tim.source_path.as_str()),
    );
    paths.extend(
        assets
            .protected_content
            .regions
            .iter()
            .map(|region| region.source_path.as_str()),
    );
    paths.extend(
        assets
            .entries
            .iter()
            .flat_map(|entry| &entry.unresolved_source_references)
            .map(|reference| reference.source_path.as_str()),
    );
    paths
}

fn validate_tim_denominator(
    assets: &PracticalResultAssets,
    tim_cache: &PracticalResultTimCache,
) -> Result<()> {
    let source_tims = tim_cache.images.keys().cloned().collect::<BTreeSet<_>>();

    let owner = &assets.coverage_boundary.existing_shared_ui_owner;
    let owner_offset = parse_hex_offset(&owner.tim_offset)?;
    let existing_owner_tims = owner
        .source_paths
        .iter()
        .map(|path| (path.clone(), owner_offset, owner.bpp))
        .collect::<BTreeSet<_>>();
    let excluded_tims = assets
        .coverage_boundary
        .excluded_tim_surfaces
        .iter()
        .map(|tim| {
            Ok((
                tim.source_path.clone(),
                parse_hex_offset(&tim.tim_offset)?,
                tim.bpp,
            ))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let mut result_tims = BTreeSet::new();
    for region in &assets.physical_region_catalog.regions {
        result_tims.insert((
            region.source_path.clone(),
            parse_hex_offset(&region.tim_offset)?,
            region.bpp,
        ));
    }
    for entry in &assets.entries {
        for source_reference in &entry.unresolved_source_references {
            result_tims.insert((
                source_reference.source_path.clone(),
                parse_hex_offset(&source_reference.tim_offset)?,
                source_reference.bpp,
            ));
        }
    }
    ensure!(
        existing_owner_tims.is_disjoint(&excluded_tims)
            && existing_owner_tims.is_disjoint(&result_tims)
            && excluded_tims.is_disjoint(&result_tims),
        "practical-result TIM ownership classes overlap"
    );
    let classified = existing_owner_tims
        .union(&excluded_tims)
        .cloned()
        .collect::<BTreeSet<_>>()
        .union(&result_tims)
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        classified == source_tims,
        "practical-result TIM denominator changed: source {}, classified {}, unclassified {:?}, absent {:?}",
        source_tims.len(),
        classified.len(),
        source_tims.difference(&classified).collect::<Vec<_>>(),
        classified.difference(&source_tims).collect::<Vec<_>>()
    );
    Ok(())
}
