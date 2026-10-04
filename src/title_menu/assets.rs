use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::text::read_length_prefixed_codes;

use super::model::{TitleMenuManifest, TitleMenuTranslation};
use super::source::{
    MENU_PATH, MENU_STORED_SHA256, OVERLAY_DECODED_SHA256, OVERLAY_PATH, OVERLAY_RUNTIME_BASE,
    OVERLAY_STORED_SHA256, TitleMenuSource,
};

const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 title-adjacent menu translation manifest";
const TRANSLATION_KIND: &str = "Justice Gakuen 2 title-adjacent menu translation unit";
const EXPECTED_IDS: [&str; 2] = ["new_enrollment", "continue_session"];

pub(super) struct LoadedTitleMenuAssets {
    pub(super) manifest_sha256: String,
    pub(super) entries: Vec<TitleMenuTranslation>,
}

pub(super) fn load_assets(root: &Path, source: &TitleMenuSource) -> Result<LoadedTitleMenuAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: TitleMenuManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown title-adjacent menu manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256,
        "title-adjacent menu manifest source BIN changed"
    );
    ensure!(
        manifest.menu_path == MENU_PATH
            && manifest.menu_stored_sha256 == MENU_STORED_SHA256
            && manifest.menu_decoded_sha256 == sha256_bytes(&source.menu_decoded),
        "title-adjacent menu manifest MENU.BIZ identity changed"
    );
    ensure!(
        manifest.overlay_path == OVERLAY_PATH
            && manifest.overlay_stored_sha256 == OVERLAY_STORED_SHA256
            && manifest.overlay_decoded_sha256 == OVERLAY_DECODED_SHA256,
        "title-adjacent menu manifest MGTIT.BIZ identity changed"
    );
    ensure!(
        manifest.entries.len() == EXPECTED_IDS.len(),
        "title-adjacent menu manifest must contain both observed entries"
    );

    let mut ids = BTreeSet::new();
    let mut record_ranges = Vec::new();
    let mut entries = Vec::with_capacity(manifest.entries.len());
    for (index, reference) in manifest.entries.into_iter().enumerate() {
        ensure!(
            reference.id == EXPECTED_IDS[index] && ids.insert(reference.id.clone()),
            "title-adjacent menu entry order or identity changed"
        );
        validate_relative_file(&reference.file)?;
        let path = root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let entry: TitleMenuTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            entry.kind == TRANSLATION_KIND && entry.id == reference.id,
            "title-adjacent menu translation identity changed"
        );
        validate_entry(&entry, source)?;
        let start = parse_hex_usize(&entry.source_offset, "source offset")?;
        record_ranges.push([start, start + entry.source_record_size]);
        entries.push(entry);
    }
    record_ranges.sort();
    ensure!(
        record_ranges
            .windows(2)
            .all(|pair| pair[0][1] <= pair[1][0]),
        "title-adjacent menu source records overlap"
    );
    Ok(LoadedTitleMenuAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        entries,
    })
}

fn validate_entry(entry: &TitleMenuTranslation, source: &TitleMenuSource) -> Result<()> {
    ensure!(
        !entry.source_text.trim().is_empty() && !entry.korean_text.trim().is_empty(),
        "title-adjacent menu {} contains empty text",
        entry.id
    );
    let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
    let source_end = source_offset
        .checked_add(entry.source_record_size)
        .context("title-adjacent source record range overflow")?;
    let source_record = source
        .overlay_decoded
        .get(source_offset..source_end)
        .context("title-adjacent source record is out of bounds")?;
    ensure!(
        sha256_bytes(source_record) == entry.source_record_sha256,
        "title-adjacent menu {} source record changed",
        entry.id
    );
    let expected_codes = entry
        .source_codes
        .iter()
        .map(|code| parse_hex_u16(code, "source code"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        read_length_prefixed_codes(&source.overlay_decoded, source_offset)? == expected_codes,
        "title-adjacent menu {} source codes changed",
        entry.id
    );
    ensure!(
        2 + expected_codes.len() * 2 <= entry.source_record_size,
        "title-adjacent menu {} source record is smaller than its codes",
        entry.id
    );

    let placement_offset = parse_hex_usize(&entry.placement_record_offset, "placement offset")?;
    let placement_end = placement_offset
        .checked_add(12)
        .context("title-adjacent placement range overflow")?;
    let placement = source
        .overlay_decoded
        .get(placement_offset..placement_end)
        .context("title-adjacent placement record is out of bounds")?;
    ensure!(
        sha256_bytes(placement) == entry.source_placement_sha256,
        "title-adjacent menu {} placement record changed",
        entry.id
    );
    ensure!(
        read_i16(placement, 0)? == entry.source_x
            && read_i16(placement, 2)? == entry.source_y
            && read_u32(placement, 4)? == OVERLAY_RUNTIME_BASE + u32::try_from(source_offset)?
            && read_u32(placement, 8)? == 0,
        "title-adjacent menu {} placement consumer binding changed",
        entry.id
    );
    Ok(())
}

pub(super) fn parse_hex_usize(value: &str, label: &str) -> Result<usize> {
    let value = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not hexadecimal"))?;
    usize::from_str_radix(value, 16).with_context(|| format!("invalid {label}"))
}

fn parse_hex_u16(value: &str, label: &str) -> Result<u16> {
    let value = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not hexadecimal"))?;
    u16::from_str_radix(value, 16).with_context(|| format!("invalid {label}"))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16> {
    Ok(i16::from_le_bytes(
        data.get(offset..offset + 2)
            .context("truncated i16")?
            .try_into()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .context("truncated u32")?
            .try_into()?,
    ))
}

fn validate_relative_file(path: &str) -> Result<()> {
    let path = Path::new(path);
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "title-adjacent menu asset path must be a plain relative path"
    );
    Ok(())
}
