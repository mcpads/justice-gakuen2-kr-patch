//! Builds every auxiliary texture record selected by the physical allocation plan.

use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::model::{
    CharacterSelectAuxiliaryRecordBuild, CharacterSelectCompressedStreamBuild,
    CharacterSelectFixedStripBuild, CharacterSelectGlyphBuild,
    CharacterSelectSourceInkCleanupAllocation, CharacterSelectSourceInkCleanupBuild,
};
use super::planned_texture_targets::planned_tim_offsets;
use super::preview::build_target_tim_reports;
use super::record_build::{TextureContributions, TextureRecordSource, build_texture_record};
use super::render::{RenderedFixedStrip, RenderedGlyph};
use super::source::CharacterSelectAuxiliarySourceRecord;

pub(super) struct AuxiliaryTextureRecordBuild {
    pub(super) stored: Vec<u8>,
    pub(super) report: CharacterSelectAuxiliaryRecordBuild,
    pub(super) glyphs: Vec<CharacterSelectGlyphBuild>,
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripBuild>,
    pub(super) source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupBuild>,
}

pub(super) fn build_auxiliary_texture_records(
    output_dir: &Path,
    sources: &[CharacterSelectAuxiliarySourceRecord],
    rendered_glyphs: &[RenderedGlyph],
    rendered_fixed_strips: &[RenderedFixedStrip],
    source_ink_cleanups: &[CharacterSelectSourceInkCleanupAllocation],
    plain_loading: Option<&crate::loading_art::PlainLoadingBuild>,
    diagnosis: &crate::cooperative_diagnosis::DiagnosisComponent,
) -> Result<Vec<AuxiliaryTextureRecordBuild>> {
    let mut builds = Vec::new();
    for source in sources {
        let mut target_tim_offsets = planned_tim_offsets(
            source.path,
            rendered_glyphs,
            rendered_fixed_strips,
            source_ink_cleanups,
        );
        if source.path == "DAT2/AISYOU.TIZ" {
            target_tim_offsets.insert(0);
            target_tim_offsets.insert(0x20800);
        }
        if target_tim_offsets.is_empty() {
            continue;
        }
        let product = build_texture_record(
            TextureRecordSource {
                path: source.path,
                stored: &source.stored,
                decoded: &source.decoded,
                compression_streams: &source.compression_streams,
            },
            rendered_glyphs,
            rendered_fixed_strips,
            source_ink_cleanups,
            TextureContributions {
                plain_loading,
                diagnosis: Some(diagnosis),
                ..Default::default()
            },
        )?;
        let target_tims = build_target_tim_reports(
            output_dir,
            source.output_file,
            &source.decoded,
            &product.patched_decoded,
            target_tim_offsets,
        )?;
        ensure!(
            !product.glyphs.is_empty()
                || !product.fixed_strips.is_empty()
                || !product.source_ink_cleanups.is_empty(),
            "{} auxiliary target was selected without a rendered allocation",
            source.path
        );
        let report = CharacterSelectAuxiliaryRecordBuild {
            source_path: source.path.to_string(),
            output_file: source.output_file.to_string(),
            source_stored_sha256: product.source_stored_sha256.clone(),
            source_decoded_sha256: product.source_decoded_sha256.clone(),
            patched_stored_sha256: product.patched_stored_sha256.clone(),
            patched_decoded_sha256: product.patched_decoded_sha256.clone(),
            source_record_size: product.source_record_size,
            changed_stored_byte_ranges: product.changed_stored_byte_ranges.clone(),
            compressed_streams: product
                .streams
                .iter()
                .map(|stream| CharacterSelectCompressedStreamBuild {
                    stream_index: stream.stream_index,
                    decoded_range: stream.decoded_range,
                    source_decoded_sha256: stream.source_decoded_sha256.clone(),
                    patched_decoded_sha256: stream.patched_decoded_sha256.clone(),
                    changed: stream.changed,
                    unpadded_stored_size: stream.unpadded_stored_size,
                    encoded_stream_offset: stream.encoded_stream_offset,
                    encoded_stream_capacity: stream.encoded_stream_capacity,
                })
                .collect(),
            target_tims,
        };
        let output_path = output_dir.join(&report.output_file);
        std::fs::write(&output_path, &product.stored)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        builds.push(AuxiliaryTextureRecordBuild {
            stored: product.stored,
            report,
            glyphs: product.glyphs,
            fixed_strips: product.fixed_strips,
            source_ink_cleanups: product.source_ink_cleanups,
        });
    }
    ensure!(
        plain_loading.is_none()
            || builds
                .iter()
                .any(|build| build.report.source_path == "DAT2/OVER.TIZ"),
        "plain loading consumers require a composed OVER texture record"
    );
    Ok(builds)
}
