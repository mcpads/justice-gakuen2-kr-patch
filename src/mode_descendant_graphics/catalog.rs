//! Physical-record catalog for mode-descendant surfaces.

use std::collections::BTreeSet;

use super::edit_command_sheets::{EDIT_COMMAND_SHEET_ARCHIVE, EDIT_COMMAND_SHEETS_PATH};
use super::indexed_member_archive::{IndexedMemberArchiveContract, IndexedMemberContract};
use super::model::{ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface};

pub const EDIT_SHARED_UI_PATH: &str = "DAT2/EDITMOJI.TIZ";
pub const GORIN_MAIN_MENU_PATH: &str = "DAT2/MINITTL0.BIZ";
pub const PRACTICAL_BASICS_TEXTURE_PATH: &str = "DAT2/SIKEN1.BIZ";
pub const PRACTICAL_1999_TEXTURE_PATH: &str = "DAT2/SIKEN10.BIZ";
pub const PRACTICAL_BASICS_DESCRIPTOR_PATH: &str = "DAT1/SIKEN.BIN";
pub const PRACTICAL_1999_DESCRIPTOR_PATH: &str = "DAT1/SIKEN2.BIN";
pub const PRACTICAL_BASICS_RESULTS_PATH: &str = "DAT2/SIKEN2.BIZ";
pub const PRACTICAL_1999_RESULTS_PATH: &str = "DAT2/SIKEN20.BIZ";
pub const PRACTICAL_RESULT_TITLES_PATH: &str = "DAT2/SIKENKK.BIZ";

const PRACTICAL_1999_RESULT_MEMBERS: [IndexedMemberContract; 2] = [
    IndexedMemberContract {
        id: "member_00",
        index: 0,
        table_pair_offset: 0,
        stored_offset: 0x800,
        stored_size: 0x3eaa0,
        stored_sha256: "7269f49a96fca6db64388710c93de0d46558dc12fcaddb84091a0cdf93f61dbd",
        slot_size: 0x3f000,
        decoded_size: 0x6d000,
        decoded_sha256: "40e85424a8481f66cd469abce8e92b019facc59cf94c0c6ca982a79e6a39987e",
    },
    IndexedMemberContract {
        id: "member_01",
        index: 1,
        table_pair_offset: 8,
        stored_offset: 0x3f800,
        stored_size: 0x37f5a,
        stored_sha256: "c110f5b16a754c48ff787ff12b02cb0625a37a2a3792ea4f2724f763028da0d1",
        slot_size: 0x38000,
        decoded_size: 0x6d000,
        decoded_sha256: "07e957cce731a01e5e9c7a5282002fcd2fab51d912dff1eafe37736cddb62df2",
    },
];

pub(super) const PRACTICAL_1999_RESULT_ARCHIVE: IndexedMemberArchiveContract =
    IndexedMemberArchiveContract {
        record_size: 0x77800,
        record_sha256: "0287be425dbff787bbbb811d176de375262f7edbd62178736aa276a4371dc8e1",
        header_size: 0x800,
        header_sha256: "916f62ff911540cea620d098f39503cd06738c633a13327106545fba4e16ec26",
        members: &PRACTICAL_1999_RESULT_MEMBERS,
    };

