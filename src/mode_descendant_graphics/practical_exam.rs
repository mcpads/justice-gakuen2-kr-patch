use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::tim::Cell;
use crate::write_scope::changed_ranges_are_within;

#[path = "practical_exam/active_title.rs"]
mod active_title;
#[path = "practical_exam/catalog.rs"]
mod catalog;
#[path = "practical_exam/descriptor.rs"]
mod descriptor;
#[path = "practical_exam/gameplay_hud.rs"]
pub(super) mod gameplay_hud;
#[path = "practical_exam/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "practical_exam/primitive_pool.rs"]
mod primitive_pool;
#[path = "practical_exam/primitive_schedule.rs"]
mod primitive_schedule;
#[path = "practical_exam/renderer.rs"]
mod renderer;
#[path = "practical_exam/stream.rs"]
mod stream;

pub(super) use active_title::install_active_title_texture_cache;
use active_title::{
    ACTIVE_TITLE_START_SLOT, PracticalExamActiveTitleBuilderReport, install_active_title_builder,
};
pub(super) use catalog::PracticalExamTranslation;
use catalog::{PracticalExamCatalogEntry, bind_translations};
pub(super) use descriptor::{
    PracticalExamConsumer, PracticalExamSecondaryTextureBank,
    audit_practical_exam_direct_texture_census, audit_practical_exam_secondary_descriptors,
    external_secondary_descriptors_avoid_source_cell, parse_practical_exam_secondary_descriptor,
};
use descriptor::{
    PracticalExamConsumerAudit, audit_practical_exam_consumer,
    practical_exam_consumer_source_sha256,
};
pub(super) use glyph_atlas::PracticalExamActiveTitleCachePlan;
use glyph_atlas::{
    PracticalExamGlyphAtlasPlan, PracticalExamSemanticBuild, PracticalExamSemanticPlacementBuild,
    build_practical_exam_glyph_atlas,
};
use primitive_schedule::{
    PracticalExamPrimitiveScheduleReport, install_practical_exam_primitive_schedule,
};
use renderer::{PracticalExamRendererInstallReport, install_compact_glyph_renderer};
use stream::{
    DescriptorGlyphStream, GlyphCoordinate, PracticalExamStreamPack,
    pack_practical_exam_glyph_streams,
};

use super::catalog::{PRACTICAL_1999_TEXTURE_PATH, PRACTICAL_BASICS_TEXTURE_PATH};
use super::model::{ModeDescendantFontRole, ModeDescendantFontSources};

pub(super) struct PracticalExamSharedUiBuild {
    pub(super) basics_texture_decoded: Vec<u8>,
    pub(super) exam_1999_texture_decoded: Vec<u8>,
    pub(super) basics_consumer_overlay: Vec<u8>,
    pub(super) exam_1999_consumer_overlay: Vec<u8>,
    pub(super) active_title_cache: PracticalExamActiveTitleCachePlan,
    pub(super) report: PracticalExamSharedUiReport,
}

pub(super) struct PracticalExamSharedUiReport {
    pub(super) semantic_entry_count: usize,
    pub(super) rendered_text_count: usize,
    pub(super) rewritten_descriptor_count: usize,
    pub(super) basics_glyph_count: usize,
    pub(super) exam_1999_glyph_count: usize,
    pub(super) basics_stream_size: usize,
    pub(super) exam_1999_stream_size: usize,
    pub(super) basics_coordinate_table_size: usize,
    pub(super) exam_1999_coordinate_table_size: usize,
    pub(super) basics_renderer_instruction_count: usize,
    pub(super) exam_1999_renderer_instruction_count: usize,
    pub(super) basics_renderer_call_count: usize,
    pub(super) exam_1999_renderer_call_count: usize,
    pub(super) basics_primitive_schedule_write_count: usize,
    pub(super) exam_1999_primitive_schedule_write_count: usize,
    pub(super) basics_primitive_schedule_instruction_count: usize,
    pub(super) exam_1999_primitive_schedule_instruction_count: usize,
    pub(super) active_title_cache_glyph_count: usize,
    pub(super) active_title_cache_covers_term_titles: bool,
    pub(super) active_title_builder_verified: bool,
    pub(super) active_title_max_sprite_count: usize,
    pub(super) active_title_primitive_pool_capacity: usize,
    pub(super) basics_texture_sha256: String,
    pub(super) exam_1999_texture_sha256: String,
    pub(super) basics_consumer_sha256: String,
    pub(super) exam_1999_consumer_sha256: String,
    pub(super) texts: Vec<PracticalExamTextBuild>,
}

