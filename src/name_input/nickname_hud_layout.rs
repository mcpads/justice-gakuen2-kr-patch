use anyhow::{Result, ensure};
use serde::Serialize;

use super::NameInputRuntimePackLayout;

const GLYPH_CELL_PIXELS: usize = 20;
const OUTLINE_MARGIN_PIXELS: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NicknameHudGlyphStyle {
    pub scale_percent: u16,
    pub vertical_shift_px: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct NicknameHudGlyphLayout {
    pub scale_percent: u16,
    pub vertical_shift_px: i16,
    pub source_bounds: [usize; 4],
    pub target_bounds: [usize; 4],
}

pub fn plan_nickname_hud_glyph_layout(
    runtime_pack: &NameInputRuntimePackLayout,
    style: NicknameHudGlyphStyle,
) -> Result<NicknameHudGlyphLayout> {
    plan_nickname_hud_glyph_layout_for_crop(runtime_pack.crop, style)
}

pub fn plan_nickname_hud_glyph_layout_for_crop(
    crop: [usize; 4],
    style: NicknameHudGlyphStyle,
) -> Result<NicknameHudGlyphLayout> {
    ensure!(
        style.scale_percent > 0,
        "nickname HUD scale must be positive"
    );
    let [fill_x, fill_y, fill_width, fill_height] = crop;
    ensure!(
        fill_x >= OUTLINE_MARGIN_PIXELS && fill_y >= OUTLINE_MARGIN_PIXELS,
        "nickname HUD source fill has no room for its outline"
    );
    ensure!(
        fill_x + fill_width + OUTLINE_MARGIN_PIXELS <= GLYPH_CELL_PIXELS
            && fill_y + fill_height + OUTLINE_MARGIN_PIXELS <= GLYPH_CELL_PIXELS,
        "nickname HUD source outline exceeds its glyph cell"
    );

    let source_bounds = [
        fill_x - OUTLINE_MARGIN_PIXELS,
        fill_y - OUTLINE_MARGIN_PIXELS,
        fill_width + OUTLINE_MARGIN_PIXELS * 2,
        fill_height + OUTLINE_MARGIN_PIXELS * 2,
    ];
    let target_width = scaled_dimension(source_bounds[2], style.scale_percent)?;
    let target_height = scaled_dimension(source_bounds[3], style.scale_percent)?;
    let source_horizontal_center_twice = source_bounds[0] * 2 + source_bounds[2];
    ensure!(
        source_horizontal_center_twice >= target_width,
        "nickname HUD target width cannot remain centered in its glyph cell"
    );
    let target_x = (source_horizontal_center_twice - target_width) / 2;
    let source_bottom = source_bounds[1] + source_bounds[3];
    let target_bottom = i32::try_from(source_bottom)? + i32::from(style.vertical_shift_px);
    let target_y = target_bottom - i32::try_from(target_height)?;
    ensure!(
        target_y >= 0
            && target_x + target_width <= GLYPH_CELL_PIXELS
            && usize::try_from(target_y)? + target_height <= GLYPH_CELL_PIXELS,
        "nickname HUD scaled glyph exceeds its 20x20 cell"
    );

    Ok(NicknameHudGlyphLayout {
        scale_percent: style.scale_percent,
        vertical_shift_px: style.vertical_shift_px,
        source_bounds,
        target_bounds: [
            target_x,
            usize::try_from(target_y)?,
            target_width,
            target_height,
        ],
    })
}

fn scaled_dimension(source: usize, scale_percent: u16) -> Result<usize> {
    let scaled = source
        .checked_mul(usize::from(scale_percent))
        .and_then(|value| value.checked_add(50))
        .ok_or_else(|| anyhow::anyhow!("nickname HUD scale overflow"))?
        / 100;
    ensure!(scaled > 0, "nickname HUD scale collapsed a glyph dimension");
    Ok(scaled)
}
