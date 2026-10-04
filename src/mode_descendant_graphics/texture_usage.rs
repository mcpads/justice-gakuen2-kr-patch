use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use super::model::{
    ModeDescendantGraphicsRecordAudit, ModeDescendantTextureConsumer, ModeDescendantTextureUsage,
};

pub(super) fn collect_texture_usage(
    records: &[ModeDescendantGraphicsRecordAudit],
) -> Result<Vec<ModeDescendantTextureUsage>> {
    let mut usage_by_hash = BTreeMap::<String, ModeDescendantTextureUsage>::new();
    for record in records {
        for tim in &record.tims {
            let consumer = ModeDescendantTextureConsumer {
                role: record.role.clone(),
                source_path: record.source_path.clone(),
                tim_offset: tim.offset,
            };
            if let Some(usage) = usage_by_hash.get_mut(&tim.source_tim_sha256) {
                ensure_same_texture(usage, tim)?;
                ensure!(
                    !usage.consumers.contains(&consumer),
                    "duplicate mode-descendant TIM consumer {} +0x{:x}",
                    consumer.source_path,
                    consumer.tim_offset
                );
                usage.consumers.push(consumer);
                continue;
            }
            usage_by_hash.insert(
                tim.source_tim_sha256.clone(),
                ModeDescendantTextureUsage {
                    source_tim_sha256: tim.source_tim_sha256.clone(),
                    bits_per_pixel: tim.bits_per_pixel,
                    total_size: tim.total_size,
                    pixel_width: tim.pixel_width,
                    pixel_height: tim.pixel_height,
                    image_vram_word_x: tim.image_vram_word_x,
                    image_vram_y: tim.image_vram_y,
                    clut_vram_x: tim.clut_vram_x,
                    clut_vram_y: tim.clut_vram_y,
                    palette_count: tim.palette_count,
                    consumer_count: 0,
                    consumers: vec![consumer],
                },
            );
        }
    }
    let mut usage = usage_by_hash.into_values().collect::<Vec<_>>();
    for item in &mut usage {
        item.consumers.sort();
        item.consumer_count = item.consumers.len();
    }
    Ok(usage)
}

fn ensure_same_texture(
    usage: &ModeDescendantTextureUsage,
    tim: &crate::embedded_tim::EmbeddedTimAudit,
) -> Result<()> {
    ensure!(
        usage.bits_per_pixel == tim.bits_per_pixel
            && usage.total_size == tim.total_size
            && usage.pixel_width == tim.pixel_width
            && usage.pixel_height == tim.pixel_height
            && usage.image_vram_word_x == tim.image_vram_word_x
            && usage.image_vram_y == tim.image_vram_y
            && usage.clut_vram_x == tim.clut_vram_x
            && usage.clut_vram_y == tim.clut_vram_y
            && usage.palette_count == tim.palette_count,
        "mode-descendant TIM hash {} has inconsistent geometry or residency metadata",
        tim.source_tim_sha256
    );
    Ok(())
}
