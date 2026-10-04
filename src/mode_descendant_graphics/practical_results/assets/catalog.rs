use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, cells_overlap};

use super::super::model::{
    PracticalResultActionCellTargetCatalog, PracticalResultActionCellTargetCatalogKind,
    PracticalResultAssets, PracticalResultDecorativeBackgroundTargetCatalog,
    PracticalResultDecorativeBackgroundTargetCatalogKind,
    PracticalResultDecorativeCompositionStage, PracticalResultDevelopmentStatus,
    PracticalResultEntry, PracticalResultFixedTextCandidateAuthorityStatus,
    PracticalResultFixedTextCandidateEvidenceStatus,
    PracticalResultFixedTextConsumerOccurrenceCatalog,
    PracticalResultFixedTextConsumerOccurrenceCatalogKind,
    PracticalResultFixedTextStaticSourceRegion, PracticalResultFixedTextTargetCatalog,
    PracticalResultFixedTextTargetCatalogKind,
    PracticalResultJudgmentStampConsumerOccurrenceCatalog,
    PracticalResultJudgmentStampConsumerOccurrenceCatalogKind,
    PracticalResultJudgmentStampTargetCatalog, PracticalResultJudgmentStampTargetCatalogKind,
    PracticalResultManifest, PracticalResultPhysicalRegionCatalog, PracticalResultReleaseStatus,
    PracticalResultShard, PracticalResultSourceGlyphCatalog, PracticalResultStrategy,
    PracticalResultTextureConsumerEvidence,
};
use super::super::opaque_pointer_run_model::PracticalResultOpaquePointerRunCatalog;
use super::super::projection_model::{
    PracticalResultActionConsumerOccurrenceCatalog, PracticalResultClutBindingCatalog,
    PracticalResultConsumerMechanism, PracticalResultConsumerProjectionCatalog,
    PracticalResultDirectNumericSpriteSite, PracticalResultProjectionEvidenceStatus,
    PracticalResultVramResidencyCatalog,
};
use super::super::source_atlas_domain_model::PracticalResultSourceAtlasDomainCatalog;
use crate::mode_descendant_graphics::model::{ModeDescendantFontRole, TextAlignment};

use super::parse_hex_offset;
use super::source_atlas_domains::validate_source_atlas_domain_catalog;

const MANIFEST_KIND: &str = "justice_gakuen2_practical_result_manifest";
const SHARD_KIND: &str = "justice_gakuen2_practical_result_shard";
const PHYSICAL_REGION_CATALOG_KIND: &str =
    "justice_gakuen2_practical_result_physical_region_catalog";
const SOURCE_GLYPH_CATALOG_KIND: &str = "justice_gakuen2_practical_result_source_glyph_catalog";

pub(super) fn load_practical_result_assets(directory: &Path) -> Result<PracticalResultAssets> {
    let manifest_path = directory.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: PracticalResultManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest(&manifest)?;

    ensure_relative_path(&manifest.physical_region_catalog_file)?;
    let physical_region_catalog_path = directory.join(&manifest.physical_region_catalog_file);
    let physical_region_catalog_bytes = std::fs::read(&physical_region_catalog_path)
        .with_context(|| format!("failed to read {}", physical_region_catalog_path.display()))?;
    let physical_region_catalog: PracticalResultPhysicalRegionCatalog =
        serde_json::from_slice(&physical_region_catalog_bytes).with_context(|| {
            format!("failed to parse {}", physical_region_catalog_path.display())
        })?;
    validate_physical_region_catalog(&physical_region_catalog, &manifest)?;

    let (source_atlas_domain_catalog, source_atlas_domain_catalog_bytes): (
        PracticalResultSourceAtlasDomainCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.source_atlas_domain_catalog_file)?;
    let (opaque_pointer_run_catalog, opaque_pointer_run_catalog_bytes): (
        PracticalResultOpaquePointerRunCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.opaque_pointer_run_catalog_file)?;

    ensure_relative_path(&manifest.source_glyph_catalog_file)?;
    let source_glyph_catalog_path = directory.join(&manifest.source_glyph_catalog_file);
    let source_glyph_catalog_bytes = std::fs::read(&source_glyph_catalog_path)
        .with_context(|| format!("failed to read {}", source_glyph_catalog_path.display()))?;
    let source_glyph_catalog: PracticalResultSourceGlyphCatalog =
        serde_json::from_slice(&source_glyph_catalog_bytes)
            .with_context(|| format!("failed to parse {}", source_glyph_catalog_path.display()))?;
    validate_source_glyph_catalog(&source_glyph_catalog, &manifest)?;
    validate_source_atlas_domain_catalog(
        &source_atlas_domain_catalog,
        &physical_region_catalog,
        &source_glyph_catalog,
        &manifest,
    )?;

    let (mut fixed_text_target_catalog, fixed_text_target_catalog_bytes): (
        PracticalResultFixedTextTargetCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.fixed_text_target_catalog_file)?;
    for target in &mut fixed_text_target_catalog.targets {
        if let Some(artwork) = &mut target.indexed_artwork {
            ensure!(
                target.semantic_entry_id.as_str() == "attendance_book_label",
                "indexed artwork is only supported for the attendance-book label"
            );
            ensure_sha256(&artwork.indices_sha256, "indexed artwork")?;
            ensure_sha256(&artwork.palette_sha256, "indexed artwork palette")?;
            ensure_sha256(&artwork.imagegen_sha256, "indexed artwork source")?;
            ensure_sha256(&artwork.dotmend_bundle_id, "indexed artwork bundle")?;
            ensure!(
                artwork.dotmend_art_id.starts_with("art_"),
                "missing Dotmend artwork identity"
            );
            artwork.indices_file = directory.join(&artwork.indices_file);
        }
    }
    let (fixed_text_consumer_occurrence_catalog, fixed_text_consumer_occurrence_catalog_bytes): (
        PracticalResultFixedTextConsumerOccurrenceCatalog,
        Vec<u8>,
    ) = load_catalog_file(
        directory,
        &manifest.fixed_text_consumer_occurrence_catalog_file,
    )?;
    let (judgment_stamp_target_catalog, judgment_stamp_target_catalog_bytes): (
        PracticalResultJudgmentStampTargetCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.judgment_stamp_target_catalog_file)?;
    let (
        judgment_stamp_consumer_occurrence_catalog,
        judgment_stamp_consumer_occurrence_catalog_bytes,
    ): (
        PracticalResultJudgmentStampConsumerOccurrenceCatalog,
        Vec<u8>,
    ) = load_catalog_file(
        directory,
        &manifest.judgment_stamp_consumer_occurrence_catalog_file,
    )?;
    let (action_cell_target_catalog, action_cell_target_catalog_bytes): (
        PracticalResultActionCellTargetCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.action_cell_target_catalog_file)?;
    let (decorative_background_target_catalog, decorative_background_target_catalog_bytes): (
        PracticalResultDecorativeBackgroundTargetCatalog,
        Vec<u8>,
    ) = load_catalog_file(
        directory,
        &manifest.decorative_background_target_catalog_file,
    )?;

    let (vram_residency_catalog, vram_residency_catalog_bytes): (
        PracticalResultVramResidencyCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.vram_residency_catalog_file)?;
    let (clut_binding_catalog, clut_binding_catalog_bytes): (
        PracticalResultClutBindingCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.clut_binding_catalog_file)?;
    let (action_consumer_occurrence_catalog, action_consumer_occurrence_catalog_bytes): (
        PracticalResultActionConsumerOccurrenceCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.action_consumer_occurrence_catalog_file)?;
    let (consumer_projection_catalog, consumer_projection_catalog_bytes): (
        PracticalResultConsumerProjectionCatalog,
        Vec<u8>,
    ) = load_catalog_file(directory, &manifest.consumer_projection_catalog_file)?;

    let mut files = BTreeSet::new();
    let mut responsibilities = BTreeSet::new();
    let mut entries = Vec::new();
    for shard_ref in &manifest.shards {
        ensure_relative_path(&shard_ref.file)?;
        ensure!(
            files.insert(shard_ref.file.clone())
                && responsibilities.insert(shard_ref.responsibility.as_str()),
            "duplicate practical-result shard {}",
            shard_ref.file.display()
        );
        let path = directory.join(&shard_ref.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let shard: PracticalResultShard = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            shard.kind == SHARD_KIND && shard.responsibility == shard_ref.responsibility,
            "practical-result shard identity changed in {}",
            path.display()
        );
        ensure!(
            !shard.entries.is_empty(),
            "practical-result shard {} is empty",
            path.display()
        );
        entries.extend(shard.entries);
    }
    validate_entries(&entries, &physical_region_catalog, &manifest)?;
    validate_fixed_text_target_catalog(
        &fixed_text_target_catalog,
        &fixed_text_consumer_occurrence_catalog,
        &consumer_projection_catalog,
        &source_atlas_domain_catalog,
        &entries,
        &physical_region_catalog,
        &manifest,
    )?;
    validate_judgment_stamp_target_catalog(
        &judgment_stamp_target_catalog,
        &judgment_stamp_consumer_occurrence_catalog,
        &entries,
        &physical_region_catalog,
        &manifest,
    )?;
    validate_action_cell_target_catalog(
        &action_cell_target_catalog,
        &action_consumer_occurrence_catalog,
        &entries,
        &manifest,
    )?;
    validate_decorative_background_target_catalog(
        &decorative_background_target_catalog,
        &fixed_text_consumer_occurrence_catalog,
        &entries,
        &physical_region_catalog,
        &manifest,
    )?;
    Ok(PracticalResultAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        physical_region_catalog_sha256: sha256_bytes(&physical_region_catalog_bytes),
        source_atlas_domain_catalog_sha256: sha256_bytes(&source_atlas_domain_catalog_bytes),
        opaque_pointer_run_catalog_sha256: sha256_bytes(&opaque_pointer_run_catalog_bytes),
        source_glyph_catalog_sha256: sha256_bytes(&source_glyph_catalog_bytes),
        fixed_text_target_catalog_sha256: sha256_bytes(&fixed_text_target_catalog_bytes),
        fixed_text_consumer_occurrence_catalog_sha256: sha256_bytes(
            &fixed_text_consumer_occurrence_catalog_bytes,
        ),
        judgment_stamp_target_catalog_sha256: sha256_bytes(&judgment_stamp_target_catalog_bytes),
        judgment_stamp_consumer_occurrence_catalog_sha256: sha256_bytes(
            &judgment_stamp_consumer_occurrence_catalog_bytes,
        ),
        action_cell_target_catalog_sha256: sha256_bytes(&action_cell_target_catalog_bytes),
        decorative_background_target_catalog_sha256: sha256_bytes(
            &decorative_background_target_catalog_bytes,
        ),
        vram_residency_catalog_sha256: sha256_bytes(&vram_residency_catalog_bytes),
        clut_binding_catalog_sha256: sha256_bytes(&clut_binding_catalog_bytes),
        action_consumer_occurrence_catalog_sha256: sha256_bytes(
            &action_consumer_occurrence_catalog_bytes,
        ),
        consumer_projection_catalog_sha256: sha256_bytes(&consumer_projection_catalog_bytes),
        sources: manifest.source_catalog,
        physical_region_catalog,
        source_atlas_domain_catalog,
        opaque_pointer_run_catalog,
        source_glyph_catalog,
        fixed_text_target_catalog,
        fixed_text_consumer_occurrence_catalog,
        judgment_stamp_target_catalog,
        judgment_stamp_consumer_occurrence_catalog,
        action_cell_target_catalog,
        decorative_background_target_catalog,
        vram_residency_catalog,
        clut_binding_catalog,
        action_consumer_occurrence_catalog,
        consumer_projection_catalog,
        coverage_boundary: manifest.coverage_boundary,
        protected_content: manifest.protected_content,
        entries,
    })
}

