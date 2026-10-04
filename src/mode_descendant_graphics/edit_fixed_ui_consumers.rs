//! Exact consumers for fixed EDIT UI cells that are not explained by the
//! record-level texture loader alone.

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::Cell;

use super::model::{ModeDescendantEntry, ModeDescendantPlacement, ModeDescendantSurface};

const CPU_TACTICS_INSTRUCTION_TITLE_ID: &str = "edit_cpu_tactics_instruction_title";
const CPU_TACTICS_INSTRUCTION_TITLE_SOURCE_TEXT: &str = "熱血戦術指導";
const CPU_TACTICS_INSTRUCTION_TITLE_TIM_OFFSET: &str = "0x21000";
const CPU_TACTICS_INSTRUCTION_TITLE_CELL: Cell = Cell {
    x: 0,
    y: 140,
    width: 104,
    height: 20,
};
const PASS_PATH: &str = "DAT1/PASS.BIN";
const PASS_SIZE: usize = 32_692;
const PASS_SHA256: &str = "f160d227922e5928eda71a1d332fa6c463a4ba57826d35ca72d3c6e0581b90d4";
const PASS_RUNTIME_BASE: u32 = 0x8017_a000;
const TITLE_DESCRIPTOR: [u8; 22] = [
    0x00, 0x00, 0xc0, 0x02, 0x00, 0x00, 0xf0, 0x00, 0xe1, 0x01, 0x00, 0x00, 0x8c, 0x00, 0x68, 0x00,
    0x14, 0x00, 0x18, 0x00, 0x3c, 0x00,
];
const TITLE_DESCRIPTOR_OFFSETS: [usize; 2] = [0x072a, 0x07c6];
const TITLE_TABLE_BINDINGS: [(usize, u16, usize, u32); 2] = [
    (0x0728, 7, 0x084c, PASS_RUNTIME_BASE + 0x0728),
    (0x07c4, 6, 0x0850, PASS_RUNTIME_BASE + 0x07c4),
];
const TITLE_CONSUMER_WORDS: [(usize, u32); 15] = [
    (0x5bf8, 0x2404_0001),
    (0x5bfc, 0x0c06_0282),
    (0x5c00, 0x0000_0000),
    (0x5c6c, 0x0c06_0282),
    (0x5c70, 0x0000_2021),
    (0x6a08, 0x27bd_ffe8),
    (0x6a0c, 0x0004_1080),
    (0x6a10, 0x3c04_801d),
    (0x6a14, 0x3484_2f4c),
    (0x6a18, 0xafbf_0010),
    (0x6a1c, 0x3c01_8018),
    (0x6a20, 0x0022_0821),
    (0x6a24, 0x8c25_a84c),
    (0x6a28, 0x0c06_01a9),
    (0x6a2c, 0x2406_0422),
];

pub(super) fn validate_edit_cpu_tactics_instruction_title_consumers(
    source: &SupportedSourceDisc,
    entries: &[ModeDescendantEntry],
) -> Result<usize> {
    let entry = entries
        .iter()
        .find(|entry| entry.id == CPU_TACTICS_INSTRUCTION_TITLE_ID)
        .context("EDIT fixed UI lost the CPU tactics-instruction title")?;
    ensure!(
        entries
            .iter()
            .filter(|candidate| candidate.id == CPU_TACTICS_INSTRUCTION_TITLE_ID)
            .count()
            == 1,
        "EDIT fixed UI has duplicate CPU tactics-instruction titles"
    );
    ensure!(
        entry.surface == ModeDescendantSurface::EditSharedUi
            && entry.source_text == CPU_TACTICS_INSTRUCTION_TITLE_SOURCE_TEXT
            && matches!(
                &entry.placement,
                ModeDescendantPlacement::Fixed4bppWithoutClut {
                    tim_offset,
                    cell,
                    ..
                } if tim_offset == CPU_TACTICS_INSTRUCTION_TITLE_TIM_OFFSET
                    && *cell == CPU_TACTICS_INSTRUCTION_TITLE_CELL
            ),
        "EDIT CPU tactics-instruction title left its source-bound fixed cell"
    );

    let (_, pass) = source.read_record(PASS_PATH)?;
    ensure!(
        pass.len() == PASS_SIZE && sha256_bytes(&pass) == PASS_SHA256,
        "unsupported {PASS_PATH} identity for the EDIT fixed-UI consumer contract"
    );
    for descriptor_offset in TITLE_DESCRIPTOR_OFFSETS {
        ensure!(
            pass.get(descriptor_offset..descriptor_offset + TITLE_DESCRIPTOR.len())
                == Some(TITLE_DESCRIPTOR.as_slice()),
            "EDIT CPU tactics-instruction title descriptor at 0x{descriptor_offset:04x} changed"
        );
    }
    for (count_offset, count, pointer_offset, pointer) in TITLE_TABLE_BINDINGS {
        ensure!(
            read_u16(&pass, count_offset)? == count && read_u32(&pass, pointer_offset)? == pointer,
            "EDIT CPU tactics-instruction title table at 0x{count_offset:04x} changed"
        );
    }
    for (instruction_offset, instruction) in TITLE_CONSUMER_WORDS {
        ensure!(
            read_u32(&pass, instruction_offset)? == instruction,
            "EDIT CPU tactics-instruction title consumer changed at 0x{instruction_offset:04x}"
        );
    }
    Ok(TITLE_DESCRIPTOR_OFFSETS.len())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .context("EDIT fixed-UI u16 field is truncated")?
            .try_into()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("EDIT fixed-UI u32 field is truncated")?
            .try_into()?,
    ))
}

