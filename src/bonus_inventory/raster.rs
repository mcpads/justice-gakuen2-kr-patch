use anyhow::{Result, ensure};

use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, RasterizedIndexedText};

use super::model::{
    BonusInventoryFontRole, BonusInventoryFontSources, BonusInventoryTextStyleSource,
};

const CLEAR_INDEX: u8 = 0;
const FIRST_INK_INDEX: u8 = 1;
const LAST_INK_INDEX: u8 = 15;

pub(super) fn font_for_role(
    fonts: &BonusInventoryFontSources,
    role: BonusInventoryFontRole,
) -> &BonusInventoryTextStyleSource {
    match role {
        BonusInventoryFontRole::Title => &fonts.title,
        BonusInventoryFontRole::CompactLabel => &fonts.compact_label,
        BonusInventoryFontRole::LargeLabel => &fonts.large_label,
        BonusInventoryFontRole::CompactAction => &fonts.compact_action,
        BonusInventoryFontRole::LargeAction => &fonts.large_action,
        BonusInventoryFontRole::Help => &fonts.help,
        BonusInventoryFontRole::ViewerNavigation => &fonts.viewer_navigation,
        BonusInventoryFontRole::ViewerCardPlaceholder => &fonts.viewer_card_placeholder,
    }
}

pub(super) fn rasterize_text(
    rasterizer: &IndexedTextRasterizer,
    style: &BonusInventoryTextStyleSource,
    role: BonusInventoryFontRole,
    text: &str,
    width: usize,
    height: usize,
) -> Result<RasterizedIndexedText> {
    let alignment = match role {
        BonusInventoryFontRole::Title
        | BonusInventoryFontRole::CompactAction
        | BonusInventoryFontRole::LargeAction
        | BonusInventoryFontRole::ViewerNavigation
        | BonusInventoryFontRole::ViewerCardPlaceholder => HorizontalTextAlignment::Center,
        BonusInventoryFontRole::CompactLabel
        | BonusInventoryFontRole::LargeLabel
        | BonusInventoryFontRole::Help => HorizontalTextAlignment::Left,
    };
    let first_ink_index = match role {
        BonusInventoryFontRole::ViewerCardPlaceholder => 9,
        _ => FIRST_INK_INDEX,
    };
    let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
        text,
        width,
        height,
        style.font_px,
        style.tracking_px,
        0,
        CLEAR_INDEX,
        first_ink_index,
        LAST_INK_INDEX,
        alignment,
    )?;
    shift_raster_vertically(raster, width, height, style.vertical_shift_px, role)
}

pub(super) fn overlay_text_preserving_background(
    source: &[u8],
    text: &[u8],
    width: usize,
    height: usize,
    maximum_background_index: u8,
) -> Result<Vec<u8>> {
    ensure!(
        source.len() == width * height && text.len() == source.len(),
        "background-preserving text cell geometry changed"
    );
    let background_pixels = source
        .iter()
        .enumerate()
        .filter_map(|(offset, index)| (*index <= maximum_background_index).then_some(offset))
        .collect::<Vec<_>>();
    ensure!(
        !background_pixels.is_empty(),
        "background-preserving text cell has no background pixels"
    );

    let mut output = source.to_vec();
    for (offset, source_index) in source.iter().copied().enumerate() {
        if source_index <= maximum_background_index {
            continue;
        }
        let x = offset % width;
        let y = offset / width;
        let nearest = background_pixels
            .iter()
            .copied()
            .min_by_key(|candidate| {
                let candidate_x = candidate % width;
                let candidate_y = candidate / width;
                (
                    x.abs_diff(candidate_x).pow(2) + y.abs_diff(candidate_y).pow(2),
                    *candidate,
                )
            })
            .expect("non-empty background pixel list");
        output[offset] = source[nearest];
    }
    for (output_index, text_index) in output.iter_mut().zip(text) {
        if *text_index != CLEAR_INDEX {
            *output_index = *text_index;
        }
    }
    Ok(output)
}

fn shift_raster_vertically(
    mut raster: RasterizedIndexedText,
    width: usize,
    height: usize,
    shift_px: i32,
    role: BonusInventoryFontRole,
) -> Result<RasterizedIndexedText> {
    if shift_px == 0 {
        return Ok(raster);
    }
    let mut pixels = vec![CLEAR_INDEX; raster.pixels.len()];
    let mut ink_bounds = [width, height, 0, 0];
    let mut has_ink = false;
    for source_y in 0..height {
        for x in 0..width {
            let pixel = raster.pixels[source_y * width + x];
            if pixel == CLEAR_INDEX {
                continue;
            }
            let target_y = source_y as i32 + shift_px;
            ensure!(
                0 <= target_y && target_y < height as i32,
                "bonus-inventory {role:?} text clips after vertical shift {shift_px}"
            );
            let target_y = target_y as usize;
            pixels[target_y * width + x] = pixel;
            has_ink = true;
            ink_bounds[0] = ink_bounds[0].min(x);
            ink_bounds[1] = ink_bounds[1].min(target_y);
            ink_bounds[2] = ink_bounds[2].max(x + 1);
            ink_bounds[3] = ink_bounds[3].max(target_y + 1);
        }
    }
    ensure!(has_ink, "bonus-inventory {role:?} text has no shifted ink");
    raster.pixels = pixels;
    raster.ink_bounds = ink_bounds;
    Ok(raster)
}
