use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::source_disc::profile::{
    EDIT_REGISTRATION_OVERLAY_RECORD, EDIT_REGISTRATION_UI_RECORD, GORIN_MAIN_MENU_RECORD,
    MAIN_EXECUTABLE_RECORD,
};
use crate::tzz::parse_tzz;

use super::catalog::{
    PRACTICAL_1999_DESCRIPTOR_PATH, PRACTICAL_BASICS_DESCRIPTOR_PATH, PRACTICAL_RESULT_TITLES_PATH,
    indexed_member_archive_contract,
};
use super::edit_command_sheets::EDIT_COMMAND_SHEETS_PATH;
use super::indexed_member_archive::decode_indexed_member_archive;
use super::model::ModeDescendantStorageKind;

pub(super) struct ModeDescendantSourceRecord {
    pub(super) role: &'static str,
    pub(super) path: &'static str,
    pub(super) extent_lba: u32,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
    pub(super) allows_trailing_bytes: bool,
    pub(super) storage_kind: ModeDescendantStorageKind,
}

pub(super) struct SourceRecordSpec {
    pub(super) role: &'static str,
    pub(super) path: &'static str,
    pub(super) stored_size: usize,
    pub(super) stored_sha256: Option<&'static str>,
    pub(super) decoded_size: usize,
    pub(super) decoded_sha256: &'static str,
}

impl SourceRecordSpec {
    fn allows_trailing_bytes(&self) -> bool {
        false
    }

    fn storage_kind(&self) -> ModeDescendantStorageKind {
        match self.path {
            "DAT2/TESTMJ.TIZ" => ModeDescendantStorageKind::TzzCompressedMembers,
            super::edit_technique_archive::PATH
            | EDIT_COMMAND_SHEETS_PATH
            | "DAT2/SIKEN20.BIZ"
            | PRACTICAL_RESULT_TITLES_PATH => ModeDescendantStorageKind::IndexedCompressedMembers,
            PRACTICAL_BASICS_DESCRIPTOR_PATH
            | PRACTICAL_1999_DESCRIPTOR_PATH
            | "DAT1/SIKENG21.BIN"
            | "DAT1/SIKENG22.BIN"
            | "DAT1/SIKENGO1.BIN"
            | "DAT1/TRAIN.BIN"
            | "DAT1/MGAME06.BIN"
            | "DAT1/MGAME07.BIN"
            | "DAT1/SIKENGO2.BIN" => ModeDescendantStorageKind::Raw,
            path if path == MAIN_EXECUTABLE_RECORD.path
                || path == EDIT_REGISTRATION_OVERLAY_RECORD.path =>
            {
                ModeDescendantStorageKind::Raw
            }
            _ => ModeDescendantStorageKind::PagedCompressed,
        }
    }
}

