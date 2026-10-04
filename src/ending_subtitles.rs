//! Subtitle pages keep their original script addresses, timing words and atlas consumers.
use crate::{
    compression::decompress,
    development_build_spec::SizedFontSource,
    font::{HorizontalTextAlignment, IndexedTextRasterizer},
    pipeline::sha256_bytes,
    source_disc::SupportedSourceDisc,
    tim::{
        Cell, RgbaImage, read_4bpp_indexed_image_in_prefix, read_4bpp_palette_words_in_prefix,
        write_indexed_cell_in_prefix_with_report,
    },
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sources {
    source_sha256: String,
    records: Vec<Overlay>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    path: String,
    source_sha256: String,
    renderer: usize,
    pools: Vec<Pool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pool {
    occurrences: Vec<Occurrence>,
    pages: Vec<Page>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Occurrence {
    path: String,
    source_sha256: String,
    decoded_sha256: String,
    tim_offset: usize,
    tim_size: usize,
    tim_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    id: String,
    offset: usize,
    end: usize,
    terminal_offset: usize,
    source_hex: String,
    source_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Texts {
    source_record: String,
    source_sha256: String,
    review_status: String,
    pages: Vec<Text>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    id: String,
    source: String,
    text: String,
    lines: Vec<String>,
}

pub(crate) struct Record {
    pub path: String,
    pub source_sha256: String,
    pub output: Vec<u8>,
}
pub(crate) struct Build {
    pub records: Vec<Record>,
}

// Glyph rasters use neutral roles 0=fill, 1=outline, 15=clear. Ending
// CLUTs differ in ordering; in ED06/10 the white and clear ends are reversed.
fn palette_roles(words: &[u16; 16]) -> Result<[u8; 16]> {
    ensure!(
        words.iter().all(|w| {
            let r = w & 31;
            r == (w >> 5) & 31 && r == (w >> 10) & 31
        }),
        "ending subtitle palette is not grayscale"
    );
    let clear = words
        .iter()
        .rposition(|&w| w == 0)
        .context("ending subtitle palette has no transparent entry")?;
    let fill = words
        .iter()
        .position(|&w| w & 0x7fff == 0x7fff)
        .context("ending subtitle palette has no white entry")?;
    let outline = words
        .iter()
        .enumerate()
        .filter(|(_, w)| **w != 0)
        .min_by_key(|(_, w)| **w & 31)
        .map(|(i, _)| i)
        .context("ending subtitle palette has no visible outline")?;
    ensure!(
        words[outline] & 31 < 16,
        "ending subtitle outline is not dark"
    );
    let mut roles = std::array::from_fn(|i| i as u8);
    roles[0] = fill as u8;
    roles[1] = outline as u8;
    roles[15] = clear as u8;
    Ok(roles)
}

// Native runs require contiguous glyph IDs. Merge overlapping lines rather than
// assigning a duplicate copy of every repeated name, sentence or punctuation.
fn glyph_pool(lines: &BTreeSet<Vec<char>>) -> Vec<char> {
    let mut parts = lines
        .iter()
        .filter(|a| {
            !lines
                .iter()
                .any(|b| b.len() > a.len() && b.windows(a.len()).any(|w| w == a.as_slice()))
        })
        .cloned()
        .collect::<Vec<_>>();
    while parts.len() > 1 {
        let mut best = (0, 0, 1);
        for a in 0..parts.len() {
            for b in 0..parts.len() {
                if a == b {
                    continue;
                }
                let n = (1..=parts[a].len().min(parts[b].len()))
                    .rev()
                    .find(|&n| parts[a][parts[a].len() - n..] == parts[b][..n])
                    .unwrap_or(0);
                if n > best.0 {
                    best = (n, a, b);
                }
            }
        }
        let (overlap, a, b) = best;
        let tail = parts[b][overlap..].to_vec();
        parts[a].extend(tail);
        parts.remove(b);
    }
    parts.pop().unwrap_or_default()
}
fn first(pool: &[char], line: &str) -> Result<usize> {
    let chars = line.chars().collect::<Vec<_>>();
    pool.windows(chars.len())
        .position(|w| w == chars)
        .context("ending line missing from contiguous glyph pool")
}
fn program(page: &Page, source: &[u8], pool: &[char], lines: &[String]) -> Result<Vec<u8>> {
    ensure!(
        !lines.is_empty() && lines.len() <= 4,
        "{}: invalid line count",
        page.id
    );
    ensure!(
        lines
            .iter()
            .all(|l| !l.is_empty() && l.chars().count() <= 26),
        "{}: line exceeds native right edge",
        page.id
    );
    ensure!(
        lines.iter().map(|l| l.chars().count()).sum::<usize>() <= 70,
        "{}: native glyph packet buffer overflow",
        page.id
    );
    let mut data = source[page.offset..page.offset + 4].to_vec();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            data.extend(0xfffeu16.to_le_bytes());
        }
        data.extend(u16::try_from(first(pool, line)?)?.to_le_bytes());
        data.extend(u16::try_from(line.chars().count())?.to_le_bytes());
    }
    let size = page.terminal_offset - page.offset;
    ensure!(
        data.len() <= size,
        "{}: commands exceed original page span",
        page.id
    );
    // The original renderer skips zero-length runs before allocating a packet.
    while size - data.len() >= 4 {
        data.extend([0, 0, 0, 0]);
    }
    if data.len() < size {
        data.extend(0xfffdu16.to_le_bytes());
    }
    data.extend(&source[page.terminal_offset..page.end]);
    ensure!(
        data.len() == page.end - page.offset,
        "ending page length changed"
    );
    Ok(data)
}
fn preview(path: &Path, lines: &[String], glyphs: &BTreeMap<char, Vec<u8>>) -> Result<()> {
    let mut pixels = vec![0; 448 * 144 * 4];
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            for (i, &v) in glyphs[&ch].iter().enumerate() {
                let color = match v {
                    0 => 255,
                    1 => 8,
                    _ => 0,
                };
                let pos = ((y * 36 + i / 16) * 448 + x * 17 + i % 16) * 4;
                pixels[pos..pos + 3].fill(color);
            }
        }
    }
    crate::tim_preview::write_tim_preview(
        path,
        &RgbaImage {
            width: 448,
            height: 144,
            pixels,
        },
    )
}

