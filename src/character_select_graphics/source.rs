use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{
        CHARACTER_SELECT_COOPERATIVE_MENU_RECORD, CHARACTER_SELECT_OVERLAY_RECORDS,
        CHARACTER_SELECT_TEXTURE_RECORDS,
    },
};

use super::compressed_record::{CompressedStreamStorage, CompressedTextureStream};
use super::texture_targets::{OP01_PATH, OVER_PATH, TITLE_PATH, TOROFY_PATH};

pub(super) struct CharacterSelectSourceRecord {
    pub(super) path: &'static str,
    pub(super) output_file: &'static str,
    pub(super) extent_lba: u32,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
    pub(super) overlay_path: &'static str,
    pub(super) overlay_output_file: &'static str,
    pub(super) overlay_extent_lba: u32,
    pub(super) overlay: Vec<u8>,
}

pub(super) struct CharacterSelectAuxiliarySourceRecord {
    pub(super) role: &'static str,
    pub(super) path: &'static str,
    pub(super) output_file: &'static str,
    pub(super) extent_lba: u32,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
    pub(super) compression_streams: Vec<CompressedTextureStream>,
}

struct SourceRecordSpec {
    path: &'static str,
    output_file: &'static str,
    overlay_path: &'static str,
    overlay_output_file: &'static str,
    overlay_sha256: &'static str,
    overlay_size: usize,
    stored_sha256: &'static str,
    decoded_sha256: &'static str,
    decoded_size: usize,
}

struct AuxiliarySourceRecordSpec {
    role: &'static str,
    path: &'static str,
    output_file: &'static str,
    stored_sha256: &'static str,
    decoded_streams: &'static [DecodedStreamSpec],
    compression_layout: AuxiliaryCompressionLayout,
}

#[derive(Clone, Copy)]
struct DecodedStreamSpec {
    stream_index: usize,
    decoded_sha256: &'static str,
    decoded_size: usize,
}

#[derive(Clone, Copy)]
enum AuxiliaryCompressionLayout {
    Direct,
    Indexed {
        header_size: usize,
        stream_count: usize,
    },
}

struct ConsumerEvidenceRecordSpec {
    path: &'static str,
    size: usize,
    sha256: &'static str,
}

const SOURCE_RECORDS: [SourceRecordSpec; 5] = [
    SourceRecordSpec {
        path: CHARACTER_SELECT_TEXTURE_RECORDS[0].path,
        output_file: "selp1.biz",
        overlay_path: CHARACTER_SELECT_OVERLAY_RECORDS[0].path,
        overlay_output_file: "plsel1.bin",
        overlay_sha256: CHARACTER_SELECT_OVERLAY_RECORDS[0].sha256,
        overlay_size: CHARACTER_SELECT_OVERLAY_RECORDS[0].size,
        stored_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[0].stored_sha256,
        decoded_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[0].decoded_sha256,
        decoded_size: CHARACTER_SELECT_TEXTURE_RECORDS[0].decoded_size,
    },
    SourceRecordSpec {
        path: CHARACTER_SELECT_TEXTURE_RECORDS[1].path,
        output_file: "selp2.biz",
        overlay_path: CHARACTER_SELECT_OVERLAY_RECORDS[1].path,
        overlay_output_file: "plsel2.bin",
        overlay_sha256: CHARACTER_SELECT_OVERLAY_RECORDS[1].sha256,
        overlay_size: CHARACTER_SELECT_OVERLAY_RECORDS[1].size,
        stored_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[1].stored_sha256,
        decoded_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[1].decoded_sha256,
        decoded_size: CHARACTER_SELECT_TEXTURE_RECORDS[1].decoded_size,
    },
    SourceRecordSpec {
        path: CHARACTER_SELECT_TEXTURE_RECORDS[2].path,
        output_file: "selp3.biz",
        overlay_path: CHARACTER_SELECT_OVERLAY_RECORDS[2].path,
        overlay_output_file: "plsel3.bin",
        overlay_sha256: CHARACTER_SELECT_OVERLAY_RECORDS[2].sha256,
        overlay_size: CHARACTER_SELECT_OVERLAY_RECORDS[2].size,
        stored_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[2].stored_sha256,
        decoded_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[2].decoded_sha256,
        decoded_size: CHARACTER_SELECT_TEXTURE_RECORDS[2].decoded_size,
    },
    SourceRecordSpec {
        path: CHARACTER_SELECT_TEXTURE_RECORDS[3].path,
        output_file: "selp4.biz",
        overlay_path: CHARACTER_SELECT_OVERLAY_RECORDS[3].path,
        overlay_output_file: "plsel4.bin",
        overlay_sha256: CHARACTER_SELECT_OVERLAY_RECORDS[3].sha256,
        overlay_size: CHARACTER_SELECT_OVERLAY_RECORDS[3].size,
        stored_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[3].stored_sha256,
        decoded_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[3].decoded_sha256,
        decoded_size: CHARACTER_SELECT_TEXTURE_RECORDS[3].decoded_size,
    },
    SourceRecordSpec {
        path: CHARACTER_SELECT_TEXTURE_RECORDS[4].path,
        output_file: "selp5.biz",
        overlay_path: CHARACTER_SELECT_OVERLAY_RECORDS[4].path,
        overlay_output_file: "plsel5.bin",
        overlay_sha256: CHARACTER_SELECT_OVERLAY_RECORDS[4].sha256,
        overlay_size: CHARACTER_SELECT_OVERLAY_RECORDS[4].size,
        stored_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[4].stored_sha256,
        decoded_sha256: CHARACTER_SELECT_TEXTURE_RECORDS[4].decoded_sha256,
        decoded_size: CHARACTER_SELECT_TEXTURE_RECORDS[4].decoded_size,
    },
];

