pub(crate) const SOURCE_BIN_SHA256: &str =
    "62dbc6ca47ec8d9dfbb5797f35d4e720d63e00b5a8e0edd080b149c3ff4f1823";
pub(crate) const SOURCE_BIN_BYTE_COUNT: u64 = 663_216_960;

#[derive(Clone, Copy)]
pub(crate) struct CompressedSourceRecordProfile {
    pub(crate) path: &'static str,
    pub(crate) stored_sha256: &'static str,
    pub(crate) decoded_sha256: &'static str,
    pub(crate) decoded_size: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceRecordProfile {
    pub(crate) path: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) size: usize,
}

pub(crate) struct TimRegionProfile {
    pub(crate) offset: usize,
    pub(crate) size: usize,
    pub(crate) pixel_width: usize,
    pub(crate) height: usize,
    pub(crate) image_x: u16,
    pub(crate) image_y: u16,
}

pub(crate) const BONUS_INVENTORY_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/KOUBAI1.TIZ",
        stored_sha256: "3f2799f520e73b672cb36eaca3dfabddaa2318790c04311782b87059911c9da4",
        decoded_sha256: "d29a680ee12008bc88d7c690ab5db82d288a89273db1913867782c99818ce4fd",
        decoded_size: 0x41800,
    };

pub(crate) const BONUS_INVENTORY_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/KOUBAI2.BIN",
    sha256: "9c1e0b2f226e3c16dc4622068477ff77ab97ecc738db1273680d391f3fa4e393",
    size: 94_704,
};

pub(crate) const BONUS_INVENTORY_GLYPH_TIM: TimRegionProfile = TimRegionProfile {
    offset: 0x21000,
    size: 0x201c0,
    pixel_width: 1024,
    height: 256,
    image_x: 512,
    image_y: 0,
};

pub(crate) const BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256: &str =
    "7a12e561363385e9dfeeab326368731c030ed4b374e7f5897ac819159d2884c5";

pub(crate) const MENU_RECORD: CompressedSourceRecordProfile = CompressedSourceRecordProfile {
    path: "DAT2/MENU.BIZ",
    stored_sha256: "9470f147b1bd17a07687608852ab388227ce5bbde9d67dd1f3d7ec21e335cde1",
    decoded_sha256: "9fe70fa139cbb7859b21d6e37e8107df550631ca516994f3f1467664a4701f6f",
    decoded_size: 653_312,
};

pub(crate) const MODE_SELECT_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/MODESEL.BIN",
    sha256: "d0f5b6cb21d2a7b0b9da5f18ac718e630105d2b9e525512cfe237257adbd4f9d",
    size: 43_288,
};

pub(crate) const GORIN_SELECTION_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/MINISEL.BIN",
    sha256: "dca00b504a885005658e860352e04ee7a4e66f774c3ce9b401e634b6b1398694",
    size: 0x2438,
};

pub(crate) const GORIN_MAIN_MENU_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/MINITTL0.BIZ",
        stored_sha256: "ccf1415783c539f6200430c416a117a48ea65d3d785126d14b7e60618e15891f",
        decoded_sha256: "91205dfe499c04abeea491f742989a98de6dcdfffd1e6b624e518ef13900a435",
        decoded_size: 0x55000,
    };

pub(crate) const EDIT_REGISTRATION_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/KANRI.BIN",
    sha256: "be317e6eb86ad7d15c68a1c6a3bf8237009a73c1b8edc4e4cdc5350576c20627",
    size: 0x83a8,
};

pub(crate) const EDIT_REGISTRATION_UI_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/EDITMOJI.TIZ",
        stored_sha256: "df9f3c26a75b5c723d2174895eb1fd014480039f970a475e111a68a2d6923c2c",
        decoded_sha256: "94df6e4047d6e1c45cbef71cedd84f571ca7a188f130757ae46788a361960df6",
        decoded_size: 0x2b800,
    };

