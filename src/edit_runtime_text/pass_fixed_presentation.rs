//! Finite PASS.BIN fixed-presentation consumers used by the EDIT mode.
//!
//! This is deliberately not an atlas scan.  Every record and descriptor below
//! is an exact, source-bound consumer.  The password alphabet and the dynamic
//! command sheet are separate semantic families. Password relabeling is owned
//! by password_alphabet and composed after fixed-text preservation checks.

use std::collections::{BTreeMap, BTreeSet};

use super::pass_password_title::{password_title_codes, validate_password_title};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::ShiftedSizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::menu_atlas_plan::MenuGlyphAllocation;
use crate::menu_audit::wrapped_cells_overlap_sized;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::text::{
    common_menu_ascii_glyph_code, fixed_menu_glyph_code, read_length_prefixed_codes,
};
use crate::tim::{
    Cell, parse_4bpp_without_clut_prefix, read_indexed_cell_without_clut_in_prefix,
    write_indexed_cell_without_clut_in_prefix,
};

use super::{kanri_name_runtime::KANRI_MENU_ATLAS_VRAM_WORD_X, source::OVERLAY_RUNTIME_BASE};

const PASS_RUNTIME_BASE: u32 = 0x8017_a000;
const PASS_IMAGE_SIZE: usize = 32_692;
const PASS_HEADER_WORDS: [u32; 3] = [0x8017_cd50, 0x8017_bad0, 0x8017_f840];
const PASS_RENDERER_PAGE_INSTRUCTIONS: [(usize, u32); 4] = [
    (0x1010, 0x3063_0fff),
    (0x1024, 0x0003_3203),
    (0x1028, 0x24c6_000c),
    (0x102c, 0x0006_3180),
];
const PASS_RENDERER_CODE_MASK: u16 = 0x0fff;
const PASS_DIRECT_PAGE_MASK: u16 = 0x0f00;
const PASS_DIRECT_PAGE: u16 = 0x0f00;
const EDITMOJI_TIM_OFFSET: usize = 0x21000;
const EDITMOJI_TIM_VRAM_WORD_X: u16 = 704;
const EDITMOJI_TIM_VRAM_WORD_WIDTH: usize = 64;
const PASSWORD_DISPLAY_CODE_TABLE_START: usize = 0x02cc;
const PASSWORD_VALUE_BYTE_TABLE_START: usize = 0x036c;
const PASSWORD_VALUE_BYTE_TABLE_END: usize = 0x03bc;
const PASSWORD_DISPLAY_ALIAS_CONSUMER_ID: &str = "pass_cpu_password_display";
const KANRI_PASSWORD_DISPLAY_RECORD_OFFSET: usize = 0x0674;
const KANRI_PASSWORD_DISPLAY_RECORD_SIZE: usize = 0x10;
const DIRECT_GLYPH_CELL_SIZE: usize = 20;
const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;
const ALLOCATABLE_SOURCE_CODE_LIMIT: u16 = 0x0400;
const TEXT_TERMINATOR: u16 = 0x0fff;
struct TextConsumer {
    id: &'static str,
    record_offset: usize,
    record_size: usize,
    descriptor_pointer_offsets: &'static [usize],
    source_codes: &'static [u16],
    asset_id: &'static str,
}