pub(super) struct PracticalExamTextBuild {
    pub(super) id: &'static str,
    pub(super) korean_text: String,
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) measured_advance_px: f32,
    pub(super) atlas_advance_px: usize,
    pub(super) glyph_count: usize,
    pub(super) consumer_count: usize,
    pub(super) cells: Vec<Cell>,
    pub(super) changed_decoded_byte_count: usize,
}

struct BuiltPracticalExamConsumer {
    audit: PracticalExamConsumerAudit,
    atlas: PracticalExamGlyphAtlasPlan,
    overlay: Vec<u8>,
    stream_size: usize,
    coordinate_table_size: usize,
    renderer: PracticalExamRendererInstallReport,
    primitive_schedule: PracticalExamPrimitiveScheduleReport,
    active_title_builder: Option<PracticalExamActiveTitleBuilderReport>,
    active_title_max_sprite_count: usize,
}

pub(super) fn build_practical_exam_shared_ui(
    basics_texture_source: &[u8],
    exam_1999_texture_source: &[u8],
    basics_consumer_source: &[u8],
    exam_1999_consumer_source: &[u8],
    translations: &[PracticalExamTranslation],
    fonts: &ModeDescendantFontSources,
) -> Result<PracticalExamSharedUiBuild> {
    let bound = bind_translations(translations)?;
    let basics_bound = bound_for_consumer(&bound, PracticalExamConsumer::BasicsReview);
    let exam_1999_bound = bound_for_consumer(&bound, PracticalExamConsumer::Exam1999);

    let basics = build_consumer(
        basics_texture_source,
        basics_consumer_source,
        PracticalExamConsumer::BasicsReview,
        &basics_bound,
        fonts,
    )?;
    let exam_1999 = build_consumer(
        exam_1999_texture_source,
        exam_1999_consumer_source,
        PracticalExamConsumer::Exam1999,
        &exam_1999_bound,
        fonts,
    )?;
    let texts = merge_semantic_reports(&bound, &basics.atlas, &exam_1999.atlas)?;
    ensure!(
        texts.len() == translations.len(),
        "practical-exam glyph pipeline omitted a translated semantic entry"
    );

    let active_title_cache = exam_1999
        .atlas
        .active_title_cache
        .clone()
        .context("1999 practical-exam atlas omitted its active-title cache")?;
    let active_title_builder_verified = exam_1999
        .active_title_builder
        .as_ref()
        .context("1999 practical-exam consumer omitted its active-title bridge")?
        .typed_readback_verified;
    let report = PracticalExamSharedUiReport {
        semantic_entry_count: bound.len(),
        rendered_text_count: texts.len(),
        rewritten_descriptor_count: basics.audit.descriptors.len()
            + exam_1999.audit.descriptors.len(),
        basics_glyph_count: basics.atlas.glyph_count,
        exam_1999_glyph_count: exam_1999.atlas.glyph_count,
        basics_stream_size: basics.stream_size,
        exam_1999_stream_size: exam_1999.stream_size,
        basics_coordinate_table_size: basics.coordinate_table_size,
        exam_1999_coordinate_table_size: exam_1999.coordinate_table_size,
        basics_renderer_instruction_count: basics.renderer.core_instruction_count
            + basics.renderer.nop_padding_instruction_count,
        exam_1999_renderer_instruction_count: exam_1999.renderer.core_instruction_count
            + exam_1999.renderer.nop_padding_instruction_count,
        basics_renderer_call_count: basics.primitive_schedule.renderer_call_count,
        exam_1999_renderer_call_count: exam_1999.primitive_schedule.renderer_call_count,
        basics_primitive_schedule_write_count: basics
            .primitive_schedule
            .readback
            .planned_instruction_count,
        exam_1999_primitive_schedule_write_count: exam_1999
            .primitive_schedule
            .readback
            .planned_instruction_count,
        basics_primitive_schedule_instruction_count: basics
            .primitive_schedule
            .readback
            .typed_instruction_readback_count,
        exam_1999_primitive_schedule_instruction_count: exam_1999
            .primitive_schedule
            .readback
            .typed_instruction_readback_count,
        active_title_cache_glyph_count: active_title_cache.glyphs.len(),
        active_title_cache_covers_term_titles: active_title_cache.covers_all_term_titles,
        active_title_builder_verified,
        active_title_max_sprite_count: exam_1999.active_title_max_sprite_count,
        active_title_primitive_pool_capacity: exam_1999.primitive_schedule.logical_slot_capacity,
        basics_texture_sha256: sha256_bytes(&basics.atlas.patched_decoded),
        exam_1999_texture_sha256: sha256_bytes(&exam_1999.atlas.patched_decoded),
        basics_consumer_sha256: sha256_bytes(&basics.overlay),
        exam_1999_consumer_sha256: sha256_bytes(&exam_1999.overlay),
        texts,
    };

    Ok(PracticalExamSharedUiBuild {
        basics_texture_decoded: basics.atlas.patched_decoded,
        exam_1999_texture_decoded: exam_1999.atlas.patched_decoded,
        basics_consumer_overlay: basics.overlay,
        exam_1999_consumer_overlay: exam_1999.overlay,
        active_title_cache,
        report,
    })
}