fn load_catalog_file<T: DeserializeOwned>(
    directory: &Path,
    relative_path: &Path,
) -> Result<(T, Vec<u8>)> {
    ensure_relative_path(relative_path)?;
    let path = directory.join(relative_path);
    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let catalog = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok((catalog, bytes))
}

fn validate_manifest(manifest: &PracticalResultManifest) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported practical-result manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.source_catalog.is_empty() && !manifest.shards.is_empty(),
        "practical-result manifest has no sources or shards"
    );
    let catalog_paths = [
        &manifest.physical_region_catalog_file,
        &manifest.source_atlas_domain_catalog_file,
        &manifest.opaque_pointer_run_catalog_file,
        &manifest.source_glyph_catalog_file,
        &manifest.fixed_text_target_catalog_file,
        &manifest.fixed_text_consumer_occurrence_catalog_file,
        &manifest.judgment_stamp_target_catalog_file,
        &manifest.judgment_stamp_consumer_occurrence_catalog_file,
        &manifest.action_cell_target_catalog_file,
        &manifest.decorative_background_target_catalog_file,
        &manifest.vram_residency_catalog_file,
        &manifest.clut_binding_catalog_file,
        &manifest.action_consumer_occurrence_catalog_file,
        &manifest.consumer_projection_catalog_file,
    ];
    ensure!(
        catalog_paths
            .iter()
            .map(|path| path.as_path())
            .collect::<BTreeSet<_>>()
            .len()
            == catalog_paths.len(),
        "practical-result manifest aliases distinct catalog responsibilities"
    );
    ensure!(
        manifest.physical_region_hash_contract.algorithm == "sha256"
            && manifest.physical_region_hash_contract.pixel_format == "palette_index_u8"
            && manifest.physical_region_hash_contract.pixel_order == "row_major"
            && manifest.physical_region_hash_contract.row_order == "top_to_bottom"
            && manifest.physical_region_hash_contract.coordinate_space
                == "decoded_tim_source_pixels",
        "practical-result physical-region hash contract changed"
    );
    ensure!(
        manifest.visual_region_hash_contract.algorithm == "sha256"
            && manifest.visual_region_hash_contract.pixel_format == "rgba8_palette_0"
            && manifest.visual_region_hash_contract.pixel_order == "row_major"
            && manifest.visual_region_hash_contract.row_order == "top_to_bottom"
            && manifest.visual_region_hash_contract.coordinate_space == "decoded_tim_source_pixels",
        "practical-result visual-region hash contract changed"
    );
    ensure!(
        manifest.glyph_sequence_hash_contract.algorithm == "sha256"
            && manifest.glyph_sequence_hash_contract.composition
                == "concatenated indexed-pixel byte streams in sequence_index order",
        "practical-result glyph-sequence hash contract changed"
    );
    let mut paths = BTreeSet::new();
    for source in &manifest.source_catalog {
        ensure!(
            paths.insert(source.source_path.as_str()),
            "duplicate practical-result source {}",
            source.source_path
        );
        ensure_sha256(&source.source_stored_sha256, "stored source")?;
        ensure_sha256(&source.source_decoded_sha256, "decoded source")?;
        ensure!(
            source.source_stored_size > 0 && source.source_decoded_size > 0,
            "practical-result source {} has an empty extent",
            source.source_path
        );
    }
    validate_coverage_boundary(manifest, &paths)?;
    validate_protected_content(manifest, &paths)?;
    Ok(())
}

