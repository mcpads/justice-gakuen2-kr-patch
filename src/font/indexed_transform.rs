use anyhow::{Result, ensure};

pub(crate) fn rotate_indexed_raster(
    pixels: &[u8],
    width: usize,
    height: usize,
    background_palette_index: u8,
    clockwise_rotation_degrees: f64,
) -> Result<Vec<u8>> {
    ensure!(
        pixels.len() == width * height && width > 0 && height > 0,
        "decorative text raster geometry changed"
    );
    if clockwise_rotation_degrees == 0.0 {
        return Ok(pixels.to_vec());
    }
    let angle = clockwise_rotation_degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    let center_x = (width - 1) as f64 / 2.0;
    let center_y = (height - 1) as f64 / 2.0;
    let mut output = vec![background_palette_index; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f64 - center_x;
            let dy = y as f64 - center_y;
            let source_x = center_x + dx * cos + dy * sin;
            let source_y = center_y - dx * sin + dy * cos;
            let source_x = source_x.round() as isize;
            let source_y = source_y.round() as isize;
            if source_x >= 0
                && source_y >= 0
                && (source_x as usize) < width
                && (source_y as usize) < height
            {
                output[y * width + x] = pixels[source_y as usize * width + source_x as usize];
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::rotate_indexed_raster;

    #[test]
    fn zero_degree_rotation_preserves_indexed_text_exactly() {
        let pixels = vec![0, 1, 2, 3, 4, 5];
        assert_eq!(
            rotate_indexed_raster(&pixels, 3, 2, 0, 0.0).unwrap(),
            pixels
        );
    }

    #[test]
    fn clockwise_rotation_keeps_background_outside_the_source_footprint() {
        let pixels = vec![0, 0, 0, 0, 7, 0, 0, 0, 0];
        let rotated = rotate_indexed_raster(&pixels, 3, 3, 0, 15.0).unwrap();
        assert_eq!(rotated.iter().filter(|pixel| **pixel == 7).count(), 1);
        assert_eq!(rotated[4], 7);
    }
}
