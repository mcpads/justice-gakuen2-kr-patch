use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

use super::model::{GlyphFit, RasterizedMenuGlyph, RasterizedMenuGlyphSet, RenderedGlyph};

const MAPLESTORY_LIGHT_SHA256: &str =
    "6d51d8e576f77b01914095aa1f69f9d37c16d93fe940d748962867f218442ba9";
const MAPLESTORY_BOLD_SHA256: &str =
    "d57eaff48a793ff872a0f33bba2943d058d07c81ed64c68054858a287b85811a";
pub(super) const CELL_WIDTH: usize = 20;
pub(super) const CELL_HEIGHT: usize = 20;
pub(super) const FILL_THRESHOLD: u8 = 96;
pub(super) const OUTLINE_RADIUS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SupportedFontIdentity {
    pub(super) name: &'static str,
    pub(super) slug: &'static str,
}

pub fn rasterize_menu_glyphs(
    font_path: &Path,
    characters: &str,
    font_px: f32,
    outline_index: u8,
    fill_index: u8,
) -> Result<RasterizedMenuGlyphSet> {
    ensure!(font_px.is_finite() && font_px > 0.0, "invalid font size");
    ensure!(outline_index < 16, "outline index is not 4-bpp");
    ensure!(fill_index < 16, "fill index is not 4-bpp");
    ensure!(outline_index != fill_index, "outline and fill must differ");
    ensure!(!characters.is_empty(), "no glyphs requested");

    let (font, font_sha256, font_identity) = load_supported_font(font_path)?;
    let mut glyphs = Vec::new();
    for character in characters.chars() {
        let rendered = render_glyph(&font, &font_sha256, character, font_px)?;
        ensure!(
            rendered.fit.fits_with_one_pixel_outline,
            "{character:?} at {font_px}px does not fit the 20x20 cell with outline"
        );
        let pixels = rendered
            .pixels
            .into_iter()
            .map(|pixel| match pixel {
                255 => fill_index,
                80 => outline_index,
                _ => 0,
            })
            .collect();
        glyphs.push(RasterizedMenuGlyph {
            character,
            pixels,
            fit: rendered.fit,
        });
    }
    Ok(RasterizedMenuGlyphSet {
        font_name: font_identity.name.to_string(),
        font_sha256,
        glyphs,
    })
}

pub(super) fn load_supported_font(path: &Path) -> Result<(Font, String, SupportedFontIdentity)> {
    let font_data =
        std::fs::read(path).with_context(|| format!("failed to read font: {}", path.display()))?;
    let font_sha256 = sha256_bytes(&font_data);
    let identity = supported_font_identity(&font_sha256).with_context(|| {
        format!("unsupported font SHA-256: {font_sha256}; expected an admitted exact font identity")
    })?;
    let font = Font::from_bytes(font_data, FontSettings::default())
        .map_err(|error| anyhow::anyhow!("failed to parse {}: {error}", identity.name))?;
    Ok((font, font_sha256, identity))
}

pub(super) fn supported_font_identity(sha256: &str) -> Option<SupportedFontIdentity> {
    match sha256 {
        "ba40b8f9ada002005d1f5f3676a82b7bff07ddbd186a923073101099131999e6" => {
            Some(SupportedFontIdentity {
                name: "Dalmoori",
                slug: "dalmoori",
            })
        }
        "d3818c0f2898a3b2d79ccd04ec1e4de5e8940aa26abee261f73e315a44ce8df9" => {
            Some(SupportedFontIdentity {
                name: "Galmuri14",
                slug: "galmuri14",
            })
        }
        "4589cb1a59bcbd669ad7ac0669827e5a4d411832048e9bdd9618c907c1a8d272" => {
            Some(SupportedFontIdentity {
                name: "DenkiChip Hangul",
                slug: "denkichip-hangul",
            })
        }
        "954e4cbdc8476561cdd042aa2f22d4fbfea57dbfc4beb45e307ac1c1c6938d5a" => {
            Some(SupportedFontIdentity {
                name: "MaruMinya Hangul",
                slug: "maruminya-hangul",
            })
        }
        "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6" => {
            Some(SupportedFontIdentity {
                name: "NeoDunggeunmo",
                slug: "neodunggeunmo",
            })
        }
        "3882bd35066c26b0392cd4963ff9b3c151041dec34adc9d5633d137d1d9b9855" => {
            Some(SupportedFontIdentity {
                name: "Galmuri7",
                slug: "galmuri7",
            })
        }
        "5cb68052ee0a15571747e91c20f145e24b51bb459c6cd58226fafee78d9c0b16" => {
            Some(SupportedFontIdentity {
                name: "Galmuri9",
                slug: "galmuri9",
            })
        }
        MAPLESTORY_LIGHT_SHA256 => Some(SupportedFontIdentity {
            name: "Maplestory Light",
            slug: "maplestory-light",
        }),
        MAPLESTORY_BOLD_SHA256 => Some(SupportedFontIdentity {
            name: "Maplestory Bold",
            slug: "maplestory-bold",
        }),
        "389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60" => {
            Some(SupportedFontIdentity {
                name: "Lv2 Gothic",
                slug: "lv2-gothic",
            })
        }
        "7d538770c259b287b1e94d3e4fcb55bac71d866dbc98a69e0aab1fbfb7c6dade" => {
            Some(SupportedFontIdentity {
                name: "Lv2 Gothic Bold",
                slug: "lv2-gothic-bold",
            })
        }
        "2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f" => {
            Some(SupportedFontIdentity {
                name: "Galmuri11",
                slug: "galmuri11",
            })
        }
        "7b433b4a007c36dfb535fdea11de3e4f4c8b641ab591ed05d3bc0a4bbd75eb5f" => {
            Some(SupportedFontIdentity {
                name: "Galmuri11 Condensed",
                slug: "galmuri11-condensed",
            })
        }
        "5265b2f437fe81f0c8095b44c0173dd9a276b58a42552bf983f21c0e69e6e8af" => {
            Some(SupportedFontIdentity {
                name: "Galmuri11 Bold",
                slug: "galmuri11-bold",
            })
        }
        "364ea508e7f58114052406369bc9c6e5c7dfdc09e3693bd20f14ce35aef66af9" => {
            Some(SupportedFontIdentity {
                name: "ForgedBlade Medium",
                slug: "forged-blade-medium",
            })
        }
        "c906b3ab3490b6450106ec1be5bef344a63c4d40cacef508a2c74e34ecc22fe7" => {
            Some(SupportedFontIdentity {
                name: "ForgedBlade Bold",
                slug: "forged-blade-bold",
            })
        }
        "37cb6eb268d6f7d45a1467c0deac6eb2fbb80e930f8e66d528494f296f571ec9" => {
            Some(SupportedFontIdentity {
                name: "Football Bold",
                slug: "football-bold",
            })
        }
        _ => None,
    }
}