fn validate_physical_region_catalog(
    catalog: &PracticalResultPhysicalRegionCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind == PHYSICAL_REGION_CATALOG_KIND && !catalog.regions.is_empty(),
        "unsupported or empty practical-result physical-region catalog"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut region_ids = BTreeSet::new();
    let mut physical_keys = BTreeSet::new();
    for region in &catalog.regions {
        let tim_offset = parse_hex_offset(&region.tim_offset)?;
        ensure!(
            !region.region_id.as_str().trim().is_empty()
                && region_ids.insert(region.region_id.as_str())
                && source_paths.contains(region.source_path.as_str())
                && matches!(region.bpp, 4 | 8)
                && region.cell.width > 0
                && region.cell.height > 0
                && physical_keys.insert((
                    region.source_path.as_str(),
                    tim_offset,
                    region.bpp,
                    region.cell.x,
                    region.cell.y,
                    region.cell.width,
                    region.cell.height,
                )),
            "invalid or duplicate practical-result physical region {}",
            region.region_id
        );
        ensure_sha256(
            &region.source_indexed_sha256,
            "physical indexed source region",
        )?;
    }
    Ok(())
}

fn validate_source_glyph_catalog(
    catalog: &PracticalResultSourceGlyphCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind == SOURCE_GLYPH_CATALOG_KIND && !catalog.banks.is_empty(),
        "unsupported or empty practical-result source-glyph catalog"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut bank_ids = BTreeSet::new();
    let mut row_ids = BTreeSet::new();
    let mut cells = Vec::<(&str, usize, u8, Cell)>::new();
    for bank in &catalog.banks {
        let tim_offset = parse_hex_offset(&bank.tim_offset)?;
        ensure!(
            bank_ids.insert(bank.id.as_str())
                && source_paths.contains(bank.source_path.as_str())
                && matches!(bank.bpp, 4 | 8)
                && bank.palette_count > 0
                && !bank.rows.is_empty(),
            "invalid practical-result source-glyph bank {}",
            bank.id
        );
        for row in &bank.rows {
            let glyphs = row.glyphs.chars().collect::<Vec<_>>();
            ensure!(
                row_ids.insert((bank.id.as_str(), row.id.as_str()))
                    && !row.role.trim().is_empty()
                    && row.cell_width > 0
                    && row.cell_height > 0
                    && !glyphs.is_empty()
                    && glyphs.iter().all(|glyph| !glyph.is_control()),
                "invalid practical-result source-glyph row {}",
                row.id
            );
            for (index, _) in glyphs.iter().enumerate() {
                let x = row
                    .start_x
                    .checked_add(
                        index
                            .checked_mul(row.cell_width)
                            .context("practical-result source-glyph row width overflow")?,
                    )
                    .context("practical-result source-glyph row x overflow")?;
                let cell = Cell {
                    x,
                    y: row.y,
                    width: row.cell_width,
                    height: row.cell_height,
                };
                ensure!(
                    cells.iter().all(|(source_path, offset, bpp, previous)| {
                        *source_path != bank.source_path
                            || *offset != tim_offset
                            || *bpp != bank.bpp
                            || !cells_overlap(*previous, cell)
                    }),
                    "practical-result source-glyph rows overlap at {},{}",
                    x,
                    row.y
                );
                cells.push((bank.source_path.as_str(), tim_offset, bank.bpp, cell));
            }
        }
    }
    Ok(())
}

fn validate_coverage_boundary(
    manifest: &PracticalResultManifest,
    source_paths: &BTreeSet<&str>,
) -> Result<()> {
    let owner = &manifest.coverage_boundary.existing_shared_ui_owner;
    ensure!(
        owner.disposition == "excluded_existing_owner"
            && owner.owned_entry_count > 0
            && matches!(owner.bpp, 4 | 8)
            && !owner.source_paths.is_empty(),
        "practical-result existing-owner boundary is incomplete"
    );
    parse_hex_offset(&owner.tim_offset)?;
    let mut owner_paths = BTreeSet::new();
    for path in &owner.source_paths {
        ensure!(
            source_paths.contains(path.as_str()) && owner_paths.insert(path.as_str()),
            "invalid practical-result existing-owner path {path}"
        );
    }
    ensure!(
        !manifest.coverage_boundary.excluded_tim_surfaces.is_empty(),
        "practical-result manifest has no explicit excluded TIMs"
    );
    let mut excluded = BTreeSet::new();
    for tim in &manifest.coverage_boundary.excluded_tim_surfaces {
        ensure!(
            source_paths.contains(tim.source_path.as_str())
                && matches!(tim.bpp, 4 | 8)
                && tim.disposition.starts_with("excluded_")
                && !tim.reason.trim().is_empty(),
            "invalid practical-result excluded TIM {}",
            tim.source_path
        );
        ensure!(
            excluded.insert((
                tim.source_path.as_str(),
                parse_hex_offset(&tim.tim_offset)?,
                tim.bpp
            )),
            "duplicate practical-result excluded TIM {}",
            tim.source_path
        );
    }
    Ok(())
}

fn validate_protected_content(
    manifest: &PracticalResultManifest,
    source_paths: &BTreeSet<&str>,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    for region in &manifest.protected_content.regions {
        ensure!(
            ids.insert(region.id.as_str())
                && source_paths.contains(region.source_path.as_str())
                && matches!(region.bpp, 4 | 8)
                && region.cell.width > 0
                && region.cell.height > 0,
            "invalid practical-result protected region {}",
            region.id
        );
        parse_hex_offset(&region.tim_offset)?;
        ensure_sha256(
            &region.source_indexed_sha256,
            "protected indexed source region",
        )?;
    }
    Ok(())
}

fn validate_entries(
    entries: &[PracticalResultEntry],
    physical_region_catalog: &PracticalResultPhysicalRegionCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        !entries.is_empty(),
        "practical-result assets have no entries"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let physical_regions = physical_region_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut reference_ids = BTreeSet::new();
    for entry in entries {
        ensure!(
            ids.insert(entry.id.as_str()) && !entry.source_text.trim().is_empty(),
            "duplicate or empty practical-result entry {}",
            entry.id
        );
        validate_translation_state(entry)?;
        validate_strategy_role(entry)?;
        if let Some(hash) = &entry.source_cell_sequence_indexed_sha256 {
            ensure_sha256(hash, "indexed source sequence")?;
        }
        for reference in &entry.source_references {
            ensure!(
                !reference.reference_id.as_str().trim().is_empty()
                    && reference_ids.insert(reference.reference_id.as_str())
                    && physical_regions.contains_key(reference.physical_region_id.as_str()),
                "invalid practical-result source reference {}",
                reference.reference_id
            );
            match entry.strategy {
                PracticalResultStrategy::GlyphSequence => ensure!(
                    reference.sequence_index.is_some(),
                    "glyph-sequence source reference {} has no sequence index",
                    reference.reference_id
                ),
                _ => ensure!(
                    reference.sequence_index.is_none(),
                    "non-sequence source reference {} has a sequence index",
                    reference.reference_id
                ),
            }
        }
        for reference in &entry.unresolved_source_references {
            ensure!(
                !reference.reference_id.as_str().trim().is_empty()
                    && reference_ids.insert(reference.reference_id.as_str())
                    && source_paths.contains(reference.source_path.as_str())
                    && !reference.reason.trim().is_empty(),
                "invalid unresolved practical-result source reference {}",
                reference.reference_id
            );
            parse_hex_offset(&reference.tim_offset)?;
            ensure!(
                matches!(reference.bpp, 4 | 8),
                "unsupported unresolved practical-result depth {}",
                reference.bpp
            );
            if let Some(cell) = reference.cell {
                ensure!(
                    cell.width > 0 && cell.height > 0,
                    "unresolved practical-result source reference {} has an empty cell",
                    reference.reference_id
                );
            }
            if let Some(hash) = &reference.source_visual_rgba_sha256 {
                ensure_sha256(hash, "unresolved source visual region")?;
            }
        }
    }
    Ok(())
}

