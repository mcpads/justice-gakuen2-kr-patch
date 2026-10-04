use std::fs::File;
use std::path::Path;

use anyhow::{Result, ensure};

use crate::tim::{Cell, RgbaImage};

const PREVIEW_SCALE: usize = 2;

pub(super) fn write_cell_preview(path: &Path, image: &RgbaImage, cell: Cell) -> Result<()> {
    ensure!(
        cell.width > 0
            && cell.height > 0
            && cell.x + cell.width <= image.width
            && cell.y + cell.height <= image.height,
        "title graphic preview cell is outside its source texture"
    );
    let scaled_width = cell.width * PREVIEW_SCALE;
    let scaled_height = cell.height * PREVIEW_SCALE;
    let mut pixels = vec![0u8; scaled_width * scaled_height * 4];
    for local_y in 0..cell.height {
        for local_x in 0..cell.width {
            let source_offset = ((cell.y + local_y) * image.width + cell.x + local_x) * 4;
            for scaled_y in 0..PREVIEW_SCALE {
                for scaled_x in 0..PREVIEW_SCALE {
                    let target_offset = ((local_y * PREVIEW_SCALE + scaled_y) * scaled_width
                        + local_x * PREVIEW_SCALE
                        + scaled_x)
                        * 4;
                    pixels[target_offset..target_offset + 4]
                        .copy_from_slice(&image.pixels[source_offset..source_offset + 4]);
                }
            }
        }
    }
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(
        file,
        u32::try_from(scaled_width)?,
        u32::try_from(scaled_height)?,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    Ok(())
}
