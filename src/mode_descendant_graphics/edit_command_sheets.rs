//! Source-bound archive and audit contract for the 32 EDIT command sheets.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::embedded_tim::EmbeddedTimAudit;
use crate::pipeline::sha256_bytes;
use crate::source_disc::profile::EDIT_REGISTRATION_OVERLAY_RECORD;

use super::indexed_member_archive::{
    IndexedMemberArchiveContract, IndexedMemberContract, rebuild_indexed_member_archive,
};
use super::model::{
    EditCommandSheetConsumerAudit, EditCommandSheetFamilyAudit, EditCommandSheetLoadSiteAudit,
    EditCommandSheetMemberAudit,
};
use super::source::ModeDescendantSourceRecord;

pub(super) const EDIT_COMMAND_SHEETS_PATH: &str = "DAT2/EDITCM.BIZ";

const EDIT_COMMAND_SHEET_MEMBERS: [IndexedMemberContract; 32] = [
    IndexedMemberContract {
        id: "member_00",
        index: 0,
        table_pair_offset: 0x000,
        stored_offset: 0x000800,
        stored_size: 0x16a98,
        stored_sha256: "c8c28929aa0771ce7a0eac05fd7ca4f4a5a5ec511d9d99e73116864b54800e04",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "d2199d9edad7cc65ecb0b8df3aa68ba437b37e9401476083de61a21ea574f38a",
    },
    IndexedMemberContract {
        id: "member_01",
        index: 1,
        table_pair_offset: 0x008,
        stored_offset: 0x017800,
        stored_size: 0x172e6,
        stored_sha256: "87776e63cebf313f8f9d4fb372a9d551f4d0adca9a7593cd5018ba70e0bbaa8a",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "5676f73e4dfed3e75d51a3b8040895cf342f5d58f762430f29dc47dbfa0af760",
    },
    IndexedMemberContract {
        id: "member_02",
        index: 2,
        table_pair_offset: 0x010,
        stored_offset: 0x02f000,
        stored_size: 0x16ad6,
        stored_sha256: "47fc48e1f011917b6a75461732261ab506eef13a07d7ac2be6617a87625d71d7",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "cc247ab92f7524cfe73b1648e4a0f868301965cea504bbb470b529af010e315e",
    },
    IndexedMemberContract {
        id: "member_03",
        index: 3,
        table_pair_offset: 0x018,
        stored_offset: 0x046000,
        stored_size: 0x16ade,
        stored_sha256: "396a4dfbd1d91da6f80b12ec6bcb4c3ea7f421f305fb029a65cd49beacf265b7",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "9217bba8e06a8d0765a9a88d40098db5176728b77d4bec30d80705006701d865",
    },
    IndexedMemberContract {
        id: "member_04",
        index: 4,
        table_pair_offset: 0x020,
        stored_offset: 0x05d000,
        stored_size: 0x16ac8,
        stored_sha256: "393a6189ce306f44b0370632075792bb27c25470a4d540297826fac6f672823b",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "8cc3764f602b337dea15cd7191399f88c625520808f3765f9488a93c40beb588",
    },
    IndexedMemberContract {
        id: "member_05",
        index: 5,
        table_pair_offset: 0x028,
        stored_offset: 0x074000,
        stored_size: 0x16a88,
        stored_sha256: "0caebe004f70733fd5c5aa8679af7170db7ac97e050f2bfb88d2f77c0457f746",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "9628fa323bf9e61d0690906e97a604bec20826aad077acb7071491b84521f8a2",
    },
    IndexedMemberContract {
        id: "member_06",
        index: 6,
        table_pair_offset: 0x030,
        stored_offset: 0x08b000,
        stored_size: 0x16a80,
        stored_sha256: "49619085ece162007e2ecc59ad1cf9f35f2ac2cb731494165068226a52af76b5",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "94a494e321078870ef81a7421d48f99367e7d8f18d124870c460dfd9ecf918fb",
    },
    IndexedMemberContract {
        id: "member_07",
        index: 7,
        table_pair_offset: 0x038,
        stored_offset: 0x0a2000,
        stored_size: 0x16ad4,
        stored_sha256: "a9a6cd11f829a3782fd9d48cbe15ec3c7dd1ff3b5da11d42f22ae551cc20b710",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "52927035dc45e9625ea6d212f8bbf4e90f811b69c56c96bf2d970992b536684a",
    },
    IndexedMemberContract {
        id: "member_08",
        index: 8,
        table_pair_offset: 0x040,
        stored_offset: 0x0b9000,
        stored_size: 0x16ab0,
        stored_sha256: "a0c6f9b9bd93b4722bbe269665065c700cb4663f3bf68bc7c2ff8b3039cd064a",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "338ee28333befced70914ffc168fc915242cf942515a6c5357cf85cf7f094970",
    },
    IndexedMemberContract {
        id: "member_09",
        index: 9,
        table_pair_offset: 0x048,
        stored_offset: 0x0d0000,
        stored_size: 0x16ab6,
        stored_sha256: "31ef9bd908a760dc2cfaa584dc1b37158e5d3996b33f879adb4973cebdb04a97",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "f165105bec25379ced2fb05142e627c64a2bcf07c91bdc2bf4c1264cad8fd16b",
    },
    IndexedMemberContract {
        id: "member_10",
        index: 10,
        table_pair_offset: 0x050,
        stored_offset: 0x0e7000,
        stored_size: 0x16aac,
        stored_sha256: "1faeaca872bfebeb91c45e45e16a511b1634b6facf1885fa916884421a931802",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "3ea71b66dc9330a8ee5746567c4500ffaba218cc99fe2f4f6e07ef0a28271750",
    },
    IndexedMemberContract {
        id: "member_11",
        index: 11,
        table_pair_offset: 0x058,
        stored_offset: 0x0fe000,
        stored_size: 0x16ad6,
        stored_sha256: "3a6af5cacd0a17c129b14b4aadc7b226e6ab1b29c8e9f87867325e0ea8fb9567",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "3ef4fec3e07b20fdd4324d9461c7986c7ad9642d5d03a5dc8465142af026464e",
    },
    IndexedMemberContract {
        id: "member_12",
        index: 12,
        table_pair_offset: 0x060,
        stored_offset: 0x115000,
        stored_size: 0x16aac,
        stored_sha256: "b314cf77181ea271bffd66f3067ef3591555802ee3de74f65d9e000a2366b1ed",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "96b360fce612e4351068714bff170362a842459e1c6336ccc0db29e18e435479",
    },
    IndexedMemberContract {
        id: "member_13",
        index: 13,
        table_pair_offset: 0x068,
        stored_offset: 0x12c000,
        stored_size: 0x16ab0,
        stored_sha256: "3964daf6b2cdf4c00ab7913fb3d27ccf2977f87e02fa8b12a71edbbe3d8d00b1",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "3f952cd37b9024adcc342f3c1ed5390a195525b15af03f07e6b99409abe01bbd",
    },
    IndexedMemberContract {
        id: "member_14",
        index: 14,
        table_pair_offset: 0x070,
        stored_offset: 0x143000,
        stored_size: 0x16aaa,
        stored_sha256: "4ac42c0aebe11710c5d701b9b4aff2a26fabf9c68a76c83c8cb92beb23ccad6c",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "49132c6c8d2b931d5b5a76cce051cf06c1610fb239954241c2b5954cbf5cc4f9",
    },
    IndexedMemberContract {
        id: "member_15",
        index: 15,
        table_pair_offset: 0x078,
        stored_offset: 0x15a000,
        stored_size: 0x16a9e,
        stored_sha256: "10e32cf31430ff46f912afa343384a19a389898c65b567d641aa3a09dc82937d",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "7bed677fd9dc5af3fd0400e648c6c3acad248a96e10baf777ad778ca4b4ebe39",
    },
    IndexedMemberContract {
        id: "member_16",
        index: 16,
        table_pair_offset: 0x080,
        stored_offset: 0x171000,
        stored_size: 0x16ad4,
        stored_sha256: "68e11a7a757e916bd41fe92e3a7f07841411aa69286459ca28e050c41ab58c59",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "5eddda2f62658520a30150a4aae2fb44dbf5441add03c55d784488ade84d3150",
    },
    IndexedMemberContract {
        id: "member_17",
        index: 17,
        table_pair_offset: 0x088,
        stored_offset: 0x188000,
        stored_size: 0x16ad6,
        stored_sha256: "886a5932ebece6f7c0dafe938d71dc5098e1b352f771e3653c3a64388941ac0d",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "b6c18465673bc18557d9a39d0262b67b544080bbf353ec625e7c83beb2e4dfe1",
    },
    IndexedMemberContract {
        id: "member_18",
        index: 18,
        table_pair_offset: 0x090,
        stored_offset: 0x19f000,
        stored_size: 0x16ad6,
        stored_sha256: "6de3fe631a6e29a54945b75689b72c3c1c98861606dadb4f2198209ab28bac30",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "d2c3d66f12be8713fd7ea5e38bdde8489e66dcd953256aca1c18ab245899e79d",
    },
    IndexedMemberContract {
        id: "member_19",
        index: 19,
        table_pair_offset: 0x098,
        stored_offset: 0x1b6000,
        stored_size: 0x172ec,
        stored_sha256: "acfc19fca65d29eb26fcf38beb53bd8dd7e48efe1015f1316ef3c87f2456bf49",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "ce64f52331de5842ab8e4accc78a778bc2088d04de79ab1af77dad24d57ae17c",
    },
    IndexedMemberContract {
        id: "member_20",
        index: 20,
        table_pair_offset: 0x0a0,
        stored_offset: 0x1cd800,
        stored_size: 0x16ad8,
        stored_sha256: "af690ad5f2b520e0ac0d42b0a33ea93730785e1565a13803ed9770955b200fae",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "dedc5c54b1a194ebc8d1d02d3396ebe04ff2468114e27fd79b2fd6ab380d4746",
    },
    IndexedMemberContract {
        id: "member_21",
        index: 21,
        table_pair_offset: 0x0a8,
        stored_offset: 0x1e4800,
        stored_size: 0x17320,
        stored_sha256: "6be6f3c41d32bb06c6b85f18ee6af9122e7c65d2a6cdbad7288f9ee3b4d089cb",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "d7b0cf286fff2e224a7db98b22852ddbec7dcfd9b921c999181e2b4512013019",
    },
    IndexedMemberContract {
        id: "member_22",
        index: 22,
        table_pair_offset: 0x0b0,
        stored_offset: 0x1fc000,
        stored_size: 0x16ab0,
        stored_sha256: "4db47a74c5763b43a30346ce0464dfa7c2eb9b8224da967a8065022f6e7a248d",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "ee090ef2be27d301c28259ef11d37e6f96c0ddc6987e511349af4d785862ef84",
    },
    IndexedMemberContract {
        id: "member_23",
        index: 23,
        table_pair_offset: 0x0b8,
        stored_offset: 0x213000,
        stored_size: 0x17304,
        stored_sha256: "d0f9bfb027a8e35538cc93b760d0330b66e9458e619d98e7cb330423456ced37",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "53c35ca580c8f0737645b42a7c9be2478e8d81993cde62a72a3d90cd7a98b3a6",
    },
    IndexedMemberContract {
        id: "member_24",
        index: 24,
        table_pair_offset: 0x0c0,
        stored_offset: 0x22a800,
        stored_size: 0x17310,
        stored_sha256: "ca4d3f9d6d6e0918da93075aa1f11739e29eb96da57f1ac9e4c42625453badff",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "4f9bdb71150b746cf4cdd73395efe2ccdf2409cf67296b648a5a6aee07f826ae",
    },
    IndexedMemberContract {
        id: "member_25",
        index: 25,
        table_pair_offset: 0x0c8,
        stored_offset: 0x242000,
        stored_size: 0x16a78,
        stored_sha256: "83f15011954cc9182b155e4252b7f30423b2772543e0a9ae69225efaa09f8576",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "9e9ca5801835f464e68cb9597a178c19c927bff32a48d8256357d45456298b90",
    },
    IndexedMemberContract {
        id: "member_26",
        index: 26,
        table_pair_offset: 0x0d0,
        stored_offset: 0x259000,
        stored_size: 0x172d2,
        stored_sha256: "5be7b7815b2b77364801974d1742225e1f7c43c102000d8ac06d3a2bab445d99",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "212153cea5151846e3b8c788a28981433f753693f5d98c858431b164e63790a4",
    },
    IndexedMemberContract {
        id: "member_27",
        index: 27,
        table_pair_offset: 0x0d8,
        stored_offset: 0x270800,
        stored_size: 0x16ac4,
        stored_sha256: "50fff0704206f9aa501f83884f74117b911d1372f4b4e25ba5ac8a63c897cce4",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "94c6269043c7d7ccfd84fdb489aecde0a41ac171dd1f4692a476029301b4fd7e",
    },
    IndexedMemberContract {
        id: "member_28",
        index: 28,
        table_pair_offset: 0x0e0,
        stored_offset: 0x287800,
        stored_size: 0x16a88,
        stored_sha256: "84d0a597b6cd3ef8a8051cb763d890ad1f0b2379480f823cc9c7979296006f6b",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "d8448ff9759b0bd91d15c1bdd24f2023ba9d17e58f8521ba31aada63af3dc2e2",
    },
    IndexedMemberContract {
        id: "member_29",
        index: 29,
        table_pair_offset: 0x0e8,
        stored_offset: 0x29e800,
        stored_size: 0x1730a,
        stored_sha256: "ed9de7fc25956709405b2f284a52a92d6ca65bc27a3bc2bc8c58ac6e37f4ca42",
        slot_size: 0x17800,
        decoded_size: 0x2b800,
        decoded_sha256: "6dbd5a50a6d7f5aae5d6ba370c053b7add66c04ba38e7de7ac4bf8c6c6f6da30",
    },
    IndexedMemberContract {
        id: "member_30",
        index: 30,
        table_pair_offset: 0x0f0,
        stored_offset: 0x2b6000,
        stored_size: 0x16a98,
        stored_sha256: "56da3c712eb550265934af70e3e22581398edc8d2b908cf004b7dbc8141849bf",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "2c69c5c054bb4f0e78cafe36e1db301304ddd7720bce876c611e644959b21303",
    },
    IndexedMemberContract {
        id: "member_31",
        index: 31,
        table_pair_offset: 0x0f8,
        stored_offset: 0x2cd000,
        stored_size: 0x16b06,
        stored_sha256: "bbe96892cbf6e04418823b6399047303bc5df1b79071f56458038a893f25b161",
        slot_size: 0x17000,
        decoded_size: 0x2b800,
        decoded_sha256: "c2a9aa6691aea65267392cbd3ffa58510aca386e6b467e7b268ed635a63f3375",
    },
];

