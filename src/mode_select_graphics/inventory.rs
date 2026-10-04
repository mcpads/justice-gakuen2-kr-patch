use anyhow::{Result, ensure};

use crate::mode_select::source::{
    ATLAS_TIM_OFFSET, DESCRIPTION_TIM_FIRST_OFFSET, DESCRIPTION_TIM_STRIDE,
    MODE_ARTWORK_FIRST_OFFSET, MODE_ARTWORK_STRIDE, MODE_COUNT, MODE_PREVIEW_FIRST_OFFSET,
    MODE_PREVIEW_STRIDE, PANEL_INDEX_BY_MODE_INDEX,
};
use crate::pipeline::sha256_bytes;
use crate::tim::{parse_4bpp_prefix, parse_4bpp_without_clut_prefix, parse_8bpp_prefix};

use super::model::{ModeSelectTimAudit, ModeSelectTimRole};
use super::tim16::{Tim16bpp, parse_16bpp_prefix};

pub(super) struct TimInventorySummary {
    pub(super) embedded_tim_count: usize,
    pub(super) classified_tim_count: usize,
    pub(super) unclassified_tim_count: usize,
    pub(super) mode_artwork_count: usize,
    pub(super) mode_preview_count: usize,
    pub(super) observed_matching_mode_preview_count: Option<usize>,
}

pub(super) fn collect_tim_inventory(
    source_decoded: &[u8],
    observed_decoded: Option<&[u8]>,
) -> Result<Vec<ModeSelectTimAudit>> {
    let mut tims = detect_embedded_tims(source_decoded);
    ensure_known_populations(&tims)?;
    if let Some(observed) = observed_decoded {
        ensure!(
            observed.len() == source_decoded.len(),
            "observed MODE SELECT MENU.BIZ decoded size changed"
        );
        for tim in &mut tims {
            let observed_bytes = &observed[tim.offset..tim.offset + tim.total_size];
            let observed_sha256 = sha256_bytes(observed_bytes);
            tim.observed_matches_source = Some(observed_sha256 == tim.source_tim_sha256);
            tim.observed_tim_sha256 = Some(observed_sha256);
        }
    }
    Ok(tims)
}

pub(super) fn summarize_tim_inventory(
    tims: &[ModeSelectTimAudit],
    has_observed_menu: bool,
) -> TimInventorySummary {
    let classified_tim_count = tims
        .iter()
        .filter(|tim| tim.role != ModeSelectTimRole::Unclassified)
        .count();
    TimInventorySummary {
        embedded_tim_count: tims.len(),
        classified_tim_count,
        unclassified_tim_count: tims.len() - classified_tim_count,
        mode_artwork_count: count_role(tims, |role| {
            matches!(role, ModeSelectTimRole::ModeArtworkPanel { .. })
        }),
        mode_preview_count: count_role(tims, |role| {
            matches!(role, ModeSelectTimRole::ModePreviewPanel { .. })
        }),
        observed_matching_mode_preview_count: has_observed_menu.then(|| {
            tims.iter()
                .filter(|tim| matches!(tim.role, ModeSelectTimRole::ModePreviewPanel { .. }))
                .filter(|tim| tim.observed_matches_source == Some(true))
                .count()
        }),
    }
}

fn ensure_known_populations(tims: &[ModeSelectTimAudit]) -> Result<()> {
    ensure!(
        tims.iter().any(|tim| {
            tim.offset == ATLAS_TIM_OFFSET && tim.role == ModeSelectTimRole::SharedAtlas
        }),
        "MODE SELECT shared atlas disappeared from the TIM inventory"
    );
    ensure_role_population(tims, "description", |role| {
        matches!(role, ModeSelectTimRole::DescriptionPanel { .. })
    })?;
    ensure_role_population(tims, "mode artwork", |role| {
        matches!(role, ModeSelectTimRole::ModeArtworkPanel { .. })
    })?;
    ensure_role_population(tims, "mode preview", |role| {
        matches!(role, ModeSelectTimRole::ModePreviewPanel { .. })
    })
}

fn count_role(
    tims: &[ModeSelectTimAudit],
    predicate: impl Fn(&ModeSelectTimRole) -> bool,
) -> usize {
    tims.iter().filter(|tim| predicate(&tim.role)).count()
}

fn ensure_role_population(
    tims: &[ModeSelectTimAudit],
    name: &str,
    predicate: impl Fn(&ModeSelectTimRole) -> bool,
) -> Result<()> {
    ensure!(
        count_role(tims, predicate) == MODE_COUNT,
        "MODE SELECT {name} TIM inventory is incomplete"
    );
    Ok(())
}

pub(super) fn detect_embedded_tims(decoded: &[u8]) -> Vec<ModeSelectTimAudit> {
    let mut tims = Vec::new();
    let mut offset = 0usize;
    while offset + 8 <= decoded.len() {
        if decoded[offset..offset + 4] != 0x10u32.to_le_bytes() {
            offset += 1;
            continue;
        }
        let flags = u32::from_le_bytes(decoded[offset + 4..offset + 8].try_into().unwrap());
        let candidate = match flags {
            0x00 => parse_4bpp_without_clut_prefix(&decoded[offset..])
                .ok()
                .map(|tim| tim_audit_without_clut(decoded, offset, flags, tim)),
            0x02 => parse_16bpp_prefix(&decoded[offset..])
                .ok()
                .map(|tim| tim_audit_16bpp(decoded, offset, flags, tim)),
            0x08 => parse_4bpp_prefix(&decoded[offset..])
                .ok()
                .map(|tim| tim_audit_4bpp(decoded, offset, flags, tim)),
            0x09 => parse_8bpp_prefix(&decoded[offset..])
                .ok()
                .map(|tim| tim_audit_8bpp(decoded, offset, flags, tim)),
            _ => None,
        };
        if let Some(tim) = candidate {
            offset += tim.total_size;
            tims.push(tim);
        } else {
            offset += 1;
        }
    }
    tims
}

