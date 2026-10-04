use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

#[path = "menu_atlas_labeling/model.rs"]
mod model;

pub use model::{
    MenuAtlasLabel, MenuAtlasLabelDocument, MenuAtlasLabelKind, MenuAtlasLabelReviewStatus,
    MenuAtlasLabelingConfig, MenuAtlasLabelingReport,
};

use crate::menu_atlas::{
    MENU_ATLAS_RECORD_PATH, MENU_GLYPH_CELL_HEIGHT, MENU_GLYPH_CELL_WIDTH, load_source_menu_atlas,
    logical_code_fragments, logical_code_position, parse_menu_code,
};
use crate::menu_audit::collect_menu_code_audit;
use crate::menu_glyph_audit::collect_menu_glyph_audit;
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{IndexedImage, decode_4bpp_rgba_in_prefix};
use crate::tim_preview::write_tim_preview;

use model::{
    MenuAtlasLabelEvidence, MenuAtlasLogicalCodeMap, MenuAtlasLogicalCodeReference,
    MenuAtlasMigrationReviewContext,
};

const DOCUMENT_KIND: &str = "justice_gakuen2_menu_atlas_labels";
const OUTPUT_MARKER_FILE: &str = ".menu-atlas-labeling-output";
const OUTPUT_MARKER_TEXT: &str = "justice_gakuen2_menu_atlas_labeling\n";
const ATLAS_PREVIEW_FILE: &str = "menu-atlas.png";
const LABEL_DOCUMENT_FILE: &str = "labels.json";
const LOGICAL_CODE_MAP_FILE: &str = "logical-code-map.json";
const EDITOR_FILE: &str = "index.html";
const REPORT_FILE: &str = "report.json";

pub(crate) struct MenuAtlasLabelSourceIdentity<'a> {
    pub(crate) source_bin_sha256: &'a str,
    pub(crate) menu_stored_sha256: &'a str,
    pub(crate) menu_decoded_sha256: &'a str,
    pub(crate) source_atlas_indexed_sha256: &'a str,
    pub(crate) atlas_width: usize,
    pub(crate) atlas_height: usize,
}