/// Preserve the admitted MapleStory placement; center the full ascent/descent
/// box for families whose descenders otherwise fall below the owned cell.
pub(super) fn font_baseline(
    font_sha256: &str,
    height: usize,
    ascent: f32,
    descent: f32,
) -> Result<i32> {
    ensure!(
        supported_font_identity(font_sha256).is_some(),
        "baseline requires an admitted font identity"
    );
    let descent = if matches!(
        font_sha256,
        MAPLESTORY_LIGHT_SHA256 | MAPLESTORY_BOLD_SHA256
    ) {
        0.0
    } else {
        descent
    };
    Ok(((height as f32 + ascent + descent) / 2.0).round() as i32)
}

pub(super) fn render_glyph(
    font: &Font,
    font_sha256: &str,
    character: char,
    font_px: f32,
) -> Result<RenderedGlyph> {
    try_render_glyph(font, font_sha256, character, font_px)?.with_context(|| {
        format!("font produced no visible pixels for {character:?} at {font_px}px")
    })
}

pub(super) fn try_render_glyph(
    font: &Font,
    font_sha256: &str,
    character: char,
    font_px: f32,
) -> Result<Option<RenderedGlyph>> {
    if !font.has_glyph(character) {
        return Ok(None);
    }
    let (metrics, raster) = font.rasterize(character, font_px);
    if metrics.width == 0 || metrics.height == 0 || !raster.iter().any(|value| *value > 0) {
        return Ok(None);
    }
    let line_metrics = font
        .horizontal_line_metrics(font_px)
        .context("font has no horizontal line metrics")?;
    let baseline = font_baseline(
        font_sha256,
        CELL_HEIGHT,
        line_metrics.ascent,
        line_metrics.descent,
    )?;
    let x_offset = (CELL_WIDTH as i32 - metrics.width as i32) / 2;
    let y_offset = baseline - metrics.ymin - metrics.height as i32;
    let mut coverage = vec![0u8; CELL_WIDTH * CELL_HEIGHT];
    let mut clipped_coverage_pixels = 0usize;
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            let value = raster[row * metrics.width + column];
            if value == 0 {
                continue;
            }
            let x = x_offset + column as i32;
            let y = y_offset + row as i32;
            if x < 0 || y < 0 || x >= CELL_WIDTH as i32 || y >= CELL_HEIGHT as i32 {
                clipped_coverage_pixels += 1;
                continue;
            }
            let offset = y as usize * CELL_WIDTH + x as usize;
            coverage[offset] = coverage[offset].max(value);
        }
    }

    let fill: Vec<bool> = coverage
        .iter()
        .map(|coverage| *coverage >= FILL_THRESHOLD)
        .collect();
    if !fill.iter().any(|pixel| *pixel) {
        return Ok(None);
    }
    let outline = dilate(&fill, CELL_WIDTH, CELL_HEIGHT, OUTLINE_RADIUS);
    let fill_touches_cell_boundary = boundary_has_pixel(&fill, CELL_WIDTH, CELL_HEIGHT);
    let mut pixels = vec![0u8; CELL_WIDTH * CELL_HEIGHT];
    for index in 0..pixels.len() {
        pixels[index] = if fill[index] {
            255
        } else if outline[index] {
            80
        } else {
            0
        };
    }
    Ok(Some(RenderedGlyph {
        pixels,
        fit: GlyphFit {
            character,
            metrics_width: metrics.width,
            metrics_height: metrics.height,
            xmin: metrics.xmin,
            ymin: metrics.ymin,
            advance_width: metrics.advance_width,
            clipped_coverage_pixels,
            fill_touches_cell_boundary,
            fits_with_one_pixel_outline: clipped_coverage_pixels == 0
                && !fill_touches_cell_boundary,
        },
    }))
}

pub(super) fn dilate(source: &[bool], width: usize, height: usize, radius: usize) -> Vec<bool> {
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            if !source[y * width + x] {
                continue;
            }
            for dy in -(radius as i32)..=radius as i32 {
                for dx in -(radius as i32)..=radius as i32 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                        output[ny as usize * width + nx as usize] = true;
                    }
                }
            }
        }
    }
    output
}

pub(super) fn boundary_has_pixel(pixels: &[bool], width: usize, height: usize) -> bool {
    (0..width).any(|x| pixels[x] || pixels[(height - 1) * width + x])
        || (0..height).any(|y| pixels[y * width] || pixels[y * width + width - 1])
}

fn sha256_bytes(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
