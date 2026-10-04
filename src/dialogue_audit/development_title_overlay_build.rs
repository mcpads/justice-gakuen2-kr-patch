use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::contextual_texture_upload::ContextualMenuGlyphUploadReport;
use crate::decoded_record_write_plan::DecodedRecordWritePlan;
use crate::disc::rebuild::DiscRecordSourceIdentity;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::title_menu::{
    TITLE_MENU_OVERLAY_PATH, TitleMenuRecordBuild, compress_title_overlay_with_source_limits,
};
use crate::title_notice::TitleNoticeBuild;
use crate::title_overlay_runtime::install_title_glyph_upload;

pub(super) struct DevelopmentTitleOverlayBuild {
    pub(super) source: DiscRecordSourceIdentity,
    pub(super) decoded: Vec<u8>,
    pub(super) stored: Vec<u8>,
    pub(super) decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) continue_names:
        crate::title_overlay_runtime::continue_names::ContinueNameRuntimeReport,
    pub(super) runtime_glyph_upload: ContextualMenuGlyphUploadReport,
}

pub(super) fn build_development_title_overlay(
    title_menu: &TitleMenuRecordBuild,
    title_notice: &TitleNoticeBuild,
    name_entry: &super::name_entry_composed_build::ComposedNameInputBuild,
    materialization: &crate::name_input::NameGlyphMaterializationBundle,
    menu_atlas_plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<DevelopmentTitleOverlayBuild> {
    let source_stored = &title_menu.source_overlay_stored;
    let source_decoded = &title_menu.source_overlay_decoded;
    let source_stored_sha256 = sha256_bytes(source_stored);
    let source_decoded_sha256 = sha256_bytes(source_decoded);
    ensure!(
        source_stored_sha256 == title_menu.report.source_overlay_stored_sha256,
        "title-adjacent menu retained a different source MGTIT.BIZ stored identity"
    );
    for (owner, reported_sha256) in [
        (
            "title-adjacent menu",
            title_menu.report.source_overlay_decoded_sha256.as_str(),
        ),
        (
            "title notice",
            title_notice.report.source_overlay_decoded_sha256.as_str(),
        ),
    ] {
        ensure!(
            reported_sha256 == source_decoded_sha256,
            "{owner} candidate consumed a different source MGTIT.BIZ decoded identity"
        );
    }
    ensure!(
        title_notice.source_overlay_decoded == *source_decoded,
        "title notice retained different source MGTIT.BIZ bytes"
    );

    let mut plan = DecodedRecordWritePlan::new(
        TITLE_MENU_OVERLAY_PATH,
        source_decoded,
        &source_decoded_sha256,
    )?;
    plan.register_data_candidate(
        "title-adjacent menu builder",
        &source_decoded_sha256,
        &title_menu.overlay_text_decoded,
        &title_menu.overlay_text_write_claims,
    )?;
    plan.register_data_candidate(
        "title notice builder",
        &source_decoded_sha256,
        &title_notice.overlay_decoded,
        &title_notice.overlay_write_claims,
    )?;
    let mut decoded = plan.apply(None)?;
    let contextual_glyphs = title_menu
        .contextual_glyphs
        .iter()
        .chain(title_notice.contextual_glyphs.iter())
        .cloned()
        .collect::<Vec<_>>();
    let runtime_upload =
        install_title_glyph_upload(source_decoded, &mut decoded, &contextual_glyphs)?;
    let occupied = runtime_upload
        .report
        .entries
        .iter()
        .map(|e| e.vram_rect)
        .collect::<Vec<_>>();
    let continue_names = crate::title_overlay_runtime::continue_names::install(
        source_decoded,
        &mut decoded,
        runtime_upload.storage_end.next_multiple_of(4),
        &name_entry.font_stored,
        &name_entry.report.font.runtime_atlas,
        materialization,
        &name_entry.direct_glyphs,
        &occupied,
        menu_atlas_plan,
    )?;
    let decoded_changed_byte_ranges = difference_ranges(source_decoded, &decoded);

    let (reencoded, _, _) = compress_title_overlay_with_source_limits(&decoded, source_stored)?;
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "development MGTIT compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source_stored.len(),
        "development MGTIT.BIZ exceeds its original record"
    );
    let mut stored = reencoded;
    stored.resize(source_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == decoded,
        "padded development MGTIT.BIZ changed decoded bytes"
    );
    Ok(DevelopmentTitleOverlayBuild {
        source: DiscRecordSourceIdentity::from_bytes(source_stored),
        decoded,
        stored,
        decoded_changed_byte_ranges,
        runtime_glyph_upload: runtime_upload.report,
        continue_names,
    })
}
