use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::{BASELINE_BIN_SHA256, ORIGINAL_MENU_DECODED_SHA256, sha256_bytes};
use crate::text::read_length_prefixed_codes;

use super::model::{TitleNoticeConsumer, TitleNoticeManifest, TitleNoticeTranslation};
use super::source::{
    MENU_PATH, MENU_STORED_SHA256, OVERLAY_DECODED_SHA256, OVERLAY_PATH, OVERLAY_RUNTIME_BASE,
    OVERLAY_STORED_SHA256, TitleNoticeSource,
};

const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_KIND: &str = "Justice Gakuen 2 title notice translation manifest";
const TRANSLATION_KIND: &str = "Justice Gakuen 2 title notice translation unit";
const EXPECTED_IDS: [&str; 11] = [
    "no_file",
    "data_unavailable",
    "await_input",
    "continue_slot_question",
    "continue_empty_slot",
    "continue_clear_slot",
    "mode_return_question",
    "confirmation_choices",
    "card_checking",
    "card_wait",
    "card_missing",
];

pub(super) struct LoadedTitleNoticeAssets {
    pub(super) manifest_sha256: String,
    pub(super) entries: Vec<TitleNoticeTranslation>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &TitleNoticeSource,
) -> Result<LoadedTitleNoticeAssets> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: TitleNoticeManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unknown title notice manifest kind"
    );
    ensure!(
        manifest.source_bin_sha256 == BASELINE_BIN_SHA256
            && manifest.source_bin_sha256 == source.source_bin_sha256,
        "title notice manifest source BIN changed"
    );
    ensure!(
        manifest.menu_path == MENU_PATH
            && manifest.menu_stored_sha256 == MENU_STORED_SHA256
            && manifest.menu_decoded_sha256 == ORIGINAL_MENU_DECODED_SHA256,
        "title notice manifest MENU.BIZ identity changed"
    );
    ensure!(
        manifest.overlay_path == OVERLAY_PATH
            && manifest.overlay_stored_sha256 == OVERLAY_STORED_SHA256
            && manifest.overlay_decoded_sha256 == OVERLAY_DECODED_SHA256,
        "title notice manifest MGTIT.BIZ identity changed"
    );
    ensure!(
        manifest.entries.len() == EXPECTED_IDS.len(),
        "title notice manifest must contain every observed line"
    );

    let mut ids = BTreeSet::new();
    let mut record_ranges = Vec::new();
    let mut placement_ranges = Vec::new();
    let mut entries = Vec::with_capacity(manifest.entries.len());
    for (index, reference) in manifest.entries.into_iter().enumerate() {
        ensure!(
            reference.id == EXPECTED_IDS[index] && ids.insert(reference.id.clone()),
            "title notice entry order or identity changed"
        );
        validate_relative_file(&reference.file)?;
        let path = root.join(&reference.file);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let entry: TitleNoticeTranslation = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            entry.kind == TRANSLATION_KIND && entry.id == reference.id,
            "title notice translation identity changed"
        );
        validate_entry(&entry, source)?;
        let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        record_ranges.push([source_offset, source_offset + entry.source_record_size]);
        if let TitleNoticeConsumer::Placement { record_offset, .. } = &entry.consumer {
            let offset = parse_hex_usize(record_offset, "placement offset")?;
            placement_ranges.push([offset, offset + 12]);
        }
        entries.push(entry);
    }
    record_ranges.sort();
    placement_ranges.sort();
    ensure!(
        record_ranges
            .windows(2)
            .all(|pair| pair[0][1] <= pair[1][0])
            && placement_ranges
                .windows(2)
                .all(|pair| pair[0][1] <= pair[1][0]),
        "title notice records overlap"
    );
    Ok(LoadedTitleNoticeAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        entries,
    })
}

fn validate_entry(entry: &TitleNoticeTranslation, source: &TitleNoticeSource) -> Result<()> {
    ensure!(
        !entry.source_text.trim().is_empty() && !entry.korean_text.trim().is_empty(),
        "title notice {} contains empty text",
        entry.id
    );
    let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
    let source_record = source
        .overlay_decoded
        .get(source_offset..source_offset + entry.source_record_size)
        .context("title notice source record is out of bounds")?;
    ensure!(
        sha256_bytes(source_record) == entry.source_record_sha256,
        "title notice {} source record changed",
        entry.id
    );
    let expected_codes = entry
        .source_codes
        .iter()
        .map(|code| parse_hex_u16(code, "source code"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        read_length_prefixed_codes(&source.overlay_decoded, source_offset)? == expected_codes,
        "title notice {} source codes changed",
        entry.id
    );
    ensure!(
        2 + expected_codes.len() * 2 <= entry.source_record_size,
        "title notice {} source record is smaller than its codes",
        entry.id
    );

    match &entry.consumer {
        TitleNoticeConsumer::Placement {
            record_offset,
            source_x,
            source_y,
            source_record_sha256,
        } => {
            let offset = parse_hex_usize(record_offset, "placement offset")?;
            ensure!(
                (0x2a0..0x378).contains(&offset) && (offset - 0x2a0).is_multiple_of(12),
                "title notice placement is not a native table row"
            );
            let placement = source
                .overlay_decoded
                .get(offset..offset + 12)
                .context("title notice placement record is out of bounds")?;
            ensure!(
                sha256_bytes(placement) == *source_record_sha256,
                "title notice {} placement record changed",
                entry.id
            );
            ensure!(
                read_i16(placement, 0)? == *source_x
                    && read_i16(placement, 2)? == *source_y
                    && read_u32(placement, 4)?
                        == OVERLAY_RUNTIME_BASE + u32::try_from(source_offset)?
                    && read_u32(placement, 8)? == 0,
                "title notice {} placement consumer binding changed",
                entry.id
            );
            crate::menu_audit::validate_mgtit_placement_renderer(&source.overlay_decoded)?;
        }
        TitleNoticeConsumer::ContinueEmptySlots => {
            super::consumer::validate_empty_slot_record(source_offset, entry.source_record_size)?;
            super::consumer::validate_continue_slot_consumers(&source.overlay_decoded)?;
        }
        TitleNoticeConsumer::ContinueClearSlots => {
            ensure!(
                source_offset == 0x6c4 && entry.source_record_size == 16,
                "Continue CLEAR record is not its native consumer target"
            );
            ensure!(
                entry.source_text == "CLEAR!!" && entry.korean_text == entry.source_text,
                "Continue CLEAR must preserve the original English wording"
            );
            super::consumer::validate_continue_slot_consumers(&source.overlay_decoded)?;
        }
    }
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
        "title notice asset path must be a plain relative path"
    );
    Ok(())
}