fn build_consumer(
    texture_source: &[u8],
    overlay_source: &[u8],
    consumer: PracticalExamConsumer,
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    fonts: &ModeDescendantFontSources,
) -> Result<BuiltPracticalExamConsumer> {
    let audit = audit_practical_exam_consumer(overlay_source, consumer)?;
    let atlas = build_practical_exam_glyph_atlas(
        practical_exam_texture_path(consumer),
        texture_source,
        &audit,
        bound,
        fonts,
    )?;
    ensure!(
        atlas.consumer == consumer
            && atlas.glyph_count == atlas.glyphs.len()
            && atlas.semantics.len() == bound.len(),
        "{consumer:?} practical-exam atlas report is incomplete"
    );
    let (streams, coordinates) = build_stream_inputs(&audit, &atlas)?;
    let active_title_max_sprite_count =
        active_title_max_sprite_count(consumer, &audit, &atlas, &streams)?;
    let packed = pack_practical_exam_glyph_streams(
        consumer,
        overlay_source,
        &audit,
        &streams,
        &coordinates,
    )?;
    let PracticalExamStreamPack {
        patched_overlay: stream_candidate,
        coordinate_table_runtime_address,
        changed_ranges,
        owned_ranges,
        stream_size,
        coordinate_table_size,
    } = packed;
    ensure!(
        !changed_ranges.is_empty(),
        "{consumer:?} practical-exam stream pack changed no bytes"
    );
    let mut renderer_candidate = overlay_source.to_vec();
    let renderer = install_compact_glyph_renderer(
        &mut renderer_candidate,
        consumer,
        coordinate_table_runtime_address,
    )?;
    let mut primitive_schedule_candidate = overlay_source.to_vec();
    let primitive_schedule = install_practical_exam_primitive_schedule(
        overlay_source,
        &mut primitive_schedule_candidate,
        consumer,
        &streams,
    )?;
    ensure!(
        primitive_schedule.readback.verified
            && primitive_schedule.maximum_batch_slot_count
                <= primitive_schedule.logical_slot_capacity
            && primitive_schedule.primitive_pool_footprint_end
                <= primitive_schedule.next_fixed_primitive_offset,
        "{consumer:?} practical-exam primitive schedule failed its typed readback"
    );
    ensure!(
        ACTIVE_TITLE_START_SLOT + active_title_max_sprite_count
            <= primitive_schedule.logical_slot_capacity,
        "{consumer:?} practical-exam active title exceeds the primitive pool"
    );
    let (active_title_builder_candidate, active_title_builder) =
        if consumer == PracticalExamConsumer::Exam1999 {
            let (candidate, report) = install_active_title_builder(overlay_source)?;
            (Some(candidate), Some(report))
        } else {
            (None, None)
        };
    let patched_overlay = compose_consumer_writes(PracticalExamConsumerWrites {
        source: overlay_source,
        consumer,
        stream_candidate: &stream_candidate,
        stream_ranges: &owned_ranges,
        renderer_candidate: &renderer_candidate,
        renderer: &renderer,
        primitive_schedule_candidate: &primitive_schedule_candidate,
        primitive_schedule: &primitive_schedule,
        active_title_builder_candidate: active_title_builder_candidate.as_deref(),
        active_title_builder: active_title_builder.as_ref(),
    })?;
    ensure_consumer_changes_are_owned(
        overlay_source,
        &patched_overlay,
        &owned_ranges,
        &renderer,
        &primitive_schedule,
        active_title_builder.as_ref(),
    )?;
    Ok(BuiltPracticalExamConsumer {
        audit,
        atlas,
        overlay: patched_overlay,
        stream_size,
        coordinate_table_size,
        renderer,
        primitive_schedule,
        active_title_builder,
        active_title_max_sprite_count,
    })
}

