//! Source-bound card localization with preserved palettes and archive slots.
mod credits;
mod decorations;
mod whole_images;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;

use crate::compression::decompress;
use crate::development_build_spec::SizedFontSource;
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::paged_compression::{
    compress_page_safe_image, compress_with_source_page_work_limit,
    source_paged_compression_profile,
};
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{Cell, parse_8bpp_prefix};
use crate::tim_preview::write_tim_preview;
use crate::tzz::{TzzMember, parse_tzz};

pub(crate) const PATH: &str = "DAT2/CARD.BIZ";
const SOURCE_SHA256: &str = "92e47a42543472e09ef2da3b5c066cc301d31c9d7eae880896f3bf6b159fc681";
const WIDTH: usize = 304;
const HEIGHT: usize = 384;
const PAPER: Cell = Cell {
    x: 0,
    y: 224,
    width: WIDTH,
    height: 160,
};

pub(crate) struct CardArtBuild {
    pub record: Vec<u8>,
    pub source_sha256: String,
}

/// Render the selected card family with the disc builder's exact asset pipeline.
/// Produces CARD.BIZ, previews and a manifest without composing a disc image.
pub fn build_card_art(cue: &Path, spec: &Path, output_dir: &Path) -> Result<()> {
    ensure!(!output_dir.exists(), "card output directory already exists");
    let config = crate::development_build_spec::load_development_build_spec(spec)?;
    let assets = config
        .assets
        .card_art
        .context("card assets are not selected")?;
    let source = SupportedSourceDisc::open(cue)?;
    build(&source, &assets, &config.fonts.card_art, output_dir)?;
    Ok(())
}

#[derive(Deserialize)]
struct Entries<T> {
    entries: Vec<T>,
}