const FIXED_TEXT_CONSUMERS: [TextConsumer; 30] = [
    TextConsumer {
        id: "pass_register_another_question",
        record_offset: 0x00d8,
        record_size: 0x28,
        descriptor_pointer_offsets: &[0x0194],
        source_codes: &[
            0xff4a, 0x0094, 0x0126, 0x0185, 0x0152, 0x00a2, 0x0177, 0x0130, 0x0157, 0x0071, 0x0171,
            0x02b7, 0x01b1, 0x0083, 0x009a, 0x0084, 0x0079, 0x0055,
        ],
        asset_id: "pass_register_another_question",
    },
    TextConsumer {
        id: "pass_registration_choices",
        record_offset: 0x000c,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x0128],
        source_codes: &[0x02b7, 0x01b1, 0x0fff, 0x0234, 0xff18],
        asset_id: "pass_registration_choices",
    },
    TextConsumer {
        id: "pass_character_acceptance_controls",
        record_offset: 0x0038,
        record_size: 0x28,
        descriptor_pointer_offsets: &[0x0140],
        source_codes: &[
            0xff48, 0x0249, 0x0fff, 0x0fff, 0x0fff, 0x0fff, 0x0fff, 0x0fff, 0x0fff, 0x0fff, 0x02b7,
            0x01b1, 0x0fff, 0x0234, 0xff18, 0x0fff, 0x0200, 0x0082,
        ],
        asset_id: "pass_character_acceptance_controls",
    },
    TextConsumer {
        id: "pass_registration_complete",
        record_offset: 0x00bc,
        record_size: 0x1c,
        descriptor_pointer_offsets: &[0x0188],
        source_codes: &[
            0x0126, 0x0185, 0x0152, 0x0094, 0x02b7, 0x01b1, 0x00b2, 0x0212, 0x02b5, 0x0083, 0x009a,
            0x0083, 0x0087,
        ],
        asset_id: "pass_registration_complete",
    },
    TextConsumer {
        id: "pass_register_another_choices",
        record_offset: 0x0100,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x01a0],
        source_codes: &[
            0x0095, 0x0075, 0x0fff, 0x0fff, 0x0fff, 0x0075, 0x0075, 0x0077,
        ],
        asset_id: "pass_register_another_choices",
    },
    TextConsumer {
        id: "pass_cpu_registration_confirmation",
        record_offset: 0x0114,
        record_size: 0x10,
        descriptor_pointer_offsets: &[0x01ac],
        source_codes: &[0x02b7, 0x01b1, 0x0083, 0x009a, 0x0084, 0x0079],
        asset_id: "pass_cpu_registration_confirmation",
    },
    TextConsumer {
        id: "pass_invalid_password",
        record_offset: 0x0060,
        record_size: 0x18,
        descriptor_pointer_offsets: &[0x014c],
        source_codes: &[
            0x0177, 0x0130, 0x0157, 0x0071, 0x0171, 0x00b2, 0x0088, 0x00b2, 0x0075, 0x009a, 0x0084,
        ],
        asset_id: "pass_invalid_password",
    },
    TextConsumer {
        id: "pass_press_any_button",
        record_offset: 0x00a0,
        record_size: 0x1c,
        descriptor_pointer_offsets: &[0x0170, 0x017c],
        source_codes: &[
            0x0198, 0x0079, 0x0176, 0x0133, 0x0159, 0x00b0, 0x0228, 0x0083, 0x008a, 0x0203, 0x0082,
            0x0075,
        ],
        asset_id: "press_any_button",
    },
    TextConsumer {
        id: "pass_password_submit_instruction",
        record_offset: 0x0078,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x0158],
        source_codes: &[
            0x0130, 0x0133, 0x0071, 0x0137, 0x0103, 0xff57, 0x0195, 0x0212, 0x02b5,
        ],
        asset_id: "pass_password_submit_instruction",
    },
    TextConsumer {
        id: "pass_password_cancel_instruction",
        record_offset: 0x008c,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x0164],
        source_codes: &[
            0x0131, 0x0155, 0x0127, 0x0137, 0x0103, 0xff57, 0x0195, 0x0234, 0xff18,
        ],
        asset_id: "pass_password_cancel_instruction",
    },
    TextConsumer {
        id: "pass_cpu_health_gauge",
        record_offset: 0x03e8,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x0454],
        source_codes: &[0x0221, 0x0195, 0x0162, 0x0071, 0x0165],
        asset_id: "health_gauge",
    },
    TextConsumer {
        id: "pass_cpu_spirit_gauge",
        record_offset: 0x03f4,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x0460],
        source_codes: &[0x022a, 0x022b, 0x0162, 0x0071, 0x0165],
        asset_id: "spirit_gauge",
    },
    TextConsumer {
        id: "pass_cpu_spirit",
        record_offset: 0x0400,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x046c],
        source_codes: &[0x022a, 0x022b],
        asset_id: "spirit",
    },
    TextConsumer {
        id: "pass_cpu_attack_power",
        record_offset: 0x0408,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x0478],
        source_codes: &[0x0218, 0x0219, 0x0195],
        asset_id: "attack_power",
    },
    TextConsumer {
        id: "pass_cpu_defense_power",
        record_offset: 0x0410,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x0484],
        source_codes: &[0x02aa, 0x02ab, 0x0195],
        asset_id: "defense_power",
    },
    TextConsumer {
        id: "pass_cpu_skill_usage",
        record_offset: 0x03bc,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x0424],
        source_codes: &[0xff60, 0x0094, 0xff66, 0xff67, 0xff68],
        asset_id: "pass_cpu_skill_usage",
    },
    TextConsumer {
        id: "pass_cpu_password_display",
        record_offset: 0x03c8,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x0430],
        source_codes: &[0xff61, 0xff62, 0xff63, 0xff5a, 0xff5b],
        asset_id: "password_display",
    },
    TextConsumer {
        id: "pass_cpu_password_input",
        record_offset: 0x03d4,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x043c],
        source_codes: &[0xff61, 0xff62, 0xff63, 0xff57, 0xff64],
        asset_id: "pass_cpu_password_input",
    },
    TextConsumer {
        id: "pass_cpu_back",
        record_offset: 0x03e0,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x0448],
        source_codes: &[0x0223, 0x00a8],
        asset_id: "back",
    },
    TextConsumer {
        id: "pass_cpu_skill_usage_subject",
        record_offset: 0x04a4,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x0528],
        source_codes: &[
            0xff6b, 0x02b0, 0xff4b, 0xff60, 0x0094, 0xff66, 0xff67, 0xff68, 0x00b0,
        ],
        asset_id: "pass_cpu_skill_usage",
    },
    TextConsumer {
        id: "pass_cpu_skill_usage_action",
        record_offset: 0x04b8,
        record_size: 0x10,
        descriptor_pointer_offsets: &[0x0534],
        source_codes: &[0xff49, 0x01a9, 0x0103, 0x007a, 0x009a, 0x0084],
        asset_id: "configure_action",
    },
    TextConsumer {
        id: "pass_cpu_password_display_subject",
        record_offset: 0x04c8,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x0540],
        source_codes: &[
            0xff6b, 0x02b0, 0xff4b, 0xff60, 0x0094, 0xff66, 0xff67, 0xff68, 0x00b0,
        ],
        asset_id: "pass_cpu_skill_usage",
    },
    TextConsumer {
        id: "pass_cpu_password_display_action",
        record_offset: 0x04dc,
        record_size: 0x1c,
        descriptor_pointer_offsets: &[0x054c],
        source_codes: &[
            0x0177, 0x0130, 0x0157, 0x0071, 0x0171, 0x01a6, 0x0083, 0x008a, 0xff5a, 0xff5b, 0x0083,
            0x009a, 0x0084,
        ],
        asset_id: "password_display_action",
    },
    TextConsumer {
        id: "pass_cpu_password_input_subject",
        record_offset: 0x04f8,
        record_size: 0x14,
        descriptor_pointer_offsets: &[0x0558],
        source_codes: &[
            0xff6b, 0x02b0, 0xff4b, 0xff60, 0x0094, 0xff66, 0xff67, 0xff68, 0x00b0,
        ],
        asset_id: "pass_cpu_skill_usage",
    },
    TextConsumer {
        id: "pass_cpu_password_input_action",
        record_offset: 0x050c,
        record_size: 0x18,
        descriptor_pointer_offsets: &[0x0564],
        source_codes: &[
            0x0177, 0x0130, 0x0157, 0x0071, 0x0171, 0x0103, 0xff57, 0xff64, 0x0083, 0x009a, 0x0084,
        ],
        asset_id: "pass_cpu_password_input_action",
    },
    TextConsumer {
        id: "pass_cpu_rating_very_low",
        record_offset: 0x056c,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x05a0],
        source_codes: &[0x0084, 0x00b6, 0x007b, 0x02a9, 0x0075],
        asset_id: "status_rating_very_low",
    },
    TextConsumer {
        id: "pass_cpu_rating_low",
        record_offset: 0x0578,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x05ac],
        source_codes: &[0x02a9, 0x0075],
        asset_id: "status_rating_low",
    },
    TextConsumer {
        id: "pass_cpu_rating_normal",
        record_offset: 0x0580,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x05b8],
        source_codes: &[0x0097, 0x0089, 0x0076],
        asset_id: "status_rating_normal",
    },
    TextConsumer {
        id: "pass_cpu_rating_high",
        record_offset: 0x0588,
        record_size: 0x08,
        descriptor_pointer_offsets: &[0x05c4],
        source_codes: &[0x02a8, 0x0075],
        asset_id: "status_rating_high",
    },
    TextConsumer {
        id: "pass_cpu_rating_very_high",
        record_offset: 0x0590,
        record_size: 0x0c,
        descriptor_pointer_offsets: &[0x05d0, 0x05dc],
        source_codes: &[0x0084, 0x00b6, 0x007b, 0x02a8, 0x0075],
        asset_id: "status_rating_very_high",
    },
];