fn active_title_max_sprite_count(
    consumer: PracticalExamConsumer,
    audit: &PracticalExamConsumerAudit,
    atlas: &PracticalExamGlyphAtlasPlan,
    streams: &[DescriptorGlyphStream],
) -> Result<usize> {
    if consumer != PracticalExamConsumer::Exam1999 {
        return Ok(0);
    }
    let title_descriptor_indices = atlas
        .semantics
        .iter()
        .filter(|semantic| {
            matches!(
                semantic.id,
                "practical_first_term_exam"
                    | "practical_second_term_exam"
                    | "practical_school_year_exam"
            )
        })
        .map(|semantic| semantic.descriptor_index)
        .collect::<BTreeSet<_>>();
    ensure!(
        title_descriptor_indices == BTreeSet::from([5, 6, 7]) && audit.descriptors.len() > 7,
        "1999 practical-exam active-title states lost their descriptor bindings"
    );
    let maximum = streams
        .iter()
        .filter(|stream| title_descriptor_indices.contains(&stream.descriptor_index))
        .map(|stream| stream.codes.len())
        .max()
        .context("1999 practical-exam active-title streams are missing")?;
    ensure!(
        maximum > 0,
        "1999 practical-exam active-title stream is empty"
    );
    // The gameplay title uses one native sprite; compact streams serve menus.
    Ok(1)
}

struct PracticalExamConsumerWrites<'a> {
    source: &'a [u8],
    consumer: PracticalExamConsumer,
    stream_candidate: &'a [u8],
    stream_ranges: &'a [[usize; 2]],
    renderer_candidate: &'a [u8],
    renderer: &'a PracticalExamRendererInstallReport,
    primitive_schedule_candidate: &'a [u8],
    primitive_schedule: &'a PracticalExamPrimitiveScheduleReport,
    active_title_builder_candidate: Option<&'a [u8]>,
    active_title_builder: Option<&'a PracticalExamActiveTitleBuilderReport>,
}