#[derive(Deserialize)]
struct Letter {
    member_index: usize,
    source_decoded_sha256: String,
    korean_body: String,
    korean_signature: String,
    review_status: String,
    source_marks: Vec<String>,
    #[serde(default)]
    hand_corrections: Vec<HandCorrection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandCorrection {
    before_text: String,
    erased_text: String,
}

fn corrected_display(text: &str, correction: &HandCorrection) -> Result<(String, String)> {
    ensure!(
        !correction.before_text.is_empty()
            && !correction.erased_text.trim().is_empty()
            && !correction.erased_text.contains(['\n', '\r'])
            && text.matches(&correction.before_text).count() == 1,
        "hand correction must have one unambiguous anchor and a nonempty erased word"
    );
    let at = text.find(&correction.before_text).unwrap();
    let prefix = &text[..at];
    Ok((
        format!("{prefix}{} {}", correction.erased_text, &text[at..]),
        prefix.to_owned(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn render_corrected_line(
    pixels: &mut [u8],
    text: &str,
    correction: &HandCorrection,
    cell: Cell,
    alignment: HorizontalTextAlignment,
    font: &SizedFontSource,
    rasterizers: &mut IndexedTextRasterizers,
    ink: u8,
    protected: &[Cell],
) -> Result<serde_json::Value> {
    let (display, prefix) = corrected_display(text, correction)?;
    let mut report = render_line(
        pixels,
        &display,
        cell,
        alignment,
        font,
        rasterizers,
        ink,
        protected,
    )?;
    let rasterizer = rasterizers.for_font(&font.path)?;
    let measure = |word: &str| -> Result<f32> {
        if word.is_empty() {
            return Ok(0.0);
        }
        Ok(rasterizer
            .rasterize(
                word,
                cell.width,
                cell.height,
                font.font_px,
                0.0,
                0,
                None,
                1,
                HorizontalTextAlignment::Left,
            )?
            .measured_advance_px)
    };
    let advance = report["advance"]
        .as_f64()
        .context("missing letter advance")? as f32;
    let origin = match alignment {
        HorizontalTextAlignment::Left => 0.0,
        HorizontalTextAlignment::Center => (cell.width as f32 - advance) / 2.0,
        HorizontalTextAlignment::Right => cell.width as f32 - advance,
    };
    let start = (origin + measure(&prefix)?).round() as usize;
    let end = (origin + measure(&prefix)? + measure(&correction.erased_text)?).round() as usize;
    ensure!(
        start < end && end <= cell.width && cell.height >= 4,
        "hand correction strike leaves its line"
    );
    let mid = cell.height / 2;
    for y in [mid - 1, mid + 1] {
        for x in start..end {
            let (px, py) = (cell.x + x, cell.y + y);
            ensure!(
                !protected.iter().any(|&r| contains(r, px, py)),
                "hand correction crosses protected drawing"
            );
            pixels[py * WIDTH + px] = ink;
        }
    }
    report["final_text"] = json!(text);
    report["hand_correction"] = json!({"erased_text":correction.erased_text,"before_text":correction.before_text,
        "strike_x":[cell.x + start,cell.x + end],"strike_y":[cell.y + mid - 1,cell.y + mid + 1]});
    Ok(report)
}

#[derive(Deserialize)]
struct Background {
    member_index: usize,
    source_decoded_sha256: String,
    indices: String,
    indices_sha256: String,
    placement: Cell,
    protected_source_regions: Vec<Cell>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LetterLayout {
    member_index: usize,
    body_lines: Vec<String>,
    body_font: String,
    body_first_row: Cell,
    line_pitch: usize,
    body_alignment: HorizontalTextAlignment,
    signature_font: String,
    signature_cell: Cell,
    ink_rgb: [u8; 3],
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&std::fs::read(path)?).with_context(|| path.display().to_string())
}

fn bound_bytes(assets: &Path, name: &str, expected: &str) -> Result<Vec<u8>> {
    ensure!(
        !name.is_empty()
            && Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe card asset path"
    );
    let data = std::fs::read(assets.join(name))?;
    ensure!(
        sha256_bytes(&data) == expected,
        "card asset changed: {name}"
    );
    Ok(data)
}

fn apply_masked_background(
    output: &mut [u8],
    background: &[u8],
    mask: &[u8],
    placement: Cell,
) -> Result<Vec<bool>> {
    ensure!(
        valid_cell(placement) && output.len() == 544 + WIDTH * HEIGHT,
        "invalid masked card geometry"
    );
    let area = placement.width * placement.height;
    ensure!(
        background.len() == area
            && mask.len() == area
            && mask.iter().all(|&p| p <= 1)
            && mask.contains(&1),
        "invalid card background or write mask"
    );
    let mut allowed = vec![false; WIDTH * HEIGHT];
    for y in 0..placement.height {
        for x in 0..placement.width {
            let at = y * placement.width + x;
            if mask[at] == 1 {
                let destination = (placement.y + y) * WIDTH + placement.x + x;
                allowed[destination] = true;
                output[544 + destination] = background[at];
            }
        }
    }
    Ok(allowed)
}

fn verify_masked_preservation(source: &[u8], output: &[u8], allowed: &[bool]) -> Result<()> {
    ensure!(
        source.len() == 544 + WIDTH * HEIGHT
            && output.len() == source.len()
            && allowed.len() == WIDTH * HEIGHT,
        "invalid card preservation geometry"
    );
    ensure!(
        source[..544] == output[..544],
        "card header or CLUT changed"
    );
    for (at, &writable) in allowed.iter().enumerate() {
        ensure!(
            writable || source[544 + at] == output[544 + at],
            "card changed protected source pixel at {},{}",
            at % WIDTH,
            at / WIDTH
        );
    }
    Ok(())
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains(cell: Cell, x: usize, y: usize) -> bool {
    x >= cell.x && y >= cell.y && x - cell.x < cell.width && y - cell.y < cell.height
}

fn valid_cell(cell: Cell) -> bool {
    cell.width > 0
        && cell.height > 0
        && cell
            .x
            .checked_add(cell.width)
            .is_some_and(|end| end <= WIDTH)
        && cell
            .y
            .checked_add(cell.height)
            .is_some_and(|end| end <= HEIGHT)
}

fn paper_cell(cell: Cell) -> bool {
    valid_cell(cell) && cell.y >= PAPER.y
}

fn verify_preservation(
    source: &[u8],
    patched: &[u8],
    pixel_offset: usize,
    protected: &[Cell],
) -> Result<()> {
    ensure!(
        source.len() == patched.len() && source.len() == pixel_offset + WIDTH * HEIGHT,
        "card TIM size changed"
    );
    ensure!(
        source[..pixel_offset] == patched[..pixel_offset],
        "card TIM header or palette changed"
    );
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if !contains(PAPER, x, y) || protected.iter().any(|&r| contains(r, x, y)) {
                let at = pixel_offset + y * WIDTH + x;
                ensure!(
                    source[at] == patched[at],
                    "protected card pixel changed at {x},{y}"
                );
            }
        }
    }
    Ok(())
}

fn ink_index(tim: &[u8], rgb: [u8; 3]) -> u8 {
    (0..256)
        .min_by_key(|&i| {
            let color = u16::from_le_bytes([tim[20 + i * 2], tim[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(c, s)| (i32::from((color >> s) & 31) * 255 / 31 - i32::from(rgb[c])).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}

#[allow(clippy::too_many_arguments)]
fn render_line(
    pixels: &mut [u8],
    text: &str,
    cell: Cell,
    alignment: HorizontalTextAlignment,
    font: &SizedFontSource,
    rasterizers: &mut IndexedTextRasterizers,
    ink: u8,
    protected: &[Cell],
) -> Result<serde_json::Value> {
    ensure!(paper_cell(cell), "letter text extends outside paper");
    let raster = rasterizers
        .for_font(&font.path)?
        .rasterize(
            text,
            cell.width,
            cell.height,
            font.font_px,
            0.0,
            0,
            None,
            1,
            alignment,
        )
        .with_context(|| format!("card letter line {text:?}"))?;
    for y in 0..cell.height {
        for x in 0..cell.width {
            if raster.pixels[y * cell.width + x] != 0 {
                let (px, py) = (cell.x + x, cell.y + y);
                ensure!(
                    !protected.iter().any(|&r| contains(r, px, py)),
                    "letter ink overlaps protected drawing"
                );
                pixels[py * WIDTH + px] = ink;
            }
        }
    }
    Ok(
        json!({"text":text,"cell":cell,"font":raster.font_name,"font_sha256":raster.font_sha256,
        "font_px":font.font_px,"advance":raster.measured_advance_px,"ink_bounds":raster.ink_bounds}),
    )
}

fn render_letter(
    source: &[u8],
    background: &[u8],
    letter: &Letter,
    paper: &Background,
    layout: &LetterLayout,
    fonts: &BTreeMap<String, SizedFontSource>,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<(Vec<u8>, Vec<serde_json::Value>)> {
    ensure!(
        paper.placement == PAPER && background.len() == PAPER.width * PAPER.height,
        "letter background geometry changed"
    );
    ensure!(
        paper
            .protected_source_regions
            .iter()
            .all(|&r| paper_cell(r)),
        "invalid protected card region"
    );
    ensure!(
        !layout.body_lines.is_empty()
            && layout.body_lines.len() <= 5
            && layout.line_pitch >= layout.body_first_row.height,
        "invalid letter line layout"
    );
    ensure!(
        normalize(&layout.body_lines.join(" ")) == normalize(&letter.korean_body),
        "letter wrapping changes translated text"
    );
    for correction in &letter.hand_corrections {
        ensure!(
            !correction.before_text.is_empty()
                && layout
                    .body_lines
                    .iter()
                    .map(|line| line.matches(&correction.before_text).count())
                    .sum::<usize>()
                    == 1,
            "hand correction anchor is missing, repeated, or split by wrapping"
        );
    }
    let tim = parse_8bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == WIDTH
            && tim.image_height == HEIGHT
            && tim.total_size == source.len()
            && tim.pixel_offset == 544
            && tim.clut_width == 256
            && tim.clut_height == 1,
        "letter TIM geometry changed"
    );
    let mut output = source.to_vec();
    let pixels = &mut output[tim.pixel_offset..];
    pixels[PAPER.y * WIDTH..].copy_from_slice(background);
    for &cell in &paper.protected_source_regions {
        for y in cell.y..cell.y + cell.height {
            let start = y * WIDTH + cell.x;
            pixels[start..start + cell.width].copy_from_slice(
                &source[tim.pixel_offset + start..tim.pixel_offset + start + cell.width],
            );
        }
    }
    let ink = ink_index(source, layout.ink_rgb);
    let body_font = fonts
        .get(&layout.body_font)
        .context("missing card body font")?;
    let signature_font = fonts
        .get(&layout.signature_font)
        .context("missing card signature font")?;
    let mut reports = Vec::new();
    for (row, text) in layout.body_lines.iter().enumerate() {
        let cell = Cell {
            y: layout
                .body_first_row
                .y
                .checked_add(
                    row.checked_mul(layout.line_pitch)
                        .context("letter pitch overflow")?,
                )
                .context("letter row overflow")?,
            ..layout.body_first_row
        };
        ensure!(
            !crate::tim::cells_overlap(cell, layout.signature_cell),
            "letter body overlaps signature"
        );
        let corrections = letter
            .hand_corrections
            .iter()
            .filter(|c| text.contains(&c.before_text))
            .collect::<Vec<_>>();
        ensure!(
            corrections.len() <= 1,
            "multiple hand corrections occupy one line"
        );
        reports.push(if let Some(correction) = corrections.first() {
            render_corrected_line(
                pixels,
                text,
                correction,
                cell,
                layout.body_alignment,
                body_font,
                rasterizers,
                ink,
                &paper.protected_source_regions,
            )?
        } else {
            render_line(
                pixels,
                text,
                cell,
                layout.body_alignment,
                body_font,
                rasterizers,
                ink,
                &paper.protected_source_regions,
            )?
        });
    }
    reports.push(render_line(
        pixels,
        &letter.korean_signature,
        layout.signature_cell,
        HorizontalTextAlignment::Right,
        signature_font,
        rasterizers,
        ink,
        &paper.protected_source_regions,
    )?);
    verify_preservation(
        source,
        &output,
        tim.pixel_offset,
        &paper.protected_source_regions,
    )?;
    Ok((output, reports))
}

fn compress_member(stored: &[u8], patched: &[u8]) -> Result<(Vec<u8>, serde_json::Value)> {
    let source_profile = source_paged_compression_profile(stored)?;
    let (compressed, method, prefix) = match compress_page_safe_image(patched, source_profile) {
        Ok(compressed) => (compressed, "aligned_control_blocks", 0),
        Err(alignment_error) => {
            let source = decompress(stored, true)?;
            ensure!(source.len() == patched.len(), "card decoded extent changed");
            let prefix = source
                .iter()
                .zip(patched)
                .position(|(a, b)| a != b)
                .unwrap_or(source.len())
                & !1;
            // Preserve source control blocks rather than modifying artwork to
            // pay for realigning an otherwise valid source stream. The shared
            // encoder also bounds each decoder invocation by the source budget.
            let compressed = compress_with_source_page_work_limit(patched, stored, prefix)
                .with_context(|| {
                    format!("card source-prefix compression after {alignment_error:#}")
                })?;
            (compressed, "source_prefix_with_page_work_limit", prefix)
        }
    };
    let report = json!({
        "method": method,
        "unchanged_decoded_prefix_bytes": prefix,
        "source_profile": source_profile,
        "rebuilt_profile": source_paged_compression_profile(&compressed)?,
    });
    Ok((compressed, report))
}

fn write_member(
    original: &[u8],
    output: &mut [u8],
    member: TzzMember,
    patched: &[u8],
    output_dir: &Path,
) -> Result<(usize, serde_json::Value)> {
    let index = member.index;
    let stored = &original[member.compressed_range()];
    let (compressed, compression_report) = compress_member(stored, patched)
        .with_context(|| format!("card member {index} compression"))?;
    ensure!(
        compressed.len() <= stored.len() && decompress(&compressed, false)? == patched,
        "card compressed extent/roundtrip failed"
    );
    output[member.offset..member.offset + compressed.len()].copy_from_slice(&compressed);
    ensure!(
        decompress(&output[member.compressed_range()], true)? == patched,
        "card stored readback failed"
    );
    let preview = decode_embedded_tim_preview(patched, &parse_embedded_tim_at(patched, 0)?)?;
    write_tim_preview(
        &output_dir.join(format!("card-{:02}.png", index + 1)),
        &preview,
    )?;
    Ok((compressed.len(), compression_report))
}

pub(crate) fn build(
    source: &SupportedSourceDisc,
    assets: &Path,
    fonts: &BTreeMap<String, SizedFontSource>,
    output_dir: &Path,
) -> Result<CardArtBuild> {
    let letters: Entries<Letter> = read_json(&assets.join("letters.json"))?;
    let backgrounds: Entries<Background> = read_json(&assets.join("letter-backgrounds.json"))?;
    let layouts: Entries<LetterLayout> = read_json(&assets.join("letter-layout.json"))?;
    let expected = (29..49).collect::<BTreeSet<_>>();
    for indices in [
        letters
            .entries
            .iter()
            .map(|e| e.member_index)
            .collect::<Vec<_>>(),
        backgrounds.entries.iter().map(|e| e.member_index).collect(),
        layouts.entries.iter().map(|e| e.member_index).collect(),
    ] {
        ensure!(
            indices.len() == expected.len()
                && indices.into_iter().collect::<BTreeSet<_>>() == expected,
            "card letter authoring population changed"
        );
    }
    let (_, original) = source.read_record(PATH)?;
    ensure!(
        original.len() == 5_394_432 && sha256_bytes(&original) == SOURCE_SHA256,
        "card archive source changed"
    );
    let members = parse_tzz(&original)?;
    ensure!(members.len() == 64, "card archive member count changed");
    let mut output = original.clone();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut drafts = BTreeMap::new();
    let whole_images = whole_images::load(assets)?;
    std::fs::create_dir_all(output_dir)?;
    for (&index, entry) in &whole_images {
        let decoded = decompress(&original[members[index].compressed_range()], false)?;
        drafts.insert(index, whole_images::render(&decoded, assets, entry)?);
    }
    for letter in &letters.entries {
        let index = letter.member_index;
        if whole_images.contains_key(&index) {
            continue;
        }
        let member = members[index];
        let paper = backgrounds
            .entries
            .iter()
            .find(|p| p.member_index == index)
            .unwrap();
        let layout = layouts
            .entries
            .iter()
            .find(|p| p.member_index == index)
            .unwrap();
        let stored = &original[member.compressed_range()];
        let decoded = decompress(stored, false)?;
        ensure!(
            sha256_bytes(&decoded) == letter.source_decoded_sha256
                && paper.source_decoded_sha256 == letter.source_decoded_sha256,
            "card letter {index} source binding changed"
        );
        ensure!(
            Path::new(&paper.indices)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            "unsafe card background path"
        );
        let background = std::fs::read(assets.join(&paper.indices))?;
        ensure!(
            sha256_bytes(&background) == paper.indices_sha256,
            "card background {index} changed"
        );
        let (patched, text_report) = render_letter(
            &decoded,
            &background,
            letter,
            paper,
            layout,
            fonts,
            &mut rasterizers,
        )
        .with_context(|| format!("card member {index}"))?;
        drafts.insert(index, (patched, json!({"member_index":index,"source_decoded_sha256":letter.source_decoded_sha256,
            "background_sha256":paper.indices_sha256,"protected_source_regions":paper.protected_source_regions,
            "review_status":letter.review_status,"source_marks":letter.source_marks,"lines":text_report})));
    }
    let credit_layouts: Entries<credits::Layout> = read_json(&assets.join("credit-layout.json"))?;
    let mut owned = expected.clone();
    for layout in &credit_layouts.entries {
        let index = layout.member_index;
        ensure!(
            layout.owns_member() && owned.insert(index),
            "duplicate or out-of-family credit layout"
        );
        if whole_images.contains_key(&index) {
            continue;
        }
        let member = members[index];
        let decoded = decompress(&original[member.compressed_range()], false)?;
        let (patched, report) = credits::render(&decoded, assets, layout, fonts, &mut rasterizers)
            .with_context(|| format!("credit card member {index}"))?;
        drafts.insert(index, (patched, report));
    }
    let decoration_layouts: Entries<decorations::Layout> =
        read_json(&assets.join("decorations.json"))?;
    let logo = if decoration_layouts
        .entries
        .iter()
        .any(|layout| !whole_images.contains_key(&layout.member_index) && layout.has_logo())
    {
        Some(decorations::SmallLogo::load(source, assets)?)
    } else {
        None
    };
    let mut decorated = BTreeSet::new();
    for layout in &decoration_layouts.entries {
        let index = layout.member_index;
        ensure!(
            index < 60 && decorated.insert(index),
            "duplicate or invalid card decoration"
        );
        if whole_images.contains_key(&index) {
            continue;
        }
        let member = members[index];
        let decoded = decompress(&original[member.compressed_range()], false)?;
        let (base, mut report) = drafts.remove(&index).unwrap_or_else(|| {
            (
                decoded.clone(),
                json!({
                    "family":"card_decorations","member_index":index,
                    "source_decoded_sha256":sha256_bytes(&decoded)
                }),
            )
        });
        let (patched, decoration_report) = decorations::render(
            &decoded,
            &base,
            assets,
            layout,
            logo.as_ref(),
            fonts,
            &mut rasterizers,
        )
        .with_context(|| format!("card decoration member {index}"))?;
        report["decorations"] = decoration_report;
        if let Some(remaining) = report
            .get("separate_unmodified_surfaces")
            .and_then(|v| v.as_array())
        {
            let localized = report["decorations"]["surfaces"].as_array().unwrap();
            let remaining = remaining
                .iter()
                .filter(|surface| {
                    let role = match surface.as_str() {
                        Some("card-number footer") => "card_number_footer",
                        Some("game logo") => "game_logo",
                        _ => return true,
                    };
                    !localized.iter().any(|s| s["role"] == role)
                })
                .cloned()
                .collect::<Vec<_>>();
            report["separate_unmodified_surfaces"] = json!(remaining);
        }
        owned.insert(index);
        drafts.insert(index, (patched, report));
    }
    owned.extend(whole_images.keys().copied());
    let mut reports = Vec::new();
    for (index, (patched, mut report)) in drafts {
        let member = members[index];
        let (compressed_size, compression_report) =
            write_member(&original, &mut output, member, &patched, output_dir)?;
        report["source_compressed_size"] = json!(member.compressed_size);
        report["patched_compressed_size"] = json!(compressed_size);
        report["compression"] = compression_report;
        report["patched_decoded_sha256"] = json!(sha256_bytes(&patched));
        report["patched_pixel_sha256"] = json!(sha256_bytes(&patched[544..]));
        reports.push(report);
    }
    ensure!(
        output[..2048] == original[..2048],
        "card archive table changed"
    );
    for member in members {
        if !owned.contains(&member.index) {
            ensure!(
                output[member.slot_range()] == original[member.slot_range()],
                "unowned card member changed"
            );
        } else {
            ensure!(
                output[member.offset + member.compressed_size..member.offset + member.slot_size]
                    == original
                        [member.offset + member.compressed_size..member.offset + member.slot_size],
                "card slot padding changed"
            );
        }
    }
    std::fs::write(output_dir.join("CARD.BIZ"), &output)?;
    let inputs = [
        "letters.json",
        "letter-backgrounds.json",
        "letter-layout.json",
        "credit-layout.json",
        "decorations.json",
    ]
    .into_iter()
    .chain(
        assets
            .join("whole-images.json")
            .try_exists()?
            .then_some("whole-images.json"),
    )
    .chain(
        credit_layouts
            .entries
            .iter()
            .map(credits::Layout::translation_file),
    )
    .collect::<BTreeSet<_>>()
    .into_iter()
    .map(|name| Ok(json!({"path":name,"sha256":sha256_bytes(&std::fs::read(assets.join(name))?)})))
    .collect::<Result<Vec<_>>>()?;
    std::fs::write(
        output_dir.join("card-art-build.json"),
        serde_json::to_vec_pretty(&json!({
            "source_sha256":SOURCE_SHA256,"patched_sha256":sha256_bytes(&output),"inputs":inputs,"members":reports,
            "claim":"development candidate; human language/art review and native verification are separate"
        }))?,
    )?;
    Ok(CardArtBuild {
        record: output,
        source_sha256: SOURCE_SHA256.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original disc and private whole-image authoring inputs"]
    fn preserved_card_art_fits_without_realigning_the_source_prefix() -> Result<()> {
        let source = SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let (_, archive) = source.read_record(PATH)?;
        assert_eq!(sha256_bytes(&archive), SOURCE_SHA256);
        let members = parse_tzz(&archive)?;
        let stored = &archive[members[13].compressed_range()];
        let decoded = decompress(stored, true)?;
        let profile = source_paged_compression_profile(stored)?;
        // The original itself cannot pay the cost of aligning every block.
        assert!(compress_page_safe_image(&decoded, profile).is_err());
        let assets = Path::new("assets/menu/card-art");
        let selected = whole_images::load(assets)?;
        let (patched, _) = whole_images::render(&decoded, assets, &selected[&13])?;
        assert!(compress_page_safe_image(&patched, profile).is_err());
        let (compressed, report) = compress_member(stored, &patched)?;
        assert_eq!(report["method"], "source_prefix_with_page_work_limit");
        assert_eq!(decompress(&compressed, false)?, patched);
        assert!(compressed.len() <= stored.len());
        assert_eq!(
            (compressed.len() - 1) / 0x800,
            (profile.stream_byte_count - 1) / 0x800
        );
        let rebuilt = source_paged_compression_profile(&compressed)?;
        assert!(rebuilt.maximum_input_page_output_words <= profile.maximum_input_page_output_words);
        let prefix = crate::compression::complete_control_block_prefix(
            stored,
            report["unchanged_decoded_prefix_bytes"].as_u64().unwrap() as usize,
        )?;
        assert!(prefix.encoded_byte_count > 0);
        assert_eq!(
            &compressed[..prefix.encoded_byte_count],
            &stored[..prefix.encoded_byte_count]
        );
        // There must be an actual source prefix; arbitrary source replacement
        // cannot use this fallback as an escape from compression constraints.
        let mut changed_head = patched;
        changed_head[0] ^= 1;
        assert!(compress_with_source_page_work_limit(&changed_head, stored, 0).is_err());
        Ok(())
    }

    #[test]
    fn masked_background_preserves_holes_and_allows_opaque_palette_zero() {
        let source = vec![7; 544 + WIDTH * HEIGHT];
        let mut output = source.clone();
        let cell = Cell {
            x: 10,
            y: 20,
            width: 2,
            height: 1,
        };
        let allowed = apply_masked_background(&mut output, &[0, 2], &[1, 0], cell).unwrap();
        assert_eq!(
            &output[544 + 20 * WIDTH + 10..544 + 20 * WIDTH + 12],
            &[0, 7]
        );
        verify_masked_preservation(&source, &output, &allowed).unwrap();
        output[544 + 20 * WIDTH + 11] = 2;
        assert!(verify_masked_preservation(&source, &output, &allowed).is_err());
        let mut unchanged = source.clone();
        assert!(apply_masked_background(&mut unchanged, &[0, 2], &[1, 2], cell).is_err());
        assert_eq!(unchanged, source);
    }

    #[test]
    fn hand_correction_preserves_final_words_and_requires_a_unique_unicode_anchor() {
        let correction = HandCorrection {
            before_text: "함께".into(),
            erased_text: "함꼐".into(),
        };
        assert_eq!(
            corrected_display("쭉 함께 있고", &correction).unwrap(),
            ("쭉 함꼐 함께 있고".into(), "쭉 ".into())
        );
        assert!(corrected_display("쭉 같이 있고", &correction).is_err());
        assert!(corrected_display("함께 함께", &correction).is_err());
        let empty = HandCorrection {
            before_text: String::new(),
            erased_text: "오류".into(),
        };
        assert!(corrected_display("함께", &empty).is_err());
    }

    #[test]
    fn preservation_rejects_palette_portrait_and_protected_drawing_changes() {
        let source = vec![0; 544 + WIDTH * HEIGHT];
        let protected = [Cell {
            x: 271,
            y: 327,
            width: 33,
            height: 43,
        }];
        for at in [20, 544 + 100 * WIDTH, 544 + 330 * WIDTH + 280] {
            let mut patched = source.clone();
            patched[at] = 1;
            assert!(verify_preservation(&source, &patched, 544, &protected).is_err());
        }
        let mut patched = source.clone();
        patched[544 + 250 * WIDTH + 20] = 1;
        assert!(verify_preservation(&source, &patched, 544, &protected).is_ok());
    }
}
