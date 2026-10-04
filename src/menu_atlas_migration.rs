use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

#[path = "menu_atlas_migration/model.rs"]
mod model;

pub use model::{MenuAtlasMigrationAuditConfig, MenuAtlasMigrationAuditReport};

use model::{
    CandidateMenuAtlasWrite, CandidateMenuAtlasWriteOverlap, MenuAtlasWriterComponent,
    MenuAtlasWriterComponentAudit, SourceMenuCodeMigrationObligation,
};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::menu_atlas::{
    MENU_ATLAS_HEIGHT, MENU_ATLAS_RECORD_PATH, MENU_ATLAS_WIDTH, MENU_GLYPH_CELL_HEIGHT,
    MENU_GLYPH_CELL_WIDTH, load_source_menu_atlas, logical_code_fragments, logical_code_position,
    parse_menu_code,
};
use crate::menu_atlas_labeling::{
    MenuAtlasLabel, MenuAtlasLabelDocument, MenuAtlasLabelReviewStatus,
    MenuAtlasLabelSourceIdentity, validate_menu_atlas_labels,
};
use crate::menu_audit::collect_menu_code_audit;
use crate::menu_glyph_audit::collect_menu_glyph_audit;
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{
    Cell, RgbaImage, cells_overlap, decode_4bpp_rgba_in_prefix, read_4bpp_indexed_image_in_prefix,
};
use crate::tim_preview::write_tim_preview;

const OPTIONS_REPORT_PATH: &str = "options/options-record-build.json";
const TITLE_MENU_REPORT_PATH: &str = "title-menu/title-menu-build.json";
const TITLE_NOTICE_REPORT_PATH: &str = "title-notice/title-notice-build.json";
const CANDIDATE_BUILD_MANIFEST_PATH: &str = "justice-gakuen2-korean-development.json";
const CANDIDATE_OUTPUT_BIN_PATH: &str = "justice-gakuen2-korean-development.bin";
const CANDIDATE_OUTPUT_CUE_PATH: &str = "justice-gakuen2-korean-development.cue";
const CANDIDATE_BUILD_MANIFEST_KIND: &str =
    "Justice Gakuen 2 non-release Korean development disc build";
const OPTIONS_REPORT_KIND: &str = "Justice Gakuen 2 source-bound options record build";
const TITLE_MENU_REPORT_KIND: &str = "Justice Gakuen 2 source-bound title-adjacent menu build";
const TITLE_NOTICE_REPORT_KIND: &str =
    "Justice Gakuen 2 source-bound title notice development build";
const TWENTY_BY_TWENTY_PIXEL_COUNT: usize = MENU_GLYPH_CELL_WIDTH * MENU_GLYPH_CELL_HEIGHT;

#[derive(Debug, Deserialize)]
struct CandidateBuildReportProjection {
    kind: String,
    source_bin_sha256: String,
    #[serde(default)]
    allocation_proven_reclaimable: Option<bool>,
    #[serde(default)]
    installed_glyph_count: Option<usize>,
    glyphs: Vec<CandidateGlyphProjection>,
}