pub(super) const EDIT_COMMAND_SHEET_ARCHIVE: IndexedMemberArchiveContract =
    IndexedMemberArchiveContract {
        record_size: 0x2e4000,
        record_sha256: "f6d61fdf4d7b4a57473d95def9855ab8da2bc28612458d54271d84df3e55cb28",
        header_size: 0x800,
        header_sha256: "904df22bedf16f6aee019f0890ab59c29ba0d0817bd6e2bab557ebb1e3301646",
        members: &EDIT_COMMAND_SHEET_MEMBERS,
    };

const MEMBER_DECODED_SIZE: usize = 0x2b800;
const SECONDARY_TIM_OFFSET: usize = 0x13000;
const KANRI_LOAD_ADDRESS: usize = 0x800a2000;
const EDIT_COMMAND_SHEET_CATALOG_INDEX: usize = 0x4a;
const EDIT_COMMAND_SHEET_DESTINATION: usize = 0x800d4000;
const CUSTOM_CHARACTER_RECORD_STRIDE: usize = 40;
const CHARACTER_RECORD_SELECTOR_BYTE_OFFSET: usize = 1;

struct EditCommandSheetLoadSiteSpec {
    id: &'static str,
    window_offset: usize,
    expected_words: &'static [u32],
    catalog_index_instruction_offset: usize,
    selector_load_instruction_offset: usize,
    selector_bias_instruction_offset: usize,
    selector_source: &'static str,
}

