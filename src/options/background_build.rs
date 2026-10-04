use anyhow::{Context, Result, ensure};

use crate::font::{
    HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers, RasterizedIndexedText,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    Cell, read_4bpp_image_metadata_in_prefix, read_4bpp_indexed_image_in_prefix,
    read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::changed_ranges_are_within;

use super::background_assets::load_options_background_assets;
use super::background_model::{
    OptionsBackgroundBuildReport, OptionsBackgroundFontRole, OptionsBackgroundLayout,
    OptionsBackgroundRegionBuild,
};
use super::model::{
    OptionsBackgroundFontStyles, OptionsFontBuild, OptionsFontStyle, OptionsReleaseStatus,
};
use super::runtime_glyph_upload::{PreparedTextureUpload, pack_indexed_pixels};

pub(super) struct OptionsBackgroundBuild {
    pub(super) report: OptionsBackgroundBuildReport,
    pub(super) contextual_uploads: Vec<PreparedTextureUpload>,
}

pub(super) fn rebuild_options_background(
    assets_root: &std::path::Path,
    source_bin_sha256: &str,
    menu_stored: &[u8],
    source_menu_decoded: &[u8],
    target_menu_decoded: &mut [u8],
    styles: &OptionsBackgroundFontStyles,
) -> Result<OptionsBackgroundBuild> {
    ensure!(
        source_menu_decoded.len() == target_menu_decoded.len(),
        "options-background source and target MENU.BIZ lengths differ"
    );
    let assets = load_options_background_assets(
        assets_root,
        source_bin_sha256,
        menu_stored,
        source_menu_decoded,
    )?;
    let before_background = target_menu_decoded.to_vec();
    let source_indexed = read_4bpp_indexed_image_in_prefix(source_menu_decoded, assets.tim_offset)?;
    let mut fonts = Vec::with_capacity(assets.units.len());
    let mut regions = Vec::new();
    let mut allowed_ranges = Vec::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    for unit in &assets.units {
        let style = font_style(styles, unit.font_role);
        ensure!(
            style.font_px.is_finite() && style.font_px > 0.0,
            "invalid {} font size",
            unit.font_role.key()
        );
        let first_cell = unit.occurrences[0].cell;
        ensure!(
            unit.occurrences
                .iter()
                .all(|occurrence| occurrence.cell.width == first_cell.width
                    && occurrence.cell.height == first_cell.height),
            "options-background {} occurrences have different geometry",
            unit.id
        );
        let (pixels, rendered) = render_unit(
            rasterizers.for_font(&style.font)?,
            style.font_px,
            &unit.korean_text,
            first_cell,
            unit.layout,
            unit.clear_index,
            unit.outline_index,
            unit.fill_index,
        )?;
        let first_render = rendered
            .first()
            .context("options-background render produced no glyphs")?;
        ensure!(
            rendered
                .iter()
                .all(|render| render.font_name == first_render.font_name
                    && render.font_sha256 == first_render.font_sha256),
            "options-background font identity changed within one unit"
        );
        fonts.push(OptionsFontBuild {
            role: unit.font_role.key().to_string(),
            font_name: first_render.font_name.clone(),
            font_sha256: first_render.font_sha256.clone(),
            font_px: style.font_px,
        });
        let ink_bounds = rendered
            .iter()
            .map(|render| render.ink_bounds)
            .collect::<Vec<_>>();
        for occurrence in &unit.occurrences {
            let source_pixels = read_indexed_cell_in_prefix(
                source_menu_decoded,
                assets.tim_offset,
                occurrence.cell,
            )?;
            ensure!(
                sha256_bytes(&source_pixels) == occurrence.source_indexed_pixel_sha256,
                "options-background {} source region changed before build",
                occurrence.id
            );
            let write = write_indexed_cell_in_prefix_with_report(
                target_menu_decoded,
                assets.tim_offset,
                occurrence.cell,
                &pixels,
            )?;
            ensure!(
                write.changed_byte_count > 0,
                "options-background {} changed no bytes",
                occurrence.id
            );
            allowed_ranges.extend(write.allowed_ranges.iter().copied());
            regions.push(OptionsBackgroundRegionBuild {
                unit_id: unit.id.clone(),
                occurrence_id: occurrence.id.clone(),
                source_text: unit.source_text.clone(),
                korean_text: unit.korean_text.clone(),
                font_role: unit.font_role.key().to_string(),
                cell: occurrence.cell,
                source_indexed_pixel_sha256: occurrence.source_indexed_pixel_sha256.clone(),
                output_indexed_pixel_sha256: sha256_bytes(&read_indexed_cell_in_prefix(
                    target_menu_decoded,
                    assets.tim_offset,
                    occurrence.cell,
                )?),
                ink_bounds: ink_bounds.clone(),
                allowed_decoded_byte_ranges: write.allowed_ranges,
                changed_decoded_byte_count: write.changed_byte_count,
            });
        }
    }
    let changed = difference_ranges(&before_background, target_menu_decoded);
    ensure!(
        changed_ranges_are_within(&changed, &allowed_ranges),
        "options-background rebuild changed protected MENU.BIZ pixels"
    );
    let output_indexed = read_4bpp_indexed_image_in_prefix(target_menu_decoded, assets.tim_offset)?;
    let release_approved_unit_count = assets
        .units
        .iter()
        .filter(|unit| unit.release_status == OptionsReleaseStatus::Approved)
        .count();
    let image = read_4bpp_image_metadata_in_prefix(target_menu_decoded, assets.tim_offset)?;
    let contextual_uploads = regions
        .iter()
        .map(|region| {
            ensure!(
                region.cell.x.is_multiple_of(4),
                "options-background {} is not VRAM-word aligned",
                region.occurrence_id
            );
            let upload_cell = Cell {
                x: region.cell.x,
                y: region.cell.y,
                width: region.cell.width.next_multiple_of(4),
                height: region.cell.height,
            };
            ensure!(
                upload_cell.x + upload_cell.width <= image.pixel_width
                    && upload_cell.y + upload_cell.height <= image.height,
                "options-background {} contextual upload leaves its source TIM",
                region.occurrence_id
            );
            let pixels =
                read_indexed_cell_in_prefix(target_menu_decoded, assets.tim_offset, upload_cell)?;
            Ok(PreparedTextureUpload {
                role: format!("options_background:{}", region.occurrence_id),
                character: None,
                code: None,
                cell: upload_cell,
                vram_rect: [
                    image
                        .vram_x
                        .checked_add(u16::try_from(upload_cell.x / 4)?)
                        .context("options-background VRAM x overflow")?,
                    image
                        .vram_y
                        .checked_add(u16::try_from(upload_cell.y)?)
                        .context("options-background VRAM y overflow")?,
                    u16::try_from(upload_cell.width / 4)?,
                    u16::try_from(upload_cell.height)?,
                ],
                source_preservation: false,
                payload: pack_indexed_pixels(&pixels)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(OptionsBackgroundBuild {
        report: OptionsBackgroundBuildReport {
            manifest_sha256: assets.manifest_sha256,
            tim_offset: format!("0x{:05x}", assets.tim_offset),
            unit_count: assets.units.len(),
            release_approved_unit_count,
            development_input_available: true,
            release_candidate_input_eligible: release_approved_unit_count == assets.units.len(),
            source_indexed_pixel_sha256: sha256_bytes(&source_indexed.pixels),
            output_indexed_pixel_sha256: sha256_bytes(&output_indexed.pixels),
            changes_confined_to_owned_regions: true,
            fonts,
            regions,
        },
        contextual_uploads,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_unit(
    rasterizer: &IndexedTextRasterizer,
    font_px: f32,
    text: &str,
    cell: Cell,
    layout: OptionsBackgroundLayout,
    clear_index: u8,
    outline_index: u8,
    fill_index: u8,
) -> Result<(Vec<u8>, Vec<RasterizedIndexedText>)> {
    match layout {
        OptionsBackgroundLayout::Centered => {
            let mut rendered = rasterizer.rasterize(
                text,
                cell.width,
                cell.height,
                font_px,
                0.0,
                clear_index,
                Some(outline_index),
                fill_index,
                HorizontalTextAlignment::Center,
            )?;
            center_rendered_ink(&mut rendered, cell.width, cell.height, clear_index)?;
            Ok((rendered.pixels.clone(), vec![rendered]))
        }
        OptionsBackgroundLayout::VerticalGlyphs => {
            let characters = text.chars().collect::<Vec<_>>();
            ensure!(
                !characters.is_empty() && cell.height.is_multiple_of(characters.len()),
                "vertical options-background text does not divide its cell"
            );
            let slot_height = cell.height / characters.len();
            let mut pixels = vec![clear_index; cell.width * cell.height];
            let mut rendered_glyphs = Vec::with_capacity(characters.len());
            for (index, character) in characters.into_iter().enumerate() {
                let mut rendered = rasterizer.rasterize(
                    &character.to_string(),
                    cell.width,
                    slot_height,
                    font_px,
                    0.0,
                    clear_index,
                    Some(outline_index),
                    fill_index,
                    HorizontalTextAlignment::Center,
                )?;
                center_rendered_ink(&mut rendered, cell.width, slot_height, clear_index)?;
                for row in 0..slot_height {
                    let target = (index * slot_height + row) * cell.width;
                    let source = row * cell.width;
                    pixels[target..target + cell.width]
                        .copy_from_slice(&rendered.pixels[source..source + cell.width]);
                }
                rendered_glyphs.push(rendered);
            }
            Ok((pixels, rendered_glyphs))
        }
    }
}

fn center_rendered_ink(
    rendered: &mut RasterizedIndexedText,
    width: usize,
    height: usize,
    clear_index: u8,
) -> Result<()> {
    ensure!(
        rendered.pixels.len() == width * height,
        "options-background render geometry changed"
    );
    let [min_x, min_y, max_x, max_y] = rendered.ink_bounds;
    let ink_width = max_x - min_x;
    let ink_height = max_y - min_y;
    let target_x = (width - ink_width) / 2;
    let target_y = (height - ink_height) / 2;
    let mut centered = vec![clear_index; width * height];
    for source_y in min_y..max_y {
        for source_x in min_x..max_x {
            let pixel = rendered.pixels[source_y * width + source_x];
            if pixel != clear_index {
                let target = (target_y + source_y - min_y) * width + target_x + source_x - min_x;
                centered[target] = pixel;
            }
        }
    }
    rendered.pixels = centered;
    rendered.ink_bounds = [
        target_x,
        target_y,
        target_x + ink_width,
        target_y + ink_height,
    ];
    Ok(())
}

fn font_style(
    styles: &OptionsBackgroundFontStyles,
    role: OptionsBackgroundFontRole,
) -> &OptionsFontStyle {
    match role {
        OptionsBackgroundFontRole::SchoolName => &styles.school_name,
        OptionsBackgroundFontRole::CrestMark => &styles.crest_mark,
    }
}
