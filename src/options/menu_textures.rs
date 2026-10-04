use anyhow::{Result, ensure};

use crate::tim::{Tim4bpp, parse_4bpp_prefix, parse_4bpp_without_clut_prefix};

use super::model::{DetectedMenuTextureRegion, OptionsMenuTextureRegion};

const EMBEDDED_MENU_FONT_TIM_OFFSET: usize = 0;
pub(super) const EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET: usize = 0x20800;
const EMBEDDED_MODE_SELECT_TIM_OFFSET: usize = 0x22800;
const FIRST_MODE_DESCRIPTION_TIM_OFFSET: usize = 0x4b800;

const TEXTURES: [(&str, usize); 3] = [
    ("menu_font", EMBEDDED_MENU_FONT_TIM_OFFSET),
    ("options_sprites", EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET),
    ("mode_select", EMBEDDED_MODE_SELECT_TIM_OFFSET),
];

pub(super) fn inspect_options_menu_textures(
    menu_decoded: &[u8],
) -> Result<Vec<OptionsMenuTextureRegion>> {
    let mut regions = Vec::with_capacity(TEXTURES.len());
    for (role, decoded_offset) in TEXTURES {
        ensure!(
            decoded_offset < menu_decoded.len(),
            "{role} TIM offset is outside MENU.BIZ"
        );
        let tim = parse_4bpp_prefix(&menu_decoded[decoded_offset..])?;
        ensure!(
            decoded_offset + tim.total_size <= menu_decoded.len(),
            "{role} TIM exceeds MENU.BIZ"
        );
        regions.push(describe_texture(role, decoded_offset, tim));
    }
    ensure!(
        EMBEDDED_MODE_SELECT_TIM_OFFSET + regions[2].decoded_size
            <= FIRST_MODE_DESCRIPTION_TIM_OFFSET,
        "mode-select TIM overlaps its first description TIM"
    );
    Ok(regions)
}

pub(super) fn detect_four_bit_tim_regions(menu_decoded: &[u8]) -> Vec<DetectedMenuTextureRegion> {
    (0..menu_decoded.len().saturating_sub(8))
        .step_by(4)
        .filter_map(|offset| {
            let data = &menu_decoded[offset..];
            let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
            let flags = u32::from_le_bytes(data[4..8].try_into().unwrap());
            if magic != 0x10 {
                return None;
            }
            match flags {
                0x08 => parse_4bpp_prefix(data)
                    .ok()
                    .map(|tim| DetectedMenuTextureRegion {
                        decoded_offset: format!("0x{offset:05x}"),
                        decoded_size: tim.total_size,
                        has_clut: true,
                        image_vram_word_x: tim.image_x,
                        image_vram_y: tim.image_y,
                        image_pixel_width: tim.pixel_width(),
                        image_height: tim.image_height,
                    }),
                0x00 => {
                    parse_4bpp_without_clut_prefix(data)
                        .ok()
                        .map(|tim| DetectedMenuTextureRegion {
                            decoded_offset: format!("0x{offset:05x}"),
                            decoded_size: tim.total_size,
                            has_clut: false,
                            image_vram_word_x: tim.image_x,
                            image_vram_y: tim.image_y,
                            image_pixel_width: tim.pixel_width(),
                            image_height: tim.image_height,
                        })
                }
                _ => None,
            }
        })
        .collect()
}

fn describe_texture(role: &str, decoded_offset: usize, tim: Tim4bpp) -> OptionsMenuTextureRegion {
    OptionsMenuTextureRegion {
        role: role.to_string(),
        decoded_offset: format!("0x{decoded_offset:05x}"),
        decoded_size: tim.total_size,
        clut_vram_x: tim.clut_x,
        clut_vram_y: tim.clut_y,
        clut_width: tim.clut_width,
        clut_height: tim.clut_height,
        image_vram_word_x: tim.image_x,
        image_vram_y: tim.image_y,
        image_pixel_width: tim.pixel_width(),
        image_height: tim.image_height,
    }
}

#[cfg(test)]
pub(super) fn describe_texture_for_test(
    role: &str,
    decoded_offset: usize,
    tim: Tim4bpp,
) -> OptionsMenuTextureRegion {
    describe_texture(role, decoded_offset, tim)
}