const CUSTOM_ARGUMENT_LOAD_WORDS: [u32; 16] = [
    0x0004_1080,
    0x0044_1021,
    0x3c04_800d,
    0x3484_4000,
    0x0002_10c0,
    0x3c03_801f,
    0x8c63_6360,
    0x2405_004a,
    0xafbf_0010,
    0x3c01_801f,
    0x0041_0821,
    0x9026_5805,
    0x8c62_015c,
    0x0000_0000,
    0x0040_f809,
    0x24c6_ffff,
];

const CUSTOM_SELECTION_LOAD_WORDS: [u32; 16] = [
    0x3c04_800d,
    0x3484_4000,
    0x8603_000e,
    0x2405_004a,
    0x0003_1080,
    0x0043_1021,
    0x0002_10c0,
    0x3c03_801f,
    0x8c63_6360,
    0x3c01_801f,
    0x0041_0821,
    0x9026_5805,
    0x8c62_015c,
    0x0000_0000,
    0x0040_f809,
    0x24c6_ffff,
];

const CPU_FIXED_RECORD_LOAD_WORDS: [u32; 11] = [
    0x3c04_800d,
    0x3484_4000,
    0x2405_004a,
    0x3c02_801f,
    0x8c42_6360,
    0x3c06_801f,
    0x90c6_5a85,
    0x8c42_015c,
    0x0000_0000,
    0x0040_f809,
    0x24c6_ffff,
];