fn compose_consumer_writes(inputs: PracticalExamConsumerWrites<'_>) -> Result<Vec<u8>> {
    let PracticalExamConsumerWrites {
        source,
        consumer,
        stream_candidate,
        stream_ranges,
        renderer_candidate,
        renderer,
        primitive_schedule_candidate,
        primitive_schedule,
        active_title_builder_candidate,
        active_title_builder,
    } = inputs;
    let target = practical_exam_consumer_path(consumer);
    let id_prefix = practical_exam_consumer_id(consumer);
    let mut machine_sources = PsxMachineCodeSources::default();
    let mut plan = DecodedRecordWritePlan::new(
        target,
        source,
        practical_exam_consumer_source_sha256(consumer),
    )?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "practical-exam compact glyph streams",
        source_sha256: practical_exam_consumer_source_sha256(consumer),
        candidate: stream_candidate,
        claims: stream_ranges
            .iter()
            .enumerate()
            .map(|(index, &[start, end])| CandidateWriteClaim {
                id: format!("{id_prefix}:glyph-streams:{index}"),
                purpose: "store compact glyph streams, coordinates, and pointers".to_string(),
                range: start..end,
                intent: WriteIntent::Data,
            })
            .collect(),
    })?;

    if let (Some(candidate), Some(builder)) = (active_title_builder_candidate, active_title_builder)
    {
        let source_id = format!("{id_prefix}:active-title-builder");
        let provenance = machine_sources.register(
            source_id.clone(),
            builder.runtime_address,
            builder.installed_instructions.clone(),
        )?;
        let mut claims = vec![CandidateWriteClaim {
            id: source_id,
            purpose: "retain the native gameplay title primitive and draw its complete raster"
                .to_string(),
            range: builder.allowed_range.clone(),
            intent: WriteIntent::MachineCode(provenance),
        }];
        claims.extend(builder.data_ranges.iter().enumerate().map(|(index, range)| CandidateWriteClaim {
            id: format!("{id_prefix}:native-hud-descriptors:{index}"),
            purpose: "bind the prefix, separator and complete decimal alphabet to the shared gameplay atlas".to_string(),
            range: range.clone(),
            intent: WriteIntent::Data,
        }));
        plan.register_candidate(CandidateRecordWrite {
            owner: "practical-exam native gameplay HUD",
            source_sha256: practical_exam_consumer_source_sha256(consumer),
            candidate,
            claims,
        })?;
    } else {
        ensure!(
            active_title_builder_candidate.is_none() && active_title_builder.is_none(),
            "practical-exam active-title builder candidate and report diverged"
        );
    }

    let renderer_source_id = format!("{id_prefix}:compact-glyph-renderer");
    let renderer_provenance = machine_sources.register(
        renderer_source_id.clone(),
        renderer.renderer_runtime_address,
        renderer.installed_instructions.clone(),
    )?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "practical-exam compact glyph renderer",
        source_sha256: practical_exam_consumer_source_sha256(consumer),
        candidate: renderer_candidate,
        claims: vec![CandidateWriteClaim {
            id: renderer_source_id,
            purpose: "render compact glyph streams through typed R3000A code".to_string(),
            range: renderer.allowed_range.clone(),
            intent: WriteIntent::MachineCode(renderer_provenance),
        }],
    })?;

    let mut primitive_claims = Vec::with_capacity(primitive_schedule.instruction_writes.len());
    for (index, write) in primitive_schedule.instruction_writes.iter().enumerate() {
        let source_id = format!("{id_prefix}:primitive-schedule:{index}");
        let provenance = machine_sources.register(
            source_id.clone(),
            write.runtime_address,
            vec![write.replacement_instruction.clone()],
        )?;
        primitive_claims.push(CandidateWriteClaim {
            id: source_id,
            purpose: format!("schedule practical-exam primitive write {:?}", write.role),
            range: write.offset..write.offset + 4,
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: "practical-exam primitive schedule",
        source_sha256: practical_exam_consumer_source_sha256(consumer),
        candidate: primitive_schedule_candidate,
        claims: primitive_claims,
    })?;

    plan.apply(Some(&machine_sources))
}