const PRACTICAL_RESULT_TITLE_MEMBERS: [IndexedMemberContract; 33] = [
    IndexedMemberContract {
        id: "member_00",
        index: 0,
        table_pair_offset: 0x000,
        stored_offset: 0x00800,
        stored_size: 0x486,
        stored_sha256: "c9f49dae2d214d6ee680caa9bc0e579f8389da1a2b1fd447b0092c555d162dfa",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "887a4c8815506ce0e96bce8a693d72b004f4bf5d5d968190bb4065c34c2d74ca",
    },
    IndexedMemberContract {
        id: "member_01",
        index: 1,
        table_pair_offset: 0x008,
        stored_offset: 0x01000,
        stored_size: 0x384,
        stored_sha256: "aef2620c9b025e91280fcec01b9d22df85be7fa166a9995fbf5c80ec49b0fe0d",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "de37745426e239d9880669ba4d9ac61296b0f6c2505c7bc4c130c04600a22d85",
    },
    IndexedMemberContract {
        id: "member_02",
        index: 2,
        table_pair_offset: 0x010,
        stored_offset: 0x01800,
        stored_size: 0x44a,
        stored_sha256: "788d14dc94d25176f6eeb2f9145c3ba5c5f14da87999dad6c556de11e79f20be",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "bf5434557d2898860f14a0350288667056071c5383e46d6e6136ac1573a8b5a7",
    },
    IndexedMemberContract {
        id: "member_03",
        index: 3,
        table_pair_offset: 0x018,
        stored_offset: 0x02000,
        stored_size: 0x3a4,
        stored_sha256: "130d7c7285fa93bfd087e4517455c523281ad1f6a359a157852f08b8429322a1",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "0c5dc5a2b123fe1e23a735fd77546ca18cdb5ca9ec5286c6cca1ef48a6a198e6",
    },
    IndexedMemberContract {
        id: "member_04",
        index: 4,
        table_pair_offset: 0x020,
        stored_offset: 0x02800,
        stored_size: 0x4b2,
        stored_sha256: "20b0c178b33e547e96379b97486f7c1489e2c92e61685b3741d073ffa2217aaa",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "2503e09649f379c4d252858c3f73d2a81918c2b0f7a479998923d8ede9444932",
    },
    IndexedMemberContract {
        id: "member_05",
        index: 5,
        table_pair_offset: 0x028,
        stored_offset: 0x03000,
        stored_size: 0x408,
        stored_sha256: "d646c9d228915c8d2df5b9f431ba5201ca156bc88ac79a10d5f8f7ecb176e793",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "2b3f260fa9ef9ffdc8b127a69484832915a1a03522d21196f2448257811995c1",
    },
    IndexedMemberContract {
        id: "member_06",
        index: 6,
        table_pair_offset: 0x030,
        stored_offset: 0x03800,
        stored_size: 0x3a2,
        stored_sha256: "dfa41c2eafba440e8b37ba7fac71bb2ee7f4d3749278395334c62f0942b97e4a",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "ae961aa848c797feb71b5d1f1ac2e89142fadfbabfa4782f0fabd86fa9da02fd",
    },
    IndexedMemberContract {
        id: "member_07",
        index: 7,
        table_pair_offset: 0x038,
        stored_offset: 0x04000,
        stored_size: 0x4fa,
        stored_sha256: "744041bc13a691792f5a3788e81a381873d4796cc0738d39ecbb460c3cd7b05b",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "8604bea836add4dd7764d2b3e49ff2a302bdfd632ddaa57325da15912cf753bc",
    },
    IndexedMemberContract {
        id: "member_08",
        index: 8,
        table_pair_offset: 0x040,
        stored_offset: 0x04800,
        stored_size: 0x338,
        stored_sha256: "22ad0b0eb82b3f04460d15a0bab955945b21dbdccd1312456c1a692f6e973b6f",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "91727744f67bd7fb9587011669bd2c68d476d89d05a5c71ba9cac4d54a0c1cb5",
    },
    IndexedMemberContract {
        id: "member_09",
        index: 9,
        table_pair_offset: 0x048,
        stored_offset: 0x05000,
        stored_size: 0x50c,
        stored_sha256: "4b55b36a92189081ed6bf4c3da734152798b03f7dd31b3c0233b4f8cfd97b7cf",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "05acbba4c622d670f1922efe9c1d2675da6e7b7fe7646db1f00cd3e442efc72f",
    },
    IndexedMemberContract {
        id: "member_10",
        index: 10,
        table_pair_offset: 0x050,
        stored_offset: 0x05800,
        stored_size: 0x4ce,
        stored_sha256: "09a42f651415b36485ae75fd6e5ba9d72f68656403b1ccbb76fcedf3f649590a",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "e9c4bf384fc8263d720d950528ceffd75374a826f50e339544c82d3e4d9b1aeb",
    },
    IndexedMemberContract {
        id: "member_11",
        index: 11,
        table_pair_offset: 0x058,
        stored_offset: 0x06000,
        stored_size: 0x580,
        stored_sha256: "84d4ad895e76673e4f4185874beaadc182e434a1d513a87682f6ac16c912b375",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "069dfc284bbf63cd0e15ff5b980498905a7ddc52ab17792f70eca13c77eb87bb",
    },
    IndexedMemberContract {
        id: "member_12",
        index: 12,
        table_pair_offset: 0x060,
        stored_offset: 0x06800,
        stored_size: 0x58c,
        stored_sha256: "71c27a30d9a0904579ec5e96d0221b160025ca78a8ad95c9341c39caac3540b5",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "4871c4c8419d6e60ab97e3a7cab89cd25f6449e0ca1cd4e14706a1cc6236fd2f",
    },
    IndexedMemberContract {
        id: "member_13",
        index: 13,
        table_pair_offset: 0x068,
        stored_offset: 0x07000,
        stored_size: 0x50e,
        stored_sha256: "d40664bbe24c30d1547d95528b0309071361a3a2972a9da32997155e595d2de0",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "f9a713d5ba17479fccd33ad5b540701ebf9121cbfaf51130a711af4d1b264a67",
    },
    IndexedMemberContract {
        id: "member_14",
        index: 14,
        table_pair_offset: 0x070,
        stored_offset: 0x07800,
        stored_size: 0x4fe,
        stored_sha256: "5cc7687650f40741ba47bfe15de9fefe512065b2ae11fa62db8f6beb6b5af05b",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "8526698bca3e488b00446a13df2178685e3943cf46d1a8cd3b1ae9d56f1eae23",
    },
    IndexedMemberContract {
        id: "member_15",
        index: 15,
        table_pair_offset: 0x078,
        stored_offset: 0x08000,
        stored_size: 0x436,
        stored_sha256: "3582083f689aabdccdba519801441371fed8150fded755dd740b50fe6168e622",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "f3819eae66a5693ec8bd68577b4c0a7aee4b1c6f60c2923cf965051e8c646175",
    },
    IndexedMemberContract {
        id: "member_16",
        index: 16,
        table_pair_offset: 0x080,
        stored_offset: 0x08800,
        stored_size: 0x570,
        stored_sha256: "32797da72d6dc7c562b3e8cda551f4fefd02a70ebbf5e79bd14fd4a5ee54cbf0",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "5e847885efdfebf0f3078a4f93ffde05c6b5732b5652419815ee52b5df539332",
    },
    IndexedMemberContract {
        id: "member_17",
        index: 17,
        table_pair_offset: 0x088,
        stored_offset: 0x09000,
        stored_size: 0x502,
        stored_sha256: "73c1c8742943a733dabc55eb48da7251e4799e055f2cb5df9266c6516293e151",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "e722ad1694d1bf7299e8f3d9022299a7bb80264e6875e317287d4fa20f562c7f",
    },
    IndexedMemberContract {
        id: "member_18",
        index: 18,
        table_pair_offset: 0x090,
        stored_offset: 0x09800,
        stored_size: 0x4ce,
        stored_sha256: "f9450b9c1bb3350989f14b4536da157cfc76150891c58a257144f9f40f1b1473",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "8b05f22773b5bb88bc0b87794f3e47fc107e09c7680c3a6051293960818ba9e1",
    },
    IndexedMemberContract {
        id: "member_19",
        index: 19,
        table_pair_offset: 0x098,
        stored_offset: 0x0a000,
        stored_size: 0x55a,
        stored_sha256: "32584581b58eec21467383d15e866e61049dd0f0a950df6a0613f5affecbd091",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "5336edb73d32ca4f168830ee824cc02aa98dfaaf9556f24ca1f1e2a31a31dc96",
    },
    IndexedMemberContract {
        id: "member_20",
        index: 20,
        table_pair_offset: 0x0a0,
        stored_offset: 0x0a800,
        stored_size: 0x524,
        stored_sha256: "a7a4171f75c100fb28c75ca97a40d3c658a6aa1283b8564952deec08d93a64c6",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "f27191584e4eb605f1efb9674e9197474990b48948da080c557a7f99602bba69",
    },
    IndexedMemberContract {
        id: "member_21",
        index: 21,
        table_pair_offset: 0x0a8,
        stored_offset: 0x0b000,
        stored_size: 0x546,
        stored_sha256: "cef117193ac9e49775789008f8281c1cf8d647f219b777a0448457ebcb43537c",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "ee18a532ec09f3fde3fa3e3196e371b4970a5563723212cb087520fb7a6dcaca",
    },
    IndexedMemberContract {
        id: "member_22",
        index: 22,
        table_pair_offset: 0x0b0,
        stored_offset: 0x0b800,
        stored_size: 0x51e,
        stored_sha256: "f8f6adf61979eaf72f387cb854e183acd002da4a1f05ff632e3db9612c9928d7",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "0d1bda9419c6db5a413d379fb18c2f518859eacee1f8d3bff2245007a385e61f",
    },
    IndexedMemberContract {
        id: "member_23",
        index: 23,
        table_pair_offset: 0x0b8,
        stored_offset: 0x0c000,
        stored_size: 0x53e,
        stored_sha256: "284752686a8ec4a8ea5b83ad228d7c44304327b50b3b182af1f92620980c01c7",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "693a20e1c3af07ce74fb46c41aad2bb349e2dfa198f277f7fdca0e0bedafe3bc",
    },
    IndexedMemberContract {
        id: "member_24",
        index: 24,
        table_pair_offset: 0x0c0,
        stored_offset: 0x0c800,
        stored_size: 0x54c,
        stored_sha256: "4018ecf5f1e8b0b03d54aa7be878ba31652ea760cdfb6d425254b9aaa69de0af",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "49a03a3392f143cadc3e2333012286f6279cd85136242695396c23ce9351b31a",
    },
    IndexedMemberContract {
        id: "member_25",
        index: 25,
        table_pair_offset: 0x0c8,
        stored_offset: 0x0d000,
        stored_size: 0x53c,
        stored_sha256: "e9083bae7b501ed97c4cf12704ca2e91b8f08e91ede8c76ceda235a35304ee42",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "7c620e0cddf2b796f8b4be56322db550d086cc808139c1bc08a81d9dfc434a5f",
    },
    IndexedMemberContract {
        id: "member_26",
        index: 26,
        table_pair_offset: 0x0d0,
        stored_offset: 0x0d800,
        stored_size: 0x588,
        stored_sha256: "98d2161a6d51e9f9e73af4bffe5c214a21526b4c60a97e922d0d63bf38663f86",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "d23256bd7287ad0adaeb681861e274bd1c982d6fb327e192e91e4267f50c367c",
    },
    IndexedMemberContract {
        id: "member_27",
        index: 27,
        table_pair_offset: 0x0d8,
        stored_offset: 0x0e000,
        stored_size: 0x58e,
        stored_sha256: "66d8b8f784b29b9e1677fcec59396109e041a060785fbc1aabc26fd4a63c0fcc",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "2eece5775fd4ae464fbc5db61eb9e6e9a3f0f5d8be767b17a64d01898a134349",
    },
    IndexedMemberContract {
        id: "member_28",
        index: 28,
        table_pair_offset: 0x0e0,
        stored_offset: 0x0e800,
        stored_size: 0x534,
        stored_sha256: "d464b73a85de728a6944681293c2f3637e2a590512c936f8b5b9bc5122b9ad8e",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "2cddafb06766e66933e75e7cff0a630f4ea31cebd1b5f34fc6cb333c7d99a923",
    },
    IndexedMemberContract {
        id: "member_29",
        index: 29,
        table_pair_offset: 0x0e8,
        stored_offset: 0x0f000,
        stored_size: 0x4ba,
        stored_sha256: "8cf92f78739ceaf280feb20f807686c65179120196d063c112cac52043062ab7",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "d053e9e29dacc6eef56fc7f0c207b8b5ac8b92fb80024cf00f0345c579ab68d6",
    },
    IndexedMemberContract {
        id: "member_30",
        index: 30,
        table_pair_offset: 0x0f0,
        stored_offset: 0x0f800,
        stored_size: 0x516,
        stored_sha256: "c46c9d3bfaf48ddebd772f4d7f1a174d1885a58f1bf3a7c6bf103b2852b6c044",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "028f0ac782f315aaa29f541bd7f0445ab7cfe91dfd369834821842c1fa7bf021",
    },
    IndexedMemberContract {
        id: "member_31",
        index: 31,
        table_pair_offset: 0x0f8,
        stored_offset: 0x10000,
        stored_size: 0x52c,
        stored_sha256: "8627d1a038b122800b2f89569247b910d567b94b7ff550e7ca22927f7c8de257",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "a85c628f0161a7e2cca9b02681bbe751e0f921590804f84ed740b3fb0ad9e4f0",
    },
    IndexedMemberContract {
        id: "member_32",
        index: 32,
        table_pair_offset: 0x100,
        stored_offset: 0x10800,
        stored_size: 0x474,
        stored_sha256: "4f0d64ebb2f9af13acf357284ccab4aca9c849edf98c6a79b1b747d1e063994c",
        slot_size: 0x800,
        decoded_size: 0x708,
        decoded_sha256: "ef1f316a58a0bae2d1cb02e4436a1537b8e4270b2249e6c1c7a724538fc4fe70",
    },
];

