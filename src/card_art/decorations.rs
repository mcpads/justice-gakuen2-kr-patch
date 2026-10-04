//! Card footer typography and reused Korean logos, independent of card captions.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Layout {
    pub member_index: usize,
    source_decoded_sha256: String,
    background: String,
    background_sha256: String,
    write_mask: String,
    write_mask_sha256: String,
    placement: Cell,
    small_logo: Option<Cell>,
    footer: Option<Footer>,
    review_status: String,
}

impl Layout {
    pub fn has_logo(&self) -> bool {
        self.small_logo.is_some()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Footer {
    source_text: String,
    korean_label: String,
    font: String,
    cell: Cell,
    ink_rgb: [u8; 3],
    outline_rgb: [u8; 3],
}

pub(super) use crate::title_graphics::shared_logo::SmallLogo;

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    source: &[u8],
    base: &[u8],
    assets: &Path,
    layout: &Layout,
    logo: Option<&SmallLogo>,
    fonts: &BTreeMap<String, SizedFontSource>,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<(Vec<u8>, serde_json::Value)> {
    ensure!(
        layout.member_index < 60 && sha256_bytes(source) == layout.source_decoded_sha256,
        "card decoration source changed"
    );
    ensure!(
        source.len() == 544 + WIDTH * HEIGHT
            && base.len() == source.len()
            && base[..544] == source[..544],
        "card decoration base header changed"
    );
    let background = bound_bytes(assets, &layout.background, &layout.background_sha256)?;
    let mask = bound_bytes(assets, &layout.write_mask, &layout.write_mask_sha256)?;
    let mut output = base.to_vec();
    let allowed = apply_masked_background(&mut output, &background, &mask, layout.placement)?;
    for (at, &writable) in allowed.iter().enumerate() {
        ensure!(
            !writable || source[544 + at] == base[544 + at],
            "card decoration overlaps an earlier card edit"
        );
    }
    let mut surfaces = Vec::new();
    if let Some(cell) = layout.small_logo {
        let logo = logo.context("shared small logo was not loaded")?;
        ensure!(
            valid_cell(cell)
                && cell.width == logo.unit.cell.width
                && cell.height == logo.unit.cell.height,
            "small logo dimensions changed"
        );
        let mapping = logo
            .palette
            .iter()
            .map(|&rgb| ink_index(source, rgb))
            .collect::<Vec<_>>();
        for y in 0..cell.height {
            for x in 0..cell.width {
                let index = logo.pixels[y * cell.width + x];
                if index != logo.unit.clear_index {
                    let at = (cell.y + y) * WIDTH + cell.x + x;
                    ensure!(allowed[at], "small logo leaves restored mask");
                    output[544 + at] = mapping[usize::from(index)];
                }
            }
        }
        surfaces.push(json!({"role":"game_logo","cell":cell,
            "korean_text":logo.unit.korean_text,"source_asset":"title-graphics/franchise-logo-small.json",
            "asset_sha256":logo.unit_sha256,"indices_sha256":logo.unit.artwork.indices_sha256,
            "source_palette_sha256":logo.unit.artwork.source_palette_sha256,"palette_mapping":mapping}));
    }
    if let Some(footer) = &layout.footer {
        let number = layout.member_index + 1;
        ensure!(
            footer.source_text == format!("熱血カード NO.{number}")
                && !footer.korean_label.trim().is_empty(),
            "card footer number or label changed"
        );
        let cell = footer.cell;
        ensure!(valid_cell(cell), "invalid card footer cell");
        if let Some(logo) = layout.small_logo {
            ensure!(
                !crate::tim::cells_overlap(cell, logo),
                "card footer overlaps logo"
            );
        }
        let font = fonts
            .get(&footer.font)
            .context("missing card footer font")?;
        let text = format!("{} NO.{number}", footer.korean_label);
        let raster = rasterizers.for_font(&font.path)?.rasterize(
            &text,
            cell.width,
            cell.height,
            font.font_px,
            0.0,
            0,
            Some(2),
            1,
            HorizontalTextAlignment::Right,
        )?;
        let ink = ink_index(source, footer.ink_rgb);
        let outline = ink_index(source, footer.outline_rgb);
        ensure!(
            ink != outline,
            "card footer colors collapse in source palette"
        );
        for y in 0..cell.height {
            for x in 0..cell.width {
                let value = raster.pixels[y * cell.width + x];
                if value != 0 {
                    let at = (cell.y + y) * WIDTH + cell.x + x;
                    ensure!(allowed[at], "card footer leaves restored mask");
                    output[544 + at] = if value == 1 { ink } else { outline };
                }
            }
        }
        surfaces.push(json!({"role":"card_number_footer","text":text,"source_text":footer.source_text,
            "cell":cell,"font":raster.font_name,"font_sha256":raster.font_sha256,"font_px":font.font_px,
            "advance":raster.measured_advance_px,"ink_bounds":raster.ink_bounds}));
    }
    ensure!(
        !surfaces.is_empty(),
        "card decoration has no localized surface"
    );
    verify_masked_preservation(base, &output, &allowed)?;
    Ok((
        output,
        json!({"background_sha256":layout.background_sha256,"write_mask":layout.write_mask,
        "write_mask_sha256":layout.write_mask_sha256,"placement":layout.placement,
        "allowed_pixels":mask.iter().filter(|&&p| p == 1).count(),"surfaces":surfaces,
        "review_status":layout.review_status}),
    ))
}