pub fn prepare_menu_atlas_labeling(
    config: &MenuAtlasLabelingConfig,
) -> Result<MenuAtlasLabelingReport> {
    let source = load_source_menu_atlas(&config.cue)?;
    let source_bin_sha256 = source.source_bin_sha256;
    let menu_stored_sha256 = source.menu_stored_sha256;
    let menu_decoded_sha256 = source.menu_decoded_sha256;
    let indexed = source.indexed;
    let source_atlas_indexed_sha256 = sha256_bytes(&indexed.pixels);
    let rgba = decode_4bpp_rgba_in_prefix(&source.menu_decoded, 0, 0)?;
    let glyph_audit = collect_menu_glyph_audit(&config.cue, &config.dialogue_codebook)?;
    let code_audit = collect_menu_code_audit(&config.cue, config.address_flow_state_budget)?;
    ensure!(
        glyph_audit.source_bin_sha256 == source_bin_sha256
            && code_audit.source_bin_sha256 == source_bin_sha256,
        "menu atlas labeling inputs do not share one source BIN"
    );

    let labels = if let Some(path) = &config.labels {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read menu atlas labels {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse menu atlas labels {}", path.display()))?
    } else {
        MenuAtlasLabelDocument {
            kind: DOCUMENT_KIND.to_string(),
            source_bin_sha256: source_bin_sha256.clone(),
            menu_stored_sha256: menu_stored_sha256.clone(),
            menu_decoded_sha256: menu_decoded_sha256.clone(),
            source_atlas_indexed_sha256: source_atlas_indexed_sha256.clone(),
            atlas_width: indexed.width,
            atlas_height: indexed.height,
            labels: Vec::new(),
        }
    };
    validate_menu_atlas_labels(
        &labels,
        MenuAtlasLabelSourceIdentity {
            source_bin_sha256: &source_bin_sha256,
            menu_stored_sha256: &menu_stored_sha256,
            menu_decoded_sha256: &menu_decoded_sha256,
            source_atlas_indexed_sha256: &source_atlas_indexed_sha256,
            atlas_width: indexed.width,
            atlas_height: indexed.height,
        },
    )?;
    let code_map = build_logical_code_map(&glyph_audit, &code_audit)?;
    let migration_review = config
        .migration_risk
        .as_ref()
        .map(|path| load_migration_review_context(path, &labels, &code_map))
        .transpose()?;
    let migration_risk_file = config
        .migration_risk
        .as_ref()
        .map(|path| canonical_path_string(path))
        .transpose()?;
    let migration_risk_sha256 = config
        .migration_risk
        .as_ref()
        .map(|path| sha256_file(path))
        .transpose()?;
    let overlapping_label_pairs = overlapping_label_pairs(&labels.labels);
    let label_evidence = collect_label_evidence(&labels.labels, &indexed);

    prepare_output_directory(&config.output_dir, config.force)?;
    write_tim_preview(&config.output_dir.join(ATLAS_PREVIEW_FILE), &rgba)?;
    write_json(&config.output_dir.join(LABEL_DOCUMENT_FILE), &labels)?;
    write_json(&config.output_dir.join(LOGICAL_CODE_MAP_FILE), &code_map)?;

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
    let report = MenuAtlasLabelingReport {
        kind: "Justice Gakuen 2 source-bound shared MENU atlas labeling workspace".to_string(),
        source_cue: canonical_path_string(&config.cue)?,
        address_flow_state_budget: config.address_flow_state_budget,
        source_bin_sha256,
        menu_path: MENU_ATLAS_RECORD_PATH.to_string(),
        menu_stored_sha256,
        menu_decoded_sha256,
        source_atlas_indexed_sha256,
        atlas_width: indexed.width,
        atlas_height: indexed.height,
        label_count: labels.labels.len(),
        confirmed_label_count,
        needs_review_label_count,
        unreviewed_label_count,
        overlapping_label_pairs,
        label_kinds: MenuAtlasLabelKind::ALL
            .into_iter()
            .map(|kind| kind.key().to_string())
            .collect(),
        label_evidence,
        exact_dialogue_pixel_match_count: glyph_audit.exact_dialogue_pixel_match_count,
        statically_used_code_count: code_audit.used_code_count,
        proven_reclaimable_code_count: code_audit.proven_reclaimable_code_count,
        atlas_preview_file: ATLAS_PREVIEW_FILE.to_string(),
        label_document_file: LABEL_DOCUMENT_FILE.to_string(),
        logical_code_map_file: LOGICAL_CODE_MAP_FILE.to_string(),
        editor_file: EDITOR_FILE.to_string(),
        report_file: REPORT_FILE.to_string(),
        migration_risk_file,
        migration_risk_sha256,
        migration_candidate_write_count: migration_review
            .as_ref()
            .map_or(0, |review| review.candidate_writes.len()),
        migration_affected_code_count: migration_review
            .as_ref()
            .map_or(0, |review| review.source_code_migration_obligations.len()),
        local_workspace_only: true,
        limitations: vec![
            "Rectangle labels are human review decisions; code use, exact pixel matches, and rectangle overlap are evidence fields rather than automatic semantic classification.".to_string(),
            "A logical 20x20 code can wrap on both byte-sized UV axes. The code map therefore records up to four physical fragments and does not present a false non-overlapping grid.".to_string(),
            "The extracted atlas PNG and editable label workspace remain ignored local material; only reviewed source identities and label decisions should later be promoted to tracked assets.".to_string(),
            "Migration-risk rectangles and affected-code subsets are review priorities only; they do not classify a physical region or prove the remaining atlas safe.".to_string(),
        ],
    };
    write_json(&config.output_dir.join(REPORT_FILE), &report)?;
    write_editor(
        &config.output_dir.join(EDITOR_FILE),
        &labels,
        &code_map,
        migration_review.as_ref(),
    )?;
    Ok(report)
}

fn load_migration_review_context(
    path: &Path,
    labels: &MenuAtlasLabelDocument,
    code_map: &MenuAtlasLogicalCodeMap,
) -> Result<MenuAtlasMigrationReviewContext> {
    let bytes = std::fs::read(path).with_context(|| {
        format!(
            "failed to read menu atlas migration risk {}",
            path.display()
        )
    })?;
    let review: MenuAtlasMigrationReviewContext =
        serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to parse menu atlas migration risk {}",
                path.display()
            )
        })?;
    validate_migration_review_context(&review, labels, code_map)?;
    Ok(review)
}