pub(super) const PRACTICAL_RESULT_TITLE_ARCHIVE: IndexedMemberArchiveContract =
    IndexedMemberArchiveContract {
        record_size: 0x11000,
        record_sha256: "e5491079a27fcdd3c45c32a7088ebd7d5188334b3ac16bf8ca440b82cfdbf654",
        header_size: 0x800,
        header_sha256: "0095f2bec5d886774adffb61fe5095166c9563ce1a149cfe1c9fdb8ac00dbc50",
        members: &PRACTICAL_RESULT_TITLE_MEMBERS,
    };

#[derive(Clone, Copy)]
pub(super) struct ModeDescendantTextureOutputSpec {
    pub(super) tim_offset: usize,
    pub(super) preview_file: &'static str,
}

pub(super) struct ModeDescendantRecordSpec {
    pub(super) record: ModeDescendantRecord,
    pub(super) surface: ModeDescendantSurface,
    pub(super) source_path: &'static str,
    pub(super) storage_kind: ModeDescendantStorageKind,
    pub(super) output_file: &'static str,
    pub(super) texture_outputs: &'static [ModeDescendantTextureOutputSpec],
}

const EDIT_TEXTURE_OUTPUTS: [ModeDescendantTextureOutputSpec; 2] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0,
        preview_file: "editmoji-shared-ui.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x21000,
        preview_file: "editmoji-clutless-ui-indexed.png",
    },
];
const GORIN_TEXTURE_OUTPUTS: [ModeDescendantTextureOutputSpec; 2] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0,
        preview_file: "minittl0-main-menu.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x18800,
        preview_file: "minittl0-heading.png",
    },
];
const PRACTICAL_BASICS_TEXTURE_OUTPUTS: [ModeDescendantTextureOutputSpec; 2] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x3c800,
        preview_file: "siken1-practical-shared-ui.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x55000,
        preview_file: "siken1-practical-results.png",
    },
];
const PRACTICAL_1999_TEXTURE_OUTPUTS: [ModeDescendantTextureOutputSpec; 2] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x3c800,
        preview_file: "siken10-practical-shared-ui.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x55000,
        preview_file: "siken10-practical-results.png",
    },
];
const PRACTICAL_BASICS_RESULT_OUTPUTS: [ModeDescendantTextureOutputSpec; 4] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0,
        preview_file: "siken2-practical-result-atlas.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x08800,
        preview_file: "siken2-practical-results.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x45000,
        preview_file: "siken2-practical-pass-stamp.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x58800,
        preview_file: "siken2-practical-fail-stamp.png",
    },
];
const PRACTICAL_1999_RESULT_OUTPUTS: [ModeDescendantTextureOutputSpec; 8] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 0,
        preview_file: "siken20-member-00-result-atlas.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x08800,
        preview_file: "siken20-member-00-practical-results.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x6d000,
        preview_file: "siken20-member-01-result-atlas.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x75800,
        preview_file: "siken20-member-01-result-summary.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x45000,
        preview_file: "siken20-member-00-pass-stamp.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0x58800,
        preview_file: "siken20-member-00-fail-stamp.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0xb2000,
        preview_file: "siken20-member-01-pass-stamp.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 0xc5800,
        preview_file: "siken20-member-01-fail-stamp.png",
    },
];
const PRACTICAL_RESULT_TITLE_OUTPUTS: [ModeDescendantTextureOutputSpec; 3] = [
    ModeDescendantTextureOutputSpec {
        tim_offset: 30 * 0x708,
        preview_file: "sikenkk-first-term-result-title.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 31 * 0x708,
        preview_file: "sikenkk-second-term-result-title.png",
    },
    ModeDescendantTextureOutputSpec {
        tim_offset: 32 * 0x708,
        preview_file: "sikenkk-school-year-result-title.png",
    },
];
const NO_TEXTURE_OUTPUTS: [ModeDescendantTextureOutputSpec; 0] = [];