pub(super) const SOURCE_RECORDS: [SourceRecordSpec; 33] = [
    SourceRecordSpec {
        role: "main_title_texture",
        path: "DAT2/TITLE.BIZ",
        stored_size: 179992,
        stored_sha256: Some("2c6c7e5947c268252e386ce069ab45a1642be1d2b17ebdda741005bc3bb205a6"),
        decoded_size: 272384,
        decoded_sha256: "64ba794c6c9710236569a839871d7f6cc2c238897037ba3c4764d35e6619ea6a",
    },
    SourceRecordSpec {
        role: "vertical_announcement_texture",
        path: "DAT2/CEFT4.BIZ",
        stored_size: 78520,
        stored_sha256: Some("8eb1840c2080c9c4c01439d3491ae3d8a42f2876dabc33216dc0e21a729a55b0"),
        decoded_size: 143360,
        decoded_sha256: "d7e463f95486433bff3bde4c3f66448a14d81a83800510bd1e21948b744ec1ad",
    },
    SourceRecordSpec {
        role: "vertical_announcement_texture",
        path: "DAT2/CEFT6.BIZ",
        stored_size: 78496,
        stored_sha256: Some("59025859588d2a9cb667402a4001bc8aac48a7a02aa05ad28a3cc4ad49791a86"),
        decoded_size: 143360,
        decoded_sha256: "410d9c53b4660f0484021c8dc4a6a9d465b289674dcdd36ab3200145439c609e",
    },
    SourceRecordSpec {
        role: "gorin_gameplay_texture",
        path: "DAT2/DANCE.BIZ",
        stored_size: 51526,
        stored_sha256: Some("c9f7101db87397e5ced246c569dc188d2cde38ce568fa632603325b67d5b72e0"),
        decoded_size: 100384,
        decoded_sha256: "1b1df6508437e6e6155a766ca4e0dbdfb221324aabadef3694dc8ce2945df70e",
    },
    SourceRecordSpec {
        role: "gorin_gameplay_texture",
        path: "DAT2/SPRINT.BIZ",
        stored_size: 33658,
        stored_sha256: Some("ef71b8eca77eda653f782f3da5961661a9a5fc07740cf341db3f3da8d765f856"),
        decoded_size: 66336,
        decoded_sha256: "7b929500bb4d86036abb4a5ddcd91d26d77eb3712ec9713d65dbe399fd44d852",
    },
    SourceRecordSpec {
        role: "gorin_gameplay_texture",
        path: "DAT2/CEFT5.BIZ",
        stored_size: 90864,
        stored_sha256: Some("bd2c33fcdffb05ee673c71707310b93f80beae8eaaeda9c45d3cc1cb62778bb8"),
        decoded_size: 169984,
        decoded_sha256: "3c0824d7cb3a0815ef00df43f1be3c36b629edb3b1a37c26d703ac1847fe9a44",
    },
    SourceRecordSpec {
        role: "battle_effects_texture",
        path: "DAT2/CEFT1.BIZ",
        stored_size: 101200,
        stored_sha256: Some("e0bc4c61ff3568f7d0b1040927afe715690bdaf08218bc45103950f91ba4c006"),
        decoded_size: 172032,
        decoded_sha256: "4bd096db062708fcc68b944963348e03e0734e2206a915d0be9862be096a41e9",
    },
    SourceRecordSpec {
        role: "gorin_selector_consumer",
        path: "DAT1/MGAME06.BIN",
        stored_size: 32432,
        stored_sha256: Some("51c5ba5e5f5e035cc29cbae996f81523e7728ae4dec383e4de38adc09937a057"),
        decoded_size: 32432,
        decoded_sha256: "51c5ba5e5f5e035cc29cbae996f81523e7728ae4dec383e4de38adc09937a057",
    },
    SourceRecordSpec {
        role: "gorin_selector_consumer",
        path: "DAT1/MGAME07.BIN",
        stored_size: 29440,
        stored_sha256: Some("784f04f46170b8b70b6f1770f34afad06ffd52f9932af92b3fc3a76d159d66a9"),
        decoded_size: 29440,
        decoded_sha256: "784f04f46170b8b70b6f1770f34afad06ffd52f9932af92b3fc3a76d159d66a9",
    },
    SourceRecordSpec {
        role: "continue_school_texture",
        path: "DAT2/MA_TIT.BIZ",
        stored_size: 113696,
        stored_sha256: Some("136badbf4d61ee1ddf576e492b3826a4b9486ebd6fa6728af94f9481081c270b"),
        decoded_size: 169984,
        decoded_sha256: "395c7b5ac3ac198c8cb7519f3860fefa145fa4177dd00be8a10a8907a8728fc8",
    },
    SourceRecordSpec {
        role: "training_menu_texture",
        path: "DAT2/CEFT3.BIZ",
        stored_size: 95324,
        stored_sha256: Some("491f80ee03a046d25b3d0ec0e263a41800ab84a2351be76107826cb9f8edcffb"),
        decoded_size: 167936,
        decoded_sha256: "eb34ce8d2e985c1620ef335eaca3a21ba7e42df15b0c8482d36a177cfc36d735",
    },
    SourceRecordSpec {
        role: "training_menu_overlay",
        path: "DAT1/TRAIN.BIN",
        stored_size: 7825,
        stored_sha256: Some("d3eb3b0066218ceabc3af609f2dcc0565f0ca7c46c7571614afcf48ab34286bf"),
        decoded_size: 7825,
        decoded_sha256: "d3eb3b0066218ceabc3af609f2dcc0565f0ca7c46c7571614afcf48ab34286bf",
    },
    SourceRecordSpec {
        role: "edit_shared_ui",
        path: EDIT_REGISTRATION_UI_RECORD.path,
        stored_size: 98_876,
        stored_sha256: Some(EDIT_REGISTRATION_UI_RECORD.stored_sha256),
        decoded_size: EDIT_REGISTRATION_UI_RECORD.decoded_size,
        decoded_sha256: EDIT_REGISTRATION_UI_RECORD.decoded_sha256,
    },
    SourceRecordSpec {
        role: "edit_technique_names",
        path: super::edit_technique_archive::PATH,
        stored_size: 0x8e800,
        stored_sha256: Some("e5294109b6438290f41aa3ddf0e2aff0fb522b77072cb8e8a21c06c867a108e6"),
        decoded_size: 32 * 0x8014,
        decoded_sha256: "4a94aee1919a1a16eaa233074c2f35cbc8c2abc065e2f1b03a3d24e47ea8634b",
    },
    SourceRecordSpec {
        role: "edit_command_sheets",
        path: EDIT_COMMAND_SHEETS_PATH,
        stored_size: 0x2e4000,
        stored_sha256: Some("f6d61fdf4d7b4a57473d95def9855ab8da2bc28612458d54271d84df3e55cb28"),
        decoded_size: 0x570000,
        decoded_sha256: "9940e4bd2231208bc5ab0b1aaa3c40f8ee569c8c460a845ccbc58b72dcb457c2",
    },
    SourceRecordSpec {
        role: "gorin_main_menu",
        path: GORIN_MAIN_MENU_RECORD.path,
        stored_size: 242_724,
        stored_sha256: Some(GORIN_MAIN_MENU_RECORD.stored_sha256),
        decoded_size: GORIN_MAIN_MENU_RECORD.decoded_size,
        decoded_sha256: GORIN_MAIN_MENU_RECORD.decoded_sha256,
    },
    SourceRecordSpec {
        role: "gorin_title_1",
        path: "DAT2/MINITTL1.BIZ",
        stored_size: 344_042,
        stored_sha256: Some("7beb056167edfe7663a4cc1487e69bf08257af2ad901dd4502ed9df6609daf8c"),
        decoded_size: 595_968,
        decoded_sha256: "4f074576e3ceb6b4d9bc30e1745f771491f2286daeeedc210e557a88f66a34ff",
    },
    SourceRecordSpec {
        role: "gorin_title_2",
        path: "DAT2/MINITTL2.BIZ",
        stored_size: 448_504,
        stored_sha256: Some("0f3219607f83d51afd7d88f0e5f09d3454041a7a91935b1416c48dcc815fe26b"),
        decoded_size: 843_776,
        decoded_sha256: "2fe902df54ded8c66d8e408df145e26b2a29d5003ec715c3aab3ab9d35f7d0bb",
    },
    SourceRecordSpec {
        role: "gorin_title_3",
        path: "DAT2/MINITTL3.BIZ",
        stored_size: 414_470,
        stored_sha256: Some("dbb5a882784f1863ca330ec3cc5adf550e6454ccc325d4ec85d79f6b04d44f3d"),
        decoded_size: 595_968,
        decoded_sha256: "39d0ce53ef486497ec75248e4704956f1f497b4982278c778de818c244746ec6",
    },
    SourceRecordSpec {
        role: "gorin_title_4",
        path: "DAT2/MINITTL4.BIZ",
        stored_size: 357_654,
        stored_sha256: Some("57afad8633f57840a70666ab80208e71be0b8484d53b02a6ef215b06b2f385d8"),
        decoded_size: 595_968,
        decoded_sha256: "58a10ec35ceffd473df3091e6410c1d405a90003520da0abba5057373fb767ec",
    },
    SourceRecordSpec {
        role: "gorin_title_5",
        path: "DAT2/MINITTL5.BIZ",
        stored_size: 354_424,
        stored_sha256: Some("c46f37698e585c364ee5c62bbdab5365ff8c649050419676b12afd151cfc6151"),
        decoded_size: 595_968,
        decoded_sha256: "5bf758eaa2435e380b937c3f5f8db9e129a5330093528c549f4eb71d6745be78",
    },
    SourceRecordSpec {
        role: "gorin_title_6",
        path: "DAT2/MINITTL6.BIZ",
        stored_size: 490_272,
        stored_sha256: Some("966d4c85b57a7a75eaa988899d0d6784f6df191c1788fd8b666455d8b80aa650"),
        decoded_size: 843_776,
        decoded_sha256: "72b361163ae7c95d47442e096fb0591d20043032cecf54f297cd92209d34af02",
    },
    SourceRecordSpec {
        role: "gorin_title_7",
        path: "DAT2/MINITTL7.BIZ",
        stored_size: 395_794,
        stored_sha256: Some("4e72cd01446eca2fca1b8ff00b1452202bd8c04648e1290adfa9e7f44c5aa0bc"),
        decoded_size: 595_968,
        decoded_sha256: "fc6d72272769e00e527f32f069afb4b3c906e052088d5c2b967246fe82830b4d",
    },
    SourceRecordSpec {
        role: "gorin_title_8",
        path: "DAT2/MINITTL8.BIZ",
        stored_size: 340_954,
        stored_sha256: Some("fddd3d70b9e53e3cdf392e9e6c81c15ca39ee992f5066b1627787714e166c5c8"),
        decoded_size: 595_968,
        decoded_sha256: "579213af6af6daee5b3931f0282934b0cbc1fdb8188c1793c025a79e060e21e9",
    },
    SourceRecordSpec {
        role: "gorin_title_9",
        path: "DAT2/MINITTL9.BIZ",
        stored_size: 368_168,
        stored_sha256: Some("dbd3314619c9acc54b0e07a1ced2e762e3b71594e2fc42e9e92cb78fb1da8790"),
        decoded_size: 632_832,
        decoded_sha256: "f828687dd1fc29c6a772644dee117046ac10e272f98f228246a3984270aeedf1",
    },
    SourceRecordSpec {
        role: "gorin_title_a",
        path: "DAT2/MINITTLA.BIZ",
        stored_size: 408_196,
        stored_sha256: Some("7f5d42b66071ccff489a4cb27f51031bf88a35a74c04e7ac69add56a0239dab7"),
        decoded_size: 632_832,
        decoded_sha256: "0a80fa735cfd99e3e48236d27ad0c5419f877c4f2cf4d8922cb3feb15eeda967",
    },
    SourceRecordSpec {
        role: "gorin_title_b",
        path: "DAT2/MINITTLB.BIZ",
        stored_size: 393_230,
        stored_sha256: Some("aeaa81b7744481d0929556cc2b400b8a6079e50472c8f14a73a124878ce8575f"),
        decoded_size: 632_832,
        decoded_sha256: "5d6329d54aff4ce079e3ad1b21e1f408b729b2f15567b739f892f028fc0d8587",
    },
    SourceRecordSpec {
        role: "gorin_title_c",
        path: "DAT2/MINITTLC.BIZ",
        stored_size: 408_212,
        stored_sha256: Some("05a0948cbbb72084abc55449970e7f433e643d3ce1f884dec812c50986f4d35d"),
        decoded_size: 632_832,
        decoded_sha256: "da35d4305c884b52e124b6a010590d106c47382f7d0aa6673fee6547ed6f8ccf",
    },
    SourceRecordSpec {
        role: "practical_siken1",
        path: "DAT2/SIKEN1.BIZ",
        stored_size: 397_730,
        stored_sha256: Some("d1a7cb64c723ccc35d03671ae6fbf36a928b253403e9f97adf21a0092360d5da"),
        decoded_size: 595_968,
        decoded_sha256: "482b8a540aa7614d1fab1f3ab78a499a615ed3b963649f7d4008e3310b728760",
    },
    SourceRecordSpec {
        role: "practical_siken10",
        path: "DAT2/SIKEN10.BIZ",
        stored_size: 368_620,
        stored_sha256: Some("b02cb79495588d944e090a88fb4f6a7be668bc317ce85899178056eab28fa54b"),
        decoded_size: 595_968,
        decoded_sha256: "82ce41741821853a353537b073d0f3805de5daf4792dda320ea7484225f5af72",
    },
    SourceRecordSpec {
        role: "practical_siken2",
        path: "DAT2/SIKEN2.BIZ",
        stored_size: 254_774,
        stored_sha256: Some("358e087e9e5343e5e5093426135e008f050d8b05ffee361b6ffd200f664221bf"),
        decoded_size: 446_464,
        decoded_sha256: "296a5cdf8a212307206de02f32337cf44c24bbbf59a7fa7b65a44d55034d1be4",
    },
    SourceRecordSpec {
        role: "practical_siken20",
        path: "DAT2/SIKEN20.BIZ",
        stored_size: 489_472,
        stored_sha256: Some("0287be425dbff787bbbb811d176de375262f7edbd62178736aa276a4371dc8e1"),
        decoded_size: 892_928,
        decoded_sha256: "cd885b1fca66dd01dcb91713f3f3404c13d6a09a139ee1128319321c701428cb",
    },
    SourceRecordSpec {
        role: "practical_sikenkk",
        path: "DAT2/SIKENKK.BIZ",
        stored_size: 69_632,
        stored_sha256: Some("e5491079a27fcdd3c45c32a7088ebd7d5188334b3ac16bf8ca440b82cfdbf654"),
        decoded_size: 59_400,
        decoded_sha256: "8ce5adb4aab8ec85aa26835c1e3a49b772f9b37f9e1b386af6ae9090a8afad63",
    },
];