pub(crate) const BONUS_MENU_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/KOUBAI.BIN",
    sha256: "dfb9074d0a05c95f09dac034087918f1cf3eb93842603ca23cebf32d2d24c49e",
    size: 0x90c0,
};

pub(crate) const BONUS_MENU_RECORD: CompressedSourceRecordProfile = CompressedSourceRecordProfile {
    path: "DAT2/KOUBAI0.TIZ",
    stored_sha256: "4cfb3f72a3fbbce90a5efc954f5a9c9501a77580315f079a375a1ad83bb24356",
    decoded_sha256: "b508bfd6dc6e36cd8dd47ed0ed3e9147efdc3700c5b67d3fba315ddc10a005c9",
    decoded_size: 0x4d000,
};

pub(crate) const CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD: SourceRecordProfile =
    SourceRecordProfile {
        path: "DAT1/PLSEL1.BIN",
        sha256: "3e791ff2c265bf85a2b11c36119e24586d41450a3a81647f77f18f7b6b9a5b35",
        size: 24_020,
    };

pub(crate) const CHARACTER_SELECT_PLSEL2_OVERLAY_RECORD: SourceRecordProfile =
    SourceRecordProfile {
        path: "DAT1/PLSEL2.BIN",
        sha256: "d1a202ec600aa5f916083a2e1189b31c1f6f52969be7ef42de2ce56c2bb7cb9c",
        size: 20_454,
    };

pub(crate) const CHARACTER_SELECT_PLSEL3_OVERLAY_RECORD: SourceRecordProfile =
    SourceRecordProfile {
        path: "DAT1/PLSEL3.BIN",
        sha256: "bb8bc2987572d3c11eee86d60473bde270979f0c6b37bab6a01e9f8bad68bff5",
        size: 36_296,
    };

pub(crate) const CHARACTER_SELECT_PLSEL4_OVERLAY_RECORD: SourceRecordProfile =
    SourceRecordProfile {
        path: "DAT1/PLSEL4.BIN",
        sha256: "ef6563a0a6bdfab248a8f8ff950c6ca72259132026ea63d6939735ebc2d64a3c",
        size: 41_676,
    };

pub(crate) const CHARACTER_SELECT_PLSEL5_OVERLAY_RECORD: SourceRecordProfile =
    SourceRecordProfile {
        path: "DAT1/PLSEL5.BIN",
        sha256: "3461b0ed24bb41acbbf44546d2c65ded2ac1d4d295423ed355d3f6b8352eca11",
        size: 50_940,
    };

pub(crate) const CHARACTER_SELECT_OVERLAY_RECORDS: [SourceRecordProfile; 5] = [
    CHARACTER_SELECT_PLSEL1_OVERLAY_RECORD,
    CHARACTER_SELECT_PLSEL2_OVERLAY_RECORD,
    CHARACTER_SELECT_PLSEL3_OVERLAY_RECORD,
    CHARACTER_SELECT_PLSEL4_OVERLAY_RECORD,
    CHARACTER_SELECT_PLSEL5_OVERLAY_RECORD,
];

pub(crate) const CHARACTER_SELECT_SELP1_TEXTURE_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/SELP1.BIZ",
        stored_sha256: "d872d152cbfe99212a124f1071365e52ee741eeafb8427ee233709e2e4688809",
        decoded_sha256: "17336b981e3fb8b0c8af8a6e2c4290b7a27cefb1012c2b28a63158a890091257",
        decoded_size: 0x50200,
    };

pub(crate) const CHARACTER_SELECT_SELP2_TEXTURE_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/SELP2.BIZ",
        stored_sha256: "e94e4c4000087a08dcfd7e2c4c14a342e10c48067788412c533ea575fa2db568",
        decoded_sha256: "a6e4987af2c92720f6df7c8887c484b306f982bf290c61042e2a5ac54c7826a8",
        decoded_size: 0x38000,
    };