fn practical_exam_consumer_id(consumer: PracticalExamConsumer) -> &'static str {
    match consumer {
        PracticalExamConsumer::BasicsReview => "practical-basics",
        PracticalExamConsumer::Exam1999 => "practical-1999",
    }
}

fn practical_exam_consumer_path(consumer: PracticalExamConsumer) -> &'static str {
    match consumer {
        PracticalExamConsumer::BasicsReview => "DAT1/SIKEN.BIN",
        PracticalExamConsumer::Exam1999 => "DAT1/SIKEN2.BIN",
    }
}

fn practical_exam_texture_path(consumer: PracticalExamConsumer) -> &'static str {
    match consumer {
        PracticalExamConsumer::BasicsReview => PRACTICAL_BASICS_TEXTURE_PATH,
        PracticalExamConsumer::Exam1999 => PRACTICAL_1999_TEXTURE_PATH,
    }
}

fn bound_for_consumer<'a>(
    bound: &'a [(
        &'static PracticalExamCatalogEntry,
        &'a PracticalExamTranslation,
    )],
    consumer: PracticalExamConsumer,
) -> Vec<(
    &'static PracticalExamCatalogEntry,
    &'a PracticalExamTranslation,
)> {
    bound
        .iter()
        .copied()
        .filter(|(catalog, _)| {
            catalog
                .bindings
                .iter()
                .any(|binding| binding.consumer == consumer)
        })
        .collect()
}

fn build_stream_inputs(
    audit: &PracticalExamConsumerAudit,
    atlas: &PracticalExamGlyphAtlasPlan,
) -> Result<(Vec<DescriptorGlyphStream>, Vec<GlyphCoordinate>)> {
    let mut coordinate_slots = vec![None; atlas.glyph_count];
    for (key, glyph) in &atlas.glyphs {
        let slot = coordinate_slots
            .get_mut(usize::from(glyph.code))
            .context("practical-exam atlas emitted a code outside its local table")?;
        ensure!(
            slot.replace(GlyphCoordinate {
                cell: glyph.cell,
                title_height: key.0 == ModeDescendantFontRole::PracticalTitle,
            })
            .is_none(),
            "practical-exam atlas emitted duplicate local glyph code {}",
            glyph.code
        );
    }
    let mut coordinates = coordinate_slots
        .into_iter()
        .enumerate()
        .map(|(code, coordinate)| {
            coordinate.with_context(|| format!("practical-exam local glyph code {code} is missing"))
        })
        .collect::<Result<Vec<_>>>()?;

    let hint_code = u8::try_from(coordinates.len())?;
    coordinates.push(GlyphCoordinate {
        cell: atlas.hint.cell,
        title_height: false,
    });
    let mut streams = Vec::with_capacity(audit.descriptors.len());
    let mut bound_descriptors = BTreeSet::new();
    for semantic in &atlas.semantics {
        ensure!(
            bound_descriptors.insert(semantic.descriptor_index),
            "practical-exam descriptor {} has duplicate semantic streams",
            semantic.descriptor_index
        );
        // These streams serve menu descriptors. Active gameplay has a native
        // complete-title sprite and must not substitute its grayscale cache
        // for the selected menu font or menu palette.
        let codes = match &semantic.placement {
            PracticalExamSemanticPlacementBuild::GlyphStream { glyph_codes, .. } => {
                glyph_codes.clone()
            }
            PracticalExamSemanticPlacementBuild::GuardedHintComposite { cell, .. } => {
                ensure!(
                    *cell == atlas.hint.cell,
                    "practical-exam hint stream changed its composite cell"
                );
                vec![hint_code]
            }
        };
        streams.push(DescriptorGlyphStream {
            descriptor_index: semantic.descriptor_index,
            codes,
        });
    }

    for descriptor in &audit.descriptors {
        if bound_descriptors.contains(&descriptor.index) {
            continue;
        }
        let mut codes = Vec::with_capacity(descriptor.fragments.len());
        for fragment in &descriptor.fragments {
            let cell = fragment.cell();
            ensure!(
                cell.x < 512 && cell.width <= 32 && cell.height == 20,
                "practical-exam retained descriptor {} cannot use the compact coordinate grammar",
                descriptor.index
            );
            let code = u8::try_from(coordinates.len())?;
            coordinates.push(GlyphCoordinate {
                cell,
                title_height: false,
            });
            codes.push(code);
        }
        streams.push(DescriptorGlyphStream {
            descriptor_index: descriptor.index,
            codes,
        });
    }
    ensure!(
        streams.len() == audit.descriptors.len(),
        "practical-exam compact stream inputs omitted a primary descriptor"
    );
    let (streams, coordinates) = compact_referenced_coordinates(&mut streams, coordinates)?;
    for semantic in &atlas.semantics {
        if let PracticalExamSemanticPlacementBuild::GlyphStream { glyph_keys, .. } =
            &semantic.placement
        {
            let stream = streams
                .iter()
                .find(|s| s.descriptor_index == semantic.descriptor_index)
                .context("authored menu descriptor lost its stream")?;
            ensure!(
                stream.codes.len() == glyph_keys.len()
                    && stream
                        .codes
                        .iter()
                        .zip(glyph_keys)
                        .all(|(code, key)| coordinates[usize::from(*code)].cell
                            == atlas.glyphs[key].cell),
                "practical menu {} does not consume its selected role glyphs",
                semantic.id
            );
        }
    }
    Ok((streams, coordinates))
}

