//! Source-bound loading portraits and the separate message TIM in each member.
#[path = "loading_art/plain_message.rs"]
mod plain_message;
pub(crate) use plain_message::{PlainLoadingBuild, prepare as build_plain_message};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;

use crate::compression::decompress;
use crate::development_build_spec::SizedFontSource;
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::paged_compression::{compress_page_safe_image, source_paged_compression_profile};
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{Cell, parse_8bpp_prefix};
use crate::tim_preview::write_tim_preview;
use crate::tzz::parse_tzz;

pub(crate) const PATH: &str = "DAT2/ALOAD32.BIZ";
const MESSAGE_OFFSET: usize = 0x3c800;
const MESSAGE_PIXELS: usize = MESSAGE_OFFSET + 224;
const MESSAGE_LAYOUT: usize = 0x76dac;

pub(crate) struct LoadingArtBuild {
    pub record: Vec<u8>,
    pub source_sha256: String,
}

/// Build the loading family independently for visual iteration before disc composition.
pub fn build_loading_art(cue: &Path, spec: &Path, output_dir: &Path) -> Result<()> {
    ensure!(
        !output_dir.exists(),
        "loading output directory already exists"
    );
    let config = crate::development_build_spec::load_development_build_spec(spec)?;
    let assets = config
        .assets
        .loading_art
        .context("loading assets are not selected")?;
    let source = SupportedSourceDisc::open(cue)?;
    build(&source, &assets, &config.fonts.loading_art, output_dir)?;
    plain_message::prepare(&source, &assets, &config.fonts.loading_art, output_dir)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authoring {
    kind: String,
    source_record: String,
    source_record_size: usize,
    source_record_sha256: String,
    source_header_size: usize,
    source_header_sha256: String,
    entries: Vec<Portrait>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Portrait {
    id: String,
    member_index: usize,
    source_text: String,
    korean_text: String,
    source_stored_sha256: String,
    source_decoded_sha256: String,
    source_tim_sha256: String,
    background_indices: String,
    background_sha256: String,
    text_mask: String,
    text_mask_sha256: String,
    mask_pixels: usize,
    outside_mask_preserved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    portraits: Vec<PortraitLayout>,
    messages: Vec<Message>,
    consumer_spans: Vec<ConsumerSpan>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumerSpan {
    offset: usize,
    size: usize,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortraitLayout {
    id: String,
    labels: Vec<Label>,
    #[serde(default)]
    embedded_lettering: Option<EmbeddedLettering>,
    fill_rgb: [i32; 3],
    shadow_rgb: [i32; 3],
    #[serde(default)]
    shadow_offset: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddedLettering {
    korean_text: String,
    imagegen_sha256: String,
    #[serde(default)]
    dotmend_art_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    text: String,
    font_role: String,
    cell: Cell,
    font_scale: f32,
    #[serde(default)]
    direction: TextDirection,
    #[serde(default)]
    alignment: Option<HorizontalTextAlignment>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TextDirection {
    #[default]
    Vertical,
    Horizontal,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    source_tail_sha256: String,
    source_text: String,
    text: String,
    font_role: String,
    shear: usize,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&std::fs::read(path)?).with_context(|| path.display().to_string())
}
fn private_pixels(assets: &Path, path: &str, hash: &str) -> Result<Vec<u8>> {
    let relative = Path::new(path).strip_prefix("assets/private-artwork/loading")?;
    ensure!(
        relative
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "unsafe loading input path"
    );
    let bytes = std::fs::read(assets.join("../../private-artwork/loading").join(relative))?;
    ensure!(
        bytes.len() == 512 * 480 && sha256_bytes(&bytes) == hash,
        "loading private input changed: {path}"
    );
    Ok(bytes)
}
fn nearest_palette(tim: &[u8], rgb: [i32; 3]) -> u8 {
    (0..256)
        .min_by_key(|&i| {
            let color = u16::from_le_bytes([tim[20 + i * 2], tim[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(c, s)| (i32::from((color >> s) & 31) * 255 / 31 - rgb[c]).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}
fn admit_background(original: &[u8], background: &[u8], mask: &[u8]) -> Result<()> {
    ensure!(
        original.len() == background.len() && mask.len() == original.len(),
        "loading mask geometry changed"
    );
    ensure!(mask.iter().all(|&m| m <= 1), "loading mask is not binary");
    ensure!(
        original
            .iter()
            .zip(background)
            .zip(mask)
            .all(|((&a, &b), &m)| m == 1 || a == b),
        "loading restoration changed unowned pixels"
    );
    Ok(())
}
fn validate_portrait_wording(
    source_text: &str,
    korean_text: &str,
    layout: &PortraitLayout,
) -> Result<()> {
    ensure!(!source_text.is_empty(), "loading source wording is empty");
    if let Some(art) = &layout.embedded_lettering {
        // Perspective lettering is already in the admitted indexed image.
        // Rendering labels again would overwrite the illustration's local shape.
        ensure!(
            layout.labels.is_empty()
                && !art.korean_text.is_empty()
                && art.korean_text == korean_text
                && art.imagegen_sha256.len() == 64
                && art.imagegen_sha256.bytes().all(|c| c.is_ascii_hexdigit())
                && art
                    .dotmend_art_id
                    .as_ref()
                    .is_none_or(|id| id.starts_with("art_")),
            "loading embedded lettering identity or rendering mode changed"
        );
    } else {
        ensure!(
            layout
                .labels
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
                == korean_text,
            "loading portrait wording/layout mismatch"
        );
    }
    Ok(())
}

fn render_portrait(
    assets: &Path,
    fonts: &BTreeMap<String, SizedFontSource>,
    rasterizers: &mut IndexedTextRasterizers,
    source: &[u8],
    portrait: &Portrait,
    layout: &PortraitLayout,
) -> Result<Vec<u8>> {
    let tim = parse_8bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == 512
            && tim.image_height == 480
            && sha256_bytes(&source[..tim.total_size]) == portrait.source_tim_sha256,
        "loading portrait source TIM changed"
    );
    let start = tim.total_size - 512 * 480;
    let mask = private_pixels(assets, &portrait.text_mask, &portrait.text_mask_sha256)?;
    let mut pixels = private_pixels(
        assets,
        &portrait.background_indices,
        &portrait.background_sha256,
    )?;
    admit_background(&source[start..tim.total_size], &pixels, &mask)?;
    ensure!(
        portrait.outside_mask_preserved
            && mask.iter().map(|&x| usize::from(x)).sum::<usize>() == portrait.mask_pixels,
        "loading background mask binding changed"
    );
    validate_portrait_wording(&portrait.source_text, &portrait.korean_text, layout)?;
    let fill = nearest_palette(source, layout.fill_rgb);
    let shadow = nearest_palette(source, layout.shadow_rgb);
    let shadow_offset = layout.shadow_offset.unwrap_or(3);
    ensure!(
        shadow_offset <= 4,
        "loading shadow offset exceeds supported extent"
    );
    for label in &layout.labels {
        let c = label.cell;
        ensure!(
            c.width > 0 && c.height > 0 && c.x + c.width <= 512 && c.y + c.height <= 480,
            "loading name cell outside image"
        );
        ensure!(
            label.font_scale.is_finite() && label.font_scale > 0.0 && label.font_scale <= 2.0,
            "invalid loading name scale"
        );
        let style = fonts
            .get(&label.font_role)
            .context("missing loading name font role")?;
        let chars = match label.direction {
            TextDirection::Vertical => label
                .text
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>(),
            TextDirection::Horizontal => vec![label.text.clone()],
        };
        ensure!(!chars.is_empty(), "empty loading name");
        let height = c.height / chars.len();
        for (row, ch) in chars.iter().enumerate() {
            let raster = rasterizers.for_font(&style.path)?.rasterize(
                ch,
                c.width,
                height,
                style.font_px * label.font_scale,
                0.0,
                0,
                None,
                1,
                label.alignment.unwrap_or(HorizontalTextAlignment::Center),
            )?;
            // Offset shadow first, then original ink. Both must remain inside the admitted mask.
            for (shift, color) in [(shadow_offset, shadow), (0, fill)] {
                for y in 0..height {
                    for x in 0..c.width {
                        if raster.pixels[y * c.width + x] == 0 {
                            continue;
                        }
                        let px = c.x + x + shift;
                        let py = c.y + row * height + y + shift;
                        ensure!(
                            px < 512 && py < 480 && mask[py * 512 + px] == 1,
                            "loading name ink escapes mask: {} {:?} at ({px}, {py})",
                            portrait.id,
                            ch
                        );
                        pixels[py * 512 + px] = color;
                    }
                }
            }
        }
    }
    let mut output = source.to_vec();
    output[start..tim.total_size].copy_from_slice(&pixels);
    Ok(output)
}

fn message_pixels(source: &[u8], height: usize) -> Result<&[u8]> {
    ensure!(
        matches!(height, 32 | 48),
        "unsupported loading message height"
    );
    // Retail TIM advertises 256 rows, but the member physically stores only its
    // used prefix. Do not synthesize missing bytes or relax the general TIM parser.
    ensure!(
        source.get(MESSAGE_OFFSET..MESSAGE_OFFSET + 8) == Some(&[16, 0, 0, 0, 8, 0, 0, 0]),
        "loading message TIM changed"
    );
    ensure!(
        source.get(MESSAGE_OFFSET + 8..MESSAGE_PIXELS).is_some(),
        "truncated message header"
    );
    ensure!(
        u32::from_le_bytes(source[MESSAGE_OFFSET + 8..MESSAGE_OFFSET + 12].try_into()?) == 204,
        "message CLUT extent changed"
    );
    source
        .get(MESSAGE_PIXELS..MESSAGE_PIXELS + 128 * height)
        .context("missing physical loading message rows")
}
fn render_message(
    source: &[u8],
    output: &mut [u8],
    message: &Message,
    height: usize,
    fonts: &BTreeMap<String, SizedFontSource>,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<()> {
    let original = message_pixels(source, height)?;
    ensure!(
        !message.source_text.is_empty() && !message.text.is_empty() && message.shear <= 16,
        "invalid loading message layout"
    );
    let style = fonts
        .get(&message.font_role)
        .context("missing loading message font")?;
    let width = 256 - message.shear;
    let raster = rasterizers.for_font(&style.path)?.rasterize(
        &message.text,
        width,
        height,
        style.font_px,
        0.0,
        0,
        None,
        1,
        HorizontalTextAlignment::Center,
    )?;
    let mut pixels = vec![0u8; 256 * height];
    let mut colors = Vec::new();
    for y in 0..height {
        let mut counts = [0usize; 16];
        for x in 0..256 {
            counts[usize::from((original[y * 128 + x / 2] >> (4 * (x % 2))) & 15)] += 1;
        }
        // Retail palettes use 5..=9 for bright gradient ink, 10..=12 for
        // antialiasing/shadow. Counting the shadow makes thin source lettering
        // turn whole Korean scanlines dark.
        let color = (5..10).max_by_key(|&i| counts[i]).unwrap();
        colors.push((counts[color] > 0).then_some(color as u8));
    }
    for y in 0..height {
        let color = colors[y]
            .or_else(|| {
                (0..height)
                    .filter(|&row| colors[row].is_some())
                    .min_by_key(|&row| row.abs_diff(y))
                    .and_then(|row| colors[row])
            })
            .context("loading message has no source ink")?;
        let shift = message.shear * (height - 1 - y) / (height - 1);
        for x in 0..width {
            if raster.pixels[y * width + x] != 0 {
                pixels[y * 256 + x + shift] = color;
            }
        }
    }
    for (i, pair) in pixels.as_chunks::<2>().0.iter().enumerate() {
        output[MESSAGE_PIXELS + i] = pair[0] | pair[1] << 4;
    }
    Ok(())
}

pub(crate) fn build(
    source: &SupportedSourceDisc,
    assets: &Path,
    fonts: &BTreeMap<String, SizedFontSource>,
    output_dir: &Path,
) -> Result<LoadingArtBuild> {
    let authoring: Authoring = read_json(&assets.join("authoring.json"))?;
    let layout: Layout = read_json(&assets.join("layout.json"))?;
    let (_, original) = source.read_record(PATH)?;
    ensure!(
        authoring.kind == "justice_gakuen2_loading_illustration_authoring"
            && authoring.source_record == PATH
            && original.len() == authoring.source_record_size
            && sha256_bytes(&original) == authoring.source_record_sha256,
        "loading archive source changed"
    );
    ensure!(
        authoring.source_header_size == 2048
            && sha256_bytes(&original[..2048]) == authoring.source_header_sha256,
        "loading archive table changed"
    );
    let (_, exe) = source.read_record("SLPS_021.20")?;
    ensure!(
        !layout.consumer_spans.is_empty(),
        "loading consumer bindings missing"
    );
    for span in &layout.consumer_spans {
        ensure!(
            sha256_bytes(
                exe.get(span.offset..span.offset + span.size)
                    .context("loading consumer outside executable")?
            ) == span.sha256,
            "loading consumer instructions changed"
        );
    }
    let members = parse_tzz(&original)?;
    ensure!(members.len() == 50, "loading member population changed");
    let mut seen = BTreeSet::new();
    for p in &authoring.entries {
        ensure!(
            p.member_index < 50
                && seen.insert(p.member_index)
                && layout.portraits.iter().filter(|l| l.id == p.id).count() == 1,
            "duplicate/unbound loading portrait"
        );
    }
    ensure!(
        layout.portraits.len() == authoring.entries.len(),
        "unused loading portrait layout"
    );
    let mut used_messages = BTreeSet::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut output = original.clone();
    let mut reports = Vec::new();
    let mut compression_failures = Vec::new();
    std::fs::create_dir_all(output_dir)?;
    for member in members {
        let stored = &original[member.compressed_range()];
        let decoded = decompress(stored, false)?;
        let tail_hash = sha256_bytes(
            decoded
                .get(MESSAGE_OFFSET..)
                .context("missing message TIM")?,
        );
        let message = layout
            .messages
            .iter()
            .find(|m| m.source_tail_sha256 == tail_hash)
            .context("unbound loading message variant")?;
        ensure!(
            layout
                .messages
                .iter()
                .filter(|m| m.source_tail_sha256 == tail_hash)
                .count()
                == 1,
            "duplicate message variant"
        );
        used_messages.insert(tail_hash);
        let portrait = authoring
            .entries
            .iter()
            .find(|p| p.member_index == member.index);
        let mut patched = if let Some(p) = portrait {
            ensure!(
                sha256_bytes(stored) == p.source_stored_sha256
                    && sha256_bytes(&decoded) == p.source_decoded_sha256,
                "loading portrait member source changed"
            );
            render_portrait(
                assets,
                fonts,
                &mut rasterizers,
                &decoded,
                p,
                layout.portraits.iter().find(|l| l.id == p.id).unwrap(),
            )?
        } else {
            decoded.clone()
        };
        let height = usize::from(exe[MESSAGE_LAYOUT + member.index * 4 + 1]);
        render_message(
            &decoded,
            &mut patched,
            message,
            height,
            fonts,
            &mut rasterizers,
        )?;
        ensure!(
            patched.len() == decoded.len()
                && patched[MESSAGE_PIXELS + 128 * height..]
                    == decoded[MESSAGE_PIXELS + 128 * height..],
            "loading trailing data changed"
        );
        let compressed =
            match compress_page_safe_image(&patched, source_paged_compression_profile(stored)?) {
                Ok(bytes) => bytes,
                Err(error) => {
                    compression_failures.push(format!("member {}: {error:#}", member.index));
                    continue;
                }
            };
        ensure!(
            compressed.len() <= stored.len() && decompress(&compressed, false)? == patched,
            "loading compressed extent/roundtrip failed"
        );
        output[member.offset..member.offset + compressed.len()].copy_from_slice(&compressed);
        ensure!(
            decompress(&output[member.compressed_range()], true)? == patched,
            "loading stored readback failed"
        );
        std::fs::write(
            output_dir.join(format!("member-{:03}.bin", member.index)),
            &patched,
        )?;
        let tim = parse_embedded_tim_at(&patched, 0)?;
        let source_portrait_sha256 = sha256_bytes(&decoded[..tim.total_size]);
        let portrait_sha256 = sha256_bytes(&patched[..tim.total_size]);
        ensure!(
            portrait.is_some() || source_portrait_sha256 == portrait_sha256,
            "unselected loading portrait changed"
        );
        let preview = decode_embedded_tim_preview(&patched, &tim)?;
        write_tim_preview(
            &output_dir.join(format!("portrait-{:03}.png", member.index)),
            &preview,
        )?;
        reports.push(json!({"member":member.index,"portrait":portrait.map(|p|&p.id),"source_portrait_sha256":source_portrait_sha256,"portrait_sha256":portrait_sha256,"source_decoded_sha256":sha256_bytes(&decoded),"patched_decoded_sha256":sha256_bytes(&patched),"source_compressed_size":stored.len(),"patched_compressed_size":compressed.len(),"source_message":message.source_text,"korean_message":message.text}));
    }
    ensure!(
        compression_failures.is_empty(),
        "loading compression failed: {}",
        compression_failures.join("; ")
    );
    ensure!(
        used_messages.len() == layout.messages.len() && output[..2048] == original[..2048],
        "unused message layout or changed archive table"
    );
    std::fs::write(output_dir.join("ALOAD32.BIZ"), &output)?;
    let font_report=fonts.iter().map(|(role,f)| Ok(json!({"role":role,"font_px":f.font_px,"sha256":sha256_bytes(&std::fs::read(&f.path)?)}))).collect::<Result<Vec<_>>>()?;
    std::fs::write(
        output_dir.join("loading-art-build.json"),
        serde_json::to_vec_pretty(
            &json!({"source_sha256":authoring.source_record_sha256,"patched_sha256":sha256_bytes(&output),"authoring_sha256":sha256_bytes(&std::fs::read(assets.join("authoring.json"))?),"layout_sha256":sha256_bytes(&std::fs::read(assets.join("layout.json"))?),"fonts":font_report,"members":reports}),
        )?,
    )?;
    Ok(LoadingArtBuild {
        record: output,
        source_sha256: authoring.source_record_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_rejects_changes_outside_binary_mask() {
        assert!(admit_background(&[1, 2, 3], &[1, 9, 3], &[0, 1, 0]).is_ok());
        assert!(admit_background(&[1, 2, 3], &[9, 2, 3], &[0, 1, 0]).is_err());
        assert!(admit_background(&[1, 2, 3], &[1, 2, 3], &[0, 2, 0]).is_err());
    }
    #[test]
    fn message_requires_physically_present_rows_despite_nominal_tim_height() {
        let height = 32;
        let mut source = vec![0u8; MESSAGE_PIXELS + 128 * height];
        source[MESSAGE_OFFSET..MESSAGE_OFFSET + 8].copy_from_slice(&[16, 0, 0, 0, 8, 0, 0, 0]);
        source[MESSAGE_OFFSET + 8..MESSAGE_OFFSET + 12].copy_from_slice(&204u32.to_le_bytes());
        assert_eq!(message_pixels(&source, height).unwrap().len(), 128 * height);
        source.pop();
        assert!(message_pixels(&source, height).is_err());
    }
}

#[cfg(test)]
mod embedded_lettering_tests {
    use super::*;

    #[test]
    fn precomposed_lettering_cannot_be_repainted_or_relabelled() {
        let mut layout: PortraitLayout = serde_json::from_value(serde_json::json!({
            "id": "newspaper", "labels": [], "fill_rgb": [0,0,0], "shadow_rgb": [0,0,0],
            "embedded_lettering": {"korean_text": "태양신문", "imagegen_sha256": "a".repeat(64), "dotmend_art_id": "art_candidate"}
        })).unwrap();
        assert!(validate_portrait_wording("太陽新聞", "태양신문", &layout).is_ok());
        // Direct palette conversion has no Dotmend artifact. The actual
        // indexed bytes remain hash-bound by Portrait.background_sha256.
        layout.embedded_lettering.as_mut().unwrap().dotmend_art_id = None;
        assert!(validate_portrait_wording("太陽新聞", "태양신문", &layout).is_ok());
        layout.embedded_lettering.as_mut().unwrap().imagegen_sha256 = "invalid".into();
        assert!(validate_portrait_wording("太陽新聞", "태양신문", &layout).is_err());
        layout.embedded_lettering.as_mut().unwrap().imagegen_sha256 = "a".repeat(64);
        assert!(validate_portrait_wording("太陽新聞", "다른 글자", &layout).is_err());
        layout.labels.push(
            serde_json::from_value(serde_json::json!({
                "text": "태양신문", "font_role": "caption_handwriting", "font_scale": 1.0,
                "cell": {"x": 0, "y": 0, "width": 40, "height": 16}
            }))
            .unwrap(),
        );
        assert!(validate_portrait_wording("太陽新聞", "태양신문", &layout).is_err());
        layout.embedded_lettering = None;
        assert!(validate_portrait_wording("太陽新聞", "태양신문", &layout).is_ok());
    }
}
