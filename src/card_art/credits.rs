//! Source-bound captions share typography and write ownership across card families.
use super::*;

#[derive(Clone, Copy, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Family {
    AnimationCredits,
    Concept,
    Guest,
    Filmstrip,
    Decorative,
    Ceremony,
}

impl Family {
    fn translation_file(self) -> &'static str {
        match self {
            Self::AnimationCredits => "animation-credits.json",
            Self::Concept => "concept-translations.json",
            Self::Guest => "guest-translations.json",
            Self::Filmstrip => "filmstrip-translations.json",
            Self::Decorative => "decorative-translations.json",
            Self::Ceremony => "ceremony-translations.json",
        }
    }

    fn members(self) -> BTreeSet<usize> {
        match self {
            Self::AnimationCredits => (49..58).collect(),
            Self::Concept => (0..9).chain(16..19).chain([23, 26, 58, 59]).collect(),
            Self::Guest => (9..16).collect(),
            Self::Filmstrip => (19..23).collect(),
            Self::Decorative => [27].into(),
            Self::Ceremony => [28].into(),
        }
    }
}

#[derive(Deserialize)]
struct Translation {
    member_index: usize,
    source_decoded_sha256: String,
    korean_parts: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    part: String,
    text: String,
    cell: Cell,
    font: Option<String>,
    ink_rgb: Option<[u8; 3]>,
    outline_rgb: Option<[u8; 3]>,
    alignment: Option<HorizontalTextAlignment>,
    #[serde(default)]
    rotation_degrees: f64,
    #[serde(default)]
    continues_word: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Layout {
    pub member_index: usize,
    family: Family,
    source_decoded_sha256: String,
    background: String,
    background_sha256: String,
    write_mask: String,
    write_mask_sha256: String,
    placement: Cell,
    font: String,
    ink_rgb: [u8; 3],
    outline_rgb: [u8; 3],
    lines: Vec<Line>,
    review_status: String,
    background_provenance: String,
}

impl Layout {
    pub fn owns_member(&self) -> bool {
        self.family.members().contains(&self.member_index)
    }

    pub fn translation_file(&self) -> &'static str {
        self.family.translation_file()
    }
}

fn validate_parts(lines: &[Line], parts: &BTreeMap<String, String>) -> Result<()> {
    let mut actual = BTreeMap::<String, String>::new();
    for line in lines {
        let text = actual.entry(line.part.clone()).or_default();
        if !text.is_empty() && !line.continues_word {
            text.push(' ');
        }
        text.push_str(&line.text);
    }
    ensure!(actual.len() == parts.len(), "credit semantic parts differ");
    for (part, text) in parts {
        let wrapped = actual.get(part).context("missing credit semantic part")?;
        ensure!(
            normalize(wrapped) == normalize(text),
            "credit wrapping changes translated part {part}"
        );
    }
    Ok(())
}