struct DirectGlyphConsumer {
    id: &'static str,
    asset_id: &'static str,
    record_offset: usize,
    record_size: usize,
    descriptor_pointer_offset: usize,
    source_codes: &'static [u16],
}

const DIRECT_GLYPH_CONSUMERS: [DirectGlyphConsumer; 2] = [
    DirectGlyphConsumer {
        id: "pass_cpu_male",
        asset_id: "pass_cpu_male",
        record_offset: 0x0418,
        record_size: 0x04,
        descriptor_pointer_offset: 0x0490,
        source_codes: &[0xff55],
    },
    DirectGlyphConsumer {
        id: "pass_cpu_female",
        asset_id: "pass_cpu_female",
        record_offset: 0x041c,
        record_size: 0x04,
        descriptor_pointer_offset: 0x049c,
        source_codes: &[0xff56],
    },
];

struct DirectGlyph {
    asset_id: &'static str,
    character_index: usize,
    code: u16,
    source_region_sha256: &'static str,
}

const DIRECT_GLYPHS: [DirectGlyph; 6] = [
    DirectGlyph {
        asset_id: "pass_cpu_male",
        character_index: 0,
        code: 0xff55,
        source_region_sha256: "92fcbdc8ddc1003086285b70212e17771175dd2bb42cdc9e7907782aaad02078",
    },
    DirectGlyph {
        asset_id: "pass_cpu_female",
        character_index: 0,
        code: 0xff56,
        source_region_sha256: "777010f07956013cb71927c1a698af103b3b57f508b8ccffcb7afbe206dfc1a5",
    },
    DirectGlyph {
        asset_id: "pass_cpu_skill_usage",
        character_index: 1,
        code: 0xff60,
        source_region_sha256: "10615b7dca49e0b6a61ffea472905d8f195652b70c6a801a6301a9e2418b5603",
    },
    DirectGlyph {
        asset_id: "pass_cpu_skill_usage",
        character_index: 2,
        code: 0xff66,
        source_region_sha256: "8fd1af945aa2dbb227bc1fa44d650c4daa9dc13feb018224655933fdb24ecd25",
    },
    DirectGlyph {
        asset_id: "pass_cpu_skill_usage",
        character_index: 3,
        code: 0xff67,
        source_region_sha256: "bd675904544d843a282c29f4920319d6a11dd21f67051345a8252d1175baf9af",
    },
    DirectGlyph {
        asset_id: "pass_cpu_skill_usage",
        character_index: 4,
        code: 0xff68,
        source_region_sha256: "84c9ecb133890fbc49a48ae51f71efb53b9457ca25b670db2535b825eaa3f495",
    },
];

pub(super) const PASSWORD_GRID_CODES: [u16; 80] = [
    0x120, 0x121, 0x122, 0x123, 0x124, 0x00a, 0x00b, 0x010, 0x011, 0x012, 0x125, 0x126, 0x127,
    0x128, 0x129, 0x013, 0x014, 0x015, 0x016, 0x017, 0x12a, 0x12b, 0x130, 0x131, 0x132, 0x018,
    0x019, 0x01a, 0x01b, 0x020, 0x133, 0x134, 0x135, 0x136, 0x137, 0x021, 0x022, 0x023, 0x024,
    0x025, 0x138, 0x139, 0x13a, 0x13b, 0x140, 0x026, 0x027, 0x028, 0x029, 0x02a, 0x141, 0x142,
    0x143, 0x144, 0x145, 0x02b, 0x000, 0x001, 0x002, 0x003, 0x146, 0x147, 0x148, 0x149, 0x14a,
    0x004, 0x005, 0x006, 0x007, 0x008, 0x152, 0x153, 0x154, 0x155, 0x156, 0x009, 0x069, 0x06a,
    0x1a8, 0x1a9,
];

const PRESERVED_RECORDS: &[(usize, &[u16])] = &[(
    0x0018,
    &[
        0x0084, 0x0084, 0x00a0, 0x0fff, 0x00a2, 0x0104, 0x00a8, 0x0fff, 0x02b7, 0x01b1, 0x0fff,
        0x00a3, 0x00a1, 0x00a8,
    ],
)];

const PRESERVED_DESCRIPTOR_BINDINGS: &[(usize, usize)] = &[(0x0134, 0x0018)];

const COMPLETE_EARLY_DESCRIPTOR_BINDINGS: &[(usize, usize)] = &[
    (0x0128, 0x000c),
    (0x0134, 0x0018),
    (0x0140, 0x0038),
    (0x014c, 0x0060),
    (0x0158, 0x0078),
    (0x0164, 0x008c),
    (0x0170, 0x00a0),
    (0x017c, 0x00a0),
    (0x0188, 0x00bc),
    (0x0194, 0x00d8),
    (0x01a0, 0x0100),
    (0x01ac, 0x0114),
];

#[derive(Debug, Serialize)]
pub(crate) struct PassFixedPresentationReport {
    pub(crate) adopted_spec_sha256: String,
    pub(crate) translated_consumer_count: usize,
    pub(crate) direct_glyph_count: usize,
    pub(crate) direct_consumer_record_count: usize,
    pub(crate) preserved_consumer_code_count: usize,
    pub(crate) descriptor_pointer_count: usize,
    pub(crate) source_bindings_verified: bool,
    pub(crate) changes_confined_to_owned_ranges: bool,
    pub(crate) password_alphabet: super::password_alphabet::PasswordAlphabetReport,
    pub(crate) password_display_uses_overlay_record_alias: bool,
    pub(crate) password_instructions_translated: bool,
    pub(crate) password_title_translated: bool,
    pub(crate) password_title_sampled_cells_protected: bool,
    pub(crate) password_title_preserved_codes: Vec<String>,
    pub(crate) password_decision_translated: bool,
    pub(crate) password_decision_renderer_sha256: String,
    pub(crate) password_decision_display_codes: Vec<String>,
    pub(crate) cpu_summary_uses_shared_spirit_unit: bool,
    pub(crate) dynamic_command_sheet_excluded: bool,
    pub(crate) current_rating_consumer_translated: bool,
    pub(crate) renderer_page_namespace_verified: bool,
    pub(crate) direct_page_preserved_consumers_disjoint: bool,
    pub(crate) direct_page_kanri_uploads_disjoint: bool,
    pub(crate) pass_changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) editmoji_changed_byte_ranges: Vec<[usize; 2]>,
}