fn validate_fixed_text_target_catalog(
    catalog: &PracticalResultFixedTextTargetCatalog,
    consumer_catalog: &PracticalResultFixedTextConsumerOccurrenceCatalog,
    projection_catalog: &PracticalResultConsumerProjectionCatalog,
    source_atlas_domain_catalog: &PracticalResultSourceAtlasDomainCatalog,
    entries: &[PracticalResultEntry],
    physical_region_catalog: &PracticalResultPhysicalRegionCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind
            == PracticalResultFixedTextTargetCatalogKind::JusticeGakuen2PracticalResultFixedTextTargetCatalog,
        "unsupported practical-result fixed-text target catalog"
    );
    ensure!(
        consumer_catalog.kind
            == PracticalResultFixedTextConsumerOccurrenceCatalogKind::JusticeGakuen2PracticalResultFixedTextConsumerOccurrenceCatalog
            && !consumer_catalog.occurrences.is_empty(),
        "unsupported or empty practical-result fixed-text consumer-occurrence catalog"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let physical_regions = physical_region_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();

    let mut consumer_occurrence_ids = BTreeSet::new();
    for occurrence in &consumer_catalog.occurrences {
        ensure!(
            !occurrence.occurrence_id.is_empty()
                && consumer_occurrence_ids.insert(occurrence.occurrence_id.as_str())
                && source_paths.contains(occurrence.target_record_path.as_str())
                && matches!(occurrence.target_bpp, 4 | 8),
            "invalid practical-result fixed-text consumer occurrence {}",
            occurrence.occurrence_id
        );
        parse_hex_offset(&occurrence.target_tim_offset)?;
        ensure_sha256(
            &occurrence.source_tim_sha256,
            "fixed-text consumer source TIM",
        )?;
        match &occurrence.evidence {
            PracticalResultTextureConsumerEvidence::RuntimeObservedTexture {
                runtime_artifact_bin_sha256,
                runtime_frame_path,
                runtime_frame_sha256,
            } => {
                ensure!(
                    !runtime_frame_path.trim().is_empty(),
                    "fixed-text consumer runtime frame path is empty"
                );
                ensure_sha256(
                    runtime_artifact_bin_sha256,
                    "fixed-text consumer runtime artifact",
                )?;
                ensure_sha256(runtime_frame_sha256, "fixed-text consumer runtime frame")?;
            }
            PracticalResultTextureConsumerEvidence::StaticBoundTexture {
                consumer_record_path,
                consumer_source_sha256,
                source_catalog_index,
                renderer_entry_offset,
                full_texture_table_offset,
                full_texture_table_entry_count,
                full_texture_table_sha256,
            } => {
                ensure!(
                    source_paths.contains(consumer_record_path.as_str())
                        && *source_catalog_index > 0
                        && *full_texture_table_entry_count > 0,
                    "invalid static fixed-text consumer evidence for {}",
                    occurrence.occurrence_id
                );
                ensure_sha256(consumer_source_sha256, "fixed-text consumer source")?;
                parse_hex_offset(renderer_entry_offset)?;
                parse_hex_offset(full_texture_table_offset)?;
                ensure_sha256(full_texture_table_sha256, "fixed-text full-texture table")?;
            }
            PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture {
                consumer_record_path,
                consumer_source_sha256,
                source_catalog_index,
                selected_member_index,
                loader_span_offset,
                loader_span_size,
                loader_span_sha256,
                indexed_load_call_offset,
                tim_upload_call_offsets,
                ..
            } => {
                let loader_start = parse_hex_offset(loader_span_offset)?;
                let loader_end = loader_start
                    .checked_add(*loader_span_size)
                    .context("indexed-member loader span overflow")?;
                ensure!(
                    source_paths.contains(consumer_record_path.as_str())
                        && *source_catalog_index > 0
                        && *selected_member_index < 2
                        && *loader_span_size > 0
                        && !tim_upload_call_offsets.is_empty()
                        && std::iter::once(indexed_load_call_offset)
                            .chain(tim_upload_call_offsets)
                            .map(|offset| parse_hex_offset(offset))
                            .collect::<Result<Vec<_>>>()?
                            .into_iter()
                            .all(|offset| {
                                offset >= loader_start
                                    && offset.is_multiple_of(4)
                                    && offset + 4 <= loader_end
                            }),
                    "invalid indexed-member fixed-text consumer evidence for {}",
                    occurrence.occurrence_id
                );
                ensure_sha256(consumer_source_sha256, "fixed-text consumer source")?;
                ensure_sha256(loader_span_sha256, "fixed-text indexed-member loader span")?;
            }
            PracticalResultTextureConsumerEvidence::StaticBoundCell {
                source_atlas_domain_id,
                source_region,
                consumer_source_cell,
                consumer_projection_ids,
            } => validate_static_fixed_text_cell_consumer_declaration(
                occurrence,
                source_atlas_domain_id,
                source_region,
                *consumer_source_cell,
                consumer_projection_ids,
                StaticFixedTextCellConsumerCatalogs {
                    projections: projection_catalog,
                    source_atlas_domains: source_atlas_domain_catalog,
                    physical_regions: physical_region_catalog,
                    manifest,
                },
            )?,
        }
    }

    let mut target_ids = BTreeSet::new();
    for target in &catalog.targets {
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| {
                format!(
                    "fixed-text target {} names unknown semantic entry {}",
                    target.target_id, target.semantic_entry_id
                )
            })?;
        let region = physical_regions
            .get(target.replaces_source_region_id.as_str())
            .with_context(|| {
                format!(
                    "fixed-text target {} names unknown replaced source region {}",
                    target.target_id, target.replaces_source_region_id
                )
            })?;
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        let mut occurrence_ids = BTreeSet::new();
        ensure!(
            !target.target_id.is_empty()
                && target_ids.insert(target.target_id.as_str())
                && entry.strategy == PracticalResultStrategy::FixedText
                && source_paths.contains(target.target_record_path.as_str())
                && matches!(target.target_bpp, 4 | 8)
                && target.target_cell.width > 0
                && target.target_cell.height > 0
                && matches!(
                    target.alignment,
                    TextAlignment::Left | TextAlignment::Center
                )
                && region.source_path == target.target_record_path
                && parse_hex_offset(&region.tim_offset)? == tim_offset
                && region.bpp == target.target_bpp
                && cell_contains(region.cell, target.target_cell)
                && !target.expected_consumer_occurrence_ids.is_empty()
                && target.expected_consumer_occurrence_ids.iter().all(|id| {
                    !id.is_empty()
                        && occurrence_ids.insert(id.as_str())
                        && consumer_occurrence_ids.contains(id.as_str())
                        && consumer_catalog.occurrences.iter().any(|occurrence| {
                            occurrence.occurrence_id == *id
                                && occurrence.target_record_path == target.target_record_path
                                && parse_hex_offset(&occurrence.target_tim_offset).ok()
                                    == Some(tim_offset)
                                && occurrence.target_bpp == target.target_bpp
                                && fixed_text_consumer_covers_cell(
                                    &occurrence.evidence,
                                    target.target_cell,
                                )
                        })
                }),
            "invalid practical-result fixed-text target {}",
            target.target_id
        );
        ensure_sha256(
            &target.expected_preimage_indexed_sha256,
            "fixed-text target indexed preimage",
        )?;
    }

    let mut candidate_ids = BTreeSet::new();
    let mut candidate_reference_ids = BTreeSet::new();
    for candidate in &catalog.layout_candidates {
        let entry = entries_by_id
            .get(candidate.semantic_entry_id.as_str())
            .with_context(|| {
                format!(
                    "fixed-text layout candidate {} names unknown semantic entry {}",
                    candidate.candidate_id, candidate.semantic_entry_id
                )
            })?;
        let reference = entry
            .source_references
            .iter()
            .find(|reference| reference.reference_id == candidate.source_reference_id)
            .with_context(|| {
                format!(
                    "fixed-text layout candidate {} names a source reference outside semantic entry {}",
                    candidate.candidate_id, candidate.semantic_entry_id
                )
            })?;
        let region = physical_regions
            .get(candidate.replaces_source_region_id.as_str())
            .with_context(|| {
                format!(
                    "fixed-text layout candidate {} names unknown replaced source region {}",
                    candidate.candidate_id, candidate.replaces_source_region_id
                )
            })?;
        let tim_offset = parse_hex_offset(&candidate.source_tim_offset)?;
        ensure!(
            !candidate.candidate_id.is_empty()
                && candidate_ids.insert(candidate.candidate_id.as_str())
                && candidate_reference_ids.insert((
                    candidate.semantic_entry_id.as_str(),
                    candidate.source_reference_id.as_str(),
                ))
                && matches!(
                    entry.strategy,
                    PracticalResultStrategy::FixedText | PracticalResultStrategy::JudgmentStamp
                )
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
                && source_paths.contains(candidate.source_path.as_str())
                && matches!(candidate.source_bpp, 4 | 8)
                && candidate.candidate_cell.width > 0
                && candidate.candidate_cell.height > 0
                && matches!(
                    candidate.alignment,
                    TextAlignment::Left | TextAlignment::Center
                )
                && reference.physical_region_id == candidate.replaces_source_region_id
                && region.source_path == candidate.source_path
                && parse_hex_offset(&region.tim_offset)? == tim_offset
                && region.bpp == candidate.source_bpp
                && cell_contains(region.cell, candidate.candidate_cell)
                && candidate.evidence_status
                    == PracticalResultFixedTextCandidateEvidenceStatus::SourceObservationOnly
                && candidate.authority_status
                    == PracticalResultFixedTextCandidateAuthorityStatus::NonAuthoritativeLayoutCandidate
                && !candidate.unresolved_reason.trim().is_empty(),
            "invalid practical-result fixed-text layout candidate {}",
            candidate.candidate_id
        );
        ensure_sha256(
            &candidate.source_indexed_sha256,
            "fixed-text layout-candidate indexed preimage",
        )?;
    }
    Ok(())
}

