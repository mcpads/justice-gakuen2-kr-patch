//! Applies a rendered plan to one compressed texture record under Expected Write guards.

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::paged_compression::{
    compress_page_safe_image_with_seeded_control_block_in_slot, source_paged_compression_profile,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::write_scope::changed_ranges_are_within;

use super::compressed_record::CompressedTextureStream;
use super::model::{
    CharacterSelectFixedStripBuild, CharacterSelectGlyphBuild,
    CharacterSelectSourceInkCleanupAllocation, CharacterSelectSourceInkCleanupBuild,
};
use super::render::{RenderedFixedStrip, RenderedGlyph};
use super::texture_surface_build::{build_texture_surface, targeted_surfaces};

pub(super) struct TextureRecordSource<'a> {
    pub(super) path: &'a str,
    pub(super) stored: &'a [u8],
    pub(super) decoded: &'a [u8],
    pub(super) compression_streams: &'a [CompressedTextureStream],
}

pub(super) struct TextureRecordStreamBuild {
    pub(super) stream_index: usize,
    pub(super) decoded_range: [usize; 2],
    pub(super) source_decoded_sha256: String,
    pub(super) patched_decoded_sha256: String,
    pub(super) changed: bool,
    pub(super) unpadded_stored_size: usize,
    pub(super) encoded_stream_offset: usize,
    pub(super) encoded_stream_capacity: usize,
}

pub(super) struct TextureRecordBuild {
    pub(super) stored: Vec<u8>,
    pub(super) patched_decoded: Vec<u8>,
    pub(super) source_stored_sha256: String,
    pub(super) source_decoded_sha256: String,
    pub(super) patched_stored_sha256: String,
    pub(super) patched_decoded_sha256: String,
    pub(super) source_record_size: usize,
    pub(super) changed_stored_byte_ranges: Vec<[usize; 2]>,
    pub(super) streams: Vec<TextureRecordStreamBuild>,
    pub(super) native_text: Option<serde_json::Value>,
    pub(super) glyphs: Vec<CharacterSelectGlyphBuild>,
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripBuild>,
    pub(super) source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupBuild>,
}

#[derive(Default)]
pub(super) struct TextureContributions<'a> {
    pub(super) plain_loading: Option<&'a crate::loading_art::PlainLoadingBuild>,
    pub(super) diagnosis: Option<&'a crate::cooperative_diagnosis::DiagnosisComponent>,
    pub(super) settings: Option<&'a crate::system_settings_text::SystemSettingsText>,
    pub(super) logo: Option<&'a super::small_logo::SmallLogo>,
    pub(super) selector_runtime: Option<&'a crate::name_input::SelectorRuntimeInstallation>,
    pub(super) roster_runtime: Option<&'a crate::name_input::RosterRuntimeInstallation>,
    pub(super) prompt_text_font: Option<&'a super::model::CharacterSelectFontStyle>,
    pub(super) native_text_font: Option<&'a crate::development_build_spec::SizedFontSource>,
}