const AUXILIARY_EVIDENCE_SOURCE_RECORDS: [SourceRecordSpec; 10] = [
    SourceRecordSpec {
        role: "continue_school_consumer",
        path: "DAT1/MGTIT.BIZ",
        stored_size: crate::source_disc::profile::TITLE_MENU_OVERLAY_STORED_SIZE,
        stored_sha256: Some(crate::source_disc::profile::TITLE_MENU_OVERLAY_RECORD.stored_sha256),
        decoded_size: crate::source_disc::profile::TITLE_MENU_OVERLAY_RECORD.decoded_size,
        decoded_sha256: crate::source_disc::profile::TITLE_MENU_OVERLAY_RECORD.decoded_sha256,
    },
    SourceRecordSpec {
        role: "edit_command_sheet_consumer",
        path: EDIT_REGISTRATION_OVERLAY_RECORD.path,
        stored_size: EDIT_REGISTRATION_OVERLAY_RECORD.size,
        stored_sha256: Some(EDIT_REGISTRATION_OVERLAY_RECORD.sha256),
        decoded_size: EDIT_REGISTRATION_OVERLAY_RECORD.size,
        decoded_sha256: EDIT_REGISTRATION_OVERLAY_RECORD.sha256,
    },
    SourceRecordSpec {
        role: "practical_basics_descriptor_consumer",
        path: PRACTICAL_BASICS_DESCRIPTOR_PATH,
        stored_size: 30_612,
        stored_sha256: Some("18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9"),
        decoded_size: 30_612,
        decoded_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
    },
    SourceRecordSpec {
        role: "practical_1999_descriptor_consumer",
        path: PRACTICAL_1999_DESCRIPTOR_PATH,
        stored_size: 30_784,
        stored_sha256: Some("0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10"),
        decoded_size: 30_784,
        decoded_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
    },
    SourceRecordSpec {
        role: "practical_basics_result_consumer_first",
        path: "DAT1/SIKENG21.BIN",
        stored_size: 20_136,
        stored_sha256: Some("aa9e1e89b934b6381516671297d8ba30f96e115e899236e719d1166741c222e8"),
        decoded_size: 20_136,
        decoded_sha256: "aa9e1e89b934b6381516671297d8ba30f96e115e899236e719d1166741c222e8",
    },
    SourceRecordSpec {
        role: "practical_basics_result_consumer_second",
        path: "DAT1/SIKENG22.BIN",
        stored_size: 20_136,
        stored_sha256: Some("c793a08264d3b41916efdcb96bc4aee71ac638c873e40c25a4c010476b6ee9c9"),
        decoded_size: 20_136,
        decoded_sha256: "c793a08264d3b41916efdcb96bc4aee71ac638c873e40c25a4c010476b6ee9c9",
    },
    SourceRecordSpec {
        role: "practical_exam_result_consumer_first",
        path: "DAT1/SIKENGO1.BIN",
        stored_size: 22_840,
        stored_sha256: Some("105b5b89ea29827c8e010d454919cfdc11715f40c3fc50688873ed40e84b0d03"),
        decoded_size: 22_840,
        decoded_sha256: "105b5b89ea29827c8e010d454919cfdc11715f40c3fc50688873ed40e84b0d03",
    },
    SourceRecordSpec {
        role: "practical_exam_result_consumer_second",
        path: "DAT1/SIKENGO2.BIN",
        stored_size: 22_840,
        stored_sha256: Some("931238dad4a20764eef58f376265e900b4bb1bb6d50faf2028c563884886cba6"),
        decoded_size: 22_840,
        decoded_sha256: "931238dad4a20764eef58f376265e900b4bb1bb6d50faf2028c563884886cba6",
    },
    SourceRecordSpec {
        role: "practical_result_residency_catalog_source",
        path: MAIN_EXECUTABLE_RECORD.path,
        stored_size: MAIN_EXECUTABLE_RECORD.size,
        stored_sha256: Some(MAIN_EXECUTABLE_RECORD.sha256),
        decoded_size: MAIN_EXECUTABLE_RECORD.size,
        decoded_sha256: MAIN_EXECUTABLE_RECORD.sha256,
    },
    SourceRecordSpec {
        role: "practical_result_external_clut_producer_family",
        path: "DAT2/TESTMJ.TIZ",
        stored_size: 679_936,
        stored_sha256: Some("f8ac06959288769b3ae21b570c10a17188d7922a7430859c6e9ef9a330dc91aa"),
        decoded_size: 1_083_456,
        decoded_sha256: "0666f1d0b5b4316efada910afa16ffc832170107faab2efae4b4216561d96d38",
    },
];

