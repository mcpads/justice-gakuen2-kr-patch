//! Main title lettering within the original indexed background and audio record.
use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::{ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    Cell, read_8bpp_indexed_cell_in_prefix, write_8bpp_indexed_cell_in_prefix_with_report,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(super) const SPEC: ModeDescendantRecordSpec = ModeDescendantRecordSpec {
    record: ModeDescendantRecord::MainTitle,
    surface: ModeDescendantSurface::MainTitle,
    source_path: "DAT2/TITLE.BIZ",
    storage_kind: ModeDescendantStorageKind::PagedCompressed,
    output_file: "main-title.biz",
    texture_outputs: &[ModeDescendantTextureOutputSpec {
        tim_offset: 0x6000,
        preview_file: "main-title.png",
    }],
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    source_text: String,
    korean_text: String,
    indices_file: PathBuf,
    indices_sha256: String,
    source_indexed_sha256: String,
    source_palette_sha256: String,
    imagegen_sha256: String,
    dotmend_art_id: String,
}

#[derive(Debug, Serialize)]
pub struct MainTitleReport {
    pub manifest_sha256: String,
    pub indices_sha256: String,
    pub imagegen_sha256: String,
    pub dotmend_art_id: String,
    pub source_palette_preserved: bool,
    pub audio_prefix_preserved: bool,
    pub runtime_verified: bool,
}

pub(super) fn apply(
    path: &Path,
    sources: &[ModeDescendantSourceRecord],
    drafts: &mut Vec<ModeDescendantRecordDraft>,
) -> Result<MainTitleReport> {
    let bytes = std::fs::read(path)?;
    let art: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(
        art.source_text == "私立ジャスティス学園 / 熱血青春日記 2"
            && art.korean_text == "사립 저스티스 학원 / 열혈청춘일기 2",
        "main title wording changed"
    );
    ensure!(
        art.imagegen_sha256.len() == 64
            && art.imagegen_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && art.dotmend_art_id.starts_with("art_"),
        "main title provenance missing"
    );
    let pixels = std::fs::read(&art.indices_file)?;
    ensure!(
        sha256_bytes(&pixels) == art.indices_sha256,
        "main title artwork changed"
    );
    let source = source_for_spec(sources, &SPEC)?;
    let cell = Cell {
        x: 20,
        y: 20,
        width: 476,
        height: 356,
    };
    let original = read_8bpp_indexed_cell_in_prefix(&source.decoded, 0x6000, cell)?;
    ensure!(
        sha256_bytes(&original) == art.source_indexed_sha256,
        "main title source region changed"
    );
    ensure!(
        sha256_bytes(&source.decoded[0x6014..0x6214]) == art.source_palette_sha256,
        "main title source palette changed"
    );
    let mut decoded = source.decoded.clone();
    write_8bpp_indexed_cell_in_prefix_with_report(&mut decoded, 0x6000, cell, &pixels)?;
    ensure!(
        read_8bpp_indexed_cell_in_prefix(&decoded, 0x6000, cell)? == pixels,
        "main title readback differs"
    );
    ensure!(
        decoded[..0x6214] == source.decoded[..0x6214],
        "main title audio or palette write forbidden"
    );
    let claims = DecodedDataClaim::from_ranges(
        "main-title-lettering",
        "Korean main title with original audio, palette, outer background and copyright",
        difference_ranges(&source.decoded, &decoded),
    );
    drafts.push(ModeDescendantRecordDraft {
        spec: &SPEC,
        decoded,
        decoded_write_claims: claims,
    });
    Ok(MainTitleReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: art.indices_sha256,
        imagegen_sha256: art.imagegen_sha256,
        dotmend_art_id: art.dotmend_art_id,
        source_palette_preserved: true,
        audio_prefix_preserved: true,
        runtime_verified: false,
    })
}