const EDIT_COMMAND_SHEET_LOAD_SITES: [EditCommandSheetLoadSiteSpec; 3] = [
    EditCommandSheetLoadSiteSpec {
        id: "custom_record_by_argument",
        window_offset: 0x4448,
        expected_words: &CUSTOM_ARGUMENT_LOAD_WORDS,
        catalog_index_instruction_offset: 0x4464,
        selector_load_instruction_offset: 0x4474,
        selector_bias_instruction_offset: 0x4484,
        selector_source: "custom_character_record[a0].byte_1",
    },
    EditCommandSheetLoadSiteSpec {
        id: "custom_record_by_selection",
        window_offset: 0x5018,
        expected_words: &CUSTOM_SELECTION_LOAD_WORDS,
        catalog_index_instruction_offset: 0x5024,
        selector_load_instruction_offset: 0x5044,
        selector_bias_instruction_offset: 0x5054,
        selector_source: "custom_character_record[selection].byte_1",
    },
    EditCommandSheetLoadSiteSpec {
        id: "cpu_fixed_record",
        window_offset: 0x63a0,
        expected_words: &CPU_FIXED_RECORD_LOAD_WORDS,
        catalog_index_instruction_offset: 0x63a8,
        selector_load_instruction_offset: 0x63b8,
        selector_bias_instruction_offset: 0x63c8,
        selector_source: "cpu_character_record_16.byte_1",
    },
];