fn compact_referenced_coordinates(
    streams: &mut [DescriptorGlyphStream],
    coordinates: Vec<GlyphCoordinate>,
) -> Result<(Vec<DescriptorGlyphStream>, Vec<GlyphCoordinate>)> {
    let referenced = streams
        .iter()
        .flat_map(|stream| stream.codes.iter().copied())
        .map(usize::from)
        .collect::<BTreeSet<_>>();
    ensure!(
        !referenced.is_empty(),
        "practical-exam streams reference no coordinates"
    );
    let mut remap = vec![None; coordinates.len()];
    let mut compact = Vec::with_capacity(referenced.len());
    for old_code in referenced {
        let coordinate = coordinates
            .get(old_code)
            .copied()
            .with_context(|| format!("practical-exam stream references missing code {old_code}"))?;
        let new_code = u8::try_from(compact.len())?;
        remap[old_code] = Some(new_code);
        compact.push(coordinate);
    }
    for stream in streams.iter_mut() {
        for code in &mut stream.codes {
            *code = remap[usize::from(*code)]
                .context("practical-exam coordinate compaction omitted a referenced code")?;
        }
    }
    Ok((streams.to_vec(), compact))
}

fn merge_semantic_reports(
    bound: &[(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )],
    basics: &PracticalExamGlyphAtlasPlan,
    exam_1999: &PracticalExamGlyphAtlasPlan,
) -> Result<Vec<PracticalExamTextBuild>> {
    let mut texts = Vec::with_capacity(bound.len());
    for (catalog, translation) in bound {
        let mut physical = Vec::new();
        for plan in [basics, exam_1999] {
            if let Some(semantic) = plan
                .semantics
                .iter()
                .find(|semantic| semantic.id == catalog.id)
            {
                physical.push((plan, semantic));
            }
        }
        ensure!(
            !physical.is_empty(),
            "practical-exam semantic {} has no physical consumer",
            catalog.id
        );
        let first = physical[0].1;
        ensure!(
            physical.iter().all(|(_, semantic)| {
                semantic.korean_text == translation.korean_text
                    && semantic.font_role == translation.font_role
                    && semantic.font_name == first.font_name
                    && semantic.font_sha256 == first.font_sha256
                    && semantic.font_px == first.font_px
                    && semantic.vertical_shift_px == first.vertical_shift_px
                    && semantic.atlas_advance_px == first.atlas_advance_px
            }),
            "practical-exam semantic {} diverged across physical consumers",
            catalog.id
        );

        let mut cells = Vec::new();
        let mut changed_decoded_byte_count = 0usize;
        let glyph_count = semantic_glyph_count(first);
        for (plan, semantic) in &physical {
            ensure!(
                semantic_glyph_count(semantic) == glyph_count,
                "practical-exam semantic {} changed glyph count across consumers",
                catalog.id
            );
            match &semantic.placement {
                PracticalExamSemanticPlacementBuild::GlyphStream { glyph_keys, .. } => {
                    let unique = glyph_keys.iter().copied().collect::<BTreeSet<_>>();
                    for key in glyph_keys {
                        cells.push(
                            plan.glyphs
                                .get(key)
                                .with_context(|| {
                                    format!(
                                        "practical-exam semantic {} lost glyph {key:?}",
                                        catalog.id
                                    )
                                })?
                                .cell,
                        );
                    }
                    changed_decoded_byte_count += unique
                        .iter()
                        .map(|key| plan.glyphs[key].changed_decoded_byte_count)
                        .sum::<usize>();
                }
                PracticalExamSemanticPlacementBuild::GuardedHintComposite { cell, .. } => {
                    cells.push(*cell);
                    changed_decoded_byte_count += plan.hint.changed_decoded_byte_count;
                }
            }
        }
        ensure!(
            !cells.is_empty(),
            "practical-exam semantic {} has no runtime cells",
            catalog.id
        );
        texts.push(PracticalExamTextBuild {
            id: catalog.id,
            korean_text: translation.korean_text.clone(),
            font_role: translation.font_role,
            font_name: first.font_name.clone(),
            font_sha256: first.font_sha256.clone(),
            font_px: first.font_px,
            vertical_shift_px: first.vertical_shift_px,
            measured_advance_px: first.measured_advance_px,
            atlas_advance_px: first.atlas_advance_px,
            glyph_count,
            consumer_count: physical.len(),
            cells,
            changed_decoded_byte_count,
        });
    }
    Ok(texts)
}