pub(super) fn render(
    source: &[u8],
    assets: &Path,
    layout: &Layout,
    fonts: &BTreeMap<String, SizedFontSource>,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<(Vec<u8>, serde_json::Value)> {
    ensure!(
        layout.owns_member(),
        "caption family does not own this card"
    );
    let translations: Entries<Translation> = read_json(&assets.join(layout.translation_file()))?;
    let indices = translations
        .entries
        .iter()
        .map(|e| e.member_index)
        .collect::<BTreeSet<_>>();
    ensure!(
        translations.entries.len() == indices.len() && indices == layout.family.members(),
        "card caption translation population changed"
    );
    let translation = translations
        .entries
        .iter()
        .find(|e| e.member_index == layout.member_index)
        .context("credit layout has no translation")?;
    ensure!(
        sha256_bytes(source) == layout.source_decoded_sha256
            && translation.source_decoded_sha256 == layout.source_decoded_sha256,
        "credit source binding changed"
    );
    validate_parts(&layout.lines, &translation.korean_parts)?;
    ensure!(
        valid_cell(layout.placement),
        "invalid credit background placement"
    );
    let tim = parse_8bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == WIDTH
            && tim.image_height == HEIGHT
            && tim.total_size == source.len()
            && tim.pixel_offset == 544
            && tim.clut_width == 256
            && tim.clut_height == 1,
        "credit TIM geometry changed"
    );
    let background = bound_bytes(assets, &layout.background, &layout.background_sha256)?;
    let mask = bound_bytes(assets, &layout.write_mask, &layout.write_mask_sha256)?;
    let mut output = source.to_vec();
    let allowed = apply_masked_background(&mut output, &background, &mask, layout.placement)?;
    let mut reports = Vec::new();
    for (i, line) in layout.lines.iter().enumerate() {
        let font = fonts
            .get(line.font.as_ref().unwrap_or(&layout.font))
            .context("missing card caption font")?;
        let ink = ink_index(source, line.ink_rgb.unwrap_or(layout.ink_rgb));
        let outline = ink_index(source, line.outline_rgb.unwrap_or(layout.outline_rgb));
        ensure!(
            ink != outline,
            "caption ink and outline collapse in source palette"
        );
        ensure!(
            line.rotation_degrees.is_finite() && line.rotation_degrees.abs() <= 90.0,
            "invalid caption rotation"
        );
        let cell = line.cell;
        ensure!(valid_cell(cell), "invalid credit text cell");
        ensure!(
            layout.lines[..i]
                .iter()
                .all(|prev| !crate::tim::cells_overlap(prev.cell, cell)),
            "credit line cells overlap"
        );
        // Logical indices keep palette index zero available as actual black ink.
        let quarter_turn = line.rotation_degrees.abs() == 90.0;
        let (raster_width, raster_height) = if quarter_turn {
            (cell.height, cell.width)
        } else {
            (cell.width, cell.height)
        };
        let raster = rasterizers.for_font(&font.path)?.rasterize(
            &line.text,
            raster_width,
            raster_height,
            font.font_px,
            0.0,
            0,
            Some(2),
            1,
            line.alignment.unwrap_or(HorizontalTextAlignment::Center),
        )?;
        let pixels = if quarter_turn {
            rotate_quarter_turn(
                &raster.pixels,
                raster_width,
                raster_height,
                line.rotation_degrees > 0.0,
            )
        } else {
            crate::font::rotate_indexed_raster(
                &raster.pixels,
                cell.width,
                cell.height,
                0,
                line.rotation_degrees,
            )?
        };
        if line.rotation_degrees != 0.0 && !quarter_turn {
            let [x, y, right, bottom] = raster.ink_bounds;
            let (sin, cos) = line.rotation_degrees.to_radians().sin_cos();
            let (cx, cy) = (
                (cell.width - 1) as f64 / 2.0,
                (cell.height - 1) as f64 / 2.0,
            );
            for (px, py) in [
                (x, y),
                (right - 1, y),
                (x, bottom - 1),
                (right - 1, bottom - 1),
            ] {
                let (dx, dy) = (px as f64 - cx, py as f64 - cy);
                let (rx, ry) = (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos);
                ensure!(
                    rx >= 0.0
                        && ry >= 0.0
                        && rx <= (cell.width - 1) as f64
                        && ry <= (cell.height - 1) as f64,
                    "rotated caption would clip"
                );
            }
        }
        for y in 0..cell.height {
            for x in 0..cell.width {
                let value = pixels[y * cell.width + x];
                if value != 0 {
                    let (px, py) = (cell.x + x, cell.y + y);
                    ensure!(
                        allowed[py * WIDTH + px],
                        "credit ink leaves restored mask at {px},{py}"
                    );
                    output[544 + py * WIDTH + px] = if value == 1 { ink } else { outline };
                }
            }
        }
        reports.push(json!({"part":line.part,"text":line.text,"cell":cell,"rotation_degrees":line.rotation_degrees,
            "continues_word":line.continues_word,"ink_index":ink,"outline_index":outline,
            "font":raster.font_name,"font_sha256":raster.font_sha256,"font_px":font.font_px,
            "advance":raster.measured_advance_px,"ink_bounds":raster.ink_bounds,
            "raster_size":[raster_width,raster_height]}));
    }
    verify_masked_preservation(source, &output, &allowed)?;
    let report = json!({"family":layout.family,"translation_file":layout.translation_file(),"member_index":layout.member_index,
        "source_decoded_sha256":layout.source_decoded_sha256,
        "background_sha256":layout.background_sha256,"write_mask":layout.write_mask,
        "write_mask_sha256":layout.write_mask_sha256,"placement":layout.placement,
        "allowed_pixels":mask.iter().filter(|&&p| p == 1).count(),
        "background_provenance":layout.background_provenance,"review_status":layout.review_status,
        "lines":reports,"separate_unmodified_surfaces":["card-number footer","game logo"]});
    Ok((output, report))
}