pub(super) fn audit_edit_command_sheets(
    source: &ModeDescendantSourceRecord,
    consumer_source: &ModeDescendantSourceRecord,
    flattened_tims: &[EmbeddedTimAudit],
) -> Result<EditCommandSheetFamilyAudit> {
    ensure!(
        source.path == EDIT_COMMAND_SHEETS_PATH,
        "EDIT command-sheet audit received another source"
    );
    let consumer = audit_consumer_bindings(consumer_source)?;
    let rebuilt = rebuild_indexed_member_archive(
        &source.stored,
        &EDIT_COMMAND_SHEET_ARCHIVE,
        &source.decoded,
    )?;
    ensure!(
        rebuilt.physical == source.stored
            && rebuilt.members.len() == EDIT_COMMAND_SHEET_MEMBERS.len()
            && rebuilt
                .members
                .iter()
                .all(|member| member.changed_stored_byte_count == 0),
        "EDIT command-sheet no-change rebuild changed the source archive"
    );

    let mut primary_hashes = BTreeSet::new();
    let mut secondary_hashes = BTreeSet::new();
    let mut members = Vec::with_capacity(EDIT_COMMAND_SHEET_MEMBERS.len());
    let mut decoded_offset = 0usize;
    for contract in &EDIT_COMMAND_SHEET_MEMBERS {
        ensure!(
            contract.decoded_size == MEMBER_DECODED_SIZE,
            "EDIT command-sheet member {} changed its decoded extent",
            contract.id
        );
        let member_tims = flattened_tims
            .iter()
            .filter(|tim| {
                (decoded_offset..decoded_offset + contract.decoded_size).contains(&tim.offset)
            })
            .collect::<Vec<_>>();
        ensure!(
            member_tims.len() == 2
                && member_tims[0].offset == decoded_offset
                && member_tims[1].offset == decoded_offset + SECONDARY_TIM_OFFSET,
            "EDIT command-sheet member {} no longer has its exact two-TIM layout",
            contract.id
        );
        let mut primary_tim = (*member_tims[0]).clone();
        let mut secondary_tim = (*member_tims[1]).clone();
        primary_tim.offset -= decoded_offset;
        secondary_tim.offset -= decoded_offset;
        validate_primary_tim(contract.id, &primary_tim)?;
        validate_secondary_tim(contract.id, &secondary_tim)?;
        primary_hashes.insert(primary_tim.source_tim_sha256.clone());
        secondary_hashes.insert(secondary_tim.source_tim_sha256.clone());
        members.push(EditCommandSheetMemberAudit {
            id: contract.id.to_string(),
            index: contract.index,
            selector_value: contract.index + 1,
            table_pair_offset: contract.table_pair_offset,
            stored_offset: contract.stored_offset,
            stored_size: contract.stored_size,
            slot_size: contract.slot_size,
            source_stored_sha256: contract.stored_sha256.to_string(),
            decoded_offset,
            decoded_size: contract.decoded_size,
            source_decoded_sha256: contract.decoded_sha256.to_string(),
            primary_tim,
            secondary_tim,
        });
        decoded_offset = decoded_offset
            .checked_add(contract.decoded_size)
            .context("EDIT command-sheet decoded member offsets overflowed")?;
    }
    ensure!(
        decoded_offset == source.decoded.len()
            && primary_hashes.len() == EDIT_COMMAND_SHEET_MEMBERS.len()
            && secondary_hashes.len() == 1,
        "EDIT command-sheet family identity changed"
    );

    Ok(EditCommandSheetFamilyAudit {
        source_path: source.path.to_string(),
        member_count: members.len(),
        all_primary_tims_are_unique: true,
        all_secondary_tims_are_identical: true,
        no_change_rebuild_is_byte_identical: true,
        consumer,
        members,
    })
}