fn semantic_glyph_count(semantic: &PracticalExamSemanticBuild) -> usize {
    match &semantic.placement {
        PracticalExamSemanticPlacementBuild::GlyphStream { glyph_codes, .. } => glyph_codes.len(),
        PracticalExamSemanticPlacementBuild::GuardedHintComposite { .. } => 1,
    }
}

fn ensure_consumer_changes_are_owned(
    source: &[u8],
    patched: &[u8],
    stream_ranges: &[[usize; 2]],
    renderer: &PracticalExamRendererInstallReport,
    primitive_schedule: &PracticalExamPrimitiveScheduleReport,
    active_title_builder: Option<&PracticalExamActiveTitleBuilderReport>,
) -> Result<()> {
    let mut allowed = stream_ranges.to_vec();
    allowed.push([renderer.allowed_range.start, renderer.allowed_range.end]);
    allowed.extend_from_slice(&primitive_schedule.expected_write_ranges);
    if let Some(builder) = active_title_builder {
        allowed.push([builder.allowed_range.start, builder.allowed_range.end]);
        allowed.extend(
            builder
                .data_ranges
                .iter()
                .map(|range| [range.start, range.end]),
        );
    }
    let changed = difference_ranges(source, patched);
    ensure!(
        !changed.is_empty() && changed_ranges_are_within(&changed, &allowed),
        "{:?} practical-exam consumer changed bytes outside compact streams, renderer, and primitive schedule",
        renderer.consumer
    );
    ensure!(
        renderer.installed_byte_count == 0x01f4
            && renderer.core_byte_count + renderer.nop_padding_instruction_count * 4
                == renderer.installed_byte_count,
        "{:?} practical-exam renderer report changed its guarded extent",
        renderer.consumer
    );
    Ok(())
}
