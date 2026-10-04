use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::model::{DiaryHeaderEntry, DiaryHeaderFontRole, DiaryHeaderTextLayout};
use crate::tim::Cell;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const DESCRIPTOR_OFFSET: usize = 0x1fa4;
// Native rows 4..12: glyph count, texture-page x/y in 64-word/row units,
// u/v, horizontal/vertical half extent, reserved byte.
const DESCRIPTORS: [[u8; 8]; 9] = [
    [6, 9, 4, 0, 96, 108, 12, 0],
    [9, 9, 4, 0, 0, 108, 12, 0],
    [8, 9, 4, 0, 120, 108, 12, 0],
    [5, 8, 4, 0, 216, 108, 12, 0],
    [2, 9, 4, 144, 24, 108, 12, 0],
    [2, 8, 4, 56, 176, 48, 12, 0],
    [2, 9, 4, 192, 96, 48, 12, 0],
    [2, 8, 4, 56, 152, 48, 12, 0],
    [2, 9, 4, 192, 24, 48, 12, 0],
];
const LABEL_IDS: [&str; 9] = [
    "diary-backup",
    "diary-message-speed",
    "diary-minigame-practice",
    "diary-exam-practice",
    "diary-exit",
    "message-speed-slow",
    "message-speed-normal",
    "message-speed-fast",
    "message-speed-instant",
];

// Native rows 13..18. A count of 0xff selects one 144x24 sprite;
// the 84-pixel half extent belongs to positioning, not texture width.
const MINIGAME_DESCRIPTORS: [[u8; 8]; 6] = [
    [255, 9, 4, 0, 168, 84, 12, 0],
    [255, 9, 4, 0, 192, 84, 12, 0],
    [3, 9, 4, 168, 48, 84, 12, 0],
    [6, 9, 4, 0, 24, 84, 12, 0],
    [7, 9, 4, 0, 48, 84, 12, 0],
    [7, 9, 4, 0, 72, 84, 12, 0],
];
const MINIGAME_LABEL_IDS: [Option<&str>; 6] = [
    Some("minigame-sprint"),
    None, // Doki-Waku-Dance retains its protected source lettering.
    Some("minigame-penalty-kicks"),
    Some("minigame-explosive-shot"),
    Some("minigame-service-ace"),
    Some("minigame-home-run"),
];

fn authored_label_rows() -> impl Iterator<Item = (&'static str, [u8; 8])> {
    LABEL_IDS.into_iter().zip(DESCRIPTORS).chain(
        MINIGAME_LABEL_IDS
            .into_iter()
            .zip(MINIGAME_DESCRIPTORS)
            .filter_map(|(id, row)| id.map(|id| (id, row))),
    )
}

fn consumed_cell(row: [u8; 8]) -> Cell {
    Cell {
        x: (usize::from(row[1]) * 64 - 512) * 4 + usize::from(row[3]),
        y: usize::from(row[2]) * 64 - 256 + usize::from(row[4]),
        width: if row[0] == 255 {
            144
        } else {
            usize::from(row[0]) * 24
        },
        height: 24,
    }
}

pub(super) fn validate_diary_menu_consumer(source: &[u8]) -> Result<()> {
    let table = source
        .get(
            DESCRIPTOR_OFFSET
                ..DESCRIPTOR_OFFSET + (DESCRIPTORS.len() + MINIGAME_DESCRIPTORS.len()) * 8,
        )
        .context("truncated MGAME Diary menu descriptors")?;
    ensure!(
        table.iter().copied().eq(DESCRIPTORS
            .into_iter()
            .chain(MINIGAME_DESCRIPTORS)
            .flatten()),
        "MGAME Diary menu source descriptors changed"
    );
    for (offset, expected) in renderer_instructions() {
        let bytes = source
            .get(offset..offset + 4)
            .context("truncated MGAME Diary menu renderer")?;
        let word = u32::from_le_bytes(bytes.try_into()?);
        let actual = decode(word, MGAME_RUNTIME_BASE + offset as u32)?;
        ensure!(
            actual == expected,
            "MGAME Diary menu renderer changed at +{offset:#x}"
        );
    }
    Ok(())
}

pub(super) fn validate_diary_menu_entries(entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (id, row) in authored_label_rows() {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("missing source-bound Diary menu label {id}"))?;
        let cell = consumed_cell(row);
        ensure!(
            entry.cell == cell && entry.font_role == DiaryHeaderFontRole::ActionLabel,
            "Diary menu label {id} differs from its consumed texture cell or font role"
        );
        // The MGAME spacing compositor makes each authored slice adjacent.
        ensure!(
            matches!(entry.layout, DiaryHeaderTextLayout::Continuous),
            "Diary menu label {id} must use continuous lettering"
        );
    }
    Ok(())
}