pub(super) struct PassFixedPresentationBuild {
    pub(super) pass: Vec<u8>,
    pub(super) edit_shared_ui_candidate: Vec<u8>,
    pub(super) pass_claims: Vec<DecodedDataClaim>,
    pub(super) edit_shared_ui_claims: Vec<DecodedDataClaim>,
    pub(super) report: PassFixedPresentationReport,
}

pub(super) fn direct_glyph_characters(
    korean_text_by_asset_id: &BTreeMap<String, String>,
) -> Result<BTreeSet<char>> {
    let characters = DIRECT_GLYPHS
        .iter()
        .map(|glyph| resolve_direct_glyph_character(glyph, korean_text_by_asset_id))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        characters.len() == DIRECT_GLYPHS.len(),
        "PASS direct-glyph semantic assets do not resolve to unique characters"
    );
    Ok(characters)
}

pub(super) fn preserved_pass_consumer_codes(pass: &[u8]) -> Result<BTreeSet<u16>> {
    validate_pass_header(pass)?;
    validate_password_title(pass)?;
    validate_pass_renderer_page_namespace(pass)?;
    validate_fixed_text_consumers(pass)?;
    validate_direct_glyph_consumers(pass)?;
    validate_complete_early_descriptor_table(pass)?;
    let preserved_record_offsets = PRESERVED_RECORDS
        .iter()
        .map(|(offset, _)| *offset)
        .collect::<BTreeSet<_>>();
    let descriptor_record_offsets = PRESERVED_DESCRIPTOR_BINDINGS
        .iter()
        .map(|(_, record_offset)| *record_offset)
        .collect::<BTreeSet<_>>();
    ensure!(
        preserved_record_offsets.len() == PRESERVED_RECORDS.len()
            && descriptor_record_offsets.len() == PRESERVED_DESCRIPTOR_BINDINGS.len()
            && descriptor_record_offsets == preserved_record_offsets,
        "PASS preserved record and descriptor inventories diverged"
    );
    for (offset, expected) in PRESERVED_RECORDS {
        ensure!(
            read_length_prefixed_codes(pass, *offset)? == *expected,
            "PASS preserved fixed-presentation record at 0x{offset:04x} changed"
        );
    }
    for (pointer_offset, record_offset) in PRESERVED_DESCRIPTOR_BINDINGS {
        ensure!(
            read_u32(pass, *pointer_offset)? == PASS_RUNTIME_BASE + u32::try_from(*record_offset)?,
            "PASS preserved descriptor at 0x{pointer_offset:04x} no longer points to record 0x{record_offset:04x}"
        );
    }
    let password_grid_bytes = pass
        .get(
            PASSWORD_DISPLAY_CODE_TABLE_START
                ..PASSWORD_DISPLAY_CODE_TABLE_START + PASSWORD_GRID_CODES.len() * 2,
        )
        .context("PASS password grid-code table is truncated")?;
    let password_grid_codes = password_grid_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| u16::from_le_bytes(*bytes))
        .collect::<Vec<_>>();
    ensure!(
        password_grid_codes == PASSWORD_GRID_CODES,
        "PASS password grid alphabet changed"
    );
    ensure!(
        password_value_display_codes(pass)? == expected_password_value_display_codes(),
        "PASS saved-value display alphabet changed"
    );
    Ok(PRESERVED_RECORDS
        .iter()
        .flat_map(|(_, codes)| codes.iter().copied())
        .chain(password_title_codes()?)
        .chain(PASSWORD_GRID_CODES)
        .chain(super::password_alphabet::NATIVE_PASSWORD_SYMBOL_CODES)
        .chain(expected_password_value_display_codes())
        .filter(|code| *code < ALLOCATABLE_SOURCE_CODE_LIMIT && *code != TEXT_TERMINATOR)
        .collect())
}