fn validate_migration_review_context(
    review: &MenuAtlasMigrationReviewContext,
    labels: &MenuAtlasLabelDocument,
    code_map: &MenuAtlasLogicalCodeMap,
) -> Result<()> {
    ensure!(
        review.kind == "Justice Gakuen 2 source-bound shared MENU atlas migration obligation audit",
        "menu atlas migration review kind changed"
    );
    ensure!(
        review.source_bin_sha256 == labels.source_bin_sha256
            && review.menu_stored_sha256 == labels.menu_stored_sha256
            && review.menu_decoded_sha256 == labels.menu_decoded_sha256
            && review.source_atlas_indexed_sha256 == labels.source_atlas_indexed_sha256,
        "menu atlas migration review targets a different source artifact"
    );
    ensure!(
        review.address_flow_state_budget == code_map.address_flow_state_budget
            && review.statically_used_source_code_count == code_map.used_code_count,
        "menu atlas migration review and logical-code map use different static-flow evidence"
    );
    ensure!(
        review.candidate_write_count == review.candidate_writes.len()
            && review.source_code_migration_obligation_count
                == review.source_code_migration_obligations.len(),
        "menu atlas migration review count fields do not match their populations"
    );

    let references = code_map
        .references
        .iter()
        .map(|reference| (reference.code.as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let mut write_ids = BTreeSet::new();
    for write in &review.candidate_writes {
        ensure!(
            write_ids.insert(write.id.as_str()),
            "menu atlas migration review repeats candidate write {}",
            write.id
        );
        ensure!(
            matches!(
                write.component.as_str(),
                "options" | "title_menu" | "title_notice"
            ),
            "menu atlas migration review has unknown writer component {}",
            write.component
        );
        ensure_atlas_cell_bounds(write.cell, labels, &write.id)?;
        ensure!(
            references.contains_key(write.code.as_str()),
            "menu atlas migration candidate write {} has unknown logical code {}",
            write.id,
            write.code
        );
    }

    let mut obligation_codes = BTreeSet::new();
    for obligation in &review.source_code_migration_obligations {
        ensure!(
            obligation_codes.insert(obligation.code.as_str()),
            "menu atlas migration review repeats affected code {}",
            obligation.code
        );
        let reference = references.get(obligation.code.as_str()).with_context(|| {
            format!(
                "menu atlas migration review has unknown affected code {}",
                obligation.code
            )
        })?;
        ensure!(
            obligation.footprint_fragments == reference.footprint_fragments
                && obligation.occurrence_count == reference.occurrence_count
                && obligation.overlays == reference.overlays
                && obligation.exact_dialogue_pixel_text == reference.exact_dialogue_pixel_text,
            "menu atlas migration affected code {} disagrees with the logical-code map",
            obligation.code
        );
        ensure!(
            !obligation.candidate_write_ids.is_empty()
                && obligation
                    .candidate_write_ids
                    .iter()
                    .all(|id| write_ids.contains(id.as_str())),
            "menu atlas migration affected code {} references an unknown candidate write",
            obligation.code
        );
        for (index, fragment) in obligation.footprint_fragments.iter().enumerate() {
            ensure_atlas_cell_bounds(
                *fragment,
                labels,
                &format!("{} fragment {index}", obligation.code),
            )?;
        }
    }
    Ok(())
}

fn ensure_atlas_cell_bounds(
    cell: crate::tim::Cell,
    labels: &MenuAtlasLabelDocument,
    owner: &str,
) -> Result<()> {
    ensure!(
        cell.width > 0
            && cell.height > 0
            && cell
                .x
                .checked_add(cell.width)
                .is_some_and(|end| end <= labels.atlas_width)
            && cell
                .y
                .checked_add(cell.height)
                .is_some_and(|end| end <= labels.atlas_height),
        "menu atlas migration review rectangle is outside the source atlas: {owner}"
    );
    Ok(())
}

fn build_logical_code_map(
    glyph_audit: &crate::menu_glyph_audit::MenuGlyphAuditManifest,
    code_audit: &crate::menu_audit::MenuCodeAuditManifest,
) -> Result<MenuAtlasLogicalCodeMap> {
    let used = code_audit
        .used_codes
        .iter()
        .map(|entry| {
            (
                entry.code.as_str(),
                (entry.occurrence_count, entry.overlays.as_slice()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let references = glyph_audit
        .cells
        .iter()
        .map(|cell| {
            let code = parse_menu_code(&cell.code)?;
            let position = logical_code_position(code)?;
            let (occurrence_count, overlays) = used
                .get(cell.code.as_str())
                .map(|(count, overlays)| (*count, *overlays))
                .unwrap_or((0, &[]));
            Ok(MenuAtlasLogicalCodeReference {
                code: cell.code.clone(),
                page: position.page,
                column: position.column,
                row: position.row,
                physical_x: position.x,
                physical_y: position.y,
                footprint_fragments: logical_code_fragments(
                    code,
                    MENU_GLYPH_CELL_WIDTH,
                    MENU_GLYPH_CELL_HEIGHT,
                )?,
                pixel_sha256: cell.pixel_sha256.clone(),
                exact_dialogue_pixel_text: cell.exact_dialogue_pixel_text.clone(),
                occurrence_count,
                overlays: overlays.to_vec(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MenuAtlasLogicalCodeMap {
        kind: "Justice Gakuen 2 shared MENU logical-code physical-footprint map".to_string(),
        source_bin_sha256: glyph_audit.source_bin_sha256.clone(),
        menu_decoded_sha256: glyph_audit.menu_decoded_sha256.clone(),
        address_flow_state_budget: code_audit.address_flow_state_budget,
        code_count: references.len(),
        used_code_count: code_audit.used_code_count,
        exact_dialogue_pixel_match_count: glyph_audit.exact_dialogue_pixel_match_count,
        references,
        limitations: vec![
            "Static references are an observed lower bound and do not prove that occurrence-free codes are reclaimable.".to_string(),
            "Exact dialogue text is admitted only when packed source pixels match the verified dialogue codebook byte-for-byte.".to_string(),
        ],
    })
}

pub(crate) fn validate_menu_atlas_labels(
    document: &MenuAtlasLabelDocument,
    expected: MenuAtlasLabelSourceIdentity<'_>,
) -> Result<()> {
    ensure!(
        document.kind == DOCUMENT_KIND,
        "menu atlas label kind changed"
    );
    ensure!(
        document.source_bin_sha256 == expected.source_bin_sha256
            && document.menu_stored_sha256 == expected.menu_stored_sha256
            && document.menu_decoded_sha256 == expected.menu_decoded_sha256
            && document.source_atlas_indexed_sha256 == expected.source_atlas_indexed_sha256,
        "menu atlas labels target a different source artifact"
    );
    ensure!(
        document.atlas_width == expected.atlas_width
            && document.atlas_height == expected.atlas_height,
        "menu atlas label dimensions changed"
    );
    let mut ids = BTreeSet::new();
    for label in &document.labels {
        ensure!(
            valid_label_id(&label.id),
            "invalid menu atlas label id: {}",
            label.id
        );
        ensure!(
            ids.insert(label.id.as_str()),
            "duplicate menu atlas label id: {}",
            label.id
        );
        ensure!(
            label.bounds.width > 0 && label.bounds.height > 0,
            "menu atlas label {} has empty bounds",
            label.id
        );
        ensure!(
            label
                .bounds
                .x
                .checked_add(label.bounds.width)
                .is_some_and(|end| end <= expected.atlas_width)
                && label
                    .bounds
                    .y
                    .checked_add(label.bounds.height)
                    .is_some_and(|end| end <= expected.atlas_height),
            "menu atlas label {} is outside the source atlas",
            label.id
        );
        if label.review_status == MenuAtlasLabelReviewStatus::Confirmed {
            ensure!(
                label.kind != MenuAtlasLabelKind::Unknown,
                "confirmed menu atlas label {} remains unknown",
                label.id
            );
            ensure!(
                label
                    .semantic_role
                    .as_deref()
                    .is_some_and(|role| !role.trim().is_empty()),
                "confirmed menu atlas label {} has no semantic role",
                label.id
            );
        }
        let mut logical_codes = BTreeSet::new();
        for code in &label.logical_codes {
            let parsed = parse_menu_code(code)
                .with_context(|| format!("menu atlas label {} has invalid code", label.id))?;
            ensure!(
                logical_codes.insert(parsed),
                "menu atlas label {} repeats logical code {}",
                label.id,
                code
            );
        }
    }
    Ok(())
}

fn valid_label_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b'-' => index > 0,
            _ => false,
        })
}

fn collect_label_evidence(
    labels: &[MenuAtlasLabel],
    indexed: &IndexedImage,
) -> Vec<MenuAtlasLabelEvidence> {
    labels
        .iter()
        .map(|label| {
            let mut pixels = Vec::with_capacity(label.bounds.width * label.bounds.height);
            let mut palette_histogram = [0usize; 16];
            for y in label.bounds.y..label.bounds.y + label.bounds.height {
                let start = y * indexed.width + label.bounds.x;
                let row = &indexed.pixels[start..start + label.bounds.width];
                for &pixel in row {
                    palette_histogram[usize::from(pixel)] += 1;
                }
                pixels.extend_from_slice(row);
            }
            MenuAtlasLabelEvidence {
                label_id: label.id.clone(),
                bounds: label.bounds,
                indexed_pixel_sha256: sha256_bytes(&pixels),
                palette_histogram,
            }
        })
        .collect()
}

fn overlapping_label_pairs(labels: &[MenuAtlasLabel]) -> Vec<[String; 2]> {
    let mut overlaps = Vec::new();
    for (index, left) in labels.iter().enumerate() {
        for right in &labels[index + 1..] {
            if crate::tim::cells_overlap(left.bounds, right.bounds) {
                overlaps.push([left.id.clone(), right.id.clone()]);
            }
        }
    }
    overlaps
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    ensure_safe_output_path(output_dir)?;
    let marker = output_dir.join(OUTPUT_MARKER_FILE);
    if output_dir.exists() {
        if !force {
            bail!(
                "menu atlas labeling output exists; pass --force to replace it: {}",
                output_dir.display()
            );
        }
        let marker_text = std::fs::read_to_string(&marker).with_context(|| {
            format!(
                "refusing to replace an unowned output directory without {}: {}",
                OUTPUT_MARKER_FILE,
                output_dir.display()
            )
        })?;
        ensure!(
            marker_text == OUTPUT_MARKER_TEXT,
            "refusing to replace menu atlas labeling output with an unknown marker: {}",
            output_dir.display()
        );
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    std::fs::write(output_dir.join(OUTPUT_MARKER_FILE), OUTPUT_MARKER_TEXT)?;
    Ok(())
}

fn ensure_safe_output_path(output_dir: &Path) -> Result<()> {
    ensure!(
        !output_dir.as_os_str().is_empty()
            && output_dir != Path::new(".")
            && output_dir != Path::new("..")
            && output_dir.file_name().is_some(),
        "menu atlas labeling output must name a dedicated directory"
    );
    Ok(())
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn write_editor(
    path: &Path,
    labels: &MenuAtlasLabelDocument,
    code_map: &MenuAtlasLogicalCodeMap,
    migration_review: Option<&MenuAtlasMigrationReviewContext>,
) -> Result<()> {
    let labels_json = script_json(labels)?;
    let code_map_json = script_json(code_map)?;
    let migration_review_json = script_json(&migration_review)?;
    let template = include_str!("menu_atlas_labeling/editor.html");
    ensure!(
        template.matches("__LABEL_DOCUMENT_JSON__").count() == 1
            && template.matches("__LOGICAL_CODE_MAP_JSON__").count() == 1
            && template.matches("__MIGRATION_REVIEW_JSON__").count() == 1,
        "menu atlas editor template placeholders changed"
    );
    let html = template
        .replace("__LABEL_DOCUMENT_JSON__", &labels_json)
        .replace("__LOGICAL_CODE_MAP_JSON__", &code_map_json)
        .replace("__MIGRATION_REVIEW_JSON__", &migration_review_json);
    std::fs::write(path, html).with_context(|| format!("failed to write {}", path.display()))
}

fn script_json(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_string(value)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e"))
}

fn canonical_path_string(path: &Path) -> Result<String> {
    Ok(path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?
        .display()
        .to_string())
}

#[cfg(test)]
#[path = "menu_atlas_labeling_tests.rs"]
mod tests;
