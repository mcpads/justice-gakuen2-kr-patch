use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::name_input::NameDialogueRuntimeProgramReport;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const NICKNAME_HUD_LOOKUP_TABLE_ADDRESS: u32 = 0x800a_2dd0;
const NICKNAME_CACHE_CODES: [u16; 4] = [0x0339, 0x033a, 0x033b, 0x033c];
const NICKNAME_HUD_GLYPH_INDICES: [u16; 4] = [0x0203, 0x0204, 0x0205, 0x0207];
const NICKNAME_HUD_GLYPH_CELL_ADDRESSES: [u32; 4] =
    [0x8016_63a0, 0x8016_6468, 0x8016_6530, 0x8016_66c0];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NicknameHudLookupReport {
    pub table_address: String,
    pub cache_codes: [String; 4],
    pub glyph_indices: [String; 4],
    pub reused_scene_cell_addresses: [String; 4],
    pub source_entries_verified: bool,
}

pub(super) fn verify_nickname_hud_lookup(
    source: &[u8],
    program: &NameDialogueRuntimeProgramReport,
) -> Result<NicknameHudLookupReport> {
    for (code, expected_index) in NICKNAME_CACHE_CODES
        .into_iter()
        .zip(NICKNAME_HUD_GLYPH_INDICES)
    {
        let entry_address = NICKNAME_HUD_LOOKUP_TABLE_ADDRESS
            .checked_add(u32::from(code) * 2)
            .context("nickname HUD lookup address overflow")?;
        let offset = usize::try_from(
            entry_address
                .checked_sub(MGAME_RUNTIME_BASE)
                .context("nickname HUD lookup precedes MGAME runtime")?,
        )?;
        let bytes = source
            .get(offset..offset + 2)
            .context("nickname HUD lookup entry is truncated")?;
        let actual_index = u16::from_le_bytes(bytes.try_into()?);
        ensure!(
            actual_index == expected_index,
            "nickname HUD lookup for code 0x{code:04x} changed: expected 0x{expected_index:04x}, got 0x{actual_index:04x}"
        );
    }

    let expected_cell_addresses =
        NICKNAME_HUD_GLYPH_CELL_ADDRESSES.map(|address| format!("0x{address:08x}"));
    ensure!(
        program.nickname_hud_reused_scene_cell_addresses == expected_cell_addresses,
        "shared nickname runtime maps to different reused scene cells"
    );

    Ok(NicknameHudLookupReport {
        table_address: format!("0x{NICKNAME_HUD_LOOKUP_TABLE_ADDRESS:08x}"),
        cache_codes: NICKNAME_CACHE_CODES.map(|code| format!("0x{code:04x}")),
        glyph_indices: NICKNAME_HUD_GLYPH_INDICES.map(|index| format!("0x{index:04x}")),
        reused_scene_cell_addresses: expected_cell_addresses,
        source_entries_verified: true,
    })
}