pub(super) fn load_mode_descendant_audit_sources(
    cue_path: &std::path::Path,
) -> Result<(String, Vec<ModeDescendantSourceRecord>)> {
    let source = SupportedSourceDisc::open(cue_path)?;
    let mut specs = SOURCE_RECORDS.iter().collect::<Vec<_>>();
    for evidence in &AUXILIARY_EVIDENCE_SOURCE_RECORDS {
        if !specs.iter().any(|spec| spec.path == evidence.path) {
            specs.push(evidence);
        }
    }
    load_sources(&source, specs)
}

pub(super) fn is_graphics_inventory_source(path: &str) -> bool {
    SOURCE_RECORDS.iter().any(|spec| spec.path == path)
}

pub(super) fn load_mode_descendant_build_sources_from_disc(
    source: &SupportedSourceDisc,
    source_paths: &[&str],
) -> Result<(String, Vec<ModeDescendantSourceRecord>)> {
    let mut specs = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let spec = SOURCE_RECORDS
            .iter()
            .chain(AUXILIARY_EVIDENCE_SOURCE_RECORDS.iter())
            .find(|spec| spec.path == *path)
            .with_context(|| format!("mode-descendant build source {path} is not audited"))?;
        ensure!(
            !specs
                .iter()
                .any(|existing: &&SourceRecordSpec| existing.path == spec.path),
            "duplicate mode-descendant build source {path}"
        );
        specs.push(spec);
    }
    load_sources(source, specs)
}