/// The distance renderer consumes four ten-byte texture selectors. Frequency
/// sprites consume one of four eight-byte selectors selected by two tactics bits.
/// Bind the writable text interiors, leaving the tabs' sloping edges untouched.
pub(super) fn validate_edit_cpu_tactics_body_consumers(
    source: &SupportedSourceDisc,
    entries: &[ModeDescendantEntry],
) -> Result<usize> {
    let (_, pass) = source.read_record(PASS_PATH)?;
    ensure!(
        pass.len() == PASS_SIZE && sha256_bytes(&pass) == PASS_SHA256,
        "unsupported PASS source for CPU tactics body"
    );
    for (start, end, digest) in [
        (
            0x664,
            0x6bc,
            "018246f526bb473e7fb5974a0dbe762c25cb442824b179c4a8d31d5be9f74221",
        ),
        (
            0x4674,
            0x4850,
            "20198e278b37e6abcd49c21cb9a7e938983f86c42f6c26469f4fa845f4d9ccc9",
        ),
        (
            0x4d40,
            0x507c,
            "fd58d664362a30e33b0d9f4f1b21f9b845273befd0e94f5487940c0de6064d95",
        ),
    ] {
        ensure!(
            sha256_bytes(&pass[start..end]) == digest,
            "CPU tactics texture selection changed at PASS +0x{start:x}"
        );
    }
    let tabs = [
        ("edit_cpu_distance_far", "遠距離", 771, 171, 17),
        ("edit_cpu_distance_middle", "中距離", 611, 235, 17),
        ("edit_cpu_distance_close", "近距離", 867, 171, 17),
        ("edit_cpu_distance_very_close", "超近距離", 515, 235, 17),
    ];
    for (index, (id, text, x, y, height)) in tabs.into_iter().enumerate() {
        let at = 0x664 + index * 10;
        let page_x = usize::from(read_u16(&pass, at)?);
        let texture_x = (page_x - 640) * 4 + usize::from(read_u16(&pass, at + 4)?);
        let texture_y = usize::from(read_u16(&pass, at + 6)?);
        let cell = Cell {
            x,
            y,
            width: 68,
            height,
        };
        ensure!(
            x == texture_x + 3 && y >= texture_y && y + height == texture_y + 20,
            "CPU distance text interior left its native sprite"
        );
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("missing CPU distance label {id}"))?;
        ensure!(
            entry.surface == ModeDescendantSurface::EditSharedUi
                && entry.source_text == text
                && matches!(&entry.placement, ModeDescendantPlacement::Fixed {
                bits_per_pixel: 4, tim_offset, cell: actual, palette_roles: Some(roles), ..
            } if tim_offset == "0x0" && *actual == cell
                && roles.clear_index == [15, 9, 12, 10][index]
                && roles.outline_index.is_none() && roles.fill_index == 1),
            "CPU distance label {id} left its source-bound cell or palette roles"
        );
    }
    let frequencies = [
        ("edit_cpu_frequency_low", "あまり使わない"),
        ("edit_cpu_frequency_normal", "普通"),
        ("edit_cpu_frequency_high", "よく使う"),
        ("edit_cpu_frequency_rare", "ほとんど使わない"),
    ];
    for (index, (id, text)) in frequencies.into_iter().enumerate() {
        let at = 0x69c + index * 8;
        let cell = Cell {
            x: usize::from(read_u16(&pass, at)?),
            y: usize::from(read_u16(&pass, at + 2)?),
            width: usize::from(read_u16(&pass, at + 4)?),
            height: 20,
        };
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("missing CPU usage frequency {id}"))?;
        ensure!(
            entry.surface == ModeDescendantSurface::EditSharedUi
                && entry.source_text == text
                && matches!(&entry.placement, ModeDescendantPlacement::Fixed4bppWithoutClut {
                tim_offset, cell: actual, clear_index: 0, outline_index: None, fill_index: 2, ..
            } if tim_offset == "0x21000" && *actual == cell),
            "CPU usage frequency {id} left its source-bound cell or palette roles"
        );
    }
    Ok(tabs.len() + frequencies.len())
}