fn fixed_text_consumer_covers_cell(
    evidence: &PracticalResultTextureConsumerEvidence,
    target_cell: Cell,
) -> bool {
    match evidence {
        PracticalResultTextureConsumerEvidence::RuntimeObservedTexture { .. }
        | PracticalResultTextureConsumerEvidence::StaticBoundTexture { .. }
        | PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture { .. } => true,
        PracticalResultTextureConsumerEvidence::StaticBoundCell {
            consumer_source_cell,
            ..
        } => cell_contains(*consumer_source_cell, target_cell),
    }
}

struct StaticFixedTextCellConsumerCatalogs<'a> {
    projections: &'a PracticalResultConsumerProjectionCatalog,
    source_atlas_domains: &'a PracticalResultSourceAtlasDomainCatalog,
    physical_regions: &'a PracticalResultPhysicalRegionCatalog,
    manifest: &'a PracticalResultManifest,
}

fn validate_static_fixed_text_cell_consumer_declaration(
    occurrence: &super::super::model::PracticalResultFixedTextConsumerOccurrence,
    source_atlas_domain_id: &super::super::source_atlas_domain_model::SourceAtlasDomainId,
    source_region: &PracticalResultFixedTextStaticSourceRegion,
    consumer_source_cell: Cell,
    consumer_projection_ids: &[super::super::projection_model::PracticalResultConsumerProjectionId],
    catalogs: StaticFixedTextCellConsumerCatalogs<'_>,
) -> Result<()> {
    let domain = catalogs
        .source_atlas_domains
        .domains
        .iter()
        .find(|domain| domain.id == *source_atlas_domain_id)
        .with_context(|| {
            format!(
                "fixed-text consumer occurrence {} names unknown source-atlas domain {}",
                occurrence.occurrence_id, source_atlas_domain_id
            )
        })?;
    let (region_path, region_tim_offset, region_bpp, region_cell, region_domain_matches) =
        match source_region {
            PracticalResultFixedTextStaticSourceRegion::Physical { region_id } => {
                let region = catalogs
                    .physical_regions
                    .regions
                    .iter()
                    .find(|region| region.region_id == *region_id)
                    .with_context(|| {
                        format!(
                            "fixed-text consumer occurrence {} names unknown physical region {}",
                            occurrence.occurrence_id, region_id
                        )
                    })?;
                (
                    region.source_path.as_str(),
                    region.tim_offset.as_str(),
                    region.bpp,
                    region.cell,
                    region.source_atlas_domain_id.as_ref() == Some(source_atlas_domain_id),
                )
            }
            PracticalResultFixedTextStaticSourceRegion::Protected { region_id } => {
                let region = catalogs
                    .manifest
                    .protected_content
                    .regions
                    .iter()
                    .find(|region| region.id == *region_id)
                    .with_context(|| {
                        format!(
                            "fixed-text consumer occurrence {} names unknown protected region {}",
                            occurrence.occurrence_id, region_id
                        )
                    })?;
                (
                    region.source_path.as_str(),
                    region.tim_offset.as_str(),
                    region.bpp,
                    region.cell,
                    region.source_atlas_domain_id == *source_atlas_domain_id,
                )
            }
        };
    ensure!(
        domain.source_path == occurrence.target_record_path
            && domain.tim_offset == occurrence.target_tim_offset
            && domain.bpp == occurrence.target_bpp
            && region_path == occurrence.target_record_path
            && region_tim_offset == occurrence.target_tim_offset
            && region_bpp == occurrence.target_bpp
            && region_domain_matches
            && cell_contains(region_cell, consumer_source_cell)
            && consumer_source_cell.width > 0
            && consumer_source_cell.height > 0,
        "fixed-text consumer occurrence {} has an inconsistent source cell",
        occurrence.occurrence_id
    );

    let projection_ids = consumer_projection_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        !projection_ids.is_empty() && projection_ids.len() == consumer_projection_ids.len(),
        "fixed-text consumer occurrence {} has duplicate or empty projections",
        occurrence.occurrence_id
    );
    for projection_id in projection_ids {
        let projection = catalogs
            .projections
            .projections
            .iter()
            .find(|projection| {
                matches!(
                    projection,
                    PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
                        source,
                        ..
                    } if source.id.as_str() == projection_id
                )
            })
            .with_context(|| {
                format!(
                    "fixed-text consumer occurrence {} names unknown direct numeric projection {projection_id}",
                    occurrence.occurrence_id
                )
            })?;
        let PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
            binding,
            sites,
            physical_region_ids,
            protected_region_ids,
            ..
        } = projection
        else {
            unreachable!("direct numeric projection matched above")
        };
        let has_exact_source_rect = sites.iter().any(|site| {
            let PracticalResultDirectNumericSpriteSite::Geometry(site) = site else {
                return false;
            };
            site.source_rects.iter().any(|rect| {
                rect.u == consumer_source_cell.x
                    && rect.v == consumer_source_cell.y
                    && rect.width == consumer_source_cell.width
                    && rect.height == consumer_source_cell.height
            })
        });
        let projection_declares_region = match source_region {
            PracticalResultFixedTextStaticSourceRegion::Physical { region_id } => {
                physical_region_ids.contains(region_id)
            }
            PracticalResultFixedTextStaticSourceRegion::Protected { region_id } => {
                protected_region_ids.contains(region_id)
            }
        };
        ensure!(
            binding.source_atlas_domain_id == *source_atlas_domain_id
                && binding.evidence_status == PracticalResultProjectionEvidenceStatus::Confirmed
                && projection_declares_region
                && has_exact_source_rect,
            "fixed-text consumer occurrence {} projection {projection_id} does not prove its exact source cell",
            occurrence.occurrence_id
        );
    }
    Ok(())
}

