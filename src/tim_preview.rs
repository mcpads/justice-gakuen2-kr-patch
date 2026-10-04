use std::fs::File;
use std::path::Path;

use anyhow::{Result, ensure};

use crate::tim::RgbaImage;

const MAX_PREVIEW_DIMENSION: usize = 1024;

pub(crate) fn write_tim_preview(path: &Path, image: &RgbaImage) -> Result<()> {
    ensure!(image.width > 0 && image.height > 0, "TIM preview is empty");
    let scale = (MAX_PREVIEW_DIMENSION / image.width.max(image.height)).max(1);
    let width = image.width * scale;
    let height = image.height * scale;
    let mut pixels = vec![0u8; width * height * 4];
    for source_y in 0..image.height {
        for source_x in 0..image.width {
            let source_offset = (source_y * image.width + source_x) * 4;
            for scale_y in 0..scale {
                for scale_x in 0..scale {
                    let target_offset =
                        ((source_y * scale + scale_y) * width + source_x * scale + scale_x) * 4;
                    pixels[target_offset..target_offset + 4]
                        .copy_from_slice(&image.pixels[source_offset..source_offset + 4]);
                }
            }
        }
    }

    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    Ok(())
}