fn tim_audit_4bpp(
    decoded: &[u8],
    offset: usize,
    flags: u32,
    tim: crate::tim::Tim4bpp,
) -> ModeSelectTimAudit {
    new_tim_audit(
        decoded,
        offset,
        flags,
        TimShape {
            bits_per_pixel: 4,
            has_clut: true,
            total_size: tim.total_size,
            pixel_width: tim.pixel_width(),
            pixel_height: tim.image_height,
            image_vram_word_x: tim.image_x,
            image_vram_y: tim.image_y,
            palette_count: tim.clut_width * tim.clut_height / 16,
        },
    )
}

fn tim_audit_without_clut(
    decoded: &[u8],
    offset: usize,
    flags: u32,
    tim: crate::tim::Tim4bppWithoutClut,
) -> ModeSelectTimAudit {
    new_tim_audit(
        decoded,
        offset,
        flags,
        TimShape {
            bits_per_pixel: 4,
            has_clut: false,
            total_size: tim.total_size,
            pixel_width: tim.pixel_width(),
            pixel_height: tim.image_height,
            image_vram_word_x: tim.image_x,
            image_vram_y: tim.image_y,
            palette_count: 0,
        },
    )
}

fn tim_audit_8bpp(
    decoded: &[u8],
    offset: usize,
    flags: u32,
    tim: crate::tim::Tim8bpp,
) -> ModeSelectTimAudit {
    new_tim_audit(
        decoded,
        offset,
        flags,
        TimShape {
            bits_per_pixel: 8,
            has_clut: true,
            total_size: tim.total_size,
            pixel_width: tim.pixel_width(),
            pixel_height: tim.image_height,
            image_vram_word_x: tim.image_x,
            image_vram_y: tim.image_y,
            palette_count: tim.clut_width * tim.clut_height / 256,
        },
    )
}

fn tim_audit_16bpp(decoded: &[u8], offset: usize, flags: u32, tim: Tim16bpp) -> ModeSelectTimAudit {
    new_tim_audit(
        decoded,
        offset,
        flags,
        TimShape {
            bits_per_pixel: 16,
            has_clut: false,
            total_size: tim.total_size,
            pixel_width: tim.pixel_width,
            pixel_height: tim.pixel_height,
            image_vram_word_x: tim.image_x,
            image_vram_y: tim.image_y,
            palette_count: 0,
        },
    )
}

#[derive(Debug, Clone, Copy)]
struct TimShape {
    bits_per_pixel: u8,
    has_clut: bool,
    total_size: usize,
    pixel_width: usize,
    pixel_height: usize,
    image_vram_word_x: u16,
    image_vram_y: u16,
    palette_count: usize,
}

fn new_tim_audit(decoded: &[u8], offset: usize, flags: u32, shape: TimShape) -> ModeSelectTimAudit {
    ModeSelectTimAudit {
        offset,
        flags,
        bits_per_pixel: shape.bits_per_pixel,
        has_clut: shape.has_clut,
        total_size: shape.total_size,
        source_tim_sha256: sha256_bytes(&decoded[offset..offset + shape.total_size]),
        pixel_width: shape.pixel_width,
        pixel_height: shape.pixel_height,
        image_vram_word_x: shape.image_vram_word_x,
        image_vram_y: shape.image_vram_y,
        palette_count: shape.palette_count,
        role: classify_tim(offset),
        preview_file: String::new(),
        preview_sha256: String::new(),
        observed_tim_sha256: None,
        observed_matches_source: None,
    }
}

pub(super) fn classify_tim(offset: usize) -> ModeSelectTimRole {
    if offset == ATLAS_TIM_OFFSET {
        return ModeSelectTimRole::SharedAtlas;
    }
    if let Some((panel_index, mode_index)) =
        classify_panel_group(offset, DESCRIPTION_TIM_FIRST_OFFSET, DESCRIPTION_TIM_STRIDE)
    {
        return ModeSelectTimRole::DescriptionPanel {
            panel_index,
            mode_index,
        };
    }
    if let Some((panel_index, mode_index)) =
        classify_panel_group(offset, MODE_ARTWORK_FIRST_OFFSET, MODE_ARTWORK_STRIDE)
    {
        return ModeSelectTimRole::ModeArtworkPanel {
            panel_index,
            mode_index,
        };
    }
    if let Some((panel_index, mode_index)) =
        classify_panel_group(offset, MODE_PREVIEW_FIRST_OFFSET, MODE_PREVIEW_STRIDE)
    {
        return ModeSelectTimRole::ModePreviewPanel {
            panel_index,
            mode_index,
        };
    }
    ModeSelectTimRole::Unclassified
}

fn classify_panel_group(
    offset: usize,
    first_offset: usize,
    stride: usize,
) -> Option<(usize, usize)> {
    let relative = offset.checked_sub(first_offset)?;
    if !relative.is_multiple_of(stride) {
        return None;
    }
    let panel_index = relative / stride;
    if panel_index >= MODE_COUNT {
        return None;
    }
    let mode_index = PANEL_INDEX_BY_MODE_INDEX
        .iter()
        .position(|candidate| *candidate == panel_index)
        .expect("MODE SELECT panel map is a permutation");
    Some((panel_index, mode_index))
}