fn validate_judgment_stamp_target_catalog(
    catalog: &PracticalResultJudgmentStampTargetCatalog,
    consumer_catalog: &PracticalResultJudgmentStampConsumerOccurrenceCatalog,
    entries: &[PracticalResultEntry],
    physical_region_catalog: &PracticalResultPhysicalRegionCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind
            == PracticalResultJudgmentStampTargetCatalogKind::JusticeGakuen2PracticalResultJudgmentStampTargetCatalog,
        "unsupported practical-result judgment-stamp target catalog"
    );
    ensure!(
        consumer_catalog.kind
            == PracticalResultJudgmentStampConsumerOccurrenceCatalogKind::JusticeGakuen2PracticalResultJudgmentStampConsumerOccurrenceCatalog
            && !consumer_catalog.occurrences.is_empty(),
        "unsupported or empty practical-result judgment-stamp consumer-occurrence catalog"
    );
    ensure!(
        !catalog.targets.is_empty(),
        "practical-result judgment-stamp catalog has no target"
    );

    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let physical_regions = physical_region_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();

    let mut occurrence_ids = BTreeSet::new();
    for occurrence in &consumer_catalog.occurrences {
        ensure!(
            !occurrence.occurrence_id.is_empty()
                && occurrence_ids.insert(occurrence.occurrence_id.as_str())
                && source_paths.contains(occurrence.target_record_path.as_str())
                && occurrence.target_bpp == 4,
            "invalid practical-result judgment-stamp consumer occurrence {}",
            occurrence.occurrence_id
        );
        parse_hex_offset(&occurrence.target_tim_offset)?;
        ensure_sha256(
            &occurrence.source_tim_sha256,
            "judgment-stamp consumer source TIM",
        )?;
        match &occurrence.evidence {
            PracticalResultTextureConsumerEvidence::RuntimeObservedTexture {
                runtime_artifact_bin_sha256,
                runtime_frame_path,
                runtime_frame_sha256,
            } => {
                ensure!(
                    !runtime_frame_path.trim().is_empty(),
                    "judgment-stamp consumer runtime frame path is empty"
                );
                ensure_sha256(
                    runtime_artifact_bin_sha256,
                    "judgment-stamp consumer runtime artifact",
                )?;
                ensure_sha256(
                    runtime_frame_sha256,
                    "judgment-stamp consumer runtime frame",
                )?;
            }
            PracticalResultTextureConsumerEvidence::StaticBoundIndexedMemberTexture {
                consumer_record_path,
                consumer_source_sha256,
                source_catalog_index,
                selected_member_index,
                loader_span_offset,
                loader_span_size,
                loader_span_sha256,
                indexed_load_call_offset,
                tim_upload_call_offsets,
                ..
            } => {
                let loader_start = parse_hex_offset(loader_span_offset)?;
                let loader_end = loader_start
                    .checked_add(*loader_span_size)
                    .context("indexed-member judgment loader span overflow")?;
                ensure!(
                    source_paths.contains(consumer_record_path.as_str())
                        && *source_catalog_index > 0
                        && *selected_member_index < 2
                        && *loader_span_size > 0
                        && !tim_upload_call_offsets.is_empty()
                        && std::iter::once(indexed_load_call_offset)
                            .chain(tim_upload_call_offsets)
                            .map(|offset| parse_hex_offset(offset))
                            .collect::<Result<Vec<_>>>()?
                            .into_iter()
                            .all(|offset| {
                                offset >= loader_start
                                    && offset.is_multiple_of(4)
                                    && offset + 4 <= loader_end
                            }),
                    "invalid indexed-member judgment consumer evidence for {}",
                    occurrence.occurrence_id
                );
                ensure_sha256(consumer_source_sha256, "judgment consumer source")?;
                ensure_sha256(loader_span_sha256, "judgment indexed-member loader span")?;
            }
            _ => anyhow::bail!(
                "judgment-stamp occurrence {} has unsupported consumer evidence",
                occurrence.occurrence_id
            ),
        }
    }

    let mut target_ids = BTreeSet::new();
    for target in &catalog.targets {
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| {
                format!(
                    "judgment-stamp target {} names unknown semantic entry {}",
                    target.target_id, target.semantic_entry_id
                )
            })?;
        let region = physical_regions
            .get(target.replaces_source_region_id.as_str())
            .with_context(|| {
                format!(
                    "judgment-stamp target {} names unknown source region {}",
                    target.target_id, target.replaces_source_region_id
                )
            })?;
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        let mut target_occurrence_ids = BTreeSet::new();
        ensure!(
            !target.target_id.is_empty()
                && target_ids.insert(target.target_id.as_str())
                && entry.strategy == PracticalResultStrategy::JudgmentStamp
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
                && source_paths.contains(target.target_record_path.as_str())
                && target.target_bpp == 4
                && target.target_cell.width > 0
                && target.target_cell.height > 0
                && region.source_path == target.target_record_path
                && parse_hex_offset(&region.tim_offset)? == tim_offset
                && region.bpp == target.target_bpp
                && region.cell == target.target_cell
                && !target.expected_consumer_occurrence_ids.is_empty()
                && target.expected_consumer_occurrence_ids.iter().all(|id| {
                    !id.is_empty()
                        && target_occurrence_ids.insert(id.as_str())
                        && occurrence_ids.contains(id.as_str())
                        && consumer_catalog.occurrences.iter().any(|occurrence| {
                            occurrence.occurrence_id == *id
                                && occurrence.target_record_path == target.target_record_path
                                && parse_hex_offset(&occurrence.target_tim_offset).ok()
                                    == Some(tim_offset)
                                && occurrence.target_bpp == target.target_bpp
                        })
                }),
            "invalid practical-result judgment-stamp target {}",
            target.target_id
        );
        ensure_sha256(
            &target.expected_preimage_indexed_sha256,
            "judgment-stamp target indexed preimage",
        )?;
    }

    let mut candidate_ids = BTreeSet::new();
    for candidate in &catalog.layout_candidates {
        let entry = entries_by_id
            .get(candidate.semantic_entry_id.as_str())
            .with_context(|| {
                format!(
                    "judgment-stamp candidate {} names unknown semantic entry {}",
                    candidate.candidate_id, candidate.semantic_entry_id
                )
            })?;
        let reference = entry
            .source_references
            .iter()
            .find(|reference| reference.reference_id == candidate.source_reference_id)
            .with_context(|| {
                format!(
                    "judgment-stamp candidate {} names an unrelated source reference",
                    candidate.candidate_id
                )
            })?;
        let region = physical_regions
            .get(candidate.replaces_source_region_id.as_str())
            .with_context(|| {
                format!(
                    "judgment-stamp candidate {} names an unknown source region",
                    candidate.candidate_id
                )
            })?;
        let tim_offset = parse_hex_offset(&candidate.source_tim_offset)?;
        ensure!(
            !candidate.candidate_id.is_empty()
                && candidate_ids.insert(candidate.candidate_id.as_str())
                && entry.strategy == PracticalResultStrategy::JudgmentStamp
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
                && source_paths.contains(candidate.source_path.as_str())
                && candidate.source_bpp == 4
                && candidate.candidate_cell.width > 0
                && candidate.candidate_cell.height > 0
                && reference.physical_region_id == candidate.replaces_source_region_id
                && region.source_path == candidate.source_path
                && parse_hex_offset(&region.tim_offset)? == tim_offset
                && region.bpp == candidate.source_bpp
                && region.cell == candidate.candidate_cell
                && candidate.evidence_status
                    == PracticalResultFixedTextCandidateEvidenceStatus::SourceObservationOnly
                && candidate.authority_status
                    == PracticalResultFixedTextCandidateAuthorityStatus::NonAuthoritativeLayoutCandidate
                && !candidate.unresolved_reason.trim().is_empty(),
            "invalid practical-result judgment-stamp candidate {}",
            candidate.candidate_id
        );
        ensure_sha256(
            &candidate.source_indexed_sha256,
            "judgment-stamp candidate indexed preimage",
        )?;
    }
    Ok(())
}