pub(super) fn build_pass_fixed_presentation(
    source_pass: &[u8],
    source_edit_shared_ui: &[u8],
    authored_kanri_overlay: &[u8],
    font: &ShiftedSizedFontSource,
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    korean_text_by_asset_id: &BTreeMap<String, String>,
    password_alphabet: &super::password_alphabet::PasswordAlphabet,
) -> Result<PassFixedPresentationBuild> {
    let preserved_codes = preserved_pass_consumer_codes(source_pass)?;
    ensure!(
        KANRI_PASSWORD_DISPLAY_RECORD_OFFSET + KANRI_PASSWORD_DISPLAY_RECORD_SIZE
            <= authored_kanri_overlay.len()
            && OVERLAY_RUNTIME_BASE + u32::try_from(authored_kanri_overlay.len())?
                <= PASS_RUNTIME_BASE,
        "KANRI password-display alias target is outside its disjoint resident overlay"
    );
    let resolved_direct_glyphs = DIRECT_GLYPHS
        .iter()
        .map(|glyph| {
            Ok((
                resolve_direct_glyph_character(glyph, korean_text_by_asset_id)?,
                glyph,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let direct_codes = resolved_direct_glyphs
        .iter()
        .map(|(character, glyph)| (*character, glyph.code))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        direct_codes.len() == DIRECT_GLYPHS.len()
            && DIRECT_GLYPHS
                .iter()
                .map(|glyph| glyph.code)
                .collect::<BTreeSet<_>>()
                .len()
                == DIRECT_GLYPHS.len(),
        "PASS direct-glyph identities are not unique"
    );
    ensure!(
        DIRECT_GLYPHS.iter().all(|glyph| {
            glyph.code & PASS_RENDERER_CODE_MASK & PASS_DIRECT_PAGE_MASK == PASS_DIRECT_PAGE
        }),
        "PASS direct glyph escaped renderer page 15"
    );
    validate_direct_page_preserved_consumer_disjointness()?;
    for consumer in &DIRECT_GLYPH_CONSUMERS {
        let character = korean_text_by_asset_id
            .get(consumer.asset_id)
            .with_context(|| {
                format!(
                    "PASS direct consumer {} has no semantic asset {}",
                    consumer.id, consumer.asset_id
                )
            })?
            .chars()
            .next()
            .context("PASS direct-consumer semantic asset is empty")?;
        ensure!(
            korean_text_by_asset_id[consumer.asset_id].chars().count() == 1
                && consumer.source_codes.len() == 1
                && direct_codes.get(&character) == consumer.source_codes.first(),
            "PASS direct consumer {} does not bind its semantic glyph to its source code",
            consumer.id
        );
    }

    let mut referenced_direct_codes = DIRECT_GLYPH_CONSUMERS
        .iter()
        .flat_map(|consumer| consumer.source_codes.iter().copied())
        .filter(|code| direct_codes.values().any(|direct_code| direct_code == code))
        .collect::<BTreeSet<_>>();

    let mut pass = source_pass.to_vec();
    let mut pass_claims = Vec::new();
    for consumer in &FIXED_TEXT_CONSUMERS {
        let korean_text = korean_text_by_asset_id
            .get(consumer.asset_id)
            .with_context(|| {
                format!(
                    "PASS fixed text {} has no authored semantic asset {}",
                    consumer.id, consumer.asset_id
                )
            })?;
        let codes = korean_text
            .chars()
            .map(|character| {
                direct_codes
                    .get(&character)
                    .copied()
                    .or_else(|| common_menu_ascii_glyph_code(character))
                    .or_else(|| fixed_menu_glyph_code(character))
                    .or_else(|| allocation.get(&character).map(|glyph| glyph.code))
                    .with_context(|| format!("PASS fixed text has no glyph for {character:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        validate_choice_columns(consumer.id, &codes)?;
        if let Some((offset, layout)) =
            super::pass_button_layout::layout(source_pass, consumer.id, codes.len())?
        {
            pass[offset..offset + 2].copy_from_slice(&layout.x.to_le_bytes());
            let tracking = super::button_layout::SOURCE_GLYPH_ADVANCE - layout.glyph_advance_px;
            pass[offset + 8..offset + 10].copy_from_slice(&tracking.to_le_bytes());
            pass_claims.extend(DecodedDataClaim::from_effective_ranges(
                &format!("pass-button-layout:{}", consumer.id),
                "center the complete label within its native parent button",
                source_pass,
                &pass,
                [[offset, offset + 2], [offset + 8, offset + 10]],
            )?);
        }
        referenced_direct_codes.extend(
            codes
                .iter()
                .copied()
                .filter(|code| direct_codes.values().any(|direct_code| direct_code == code)),
        );
        if consumer.id == PASSWORD_DISPLAY_ALIAS_CONSUMER_ID {
            ensure!(
                2 + codes.len() * 2 <= KANRI_PASSWORD_DISPLAY_RECORD_SIZE,
                "aliased KANRI password-display text exceeds its target record"
            );
            ensure!(
                read_length_prefixed_codes(
                    authored_kanri_overlay,
                    KANRI_PASSWORD_DISPLAY_RECORD_OFFSET,
                )? == codes,
                "PASS password-display alias target does not contain the authored text"
            );
            for pointer_offset in consumer.descriptor_pointer_offsets {
                let target =
                    OVERLAY_RUNTIME_BASE + u32::try_from(KANRI_PASSWORD_DISPLAY_RECORD_OFFSET)?;
                pass.get_mut(*pointer_offset..*pointer_offset + 4)
                    .context("PASS password-display descriptor left the source image")?
                    .copy_from_slice(&target.to_le_bytes());
                pass_claims.extend(DecodedDataClaim::from_effective_ranges(
                    &format!("pass-fixed-presentation:{}", consumer.id),
                    "bind PASS password display to the authored KANRI overlay record",
                    source_pass,
                    &pass,
                    [[*pointer_offset, *pointer_offset + 4]],
                )?);
            }
            ensure!(
                pass[consumer.record_offset..consumer.record_offset + consumer.record_size]
                    == source_pass
                        [consumer.record_offset..consumer.record_offset + consumer.record_size],
                "PASS password-display alias changed sparse password display codes"
            );
            continue;
        }
        let record = pass
            .get_mut(consumer.record_offset..consumer.record_offset + consumer.record_size)
            .with_context(|| format!("PASS record {} left the source image", consumer.id))?;
        write_record(record, &codes)?;
        pass_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("pass-fixed-presentation:{}", consumer.id),
            &format!("translate PASS fixed presentation {}", consumer.id),
            source_pass,
            &pass,
            [[
                consumer.record_offset,
                consumer.record_offset + consumer.record_size,
            ]],
        )?);
    }
    ensure!(
        referenced_direct_codes == direct_codes.values().copied().collect(),
        "PASS direct glyphs are not all bound to output consumers"
    );
    ensure!(
        password_value_display_codes(&pass)? == expected_password_value_display_codes(),
        "PASS build changed the saved-value display alphabet"
    );

    let decision_codes = korean_text_by_asset_id
        .get("pass_password_decision")
        .context("PASS decision semantic asset is missing")?
        .chars()
        .map(|character| {
            allocation
                .get(&character)
                .map(|glyph| glyph.code)
                .with_context(|| {
                    format!("PASS decision lacks an allocated glyph for {character:?}")
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let decision_candidate = super::pass_password_decision::install(
        source_pass,
        authored_kanri_overlay,
        &decision_codes,
    )?;
    let start = super::pass_password_decision::START;
    let end = super::pass_password_decision::END;
    pass[start..end].copy_from_slice(&decision_candidate[start..end]);
    pass_claims.extend(DecodedDataClaim::from_effective_ranges(
        "pass-password-decision",
        "compose the separately machine-code-verified decision renderer",
        source_pass,
        &pass,
        [[start, end]],
    )?);

    let title_candidate = super::pass_title_renderer::install(
        source_pass,
        allocation,
        &korean_text_by_asset_id["pass_password_title"],
    )?;
    let title_start = super::pass_title_renderer::START;
    let title_end = super::pass_title_renderer::END;
    pass[title_start..title_end].copy_from_slice(&title_candidate[title_start..title_end]);
    pass_claims.extend(DecodedDataClaim::from_effective_ranges(
        "pass-password-title",
        "compose separately verified title program and GPU templates",
        source_pass,
        &pass,
        [[title_start, title_end]],
    )?);

    let summary = super::pass_cpu_summary::install(source_pass, authored_kanri_overlay)?;
    let offset = super::pass_cpu_summary::UNIT_OFFSET;
    pass[offset..offset + 4].copy_from_slice(&summary[offset..offset + 4]);
    pass_claims.extend(DecodedDataClaim::from_effective_ranges(
        "pass-cpu-summary-unit",
        "compose the verified shared Korean counter unit",
        source_pass,
        &pass,
        [[offset, offset + 4]],
    )?);

    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let direct_tim = parse_4bpp_without_clut_prefix(
        source_edit_shared_ui
            .get(EDITMOJI_TIM_OFFSET..)
            .context("PASS direct-glyph TIM offset left EDITMOJI")?,
    )?;
    ensure!(
        direct_tim.image_x == EDITMOJI_TIM_VRAM_WORD_X
            && direct_tim.image_word_width == EDITMOJI_TIM_VRAM_WORD_WIDTH
            && direct_tim.image_height == 256
            && usize::from(direct_tim.image_x) + direct_tim.image_word_width
                <= usize::try_from(KANRI_MENU_ATLAS_VRAM_WORD_X)?,
        "PASS direct-glyph TIM overlaps the KANRI menu-atlas upload range"
    );
    let mut edit_shared_ui_candidate = source_edit_shared_ui.to_vec();
    let mut allowed_ranges = Vec::new();
    for (character, glyph) in &resolved_direct_glyphs {
        let cell = direct_glyph_cell(glyph.code);
        let source_pixels = read_indexed_cell_without_clut_in_prefix(
            source_edit_shared_ui,
            EDITMOJI_TIM_OFFSET,
            cell,
        )?;
        ensure!(
            sha256_bytes(&source_pixels) == glyph.source_region_sha256,
            "PASS direct glyph {character:?} source cell changed"
        );
        let raster = rasterizer.rasterize_shifted(
            &character.to_string(),
            DIRECT_GLYPH_CELL_SIZE,
            DIRECT_GLYPH_CELL_SIZE,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            CLEAR_INDEX,
            Some(OUTLINE_INDEX),
            FILL_INDEX,
            HorizontalTextAlignment::Center,
        )?;
        allowed_ranges.extend(write_indexed_cell_without_clut_in_prefix(
            &mut edit_shared_ui_candidate,
            EDITMOJI_TIM_OFFSET,
            cell,
            &raster.pixels,
        )?);
    }
    let edit_shared_ui_claims = DecodedDataClaim::from_effective_ranges(
        "pass-fixed-presentation:direct-glyphs",
        "replace PASS-only fixed glyph cells in EDITMOJI TIM1",
        source_edit_shared_ui,
        &edit_shared_ui_candidate,
        allowed_ranges,
    )?;
    ensure!(
        !pass_claims.is_empty() && !edit_shared_ui_claims.is_empty(),
        "PASS fixed-presentation build changed no owned bytes"
    );
    let descriptor_pointer_count = FIXED_TEXT_CONSUMERS
        .iter()
        .map(|consumer| consumer.descriptor_pointer_offsets.len())
        .sum::<usize>()
        + DIRECT_GLYPH_CONSUMERS.len();
    let (password_alphabet_report, password_claims) = password_alphabet.install(&mut pass)?;
    pass_claims.extend(password_claims);

    let report = PassFixedPresentationReport {
        adopted_spec_sha256: adopted_spec_sha256(korean_text_by_asset_id)?,
        translated_consumer_count: FIXED_TEXT_CONSUMERS.len(),
        direct_glyph_count: DIRECT_GLYPHS.len(),
        direct_consumer_record_count: DIRECT_GLYPH_CONSUMERS.len(),
        preserved_consumer_code_count: preserved_codes.len(),
        descriptor_pointer_count,
        source_bindings_verified: true,
        changes_confined_to_owned_ranges: true,
        password_alphabet: password_alphabet_report,
        password_display_uses_overlay_record_alias: true,
        password_instructions_translated: true,
        password_title_translated: true,
        password_title_sampled_cells_protected: true,
        password_title_preserved_codes: password_title_codes()?
            .into_iter()
            .map(|code| format!("0x{code:04x}"))
            .collect(),
        password_decision_translated: true,
        password_decision_renderer_sha256: sha256_bytes(&pass[start..end]),
        password_decision_display_codes: decision_codes
            .iter()
            .map(|code| format!("0x{code:04x}"))
            .collect(),
        cpu_summary_uses_shared_spirit_unit: true,
        dynamic_command_sheet_excluded: true,
        current_rating_consumer_translated: true,
        renderer_page_namespace_verified: true,
        direct_page_preserved_consumers_disjoint: true,
        direct_page_kanri_uploads_disjoint: true,
        pass_changed_byte_ranges: difference_ranges(source_pass, &pass),
        editmoji_changed_byte_ranges: difference_ranges(
            source_edit_shared_ui,
            &edit_shared_ui_candidate,
        ),
    };
    Ok(PassFixedPresentationBuild {
        pass,
        edit_shared_ui_candidate,
        pass_claims,
        edit_shared_ui_claims,
        report,
    })
}

fn validate_pass_header(pass: &[u8]) -> Result<()> {
    ensure!(pass.len() == PASS_IMAGE_SIZE, "PASS image size changed");
    ensure!(
        read_u32(pass, 0x00)? == PASS_HEADER_WORDS[0]
            && read_u32(pass, 0x04)? == PASS_HEADER_WORDS[1]
            && read_u32(pass, 0x08)? == PASS_HEADER_WORDS[2],
        "PASS entrypoint header changed"
    );
    Ok(())
}

fn validate_pass_renderer_page_namespace(pass: &[u8]) -> Result<()> {
    for (offset, expected) in PASS_RENDERER_PAGE_INSTRUCTIONS {
        ensure!(
            read_u32(pass, offset)? == expected,
            "PASS fixed-text renderer page-selection grammar changed at 0x{offset:04x}"
        );
    }
    Ok(())
}

fn validate_direct_page_preserved_consumer_disjointness() -> Result<()> {
    ensure!(
        DIRECT_GLYPHS.iter().enumerate().all(|(index, left)| {
            DIRECT_GLYPHS[index + 1..].iter().all(|right| {
                !wrapped_cells_overlap_sized(
                    left.code & 0x00ff,
                    DIRECT_GLYPH_CELL_SIZE,
                    right.code & 0x00ff,
                    DIRECT_GLYPH_CELL_SIZE,
                )
            })
        }),
        "PASS direct page glyph cells overlap each other"
    );
    let preserved_direct_page_codes = PRESERVED_RECORDS
        .iter()
        .flat_map(|(_, codes)| codes.iter().copied())
        .chain(password_title_codes()?)
        .filter(|code| code & PASS_RENDERER_CODE_MASK & PASS_DIRECT_PAGE_MASK == PASS_DIRECT_PAGE)
        .map(|code| code & 0x00ff)
        .collect::<BTreeSet<_>>();
    for glyph in &DIRECT_GLYPHS {
        let local_code = glyph.code & 0x00ff;
        ensure!(
            preserved_direct_page_codes.iter().all(|preserved| {
                !wrapped_cells_overlap_sized(local_code, DIRECT_GLYPH_CELL_SIZE, *preserved, 20)
            }),
            "PASS direct glyph 0x{:04x} overlaps a preserved page-15 consumer",
            glyph.code
        );
    }
    Ok(())
}

fn validate_fixed_text_consumers(pass: &[u8]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut record_ranges = BTreeSet::new();
    let mut descriptor_pointer_offsets = BTreeSet::new();
    for consumer in &FIXED_TEXT_CONSUMERS {
        ensure!(
            ids.insert(consumer.id)
                && record_ranges.insert((
                    consumer.record_offset,
                    consumer.record_offset + consumer.record_size,
                )),
            "PASS fixed-presentation consumer identity is duplicated"
        );
        ensure!(
            read_length_prefixed_codes(pass, consumer.record_offset)? == consumer.source_codes,
            "PASS fixed-presentation source record {} changed",
            consumer.id
        );
        let logical_size = 2 + consumer.source_codes.len() * 2;
        ensure!(
            logical_size <= consumer.record_size
                && pass[consumer.record_offset + logical_size
                    ..consumer.record_offset + consumer.record_size]
                    .iter()
                    .all(|byte| *byte == 0),
            "PASS fixed-presentation record {} changed its owned zero padding",
            consumer.id
        );
        for pointer_offset in consumer.descriptor_pointer_offsets {
            ensure!(
                descriptor_pointer_offsets.insert(*pointer_offset)
                    && read_u32(pass, *pointer_offset)?
                        == PASS_RUNTIME_BASE + u32::try_from(consumer.record_offset)?,
                "PASS descriptor for {} no longer points to its source record",
                consumer.id
            );
        }
    }
    ensure!(
        record_ranges
            .iter()
            .zip(record_ranges.iter().skip(1))
            .all(|(left, right)| left.1 <= right.0),
        "PASS fixed-presentation source records overlap"
    );
    Ok(())
}

fn validate_direct_glyph_consumers(pass: &[u8]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut records = BTreeSet::new();
    let mut descriptors = BTreeSet::new();
    for consumer in &DIRECT_GLYPH_CONSUMERS {
        ensure!(
            ids.insert(consumer.id)
                && records.insert(consumer.record_offset)
                && descriptors.insert(consumer.descriptor_pointer_offset),
            "PASS direct-glyph consumer identity is duplicated"
        );
        ensure!(
            read_length_prefixed_codes(pass, consumer.record_offset)? == consumer.source_codes,
            "PASS direct-glyph source record {} changed",
            consumer.id
        );
        let logical_size = 2 + consumer.source_codes.len() * 2;
        ensure!(
            logical_size <= consumer.record_size
                && pass[consumer.record_offset + logical_size
                    ..consumer.record_offset + consumer.record_size]
                    .iter()
                    .all(|byte| *byte == 0),
            "PASS direct-glyph source record {} changed its owned zero padding",
            consumer.id
        );
        ensure!(
            read_u32(pass, consumer.descriptor_pointer_offset)?
                == PASS_RUNTIME_BASE + u32::try_from(consumer.record_offset)?,
            "PASS direct-glyph descriptor for {} changed",
            consumer.id
        );
    }
    Ok(())
}

fn validate_complete_early_descriptor_table(pass: &[u8]) -> Result<()> {
    let declared_bindings = PRESERVED_DESCRIPTOR_BINDINGS
        .iter()
        .copied()
        .chain(FIXED_TEXT_CONSUMERS.iter().flat_map(|consumer| {
            consumer
                .descriptor_pointer_offsets
                .iter()
                .map(|pointer_offset| (*pointer_offset, consumer.record_offset))
        }))
        .filter(|(pointer_offset, _)| (0x0128..=0x01ac).contains(pointer_offset))
        .collect::<BTreeSet<_>>();
    let complete_bindings = COMPLETE_EARLY_DESCRIPTOR_BINDINGS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        complete_bindings.len() == COMPLETE_EARLY_DESCRIPTOR_BINDINGS.len()
            && declared_bindings == complete_bindings,
        "PASS early descriptor declarations do not cover the complete bounded table"
    );
    for (pointer_offset, record_offset) in COMPLETE_EARLY_DESCRIPTOR_BINDINGS {
        ensure!(
            read_u32(pass, *pointer_offset)? == PASS_RUNTIME_BASE + u32::try_from(*record_offset)?,
            "PASS early descriptor at 0x{pointer_offset:04x} changed"
        );
    }
    Ok(())
}

fn adopted_spec_sha256(korean_text_by_asset_id: &BTreeMap<String, String>) -> Result<String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"Justice Gakuen 2 PASS fixed-presentation adopted spec\0");
    bytes.extend_from_slice(&PASS_RUNTIME_BASE.to_le_bytes());
    append_usize(&mut bytes, PASS_IMAGE_SIZE);
    for word in PASS_HEADER_WORDS {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for (offset, instruction) in PASS_RENDERER_PAGE_INSTRUCTIONS {
        append_usize(&mut bytes, offset);
        bytes.extend_from_slice(&instruction.to_le_bytes());
    }
    bytes.extend_from_slice(&PASS_RENDERER_CODE_MASK.to_le_bytes());
    bytes.extend_from_slice(&PASS_DIRECT_PAGE_MASK.to_le_bytes());
    bytes.extend_from_slice(&PASS_DIRECT_PAGE.to_le_bytes());
    append_usize(&mut bytes, EDITMOJI_TIM_OFFSET);
    bytes.extend_from_slice(&EDITMOJI_TIM_VRAM_WORD_X.to_le_bytes());
    append_usize(&mut bytes, EDITMOJI_TIM_VRAM_WORD_WIDTH);
    bytes.extend_from_slice(&KANRI_MENU_ATLAS_VRAM_WORD_X.to_le_bytes());
    append_usize(&mut bytes, PASSWORD_DISPLAY_CODE_TABLE_START);
    append_usize(&mut bytes, PASSWORD_VALUE_BYTE_TABLE_START);
    append_usize(&mut bytes, PASSWORD_VALUE_BYTE_TABLE_END);
    bytes.extend_from_slice(PASSWORD_DISPLAY_ALIAS_CONSUMER_ID.as_bytes());
    bytes.push(0);
    append_usize(&mut bytes, KANRI_PASSWORD_DISPLAY_RECORD_OFFSET);
    append_usize(&mut bytes, KANRI_PASSWORD_DISPLAY_RECORD_SIZE);
    append_usize(&mut bytes, DIRECT_GLYPH_CELL_SIZE);
    bytes.extend_from_slice(&[CLEAR_INDEX, OUTLINE_INDEX, FILL_INDEX]);
    bytes.extend_from_slice(&ALLOCATABLE_SOURCE_CODE_LIMIT.to_le_bytes());
    bytes.extend_from_slice(&TEXT_TERMINATOR.to_le_bytes());
    for consumer in &FIXED_TEXT_CONSUMERS {
        bytes.extend_from_slice(consumer.id.as_bytes());
        bytes.push(0);
        append_usize(&mut bytes, consumer.record_offset);
        append_usize(&mut bytes, consumer.record_size);
        for offset in consumer.descriptor_pointer_offsets {
            append_usize(&mut bytes, *offset);
        }
        bytes.push(0xff);
        for code in consumer.source_codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
        bytes.extend_from_slice(consumer.asset_id.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(
            korean_text_by_asset_id
                .get(consumer.asset_id)
                .expect("PASS semantic assets were validated before hashing")
                .as_bytes(),
        );
        bytes.push(0);
    }
    for glyph in &DIRECT_GLYPHS {
        bytes.extend_from_slice(glyph.asset_id.as_bytes());
        bytes.push(0);
        append_usize(&mut bytes, glyph.character_index);
        bytes.extend_from_slice(
            &u32::from(resolve_direct_glyph_character(
                glyph,
                korean_text_by_asset_id,
            )?)
            .to_le_bytes(),
        );
        bytes.extend_from_slice(&glyph.code.to_le_bytes());
        bytes.extend_from_slice(glyph.source_region_sha256.as_bytes());
    }
    for consumer in &DIRECT_GLYPH_CONSUMERS {
        bytes.extend_from_slice(consumer.id.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(consumer.asset_id.as_bytes());
        bytes.push(0);
        append_usize(&mut bytes, consumer.record_offset);
        append_usize(&mut bytes, consumer.record_size);
        append_usize(&mut bytes, consumer.descriptor_pointer_offset);
        for code in consumer.source_codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
    }
    for (record_offset, codes) in PRESERVED_RECORDS {
        append_usize(&mut bytes, *record_offset);
        for code in *codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
        bytes.push(0);
    }
    for (pointer_offset, record_offset) in PRESERVED_DESCRIPTOR_BINDINGS {
        append_usize(&mut bytes, *pointer_offset);
        append_usize(&mut bytes, *record_offset);
    }
    for (pointer_offset, record_offset) in COMPLETE_EARLY_DESCRIPTOR_BINDINGS {
        append_usize(&mut bytes, *pointer_offset);
        append_usize(&mut bytes, *record_offset);
    }
    for code in PASSWORD_GRID_CODES {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    for code in expected_password_value_display_codes() {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    Ok(sha256_bytes(&bytes))
}

fn resolve_direct_glyph_character(
    glyph: &DirectGlyph,
    korean_text_by_asset_id: &BTreeMap<String, String>,
) -> Result<char> {
    korean_text_by_asset_id
        .get(glyph.asset_id)
        .with_context(|| {
            format!(
                "PASS direct glyph 0x{:04x} has no semantic asset {}",
                glyph.code, glyph.asset_id
            )
        })?
        .chars()
        .nth(glyph.character_index)
        .with_context(|| {
            format!(
                "PASS semantic asset {} has no character at index {}",
                glyph.asset_id, glyph.character_index
            )
        })
}

fn direct_glyph_cell(code: u16) -> Cell {
    Cell {
        x: (usize::from(code & 0x000f) * DIRECT_GLYPH_CELL_SIZE) & 0xff,
        y: (usize::from((code >> 4) & 0x000f) * DIRECT_GLYPH_CELL_SIZE) & 0xff,
        width: DIRECT_GLYPH_CELL_SIZE,
        height: DIRECT_GLYPH_CELL_SIZE,
    }
}

fn append_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&(value as u64).to_le_bytes());
}

pub(super) fn password_value_display_codes(pass: &[u8]) -> Result<Vec<u16>> {
    let value_bytes = pass
        .get(PASSWORD_VALUE_BYTE_TABLE_START..PASSWORD_VALUE_BYTE_TABLE_END)
        .context("PASS password value-byte table is truncated")?;
    ensure!(
        value_bytes.len() == PASSWORD_GRID_CODES.len()
            && value_bytes[..76].iter().copied().eq(0u8..=0x4b)
            && value_bytes[76..] == [0x80, 0x81, 0x82, 0x83],
        "PASS password saved-value byte alphabet changed"
    );
    value_bytes
        .iter()
        .map(|value| {
            let offset = PASSWORD_DISPLAY_CODE_TABLE_START + usize::from(*value) * 2;
            Ok(u16::from_le_bytes(
                pass.get(offset..offset + 2)
                    .context("PASS password display-code lookup is truncated")?
                    .try_into()?,
            ))
        })
        .collect()
}

fn expected_password_value_display_codes() -> Vec<u16> {
    PASSWORD_GRID_CODES[..76]
        .iter()
        .copied()
        .chain([0xff62, 0xff63, 0xff5a, 0xff5b])
        .collect()
}

pub(super) fn validate_choice_columns(id: &str, codes: &[u16]) -> Result<()> {
    // Native cursors and the separate name renderer use fixed columns. Keep
    // spaces in the record rather than shrinking a translated choice row.
    let (width, labels): (usize, &[(usize, usize)]) = match id {
        "pass_registration_choices" => (5, &[(0, 2), (3, 2)]),
        "pass_character_acceptance_controls" => (18, &[(0, 2), (10, 2), (13, 2), (16, 2)]),
        "pass_register_another_choices" => (8, &[(0, 2), (5, 3)]),
        _ => return Ok(()),
    };
    ensure!(
        codes.len() == width,
        "PASS choice row {id} changed its native columns"
    );
    for (start, capacity) in labels {
        ensure!(
            codes[*start] != TEXT_TERMINATOR,
            "PASS choice row {id} has an empty label"
        );
        let mut padding = false;
        for code in &codes[*start..start + capacity] {
            if *code == TEXT_TERMINATOR {
                padding = true;
            } else {
                ensure!(
                    !padding,
                    "PASS choice row {id} splits a label across padding"
                );
            }
        }
    }
    for (column, code) in codes.iter().enumerate() {
        if !labels
            .iter()
            .any(|(start, capacity)| (*start..start + capacity).contains(&column))
        {
            ensure!(
                *code == TEXT_TERMINATOR,
                "PASS choice row {id} overlaps a cursor gap or name field"
            );
        }
    }
    Ok(())
}

fn write_record(record: &mut [u8], codes: &[u16]) -> Result<()> {
    ensure!(
        2 + codes.len() * 2 <= record.len(),
        "translated PASS fixed text exceeds its source record"
    );
    record.fill(0);
    record[..2].copy_from_slice(&u16::try_from(codes.len())?.to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let offset = 2 + index * 2;
        record[offset..offset + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("PASS u32 field is truncated")?
            .try_into()?,
    ))
}