pub(super) struct PracticalExamRecordSpecs {
    pub(super) basics_texture_producer: &'static ModeDescendantRecordSpec,
    pub(super) exam_1999_texture_producer: &'static ModeDescendantRecordSpec,
    pub(super) basics_descriptor_consumer: &'static ModeDescendantRecordSpec,
    pub(super) exam_1999_descriptor_consumer: &'static ModeDescendantRecordSpec,
}

#[derive(Clone, Copy)]
pub(super) struct PracticalResultRecordSpecs {
    pub(super) basics_texture_results: &'static ModeDescendantRecordSpec,
    pub(super) exam_1999_texture_results: &'static ModeDescendantRecordSpec,
    pub(super) basics_results: &'static ModeDescendantRecordSpec,
    pub(super) exam_1999_results: &'static ModeDescendantRecordSpec,
    pub(super) term_titles: &'static ModeDescendantRecordSpec,
}

const RECORD_SPECS: [ModeDescendantRecordSpec; 11] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::EditSharedUi,
        surface: ModeDescendantSurface::EditSharedUi,
        source_path: EDIT_SHARED_UI_PATH,
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "editmoji-shared-ui.tiz",
        texture_outputs: &EDIT_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinMainMenu,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: GORIN_MAIN_MENU_PATH,
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl0-main-menu.biz",
        texture_outputs: &GORIN_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::PracticalBasicsTextureProducer,
        surface: ModeDescendantSurface::PracticalExamSharedUi,
        source_path: PRACTICAL_BASICS_TEXTURE_PATH,
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "siken1-practical-shared-ui.biz",
        texture_outputs: &PRACTICAL_BASICS_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::Practical1999TextureProducer,
        surface: ModeDescendantSurface::PracticalExamSharedUi,
        source_path: PRACTICAL_1999_TEXTURE_PATH,
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "siken10-practical-shared-ui.biz",
        texture_outputs: &PRACTICAL_1999_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::PracticalBasicsDescriptorConsumer,
        surface: ModeDescendantSurface::PracticalExamSharedUi,
        source_path: PRACTICAL_BASICS_DESCRIPTOR_PATH,
        storage_kind: ModeDescendantStorageKind::Raw,
        output_file: "siken-practical-descriptors.bin",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::Practical1999DescriptorConsumer,
        surface: ModeDescendantSurface::PracticalExamSharedUi,
        source_path: PRACTICAL_1999_DESCRIPTOR_PATH,
        storage_kind: ModeDescendantStorageKind::Raw,
        output_file: "siken2-practical-descriptors.bin",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::PracticalBasicsResultGraphics,
        surface: ModeDescendantSurface::PracticalExamResults,
        source_path: PRACTICAL_BASICS_RESULTS_PATH,
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "siken2-practical-results.biz",
        texture_outputs: &PRACTICAL_BASICS_RESULT_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::Practical1999ResultGraphics,
        surface: ModeDescendantSurface::PracticalExamResults,
        source_path: PRACTICAL_1999_RESULTS_PATH,
        storage_kind: ModeDescendantStorageKind::IndexedCompressedMembers,
        output_file: "siken20-practical-results.biz",
        texture_outputs: &PRACTICAL_1999_RESULT_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::PracticalResultTermTitles,
        surface: ModeDescendantSurface::PracticalExamResults,
        source_path: PRACTICAL_RESULT_TITLES_PATH,
        storage_kind: ModeDescendantStorageKind::IndexedCompressedMembers,
        output_file: "sikenkk-practical-result-term-titles.biz",
        texture_outputs: &PRACTICAL_RESULT_TITLE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::EditCommandSheets,
        surface: ModeDescendantSurface::EditSharedUi,
        source_path: EDIT_COMMAND_SHEETS_PATH,
        storage_kind: ModeDescendantStorageKind::IndexedCompressedMembers,
        output_file: "editcm-command-sheets.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::EditTechniqueNames,
        surface: ModeDescendantSurface::EditSharedUi,
        source_path: super::edit_technique_archive::PATH,
        storage_kind: ModeDescendantStorageKind::IndexedCompressedMembers,
        output_file: "editce-technique-names.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
];