fn audit_consumer_bindings(
    source: &ModeDescendantSourceRecord,
) -> Result<EditCommandSheetConsumerAudit> {
    ensure!(
        source.path == EDIT_REGISTRATION_OVERLAY_RECORD.path
            && source.stored == source.decoded
            && source.decoded.len() == EDIT_REGISTRATION_OVERLAY_RECORD.size
            && sha256_bytes(&source.decoded) == EDIT_REGISTRATION_OVERLAY_RECORD.sha256,
        "EDIT command-sheet consumer identity changed"
    );

    let catalog_word = 0x2405_004au32.to_le_bytes();
    let (instruction_words, trailing_bytes) = source.decoded.as_chunks::<4>();
    ensure!(
        trailing_bytes.is_empty(),
        "KANRI executable extent is not instruction-aligned"
    );
    let catalog_load_offsets = instruction_words
        .iter()
        .enumerate()
        .filter_map(|(index, word)| (*word == catalog_word).then_some(index * 4))
        .collect::<Vec<_>>();
    ensure!(
        catalog_load_offsets
            == EDIT_COMMAND_SHEET_LOAD_SITES
                .iter()
                .map(|site| site.catalog_index_instruction_offset)
                .collect::<Vec<_>>(),
        "KANRI EDITCM loader denominator changed"
    );

    let mut load_sites = Vec::with_capacity(EDIT_COMMAND_SHEET_LOAD_SITES.len());
    for site in &EDIT_COMMAND_SHEET_LOAD_SITES {
        validate_instruction_window(&source.decoded, site)?;
        load_sites.push(EditCommandSheetLoadSiteAudit {
            id: site.id.to_string(),
            catalog_index_instruction_offset: format!(
                "0x{:04x}",
                site.catalog_index_instruction_offset
            ),
            selector_load_instruction_offset: format!(
                "0x{:04x}",
                site.selector_load_instruction_offset
            ),
            selector_bias_instruction_offset: format!(
                "0x{:04x}",
                site.selector_bias_instruction_offset
            ),
            selector_source: site.selector_source.to_string(),
        });
    }

    Ok(EditCommandSheetConsumerAudit {
        source_path: source.path.to_string(),
        source_sha256: sha256_bytes(&source.decoded),
        source_load_address: format!("0x{KANRI_LOAD_ADDRESS:08x}"),
        catalog_index: EDIT_COMMAND_SHEET_CATALOG_INDEX,
        decoded_destination_address: format!("0x{EDIT_COMMAND_SHEET_DESTINATION:08x}"),
        selector_bias: -1,
        custom_record_stride_bytes: CUSTOM_CHARACTER_RECORD_STRIDE,
        custom_record_selector_byte_offset: CHARACTER_RECORD_SELECTOR_BYTE_OFFSET,
        load_site_count: load_sites.len(),
        member_selector_value_range_guard_present: false,
        source_bindings_verified: true,
        load_sites,
    })
}

