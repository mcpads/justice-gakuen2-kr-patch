//! Vertical announcement artwork; retain every original palette variant.
use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::{ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(super) const SPECS: [ModeDescendantRecordSpec; 2] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::VerticalAnnouncementEffects,
        surface: ModeDescendantSurface::GorinGameplayHud,
        source_path: "DAT2/CEFT4.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "ceft4-announcements.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: 0x10800,
            preview_file: "ceft4-announcements.png",
        }],
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::AlternateVerticalAnnouncementEffects,
        surface: ModeDescendantSurface::GorinGameplayHud,
        source_path: "DAT2/CEFT6.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "ceft6-announcements.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: 0x10800,
            preview_file: "ceft6-announcements.png",
        }],
    },
];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    source_text: String,
    korean_text: String,
    indices_file: PathBuf,
    indices_sha256: String,
    source_indexed_sha256: String,
    imagegen_sha256: String,
    correction_imagegen_sha256: String,
    dotmend_art_id: String,
}

#[derive(Debug, Serialize)]
pub struct AnnouncementReport {
    pub manifest_sha256: String,
    pub indices_sha256: String,
    pub source_paths: Vec<String>,
    pub all_source_palettes_preserved: bool,
    pub palette_residency_verified: bool,
    pub runtime_verified: bool,
}

pub(super) fn apply(
    path: &Path,
    sources: &[ModeDescendantSourceRecord],
    drafts: &mut Vec<ModeDescendantRecordDraft>,
) -> Result<AnnouncementReport> {
    let bytes = std::fs::read(path)?;
    let art: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(
        art.source_text == "開始 / 終了" && art.korean_text == "시작 / 종료",
        "announcement wording changed"
    );
    for hash in [&art.imagegen_sha256, &art.correction_imagegen_sha256] {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "announcement provenance missing"
        );
    }
    ensure!(
        art.dotmend_art_id.starts_with("art_"),
        "announcement Dotmend provenance missing"
    );
    let pixels = std::fs::read(&art.indices_file)?;
    ensure!(
        sha256_bytes(&pixels) == art.indices_sha256,
        "announcement artwork changed"
    );
    ensure!(
        pixels.len() == 160 * 96 && pixels.iter().all(|p| *p < 16),
        "announcement canvas or index range changed"
    );
    let cell = Cell {
        x: 256,
        y: 0,
        width: 160,
        height: 96,
    };
    for spec in &SPECS {
        let source = source_for_spec(sources, spec)?;
        let original = read_indexed_cell_in_prefix(&source.decoded, 0x10800, cell)?;
        ensure!(
            sha256_bytes(&original) == art.source_indexed_sha256,
            "announcement source cells changed"
        );
        ensure!(
            sha256_bytes(&source.decoded[0x10814..0x10c14])
                == "7ff19236e21313cbd8c01391697d2ba12d5191391e28e837c211aad9113e1b7a",
            "announcement palette table changed"
        );
        let mut decoded = source.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut decoded, 0x10800, cell, &pixels)?;
        ensure!(
            read_indexed_cell_in_prefix(&decoded, 0x10800, cell)? == pixels,
            "announcement readback differs"
        );
        ensure!(
            decoded[0x10814..0x10c14] == source.decoded[0x10814..0x10c14],
            "announcement palette write forbidden"
        );
        let claims = DecodedDataClaim::from_ranges(
            "vertical-announcements",
            "Korean start and finish columns, original indexed palettes retained",
            difference_ranges(&source.decoded, &decoded),
        );
        drafts.push(ModeDescendantRecordDraft {
            spec,
            decoded,
            decoded_write_claims: claims,
        });
    }
    Ok(AnnouncementReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: art.indices_sha256,
        source_paths: SPECS.iter().map(|s| s.source_path.into()).collect(),
        all_source_palettes_preserved: true,
        palette_residency_verified: false,
        runtime_verified: false,
    })
}