pub(super) fn edit_technique_record_spec() -> &'static ModeDescendantRecordSpec {
    &RECORD_SPECS[10]
}

// These records each carry the same source-pinned common menu TIM at offset zero.
const GORIN_GAME_RECORD_SPECS: [ModeDescendantRecordSpec; 12] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle1,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL1.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl1-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle2,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL2.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl2-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle3,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL3.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl3-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle4,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL4.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl4-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle5,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL5.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl5-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle6,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL6.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl6-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle7,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL7.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl7-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle8,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL8.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl8-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle9,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTL9.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittl9-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle10,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTLA.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittla-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle11,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTLB.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittlb-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinTitle12,
        surface: ModeDescendantSurface::GorinMainMenu,
        source_path: "DAT2/MINITTLC.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "minittlc-game-menu.biz",
        texture_outputs: &NO_TEXTURE_OUTPUTS,
    },
];

pub(super) fn fixed_record_specs_for_surfaces(
    surfaces: &BTreeSet<ModeDescendantSurface>,
) -> Vec<&'static ModeDescendantRecordSpec> {
    RECORD_SPECS[..2]
        .iter()
        .chain(GORIN_GAME_RECORD_SPECS.iter())
        .filter(|spec| surfaces.contains(&spec.surface))
        .collect()
}