fn validate_instruction_window(source: &[u8], site: &EditCommandSheetLoadSiteSpec) -> Result<()> {
    let end = site
        .window_offset
        .checked_add(site.expected_words.len() * 4)
        .context("EDIT command-sheet consumer window overflowed")?;
    let actual = source
        .get(site.window_offset..end)
        .with_context(|| format!("EDIT command-sheet consumer {} is out of bounds", site.id))?;
    let expected = site
        .expected_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    ensure!(
        actual == expected,
        "EDIT command-sheet consumer {} instruction window changed",
        site.id
    );
    Ok(())
}

fn validate_primary_tim(member_id: &str, tim: &EmbeddedTimAudit) -> Result<()> {
    ensure!(
        tim.offset == 0
            && tim.bits_per_pixel == 4
            && tim.pixel_width == 412
            && tim.pixel_height == 369
            && tim.image_vram_word_x == 512
            && tim.image_vram_y == 0
            && tim.clut_vram_x == 0
            && tim.clut_vram_y == 485
            && tim.palette_count == 1,
        "EDIT command-sheet member {member_id} primary TIM geometry changed"
    );
    Ok(())
}

fn validate_secondary_tim(member_id: &str, tim: &EmbeddedTimAudit) -> Result<()> {
    ensure!(
        tim.offset == SECONDARY_TIM_OFFSET
            && tim.bits_per_pixel == 4
            && tim.pixel_width == 512
            && tim.pixel_height == 256
            && tim.image_vram_word_x == 640
            && tim.image_vram_y == 256
            && tim.clut_vram_x == 0
            && tim.clut_vram_y == 484
            && tim.palette_count == 1,
        "EDIT command-sheet member {member_id} secondary TIM geometry changed"
    );
    Ok(())
}
