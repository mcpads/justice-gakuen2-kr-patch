use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub(super) struct Tim16bpp {
    pub(super) image_x: u16,
    pub(super) image_y: u16,
    pub(super) pixel_width: usize,
    pub(super) pixel_height: usize,
    pub(super) pixel_offset: usize,
    pub(super) total_size: usize,
}

pub(super) fn parse_16bpp_prefix(data: &[u8]) -> Result<Tim16bpp> {
    ensure!(data.len() >= 20, "truncated 16-bpp TIM header");
    ensure!(read_u32(data, 0)? == 0x10, "invalid TIM magic");
    ensure!(read_u32(data, 4)? == 0x02, "expected 16-bpp TIM");
    let image_size = usize::try_from(read_u32(data, 8)?)?;
    let image_x = read_u16(data, 12)?;
    let image_y = read_u16(data, 14)?;
    let pixel_width = usize::from(read_u16(data, 16)?);
    let pixel_height = usize::from(read_u16(data, 18)?);
    ensure!(
        pixel_width > 0 && pixel_height > 0,
        "empty 16-bpp TIM image"
    );
    ensure!(
        image_size == 12 + pixel_width * pixel_height * 2,
        "16-bpp TIM dimensions do not match block size"
    );
    let total_size = 8 + image_size;
    ensure!(total_size <= data.len(), "truncated 16-bpp TIM image");
    Ok(Tim16bpp {
        image_x,
        image_y,
        pixel_width,
        pixel_height,
        pixel_offset: 20,
        total_size,
    })
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    ensure!(offset + 2 <= data.len(), "truncated u16");
    Ok(u16::from_le_bytes(data[offset..offset + 2].try_into()?))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    ensure!(offset + 4 <= data.len(), "truncated u32");
    Ok(u32::from_le_bytes(data[offset..offset + 4].try_into()?))
}
