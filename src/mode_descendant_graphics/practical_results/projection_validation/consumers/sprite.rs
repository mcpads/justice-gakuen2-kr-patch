//! Direct sprite and numeric lookup consumers.

use anyhow::{Result, ensure};

use super::super::super::projection_model::*;
use super::super::geometry::validate_digit_geometry;
use super::super::source_evidence::{ValidationCounts, address_and_span, address_only};

pub(super) fn validate_direct_renderer(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultDirectSpriteRendererEvidence,
    counts: &mut ValidationCounts,
) -> Result<()> {
    address_and_span(
        overlay,
        base,
        &evidence.function_offset,
        &evidence.function_runtime_address,
        evidence.function_size,
        &evidence.function_sha256,
        "direct renderer",
        counts,
    )?;
    for (role, offset, address) in [
        (
            "texture-page setup",
            &evidence.texture_page_setup_offset,
            &evidence.texture_page_setup_runtime_address,
        ),
        (
            "GetTPage call",
            &evidence.get_tpage_call_offset,
            &evidence.get_tpage_call_runtime_address,
        ),
        (
            "GetClut call",
            &evidence.get_clut_call_offset,
            &evidence.get_clut_call_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    Ok(())
}

pub(super) fn validate_direct_numeric_site(
    overlay: &[u8],
    base: u32,
    site: &PracticalResultDirectNumericSpriteSite,
    counts: &mut ValidationCounts,
) -> Result<()> {
    match site {
        PracticalResultDirectNumericSpriteSite::Geometry(site) => {
            for (role, offset, address) in [
                (
                    "numeric texture-page setup",
                    &site.texture_page_setup_offset,
                    &site.texture_page_setup_runtime_address,
                ),
                (
                    "numeric GetTPage call",
                    &site.get_tpage_call_offset,
                    &site.get_tpage_call_runtime_address,
                ),
                (
                    "numeric GetClut call",
                    &site.get_clut_call_offset,
                    &site.get_clut_call_runtime_address,
                ),
            ] {
                address_only(overlay, base, offset, address, role, counts)?;
            }
            address_and_span(
                overlay,
                base,
                &site.geometry_table_offset,
                &site.geometry_table_runtime_address,
                site.geometry_table_size,
                &site.geometry_table_sha256,
                "numeric geometry table",
                counts,
            )?;
            ensure!(
                !site.source_rects.is_empty()
                    && site
                        .source_rects
                        .iter()
                        .all(|rect| rect.width > 0 && rect.height > 0),
                "numeric geometry site has invalid rectangles"
            );
        }
        PracticalResultDirectNumericSpriteSite::Lookup(site) => {
            for (role, offset, address) in [
                (
                    "numeric texture-page setup",
                    &site.texture_page_setup_offset,
                    &site.texture_page_setup_runtime_address,
                ),
                (
                    "numeric GetTPage call",
                    &site.get_tpage_call_offset,
                    &site.get_tpage_call_runtime_address,
                ),
                (
                    "numeric GetClut call",
                    &site.get_clut_call_offset,
                    &site.get_clut_call_runtime_address,
                ),
            ] {
                address_only(overlay, base, offset, address, role, counts)?;
            }
            address_and_span(
                overlay,
                base,
                &site.lookup_table_offset,
                &site.lookup_table_runtime_address,
                site.lookup_table_size,
                &site.lookup_table_sha256,
                "numeric lookup table",
                counts,
            )?;
            validate_digit_geometry(&site.source_geometry)?;
        }
    }
    Ok(())
}