pub(super) fn build_texture_record(
    source: TextureRecordSource<'_>,
    rendered: &[RenderedGlyph],
    rendered_fixed_strips: &[RenderedFixedStrip],
    source_ink_cleanups: &[CharacterSelectSourceInkCleanupAllocation],
    contributions: TextureContributions<'_>,
) -> Result<TextureRecordBuild> {
    validate_compression_streams(source.path, source.decoded, source.compression_streams)?;
    let source_decoded_sha256 = sha256_bytes(source.decoded);
    let surfaces = targeted_surfaces(
        source.path,
        rendered,
        rendered_fixed_strips,
        source_ink_cleanups,
    );
    ensure!(
        !surfaces.is_empty(),
        "{} has no planned character-select texture surfaces",
        source.path
    );
    let mut write_plan =
        DecodedRecordWritePlan::new(source.path, source.decoded, &source_decoded_sha256)?;
    let mut glyphs = Vec::with_capacity(rendered.len());
    let mut fixed_strips = Vec::with_capacity(rendered_fixed_strips.len());
    let mut applied_source_ink_cleanups = Vec::with_capacity(source_ink_cleanups.len());
    for surface in surfaces {
        let surface_build = build_texture_surface(
            source.path,
            source.decoded,
            surface,
            rendered,
            rendered_fixed_strips,
            source_ink_cleanups,
        )?;
        let surface_id = format!("{surface:?}");
        let claims = DecodedDataClaim::from_effective_ranges(
            &format!("character-select-surface:{}:{surface_id}", source.path),
            &format!("compose character-select texture surface {surface_id}"),
            source.decoded,
            &surface_build.candidate,
            surface_build.allowed_ranges,
        )?;
        if !claims.is_empty() {
            let owner = format!("character-select texture-surface producer: {surface_id}");
            write_plan.register_data_candidate(
                &owner,
                &source_decoded_sha256,
                &surface_build.candidate,
                &claims,
            )?;
        }
        glyphs.extend(surface_build.glyphs);
        fixed_strips.extend(surface_build.fixed_strips);
        applied_source_ink_cleanups.extend(surface_build.source_ink_cleanups);
    }
    if source.path == "DAT2/OVER.TIZ"
        && let Some(loading) = contributions.plain_loading
    {
        loading.register_texture(&mut write_plan)?;
    }
    if source.path == "DAT2/AISYOU.TIZ"
        && let Some(diagnosis) = contributions.diagnosis
    {
        diagnosis.register_texture(source.decoded, &mut write_plan)?;
    }
    if source.path == "DAT2/SELP1.BIZ"
        && let Some(settings) = contributions.settings
    {
        settings.register_texture(source.decoded, &mut write_plan)?;
    }
    if let Some(logo) = contributions.logo {
        logo.register(source.path, source.decoded, &mut write_plan)?;
    }
    if source.path == "DAT2/SELP1.BIZ"
        && let Some(runtime) = contributions.selector_runtime
    {
        runtime.register_staging(source.decoded, &mut write_plan)?;
    }
    if let Some(runtime) = contributions.roster_runtime {
        runtime.register_staging(source.decoded, &mut write_plan)?;
    }
    let native_text = if let Some(style) = contributions.native_text_font {
        ensure!(
            matches!(
                source.path,
                "DAT2/SELP1.BIZ"
                    | "DAT2/SELP2.BIZ"
                    | "DAT2/SELP3.BIZ"
                    | "DAT2/SELP4.BIZ"
                    | "DAT2/SELP5.BIZ"
            ),
            "native text font supplied to an unbound character-select record"
        );
        let mut candidate = source.decoded.to_vec();
        let (claims, report) = crate::menu_atlas::native_text::apply_selection_text(
            source.decoded,
            &mut candidate,
            super::texture_targets::SHARED_ATLAS_OFFSET,
            style,
            contributions
                .prompt_text_font
                .ok_or_else(|| anyhow::anyhow!("native selection text requires its prompt font"))?,
        )?;
        write_plan.register_data_candidate(
            "character-select native text glyphs",
            &source_decoded_sha256,
            &candidate,
            &claims,
        )?;
        Some(report)
    } else {
        None
    };
    let patched = write_plan.apply(None)?;
    if native_text.is_some() {
        super::native_regions::validate_retained(source.decoded, &patched)?;
    }
    ensure!(
        patched != source.decoded,
        "{} character-select texture plan changed no bytes",
        source.path
    );
    let mut stored = source.stored.to_vec();
    let mut stream_builds = Vec::with_capacity(source.compression_streams.len());
    let mut allowed_stored_ranges = Vec::new();
    for stream in source.compression_streams {
        let [decoded_start, decoded_end] = stream.decoded_range;
        let source_stream_decoded = &source.decoded[decoded_start..decoded_end];
        let patched_stream_decoded = &patched[decoded_start..decoded_end];
        let source_encoded = stream.storage.source_encoded(source.stored)?;
        let source_slot = stream.storage.source_slot(source.stored)?;
        let source_profile = source_paged_compression_profile(source_encoded)?;
        stream
            .storage
            .validate_stream_size(source_profile.stream_byte_count)?;
        ensure!(
            source_slot[source_profile.stream_byte_count..]
                .iter()
                .all(|byte| *byte == 0),
            "{} encoded stream {} has non-padding bytes after its terminator",
            source.path,
            stream.stream_index
        );
        let changed = source_stream_decoded != patched_stream_decoded;
        let unpadded_stored_size = if changed {
            let source_control_block = first_complete_control_block(source_encoded)?;
            let reencoded = compress_page_safe_image_with_seeded_control_block_in_slot(
                patched_stream_decoded,
                source_profile,
                source_control_block,
                stream.storage.slot_capacity(),
            )?;
            ensure!(
                reencoded[..4] == source_encoded[..4],
                "{} rebuilt stream {} changed its catalog prefix",
                source.path,
                stream.stream_index
            );
            ensure!(
                decompress(&reencoded, false)? == patched_stream_decoded,
                "{} rebuilt stream {} failed compression roundtrip",
                source.path,
                stream.stream_index
            );
            allowed_stored_ranges.extend(stream.storage.owned_stored_ranges());
            stored = stream.storage.rebuild(&stored, &reencoded)?;
            reencoded.len()
        } else {
            source_profile.stream_byte_count
        };
        let rebuilt_slot = stream.storage.source_slot(&stored)?;
        ensure!(
            decompress(rebuilt_slot, true)? == patched_stream_decoded,
            "{} padded rebuilt stream {} changed decoded bytes",
            source.path,
            stream.stream_index
        );
        stream_builds.push(TextureRecordStreamBuild {
            stream_index: stream.stream_index,
            decoded_range: stream.decoded_range,
            source_decoded_sha256: sha256_bytes(source_stream_decoded),
            patched_decoded_sha256: sha256_bytes(patched_stream_decoded),
            changed,
            unpadded_stored_size,
            encoded_stream_offset: stream.storage.stream_offset(),
            encoded_stream_capacity: stream.storage.slot_capacity(),
        });
    }
    let changed_stored_byte_ranges = difference_ranges(source.stored, &stored);
    ensure!(
        !changed_stored_byte_ranges.is_empty()
            && changed_ranges_are_within(&changed_stored_byte_ranges, &allowed_stored_ranges),
        "{} rebuilt compressed record changed bytes outside its owned stream slots",
        source.path
    );
    Ok(TextureRecordBuild {
        source_stored_sha256: sha256_bytes(source.stored),
        source_decoded_sha256: sha256_bytes(source.decoded),
        patched_stored_sha256: sha256_bytes(&stored),
        patched_decoded_sha256: sha256_bytes(&patched),
        source_record_size: source.stored.len(),
        changed_stored_byte_ranges,
        streams: stream_builds,
        native_text,
        stored,
        patched_decoded: patched,
        glyphs,
        fixed_strips,
        source_ink_cleanups: applied_source_ink_cleanups,
    })
}