pub(super) fn practical_exam_record_specs() -> PracticalExamRecordSpecs {
    PracticalExamRecordSpecs {
        basics_texture_producer: &RECORD_SPECS[2],
        exam_1999_texture_producer: &RECORD_SPECS[3],
        basics_descriptor_consumer: &RECORD_SPECS[4],
        exam_1999_descriptor_consumer: &RECORD_SPECS[5],
    }
}

pub(super) fn practical_result_record_specs() -> PracticalResultRecordSpecs {
    PracticalResultRecordSpecs {
        basics_texture_results: &RECORD_SPECS[2],
        exam_1999_texture_results: &RECORD_SPECS[3],
        basics_results: &RECORD_SPECS[6],
        exam_1999_results: &RECORD_SPECS[7],
        term_titles: &RECORD_SPECS[8],
    }
}

pub(super) fn edit_command_sheet_record_spec() -> &'static ModeDescendantRecordSpec {
    &RECORD_SPECS[9]
}

pub(super) fn indexed_member_archive_contract(
    source_path: &str,
) -> Option<&'static IndexedMemberArchiveContract> {
    match source_path {
        EDIT_COMMAND_SHEETS_PATH => Some(&EDIT_COMMAND_SHEET_ARCHIVE),
        super::edit_technique_archive::PATH => Some(&super::edit_technique_archive::ARCHIVE),
        PRACTICAL_1999_RESULTS_PATH => Some(&PRACTICAL_1999_RESULT_ARCHIVE),
        PRACTICAL_RESULT_TITLES_PATH => Some(&PRACTICAL_RESULT_TITLE_ARCHIVE),
        _ => None,
    }
}