const COOPERATIVE_MODE_MENU_STREAMS: [DecodedStreamSpec; 1] = [DecodedStreamSpec {
    stream_index: 0,
    decoded_sha256: CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.decoded_sha256,
    decoded_size: CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.decoded_size,
}];
const SOLO_STATE_PROMPT_STREAMS: [DecodedStreamSpec; 1] = [DecodedStreamSpec {
    stream_index: 0,
    decoded_sha256: "db1d1d0f98aeeb268b1c39537d222568b340aaea559f7103d6fec84fd516807f",
    decoded_size: 0x7bc0,
}];
const SOLO_STORY_INTRO_STREAMS: [DecodedStreamSpec; 1] = [DecodedStreamSpec {
    stream_index: 0,
    decoded_sha256: "d4221d245aaed24b96dbccab5de4a996343f79079ada1afcc5031e5cd460a380",
    decoded_size: 0x4d000,
}];
const SOLO_EPISODE_CARD_STREAMS: [DecodedStreamSpec; 8] = [
    DecodedStreamSpec {
        stream_index: 0,
        decoded_sha256: "62a3a75d91d66c16b3d3262fd2988b97f1c2fa197ff49e9b6b71cf3a51469cea",
        decoded_size: 0x4840,
    },
    DecodedStreamSpec {
        stream_index: 1,
        decoded_sha256: "a95978ddc5f2a0ef55932d5587064b61efc6feb6d9a3f55eb8c839b0dab39d20",
        decoded_size: 0x3840,
    },
    DecodedStreamSpec {
        stream_index: 2,
        decoded_sha256: "913559aae846ba62e0363601d98c57da1a53c08b3916191eb805390286d0e8b7",
        decoded_size: 0x4040,
    },
    DecodedStreamSpec {
        stream_index: 3,
        decoded_sha256: "2d2fff24a37d974b8fd5d354fcb12b99f41b647869370bc561307d3fa315a61d",
        decoded_size: 0x3840,
    },
    DecodedStreamSpec {
        stream_index: 4,
        decoded_sha256: "af37bcc9b02ed2f15b50e5b13a6f8070ac3b1bb15d89b2236c82b27a39c91683",
        decoded_size: 0x4040,
    },
    DecodedStreamSpec {
        stream_index: 5,
        decoded_sha256: "f413bce4bc1bd8162d5213af3f391aaa18c1ce74dd74fbdf11dac377dd3a40ce",
        decoded_size: 0x5840,
    },
    DecodedStreamSpec {
        stream_index: 6,
        decoded_sha256: "3fdd9cd055c881e55f7a16d0f9234c1a8d349e8ab694a424d9642d14fa988bce",
        decoded_size: 0x4040,
    },
    DecodedStreamSpec {
        stream_index: 7,
        decoded_sha256: "ab467aaba9a73ad2c29abc04d8de8c1b380b9f2173345e473ea71ff5b897e170",
        decoded_size: 0x3840,
    },
];
const TOURNAMENT_CERTIFICATE_STREAMS: [DecodedStreamSpec; 1] = [DecodedStreamSpec {
    stream_index: 0,
    decoded_sha256: "d546eba2420eb2424f509737643f51edac3e87addeb8a0447ce695a35634157b",
    decoded_size: 0x63000,
}];

