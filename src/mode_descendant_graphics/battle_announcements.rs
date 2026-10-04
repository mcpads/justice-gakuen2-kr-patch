//! Shared battle lettering, composed with existing CEFT3 training-menu edits.
use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::ModeDescendantGraphicsBuildConfig;
use super::model::{ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::IndexedTextRasterizers;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[path = "battle_counter.rs"]
mod counter;

pub(super) const SPEC: ModeDescendantRecordSpec = ModeDescendantRecordSpec {
    record: ModeDescendantRecord::BattleEffects,
    surface: ModeDescendantSurface::BattleAnnouncements,
    source_path: "DAT2/CEFT1.BIZ",
    storage_kind: ModeDescendantStorageKind::PagedCompressed,
    output_file: "ceft1-battle-announcements.biz",
    texture_outputs: &[ModeDescendantTextureOutputSpec {
        tim_offset: 0x10800,
        preview_file: "battle-announcements.png",
    }],
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    #[serde(default)]
    kind: AnnouncementKind,
    #[serde(default)]
    roster_id: Option<u8>,
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
pub struct BattleArtworkReport {
    pub vertical_name_id: Option<u8>,
    pub round_numerals_repacked: bool,
    pub special_finish_trimmed: bool,
    pub source_text: String,
    pub korean_text: String,
    pub manifest_sha256: String,
    pub indices_sha256: String,
    pub imagegen_sha256: String,
    pub dotmend_art_id: String,
    pub source_records: Vec<String>,
    pub palette_and_unowned_bytes_preserved: bool,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnnouncementKind {
    #[default]
    Start,
    Finish,
    Timeout,
    Draw,
    VictoryWin,
    VictoryGain,
    VictoryOverwhelm,
    VictoryNarrow,
    VictoryControl,
    RoundPrefix,
    RoundMatch,
    RoundFinalFirst,
    RoundFinalLast,
    RoundNumerals,
    FullBurn,
    GutsCounter,
    TeamUp,
    SpecialFinish,
    ChangeProtagonist,
    ChangePartner,
    HudReady,
    HudMax,
    HudPerfect,
    VerticalName,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Family {
    artworks: Vec<PathBuf>,
    counter_labels: Vec<counter::Label>,
}
#[derive(Debug, Serialize)]
pub struct BattleAnnouncementReport {
    pub manifest_sha256: String,
    pub artworks: Vec<BattleArtworkReport>,
    pub counter_labels: Vec<counter::LabelReport>,
}
pub(super) fn apply(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    path: &Path,
    sources: &[ModeDescendantSourceRecord],
    drafts: &mut Vec<ModeDescendantRecordDraft>,
) -> Result<BattleAnnouncementReport> {
    let bytes = std::fs::read(path)?;
    let family: Family = serde_json::from_slice(&bytes)?;
    ensure!(
        !family.artworks.is_empty(),
        "battle artwork family is empty"
    );
    let source = source_for_spec(sources, &SPEC)?;
    drafts.push(ModeDescendantRecordDraft {
        spec: &SPEC,
        decoded: source.decoded.clone(),
        decoded_write_claims: Vec::new(),
    });
    let mut artworks = Vec::new();
    let mut paths = std::collections::BTreeSet::new();
    for entry in family.artworks {
        ensure!(
            paths.insert(entry.clone())
                && entry
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "invalid or duplicate battle artwork path"
        );
        artworks.push(apply_artwork(
            &path
                .parent()
                .context("battle family parent missing")?
                .join(entry),
            drafts,
        )?);
    }
    super::vertical_names::validate_selection(
        &artworks
            .iter()
            .filter_map(|a| a.vertical_name_id)
            .collect::<Vec<_>>(),
    )?;
    let counter_labels = counter::apply(config, rasterizers, &family.counter_labels, drafts)?;
    Ok(BattleAnnouncementReport {
        manifest_sha256: sha256_bytes(&bytes),
        artworks,
        counter_labels,
    })
}
fn apply_artwork(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<BattleArtworkReport> {
    let bytes = std::fs::read(path)?;
    let art: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(
        matches!(art.kind, AnnouncementKind::VerticalName) == art.roster_id.is_some(),
        "roster ID is required only for fixed HUD names"
    );
    let (id, source_text, korean_text, x, y, width, height, palette_offset) = match art.kind {
        AnnouncementKind::VerticalName => {
            super::vertical_names::cell(art.roster_id.context("HUD name ID missing")?)?
        }
        AnnouncementKind::Start => ("battle-start", "開始!", "시작!", 256, 128, 168, 56, 976),
        AnnouncementKind::Finish => ("battle-finish", "決着", "결판", 256, 184, 160, 64, 1024),
        AnnouncementKind::Timeout => (
            "battle-timeout",
            "時間切れ",
            "시간 종료",
            512,
            80,
            96,
            32,
            960,
        ),
        AnnouncementKind::Draw => ("battle-draw", "引き分け", "무승부", 608, 80, 96, 32, 960),
        AnnouncementKind::VictoryWin => ("victory-win", "勝", "승", 512, 112, 64, 40, 1008),
        AnnouncementKind::VictoryGain => ("victory-gain", "利", "리", 512, 32, 56, 40, 1008),
        AnnouncementKind::VictoryOverwhelm => {
            ("victory-overwhelm", "圧", "압", 568, 32, 56, 40, 1008)
        }
        AnnouncementKind::VictoryNarrow => ("victory-narrow", "辛", "신", 704, 96, 64, 48, 1008),
        AnnouncementKind::VictoryControl => ("victory-control", "制", "제", 624, 32, 56, 40, 1008),
        AnnouncementKind::RoundPrefix => ("round-prefix", "第", "제", 256, 64, 80, 64, 992),
        AnnouncementKind::RoundMatch => ("round-match", "戦", "전", 336, 64, 88, 64, 992),
        AnnouncementKind::RoundFinalFirst => ("round-final-first", "最", "최", 256, 0, 80, 64, 992),
        AnnouncementKind::RoundFinalLast => ("round-final-last", "終", "종", 336, 0, 88, 64, 992),
        AnnouncementKind::RoundNumerals => {
            ("round-numerals", "一・二", "1・2", 432, 0, 80, 72, 992)
        }
        AnnouncementKind::FullBurn => (
            "full-burn",
            "完全燃焼アタック",
            "완전연소 어택",
            512,
            160,
            128,
            32,
            1072,
        ),
        AnnouncementKind::GutsCounter => (
            "guts-counter",
            "根性カウンター",
            "근성 카운터",
            640,
            160,
            128,
            32,
            1072,
        ),
        AnnouncementKind::TeamUp => (
            "team-up",
            "愛と友情のツープラトン",
            "사랑과 우정의 투 플라톤",
            576,
            112,
            128,
            32,
            1072,
        ),
        AnnouncementKind::SpecialFinish => (
            "special-finish",
            "フィニッシュ",
            "피니시",
            576,
            144,
            88,
            16,
            1088,
        ),
        AnnouncementKind::ChangeProtagonist => (
            "change-protagonist",
            "主役交代?",
            "주역 교대?",
            512,
            192,
            80,
            12,
            784,
        ),
        AnnouncementKind::ChangePartner => (
            "change-partner",
            "相棒交代?",
            "파트너 교대?",
            592,
            192,
            80,
            12,
            784,
        ),
        AnnouncementKind::HudPerfect => ("hud-perfect", "完全", "완전", 160, 128, 24, 16, 224),
        AnnouncementKind::HudReady => ("hud-ready", "準備OK!!", "준비 OK!!", 160, 96, 80, 12, 32),
        AnnouncementKind::HudMax => ("hud-max", "MAX!", "최대!", 160, 112, 48, 16, 32),
    };
    ensure!(
        art.source_text == source_text && art.korean_text == korean_text,
        "battle artwork identity changed"
    );
    let pixels = std::fs::read(
        path.parent()
            .context("battle artwork parent missing")?
            .join(&art.indices_file),
    )?;
    ensure!(
        pixels.len() == width * height
            && pixels.iter().all(|p| *p < 16)
            && sha256_bytes(&pixels) == art.indices_sha256,
        "battle announcement indexed artwork changed"
    );
    let mut records = Vec::new();
    let mut consumers = vec![(ModeDescendantRecord::BattleEffects, 0x10800, x)];
    // CEFT3 carries only the 832 page. The outcome sprites live on page 896
    // in CEFT1 and have no corresponding cell in the training texture.
    if matches!(
        art.kind,
        AnnouncementKind::Start
            | AnnouncementKind::Finish
            | AnnouncementKind::RoundPrefix
            | AnnouncementKind::RoundMatch
            | AnnouncementKind::RoundFinalFirst
            | AnnouncementKind::RoundFinalLast
            | AnnouncementKind::RoundNumerals
    ) {
        consumers.push((ModeDescendantRecord::TrainingMenuTexture, 0x19000, x - 256));
    }
    for (record, tim_offset, x) in consumers {
        let draft = drafts
            .iter_mut()
            .find(|d| d.spec.record == record)
            .context("battle announcements require both CEFT texture consumers")?;
        let cell = Cell {
            x,
            y,
            width,
            height,
        };
        let original = read_indexed_cell_in_prefix(&draft.decoded, tim_offset, cell)?;
        ensure!(
            sha256_bytes(&original) == art.source_indexed_sha256,
            "battle announcement source cell changed or is already owned"
        );
        let pal = tim_offset + 20 + palette_offset * 2;
        ensure!(
            sha256_bytes(
                draft
                    .decoded
                    .get(pal..pal + 32)
                    .context("battle CLUT missing")?
            ) == art.source_palette_sha256,
            "battle announcement CLUT changed"
        );
        let before = draft.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut draft.decoded, tim_offset, cell, &pixels)?;
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_ranges(
                id,
                "complete Korean battle announcement sprite",
                difference_ranges(&before, &draft.decoded),
            ));
        records.push(draft.spec.source_path.to_string());
    }
    Ok(BattleArtworkReport {
        vertical_name_id: art.roster_id,
        round_numerals_repacked: matches!(art.kind, AnnouncementKind::RoundNumerals),
        special_finish_trimmed: matches!(art.kind, AnnouncementKind::SpecialFinish),
        source_text: art.source_text,
        korean_text: art.korean_text,
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: art.indices_sha256,
        imagegen_sha256: art.imagegen_sha256,
        dotmend_art_id: art.dotmend_art_id,
        source_records: records,
        palette_and_unowned_bytes_preserved: true,
    })
}