pub(crate) const CHARACTER_SELECT_SELP3_TEXTURE_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/SELP3.BIZ",
        stored_sha256: "f74021230435c57622f99024d33391aded710cbbdd1442f7b8b83d51eb28570f",
        decoded_sha256: "0d9fce797c2be8f2d63dd405b0dee4df94f17b072e82b9f73b82dac06b453449",
        decoded_size: 0x38000,
    };

pub(crate) const CHARACTER_SELECT_SELP4_TEXTURE_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/SELP4.BIZ",
        stored_sha256: "d5e7b77781a97dfbd57a3481107f32389beaa973f1b81147d599b8eeec963a49",
        decoded_sha256: "2a5fbcfa15aa5cb29e226fd31fc7e02743b70435eae636e034baac6b97e7d728",
        decoded_size: 0x38000,
    };

pub(crate) const CHARACTER_SELECT_SELP5_TEXTURE_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/SELP5.BIZ",
        stored_sha256: "b84fa5dba7cd7a9b98794f6a8737f8b07c1b5bfc638ad8d29551fb7a26bf2ca3",
        decoded_sha256: "59f3d05cd1c03fd1530361a99e0a1a613eefa0a5dd0c26c27974fe4cdb1957d9",
        decoded_size: 0x38000,
    };

pub(crate) const CHARACTER_SELECT_TEXTURE_RECORDS: [CompressedSourceRecordProfile; 5] = [
    CHARACTER_SELECT_SELP1_TEXTURE_RECORD,
    CHARACTER_SELECT_SELP2_TEXTURE_RECORD,
    CHARACTER_SELECT_SELP3_TEXTURE_RECORD,
    CHARACTER_SELECT_SELP4_TEXTURE_RECORD,
    CHARACTER_SELECT_SELP5_TEXTURE_RECORD,
];

pub(crate) const CHARACTER_SELECT_COOPERATIVE_MENU_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/AISYOU.TIZ",
        stored_sha256: "ef2f40c68e5593391dad6f42702b1fad09a215b3457ce23620841617351ca80f",
        decoded_sha256: "e91b22fe572c61da07ab1779f35a59fbaa2d21588555ef639e8ce39b2ce0e862",
        decoded_size: 0x6e800,
    };

pub(crate) const TITLE_MENU_OVERLAY_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT1/MGTIT.BIZ",
        stored_sha256: "2130913aba89a8dd19555bad5e2b21d0f666a9e684fd7f35f2e703a2df4d5b62",
        decoded_sha256: "859515ec9f58a2326708dbbf46516e8601cb59d158c0da3f2e214311762a07d1",
        decoded_size: 48_360,
    };

pub(crate) const TITLE_MENU_OVERLAY_STORED_SIZE: usize = 26_890;

pub(crate) const TITLE_MENU_DECODED_ORACLE_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/MGTIT.BIN",
    sha256: TITLE_MENU_OVERLAY_RECORD.decoded_sha256,
    size: TITLE_MENU_OVERLAY_RECORD.decoded_size,
};

pub(crate) const OPTIONS_OVERLAY_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "DAT1/NEWOPT.BIN",
    sha256: "221f6eb284509304c4bf3c68c68eb944378f93026864bf33cf66a04aeca95187",
    size: 14_896,
};

pub(crate) const OPTIONS_INFO_RECORD: CompressedSourceRecordProfile =
    CompressedSourceRecordProfile {
        path: "DAT2/OPTINFO.TIZ",
        stored_sha256: "8af6bc67f029902f75f54b4b75f95c5c70424f694185b88de44454a20330eced",
        decoded_sha256: "72f7a4b4dbfc4985c4afab40904876492fd443fbf344d9774bda768465e7bf8b",
        decoded_size: 5 * 0x8800,
    };

pub(crate) const MAIN_EXECUTABLE_RECORD: SourceRecordProfile = SourceRecordProfile {
    path: "SLPS_021.20",
    sha256: "3aaa6696582d918ea6ba88b6efe2285ecad2d1c681e1a64cd757df63d24bbfe5",
    size: 595_968,
};