const AUXILIARY_SOURCE_RECORDS: [AuxiliarySourceRecordSpec; 5] = [
    AuxiliarySourceRecordSpec {
        role: "cooperative_mode_menu",
        path: CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.path,
        output_file: "aisyou.tiz",
        stored_sha256: CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.stored_sha256,
        decoded_streams: &COOPERATIVE_MODE_MENU_STREAMS,
        compression_layout: AuxiliaryCompressionLayout::Direct,
    },
    AuxiliarySourceRecordSpec {
        role: "solo_state_prompts",
        path: OVER_PATH,
        output_file: "over.tiz",
        stored_sha256: "258db2adacd1c811f83aeb9f13177e93e764616ff26e799e89fbcefdf5b75d6d",
        decoded_streams: &SOLO_STATE_PROMPT_STREAMS,
        compression_layout: AuxiliaryCompressionLayout::Direct,
    },
    AuxiliarySourceRecordSpec {
        role: "solo_story_intro",
        path: OP01_PATH,
        output_file: "op01.biz",
        stored_sha256: "49304752afd0e46fbbdd64684150f3255f09baf79cda058f6c1c52119fc3e8ea",
        decoded_streams: &SOLO_STORY_INTRO_STREAMS,
        compression_layout: AuxiliaryCompressionLayout::Indexed {
            header_size: 0x800,
            stream_count: 3,
        },
    },
    AuxiliarySourceRecordSpec {
        role: "solo_episode_card",
        path: TITLE_PATH,
        output_file: "title.bin",
        stored_sha256: "b39d2b4c36f6bfec7e2c5402677dadd219a1380aaf33cb5d4306e430bb16538e",
        decoded_streams: &SOLO_EPISODE_CARD_STREAMS,
        compression_layout: AuxiliaryCompressionLayout::Indexed {
            header_size: 0x800,
            stream_count: 8,
        },
    },
    AuxiliarySourceRecordSpec {
        role: "tournament_certificate",
        path: TOROFY_PATH,
        output_file: "torofy.biz",
        stored_sha256: "9494f054c72901f311cc22f68cb2a9e1671b1a1de2e8c458286868bedcb21a40",
        decoded_streams: &TOURNAMENT_CERTIFICATE_STREAMS,
        compression_layout: AuxiliaryCompressionLayout::Direct,
    },
];

const CONSUMER_EVIDENCE_RECORDS: [ConsumerEvidenceRecordSpec; 5] = [
    ConsumerEvidenceRecordSpec {
        path: "DAT1/ODEMO.BIN",
        size: 3_220,
        sha256: "34fb958d3ba7932d9097578e1be654bef5556d1731f1611fb5e8a71e52b5ff7d",
    },
    ConsumerEvidenceRecordSpec {
        path: "DAT1/CDEMO.BIN",
        size: 4_312,
        sha256: "f8ab667ffa7c9ec6a548f82bff1ce9ec6abf97de53ca57bbf935a75a9452f96e",
    },
    ConsumerEvidenceRecordSpec {
        path: "DAT1/SIKEN.BIN",
        size: 30_612,
        sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
    },
    ConsumerEvidenceRecordSpec {
        path: "DAT1/SIKEN2.BIN",
        size: 30_784,
        sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
    },
    ConsumerEvidenceRecordSpec {
        path: super::pause_menu_consumer::CONSUMER_RECORD,
        size: 20_136,
        sha256: "c793a08264d3b41916efdcb96bc4aee71ac638c873e40c25a4c010476b6ee9c9",
    },
];

#[cfg(test)]
pub(super) fn load_character_select_source(
    cue_path: &std::path::Path,
) -> Result<(String, Vec<CharacterSelectSourceRecord>)> {
    let source = SupportedSourceDisc::open(cue_path)?;
    load_character_select_source_from_disc(&source)
}

pub(super) fn load_character_select_source_from_disc(
    source: &SupportedSourceDisc,
) -> Result<(String, Vec<CharacterSelectSourceRecord>)> {
    let mut records = Vec::with_capacity(SOURCE_RECORDS.len());
    for spec in SOURCE_RECORDS {
        let (record, stored) = source.read_record(spec.path)?;
        ensure!(
            sha256_bytes(&stored) == spec.stored_sha256,
            "{} stored source identity changed",
            spec.path
        );
        let decoded = decompress(&stored, false)?;
        let decoded_sha256 = sha256_bytes(&decoded);
        ensure!(
            decoded.len() == spec.decoded_size && decoded_sha256 == spec.decoded_sha256,
            "{} decoded source identity changed: expected {} bytes / {}, found {} bytes / {}",
            spec.path,
            spec.decoded_size,
            spec.decoded_sha256,
            decoded.len(),
            decoded_sha256
        );
        let (overlay_record, overlay) = source.read_record(spec.overlay_path)?;
        ensure!(
            overlay.len() == spec.overlay_size && sha256_bytes(&overlay) == spec.overlay_sha256,
            "{} source identity changed",
            spec.overlay_path
        );
        super::resource_loads::validate_consumer_resource_loads(spec.overlay_path, &overlay)?;
        records.push(CharacterSelectSourceRecord {
            path: spec.path,
            output_file: spec.output_file,
            extent_lba: record.extent_lba,
            stored,
            decoded,
            overlay_path: spec.overlay_path,
            overlay_output_file: spec.overlay_output_file,
            overlay_extent_lba: overlay_record.extent_lba,
            overlay,
        });
    }
    validate_additional_consumer_evidence(source)?;
    Ok((source.source_bin_sha256().to_string(), records))
}

