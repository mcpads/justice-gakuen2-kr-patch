use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

#[path = "disc_asset_comparison/html.rs"]
mod html;
#[path = "disc_asset_comparison/model.rs"]
mod model;

pub use model::{
    DiscAssetComparisonConfig, DiscAssetComparisonReport, FixedPresentationGlyphSourceAuditConfig,
    FixedPresentationGlyphSourceAuditReport,
};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::RawTrack;
use crate::disc::iso9660::{FileRecord, Iso9660};
use crate::embedded_tim::{EmbeddedTimAudit, decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::tim::{
    Cell, RgbaImage, parse_4bpp_prefix, read_4bpp_indexed_image_in_prefix,
    read_4bpp_palette_words_in_prefix, read_8bpp_indexed_cell_in_prefix,
    read_8bpp_palette_words_in_prefix, read_indexed_cell_without_clut_in_prefix,
};
use crate::tim_preview::write_tim_preview;
use crate::tzz::parse_tzz;

use model::{
    ByteDifferenceSummary, ComparisonSide, ComparisonStatus, DecodeIssue, DecodedLayerComparison,
    DecodedLayerIdentity, DiscRecordComparison, DiscRecordIdentity, EmbeddedTimComparison,
    EmbeddedTimIdentity, FixedPresentationGlyphAtlasFamilyAudit,
    FixedPresentationGlyphAtlasSideAudit, FixedPresentationGlyphAtlasSurfaceAudit,
    FixedPresentationGlyphRuntimeUse, FixedPresentationGlyphSourceMatch,
    FixedPresentationGlyphTargetAudit, FixedPresentationRuntimeDrawSourceMatch,
    FixedPresentationRuntimeResidentSurface, FixedPresentationRuntimeSpriteCellAudit,
    FixedPresentationRuntimeTextureSourceMatch,
};

const OUTPUT_MARKER_FILE: &str = ".disc-asset-comparison-output";
const OUTPUT_MARKER_TEXT: &str = "justice_gakuen2_disc_asset_comparison\n";
const REPORT_FILE: &str = "report.json";
const HTML_FILE: &str = "index.html";
const MAX_REPORTED_DIFFERENCE_RANGES: usize = 64;
const FIXED_PRESENTATION_OUTPUT_MARKER_FILE: &str = ".fixed-presentation-glyph-source-audit-output";
const FIXED_PRESENTATION_OUTPUT_MARKER_TEXT: &str =
    "justice_gakuen2_fixed_presentation_glyph_source_audit\n";
const FIXED_PRESENTATION_REPORT_FILE: &str = "report.json";
const FIXED_PRESENTATION_GLYPH_WIDTH: usize = 20;
const FIXED_PRESENTATION_GLYPH_HEIGHT: usize = 20;
const PSX_VRAM_WIDTH_WORDS: usize = 1024;
const PSX_VRAM_HEIGHT: usize = 512;
const PSX_VRAM_WORD_BYTES: usize = 2;
const PSX_VRAM_BYTE_COUNT: usize = PSX_VRAM_WIDTH_WORDS * PSX_VRAM_HEIGHT * PSX_VRAM_WORD_BYTES;
const PSX_RAM_BYTE_COUNT: usize = 2 * 1024 * 1024;
const PSX_RAM_RUNTIME_BASE: u32 = 0x8000_0000;
const SPRT_PACKET_BYTE_COUNT: usize = 20;
const DR_TPAGE_PACKET_BYTE_COUNT: usize = 8;
const SPRT_COMMAND: u8 = 0x64;
const DR_TPAGE_COMMAND: u8 = 0xe1;

struct RecordBytes {
    record: FileRecord,
    bytes: Vec<u8>,
}

struct DecodedRecord {
    layers: BTreeMap<String, DecodedLayer>,
    issues: Vec<DecodeIssue>,
}

#[derive(Clone)]
struct DecodedLayer {
    id: String,
    kind: String,
    stored_offset: usize,
    stored_byte_count: usize,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct IndexedPixels {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

struct EmbeddedTimComparisonSet {
    source_count: usize,
    patched_count: usize,
    comparison_count: usize,
    unchanged_count: usize,
    changed: Vec<EmbeddedTimComparison>,
}

#[derive(Debug, Deserialize)]
struct RuntimeGlyphAuditInput {
    draw_consumers: Vec<RuntimeGlyphDrawConsumerInput>,
}

#[derive(Debug, Deserialize)]
struct RuntimeGlyphDrawConsumerInput {
    screen_x: i32,
    screen_y: i32,
    sprite_width_pixels: usize,
    sprite_height_pixels: usize,
    texture_vram_word_x: usize,
    texture_vram_y: usize,
    pixel_sha256: String,
    source_glyphs: Vec<RuntimeSourceGlyphInput>,
}

#[derive(Debug, Deserialize)]
struct RuntimeSourceGlyphInput {
    source_code: String,
    source_text: Option<String>,
    source_semantic_id: Option<String>,
    source_text_uses_japanese_script: bool,
}

#[derive(Debug, Deserialize)]
struct DialogueAllocationInput {
    assets: Vec<DialogueAllocationAssetInput>,
}

#[derive(Debug, Deserialize)]
struct DialogueAllocationAssetInput {
    source_path: String,
}

#[derive(Debug, Deserialize)]
struct PresentationCodebookInput {
    entries: Vec<PresentationCodebookEntryInput>,
}

#[derive(Debug, Deserialize)]
struct PresentationCodebookEntryInput {
    pixel_sha256: String,
    meaning: PresentationCodebookMeaningInput,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PresentationCodebookMeaningInput {
    Character {
        text: String,
    },
    Symbol {
        #[serde(rename = "id")]
        _id: String,
        #[serde(rename = "display")]
        _display: String,
    },
}

#[derive(Debug, Clone)]
struct FixedPresentationTarget {
    pixel_sha256: String,
    pixels: Vec<u8>,
    source_codes: BTreeSet<String>,
    source_texts: BTreeSet<String>,
    source_semantic_ids: BTreeSet<String>,
    runtime_uses: BTreeSet<(i32, i32, usize, usize, usize, usize)>,
}

struct FixedPresentationDiscScan {
    iso_file_count: usize,
    decoded_layer_count: usize,
    embedded_tim_count: usize,
    decode_issues: Vec<DecodeIssue>,
    matches_by_digest: BTreeMap<String, Vec<FixedPresentationGlyphSourceMatch>>,
    runtime_resident_surfaces: Vec<FixedPresentationRuntimeResidentSurface>,
    runtime_draw_source_matches: Vec<FixedPresentationRuntimeDrawSourceMatch>,
    runtime_texture_source_matches: Vec<FixedPresentationRuntimeTextureSourceMatch>,
}

struct FixedPresentationDiscScanInput<'a> {
    cue_path: &'a Path,
    side_label: &'a str,
    side: ComparisonSide,
    targets: &'a [FixedPresentationTarget],
    allocation_paths: &'a BTreeSet<String>,
    runtime_gpu_dump: Option<&'a [u8]>,
    runtime_sprite_packets: &'a [RuntimeFourBitSpritePacket],
    runtime_texture_targets: &'a [RuntimeFourBitTextureTarget],
    output_dir: &'a Path,
}

#[derive(Debug, Clone)]
struct RuntimeFourBitSpritePacket {
    packet_address: u32,
    screen_x: i32,
    screen_y: i32,
    width: usize,
    height: usize,
    texture_page_word_x: usize,
    texture_page_y: usize,
    texture_u: usize,
    texture_v: usize,
    clut: u16,
    draw_mode: u32,
}

#[derive(Debug, Clone)]
struct RuntimeFourBitTextureTarget {
    pixel_sha256: String,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    packets: Vec<RuntimeFourBitSpritePacket>,
}

#[derive(Default)]
struct FixedPresentationGlyphAtlasSurfaceAccumulator {
    source: Option<FixedPresentationGlyphAtlasSideAccumulator>,
    patched: Option<FixedPresentationGlyphAtlasSideAccumulator>,
}

#[derive(Default)]
struct FixedPresentationGlyphAtlasSideAccumulator {
    representative: Option<FixedPresentationGlyphSourceMatch>,
    match_count: usize,
    target_pixel_sha256s: BTreeSet<String>,
    runtime_observed_target_pixel_sha256s: BTreeSet<String>,
    source_texts: BTreeSet<String>,
}

impl FixedPresentationGlyphAtlasSideAccumulator {
    fn add_match(
        &mut self,
        target: &FixedPresentationGlyphTargetAudit,
        matched: &FixedPresentationGlyphSourceMatch,
    ) -> Result<()> {
        if let Some(representative) = &self.representative {
            ensure!(
                fixed_presentation_matches_share_atlas_identity(representative, matched),
                "fixed-presentation matches disagree about one atlas surface: {}#{} TIM 0x{:x}",
                matched.record_path,
                matched.decoded_layer_id,
                matched.tim_offset
            );
        } else {
            self.representative = Some(matched.clone());
        }
        self.match_count += 1;
        self.target_pixel_sha256s
            .insert(target.pixel_sha256.clone());
        if !target.runtime_uses.is_empty() {
            self.runtime_observed_target_pixel_sha256s
                .insert(target.pixel_sha256.clone());
        }
        self.source_texts
            .extend(target.source_texts.iter().cloned());
        Ok(())
    }

    fn finish(self) -> Result<FixedPresentationGlyphAtlasSideAudit> {
        let representative = self
            .representative
            .context("fixed-presentation atlas side has no representative match")?;
        Ok(FixedPresentationGlyphAtlasSideAudit {
            decoded_layer_sha256: representative.decoded_layer_sha256,
            tim_sha256: representative.tim_sha256,
            tim_bits_per_pixel: representative.tim_bits_per_pixel,
            tim_pixel_width: representative.tim_pixel_width,
            tim_pixel_height: representative.tim_pixel_height,
            tim_image_vram_word_x: representative.tim_image_vram_word_x,
            tim_image_vram_y: representative.tim_image_vram_y,
            match_count: self.match_count,
            target_count: self.target_pixel_sha256s.len(),
            runtime_observed_target_count: self.runtime_observed_target_pixel_sha256s.len(),
            target_pixel_sha256s: self.target_pixel_sha256s.into_iter().collect(),
            runtime_observed_target_pixel_sha256s: self
                .runtime_observed_target_pixel_sha256s
                .into_iter()
                .collect(),
            source_texts: self.source_texts.into_iter().collect(),
            preview_file: representative.preview_file,
            preview_sha256: representative.preview_sha256,
        })
    }
}

struct FixedPresentationTargetIndex {
    anchors: HashMap<[u8; FIXED_PRESENTATION_GLYPH_WIDTH], Vec<(usize, usize)>>,
}

impl FixedPresentationTargetIndex {
    fn new(targets: &[FixedPresentationTarget]) -> Self {
        let mut anchors =
            HashMap::<[u8; FIXED_PRESENTATION_GLYPH_WIDTH], Vec<(usize, usize)>>::new();
        for (target_index, target) in targets.iter().enumerate() {
            let anchor_row = (0..FIXED_PRESENTATION_GLYPH_HEIGHT)
                .max_by_key(|row| {
                    target.pixels[row * FIXED_PRESENTATION_GLYPH_WIDTH
                        ..(row + 1) * FIXED_PRESENTATION_GLYPH_WIDTH]
                        .iter()
                        .filter(|pixel| **pixel != 0)
                        .count()
                })
                .unwrap_or(0);
            let mut anchor = [0u8; FIXED_PRESENTATION_GLYPH_WIDTH];
            anchor.copy_from_slice(
                &target.pixels[anchor_row * FIXED_PRESENTATION_GLYPH_WIDTH
                    ..(anchor_row + 1) * FIXED_PRESENTATION_GLYPH_WIDTH],
            );
            anchors
                .entry(anchor)
                .or_default()
                .push((target_index, anchor_row));
        }
        Self { anchors }
    }

    fn find(
        &self,
        image: &IndexedPixels,
        targets: &[FixedPresentationTarget],
    ) -> Vec<(usize, usize, usize)> {
        if image.width < FIXED_PRESENTATION_GLYPH_WIDTH
            || image.height < FIXED_PRESENTATION_GLYPH_HEIGHT
        {
            return Vec::new();
        }
        let mut matches = Vec::new();
        for image_row in 0..image.height {
            let row_start = image_row * image.width;
            for pixel_x in 0..=image.width - FIXED_PRESENTATION_GLYPH_WIDTH {
                let mut anchor = [0u8; FIXED_PRESENTATION_GLYPH_WIDTH];
                anchor.copy_from_slice(
                    &image.pixels
                        [row_start + pixel_x..row_start + pixel_x + FIXED_PRESENTATION_GLYPH_WIDTH],
                );
                let Some(candidates) = self.anchors.get(&anchor) else {
                    continue;
                };
                for &(target_index, anchor_row) in candidates {
                    let Some(pixel_y) = image_row.checked_sub(anchor_row) else {
                        continue;
                    };
                    if pixel_y + FIXED_PRESENTATION_GLYPH_HEIGHT > image.height {
                        continue;
                    }
                    let target = &targets[target_index];
                    let exact = (0..FIXED_PRESENTATION_GLYPH_HEIGHT).all(|row| {
                        let image_start = (pixel_y + row) * image.width + pixel_x;
                        let target_start = row * FIXED_PRESENTATION_GLYPH_WIDTH;
                        image.pixels[image_start..image_start + FIXED_PRESENTATION_GLYPH_WIDTH]
                            == target.pixels
                                [target_start..target_start + FIXED_PRESENTATION_GLYPH_WIDTH]
                    });
                    if exact {
                        matches.push((target_index, pixel_x, pixel_y));
                    }
                }
            }
        }
        matches.sort_unstable();
        matches
    }
}

pub fn audit_fixed_presentation_glyph_sources(
    config: &FixedPresentationGlyphSourceAuditConfig,
) -> Result<FixedPresentationGlyphSourceAuditReport> {
    let source_cue = CueSheet::parse(&config.source_cue)?;
    let patched_cue = CueSheet::parse(&config.patched_cue)?;
    let source_bin_sha256 = sha256_file(&source_cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let patched_bin_sha256 = sha256_file(&patched_cue.image_path)?;
    let source_decoded = std::fs::read(&config.source_decoded)
        .with_context(|| format!("failed to read {}", config.source_decoded.display()))?;
    let runtime_report_bytes = std::fs::read(&config.runtime_report)
        .with_context(|| format!("failed to read {}", config.runtime_report.display()))?;
    let runtime_report: RuntimeGlyphAuditInput = serde_json::from_slice(&runtime_report_bytes)
        .with_context(|| format!("failed to parse {}", config.runtime_report.display()))?;
    let runtime_gpu_dump = config
        .runtime_gpu_dump
        .as_ref()
        .map(|path| {
            let bytes = std::fs::read(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            ensure!(
                bytes.len() == PSX_VRAM_BYTE_COUNT,
                "runtime GPU dump must be an exact 1024-word by 512-row PS1 VRAM image"
            );
            Ok(bytes)
        })
        .transpose()?;
    let runtime_ram_dump = config
        .runtime_ram_dump
        .as_ref()
        .map(|path| {
            let bytes = std::fs::read(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            ensure!(
                bytes.len() == PSX_RAM_BYTE_COUNT,
                "runtime RAM dump must be an exact 2 MiB PS1 RAM image"
            );
            Ok(bytes)
        })
        .transpose()?;
    ensure!(
        runtime_ram_dump.is_none() || runtime_gpu_dump.is_some(),
        "runtime RAM draw binding also requires the paired GPU dump"
    );
    ensure!(
        config.runtime_texture_source_min_pixel_count.is_none()
            || (runtime_ram_dump.is_some() && runtime_gpu_dump.is_some()),
        "runtime texture source matching requires paired RAM and GPU dumps"
    );
    ensure!(
        config.runtime_texture_source_min_pixel_count != Some(0),
        "runtime texture source minimum pixel count must be positive"
    );
    let runtime_sprite_packets = runtime_ram_dump
        .as_deref()
        .map(scan_runtime_four_bit_sprite_packets)
        .transpose()?
        .unwrap_or_default();
    let runtime_sprite_cells = runtime_gpu_dump
        .as_deref()
        .map(|gpu_dump| runtime_sprite_cells(&runtime_sprite_packets, gpu_dump))
        .transpose()?
        .unwrap_or_default();
    let runtime_texture_targets = match (
        runtime_gpu_dump.as_deref(),
        config.runtime_texture_source_min_pixel_count,
    ) {
        (Some(gpu_dump), Some(min_pixel_count)) => {
            runtime_four_bit_texture_targets(&runtime_sprite_packets, gpu_dump, min_pixel_count)?
        }
        _ => Vec::new(),
    };
    let codebook_bytes = std::fs::read(&config.codebook)
        .with_context(|| format!("failed to read {}", config.codebook.display()))?;
    let codebook: PresentationCodebookInput = serde_json::from_slice(&codebook_bytes)
        .with_context(|| format!("failed to parse {}", config.codebook.display()))?;
    let allocation_bytes = std::fs::read(&config.allocation)
        .with_context(|| format!("failed to read {}", config.allocation.display()))?;
    let allocation: DialogueAllocationInput = serde_json::from_slice(&allocation_bytes)
        .with_context(|| format!("failed to parse {}", config.allocation.display()))?;
    let allocation_paths = allocation
        .assets
        .into_iter()
        .map(|asset| asset.source_path)
        .collect::<BTreeSet<_>>();
    let targets = fixed_presentation_targets(&source_decoded, &runtime_report, &codebook)?;
    ensure!(
        !targets.is_empty(),
        "source dialogue image and runtime report have no auditable targets"
    );
    let japanese_script_target_count = targets
        .iter()
        .filter(|target| {
            target
                .source_texts
                .iter()
                .any(|text| uses_japanese_script(text))
        })
        .count();
    let runtime_observed_target_count = targets
        .iter()
        .filter(|target| !target.runtime_uses.is_empty())
        .count();

    prepare_fixed_presentation_output_directory(&config.output_dir, config.force)?;
    let source_scan = scan_fixed_presentation_disc(FixedPresentationDiscScanInput {
        cue_path: &config.source_cue,
        side_label: "source",
        side: ComparisonSide::Source,
        targets: &targets,
        allocation_paths: &allocation_paths,
        runtime_gpu_dump: runtime_gpu_dump.as_deref(),
        runtime_sprite_packets: &runtime_sprite_packets,
        runtime_texture_targets: &runtime_texture_targets,
        output_dir: &config.output_dir,
    })?;
    let patched_scan = scan_fixed_presentation_disc(FixedPresentationDiscScanInput {
        cue_path: &config.patched_cue,
        side_label: "patched",
        side: ComparisonSide::Patched,
        targets: &targets,
        allocation_paths: &allocation_paths,
        runtime_gpu_dump: runtime_gpu_dump.as_deref(),
        runtime_sprite_packets: &runtime_sprite_packets,
        runtime_texture_targets: &runtime_texture_targets,
        output_dir: &config.output_dir,
    })?;

    let mut target_reports = Vec::with_capacity(targets.len());
    for target in targets {
        let source_matches = source_scan
            .matches_by_digest
            .get(&target.pixel_sha256)
            .cloned()
            .unwrap_or_default();
        let patched_matches = patched_scan
            .matches_by_digest
            .get(&target.pixel_sha256)
            .cloned()
            .unwrap_or_default();
        let source_candidate_locations = source_matches
            .iter()
            .filter(|matched| matched.source_class == "presentation_candidate")
            .map(fixed_presentation_match_location)
            .collect::<BTreeSet<_>>();
        let patched_candidate_locations = patched_matches
            .iter()
            .filter(|matched| matched.source_class == "presentation_candidate")
            .map(fixed_presentation_match_location)
            .collect::<BTreeSet<_>>();
        let unchanged_presentation_candidate_locations = source_candidate_locations
            .intersection(&patched_candidate_locations)
            .cloned()
            .collect::<Vec<_>>();
        let source_presentation_candidate_match_count = source_candidate_locations.len();
        let patched_presentation_candidate_match_count = patched_candidate_locations.len();
        let runtime_uses = target
            .runtime_uses
            .into_iter()
            .map(
                |(
                    screen_x,
                    screen_y,
                    sprite_width_pixels,
                    sprite_height_pixels,
                    texture_vram_word_x,
                    texture_vram_y,
                )| FixedPresentationGlyphRuntimeUse {
                    screen_x,
                    screen_y,
                    sprite_width_pixels,
                    sprite_height_pixels,
                    texture_vram_word_x,
                    texture_vram_y,
                },
            )
            .collect();
        target_reports.push(FixedPresentationGlyphTargetAudit {
            pixel_sha256: target.pixel_sha256,
            source_codes: target.source_codes.into_iter().collect(),
            source_texts: target.source_texts.into_iter().collect(),
            source_semantic_ids: target.source_semantic_ids.into_iter().collect(),
            runtime_uses,
            source_matches,
            patched_matches,
            source_presentation_candidate_match_count,
            patched_presentation_candidate_match_count,
            unchanged_presentation_candidate_locations,
        });
    }
    let source_match_count = target_reports
        .iter()
        .map(|target| target.source_matches.len())
        .sum();
    let patched_match_count = target_reports
        .iter()
        .map(|target| target.patched_matches.len())
        .sum();
    let source_presentation_candidate_match_count = target_reports
        .iter()
        .map(|target| target.source_presentation_candidate_match_count)
        .sum();
    let patched_presentation_candidate_match_count = target_reports
        .iter()
        .map(|target| target.patched_presentation_candidate_match_count)
        .sum();
    let presentation_candidate_atlas_families = fixed_presentation_atlas_families(&target_reports)?;
    let source_presentation_candidate_surface_count = presentation_candidate_atlas_families
        .iter()
        .map(|family| family.source_surface_count)
        .sum();
    let patched_presentation_candidate_surface_count = presentation_candidate_atlas_families
        .iter()
        .map(|family| family.patched_surface_count)
        .sum();
    let source_presentation_candidate_atlas_family_count = presentation_candidate_atlas_families
        .iter()
        .filter(|family| family.source_tim_sha256.is_some())
        .count();
    let duplicated_source_presentation_candidate_atlas_family_count =
        presentation_candidate_atlas_families
            .iter()
            .filter(|family| family.source_surface_count > 1)
            .count();
    let report = FixedPresentationGlyphSourceAuditReport {
        kind: "justice_gakuen2_fixed_presentation_glyph_source_audit".to_string(),
        source_cue: canonical_path_string(&config.source_cue)?,
        source_bin_sha256,
        patched_cue: canonical_path_string(&config.patched_cue)?,
        patched_bin_sha256,
        source_decoded_sha256: sha256_file(&config.source_decoded)?,
        codebook_sha256: sha256_file(&config.codebook)?,
        allocation_sha256: sha256_file(&config.allocation)?,
        runtime_report_sha256: sha256_file(&config.runtime_report)?,
        runtime_gpu_dump_sha256: config
            .runtime_gpu_dump
            .as_ref()
            .map(|path| sha256_file(path))
            .transpose()?,
        runtime_ram_dump_sha256: config
            .runtime_ram_dump
            .as_ref()
            .map(|path| sha256_file(path))
            .transpose()?,
        runtime_texture_source_min_pixel_count: config.runtime_texture_source_min_pixel_count,
        runtime_texture_target_count: runtime_texture_targets.len(),
        runtime_sprite_cell_count: runtime_sprite_cells.len(),
        runtime_sprite_cells,
        target_count: target_reports.len(),
        japanese_script_target_count,
        runtime_observed_target_count,
        source_iso_file_count: source_scan.iso_file_count,
        patched_iso_file_count: patched_scan.iso_file_count,
        source_decoded_layer_count: source_scan.decoded_layer_count,
        patched_decoded_layer_count: patched_scan.decoded_layer_count,
        source_embedded_tim_count: source_scan.embedded_tim_count,
        patched_embedded_tim_count: patched_scan.embedded_tim_count,
        source_decode_issue_count: source_scan.decode_issues.len(),
        patched_decode_issue_count: patched_scan.decode_issues.len(),
        source_decode_issues: source_scan.decode_issues,
        patched_decode_issues: patched_scan.decode_issues,
        source_match_count,
        patched_match_count,
        source_presentation_candidate_match_count,
        patched_presentation_candidate_match_count,
        source_presentation_candidate_surface_count,
        patched_presentation_candidate_surface_count,
        source_presentation_candidate_atlas_family_count,
        duplicated_source_presentation_candidate_atlas_family_count,
        source_runtime_resident_surface_count: source_scan.runtime_resident_surfaces.len(),
        patched_runtime_resident_surface_count: patched_scan.runtime_resident_surfaces.len(),
        source_runtime_resident_surfaces: source_scan.runtime_resident_surfaces,
        patched_runtime_resident_surfaces: patched_scan.runtime_resident_surfaces,
        source_runtime_draw_source_match_count: source_scan.runtime_draw_source_matches.len(),
        patched_runtime_draw_source_match_count: patched_scan.runtime_draw_source_matches.len(),
        source_runtime_draw_source_matches: source_scan.runtime_draw_source_matches,
        patched_runtime_draw_source_matches: patched_scan.runtime_draw_source_matches,
        source_runtime_texture_source_match_count: source_scan
            .runtime_texture_source_matches
            .len(),
        patched_runtime_texture_source_match_count: patched_scan
            .runtime_texture_source_matches
            .len(),
        source_runtime_texture_source_matches: source_scan.runtime_texture_source_matches,
        patched_runtime_texture_source_matches: patched_scan.runtime_texture_source_matches,
        presentation_candidate_atlas_families,
        targets: target_reports,
        limitations: vec![
            "A byte-exact indexed-pixel match identifies a possible stored source, not the runtime loader or draw owner.".to_string(),
            "The scan covers supported embedded 4-bpp TIM images in decoded ISO records; raw textures, generated glyphs, unsupported compression, and palette-equivalent pixels with different indices remain outside the result.".to_string(),
            "Dialogue runtime-image records are classified from the exact allocation asset set, and standalone 20-pixel glyph strips are classified separately; remaining matches are presentation candidates, not automatically active consumers.".to_string(),
            "Atlas families group byte-identical source TIM payloads and same-location patched TIMs; grouping does not prove that the records share one loader, route, or semantic role.".to_string(),
            "A runtime-observed target count means that the exact target pixels appeared in the audited frame, not that every matching atlas family or surface supplied those pixels at runtime.".to_string(),
            "A runtime-resident surface requires the complete nonblank indexed image payload to match the exact GPU dump at the TIM-declared VRAM coordinates; residency still does not by itself prove a visible draw primitive.".to_string(),
            "A runtime draw-source match binds an adjacent DR_TPAGE/SPRT packet and its exact GPU pixels to a source TIM region; RAM may retain an inactive ordering-table buffer, so screen visibility remains a separate frame judgment.".to_string(),
            "A runtime texture-source match finds the complete nonblank runtime sprite texture inside a stored 4-bpp TIM without assuming that the TIM was uploaded at its declared coordinates; RAM may retain inactive packets, so a matching packet still needs frame-level visibility evidence.".to_string(),
        ],
    };
    write_fixed_presentation_report(&config.output_dir, &report)?;
    html::write_fixed_presentation_html_report(&config.output_dir, &report)?;
    ensure!(
        sha256_file(&source_cue.image_path)? == report.source_bin_sha256,
        "source BIN changed while auditing fixed presentation glyph sources"
    );
    ensure!(
        sha256_file(&patched_cue.image_path)? == report.patched_bin_sha256,
        "patched BIN changed while auditing fixed presentation glyph sources"
    );
    Ok(report)
}

fn fixed_presentation_targets(
    source_decoded: &[u8],
    runtime_report: &RuntimeGlyphAuditInput,
    codebook: &PresentationCodebookInput,
) -> Result<Vec<FixedPresentationTarget>> {
    let tim = parse_4bpp_prefix(source_decoded)?;
    ensure!(
        tim.pixel_width() == FIXED_PRESENTATION_GLYPH_WIDTH,
        "fixed-presentation source dialogue TIM is not one 20-pixel glyph column"
    );
    ensure!(
        tim.image_height
            .is_multiple_of(FIXED_PRESENTATION_GLYPH_HEIGHT),
        "fixed-presentation source dialogue TIM contains a partial glyph cell"
    );
    let indexed = read_4bpp_indexed_image_in_prefix(source_decoded, 0)?;
    let cell_byte_count = tim.row_bytes() * FIXED_PRESENTATION_GLYPH_HEIGHT;
    let fixed_cell_count = tim.image_height / FIXED_PRESENTATION_GLYPH_HEIGHT;
    let mut targets = BTreeMap::<String, FixedPresentationTarget>::new();
    let mut codebook_by_digest = BTreeMap::new();
    for entry in &codebook.entries {
        ensure!(
            codebook_by_digest
                .insert(entry.pixel_sha256.as_str(), entry)
                .is_none(),
            "codebook defines pixel hash {} more than once",
            entry.pixel_sha256
        );
    }
    for code in 0..fixed_cell_count {
        let packed_start = tim
            .pixel_offset
            .checked_add(code * cell_byte_count)
            .context("fixed-presentation source cell offset overflow")?;
        let packed = source_decoded
            .get(packed_start..packed_start + cell_byte_count)
            .context("fixed-presentation source cell exceeds the source TIM")?;
        let pixel_sha256 = sha256_bytes(packed);
        let Some(entry) = codebook_by_digest.get(pixel_sha256.as_str()) else {
            continue;
        };
        let PresentationCodebookMeaningInput::Character { text } = &entry.meaning else {
            continue;
        };
        if !uses_japanese_script(text) {
            continue;
        }
        let pixels =
            fixed_presentation_cell_pixels(indexed.width, indexed.height, &indexed.pixels, code)?;
        let target =
            targets
                .entry(pixel_sha256.clone())
                .or_insert_with(|| FixedPresentationTarget {
                    pixel_sha256,
                    pixels: pixels.clone(),
                    source_codes: BTreeSet::new(),
                    source_texts: BTreeSet::new(),
                    source_semantic_ids: BTreeSet::new(),
                    runtime_uses: BTreeSet::new(),
                });
        ensure!(
            target.pixels == pixels,
            "one codebook pixel hash resolved to different indexed glyphs"
        );
        target.source_codes.insert(format!("0x{code:04x}"));
        target.source_texts.insert(text.clone());
    }
    for consumer in &runtime_report.draw_consumers {
        let relevant_glyphs = consumer
            .source_glyphs
            .iter()
            .filter(|glyph| {
                glyph.source_text_uses_japanese_script || glyph.source_semantic_id.is_some()
            })
            .collect::<Vec<_>>();
        if relevant_glyphs.is_empty() {
            continue;
        }
        for glyph in relevant_glyphs {
            let code = parse_fixed_presentation_code(&glyph.source_code)?;
            ensure!(
                code < fixed_cell_count,
                "runtime glyph source code {} exceeds the source TIM",
                glyph.source_code
            );
            let packed_start = tim
                .pixel_offset
                .checked_add(code * cell_byte_count)
                .context("fixed-presentation source cell offset overflow")?;
            let packed = source_decoded
                .get(packed_start..packed_start + cell_byte_count)
                .context("fixed-presentation source cell exceeds the source TIM")?;
            ensure!(
                sha256_bytes(packed) == consumer.pixel_sha256,
                "runtime report pixel hash disagrees with source code {}",
                glyph.source_code
            );
            let pixels = fixed_presentation_cell_pixels(
                indexed.width,
                indexed.height,
                &indexed.pixels,
                code,
            )?;
            let target = targets
                .entry(consumer.pixel_sha256.clone())
                .or_insert_with(|| FixedPresentationTarget {
                    pixel_sha256: consumer.pixel_sha256.clone(),
                    pixels: pixels.clone(),
                    source_codes: BTreeSet::new(),
                    source_texts: BTreeSet::new(),
                    source_semantic_ids: BTreeSet::new(),
                    runtime_uses: BTreeSet::new(),
                });
            ensure!(
                target.pixels == pixels,
                "one runtime pixel hash resolved to different indexed glyphs"
            );
            target.source_codes.insert(glyph.source_code.clone());
            if let Some(text) = &glyph.source_text {
                target.source_texts.insert(text.clone());
            }
            if let Some(id) = &glyph.source_semantic_id {
                target.source_semantic_ids.insert(id.clone());
            }
            target.runtime_uses.insert((
                consumer.screen_x,
                consumer.screen_y,
                consumer.sprite_width_pixels,
                consumer.sprite_height_pixels,
                consumer.texture_vram_word_x,
                consumer.texture_vram_y,
            ));
        }
    }
    Ok(targets.into_values().collect())
}

fn fixed_presentation_cell_pixels(
    indexed_width: usize,
    indexed_height: usize,
    indexed_pixels: &[u8],
    code: usize,
) -> Result<Vec<u8>> {
    ensure!(
        indexed_width == FIXED_PRESENTATION_GLYPH_WIDTH,
        "fixed-presentation source image width changed"
    );
    let pixel_y = code
        .checked_mul(FIXED_PRESENTATION_GLYPH_HEIGHT)
        .context("fixed-presentation source cell pixel offset overflow")?;
    ensure!(
        pixel_y + FIXED_PRESENTATION_GLYPH_HEIGHT <= indexed_height,
        "fixed-presentation source cell exceeds indexed pixels"
    );
    let mut pixels =
        Vec::with_capacity(FIXED_PRESENTATION_GLYPH_WIDTH * FIXED_PRESENTATION_GLYPH_HEIGHT);
    for row in 0..FIXED_PRESENTATION_GLYPH_HEIGHT {
        let start = (pixel_y + row) * indexed_width;
        pixels.extend_from_slice(&indexed_pixels[start..start + FIXED_PRESENTATION_GLYPH_WIDTH]);
    }
    Ok(pixels)
}

fn uses_japanese_script(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{f900}'..='\u{faff}'
        )
    })
}

fn parse_fixed_presentation_code(value: &str) -> Result<usize> {
    let hex = value
        .strip_prefix("0x")
        .with_context(|| format!("glyph code is not hexadecimal: {value}"))?;
    usize::from_str_radix(hex, 16).with_context(|| format!("invalid glyph code: {value}"))
}

fn scan_fixed_presentation_disc(
    input: FixedPresentationDiscScanInput<'_>,
) -> Result<FixedPresentationDiscScan> {
    let FixedPresentationDiscScanInput {
        cue_path,
        side_label,
        side,
        targets,
        allocation_paths,
        runtime_gpu_dump,
        runtime_sprite_packets,
        runtime_texture_targets,
        output_dir,
    } = input;
    let cue = CueSheet::parse(cue_path)?;
    let mut track = RawTrack::open(&cue.image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let files = file_map(iso.files()?)?;
    let mut decoded_layer_count = 0usize;
    let mut embedded_tim_count = 0usize;
    let mut decode_issues = Vec::new();
    let mut matches_by_digest = targets
        .iter()
        .map(|target| (target.pixel_sha256.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut runtime_resident_surfaces = Vec::new();
    let mut runtime_draw_source_matches = Vec::new();
    let mut runtime_texture_source_matches = Vec::new();
    let target_index = FixedPresentationTargetIndex::new(targets);

    for (record_path, record) in &files {
        let stored = iso
            .read_record(record)
            .with_context(|| format!("failed to read ISO record {record_path}"))?;
        let decoded = decode_record(record_path, &stored, side);
        decode_issues.extend(decoded.issues.iter().cloned());
        for layer in decoded.layers.values() {
            decoded_layer_count += 1;
            let layer_sha256 = sha256_bytes(&layer.bytes);
            for tim in embedded_tim_candidates(&layer.bytes) {
                embedded_tim_count += 1;
                if tim.bits_per_pixel != 4 {
                    continue;
                }
                let indexed = match read_embedded_tim_indices(&layer.bytes, &tim) {
                    Ok(indexed) => indexed,
                    Err(_) => continue,
                };
                let mut preview_rectangles = BTreeSet::<(usize, usize, usize, usize)>::new();
                let source_class =
                    fixed_presentation_source_class(record_path, &tim, allocation_paths);
                if runtime_gpu_dump.is_some_and(|gpu_dump| {
                    tim_image_payload_matches_vram(&layer.bytes, &tim, gpu_dump).unwrap_or(false)
                }) {
                    runtime_resident_surfaces.push(FixedPresentationRuntimeResidentSurface {
                        record_path: record_path.clone(),
                        decoded_layer_id: layer.id.clone(),
                        decoded_layer_sha256: layer_sha256.clone(),
                        tim_offset: tim.offset,
                        tim_sha256: tim.source_tim_sha256.clone(),
                        tim_bits_per_pixel: tim.bits_per_pixel,
                        tim_pixel_width: tim.pixel_width,
                        tim_pixel_height: tim.pixel_height,
                        tim_image_vram_word_x: tim.image_vram_word_x,
                        tim_image_vram_y: tim.image_vram_y,
                        source_class: source_class.to_string(),
                    });
                }
                if let Some(gpu_dump) = runtime_gpu_dump {
                    for packet in runtime_sprite_packets {
                        let Some((source_pixel_x, source_pixel_y)) =
                            sprite_source_pixel_origin(&tim, packet)
                        else {
                            continue;
                        };
                        if sprite_pixels_match_source(
                            &indexed,
                            source_pixel_x,
                            source_pixel_y,
                            gpu_dump,
                            packet,
                        )? {
                            runtime_draw_source_matches.push(
                                FixedPresentationRuntimeDrawSourceMatch {
                                    record_path: record_path.clone(),
                                    decoded_layer_id: layer.id.clone(),
                                    decoded_layer_sha256: layer_sha256.clone(),
                                    tim_offset: tim.offset,
                                    tim_sha256: tim.source_tim_sha256.clone(),
                                    tim_bits_per_pixel: tim.bits_per_pixel,
                                    tim_pixel_width: tim.pixel_width,
                                    tim_pixel_height: tim.pixel_height,
                                    tim_image_vram_word_x: tim.image_vram_word_x,
                                    tim_image_vram_y: tim.image_vram_y,
                                    source_class: source_class.to_string(),
                                    source_pixel_x,
                                    source_pixel_y,
                                    packet_address: format!("0x{:08x}", packet.packet_address),
                                    screen_x: packet.screen_x,
                                    screen_y: packet.screen_y,
                                    sprite_width_pixels: packet.width,
                                    sprite_height_pixels: packet.height,
                                    texture_vram_word_x: packet.texture_page_word_x
                                        + packet.texture_u / 4,
                                    texture_vram_y: packet.texture_page_y + packet.texture_v,
                                    clut: format!("0x{:04x}", packet.clut),
                                    draw_mode: format!("0x{:08x}", packet.draw_mode),
                                },
                            );
                        }
                    }
                }
                for target in runtime_texture_targets {
                    for (source_pixel_x, source_pixel_y) in
                        runtime_texture_source_origins(&indexed, target)
                    {
                        if source_class == "presentation_candidate" {
                            preview_rectangles.insert((
                                source_pixel_x,
                                source_pixel_y,
                                target.width,
                                target.height,
                            ));
                        }
                        for packet in &target.packets {
                            runtime_texture_source_matches.push(
                                FixedPresentationRuntimeTextureSourceMatch {
                                    runtime_texture_pixel_sha256: target.pixel_sha256.clone(),
                                    record_path: record_path.clone(),
                                    decoded_layer_id: layer.id.clone(),
                                    decoded_layer_sha256: layer_sha256.clone(),
                                    tim_offset: tim.offset,
                                    tim_sha256: tim.source_tim_sha256.clone(),
                                    tim_bits_per_pixel: tim.bits_per_pixel,
                                    tim_pixel_width: tim.pixel_width,
                                    tim_pixel_height: tim.pixel_height,
                                    tim_image_vram_word_x: tim.image_vram_word_x,
                                    tim_image_vram_y: tim.image_vram_y,
                                    source_class: source_class.to_string(),
                                    source_pixel_x,
                                    source_pixel_y,
                                    packet_address: format!("0x{:08x}", packet.packet_address),
                                    screen_x: packet.screen_x,
                                    screen_y: packet.screen_y,
                                    sprite_width_pixels: packet.width,
                                    sprite_height_pixels: packet.height,
                                    texture_vram_word_x: packet.texture_page_word_x
                                        + packet.texture_u / 4,
                                    texture_vram_y: packet.texture_page_y + packet.texture_v,
                                    clut: format!("0x{:04x}", packet.clut),
                                    draw_mode: format!("0x{:08x}", packet.draw_mode),
                                },
                            );
                        }
                    }
                }
                let mut local_matches = Vec::<(String, FixedPresentationGlyphSourceMatch)>::new();
                for (matched_target_index, pixel_x, pixel_y) in target_index.find(&indexed, targets)
                {
                    let target = &targets[matched_target_index];
                    if source_class == "presentation_candidate" {
                        preview_rectangles.insert((
                            pixel_x,
                            pixel_y,
                            FIXED_PRESENTATION_GLYPH_WIDTH,
                            FIXED_PRESENTATION_GLYPH_HEIGHT,
                        ));
                    }
                    local_matches.push((
                        target.pixel_sha256.clone(),
                        FixedPresentationGlyphSourceMatch {
                            record_path: record_path.clone(),
                            decoded_layer_id: layer.id.clone(),
                            decoded_layer_sha256: layer_sha256.clone(),
                            tim_offset: tim.offset,
                            tim_sha256: tim.source_tim_sha256.clone(),
                            tim_bits_per_pixel: tim.bits_per_pixel,
                            tim_pixel_width: tim.pixel_width,
                            tim_pixel_height: tim.pixel_height,
                            tim_image_vram_word_x: tim.image_vram_word_x,
                            tim_image_vram_y: tim.image_vram_y,
                            pixel_x,
                            pixel_y,
                            source_class: source_class.to_string(),
                            preview_file: None,
                            preview_sha256: None,
                        },
                    ));
                }
                let preview = if preview_rectangles.is_empty() {
                    None
                } else {
                    write_fixed_presentation_preview(
                        output_dir,
                        side_label,
                        record_path,
                        &layer.id,
                        &layer.bytes,
                        &tim,
                        &preview_rectangles,
                    )?
                };
                for (digest, mut matched) in local_matches {
                    if matched.source_class == "presentation_candidate"
                        && let Some((path, sha256)) = &preview
                    {
                        matched.preview_file = Some(path.clone());
                        matched.preview_sha256 = Some(sha256.clone());
                    }
                    matches_by_digest
                        .get_mut(&digest)
                        .expect("fixed-presentation target map was initialized")
                        .push(matched);
                }
            }
        }
    }
    for matches in matches_by_digest.values_mut() {
        matches.sort_by(|left, right| {
            (
                &left.record_path,
                &left.decoded_layer_id,
                left.tim_offset,
                left.pixel_y,
                left.pixel_x,
            )
                .cmp(&(
                    &right.record_path,
                    &right.decoded_layer_id,
                    right.tim_offset,
                    right.pixel_y,
                    right.pixel_x,
                ))
        });
    }
    runtime_resident_surfaces.sort_by(|left, right| {
        (&left.record_path, &left.decoded_layer_id, left.tim_offset).cmp(&(
            &right.record_path,
            &right.decoded_layer_id,
            right.tim_offset,
        ))
    });
    runtime_draw_source_matches.sort_by(|left, right| {
        (
            left.screen_y,
            left.screen_x,
            &left.record_path,
            &left.decoded_layer_id,
            left.tim_offset,
            &left.packet_address,
        )
            .cmp(&(
                right.screen_y,
                right.screen_x,
                &right.record_path,
                &right.decoded_layer_id,
                right.tim_offset,
                &right.packet_address,
            ))
    });
    runtime_texture_source_matches.sort_by(|left, right| {
        (
            left.screen_y,
            left.screen_x,
            &left.record_path,
            &left.decoded_layer_id,
            left.tim_offset,
            left.source_pixel_y,
            left.source_pixel_x,
            &left.packet_address,
        )
            .cmp(&(
                right.screen_y,
                right.screen_x,
                &right.record_path,
                &right.decoded_layer_id,
                right.tim_offset,
                right.source_pixel_y,
                right.source_pixel_x,
                &right.packet_address,
            ))
    });
    Ok(FixedPresentationDiscScan {
        iso_file_count: files.len(),
        decoded_layer_count,
        embedded_tim_count,
        decode_issues,
        matches_by_digest,
        runtime_resident_surfaces,
        runtime_draw_source_matches,
        runtime_texture_source_matches,
    })
}

fn scan_runtime_four_bit_sprite_packets(
    ram_dump: &[u8],
) -> Result<Vec<RuntimeFourBitSpritePacket>> {
    ensure!(
        ram_dump.len() == PSX_RAM_BYTE_COUNT,
        "runtime RAM dump geometry changed"
    );
    let mut packets = Vec::new();
    for packet_offset in
        (DR_TPAGE_PACKET_BYTE_COUNT..=ram_dump.len() - SPRT_PACKET_BYTE_COUNT).step_by(4)
    {
        if ram_dump[packet_offset + 7] != SPRT_COMMAND {
            continue;
        }
        let draw_mode = read_runtime_u32(ram_dump, packet_offset - 4);
        if (draw_mode >> 24) as u8 != DR_TPAGE_COMMAND {
            continue;
        }
        let tpage = (draw_mode & 0x1ff) as usize;
        if ((tpage >> 7) & 0x3) != 0 {
            continue;
        }
        let width = usize::from(read_runtime_u16(ram_dump, packet_offset + 16));
        let height = usize::from(read_runtime_u16(ram_dump, packet_offset + 18));
        if width == 0 || height == 0 || width > 256 || height > 256 {
            continue;
        }
        let texture_page_word_x = (tpage & 0x0f) * 64;
        let texture_page_y = ((tpage >> 4) & 1) * 256;
        let texture_u = usize::from(ram_dump[packet_offset + 12]);
        let texture_v = usize::from(ram_dump[packet_offset + 13]);
        if texture_page_word_x * 4 + texture_u + width > PSX_VRAM_WIDTH_WORDS * 4
            || texture_page_y + texture_v + height > PSX_VRAM_HEIGHT
        {
            continue;
        }
        packets.push(RuntimeFourBitSpritePacket {
            packet_address: PSX_RAM_RUNTIME_BASE + packet_offset as u32,
            screen_x: i32::from(read_runtime_i16(ram_dump, packet_offset + 8)),
            screen_y: i32::from(read_runtime_i16(ram_dump, packet_offset + 10)),
            width,
            height,
            texture_page_word_x,
            texture_page_y,
            texture_u,
            texture_v,
            clut: read_runtime_u16(ram_dump, packet_offset + 14),
            draw_mode,
        });
    }
    Ok(packets)
}

fn sprite_source_pixel_origin(
    tim: &EmbeddedTimAudit,
    packet: &RuntimeFourBitSpritePacket,
) -> Option<(usize, usize)> {
    let tim_global_pixel_x = usize::from(tim.image_vram_word_x) * 4;
    let tim_global_y = usize::from(tim.image_vram_y);
    let packet_global_pixel_x = packet.texture_page_word_x * 4 + packet.texture_u;
    let packet_global_y = packet.texture_page_y + packet.texture_v;
    let source_pixel_x = packet_global_pixel_x.checked_sub(tim_global_pixel_x)?;
    let source_pixel_y = packet_global_y.checked_sub(tim_global_y)?;
    (source_pixel_x + packet.width <= tim.pixel_width
        && source_pixel_y + packet.height <= tim.pixel_height)
        .then_some((source_pixel_x, source_pixel_y))
}

fn sprite_pixels_match_source(
    source: &IndexedPixels,
    source_pixel_x: usize,
    source_pixel_y: usize,
    gpu_dump: &[u8],
    packet: &RuntimeFourBitSpritePacket,
) -> Result<bool> {
    ensure!(
        gpu_dump.len() == PSX_VRAM_BYTE_COUNT,
        "runtime GPU dump geometry changed"
    );
    for y in 0..packet.height {
        for x in 0..packet.width {
            let source_pixel =
                source.pixels[(source_pixel_y + y) * source.width + source_pixel_x + x];
            let global_pixel_x = packet.texture_page_word_x * 4 + packet.texture_u + x;
            let global_y = packet.texture_page_y + packet.texture_v + y;
            let packed_offset =
                global_y * PSX_VRAM_WIDTH_WORDS * PSX_VRAM_WORD_BYTES + global_pixel_x / 2;
            let packed = gpu_dump[packed_offset];
            let runtime_pixel = if global_pixel_x.is_multiple_of(2) {
                packed & 0x0f
            } else {
                packed >> 4
            };
            if source_pixel != runtime_pixel {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn runtime_four_bit_texture_targets(
    packets: &[RuntimeFourBitSpritePacket],
    gpu_dump: &[u8],
    min_pixel_count: usize,
) -> Result<Vec<RuntimeFourBitTextureTarget>> {
    ensure!(
        gpu_dump.len() == PSX_VRAM_BYTE_COUNT,
        "runtime GPU dump geometry changed"
    );
    let mut targets = BTreeMap::<(usize, usize, String), RuntimeFourBitTextureTarget>::new();
    for packet in packets {
        let pixel_count = packet
            .width
            .checked_mul(packet.height)
            .context("runtime sprite pixel count overflow")?;
        if pixel_count < min_pixel_count {
            continue;
        }
        let pixels = runtime_sprite_texture_pixels(packet, gpu_dump);
        if pixels.iter().all(|pixel| *pixel == 0) {
            continue;
        }
        let pixel_sha256 = sha256_bytes(&pixels);
        let key = (packet.width, packet.height, pixel_sha256.clone());
        let target = targets
            .entry(key)
            .or_insert_with(|| RuntimeFourBitTextureTarget {
                pixel_sha256,
                width: packet.width,
                height: packet.height,
                pixels,
                packets: Vec::new(),
            });
        ensure!(
            target.pixels == runtime_sprite_texture_pixels(packet, gpu_dump),
            "runtime texture digest collision"
        );
        target.packets.push(packet.clone());
    }
    let mut targets = targets.into_values().collect::<Vec<_>>();
    for target in &mut targets {
        target.packets.sort_by_key(|packet| packet.packet_address);
    }
    targets.sort_by(|left, right| {
        (left.height, left.width, &left.pixel_sha256).cmp(&(
            right.height,
            right.width,
            &right.pixel_sha256,
        ))
    });
    Ok(targets)
}

fn runtime_sprite_texture_pixels(packet: &RuntimeFourBitSpritePacket, gpu_dump: &[u8]) -> Vec<u8> {
    let global_pixel_x = packet.texture_page_word_x * 4 + packet.texture_u;
    let global_y = packet.texture_page_y + packet.texture_v;
    let mut pixels = Vec::with_capacity(packet.width * packet.height);
    for y in 0..packet.height {
        for x in 0..packet.width {
            pixels.push(vram_four_bit_pixel(
                gpu_dump,
                global_pixel_x + x,
                global_y + y,
            ));
        }
    }
    pixels
}

fn runtime_texture_source_origins(
    source: &IndexedPixels,
    target: &RuntimeFourBitTextureTarget,
) -> Vec<(usize, usize)> {
    if target.width > source.width || target.height > source.height {
        return Vec::new();
    }
    let (anchor_x, anchor_y, anchor) = runtime_texture_anchor(target);
    let mut origins = Vec::new();
    for source_y in 0..=source.height - target.height {
        for source_x in 0..=source.width - target.width {
            let anchor_start = (source_y + anchor_y) * source.width + source_x + anchor_x;
            if source.pixels[anchor_start..anchor_start + anchor.len()] != *anchor {
                continue;
            }
            if runtime_texture_matches_at(source, target, source_x, source_y) {
                origins.push((source_x, source_y));
            }
        }
    }
    origins
}

fn runtime_texture_anchor(target: &RuntimeFourBitTextureTarget) -> (usize, usize, &[u8]) {
    let anchor_width = target.width.min(32);
    let mut best = (0usize, 0usize, 0usize);
    for y in 0..target.height {
        let row_start = y * target.width;
        for x in 0..=target.width - anchor_width {
            let window = &target.pixels[row_start + x..row_start + x + anchor_width];
            let nonzero = window.iter().filter(|pixel| **pixel != 0).count();
            let transitions = window.windows(2).filter(|pair| pair[0] != pair[1]).count();
            let distinct = window.iter().copied().collect::<BTreeSet<_>>().len();
            let score = nonzero * 4 + transitions * 8 + distinct;
            if score > best.2 {
                best = (x, y, score);
            }
        }
    }
    let start = best.1 * target.width + best.0;
    (best.0, best.1, &target.pixels[start..start + anchor_width])
}

fn runtime_texture_matches_at(
    source: &IndexedPixels,
    target: &RuntimeFourBitTextureTarget,
    source_x: usize,
    source_y: usize,
) -> bool {
    (0..target.height).all(|row| {
        let source_start = (source_y + row) * source.width + source_x;
        let target_start = row * target.width;
        source.pixels[source_start..source_start + target.width]
            == target.pixels[target_start..target_start + target.width]
    })
}

fn runtime_sprite_cells(
    packets: &[RuntimeFourBitSpritePacket],
    gpu_dump: &[u8],
) -> Result<Vec<FixedPresentationRuntimeSpriteCellAudit>> {
    ensure!(
        gpu_dump.len() == PSX_VRAM_BYTE_COUNT,
        "runtime GPU dump geometry changed"
    );
    let mut cells = Vec::new();
    for packet in packets {
        if packet.width < FIXED_PRESENTATION_GLYPH_WIDTH
            || packet.height < FIXED_PRESENTATION_GLYPH_HEIGHT
        {
            continue;
        }
        for cell_offset_y in (0..=packet.height - FIXED_PRESENTATION_GLYPH_HEIGHT)
            .step_by(FIXED_PRESENTATION_GLYPH_HEIGHT)
        {
            for cell_offset_x in (0..=packet.width - FIXED_PRESENTATION_GLYPH_WIDTH)
                .step_by(FIXED_PRESENTATION_GLYPH_WIDTH)
            {
                let global_pixel_x =
                    packet.texture_page_word_x * 4 + packet.texture_u + cell_offset_x;
                let global_y = packet.texture_page_y + packet.texture_v + cell_offset_y;
                let mut packed = Vec::with_capacity(
                    FIXED_PRESENTATION_GLYPH_WIDTH * FIXED_PRESENTATION_GLYPH_HEIGHT / 2,
                );
                let mut nonzero_pixel_count = 0usize;
                for y in 0..FIXED_PRESENTATION_GLYPH_HEIGHT {
                    for x in (0..FIXED_PRESENTATION_GLYPH_WIDTH).step_by(2) {
                        let left = vram_four_bit_pixel(gpu_dump, global_pixel_x + x, global_y + y);
                        let right =
                            vram_four_bit_pixel(gpu_dump, global_pixel_x + x + 1, global_y + y);
                        nonzero_pixel_count += usize::from(left != 0) + usize::from(right != 0);
                        packed.push(left | (right << 4));
                    }
                }
                if nonzero_pixel_count == 0 {
                    continue;
                }
                cells.push(FixedPresentationRuntimeSpriteCellAudit {
                    packet_address: format!("0x{:08x}", packet.packet_address),
                    screen_x: packet.screen_x + cell_offset_x as i32,
                    screen_y: packet.screen_y + cell_offset_y as i32,
                    sprite_width_pixels: packet.width,
                    sprite_height_pixels: packet.height,
                    cell_offset_x_pixels: cell_offset_x,
                    cell_offset_y_pixels: cell_offset_y,
                    texture_vram_word_x: global_pixel_x / 4,
                    texture_vram_y: global_y,
                    pixel_sha256: sha256_bytes(&packed),
                    nonzero_pixel_count,
                    clut: format!("0x{:04x}", packet.clut),
                    draw_mode: format!("0x{:08x}", packet.draw_mode),
                });
            }
        }
    }
    cells.sort_by(|left, right| {
        (
            left.screen_y,
            left.screen_x,
            &left.packet_address,
            left.cell_offset_y_pixels,
            left.cell_offset_x_pixels,
        )
            .cmp(&(
                right.screen_y,
                right.screen_x,
                &right.packet_address,
                right.cell_offset_y_pixels,
                right.cell_offset_x_pixels,
            ))
    });
    Ok(cells)
}

fn vram_four_bit_pixel(gpu_dump: &[u8], global_pixel_x: usize, global_y: usize) -> u8 {
    let packed_offset = global_y * PSX_VRAM_WIDTH_WORDS * PSX_VRAM_WORD_BYTES + global_pixel_x / 2;
    let packed = gpu_dump[packed_offset];
    if global_pixel_x.is_multiple_of(2) {
        packed & 0x0f
    } else {
        packed >> 4
    }
}

fn read_runtime_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16 slice"))
}

fn read_runtime_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("i16 slice"))
}

fn read_runtime_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 slice"))
}

fn tim_image_payload_matches_vram(
    decoded: &[u8],
    embedded: &EmbeddedTimAudit,
    gpu_dump: &[u8],
) -> Result<bool> {
    ensure!(
        gpu_dump.len() == PSX_VRAM_BYTE_COUNT,
        "runtime GPU dump geometry changed"
    );
    ensure!(
        embedded.bits_per_pixel == 4,
        "runtime TIM match requires 4bpp"
    );
    let prefix = decoded
        .get(embedded.offset..)
        .context("embedded TIM disappeared from its decoded layer")?;
    let tim = parse_4bpp_prefix(prefix)?;
    let image_word_x = usize::from(tim.image_x);
    let image_y = usize::from(tim.image_y);
    let row_bytes = tim.row_bytes();
    let image_width_words = row_bytes / PSX_VRAM_WORD_BYTES;
    if image_word_x + image_width_words > PSX_VRAM_WIDTH_WORDS
        || image_y + tim.image_height > PSX_VRAM_HEIGHT
    {
        return Ok(false);
    }
    let image_byte_count = row_bytes
        .checked_mul(tim.image_height)
        .context("TIM image byte count overflow")?;
    let image = prefix
        .get(tim.pixel_offset..tim.pixel_offset + image_byte_count)
        .context("TIM image payload is truncated")?;
    if image.iter().all(|byte| *byte == 0) {
        return Ok(false);
    }
    let vram_row_bytes = PSX_VRAM_WIDTH_WORDS * PSX_VRAM_WORD_BYTES;
    Ok((0..tim.image_height).all(|row| {
        let source_start = row * row_bytes;
        let vram_start = (image_y + row) * vram_row_bytes + image_word_x * PSX_VRAM_WORD_BYTES;
        image[source_start..source_start + row_bytes]
            == gpu_dump[vram_start..vram_start + row_bytes]
    }))
}

fn fixed_presentation_source_class(
    record_path: &str,
    tim: &EmbeddedTimAudit,
    allocation_paths: &BTreeSet<String>,
) -> &'static str {
    if allocation_paths.contains(record_path) {
        "dialogue_runtime_font"
    } else if tim.pixel_width == FIXED_PRESENTATION_GLYPH_WIDTH
        && tim
            .pixel_height
            .is_multiple_of(FIXED_PRESENTATION_GLYPH_HEIGHT)
    {
        "standalone_glyph_strip"
    } else {
        "presentation_candidate"
    }
}

fn write_fixed_presentation_preview(
    output_dir: &Path,
    side_label: &str,
    record_path: &str,
    layer_id: &str,
    decoded: &[u8],
    tim: &EmbeddedTimAudit,
    rectangles: &BTreeSet<(usize, usize, usize, usize)>,
) -> Result<Option<(String, String)>> {
    let Ok(mut image) = decode_embedded_tim_preview(decoded, tim) else {
        return Ok(None);
    };
    for &(x, y, width, height) in rectangles {
        draw_fixed_presentation_rectangle(&mut image, x, y, width, height);
    }
    let relative = format!(
        "previews/{side_label}/{}-{}-tim-{:08x}-matches.png",
        path_slug(record_path),
        path_slug(layer_id),
        tim.offset,
    );
    let path = output_dir.join(&relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    write_tim_preview(&path, &image)?;
    Ok(Some((relative, sha256_file(&path)?)))
}

fn draw_fixed_presentation_rectangle(
    image: &mut RgbaImage,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    let right = x + width - 1;
    let bottom = y + height - 1;
    for pixel_x in x..=right {
        set_fixed_presentation_preview_pixel(image, pixel_x, y);
        set_fixed_presentation_preview_pixel(image, pixel_x, bottom);
    }
    for pixel_y in y..=bottom {
        set_fixed_presentation_preview_pixel(image, x, pixel_y);
        set_fixed_presentation_preview_pixel(image, right, pixel_y);
    }
}

fn set_fixed_presentation_preview_pixel(image: &mut RgbaImage, x: usize, y: usize) {
    if x >= image.width || y >= image.height {
        return;
    }
    let offset = (y * image.width + x) * 4;
    image.pixels[offset..offset + 4].copy_from_slice(&[255, 0, 255, 255]);
}

fn fixed_presentation_match_location(matched: &FixedPresentationGlyphSourceMatch) -> String {
    format!(
        "{}#{}:tim-0x{:x}:x{}:y{}",
        matched.record_path,
        matched.decoded_layer_id,
        matched.tim_offset,
        matched.pixel_x,
        matched.pixel_y,
    )
}

fn fixed_presentation_matches_share_atlas_identity(
    left: &FixedPresentationGlyphSourceMatch,
    right: &FixedPresentationGlyphSourceMatch,
) -> bool {
    left.record_path == right.record_path
        && left.decoded_layer_id == right.decoded_layer_id
        && left.decoded_layer_sha256 == right.decoded_layer_sha256
        && left.tim_offset == right.tim_offset
        && left.tim_sha256 == right.tim_sha256
        && left.tim_bits_per_pixel == right.tim_bits_per_pixel
        && left.tim_pixel_width == right.tim_pixel_width
        && left.tim_pixel_height == right.tim_pixel_height
        && left.tim_image_vram_word_x == right.tim_image_vram_word_x
        && left.tim_image_vram_y == right.tim_image_vram_y
        && left.preview_file == right.preview_file
        && left.preview_sha256 == right.preview_sha256
}

fn fixed_presentation_atlas_families(
    targets: &[FixedPresentationGlyphTargetAudit],
) -> Result<Vec<FixedPresentationGlyphAtlasFamilyAudit>> {
    let mut surface_accumulators =
        BTreeMap::<(String, String, usize), FixedPresentationGlyphAtlasSurfaceAccumulator>::new();
    for target in targets {
        for matched in &target.source_matches {
            if matched.source_class != "presentation_candidate" {
                continue;
            }
            let surface = surface_accumulators
                .entry((
                    matched.record_path.clone(),
                    matched.decoded_layer_id.clone(),
                    matched.tim_offset,
                ))
                .or_default();
            surface
                .source
                .get_or_insert_with(Default::default)
                .add_match(target, matched)?;
        }
        for matched in &target.patched_matches {
            if matched.source_class != "presentation_candidate" {
                continue;
            }
            let surface = surface_accumulators
                .entry((
                    matched.record_path.clone(),
                    matched.decoded_layer_id.clone(),
                    matched.tim_offset,
                ))
                .or_default();
            surface
                .patched
                .get_or_insert_with(Default::default)
                .add_match(target, matched)?;
        }
    }

    let mut family_surfaces =
        BTreeMap::<String, Vec<FixedPresentationGlyphAtlasSurfaceAudit>>::new();
    for ((record_path, decoded_layer_id, tim_offset), accumulated) in surface_accumulators {
        let source = accumulated
            .source
            .map(FixedPresentationGlyphAtlasSideAccumulator::finish)
            .transpose()?;
        let patched = accumulated
            .patched
            .map(FixedPresentationGlyphAtlasSideAccumulator::finish)
            .transpose()?;
        let tim_status = comparison_status(
            source.as_ref().map(|side| side.tim_sha256.as_bytes()),
            patched.as_ref().map(|side| side.tim_sha256.as_bytes()),
        );
        let family_id = if let Some(source) = &source {
            format!("source-tim-sha256:{}", source.tim_sha256)
        } else {
            let patched = patched
                .as_ref()
                .context("fixed-presentation atlas surface has neither source nor patched side")?;
            format!("patched-only-tim-sha256:{}", patched.tim_sha256)
        };
        family_surfaces.entry(family_id).or_default().push(
            FixedPresentationGlyphAtlasSurfaceAudit {
                record_path,
                decoded_layer_id,
                tim_offset,
                tim_status,
                source,
                patched,
            },
        );
    }

    let mut families = Vec::with_capacity(family_surfaces.len());
    for (family_id, mut surfaces) in family_surfaces {
        surfaces.sort_by(|left, right| {
            (&left.record_path, &left.decoded_layer_id, left.tim_offset).cmp(&(
                &right.record_path,
                &right.decoded_layer_id,
                right.tim_offset,
            ))
        });
        let source_tim_sha256 = surfaces
            .iter()
            .find_map(|surface| surface.source.as_ref().map(|side| side.tim_sha256.clone()));
        let patched_tim_sha256s = surfaces
            .iter()
            .filter_map(|surface| surface.patched.as_ref())
            .map(|side| side.tim_sha256.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let source_surface_count = surfaces
            .iter()
            .filter(|surface| surface.source.is_some())
            .count();
        let patched_surface_count = surfaces
            .iter()
            .filter(|surface| surface.patched.is_some())
            .count();
        let unchanged_surface_count = surfaces
            .iter()
            .filter(|surface| surface.tim_status == ComparisonStatus::Unchanged)
            .count();
        let changed_surface_count = surfaces
            .iter()
            .filter(|surface| surface.tim_status == ComparisonStatus::Changed)
            .count();
        let source_only_surface_count = surfaces
            .iter()
            .filter(|surface| surface.tim_status == ComparisonStatus::SourceOnly)
            .count();
        let patched_only_surface_count = surfaces
            .iter()
            .filter(|surface| surface.tim_status == ComparisonStatus::PatchedOnly)
            .count();
        let source_match_count = surfaces
            .iter()
            .filter_map(|surface| surface.source.as_ref())
            .map(|side| side.match_count)
            .sum();
        let patched_match_count = surfaces
            .iter()
            .filter_map(|surface| surface.patched.as_ref())
            .map(|side| side.match_count)
            .sum();
        let source_target_pixel_sha256s = surfaces
            .iter()
            .filter_map(|surface| surface.source.as_ref())
            .flat_map(|side| side.target_pixel_sha256s.iter().cloned())
            .collect::<BTreeSet<_>>();
        let patched_target_pixel_sha256s = surfaces
            .iter()
            .filter_map(|surface| surface.patched.as_ref())
            .flat_map(|side| side.target_pixel_sha256s.iter().cloned())
            .collect::<BTreeSet<_>>();
        let source_runtime_observed_target_count = surfaces
            .iter()
            .filter_map(|surface| surface.source.as_ref())
            .flat_map(|side| side.runtime_observed_target_pixel_sha256s.iter().cloned())
            .collect::<BTreeSet<_>>()
            .len();
        let patched_runtime_observed_target_count = surfaces
            .iter()
            .filter_map(|surface| surface.patched.as_ref())
            .flat_map(|side| side.runtime_observed_target_pixel_sha256s.iter().cloned())
            .collect::<BTreeSet<_>>()
            .len();
        let source_texts = surfaces
            .iter()
            .filter_map(|surface| surface.source.as_ref())
            .flat_map(|side| side.source_texts.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let patched_texts = surfaces
            .iter()
            .filter_map(|surface| surface.patched.as_ref())
            .flat_map(|side| side.source_texts.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        families.push(FixedPresentationGlyphAtlasFamilyAudit {
            family_id,
            source_tim_sha256,
            patched_tim_sha256s,
            surface_count: surfaces.len(),
            source_surface_count,
            patched_surface_count,
            unchanged_surface_count,
            changed_surface_count,
            source_only_surface_count,
            patched_only_surface_count,
            source_match_count,
            patched_match_count,
            source_target_count: source_target_pixel_sha256s.len(),
            patched_target_count: patched_target_pixel_sha256s.len(),
            source_runtime_observed_target_count,
            patched_runtime_observed_target_count,
            source_texts,
            patched_texts,
            surfaces,
        });
    }
    families.sort_by(|left, right| {
        right
            .source_surface_count
            .cmp(&left.source_surface_count)
            .then_with(|| right.source_target_count.cmp(&left.source_target_count))
            .then_with(|| left.family_id.cmp(&right.family_id))
    });
    Ok(families)
}

fn prepare_fixed_presentation_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    ensure_safe_output_path(output_dir)?;
    let marker = output_dir.join(FIXED_PRESENTATION_OUTPUT_MARKER_FILE);
    if output_dir.exists() {
        if !force {
            bail!(
                "fixed-presentation glyph source audit output exists; pass --force to replace it: {}",
                output_dir.display()
            );
        }
        let marker_text = std::fs::read_to_string(&marker).with_context(|| {
            format!(
                "refusing to replace an unowned output directory without {}: {}",
                FIXED_PRESENTATION_OUTPUT_MARKER_FILE,
                output_dir.display()
            )
        })?;
        ensure!(
            marker_text == FIXED_PRESENTATION_OUTPUT_MARKER_TEXT,
            "refusing to replace fixed-presentation output with an unknown ownership marker: {}",
            output_dir.display()
        );
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    std::fs::write(
        output_dir.join(FIXED_PRESENTATION_OUTPUT_MARKER_FILE),
        FIXED_PRESENTATION_OUTPUT_MARKER_TEXT,
    )?;
    Ok(())
}

fn write_fixed_presentation_report(
    output_dir: &Path,
    report: &FixedPresentationGlyphSourceAuditReport,
) -> Result<()> {
    let path = output_dir.join(FIXED_PRESENTATION_REPORT_FILE);
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

pub fn compare_disc_assets(
    config: &DiscAssetComparisonConfig,
) -> Result<DiscAssetComparisonReport> {
    let source_cue = CueSheet::parse(&config.source_cue)?;
    let patched_cue = CueSheet::parse(&config.patched_cue)?;
    let source_bin_sha256 = sha256_file(&source_cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let patched_bin_sha256 = sha256_file(&patched_cue.image_path)?;
    let source_bin_byte_count = std::fs::metadata(&source_cue.image_path)?.len();
    let patched_bin_byte_count = std::fs::metadata(&patched_cue.image_path)?.len();

    let mut source_track = RawTrack::open(&source_cue.image_path)?;
    let mut patched_track = RawTrack::open(&patched_cue.image_path)?;
    let mut source_iso = Iso9660::open(&mut source_track)?;
    let mut patched_iso = Iso9660::open(&mut patched_track)?;
    let source_files = file_map(source_iso.files()?)?;
    let patched_files = file_map(patched_iso.files()?)?;
    let source_paths = source_files.keys().cloned().collect::<BTreeSet<_>>();
    let patched_paths = patched_files.keys().cloned().collect::<BTreeSet<_>>();
    prepare_output_directory(&config.output_dir, config.force)?;
    let all_paths = source_paths
        .union(&patched_paths)
        .cloned()
        .collect::<Vec<_>>();

    let mut records = Vec::with_capacity(all_paths.len());
    for (record_index, path) in all_paths.iter().enumerate() {
        let source = source_files
            .get(path)
            .map(|record| read_record(&mut source_iso, record, path))
            .transpose()?;
        let patched = patched_files
            .get(path)
            .map(|record| read_record(&mut patched_iso, record, path))
            .transpose()?;
        records.push(compare_record(
            record_index,
            path,
            source,
            patched,
            &config.output_dir,
        )?);
    }

    let unchanged_record_count = count_records(&records, ComparisonStatus::Unchanged);
    let changed_record_count = count_records(&records, ComparisonStatus::Changed);
    let source_only_record_count = count_records(&records, ComparisonStatus::SourceOnly);
    let patched_only_record_count = count_records(&records, ComparisonStatus::PatchedOnly);
    let decoded_layer_comparison_count = records
        .iter()
        .map(|record| record.decoded_layers.len())
        .sum();
    let changed_decoded_layer_count = records
        .iter()
        .flat_map(|record| &record.decoded_layers)
        .filter(|layer| layer.status != ComparisonStatus::Unchanged)
        .count();
    let embedded_tim_comparison_count = records
        .iter()
        .flat_map(|record| &record.decoded_layers)
        .map(|layer| layer.embedded_tim_comparison_count)
        .sum();
    let changed_embedded_tim_count = records
        .iter()
        .flat_map(|record| &record.decoded_layers)
        .flat_map(|layer| &layer.embedded_tims)
        .filter(|tim| tim.status != ComparisonStatus::Unchanged)
        .count();
    let visually_changed_tim_count = records
        .iter()
        .flat_map(|record| &record.decoded_layers)
        .flat_map(|layer| &layer.embedded_tims)
        .filter(|tim| {
            tim.indexed_pixel_difference_count
                .is_some_and(|count| count > 0)
                || tim
                    .palette_zero_rgba_difference_count
                    .is_some_and(|count| count > 0)
                || matches!(
                    tim.status,
                    ComparisonStatus::SourceOnly | ComparisonStatus::PatchedOnly
                )
        })
        .count();
    let changed_clut_tim_count = records
        .iter()
        .flat_map(|record| &record.decoded_layers)
        .flat_map(|layer| &layer.embedded_tims)
        .filter(|tim| {
            tim.clut_word_difference_count
                .is_some_and(|count| count > 0)
        })
        .count();
    let changed_record_without_embedded_tim_count = records
        .iter()
        .filter(|record| {
            record.status != ComparisonStatus::Unchanged
                && record
                    .decoded_layers
                    .iter()
                    .all(|layer| layer.embedded_tim_comparison_count == 0)
        })
        .count();
    let decode_issue_count = records
        .iter()
        .map(|record| record.decode_issues.len())
        .sum();

    let report = DiscAssetComparisonReport {
        kind: "justice_gakuen2_disc_asset_comparison".to_string(),
        visual_scan_scope: "Every ISO record is compared by path and bytes. Changed or unpaired decoded layers are exhaustively scanned for supported embedded TIMs. Side-by-side previews are generated only for changed or unpaired TIMs and use palette 0 when a CLUT is present; the exact all-palette CLUT word difference is reported separately. Byte-identical records, layers, and TIMs are counted but not redundantly rendered.".to_string(),
        output_directory: canonical_path_string(&config.output_dir)?,
        source_cue: canonical_path_string(&config.source_cue)?,
        source_bin: canonical_path_string(&source_cue.image_path)?,
        source_bin_sha256,
        source_bin_byte_count,
        patched_cue: canonical_path_string(&config.patched_cue)?,
        patched_bin: canonical_path_string(&patched_cue.image_path)?,
        patched_bin_sha256,
        patched_bin_byte_count,
        source_iso_file_count: source_files.len(),
        patched_iso_file_count: patched_files.len(),
        file_inventory_matches: source_paths == patched_paths,
        compared_record_count: records.len(),
        unchanged_record_count,
        changed_record_count,
        source_only_record_count,
        patched_only_record_count,
        changed_record_without_embedded_tim_count,
        decoded_layer_comparison_count,
        changed_decoded_layer_count,
        embedded_tim_comparison_count,
        changed_embedded_tim_count,
        visually_changed_tim_count,
        changed_clut_tim_count,
        decode_issue_count,
        report_file: REPORT_FILE.to_string(),
        html_file: HTML_FILE.to_string(),
        records,
    };
    write_report(&config.output_dir, &report)?;
    html::write_html_report(&config.output_dir, &report)?;

    ensure!(
        sha256_file(&source_cue.image_path)? == report.source_bin_sha256,
        "source BIN changed while comparing assets"
    );
    ensure!(
        sha256_file(&patched_cue.image_path)? == report.patched_bin_sha256,
        "patched BIN changed while comparing assets"
    );
    Ok(report)
}

fn file_map(files: Vec<(String, FileRecord)>) -> Result<BTreeMap<String, FileRecord>> {
    let mut output = BTreeMap::new();
    for (path, record) in files {
        ensure!(
            output.insert(path.clone(), record).is_none(),
            "duplicate ISO path: {path}"
        );
    }
    Ok(output)
}

fn read_record(iso: &mut Iso9660<'_>, record: &FileRecord, path: &str) -> Result<RecordBytes> {
    let bytes = iso
        .read_record(record)
        .with_context(|| format!("failed to read ISO record {path}"))?;
    ensure!(
        bytes.len() == record.size as usize,
        "ISO record {path} size differs from its directory entry"
    );
    Ok(RecordBytes {
        record: record.clone(),
        bytes,
    })
}

fn compare_record(
    record_index: usize,
    path: &str,
    source: Option<RecordBytes>,
    patched: Option<RecordBytes>,
    output_dir: &Path,
) -> Result<DiscRecordComparison> {
    let status = comparison_status(
        source.as_ref().map(|record| record.bytes.as_slice()),
        patched.as_ref().map(|record| record.bytes.as_slice()),
    );
    let extent_metadata_matches = match (&source, &patched) {
        (Some(source), Some(patched)) => source.record == patched.record,
        _ => false,
    };
    let status = if status == ComparisonStatus::Unchanged && !extent_metadata_matches {
        ComparisonStatus::Changed
    } else {
        status
    };
    let stored_difference = match (&source, &patched) {
        (Some(source), Some(patched)) if status == ComparisonStatus::Changed => {
            Some(byte_difference_summary(&source.bytes, &patched.bytes))
        }
        _ => None,
    };

    let (decoded_layers, decode_issues) = if status == ComparisonStatus::Unchanged {
        (Vec::new(), Vec::new())
    } else {
        compare_decoded_layers(
            record_index,
            path,
            source.as_ref().map(|record| record.bytes.as_slice()),
            patched.as_ref().map(|record| record.bytes.as_slice()),
            output_dir,
        )?
    };

    Ok(DiscRecordComparison {
        path: path.to_string(),
        status,
        source: source.as_ref().map(record_identity),
        patched: patched.as_ref().map(record_identity),
        extent_metadata_matches,
        stored_difference,
        decoded_layers,
        decode_issues,
    })
}

fn record_identity(record: &RecordBytes) -> DiscRecordIdentity {
    DiscRecordIdentity {
        extent_lba: record.record.extent_lba,
        extended_attribute_blocks: record.record.extended_attribute_blocks,
        byte_count: record.bytes.len(),
        sha256: sha256_bytes(&record.bytes),
    }
}

fn compare_decoded_layers(
    record_index: usize,
    path: &str,
    source: Option<&[u8]>,
    patched: Option<&[u8]>,
    output_dir: &Path,
) -> Result<(Vec<DecodedLayerComparison>, Vec<DecodeIssue>)> {
    let source = source.map(|bytes| decode_record(path, bytes, ComparisonSide::Source));
    let patched = patched.map(|bytes| decode_record(path, bytes, ComparisonSide::Patched));
    let mut issues = Vec::new();
    if let Some(decoded) = &source {
        issues.extend(decoded.issues.iter().map(clone_decode_issue));
    }
    if let Some(decoded) = &patched {
        issues.extend(decoded.issues.iter().map(clone_decode_issue));
    }
    let source_layers = source
        .as_ref()
        .map(|decoded| &decoded.layers)
        .cloned()
        .unwrap_or_default();
    let patched_layers = patched
        .as_ref()
        .map(|decoded| &decoded.layers)
        .cloned()
        .unwrap_or_default();
    let layer_ids = source_layers
        .keys()
        .chain(patched_layers.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut layers = Vec::with_capacity(layer_ids.len());
    for (layer_index, id) in layer_ids.into_iter().enumerate() {
        let source_layer = source_layers.get(&id);
        let patched_layer = patched_layers.get(&id);
        let status = comparison_status(
            source_layer.map(|layer| layer.bytes.as_slice()),
            patched_layer.map(|layer| layer.bytes.as_slice()),
        );
        let kind = match (source_layer, patched_layer) {
            (Some(source), Some(patched)) if source.kind != patched.kind => {
                format!("{} -> {}", source.kind, patched.kind)
            }
            (Some(layer), _) | (_, Some(layer)) => layer.kind.clone(),
            (None, None) => unreachable!("layer ID came from neither side"),
        };
        let decoded_difference = match (source_layer, patched_layer) {
            (Some(source), Some(patched)) if status == ComparisonStatus::Changed => {
                Some(byte_difference_summary(&source.bytes, &patched.bytes))
            }
            _ => None,
        };
        let tim_set = if status == ComparisonStatus::Unchanged {
            EmbeddedTimComparisonSet {
                source_count: 0,
                patched_count: 0,
                comparison_count: 0,
                unchanged_count: 0,
                changed: Vec::new(),
            }
        } else {
            compare_embedded_tims(
                record_index,
                layer_index,
                path,
                &id,
                source_layer,
                patched_layer,
                output_dir,
            )?
        };
        layers.push(DecodedLayerComparison {
            id,
            kind,
            status,
            source: source_layer.map(decoded_layer_identity),
            patched: patched_layer.map(decoded_layer_identity),
            decoded_difference,
            embedded_tim_scan_performed: status != ComparisonStatus::Unchanged,
            source_embedded_tim_count: tim_set.source_count,
            patched_embedded_tim_count: tim_set.patched_count,
            embedded_tim_comparison_count: tim_set.comparison_count,
            unchanged_embedded_tim_count: tim_set.unchanged_count,
            embedded_tims: tim_set.changed,
        });
    }
    Ok((layers, issues))
}

fn clone_decode_issue(issue: &DecodeIssue) -> DecodeIssue {
    DecodeIssue {
        side: issue.side,
        stage: issue.stage.clone(),
        message: issue.message.clone(),
    }
}

fn decode_record(path: &str, stored: &[u8], side: ComparisonSide) -> DecodedRecord {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_uppercase();
    let may_be_indexed = matches!(extension.as_str(), "BIZ" | "TIZ" | "TZZ" | "BZZ" | "BIN");
    let expects_compression = matches!(extension.as_str(), "BIZ" | "TIZ" | "TZZ" | "BZZ");
    let indexed_kind = if matches!(extension.as_str(), "TZZ" | "BZZ") {
        "indexed_compressed_member"
    } else {
        "indexed_compressed_stream"
    };

    let indexed_error = if may_be_indexed {
        match decode_indexed_layers(stored, indexed_kind) {
            Ok(layers) => {
                return DecodedRecord {
                    layers,
                    issues: Vec::new(),
                };
            }
            Err(error) => Some(error.to_string()),
        }
    } else {
        None
    };

    if matches!(extension.as_str(), "BIZ" | "TIZ") {
        match decompress(stored, true) {
            Ok(bytes) => {
                let layer = DecodedLayer {
                    id: "stream".to_string(),
                    kind: "direct_lz_stream".to_string(),
                    stored_offset: 0,
                    stored_byte_count: stored.len(),
                    bytes,
                };
                return DecodedRecord {
                    layers: [(layer.id.clone(), layer)].into_iter().collect(),
                    issues: Vec::new(),
                };
            }
            Err(error) => {
                let mut messages = vec![format!("direct LZ: {error}")];
                if let Some(indexed_error) = indexed_error {
                    messages.push(format!("indexed streams: {indexed_error}"));
                }
                return stored_record_with_issue(path, stored, side, messages.join("; "));
            }
        }
    }

    if expects_compression {
        return stored_record_with_issue(
            path,
            stored,
            side,
            indexed_error.unwrap_or_else(|| "compressed container was not decoded".to_string()),
        );
    }

    DecodedRecord {
        layers: [stored_layer(stored)].into_iter().collect(),
        issues: Vec::new(),
    }
}

fn decode_indexed_layers(stored: &[u8], kind: &str) -> Result<BTreeMap<String, DecodedLayer>> {
    let members = parse_tzz(stored)?;
    let mut layers = BTreeMap::new();
    for member in members {
        let compressed = &stored[member.compressed_range()];
        let bytes = decompress(compressed, true)
            .with_context(|| format!("failed to decode indexed member {}", member.index))?;
        let id = format!("member-{:03}", member.index);
        let layer = DecodedLayer {
            id: id.clone(),
            kind: kind.to_string(),
            stored_offset: member.offset,
            stored_byte_count: member.compressed_size,
            bytes,
        };
        ensure!(
            layers.insert(id, layer).is_none(),
            "duplicate indexed member"
        );
    }
    Ok(layers)
}

fn stored_record_with_issue(
    path: &str,
    stored: &[u8],
    side: ComparisonSide,
    message: String,
) -> DecodedRecord {
    DecodedRecord {
        layers: [stored_layer(stored)].into_iter().collect(),
        issues: vec![DecodeIssue {
            side,
            stage: format!("decode {path}"),
            message,
        }],
    }
}

fn stored_layer(stored: &[u8]) -> (String, DecodedLayer) {
    let id = "stored-record".to_string();
    (
        id.clone(),
        DecodedLayer {
            id,
            kind: "stored_record".to_string(),
            stored_offset: 0,
            stored_byte_count: stored.len(),
            bytes: stored.to_vec(),
        },
    )
}

fn decoded_layer_identity(layer: &DecodedLayer) -> DecodedLayerIdentity {
    DecodedLayerIdentity {
        stored_offset: layer.stored_offset,
        stored_byte_count: layer.stored_byte_count,
        decoded_byte_count: layer.bytes.len(),
        decoded_sha256: sha256_bytes(&layer.bytes),
    }
}

fn compare_embedded_tims(
    record_index: usize,
    layer_index: usize,
    record_path: &str,
    layer_id: &str,
    source_layer: Option<&DecodedLayer>,
    patched_layer: Option<&DecodedLayer>,
    output_dir: &Path,
) -> Result<EmbeddedTimComparisonSet> {
    let source_tims = source_layer
        .map(|layer| embedded_tim_candidates(&layer.bytes))
        .unwrap_or_default()
        .into_iter()
        .map(|tim| (tim.offset, tim))
        .collect::<BTreeMap<_, _>>();
    let patched_tims = patched_layer
        .map(|layer| embedded_tim_candidates(&layer.bytes))
        .unwrap_or_default()
        .into_iter()
        .map(|tim| (tim.offset, tim))
        .collect::<BTreeMap<_, _>>();
    let offsets = source_tims
        .keys()
        .chain(patched_tims.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let preview_dir = format!(
        "previews/{record_index:04}-{}-{layer_index:03}-{}",
        path_slug(record_path),
        path_slug(layer_id)
    );
    let comparison_count = offsets.len();
    let mut unchanged_count = 0;
    let mut changed = Vec::new();
    for offset in offsets {
        let source_tim = source_tims.get(&offset);
        let patched_tim = patched_tims.get(&offset);
        let status = comparison_status(
            tim_bytes(source_layer, source_tim)?,
            tim_bytes(patched_layer, patched_tim)?,
        );
        if status == ComparisonStatus::Unchanged {
            unchanged_count += 1;
            continue;
        }
        changed.push(compare_embedded_tim(
            offset,
            source_layer,
            patched_layer,
            source_tim,
            patched_tim,
            output_dir,
            &preview_dir,
        )?);
    }
    Ok(EmbeddedTimComparisonSet {
        source_count: source_tims.len(),
        patched_count: patched_tims.len(),
        comparison_count,
        unchanged_count,
        changed,
    })
}

fn embedded_tim_candidates(decoded: &[u8]) -> Vec<EmbeddedTimAudit> {
    if decoded.len() < 8 {
        return Vec::new();
    }
    decoded
        .windows(4)
        .enumerate()
        .filter(|(_, bytes)| *bytes == 0x10u32.to_le_bytes())
        .filter_map(|(offset, _)| parse_embedded_tim_at(decoded, offset).ok())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn compare_embedded_tim(
    offset: usize,
    source_layer: Option<&DecodedLayer>,
    patched_layer: Option<&DecodedLayer>,
    source_tim: Option<&EmbeddedTimAudit>,
    patched_tim: Option<&EmbeddedTimAudit>,
    output_dir: &Path,
    preview_dir: &str,
) -> Result<EmbeddedTimComparison> {
    let source_bytes = tim_bytes(source_layer, source_tim)?;
    let patched_bytes = tim_bytes(patched_layer, patched_tim)?;
    let status = comparison_status(source_bytes, patched_bytes);
    let tim_byte_difference = match (source_bytes, patched_bytes) {
        (Some(source), Some(patched)) if status == ComparisonStatus::Changed => {
            Some(byte_difference_summary(source, patched))
        }
        _ => None,
    };

    let mut preview_errors = Vec::new();
    let source_rgba = decode_preview("source", source_layer, source_tim, &mut preview_errors);
    let patched_rgba = decode_preview("patched", patched_layer, patched_tim, &mut preview_errors);
    let source_indexed = decode_indexed("source", source_layer, source_tim, &mut preview_errors);
    let patched_indexed =
        decode_indexed("patched", patched_layer, patched_tim, &mut preview_errors);
    let source_palette_words =
        decode_palette_words("source", source_layer, source_tim, &mut preview_errors);
    let patched_palette_words =
        decode_palette_words("patched", patched_layer, patched_tim, &mut preview_errors);

    let source_file = format!("{preview_dir}/tim-{offset:08x}-source.png");
    let patched_file = format!("{preview_dir}/tim-{offset:08x}-patched.png");
    let difference_file = format!("{preview_dir}/tim-{offset:08x}-difference.png");
    let (source_preview_file, source_preview_sha256) =
        write_optional_preview(output_dir, &source_file, source_rgba.as_ref())?;
    let (patched_preview_file, patched_preview_sha256) =
        write_optional_preview(output_dir, &patched_file, patched_rgba.as_ref())?;
    let difference = indexed_difference_image(source_indexed.as_ref(), patched_indexed.as_ref());
    let (difference_preview_file, difference_preview_sha256) =
        write_optional_preview(output_dir, &difference_file, difference.as_ref())?;

    Ok(EmbeddedTimComparison {
        offset,
        status,
        source: source_tim.map(embedded_tim_identity),
        patched: patched_tim.map(embedded_tim_identity),
        tim_byte_difference,
        indexed_pixel_difference_count: pixel_difference_count(
            source_indexed.as_ref(),
            patched_indexed.as_ref(),
        ),
        clut_word_difference_count: palette_word_difference_count(
            source_palette_words.as_deref(),
            patched_palette_words.as_deref(),
        ),
        preview_palette_index: source_tim
            .or(patched_tim)
            .and_then(|tim| (tim.palette_count > 0).then_some(0)),
        palette_zero_rgba_difference_count: rgba_difference_count(
            source_rgba.as_ref(),
            patched_rgba.as_ref(),
        ),
        source_preview_file,
        source_preview_sha256,
        patched_preview_file,
        patched_preview_sha256,
        difference_preview_file,
        difference_preview_sha256,
        preview_error: (!preview_errors.is_empty()).then(|| preview_errors.join("; ")),
    })
}

fn decode_palette_words(
    label: &str,
    layer: Option<&DecodedLayer>,
    tim: Option<&EmbeddedTimAudit>,
    errors: &mut Vec<String>,
) -> Option<Vec<u16>> {
    let (Some(layer), Some(tim)) = (layer, tim) else {
        return None;
    };
    let result = (|| -> Result<Vec<u16>> {
        if tim.palette_count == 0 {
            return Ok(Vec::new());
        }
        let mut words = Vec::with_capacity(
            tim.palette_count
                * match tim.bits_per_pixel {
                    4 => 16,
                    8 => 256,
                    bits => bail!("unsupported embedded TIM depth {bits}"),
                },
        );
        for palette_index in 0..tim.palette_count {
            match tim.bits_per_pixel {
                4 => words.extend_from_slice(&read_4bpp_palette_words_in_prefix(
                    &layer.bytes,
                    tim.offset,
                    palette_index,
                )?),
                8 => words.extend_from_slice(&read_8bpp_palette_words_in_prefix(
                    &layer.bytes,
                    tim.offset,
                    palette_index,
                )?),
                bits => bail!("unsupported embedded TIM depth {bits}"),
            }
        }
        Ok(words)
    })();
    match result {
        Ok(words) => Some(words),
        Err(error) => {
            errors.push(format!("{label} CLUT: {error}"));
            None
        }
    }
}

fn palette_word_difference_count(source: Option<&[u16]>, patched: Option<&[u16]>) -> Option<usize> {
    let (Some(source), Some(patched)) = (source, patched) else {
        return None;
    };
    let shared = source
        .iter()
        .zip(patched)
        .filter(|(left, right)| left != right)
        .count();
    Some(shared + source.len().abs_diff(patched.len()))
}

fn tim_bytes<'a>(
    layer: Option<&'a DecodedLayer>,
    tim: Option<&EmbeddedTimAudit>,
) -> Result<Option<&'a [u8]>> {
    let (Some(layer), Some(tim)) = (layer, tim) else {
        return Ok(None);
    };
    Ok(Some(
        layer
            .bytes
            .get(tim.offset..tim.offset + tim.total_size)
            .context("parsed TIM exceeds its decoded layer")?,
    ))
}

fn decode_preview(
    label: &str,
    layer: Option<&DecodedLayer>,
    tim: Option<&EmbeddedTimAudit>,
    errors: &mut Vec<String>,
) -> Option<RgbaImage> {
    let (Some(layer), Some(tim)) = (layer, tim) else {
        return None;
    };
    match decode_embedded_tim_preview(&layer.bytes, tim) {
        Ok(image) => Some(image),
        Err(error) => {
            errors.push(format!("{label} RGBA: {error}"));
            None
        }
    }
}

fn decode_indexed(
    label: &str,
    layer: Option<&DecodedLayer>,
    tim: Option<&EmbeddedTimAudit>,
    errors: &mut Vec<String>,
) -> Option<IndexedPixels> {
    let (Some(layer), Some(tim)) = (layer, tim) else {
        return None;
    };
    match read_embedded_tim_indices(&layer.bytes, tim) {
        Ok(image) => Some(image),
        Err(error) => {
            errors.push(format!("{label} indexed: {error}"));
            None
        }
    }
}

fn read_embedded_tim_indices(decoded: &[u8], tim: &EmbeddedTimAudit) -> Result<IndexedPixels> {
    let pixels = match tim.bits_per_pixel {
        4 if tim.palette_count == 0 => read_indexed_cell_without_clut_in_prefix(
            decoded,
            tim.offset,
            Cell {
                x: 0,
                y: 0,
                width: tim.pixel_width,
                height: tim.pixel_height,
            },
        )?,
        4 => read_4bpp_indexed_image_in_prefix(decoded, tim.offset)?.pixels,
        8 => read_8bpp_indexed_cell_in_prefix(
            decoded,
            tim.offset,
            Cell {
                x: 0,
                y: 0,
                width: tim.pixel_width,
                height: tim.pixel_height,
            },
        )?,
        bits => bail!("unsupported TIM depth {bits}"),
    };
    ensure!(
        pixels.len() == tim.pixel_width * tim.pixel_height,
        "TIM indexed pixel count differs from its geometry"
    );
    Ok(IndexedPixels {
        width: tim.pixel_width,
        height: tim.pixel_height,
        pixels,
    })
}

fn indexed_difference_image(
    source: Option<&IndexedPixels>,
    patched: Option<&IndexedPixels>,
) -> Option<RgbaImage> {
    if source.is_none() && patched.is_none() {
        return None;
    }
    let width = source
        .map(|image| image.width)
        .unwrap_or(0)
        .max(patched.map(|image| image.width).unwrap_or(0));
    let height = source
        .map(|image| image.height)
        .unwrap_or(0)
        .max(patched.map(|image| image.height).unwrap_or(0));
    if width == 0 || height == 0 {
        return None;
    }
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let left = indexed_pixel(source, x, y);
            let right = indexed_pixel(patched, x, y);
            let color = match (left, right) {
                (Some(left), Some(right)) if left == right => {
                    let value = 28u8.saturating_add(right / 2);
                    [value, value, value, 255]
                }
                (Some(_), Some(_)) => [255, 32, 180, 255],
                (Some(_), None) => [255, 64, 64, 255],
                (None, Some(_)) => [64, 255, 128, 255],
                (None, None) => [0, 0, 0, 0],
            };
            pixels.extend_from_slice(&color);
        }
    }
    Some(RgbaImage {
        width,
        height,
        pixels,
    })
}

fn indexed_pixel(image: Option<&IndexedPixels>, x: usize, y: usize) -> Option<u8> {
    let image = image?;
    (x < image.width && y < image.height).then(|| image.pixels[y * image.width + x])
}

fn pixel_difference_count(
    source: Option<&IndexedPixels>,
    patched: Option<&IndexedPixels>,
) -> Option<usize> {
    difference_count(
        source.map(|image| (image.width, image.height, image.pixels.as_slice())),
        patched.map(|image| (image.width, image.height, image.pixels.as_slice())),
        1,
    )
}

fn rgba_difference_count(source: Option<&RgbaImage>, patched: Option<&RgbaImage>) -> Option<usize> {
    difference_count(
        source.map(|image| (image.width, image.height, image.pixels.as_slice())),
        patched.map(|image| (image.width, image.height, image.pixels.as_slice())),
        4,
    )
}

fn difference_count(
    source: Option<(usize, usize, &[u8])>,
    patched: Option<(usize, usize, &[u8])>,
    stride: usize,
) -> Option<usize> {
    if source.is_none() && patched.is_none() {
        return None;
    }
    let width = source
        .map(|image| image.0)
        .unwrap_or(0)
        .max(patched.map(|image| image.0).unwrap_or(0));
    let height = source
        .map(|image| image.1)
        .unwrap_or(0)
        .max(patched.map(|image| image.1).unwrap_or(0));
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            let left = image_pixel(source, x, y, stride);
            let right = image_pixel(patched, x, y, stride);
            if left != right {
                count += 1;
            }
        }
    }
    Some(count)
}

fn image_pixel(
    image: Option<(usize, usize, &[u8])>,
    x: usize,
    y: usize,
    stride: usize,
) -> Option<&[u8]> {
    let (width, height, pixels) = image?;
    if x >= width || y >= height {
        return None;
    }
    let offset = (y * width + x) * stride;
    Some(&pixels[offset..offset + stride])
}

fn write_optional_preview(
    output_dir: &Path,
    relative_path: &str,
    image: Option<&RgbaImage>,
) -> Result<(Option<String>, Option<String>)> {
    let Some(image) = image else {
        return Ok((None, None));
    };
    let path = output_dir.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    write_tim_preview(&path, image)?;
    Ok((Some(relative_path.to_string()), Some(sha256_file(&path)?)))
}

fn embedded_tim_identity(tim: &EmbeddedTimAudit) -> EmbeddedTimIdentity {
    EmbeddedTimIdentity {
        bits_per_pixel: tim.bits_per_pixel,
        total_size: tim.total_size,
        tim_sha256: tim.source_tim_sha256.clone(),
        pixel_width: tim.pixel_width,
        pixel_height: tim.pixel_height,
        image_vram_word_x: tim.image_vram_word_x,
        image_vram_y: tim.image_vram_y,
        clut_vram_x: tim.clut_vram_x,
        clut_vram_y: tim.clut_vram_y,
        palette_count: tim.palette_count,
    }
}

fn comparison_status(source: Option<&[u8]>, patched: Option<&[u8]>) -> ComparisonStatus {
    match (source, patched) {
        (Some(source), Some(patched)) if source == patched => ComparisonStatus::Unchanged,
        (Some(_), Some(_)) => ComparisonStatus::Changed,
        (Some(_), None) => ComparisonStatus::SourceOnly,
        (None, Some(_)) => ComparisonStatus::PatchedOnly,
        (None, None) => unreachable!("comparison has no source or patched value"),
    }
}

fn byte_difference_summary(source: &[u8], patched: &[u8]) -> ByteDifferenceSummary {
    let common_length = source.len().min(patched.len());
    let mut changed_byte_count = 0;
    let mut ranges = Vec::new();
    let mut range_start = None;
    for offset in 0..common_length {
        if source[offset] != patched[offset] {
            changed_byte_count += 1;
            range_start.get_or_insert(offset);
        } else if let Some(start) = range_start.take() {
            ranges.push([start, offset]);
        }
    }
    if let Some(start) = range_start.take() {
        ranges.push([start, common_length]);
    }
    if source.len() != patched.len() {
        changed_byte_count += source.len().abs_diff(patched.len());
        let maximum_length = source.len().max(patched.len());
        if let Some(last) = ranges.last_mut()
            && last[1] == common_length
        {
            last[1] = maximum_length;
        } else {
            ranges.push([common_length, maximum_length]);
        }
    }
    let changed_range_count = ranges.len();
    ranges.truncate(MAX_REPORTED_DIFFERENCE_RANGES);
    ByteDifferenceSummary {
        changed_byte_count,
        changed_range_count,
        ranges_truncated: changed_range_count > ranges.len(),
        changed_byte_ranges_preview: ranges,
    }
}

fn count_records(records: &[DiscRecordComparison], status: ComparisonStatus) -> usize {
    records
        .iter()
        .filter(|record| record.status == status)
        .count()
}

fn write_report(output_dir: &Path, report: &DiscAssetComparisonReport) -> Result<()> {
    let path = output_dir.join(REPORT_FILE);
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    ensure_safe_output_path(output_dir)?;
    let marker = output_dir.join(OUTPUT_MARKER_FILE);
    if output_dir.exists() {
        if !force {
            bail!(
                "disc asset comparison output exists; pass --force to replace it: {}",
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
            "refusing to replace output directory with an unknown ownership marker: {}",
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
        "comparison output must name a dedicated directory"
    );
    Ok(())
}

fn path_slug(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            output.push('-');
            previous_separator = true;
        }
    }
    output.trim_matches('-').to_string()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn canonical_path_string(path: &Path) -> Result<String> {
    Ok(path_string(&std::fs::canonicalize(path).with_context(
        || format!("failed to resolve {}", path.display()),
    )?))
}

#[cfg(test)]
#[path = "disc_asset_comparison_tests.rs"]
mod tests;
