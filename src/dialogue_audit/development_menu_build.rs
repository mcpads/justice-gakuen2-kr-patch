use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedRecordWritePlan;
use crate::disc::rebuild::DiscRecordSourceIdentity;
use crate::menu_atlas::require_proven_shared_atlas_writes;
use crate::menu_compression::{MenuCompressionProfile, compress_menu_with_source_limits};
use crate::mode_select::{MODE_SELECT_MENU_PATH, ModeSelectRecordBuild};
use crate::options::OptionsRecordBuild;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::title_menu::TitleMenuRecordBuild;
use crate::title_notice::TitleNoticeBuild;

pub(super) struct DevelopmentMenuBuild {
    pub(super) source: DiscRecordSourceIdentity,
    pub(super) decoded: Vec<u8>,
    pub(super) stored: Vec<u8>,
    pub(super) decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) source_compression: MenuCompressionProfile,
    pub(super) rebuilt_compression: MenuCompressionProfile,
    pub(super) unpadded_stored_size: usize,
}

pub(super) fn build_development_menu(
    mode_select: &ModeSelectRecordBuild,
    options: &OptionsRecordBuild,
    title_menu: &TitleMenuRecordBuild,
    title_notice: &TitleNoticeBuild,
    shared_menu_atlas: &crate::menu_atlas::presentation::SharedMenuAtlasBuild,
) -> Result<DevelopmentMenuBuild> {
    require_proven_shared_atlas_writes(
        "Options glyph",
        options
            .report
            .glyphs
            .iter()
            .filter(|glyph| glyph.global_menu_resident)
            .count(),
        options.report.allocation_proven_reclaimable,
    )?;
    require_proven_shared_atlas_writes(
        "title-adjacent menu glyph",
        title_menu
            .report
            .glyphs
            .iter()
            .filter(|glyph| glyph.global_menu_resident)
            .count(),
        title_menu.report.allocation_proven_reclaimable,
    )?;
    require_proven_shared_atlas_writes(
        "title-notice glyph",
        title_notice.report.global_menu_write_count,
        false,
    )?;
    let source_stored = &options.source_menu_stored;
    let source_decoded = &options.source_menu_decoded;
    let source_stored_sha256 = sha256_bytes(source_stored);
    let source_decoded_sha256 = sha256_bytes(source_decoded);

    for (owner, reported_sha256) in [
        (
            "MODE SELECT",
            mode_select.report.source_menu_decoded_sha256.as_str(),
        ),
        (
            "options",
            options.report.source_menu_decoded_sha256.as_str(),
        ),
        (
            "title-adjacent menu",
            title_menu.report.source_menu_decoded_sha256.as_str(),
        ),
        (
            "title notice",
            title_notice.report.source_menu_decoded_sha256.as_str(),
        ),
    ] {
        ensure!(
            reported_sha256 == source_decoded_sha256,
            "{owner} candidate consumed a different source MENU.BIZ decoded identity"
        );
    }
    for (owner, reported_sha256) in [
        (
            "MODE SELECT",
            mode_select.report.source_menu_stored_sha256.as_str(),
        ),
        ("options", options.report.source_menu_stored_sha256.as_str()),
        (
            "title-adjacent menu",
            title_menu.report.source_menu_stored_sha256.as_str(),
        ),
    ] {
        ensure!(
            reported_sha256 == source_stored_sha256,
            "{owner} candidate consumed a different source MENU.BIZ stored identity"
        );
    }
    ensure!(
        title_menu.source_menu_stored == *source_stored
            && title_menu.source_menu_decoded == *source_decoded,
        "title-adjacent menu retained different source MENU.BIZ bytes"
    );

    let mut plan = DecodedRecordWritePlan::new(
        MODE_SELECT_MENU_PATH,
        source_decoded,
        &source_decoded_sha256,
    )?;
    plan.register_data_candidate(
        "MODE SELECT builder",
        &source_decoded_sha256,
        &mode_select.menu_decoded,
        &mode_select.menu_write_claims,
    )?;
    plan.register_data_candidate(
        "options builder",
        &source_decoded_sha256,
        &options.menu_decoded,
        &options.menu_write_claims,
    )?;
    if !title_menu.menu_write_claims.is_empty() {
        plan.register_data_candidate(
            "title-adjacent menu builder",
            &source_decoded_sha256,
            &title_menu.menu_decoded,
            &title_menu.menu_write_claims,
        )?;
    }
    if !title_notice.menu_write_claims.is_empty() {
        plan.register_data_candidate(
            "title notice builder",
            &source_decoded_sha256,
            &title_notice.menu_decoded,
            &title_notice.menu_write_claims,
        )?;
    }
    shared_menu_atlas.register(&mut plan)?;
    let decoded = plan.apply(None)?;
    let decoded_changed_byte_ranges = difference_ranges(source_decoded, &decoded);

    let (reencoded, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&decoded, source_stored)?;
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "development MENU compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source_stored.len(),
        "development MENU.BIZ exceeds its original record"
    );
    ensure!(
        reencoded[..4] == source_stored[..4],
        "development MENU compression changed the catalog prefix"
    );
    let unpadded_stored_size = reencoded.len();
    let mut stored = reencoded;
    stored.resize(source_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == decoded,
        "padded development MENU.BIZ changed decoded bytes"
    );
    Ok(DevelopmentMenuBuild {
        source: DiscRecordSourceIdentity::from_bytes(source_stored),
        decoded,
        stored,
        decoded_changed_byte_ranges,
        source_compression,
        rebuilt_compression,
        unpadded_stored_size,
    })
}