fn validate_additional_consumer_evidence(source: &SupportedSourceDisc) -> Result<()> {
    for spec in CONSUMER_EVIDENCE_RECORDS {
        let (_, overlay) = source.read_record(spec.path)?;
        ensure!(
            overlay.len() == spec.size && sha256_bytes(&overlay) == spec.sha256,
            "{} consumer-evidence source identity changed",
            spec.path
        );
        super::resource_loads::validate_consumer_resource_loads(spec.path, &overlay)?;
        if spec.path == super::episode_card_consumer::CONSUMER_RECORD {
            super::episode_card_consumer::validate_solo_episode_card_consumer(&overlay)?;
        }
        if spec.path == super::pause_menu_consumer::CONSUMER_RECORD {
            super::pause_menu_consumer::validate_pause_menu_consumer(&overlay)?;
        }
    }
    Ok(())
}

pub(super) fn load_character_select_auxiliary_source_from_disc(
    source: &SupportedSourceDisc,
) -> Result<Vec<CharacterSelectAuxiliarySourceRecord>> {
    let mut records = Vec::with_capacity(AUXILIARY_SOURCE_RECORDS.len());
    for spec in AUXILIARY_SOURCE_RECORDS {
        let (record, stored) = source.read_record(spec.path)?;
        ensure!(
            sha256_bytes(&stored) == spec.stored_sha256,
            "{} stored source identity changed",
            spec.path
        );
        ensure!(
            !spec.decoded_streams.is_empty(),
            "{} has no declared decoded streams",
            spec.path
        );
        let mut decoded = Vec::new();
        let mut compression_streams = Vec::with_capacity(spec.decoded_streams.len());
        for decoded_spec in spec.decoded_streams {
            let (storage, allow_trailing_bytes) = match spec.compression_layout {
                AuxiliaryCompressionLayout::Direct => {
                    ensure!(
                        spec.decoded_streams.len() == 1 && decoded_spec.stream_index == 0,
                        "{} direct compressed record must declare one stream",
                        spec.path
                    );
                    (CompressedStreamStorage::direct(stored.len())?, true)
                }
                AuxiliaryCompressionLayout::Indexed {
                    header_size,
                    stream_count,
                } => (
                    CompressedStreamStorage::indexed(
                        &stored,
                        header_size,
                        stream_count,
                        decoded_spec.stream_index,
                    )?,
                    false,
                ),
            };
            let member = decompress(storage.source_encoded(&stored)?, allow_trailing_bytes)?;
            let member_sha256 = sha256_bytes(&member);
            ensure!(
                member.len() == decoded_spec.decoded_size
                    && member_sha256 == decoded_spec.decoded_sha256,
                "{} stream {} decoded source identity changed: expected {} bytes / {}, found {} bytes / {}",
                spec.path,
                decoded_spec.stream_index,
                decoded_spec.decoded_size,
                decoded_spec.decoded_sha256,
                member.len(),
                member_sha256
            );
            let decoded_start = decoded.len();
            decoded.extend_from_slice(&member);
            compression_streams.push(CompressedTextureStream {
                stream_index: decoded_spec.stream_index,
                decoded_range: [decoded_start, decoded.len()],
                storage,
            });
        }
        records.push(CharacterSelectAuxiliarySourceRecord {
            role: spec.role,
            path: spec.path,
            output_file: spec.output_file,
            extent_lba: record.extent_lba,
            stored,
            decoded,
            compression_streams,
        });
    }
    Ok(records)
}

pub(super) fn character_select_overlay_output_file(source_path: &str) -> Result<&'static str> {
    SOURCE_RECORDS
        .iter()
        .find(|spec| spec.overlay_path == source_path)
        .map(|spec| spec.overlay_output_file)
        .ok_or_else(|| anyhow::anyhow!("unsupported character-select overlay {source_path}"))
}