fn load_sources<'a>(
    source: &SupportedSourceDisc,
    specs: impl IntoIterator<Item = &'a SourceRecordSpec>,
) -> Result<(String, Vec<ModeDescendantSourceRecord>)> {
    let specs = specs.into_iter().collect::<Vec<_>>();
    let mut records = Vec::with_capacity(specs.len());
    for spec in &specs {
        let (record, stored) = source.read_record(spec.path)?;
        ensure!(
            stored.len() == spec.stored_size,
            "{} stored size changed",
            spec.path
        );
        if let Some(expected) = spec.stored_sha256 {
            ensure!(
                sha256_bytes(&stored) == expected,
                "{} stored source identity changed",
                spec.path
            );
        }
        let allows_trailing_bytes = spec.allows_trailing_bytes();
        let storage_kind = spec.storage_kind();
        let decoded = match storage_kind {
            ModeDescendantStorageKind::IndexedCompressedMembers => {
                let contract = indexed_member_archive_contract(spec.path).with_context(|| {
                    format!(
                        "indexed-member mode-descendant source {} has no archive contract",
                        spec.path
                    )
                })?;
                decode_indexed_member_archive(&stored, contract)?
            }
            ModeDescendantStorageKind::PagedCompressed => {
                decompress(&stored, allows_trailing_bytes)
                    .with_context(|| format!("failed to decompress {}", spec.path))?
            }
            ModeDescendantStorageKind::Raw => stored.clone(),
            ModeDescendantStorageKind::TzzCompressedMembers => {
                let members =
                    parse_tzz(&stored).with_context(|| format!("failed to parse {}", spec.path))?;
                let mut decoded = Vec::with_capacity(spec.decoded_size);
                for member in members {
                    decoded.extend(
                        decompress(&stored[member.compressed_range()], true).with_context(
                            || {
                                format!(
                                    "failed to decompress {} member {}",
                                    spec.path, member.index
                                )
                            },
                        )?,
                    );
                }
                decoded
            }
        };
        ensure!(
            decoded.len() == spec.decoded_size && sha256_bytes(&decoded) == spec.decoded_sha256,
            "{} decoded source identity changed",
            spec.path
        );
        records.push(ModeDescendantSourceRecord {
            role: spec.role,
            path: spec.path,
            extent_lba: record.extent_lba,
            stored,
            decoded,
            allows_trailing_bytes,
            storage_kind,
        });
    }
    Ok((source.source_bin_sha256().to_string(), records))
}