fn validate_compression_streams(
    path: &str,
    decoded: &[u8],
    streams: &[CompressedTextureStream],
) -> Result<()> {
    ensure!(
        !streams.is_empty(),
        "{path} has no compressed texture streams"
    );
    let mut expected_start = 0;
    let mut stream_indices = std::collections::BTreeSet::new();
    for stream in streams {
        let [start, end] = stream.decoded_range;
        ensure!(
            start == expected_start
                && start < end
                && end <= decoded.len()
                && stream_indices.insert(stream.stream_index),
            "{path} has an invalid compressed texture stream layout"
        );
        expected_start = end;
    }
    ensure!(
        expected_start == decoded.len(),
        "{path} compressed texture streams do not cover the decoded record"
    );
    Ok(())
}

pub(super) fn first_complete_control_block(stored: &[u8]) -> Result<&[u8]> {
    let mut offset = 0usize;
    let mut control = read_u16(stored, &mut offset)?;
    for _ in 0..16 {
        if control & 0x8000 != 0 {
            let token = read_u16(stored, &mut offset)?;
            if token >> 11 == 0 {
                let extended_length = read_u16(stored, &mut offset)?;
                ensure!(
                    token & 0x07ff != 0 || extended_length != 0,
                    "character-select source terminates inside its first control block"
                );
            }
        } else {
            read_u16(stored, &mut offset)?;
        }
        control <<= 1;
    }
    stored
        .get(..offset)
        .context("character-select first compression control block is truncated")
}

fn read_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    let bytes = data
        .get(*offset..*offset + 2)
        .context("character-select compression token is truncated")?;
    *offset += 2;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}