fn renderer_instructions() -> Vec<(usize, Instruction)> {
    use Instruction::*;
    vec![
        (
            0x18724,
            Lbu {
                rt: Register::V0,
                base: Register::S5,
                offset: 5,
            },
        ),
        (
            0x1872c,
            Sb {
                rt: Register::V0,
                base: Register::S2,
                offset: 0x2cc,
            },
        ),
        (
            0x18730,
            Lbu {
                rt: Register::V0,
                base: Register::S5,
                offset: 6,
            },
        ),
        (
            0x18738,
            Sb {
                rt: Register::V0,
                base: Register::S2,
                offset: 0x2cd,
            },
        ),
        (
            0x18704,
            Lbu {
                rt: Register::V0,
                base: Register::A0,
                offset: 3,
            },
        ),
        (
            0x18708,
            Lui {
                rt: Register::V1,
                immediate: 0x800a,
            },
        ),
        (
            0x1870c,
            Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x3f84,
            },
        ),
        (
            0x18710,
            Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            0x18714,
            Addu {
                rd: Register::S5,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x18718,
            Lbu {
                rt: Register::V0,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x18720,
            Sb {
                rt: Register::V0,
                base: Register::S2,
                offset: 0x2c8,
            },
        ),
        (
            0x18788,
            Lbu {
                rt: Register::FP,
                base: Register::S2,
                offset: 0x2c8,
            },
        ),
        (
            0x1878c,
            Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 255,
            },
        ),
        (
            0x18790,
            Bne {
                rs: Register::FP,
                rt: Register::V0,
                target: 0x800b_a79c,
            },
        ),
        (0x18794, Instruction::nop()),
        (
            0x18798,
            Addiu {
                rt: Register::FP,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x18858,
            Lbu {
                rt: Register::V1,
                base: Register::S2,
                offset: 0x2c8,
            },
        ),
        (
            0x1885c,
            Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 255,
            },
        ),
        (
            0x18860,
            Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: 0x800b_a874,
            },
        ),
        (
            0x18868,
            Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 144,
            },
        ),
        (
            0x1886c,
            J {
                target: 0x800b_a87c,
            },
        ),
        (
            0x18870,
            Sh {
                rt: Register::V0,
                base: Register::S0,
                offset: 0x18,
            },
        ),
        (
            0x187bc,
            Lbu {
                rt: Register::A2,
                base: Register::S5,
                offset: 1,
            },
        ),
        (
            0x187c0,
            Lbu {
                rt: Register::A3,
                base: Register::S5,
                offset: 2,
            },
        ),
        (
            0x187cc,
            Sll {
                rd: Register::A2,
                rt: Register::A2,
                shift: 6,
            },
        ),
        (
            0x187dc,
            Sll {
                rd: Register::A3,
                rt: Register::A3,
                shift: 6,
            },
        ),
        (
            0x1883c,
            Lbu {
                rt: Register::V0,
                base: Register::S5,
                offset: 3,
            },
        ),
        (
            0x18844,
            Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::S7,
            },
        ),
        (
            0x18848,
            Sb {
                rt: Register::V0,
                base: Register::S0,
                offset: 0x14,
            },
        ),
        (
            0x1884c,
            Lbu {
                rt: Register::V0,
                base: Register::S5,
                offset: 4,
            },
        ),
        (
            0x18854,
            Sb {
                rt: Register::V0,
                base: Register::S0,
                offset: 0x15,
            },
        ),
        (
            0x18864,
            Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 24,
            },
        ),
        (
            0x18874,
            Sh {
                rt: Register::T0,
                base: Register::S0,
                offset: 0x18,
            },
        ),
        (
            0x18878,
            Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 24,
            },
        ),
        (
            0x1887c,
            Sh {
                rt: Register::T0,
                base: Register::S1,
                offset: 0x12,
            },
        ),
        (
            0x188cc,
            Addiu {
                rt: Register::S7,
                rs: Register::S7,
                immediate: 24,
            },
        ),
        (
            0x188d0,
            Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: 1,
            },
        ),
        (
            0x188d4,
            Slt {
                rd: Register::V0,
                rs: Register::S4,
                rt: Register::FP,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<DiaryHeaderEntry> {
        authored_label_rows()
            .map(|(id, row)| DiaryHeaderEntry {
                id: id.into(),
                source_text: String::new(),
                korean_text: "한글".into(),
                font_role: DiaryHeaderFontRole::ActionLabel,
                cell: consumed_cell(row),
                layout: DiaryHeaderTextLayout::Continuous,
                font_px: None,
                vertical_shift_px: None,
                indexed_art: None,
                source_region_sha256: String::new(),
            })
            .collect()
    }

    #[test]
    fn menu_labels_reject_inconsistent_glyph_spacing() {
        let baseline = entries();
        validate_diary_menu_entries(&baseline).unwrap();
        for index in 0..baseline.len() {
            let mut changed = entries();
            changed[index].layout = DiaryHeaderTextLayout::GlyphSlots { slots: vec![0, 1] };
            assert!(validate_diary_menu_entries(&changed).is_err());
        }
    }

    #[test]
    fn whole_sprite_label_accepts_continuous_text_with_native_texture_width() {
        let mut entries = entries();
        let sprint = entries
            .iter_mut()
            .find(|entry| entry.id == "minigame-sprint")
            .unwrap();
        sprint.layout = DiaryHeaderTextLayout::Continuous;
        validate_diary_menu_entries(&entries).unwrap();
        // The panel's 168-pixel extent includes pixels outside the 144-pixel texture.
        entries
            .iter_mut()
            .find(|entry| entry.id == "minigame-sprint")
            .unwrap()
            .cell
            .width = 168;
        assert!(validate_diary_menu_entries(&entries).is_err());
    }

    #[test]
    fn menu_labels_cannot_move_out_of_their_native_cells() {
        let mut entries = entries();
        entries[2].cell.x += 24;
        assert!(validate_diary_menu_entries(&entries).is_err());
    }

    #[test]
    fn menu_family_cannot_silently_omit_a_label() {
        let mut entries = entries();
        entries.pop();
        assert!(validate_diary_menu_entries(&entries).is_err());
    }
}