// A quarter turn swaps the canvas dimensions and preserves every source pixel.
// Rotating in the tall destination canvas would first clip a horizontal word.
fn rotate_quarter_turn(pixels: &[u8], width: usize, height: usize, clockwise: bool) -> Vec<u8> {
    let mut output = vec![0; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = if clockwise {
                (height - 1 - y, x)
            } else {
                (y, width - 1 - x)
            };
            output[dy * height + dx] = pixels[y * width + x];
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turn_preserves_nonsquare_caption_and_reading_direction() {
        let pixels = [1, 2, 3, 4, 5, 6];
        let turned = rotate_quarter_turn(&pixels, 3, 2, true);
        assert_eq!(turned, [4, 1, 5, 2, 6, 3]);
        assert_eq!(rotate_quarter_turn(&turned, 2, 3, false), pixels);
    }

    #[test]
    fn credit_wrap_cannot_drop_duplicate_or_move_words_between_parts() {
        let parts = BTreeMap::from([
            ("title".into(), "사립 저스티스 학원".into()),
            ("studio".into(), "제작 아시 프로덕션".into()),
        ]);
        let line = |part: &str, text: &str| Line {
            part: part.into(),
            text: text.into(),
            cell: Cell {
                x: 0,
                y: 0,
                width: 100,
                height: 16,
            },
            font: None,
            ink_rgb: None,
            outline_rgb: None,
            alignment: None,
            rotation_degrees: 0.0,
            continues_word: false,
        };
        let mut lines = vec![
            line("title", "사립 저스티스"),
            line("title", "학원"),
            line("studio", "제작 아시 프로덕션"),
        ];
        validate_parts(&lines, &parts).unwrap();
        lines[1].part = "studio".into();
        assert!(validate_parts(&lines, &parts).is_err());
        lines[1].part = "title".into();
        lines.push(line("title", "학원"));
        assert!(validate_parts(&lines, &parts).is_err());
        lines.pop();
        lines.remove(1);
        assert!(validate_parts(&lines, &parts).is_err());
    }

    #[test]
    fn syllable_placement_requires_explicit_word_continuation() {
        let parts = BTreeMap::from([("title".into(), "포켓 저스티스".into())]);
        let line = |text: &str, continues_word: bool| {
            serde_json::from_value::<Line>(json!({
                "part":"title", "text":text, "continues_word":continues_word,
                "cell":{"x":0,"y":0,"width":40,"height":40}
            }))
            .unwrap()
        };
        let mut lines = vec![
            line("포", false),
            line("켓", true),
            line("저스", false),
            line("티스", true),
        ];
        validate_parts(&lines, &parts).unwrap();
        lines[2].continues_word = true;
        assert!(validate_parts(&lines, &parts).is_err());
        lines[2].continues_word = false;
        lines[1].text = "켓켓".into();
        assert!(validate_parts(&lines, &parts).is_err());
    }
}