pub(crate) fn build(
    source: &SupportedSourceDisc,
    assets: &Path,
    font: &SizedFontSource,
    output: &Path,
) -> Result<Build> {
    let manifest: Sources = serde_json::from_slice(&std::fs::read(assets.join("source.json"))?)?;
    ensure!(
        manifest.source_sha256
            == "62dbc6ca47ec8d9dfbb5797f35d4e720d63e00b5a8e0edd080b149c3ff4f1823"
            && manifest.records.len() == 24,
        "ending source population changed"
    );
    let raster = IndexedTextRasterizer::load(&font.path)?;
    let mut glyphs = BTreeMap::new();
    glyphs.insert(' ', vec![15; 16 * 24]);
    let mut records = Vec::new();
    let mut graphics: BTreeMap<String, (Vec<u8>, Vec<u8>, String, usize)> = BTreeMap::new();
    let mut reports = Vec::new();
    let mut total_pages = 0;
    let mut total_fonts = 0;
    std::fs::create_dir_all(output)?;
    for overlay in manifest.records {
        let (_, original) = source.read_record(&overlay.path)?;
        ensure!(
            sha256_bytes(&original) == overlay.source_sha256,
            "ending overlay source changed"
        );
        ensure!(
            original.get(overlay.renderer..overlay.renderer + 4) == Some(&[0xb0, 0xff, 0xbd, 0x27]),
            "ending renderer binding changed"
        );
        let name = Path::new(&overlay.path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();
        let text: Texts = serde_json::from_slice(&std::fs::read(
            assets.join("text").join(format!("{name}.json")),
        )?)?;
        ensure!(
            text.source_record == overlay.path
                && text.source_sha256 == overlay.source_sha256
                && !text.review_status.is_empty(),
            "ending translation source mismatch"
        );
        let mut translations = BTreeMap::new();
        for entry in &text.pages {
            ensure!(
                !entry.text.is_empty() && !entry.source.is_empty(),
                "ending text/source is empty"
            );
            ensure!(
                entry.text.split_whitespace().collect::<String>()
                    == entry.lines.join("").split_whitespace().collect::<String>(),
                "{}: layout dropped authored text",
                entry.id
            );
            ensure!(
                translations.insert(entry.id.as_str(), entry).is_none(),
                "duplicate ending text ID"
            );
            for ch in entry.lines.iter().flat_map(|l| l.chars()) {
                if let std::collections::btree_map::Entry::Vacant(e) = glyphs.entry(ch) {
                    e.insert(
                        raster
                            .rasterize(
                                &ch.to_string(),
                                16,
                                24,
                                font.font_px,
                                0.0,
                                15,
                                Some(1),
                                0,
                                HorizontalTextAlignment::Center,
                            )
                            .with_context(|| format!("ending glyph {ch}"))?
                            .pixels,
                    );
                }
            }
        }
        let mut patched = original.clone();
        let mut spans = Vec::new();
        let mut used = BTreeSet::new();
        for group in overlay.pools {
            let lines = group
                .pages
                .iter()
                .map(|p| {
                    translations
                        .get(p.id.as_str())
                        .with_context(|| format!("missing {}", p.id))
                })
                .collect::<Result<Vec<_>>>()?
                .iter()
                .flat_map(|t| t.lines.iter().map(|s| s.chars().collect()))
                .collect::<BTreeSet<Vec<char>>>();
            let pool = glyph_pool(&lines);
            for page in &group.pages {
                ensure!(
                    page.offset + 4 <= page.terminal_offset
                        && page.terminal_offset + 2 <= page.end
                        && page.end <= original.len(),
                    "invalid ending source page bounds"
                );
                let src = &original[page.offset..page.end];
                ensure!(
                    sha256_bytes(src) == page.source_sha256
                        && src.iter().map(|b| format!("{b:02x}")).collect::<String>()
                            == page.source_hex,
                    "{}: original page bytes changed",
                    page.id
                );
                ensure!(used.insert(page.id.clone()), "ending page duplicated");
                let lines = &translations[page.id.as_str()].lines;
                let bytes = program(page, &original, &pool, lines)?;
                patched[page.offset..page.end].copy_from_slice(&bytes);
                spans.push(page.offset + 4..page.terminal_offset);
                preview(&output.join(format!("{}.png", page.id)), lines, &glyphs)?;
                reports.push(json!({"id":page.id,"lines":lines,"source_sha256":page.source_sha256,"output_sha256":sha256_bytes(&bytes),"offset":page.offset,"end":page.end,"glyphs":lines.iter().map(|l|l.chars().count()).sum::<usize>()}));
                total_pages += 1;
            }
            for occurrence in group.occurrences {
                if !graphics.contains_key(&occurrence.path) {
                    let (_, stored) = source.read_record(&occurrence.path)?;
                    ensure!(
                        sha256_bytes(&stored) == occurrence.source_sha256,
                        "ending graphics source changed"
                    );
                    let decoded = decompress(&stored, true)?;
                    ensure!(
                        sha256_bytes(&decoded) == occurrence.decoded_sha256,
                        "ending decoded source changed"
                    );
                    graphics.insert(
                        occurrence.path.clone(),
                        (
                            stored,
                            decoded,
                            occurrence.source_sha256.clone(),
                            occurrence.tim_offset,
                        ),
                    );
                }
                let (_, decoded, _, prefix) = graphics.get_mut(&occurrence.path).unwrap();
                *prefix = (*prefix).min(occurrence.tim_offset);
                ensure!(
                    sha256_bytes(
                        &decoded
                            [occurrence.tim_offset..occurrence.tim_offset + occurrence.tim_size]
                    ) == occurrence.tim_sha256,
                    "ending font TIM source/alias changed"
                );
                let atlas = read_4bpp_indexed_image_in_prefix(decoded, occurrence.tim_offset)?;
                let roles = palette_roles(&read_4bpp_palette_words_in_prefix(
                    decoded,
                    occurrence.tim_offset,
                    0,
                )?)?;
                let capacity = atlas.width / 256 * (atlas.height / 24) * 16;
                ensure!(
                    pool.len() <= capacity,
                    "{}: {} glyphs exceed {capacity} cells",
                    occurrence.path,
                    pool.len()
                );
                let mut pixels = vec![15; atlas.width * atlas.height];
                for (code, ch) in pool.iter().enumerate() {
                    let x = code / 160 * 256 + code % 16 * 16;
                    let y = code % 160 / 16 * 24;
                    ensure!(
                        x + 16 <= atlas.width && y + 24 <= atlas.height,
                        "ending glyph address out of atlas"
                    );
                    for row in 0..24 {
                        pixels[(y + row) * atlas.width + x..(y + row) * atlas.width + x + 16]
                            .copy_from_slice(&glyphs[ch][row * 16..row * 16 + 16]);
                    }
                }
                for pixel in &mut pixels {
                    ensure!(
                        matches!(*pixel, 0 | 1 | 15),
                        "unexpected neutral subtitle pixel"
                    );
                    *pixel = roles[*pixel as usize];
                }
                write_indexed_cell_in_prefix_with_report(
                    decoded,
                    occurrence.tim_offset,
                    Cell {
                        x: 0,
                        y: 0,
                        width: atlas.width,
                        height: atlas.height,
                    },
                    &pixels,
                )?;
                total_fonts += 1;
            }
        }
        ensure!(
            used.len() == translations.len(),
            "unused ending translations"
        );
        ensure!(
            original
                .iter()
                .zip(&patched)
                .enumerate()
                .all(|(i, (a, b))| a == b || spans.iter().any(|s| s.contains(&i))),
            "ending commands changed outside owned page bodies"
        );
        std::fs::write(output.join(format!("{name}.BIN")), &patched)?;
        records.push(Record {
            path: overlay.path,
            source_sha256: overlay.source_sha256,
            output: patched,
        });
    }
    ensure!(
        total_pages == 375 && total_fonts == 56 && graphics.len() == 45,
        "incomplete ending family population"
    );
    let mut compression_reports = Vec::new();
    for (path, (stored, decoded, hash, prefix)) in graphics {
        let compressed = crate::paged_compression::compress_with_source_page_work_limit(
            &decoded, &stored, prefix,
        )?;
        ensure!(
            compressed.len() <= stored.len() && compressed[..4] == stored[..4],
            "{path}: ending compressed slot/catalog overflow"
        );
        compression_reports.push(json!({
            "path": path,
            "source": crate::paged_compression::source_paged_compression_profile(&stored)?,
            "rebuilt": crate::paged_compression::source_paged_compression_profile(&compressed)?,
        }));
        let mut physical = stored.clone();
        physical.fill(0);
        physical[..compressed.len()].copy_from_slice(&compressed);
        ensure!(
            decompress(&physical, true)? == decoded,
            "{path}: ending compressed readback mismatch"
        );
        std::fs::write(
            output.join(Path::new(&path).file_name().unwrap()),
            &physical,
        )?;
        records.push(Record {
            path,
            source_sha256: hash,
            output: physical,
        });
    }
    std::fs::write(
        output.join("ending-subtitles.json"),
        serde_json::to_vec_pretty(
            &json!({"pages":reports,"font_occurrences":total_fonts,"physical_records":records.len(),"runtime_verified":false,"compression":compression_reports}),
        )?,
    )?;
    Ok(Build { records })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subtitle_roles_follow_palette_colors_instead_of_fixed_indices() {
        for words in [
            [
                0xffff, 0, 0x8842, 0x9084, 0x98c6, 0xa108, 0xa94a, 0xad6b, 0xb9ce, 0xc631, 0xce73,
                0xdad6, 0xe318, 0xef7b, 0xf7bd, 0,
            ],
            [
                0, 0x8842, 0x9084, 0x98c6, 0xa108, 0xa94a, 0xb18c, 0xb9ce, 0xc631, 0xd6b5, 0xdef7,
                0xe318, 0xef7b, 0xf7bd, 0xffff, 0xffff,
            ],
            [
                0, 0, 0x8842, 0x9084, 0x9ce7, 0xa529, 0xad6b, 0xb5ad, 0xc210, 0xce73, 0xd6b5,
                0xdef7, 0xeb5a, 0xf39c, 0xfbde, 0xffff,
            ],
            [
                0xffff, 0xfbde, 0xf7bd, 0xef7b, 0xe739, 0xdef7, 0xd6b5, 0xce73, 0xc631, 0xb5ad,
                0xa529, 0x98c6, 0x9084, 0x8842, 0x8421, 0,
            ],
        ] {
            let roles = palette_roles(&words).unwrap();
            assert_eq!(words[roles[15] as usize], 0);
            assert_eq!(words[roles[0] as usize] & 0x7fff, 0x7fff);
            assert_ne!(words[roles[1] as usize], 0);
            assert!(words[roles[1] as usize] & 31 <= 2);
        }
        assert!(palette_roles(&[0xffff; 16]).is_err());
        assert!(palette_roles(&[0; 16]).is_err());
    }
    #[test]
    fn overlapping_native_runs_reuse_glyph_storage() {
        let lines = ["가나다", "다라마", "나다라", "가나다"]
            .into_iter()
            .map(|s| s.chars().collect())
            .collect();
        let pool = glyph_pool(&lines);
        assert_eq!(pool.len(), 5);
        for l in lines {
            assert!(pool.windows(l.len()).any(|w| w == l));
        }
    }
    #[test]
    fn page_rewrite_keeps_headers_terminal_and_delays() {
        let src = [1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 255, 255, 90, 0];
        let p = Page {
            id: "test".into(),
            offset: 0,
            end: 16,
            terminal_offset: 12,
            source_hex: String::new(),
            source_sha256: String::new(),
        };
        let out = program(&p, &src, &['가'], &["가".into()]).unwrap();
        assert_eq!(&out[..4], &src[..4]);
        assert_eq!(&out[12..], &src[12..]);
        assert_eq!(&out[8..12], &[0; 4]);
        assert!(program(&p, &src, &['가'], &["가".repeat(27)]).is_err());
    }
}

#[cfg(test)]
mod source_validation {
    use super::*;
    #[test]
    #[ignore = "requires original disc"]
    fn subtitle_palette_roles_cover_original_atlases() -> Result<()> {
        let source = SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let manifest: Sources =
            serde_json::from_slice(&std::fs::read("assets/endings/source.json")?)?;
        let mut seen = BTreeSet::new();
        for overlay in manifest.records {
            for pool in overlay.pools {
                for occurrence in pool.occurrences {
                    if !seen.insert((occurrence.path.clone(), occurrence.tim_offset)) {
                        continue;
                    }
                    let (_, stored) = source.read_record(&occurrence.path)?;
                    assert_eq!(sha256_bytes(&stored), occurrence.source_sha256);
                    let decoded = decompress(&stored, true)?;
                    assert_eq!(sha256_bytes(&decoded), occurrence.decoded_sha256);
                    let words =
                        read_4bpp_palette_words_in_prefix(&decoded, occurrence.tim_offset, 0)?;
                    let roles = palette_roles(&words)?;
                    assert_eq!(words[roles[15] as usize], 0, "{} clear", occurrence.path);
                    assert_eq!(
                        words[roles[0] as usize] & 0x7fff,
                        0x7fff,
                        "{} fill",
                        occurrence.path
                    );
                    assert_ne!(words[roles[1] as usize], 0, "{} outline", occurrence.path);
                    // ED22's darkest visible source gray is 4/31; zero is transparent.
                    assert!(
                        words[roles[1] as usize] & 31 <= 4,
                        "{} outline gray",
                        occurrence.path
                    );
                }
            }
        }
        assert_eq!(seen.len(), 56);
        Ok(())
    }
    #[test]
    #[ignore = "requires original disc and private whole-image authoring inputs"]
    fn complete_ending_and_pocket_family_static_build() -> Result<()> {
        let config = crate::development_build_spec::load_development_build_spec(Path::new(
            "assets/build/development.json",
        ))?;
        let source = SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let output = Path::new("work/known-family-static");
        let endings = build(
            &source,
            config.assets.endings.as_deref().unwrap(),
            &config.fonts.dialogue_body,
            &output.join("endings"),
        )?;
        assert_eq!(endings.records.len(), 69);
        let help = crate::pocket_help::build(
            &source,
            config.assets.pocket_help.as_deref().unwrap(),
            &config.fonts,
            &output.join("pocket-help"),
        )?;
        assert_eq!(help.output.len(), 702464);
        Ok(())
    }
}