#[derive(Debug, Deserialize)]
struct CandidateGlyphProjection {
    character: char,
    code: String,
    cell: Cell,
    #[serde(default)]
    global_menu_resident: Option<bool>,
    #[serde(default)]
    reused: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CandidateDiscBuildProjection {
    kind: String,
    source_bin_sha256: String,
    output_bin_sha256: String,
    options_build_manifest_sha256: String,
    title_menu_build_manifest_sha256: String,
    title_notice_build_manifest_sha256: String,
    menu_surface_writes_disjoint: bool,
    menu: CandidateMenuRecordProjection,
}

#[derive(Debug, Deserialize)]
struct CandidateMenuRecordProjection {
    path: String,
    expected_stored_sha256: String,
    readback_stored_sha256: String,
    expected_decoded_sha256: String,
    readback_decoded_sha256: String,
}

struct CandidateArtifactBinding {
    manifest_file: String,
    manifest_sha256: String,
    output_bin_file: String,
    output_bin_sha256: String,
    output_cue_file: String,
    output_cue_sha256: String,
    menu_stored_sha256: String,
    menu_decoded_sha256: String,
    atlas_indexed_sha256: String,
    atlas_indexed_pixels: Vec<u8>,
    menu_surface_writes_disjoint: bool,
    options_report_sha256: String,
    title_menu_report_sha256: String,
    title_notice_report_sha256: String,
}

#[derive(Debug, Clone)]
struct CandidateWriteInput {
    id: String,
    component: MenuAtlasWriterComponent,
    character: char,
    code: String,
    cell: Cell,
}

#[derive(Debug, Clone)]
struct SourceCodeReference {
    code: String,
    footprint_fragments: Vec<Cell>,
    occurrence_count: usize,
    overlays: Vec<String>,
    exact_dialogue_pixel_text: Option<String>,
}

struct MigrationAnalysis {
    candidate_declared_written_pixel_count: usize,
    candidate_unique_written_pixel_count: usize,
    candidate_declared_twenty_by_twenty_area_equivalent_count: usize,
    candidate_write_overlaps: Vec<CandidateMenuAtlasWriteOverlap>,
    source_code_occurrence_migration_obligation_count: usize,
    source_consumer_overlays: Vec<String>,
    exact_dialogue_texts: Vec<String>,
    confirmed_label_covered_unique_written_pixel_count: usize,
    unclassified_unique_written_pixel_count: usize,
    candidate_writes: Vec<CandidateMenuAtlasWrite>,
    source_code_migration_obligations: Vec<SourceMenuCodeMigrationObligation>,
}

struct CandidateAtlasReadbackAnalysis {
    changed_pixel_count: usize,
    changed_pixel_outside_declared_writes_count: usize,
    declared_write_unchanged_pixel_count: usize,
}

pub fn audit_menu_atlas_migration(
    config: &MenuAtlasMigrationAuditConfig,
) -> Result<MenuAtlasMigrationAuditReport> {
    let preview_path = migration_preview_path(&config.output)?;
    require_replace_permission(&config.output, config.force)?;
    require_replace_permission(&preview_path, config.force)?;

    let source = load_source_menu_atlas(&config.cue)?;
    let source_atlas_indexed_sha256 = sha256_bytes(&source.indexed.pixels);
    let source_preview = decode_4bpp_rgba_in_prefix(&source.menu_decoded, 0, 0)?;
    let labels = read_json::<MenuAtlasLabelDocument>(&config.labels)?;
    validate_menu_atlas_labels(
        &labels,
        MenuAtlasLabelSourceIdentity {
            source_bin_sha256: &source.source_bin_sha256,
            menu_stored_sha256: &source.menu_stored_sha256,
            menu_decoded_sha256: &source.menu_decoded_sha256,
            source_atlas_indexed_sha256: &source_atlas_indexed_sha256,
            atlas_width: source.indexed.width,
            atlas_height: source.indexed.height,
        },
    )?;

    let glyph_audit = collect_menu_glyph_audit(&config.cue, &config.dialogue_codebook)?;
    let code_audit = collect_menu_code_audit(&config.cue, config.address_flow_state_budget)?;
    ensure!(
        glyph_audit.source_bin_sha256 == source.source_bin_sha256
            && glyph_audit.menu_stored_sha256 == source.menu_stored_sha256
            && glyph_audit.menu_decoded_sha256 == source.menu_decoded_sha256
            && code_audit.source_bin_sha256 == source.source_bin_sha256,
        "menu atlas migration inputs do not share one exact source artifact"
    );
    let source_references = collect_source_code_references(&glyph_audit, &code_audit)?;
    let candidate_artifact =
        load_candidate_artifact_binding(&config.candidate_build_dir, &source.source_bin_sha256)?;

    let component_specs = [
        (
            MenuAtlasWriterComponent::Options,
            OPTIONS_REPORT_PATH,
            OPTIONS_REPORT_KIND,
        ),
        (
            MenuAtlasWriterComponent::TitleMenu,
            TITLE_MENU_REPORT_PATH,
            TITLE_MENU_REPORT_KIND,
        ),
        (
            MenuAtlasWriterComponent::TitleNotice,
            TITLE_NOTICE_REPORT_PATH,
            TITLE_NOTICE_REPORT_KIND,
        ),
    ];
    let mut component_audits = Vec::with_capacity(component_specs.len());
    let mut candidate_writes = Vec::new();
    for (component, relative_path, expected_kind) in component_specs {
        let expected_report_sha256 = match component {
            MenuAtlasWriterComponent::Options => &candidate_artifact.options_report_sha256,
            MenuAtlasWriterComponent::TitleMenu => &candidate_artifact.title_menu_report_sha256,
            MenuAtlasWriterComponent::TitleNotice => &candidate_artifact.title_notice_report_sha256,
        };
        let (component_audit, mut writes) = load_candidate_component(
            &config.candidate_build_dir.join(relative_path),
            component,
            expected_kind,
            &source.source_bin_sha256,
            expected_report_sha256,
        )?;
        component_audits.push(component_audit);
        candidate_writes.append(&mut writes);
    }
    ensure_unique_candidate_write_ids(&candidate_writes)?;
    let analysis = analyze_migration(&candidate_writes, &source_references, &labels.labels)?;
    let candidate_atlas_readback = analyze_candidate_atlas_readback(
        &source.indexed.pixels,
        &candidate_artifact.atlas_indexed_pixels,
        &candidate_writes,
    )?;
    ensure!(
        candidate_atlas_readback.changed_pixel_outside_declared_writes_count == 0,
        "candidate atlas changes pixels outside the three declared global-writer reports"
    );
    ensure!(
        candidate_artifact.menu_surface_writes_disjoint
            == analysis.candidate_write_overlaps.is_empty(),
        "candidate atlas write overlap result disagrees with the cumulative build manifest"
    );

    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let overlay_preview =
        build_migration_overlay_preview(source_preview, &candidate_writes, &labels.labels);
    write_tim_preview(&preview_path, &overlay_preview)?;
    let migration_risk_overlay_preview_sha256 = sha256_file(&preview_path)?;

    let confirmed_label_count = labels
        .labels
        .iter()
        .filter(|label| label.review_status == MenuAtlasLabelReviewStatus::Confirmed)
        .count();
    let needs_review_label_count = labels
        .labels
        .iter()
        .filter(|label| label.review_status == MenuAtlasLabelReviewStatus::NeedsReview)
        .count();
    let unreviewed_label_count =
        labels.labels.len() - confirmed_label_count - needs_review_label_count;
    let fully_overwritten_source_code_obligation_count = analysis
        .source_code_migration_obligations
        .iter()
        .filter(|obligation| obligation.fully_overwritten)
        .count();
    let source_code_with_exact_dialogue_text_obligation_count = analysis
        .source_code_migration_obligations
        .iter()
        .filter(|obligation| obligation.exact_dialogue_pixel_text.is_some())
        .count();
    let shared_reference_address_flow_budget_exhausted_seed_count = code_audit
        .shared_reference_sources
        .iter()
        .map(|source| source.address_flow_budget_exhausted_seed_count)
        .sum();
    let candidate_write_overlap_pair_count = analysis.candidate_write_overlaps.len();
    let source_code_migration_obligation_count = analysis.source_code_migration_obligations.len();
    let partially_overwritten_source_code_obligation_count =
        source_code_migration_obligation_count - fully_overwritten_source_code_obligation_count;
    let report = MenuAtlasMigrationAuditReport {
        kind: "Justice Gakuen 2 source-bound shared MENU atlas migration obligation audit"
            .to_string(),
        source_cue: canonical_path_string(&config.cue)?,
        source_bin_sha256: source.source_bin_sha256,
        menu_path: MENU_ATLAS_RECORD_PATH.to_string(),
        menu_stored_sha256: source.menu_stored_sha256,
        menu_decoded_sha256: source.menu_decoded_sha256,
        source_atlas_indexed_sha256,
        dialogue_codebook_sha256: glyph_audit.dialogue_codebook_sha256,
        candidate_build_directory: canonical_path_string(&config.candidate_build_dir)?,
        candidate_build_manifest_file: candidate_artifact.manifest_file,
        candidate_build_manifest_sha256: candidate_artifact.manifest_sha256,
        candidate_output_bin_file: candidate_artifact.output_bin_file,
        candidate_output_bin_sha256: candidate_artifact.output_bin_sha256,
        candidate_output_cue_file: candidate_artifact.output_cue_file,
        candidate_output_cue_sha256: candidate_artifact.output_cue_sha256,
        candidate_menu_stored_sha256: candidate_artifact.menu_stored_sha256,
        candidate_menu_decoded_sha256: candidate_artifact.menu_decoded_sha256,
        candidate_atlas_indexed_sha256: candidate_artifact.atlas_indexed_sha256,
        candidate_menu_surface_writes_disjoint: candidate_artifact
            .menu_surface_writes_disjoint,
        label_document_file: canonical_path_string(&config.labels)?,
        address_flow_state_budget: code_audit.address_flow_state_budget,
        dat1_address_flow_budget_exhausted_seed_count: code_audit
            .dat1_address_flow_budget_exhausted_seed_count,
        shared_reference_address_flow_budget_exhausted_seed_count,
        statically_used_source_code_count: code_audit.used_code_count,
        proven_reclaimable_source_code_count: code_audit.proven_reclaimable_code_count,
        candidate_components: component_audits,
        candidate_write_count: analysis.candidate_writes.len(),
        candidate_declared_written_pixel_count: analysis
            .candidate_declared_written_pixel_count,
        candidate_unique_written_pixel_count: analysis.candidate_unique_written_pixel_count,
        candidate_declared_twenty_by_twenty_area_equivalent_count: analysis
            .candidate_declared_twenty_by_twenty_area_equivalent_count,
        candidate_atlas_changed_pixel_count: candidate_atlas_readback.changed_pixel_count,
        candidate_atlas_changed_pixel_outside_declared_writes_count: candidate_atlas_readback
            .changed_pixel_outside_declared_writes_count,
        candidate_declared_write_unchanged_pixel_count: candidate_atlas_readback
            .declared_write_unchanged_pixel_count,
        candidate_atlas_changes_confined_to_declared_writes: true,
        candidate_write_overlap_pair_count,
        candidate_write_overlaps: analysis.candidate_write_overlaps,
        source_code_migration_obligation_count,
        fully_overwritten_source_code_obligation_count,
        partially_overwritten_source_code_obligation_count,
        source_code_occurrence_migration_obligation_count: analysis
            .source_code_occurrence_migration_obligation_count,
        source_consumer_overlay_count: analysis.source_consumer_overlays.len(),
        source_consumer_overlays: analysis.source_consumer_overlays,
        source_code_with_exact_dialogue_text_obligation_count,
        exact_dialogue_texts: analysis.exact_dialogue_texts,
        label_count: labels.labels.len(),
        confirmed_label_count,
        needs_review_label_count,
        unreviewed_label_count,
        candidate_written_pixels_fully_classified: analysis
            .unclassified_unique_written_pixel_count
            == 0,
        confirmed_label_covered_unique_written_pixel_count: analysis
            .confirmed_label_covered_unique_written_pixel_count,
        unclassified_unique_written_pixel_count: analysis
            .unclassified_unique_written_pixel_count,
        candidate_writes: analysis.candidate_writes,
        source_code_migration_obligations: analysis.source_code_migration_obligations,
        migration_risk_overlay_preview_file: preview_path
            .file_name()
            .context("migration preview has no file name")?
            .to_string_lossy()
            .into_owned(),
        migration_risk_overlay_preview_sha256,
        migration_proof_complete: false,
        local_evidence_only: true,
        limitations: vec![
            "This report turns the last candidate's global writes into source migration obligations; it does not rehabilitate that artifact as a current build or release baseline.".to_string(),
            "Candidate-BIN pixel changes are confined to the declared writer rectangles, but write confinement does not prove that the covered source graphics or consumers were safe to replace.".to_string(),
            "Static source references are an observed lower bound. Every reported consumer must be migrated and the rebuilt consumers must then pass a residual-reference audit before any shared region becomes reclaimable.".to_string(),
            "Free-rectangle labels are human semantic decisions. Even complete confirmed-label coverage classifies candidate pixels but does not prove consumer migration, runtime correctness, or release approval.".to_string(),
            "Twenty-by-twenty area equivalents are area measurements only. They are not claims that the atlas consists of independent fixed-size glyph slots.".to_string(),
            "Exact dialogue text is inherited only from byte-identical source pixels; unmatched source graphics and different-sized glyphs remain intentionally unlabeled.".to_string(),
        ],
    };
    write_json(&config.output, &report)?;
    Ok(report)
}

fn load_candidate_component(
    path: &Path,
    component: MenuAtlasWriterComponent,
    expected_kind: &str,
    expected_source_bin_sha256: &str,
    expected_report_sha256: &str,
) -> Result<(MenuAtlasWriterComponentAudit, Vec<CandidateWriteInput>)> {
    let report_sha256 = sha256_file(path)?;
    ensure!(
        report_sha256 == expected_report_sha256,
        "{} candidate report hash disagrees with the cumulative build manifest",
        component.key()
    );
    let report = read_json::<CandidateBuildReportProjection>(path)?;
    ensure!(
        report.kind == expected_kind,
        "unexpected {} candidate report kind: {}",
        component.key(),
        report.kind
    );
    ensure!(
        report.source_bin_sha256 == expected_source_bin_sha256,
        "{} candidate report targets a different source BIN",
        component.key()
    );
    match component {
        MenuAtlasWriterComponent::Options | MenuAtlasWriterComponent::TitleMenu => ensure!(
            report.allocation_proven_reclaimable.is_some(),
            "{} candidate report omits its allocation proof state",
            component.key()
        ),
        MenuAtlasWriterComponent::TitleNotice => {}
    }

    let glyph_entry_count = report.glyphs.len();
    let selection_policy = match component {
        MenuAtlasWriterComponent::Options => {
            ensure!(
                report
                    .glyphs
                    .iter()
                    .all(|glyph| glyph.global_menu_resident.is_some()),
                "options candidate glyph omits global residency"
            );
            "select glyph entries whose global_menu_resident field is true"
        }
        MenuAtlasWriterComponent::TitleMenu => "select every title-menu glyph entry",
        MenuAtlasWriterComponent::TitleNotice => {
            ensure!(
                report.glyphs.iter().all(|glyph| glyph.reused.is_some()),
                "title-notice candidate glyph omits reuse state"
            );
            "select title-notice glyph entries whose reused field is false"
        }
    };
    let selected = report
        .glyphs
        .into_iter()
        .filter(|glyph| match component {
            MenuAtlasWriterComponent::Options => glyph.global_menu_resident == Some(true),
            MenuAtlasWriterComponent::TitleMenu => true,
            MenuAtlasWriterComponent::TitleNotice => glyph.reused == Some(false),
        })
        .map(|glyph| candidate_write_input(component, glyph))
        .collect::<Result<Vec<_>>>()?;
    if component == MenuAtlasWriterComponent::TitleNotice {
        ensure!(
            report.installed_glyph_count == Some(selected.len()),
            "title-notice installed glyph count disagrees with non-reused writes"
        );
    }
    let selected_written_pixel_count = selected.iter().map(|write| cell_area(write.cell)).sum();
    let selected_twenty_by_twenty_area_equivalent_count = selected
        .iter()
        .map(|write| cell_area(write.cell) / TWENTY_BY_TWENTY_PIXEL_COUNT)
        .sum();
    let audit = MenuAtlasWriterComponentAudit {
        component,
        report_file: canonical_path_string(path)?,
        report_sha256,
        report_kind: report.kind,
        source_bin_sha256: report.source_bin_sha256,
        allocation_proven_reclaimable: report.allocation_proven_reclaimable,
        glyph_entry_count,
        selected_global_write_count: selected.len(),
        selected_written_pixel_count,
        selected_twenty_by_twenty_area_equivalent_count,
        selection_policy: selection_policy.to_string(),
        preview_color_rgb: component.preview_color_rgb(),
    };
    Ok((audit, selected))
}

fn load_candidate_artifact_binding(
    candidate_build_dir: &Path,
    expected_source_bin_sha256: &str,
) -> Result<CandidateArtifactBinding> {
    let manifest_path = candidate_build_dir.join(CANDIDATE_BUILD_MANIFEST_PATH);
    let output_bin_path = candidate_build_dir.join(CANDIDATE_OUTPUT_BIN_PATH);
    let output_cue_path = candidate_build_dir.join(CANDIDATE_OUTPUT_CUE_PATH);
    let manifest = read_json::<CandidateDiscBuildProjection>(&manifest_path)?;
    ensure!(
        manifest.kind == CANDIDATE_BUILD_MANIFEST_KIND,
        "unexpected candidate disc build manifest kind: {}",
        manifest.kind
    );
    ensure!(
        manifest.source_bin_sha256 == expected_source_bin_sha256,
        "candidate disc build manifest targets a different source BIN"
    );
    let output_bin_sha256 = sha256_file(&output_bin_path)?;
    ensure!(
        output_bin_sha256 == manifest.output_bin_sha256,
        "candidate output BIN hash disagrees with its build manifest"
    );
    let cue = CueSheet::parse(&output_cue_path)?;
    ensure!(
        cue.image_path.canonicalize()? == output_bin_path.canonicalize()?,
        "candidate output CUE does not bind the audited BIN"
    );
    ensure!(
        manifest.menu.path == MENU_ATLAS_RECORD_PATH,
        "candidate build manifest MENU record path changed"
    );
    let (_, menu_stored) = rebuild::read_record(&output_bin_path, MENU_ATLAS_RECORD_PATH)?;
    let menu_stored_sha256 = sha256_bytes(&menu_stored);
    ensure!(
        manifest.menu.expected_stored_sha256 == menu_stored_sha256
            && manifest.menu.readback_stored_sha256 == menu_stored_sha256,
        "candidate MENU stored hash disagrees with its build manifest"
    );
    let menu_decoded = decompress(&menu_stored, true)?;
    let menu_decoded_sha256 = sha256_bytes(&menu_decoded);
    ensure!(
        manifest.menu.expected_decoded_sha256 == menu_decoded_sha256
            && manifest.menu.readback_decoded_sha256 == menu_decoded_sha256,
        "candidate MENU decoded hash disagrees with its build manifest"
    );
    let indexed = read_4bpp_indexed_image_in_prefix(&menu_decoded, 0)?;
    ensure!(
        indexed.width == MENU_ATLAS_WIDTH && indexed.height == MENU_ATLAS_HEIGHT,
        "candidate MENU shared atlas dimensions changed"
    );
    Ok(CandidateArtifactBinding {
        manifest_file: canonical_path_string(&manifest_path)?,
        manifest_sha256: sha256_file(&manifest_path)?,
        output_bin_file: canonical_path_string(&output_bin_path)?,
        output_bin_sha256,
        output_cue_file: canonical_path_string(&output_cue_path)?,
        output_cue_sha256: sha256_file(&output_cue_path)?,
        menu_stored_sha256,
        menu_decoded_sha256,
        atlas_indexed_sha256: sha256_bytes(&indexed.pixels),
        atlas_indexed_pixels: indexed.pixels,
        menu_surface_writes_disjoint: manifest.menu_surface_writes_disjoint,
        options_report_sha256: manifest.options_build_manifest_sha256,
        title_menu_report_sha256: manifest.title_menu_build_manifest_sha256,
        title_notice_report_sha256: manifest.title_notice_build_manifest_sha256,
    })
}

fn analyze_candidate_atlas_readback(
    source_pixels: &[u8],
    candidate_pixels: &[u8],
    writes: &[CandidateWriteInput],
) -> Result<CandidateAtlasReadbackAnalysis> {
    ensure!(
        source_pixels.len() == MENU_ATLAS_WIDTH * MENU_ATLAS_HEIGHT
            && candidate_pixels.len() == source_pixels.len(),
        "source and candidate atlas pixel dimensions disagree"
    );
    let mut declared_write_mask = vec![false; source_pixels.len()];
    for write in writes {
        ensure_cell_in_atlas(write.cell)?;
        mark_cell(&mut declared_write_mask, write.cell);
    }
    let mut changed_pixel_count = 0;
    let mut changed_pixel_outside_declared_writes_count = 0;
    let mut declared_write_unchanged_pixel_count = 0;
    for ((&source, &candidate), &declared) in source_pixels
        .iter()
        .zip(candidate_pixels)
        .zip(&declared_write_mask)
    {
        if source != candidate {
            changed_pixel_count += 1;
            if !declared {
                changed_pixel_outside_declared_writes_count += 1;
            }
        } else if declared {
            declared_write_unchanged_pixel_count += 1;
        }
    }
    Ok(CandidateAtlasReadbackAnalysis {
        changed_pixel_count,
        changed_pixel_outside_declared_writes_count,
        declared_write_unchanged_pixel_count,
    })
}

fn candidate_write_input(
    component: MenuAtlasWriterComponent,
    glyph: CandidateGlyphProjection,
) -> Result<CandidateWriteInput> {
    let parsed_code = parse_menu_code(&glyph.code)?;
    let position = logical_code_position(parsed_code)?;
    ensure!(
        glyph.cell.x == position.x && glyph.cell.y == position.y,
        "{} candidate cell does not begin at code {} physical position",
        component.key(),
        glyph.code
    );
    ensure!(
        glyph.cell.width > 0
            && glyph.cell.height > 0
            && glyph.cell.width.is_multiple_of(MENU_GLYPH_CELL_WIDTH)
            && glyph.cell.height.is_multiple_of(MENU_GLYPH_CELL_HEIGHT),
        "{} candidate cell for {} is not a positive multiple of 20x20",
        component.key(),
        glyph.code
    );
    ensure_cell_in_atlas(glyph.cell)?;
    ensure!(
        logical_code_fragments(parsed_code, glyph.cell.width, glyph.cell.height)? == [glyph.cell],
        "{} candidate cell for {} relies on wrapped writes not represented by its report",
        component.key(),
        glyph.code
    );
    Ok(CandidateWriteInput {
        id: format!("{}:{}", component.key(), glyph.code),
        component,
        character: glyph.character,
        code: glyph.code,
        cell: glyph.cell,
    })
}

fn ensure_unique_candidate_write_ids(writes: &[CandidateWriteInput]) -> Result<()> {
    let mut ids = BTreeSet::new();
    for write in writes {
        ensure!(
            ids.insert(write.id.as_str()),
            "candidate report repeats global write {}",
            write.id
        );
    }
    Ok(())
}

fn collect_source_code_references(
    glyph_audit: &crate::menu_glyph_audit::MenuGlyphAuditManifest,
    code_audit: &crate::menu_audit::MenuCodeAuditManifest,
) -> Result<Vec<SourceCodeReference>> {
    let mut glyphs = BTreeMap::new();
    for glyph in &glyph_audit.cells {
        ensure!(
            glyphs.insert(glyph.code.as_str(), glyph).is_none(),
            "menu glyph audit repeats code {}",
            glyph.code
        );
    }
    ensure!(
        glyphs.len() == glyph_audit.addressable_code_count,
        "menu glyph audit code count disagrees with its cells"
    );
    ensure!(
        code_audit.used_codes.len() == code_audit.used_code_count,
        "menu code audit used-code count disagrees with its entries"
    );
    code_audit
        .used_codes
        .iter()
        .map(|used| {
            ensure!(
                used.occurrence_count > 0,
                "used menu code has no occurrence"
            );
            let parsed = parse_menu_code(&used.code)?;
            let glyph = glyphs
                .get(used.code.as_str())
                .with_context(|| format!("menu glyph audit omits used code {}", used.code))?;
            Ok(SourceCodeReference {
                code: used.code.clone(),
                footprint_fragments: logical_code_fragments(
                    parsed,
                    MENU_GLYPH_CELL_WIDTH,
                    MENU_GLYPH_CELL_HEIGHT,
                )?,
                occurrence_count: used.occurrence_count,
                overlays: used.overlays.clone(),
                exact_dialogue_pixel_text: glyph.exact_dialogue_pixel_text.clone(),
            })
        })
        .collect()
}

fn analyze_migration(
    writes: &[CandidateWriteInput],
    source_references: &[SourceCodeReference],
    labels: &[MenuAtlasLabel],
) -> Result<MigrationAnalysis> {
    for write in writes {
        ensure_cell_in_atlas(write.cell)?;
        ensure!(
            write.cell.width.is_multiple_of(MENU_GLYPH_CELL_WIDTH)
                && write.cell.height.is_multiple_of(MENU_GLYPH_CELL_HEIGHT),
            "candidate write is not a multiple of the 20x20 renderer footprint"
        );
    }
    let mut candidate_mask = vec![false; MENU_ATLAS_WIDTH * MENU_ATLAS_HEIGHT];
    let candidate_declared_written_pixel_count = writes
        .iter()
        .map(|write| {
            mark_cell(&mut candidate_mask, write.cell);
            cell_area(write.cell)
        })
        .sum();
    let candidate_unique_written_pixel_count = candidate_mask.iter().filter(|&&set| set).count();
    let candidate_declared_twenty_by_twenty_area_equivalent_count = writes
        .iter()
        .map(|write| cell_area(write.cell) / TWENTY_BY_TWENTY_PIXEL_COUNT)
        .sum();

    let mut overlap_ids = BTreeMap::<&str, BTreeSet<String>>::new();
    let mut candidate_write_overlaps = Vec::new();
    for (index, left) in writes.iter().enumerate() {
        for right in &writes[index + 1..] {
            let overlapping_pixel_count = cell_intersection_area(left.cell, right.cell);
            if overlapping_pixel_count == 0 {
                continue;
            }
            overlap_ids
                .entry(&left.id)
                .or_default()
                .insert(right.id.clone());
            overlap_ids
                .entry(&right.id)
                .or_default()
                .insert(left.id.clone());
            candidate_write_overlaps.push(CandidateMenuAtlasWriteOverlap {
                left_write_id: left.id.clone(),
                right_write_id: right.id.clone(),
                overlapping_pixel_count,
            });
        }
    }

    let mut confirmed_label_mask = vec![false; MENU_ATLAS_WIDTH * MENU_ATLAS_HEIGHT];
    for label in labels
        .iter()
        .filter(|label| label.review_status == MenuAtlasLabelReviewStatus::Confirmed)
    {
        mark_cell(&mut confirmed_label_mask, label.bounds);
    }
    let confirmed_label_covered_unique_written_pixel_count = candidate_mask
        .iter()
        .zip(&confirmed_label_mask)
        .filter(|(candidate, confirmed)| **candidate && **confirmed)
        .count();
    let unclassified_unique_written_pixel_count =
        candidate_unique_written_pixel_count - confirmed_label_covered_unique_written_pixel_count;

    let mut source_code_migration_obligations = Vec::new();
    let mut source_consumer_overlays = BTreeSet::new();
    let mut exact_dialogue_texts = BTreeSet::new();
    let mut source_code_occurrence_migration_obligation_count = 0;
    for reference in source_references {
        let overwritten_pixel_count = reference
            .footprint_fragments
            .iter()
            .map(|&fragment| count_mask_pixels_in_cell(&candidate_mask, fragment))
            .sum::<usize>();
        if overwritten_pixel_count == 0 {
            continue;
        }
        let candidate_write_ids = writes
            .iter()
            .filter(|write| {
                reference
                    .footprint_fragments
                    .iter()
                    .any(|&fragment| cells_overlap(write.cell, fragment))
            })
            .map(|write| write.id.clone())
            .collect();
        source_code_occurrence_migration_obligation_count += reference.occurrence_count;
        source_consumer_overlays.extend(reference.overlays.iter().cloned());
        if let Some(text) = &reference.exact_dialogue_pixel_text {
            exact_dialogue_texts.insert(text.clone());
        }
        let footprint_pixel_count = reference
            .footprint_fragments
            .iter()
            .map(|&fragment| cell_area(fragment))
            .sum();
        source_code_migration_obligations.push(SourceMenuCodeMigrationObligation {
            code: reference.code.clone(),
            footprint_fragments: reference.footprint_fragments.clone(),
            footprint_pixel_count,
            overwritten_pixel_count,
            fully_overwritten: overwritten_pixel_count == footprint_pixel_count,
            occurrence_count: reference.occurrence_count,
            overlays: reference.overlays.clone(),
            exact_dialogue_pixel_text: reference.exact_dialogue_pixel_text.clone(),
            candidate_write_ids,
        });
    }

    let candidate_writes = writes
        .iter()
        .map(|write| {
            let overlapping_references = source_references
                .iter()
                .filter(|reference| {
                    reference
                        .footprint_fragments
                        .iter()
                        .any(|&fragment| cells_overlap(write.cell, fragment))
                })
                .collect::<Vec<_>>();
            let source_code_occurrence_count = overlapping_references
                .iter()
                .map(|reference| reference.occurrence_count)
                .sum();
            let source_consumer_overlays = overlapping_references
                .iter()
                .flat_map(|reference| reference.overlays.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let exact_dialogue_pixel_texts = overlapping_references
                .iter()
                .filter_map(|reference| reference.exact_dialogue_pixel_text.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let overlapping_labels = labels
                .iter()
                .filter(|label| cells_overlap(write.cell, label.bounds))
                .collect::<Vec<_>>();
            let overlapping_label_ids = overlapping_labels
                .iter()
                .map(|label| label.id.clone())
                .collect();
            let confirmed_label_ids = overlapping_labels
                .iter()
                .filter(|label| label.review_status == MenuAtlasLabelReviewStatus::Confirmed)
                .map(|label| label.id.clone())
                .collect();
            let confirmed_label_covered_pixel_count =
                count_mask_pixels_in_cell(&confirmed_label_mask, write.cell);
            Ok(CandidateMenuAtlasWrite {
                id: write.id.clone(),
                component: write.component,
                character: write.character,
                code: write.code.clone(),
                cell: write.cell,
                written_pixel_count: cell_area(write.cell),
                twenty_by_twenty_area_equivalent_count: cell_area(write.cell)
                    / TWENTY_BY_TWENTY_PIXEL_COUNT,
                overlapping_candidate_write_ids: overlap_ids
                    .get(write.id.as_str())
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                source_code_obligation_count: overlapping_references.len(),
                source_code_occurrence_count,
                source_consumer_overlays,
                exact_dialogue_pixel_texts,
                overlapping_label_ids,
                confirmed_label_ids,
                confirmed_label_covered_pixel_count,
                unclassified_written_pixel_count: cell_area(write.cell)
                    - confirmed_label_covered_pixel_count,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(MigrationAnalysis {
        candidate_declared_written_pixel_count,
        candidate_unique_written_pixel_count,
        candidate_declared_twenty_by_twenty_area_equivalent_count,
        candidate_write_overlaps,
        source_code_occurrence_migration_obligation_count,
        source_consumer_overlays: source_consumer_overlays.into_iter().collect(),
        exact_dialogue_texts: exact_dialogue_texts.into_iter().collect(),
        confirmed_label_covered_unique_written_pixel_count,
        unclassified_unique_written_pixel_count,
        candidate_writes,
        source_code_migration_obligations,
    })
}

fn build_migration_overlay_preview(
    mut image: RgbaImage,
    writes: &[CandidateWriteInput],
    labels: &[MenuAtlasLabel],
) -> RgbaImage {
    for write in writes {
        tint_cell(&mut image, write.cell, write.component.preview_color_rgb());
        outline_cell(&mut image, write.cell, write.component.preview_color_rgb());
    }
    for label in labels {
        let color = match label.review_status {
            MenuAtlasLabelReviewStatus::Confirmed => [64, 255, 96],
            MenuAtlasLabelReviewStatus::NeedsReview => [64, 224, 255],
            MenuAtlasLabelReviewStatus::Unreviewed => [96, 128, 255],
        };
        outline_cell(&mut image, label.bounds, color);
    }
    image
}

fn tint_cell(image: &mut RgbaImage, cell: Cell, color: [u8; 3]) {
    const OVERLAY_WEIGHT: u16 = 96;
    const SOURCE_WEIGHT: u16 = 255 - OVERLAY_WEIGHT;
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            let offset = (y * image.width + x) * 4;
            for (source_channel, overlay_channel) in
                image.pixels[offset..offset + 3].iter_mut().zip(color)
            {
                *source_channel = ((u16::from(*source_channel) * SOURCE_WEIGHT
                    + u16::from(overlay_channel) * OVERLAY_WEIGHT)
                    / 255) as u8;
            }
            image.pixels[offset + 3] = 255;
        }
    }
}

fn outline_cell(image: &mut RgbaImage, cell: Cell, color: [u8; 3]) {
    let bottom = cell.y + cell.height - 1;
    let right = cell.x + cell.width - 1;
    for x in cell.x..=right {
        set_preview_pixel(image, x, cell.y, color);
        set_preview_pixel(image, x, bottom, color);
    }
    for y in cell.y..=bottom {
        set_preview_pixel(image, cell.x, y, color);
        set_preview_pixel(image, right, y, color);
    }
}

fn set_preview_pixel(image: &mut RgbaImage, x: usize, y: usize, color: [u8; 3]) {
    let offset = (y * image.width + x) * 4;
    image.pixels[offset..offset + 3].copy_from_slice(&color);
    image.pixels[offset + 3] = 255;
}

fn migration_preview_path(output: &Path) -> Result<PathBuf> {
    let stem = output
        .file_stem()
        .context("menu atlas migration output must name a JSON file")?
        .to_string_lossy();
    Ok(output.with_file_name(format!("{stem}-overlay.png")))
}

fn require_replace_permission(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "menu atlas migration output exists; pass --force to replace it: {}",
            path.display()
        );
    }
    Ok(())
}

fn ensure_cell_in_atlas(cell: Cell) -> Result<()> {
    ensure!(cell.width > 0 && cell.height > 0, "empty menu atlas cell");
    ensure!(
        cell.x
            .checked_add(cell.width)
            .is_some_and(|end| end <= MENU_ATLAS_WIDTH)
            && cell
                .y
                .checked_add(cell.height)
                .is_some_and(|end| end <= MENU_ATLAS_HEIGHT),
        "menu atlas cell is out of bounds"
    );
    Ok(())
}

fn mark_cell(mask: &mut [bool], cell: Cell) {
    for y in cell.y..cell.y + cell.height {
        let start = y * MENU_ATLAS_WIDTH + cell.x;
        mask[start..start + cell.width].fill(true);
    }
}

fn count_mask_pixels_in_cell(mask: &[bool], cell: Cell) -> usize {
    (cell.y..cell.y + cell.height)
        .map(|y| {
            let start = y * MENU_ATLAS_WIDTH + cell.x;
            mask[start..start + cell.width]
                .iter()
                .filter(|&&set| set)
                .count()
        })
        .sum()
}

fn cell_intersection_area(left: Cell, right: Cell) -> usize {
    let width = (left.x + left.width)
        .min(right.x + right.width)
        .saturating_sub(left.x.max(right.x));
    let height = (left.y + left.height)
        .min(right.y + right.height)
        .saturating_sub(left.y.max(right.y));
    width * height
}

const fn cell_area(cell: Cell) -> usize {
    cell.width * cell.height
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn canonical_path_string(path: &Path) -> Result<String> {
    Ok(path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?
        .display()
        .to_string())
}

#[cfg(test)]
#[path = "menu_atlas_migration_tests.rs"]
mod tests;