fn validate_action_cell_target_catalog(
    catalog: &PracticalResultActionCellTargetCatalog,
    consumer_catalog: &PracticalResultActionConsumerOccurrenceCatalog,
    entries: &[PracticalResultEntry],
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind
            == PracticalResultActionCellTargetCatalogKind::JusticeGakuen2PracticalResultActionCellTargetCatalog
            && !catalog.targets.is_empty(),
        "unsupported or empty practical-result action-cell target catalog"
    );
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let occurrences_by_id = consumer_catalog
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.occurrence_id.as_str(), occurrence))
        .collect::<BTreeMap<_, _>>();
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut target_ids = BTreeSet::new();
    let mut target_cells = Vec::new();
    let mut targeted_occurrences = BTreeSet::new();
    for target in &catalog.targets {
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| {
                format!(
                    "action-cell target {} names unknown semantic entry {}",
                    target.target_id, target.semantic_entry_id
                )
            })?;
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        ensure!(
            !target.target_id.is_empty()
                && target_ids.insert(target.target_id.as_str())
                && entry.strategy == PracticalResultStrategy::GlyphSequence
                && entry.font_role == ModeDescendantFontRole::PracticalResultAction
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
                && source_paths.contains(target.target_record_path.as_str())
                && target.target_bpp == 4
                && target.target_cell.width > 0
                && target.target_cell.height > 0
                && target.target_cell.x + target.target_cell.width <= 256
                && target.target_cell.y + target.target_cell.height <= 256
                && target.palette_index > 0
                && target.palette_index < 16
                && target.font_px.is_finite()
                && target.font_px > 0.0
                && !target.consumer_occurrence_ids.is_empty(),
            "invalid practical-result action-cell target {}",
            target.target_id
        );
        ensure_sha256(
            &target.expected_preimage_indexed_sha256,
            "action-cell target indexed preimage",
        )?;
        ensure!(
            target_cells.iter().all(|(path, offset, cell)| {
                *path != target.target_record_path
                    || *offset != tim_offset
                    || !cells_overlap(*cell, target.target_cell)
            }),
            "practical-result action-cell targets overlap"
        );
        target_cells.push((
            target.target_record_path.as_str(),
            tim_offset,
            target.target_cell,
        ));
        let mut target_occurrences = BTreeSet::new();
        for occurrence_id in &target.consumer_occurrence_ids {
            let occurrence = occurrences_by_id
                .get(occurrence_id.as_str())
                .with_context(|| {
                    format!(
                        "action-cell target {} names unknown occurrence {}",
                        target.target_id, occurrence_id
                    )
                })?;
            ensure!(
                target_occurrences.insert(occurrence_id.as_str())
                    && targeted_occurrences.insert(occurrence_id.as_str())
                    && occurrence.semantic_entry_id.as_ref() == Some(&target.semantic_entry_id)
                    && occurrence.unresolved_reason.is_none(),
                "action-cell target {} has an inconsistent occurrence {}",
                target.target_id,
                occurrence_id
            );
        }
        ensure!(
            manifest.protected_content.regions.iter().all(|protected| {
                protected.source_path != target.target_record_path
                    || parse_hex_offset(&protected.tim_offset).ok() != Some(tim_offset)
                    || protected.bpp != target.target_bpp
                    || !cells_overlap(protected.cell, target.target_cell)
            }),
            "action-cell target {} overlaps protected source content",
            target.target_id
        );
    }
    Ok(())
}

fn validate_decorative_background_target_catalog(
    catalog: &PracticalResultDecorativeBackgroundTargetCatalog,
    consumer_catalog: &PracticalResultFixedTextConsumerOccurrenceCatalog,
    entries: &[PracticalResultEntry],
    physical_region_catalog: &PracticalResultPhysicalRegionCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        catalog.kind
            == PracticalResultDecorativeBackgroundTargetCatalogKind::JusticeGakuen2PracticalResultDecorativeBackgroundTargetCatalog
            && !catalog.targets.is_empty(),
        "unsupported or empty practical-result decorative-background target catalog"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let physical_regions = physical_region_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let occurrences = consumer_catalog
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.occurrence_id.as_str(), occurrence))
        .collect::<BTreeMap<_, _>>();
    let authored_decorative_ids = entries
        .iter()
        .filter(|entry| {
            entry.strategy == PracticalResultStrategy::DecorativeMask
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
        })
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut target_ids = BTreeSet::new();
    let mut target_surfaces = BTreeSet::new();
    let mut targeted_semantic_ids = BTreeSet::new();
    for target in &catalog.targets {
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        let source_region = physical_regions
            .get(target.replaces_source_region_id.as_str())
            .with_context(|| {
                format!(
                    "decorative-background target {} names unknown source region {}",
                    target.target_id, target.replaces_source_region_id
                )
            })?;
        ensure!(
            !target.target_id.is_empty()
                && target_ids.insert(target.target_id.as_str())
                && target_surfaces.insert((target.target_record_path.as_str(), tim_offset))
                && source_paths.contains(target.target_record_path.as_str())
                && target.target_bpp == 8
                && target.source_cell.width == 512
                && target.source_cell.height == 480
                && source_region.source_path == target.target_record_path
                && parse_hex_offset(&source_region.tim_offset)? == tim_offset
                && source_region.bpp == target.target_bpp
                && source_region.cell == target.source_cell
                && source_region.source_indexed_sha256 == target.expected_preimage_indexed_sha256
                && target.background_palette_index < u8::MAX
                && (target.clear_cells.is_empty() != target.preserve_regions.is_empty())
                && ((target.composition_stage
                    == PracticalResultDecorativeCompositionStage::AfterFixedText
                    && !target.clear_cells.is_empty())
                    || (target.composition_stage
                        == PracticalResultDecorativeCompositionStage::BeforeFixedText
                        && !target.preserve_regions.is_empty()))
                && !target.text_placements.is_empty()
                && !target.expected_consumer_occurrence_ids.is_empty(),
            "invalid decorative-background target {}",
            target.target_id
        );
        ensure_sha256(
            &target.expected_preimage_indexed_sha256,
            "decorative-background indexed preimage",
        )?;
        parse_palette_word(&target.background_palette_word)?;
        ensure!(
            target.clear_cells.iter().all(|cell| {
                cell_contains(target.source_cell, *cell)
                    && target
                        .clear_cells
                        .iter()
                        .filter(|other| cells_overlap(**other, *cell))
                        .count()
                        == 1
            }),
            "decorative-background target {} has invalid or overlapping clear cells",
            target.target_id
        );
        let mut preserve_region_ids = BTreeSet::new();
        let mut preserve_content_kinds = BTreeSet::new();
        for region in &target.preserve_regions {
            ensure!(
                !region.region_id.trim().is_empty()
                    && preserve_region_ids.insert(region.region_id.as_str())
                    && preserve_content_kinds.insert(region.content_kind)
                    && !region.spans.is_empty(),
                "decorative-background target {} has an invalid preservation label",
                target.target_id
            );
            let mut rows = BTreeMap::<usize, Vec<(usize, usize)>>::new();
            for span in &region.spans {
                ensure!(
                    span.width > 0
                        && span.y >= target.source_cell.y
                        && span.y < target.source_cell.y + target.source_cell.height
                        && span.x >= target.source_cell.x
                        && span.x + span.width <= target.source_cell.x + target.source_cell.width,
                    "decorative-background target {} has an out-of-bounds preservation span",
                    target.target_id
                );
                rows.entry(span.y)
                    .or_default()
                    .push((span.x, span.x + span.width));
            }
            for spans in rows.values_mut() {
                spans.sort_unstable();
                ensure!(
                    spans.windows(2).all(|pair| pair[0].1 <= pair[1].0),
                    "decorative-background target {} has overlapping preservation spans",
                    target.target_id
                );
            }
        }
        ensure!(
            target.preserve_cells.iter().all(|cell| {
                cell_contains(target.source_cell, *cell)
                    && target
                        .preserve_cells
                        .iter()
                        .filter(|other| cells_overlap(**other, *cell))
                        .count()
                        == 1
            }),
            "decorative-background target {} has invalid fixed-text preserve cells",
            target.target_id
        );
        let expected_semantic_ids = entries
            .iter()
            .filter(|entry| {
                entry.strategy == PracticalResultStrategy::DecorativeMask
                    && entry.development_status == PracticalResultDevelopmentStatus::Authored
                    && entry.source_references.iter().any(|reference| {
                        reference.physical_region_id == target.replaces_source_region_id
                    })
            })
            .map(|entry| entry.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut placement_ids = BTreeSet::new();
        let mut target_semantic_ids = BTreeSet::new();
        for placement in &target.text_placements {
            let entry = entries_by_id
                .get(placement.semantic_entry_id.as_str())
                .with_context(|| {
                    format!(
                        "decorative-background target {} names unknown semantic entry {}",
                        target.target_id, placement.semantic_entry_id
                    )
                })?;
            let new_in_target = !placement.placement_id.trim().is_empty()
                && placement_ids.insert(placement.placement_id.as_str());
            target_semantic_ids.insert(placement.semantic_entry_id.as_str());
            targeted_semantic_ids.insert(placement.semantic_entry_id.as_str());
            ensure!(
                new_in_target
                    && entry.strategy == PracticalResultStrategy::DecorativeMask
                    && entry.font_role == ModeDescendantFontRole::PracticalResultBranding
                    && entry.development_status == PracticalResultDevelopmentStatus::Authored
                    && entry.source_references.iter().any(|reference| {
                        reference.physical_region_id == target.replaces_source_region_id
                    })
                    && (target
                        .clear_cells
                        .iter()
                        .any(|cell| cells_overlap(*cell, placement.cell))
                        || (!target.preserve_regions.is_empty()
                            && cell_contains(target.source_cell, placement.cell)))
                    && cell_contains(target.source_cell, placement.cell)
                    && placement.font_px.is_finite()
                    && (8.0..=32.0).contains(&placement.font_px)
                    && placement.clockwise_rotation_degrees.is_finite()
                    && (-20.0..=20.0).contains(&placement.clockwise_rotation_degrees)
                    && placement.outline_palette_index != target.background_palette_index
                    && placement.fill_palette_index != target.background_palette_index
                    && placement.outline_palette_index != placement.fill_palette_index,
                "invalid decorative text placement {} for {} in {}",
                placement.placement_id,
                placement.semantic_entry_id,
                target.target_id
            );
            parse_palette_word(&placement.outline_palette_word)?;
            parse_palette_word(&placement.fill_palette_word)?;
        }
        ensure!(
            target_semantic_ids == expected_semantic_ids,
            "decorative-background target {} does not cover its semantic wordmarks",
            target.target_id
        );
        for occurrence_id in &target.expected_consumer_occurrence_ids {
            let occurrence = occurrences.get(occurrence_id.as_str()).with_context(|| {
                format!(
                    "decorative-background target {} names unknown consumer occurrence {}",
                    target.target_id, occurrence_id
                )
            })?;
            ensure!(
                occurrence.target_record_path == target.target_record_path
                    && parse_hex_offset(&occurrence.target_tim_offset)? == tim_offset
                    && occurrence.target_bpp == target.target_bpp,
                "decorative-background target {} mismatches consumer occurrence {}",
                target.target_id,
                occurrence_id
            );
        }
    }
    ensure!(
        targeted_semantic_ids == authored_decorative_ids,
        "decorative-background catalog does not cover the authored semantic denominator"
    );
    Ok(())
}

fn parse_palette_word(value: &str) -> Result<u16> {
    u16::from_str_radix(
        value
            .strip_prefix("0x")
            .context("decorative palette word is not hexadecimal")?,
        16,
    )
    .context("invalid decorative palette word")
}

fn validate_translation_state(entry: &PracticalResultEntry) -> Result<()> {
    match entry.development_status {
        PracticalResultDevelopmentStatus::Authored => ensure!(
            entry
                .korean_text
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
                && entry.release_status == PracticalResultReleaseStatus::NeedsHumanReview
                && entry.unresolved_reason.is_none()
                && !entry.source_references.is_empty(),
            "authored practical-result entry {} lacks Korean text or source references",
            entry.id
        ),
        PracticalResultDevelopmentStatus::Unresolved => ensure!(
            entry.korean_text.is_none()
                && entry.release_status == PracticalResultReleaseStatus::NotApplicable
                && (entry
                    .unresolved_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty())
                    || !entry.unresolved_source_references.is_empty())
                && (!entry.source_references.is_empty()
                    || !entry.unresolved_source_references.is_empty()),
            "unresolved practical-result entry {} contains authored state",
            entry.id
        ),
    }
    Ok(())
}

fn validate_strategy_role(entry: &PracticalResultEntry) -> Result<()> {
    let valid = match entry.strategy {
        PracticalResultStrategy::FixedText => matches!(
            entry.font_role,
            ModeDescendantFontRole::PracticalResultHeading
                | ModeDescendantFontRole::PracticalResultLabel
                | ModeDescendantFontRole::PracticalResultHint
                | ModeDescendantFontRole::PracticalResultSmallJudgment
        ),
        PracticalResultStrategy::GlyphSequence => matches!(
            entry.font_role,
            ModeDescendantFontRole::PracticalResultAction
                | ModeDescendantFontRole::PracticalResultLabel
        ),
        PracticalResultStrategy::JudgmentStamp => {
            entry.font_role == ModeDescendantFontRole::PracticalResultJudgmentStamp
        }
        PracticalResultStrategy::DecorativeMask => {
            entry.font_role == ModeDescendantFontRole::PracticalResultBranding
        }
    };
    ensure!(
        valid,
        "practical-result entry {} uses a font role for another strategy",
        entry.id
    );
    Ok(())
}

fn cell_contains(outer: Cell, inner: Cell) -> bool {
    inner.width > 0
        && inner.height > 0
        && inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn ensure_sha256(value: &str, role: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "practical-result {role} SHA-256 is invalid"
    );
    Ok(())
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "practical-result asset path must stay inside its asset directory"
    );
    Ok(())
}
