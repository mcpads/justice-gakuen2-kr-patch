use psx_r3000a::{Instruction, Register, encode};

use super::consumer::rewrite_character_select_overlay;
use super::model::{
    CharacterSelectConsumerPlan, CharacterSelectFontRole, CharacterSelectGlyphAllocation,
    CharacterSelectLocalizedSource, CharacterSelectSourceCellOccupancy,
    CharacterSelectTextureSurface,
};
use crate::tim::Cell;

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;

#[test]
fn plsel1_rewrite_uses_one_plan_for_descriptors_and_texture_page() {
    let mut source = vec![0_u8; 24_020];
    source[0x056c..0x0581].copy_from_slice(&[
        0x0a, 0x01, 0x02, 0x02, 0x02, 0x03, 0x02, 0x04, 0x02, 0x05, 0x02, 0x02, 0x01, 0x06, 0x02,
        0x07, 0x02, 0x04, 0x02, 0x00, 0x03,
    ]);
    source[0x0584..0x058b].copy_from_slice(&[0x03, 0x05, 0x00, 0x00, 0x00, 0x04, 0x00]);
    source[0x058c..0x0592].copy_from_slice(&[0x08, 0x0a, 0x09, 0x0a, 0x03, 0x05]);
    source[0x0594..0x05a3].copy_from_slice(&[
        0x07, 0x00, 0x04, 0x04, 0x03, 0x01, 0x04, 0x02, 0x04, 0x03, 0x04, 0x04, 0x04, 0x05, 0x04,
    ]);
    write_instruction(
        &mut source,
        0x1ee0,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x256c,
        },
    );
    write_instruction(
        &mut source,
        0x1ef4,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x2584,
        },
    );
    write_instruction(
        &mut source,
        0x2df4,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x258c,
        },
    );
    write_instruction(
        &mut source,
        0x35dc,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x2594,
        },
    );
    write_instruction(
        &mut source,
        0x1f60,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x0340,
        },
    );
    let entries = vec![
        dynamic_entry("character_select_heading", "캐릭터 선택"),
        dynamic_entry("versus_heading", "대전"),
        entry("win_count_label", "승리 수"),
        entry("versus_handicap_label", "핸디캡"),
    ];
    let mut allocations = "캐릭터 선택대전"
        .chars()
        .enumerate()
        .map(|(index, character)| allocation(character, index))
        .collect::<Vec<_>>();
    for (character, cell) in [
        (
            '승',
            Cell {
                x: 160,
                y: 200,
                width: 20,
                height: 20,
            },
        ),
        (
            '리',
            Cell {
                x: 180,
                y: 200,
                width: 20,
                height: 20,
            },
        ),
        (
            '수',
            Cell {
                x: 60,
                y: 100,
                width: 20,
                height: 20,
            },
        ),
    ] {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface: CharacterSelectTextureSurface::VersusLabelAtlas,
            tim_offset: 0x17800,
            texture_page_index: 0,
            texture_uv: [cell.x as u8, cell.y as u8],
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }
    allocations.push(CharacterSelectGlyphAllocation {
        font_role: CharacterSelectFontRole::Label,
        character: '승',
        surface: CharacterSelectTextureSurface::ParticipantLabelAtlas,
        tim_offset: 0x17800,
        texture_page_index: 0,
        texture_uv: [120, 100],
        cell: Cell {
            x: 120,
            y: 100,
            width: 20,
            height: 20,
        },
        source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
    });
    for (index, character) in "핸디캡".chars().enumerate() {
        let x = index * 20;
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface: CharacterSelectTextureSurface::VersusHandicapAtlas,
            tim_offset: 0x17800,
            texture_page_index: 0,
            texture_uv: [x as u8, 80],
            cell: Cell {
                x,
                y: 80,
                width: 20,
                height: 20,
            },
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }

    let without_blank = allocations
        .iter()
        .filter(|glyph| glyph.character != ' ')
        .cloned()
        .collect::<Vec<_>>();
    let error = rewrite_character_select_overlay(
        "DAT1/PLSEL1.BIN",
        &source,
        &without_blank,
        &consumer_plan(),
        &entries,
    )
    .unwrap_err();
    assert!(error.to_string().contains("has no allocation for ' '"));
    let mut wrong_page = allocations.clone();
    wrong_page
        .iter_mut()
        .find(|glyph| glyph.character == ' ')
        .unwrap()
        .texture_page_index = 1;
    let error = rewrite_character_select_overlay(
        "DAT1/PLSEL1.BIN",
        &source,
        &wrong_page,
        &consumer_plan(),
        &entries,
    )
    .unwrap_err();
    assert!(error.to_string().contains("spans texture pages"));

    let (patched, report) = rewrite_character_select_overlay(
        "DAT1/PLSEL1.BIN",
        &source,
        &allocations,
        &consumer_plan(),
        &entries,
    )
    .unwrap();

    assert_eq!(patched[0x056c], 6);
    // The space is the fourth allocated glyph, not an assumed blank at UV 0,0.
    let blank = allocations
        .iter()
        .find(|glyph| glyph.character == ' ')
        .unwrap();
    assert_ne!(blank.texture_uv, [0, 0]);
    assert_eq!(
        &patched[0x0573..0x0575],
        &[blank.texture_uv[0] / 32, blank.texture_uv[1] / 32]
    );
    assert!(patched[0x0579..0x0581].iter().all(|byte| *byte == 0));
    assert_eq!(patched[0x0584], 2);
    assert!(patched[0x0589..0x058b].iter().all(|byte| *byte == 0));
    assert_eq!(
        read_instruction(&patched, 0x1f60),
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x0380,
        }
    );
    assert_eq!(&patched[0x058c..0x0592], &[8, 10, 9, 10, 3, 5]);
    assert_eq!(
        &patched[0x0594..0x05a3],
        &[3, 0, 4, 1, 4, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(report.descriptors.len(), 4);
    assert!(report.changed_byte_ranges.iter().all(|[start, end]| {
        report
            .expected_write_ranges
            .iter()
            .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)
    }));
}

#[test]
fn plsel1_rewrite_rejects_descriptor_drift_before_writing() {
    let mut source = vec![0_u8; 24_020];
    source[0x056c] = 0xff;
    write_instruction(
        &mut source,
        0x1ee0,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x256c,
        },
    );
    let error = rewrite_character_select_overlay(
        "DAT1/PLSEL1.BIN",
        &source,
        &[],
        &consumer_plan(),
        &[dynamic_entry("character_select_heading", "캐릭터 선택")],
    )
    .unwrap_err();

    assert!(error.to_string().contains("source descriptor changed"));
}

#[test]
fn cooperative_runtime_cache_descriptors_remain_source_owned_until_producer_is_bound() {
    let mut source = vec![0_u8; 50_940];
    source[0x3618..0x3624].copy_from_slice(&[
        0x00, 0x80, 0xa8, 0x20, 0x00, 0xa0, 0x7a, 0x20, 0x00, 0x60, 0xee, 0x20,
    ]);
    source[0x3704..0x370b].copy_from_slice(&[0x03, 0xa8, 0x07, 0xc8, 0x07, 0x80, 0x00]);
    source[0x370c..0x3713].copy_from_slice(&[0x03, 0xa0, 0x00, 0x00, 0x00, 0x80, 0x00]);
    source[0x3714..0x371a].copy_from_slice(&[0x90, 0x00, 0xb8, 0x00, 0x90, 0x28]);
    source[0x371c..0x3722].copy_from_slice(&[0x08, 0x0a, 0x09, 0x0a, 0x03, 0x05]);
    for (offset, instruction) in [
        (
            0x8168,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x5704,
            },
        ),
        (
            0x81a0,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x570c,
            },
        ),
        (
            0x821c,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0340,
            },
        ),
        (
            0x83c0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0300,
            },
        ),
        (
            0x91b8,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x571c,
            },
        ),
    ] {
        write_instruction(&mut source, offset, instruction);
    }
    let entries = vec![
        dynamic_entry("cooperative_heading", "협력전"),
        dynamic_entry("versus_heading", "대전"),
        entry("win_count_label", "승리 수"),
        entry("cooperative_mode_menu_heading", "협력전"),
    ];
    let mut allocations = "협력전대"
        .chars()
        .enumerate()
        .map(|(index, character)| allocation(character, index))
        .collect::<Vec<_>>();
    for (character, cell) in [
        (
            '승',
            Cell {
                x: 160,
                y: 200,
                width: 20,
                height: 20,
            },
        ),
        (
            '리',
            Cell {
                x: 180,
                y: 200,
                width: 20,
                height: 20,
            },
        ),
        (
            '수',
            Cell {
                x: 60,
                y: 100,
                width: 20,
                height: 20,
            },
        ),
    ] {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface: CharacterSelectTextureSurface::VersusLabelAtlas,
            tim_offset: 0x17800,
            texture_page_index: 0,
            texture_uv: [cell.x as u8, cell.y as u8],
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }

    let (patched, report) = rewrite_character_select_overlay(
        "DAT1/PLSEL5.BIN",
        &source,
        &allocations,
        &consumer_plan(),
        &entries,
    )
    .unwrap();

    assert_eq!(&patched[0x3618..0x3624], &source[0x3618..0x3624]);
    assert_eq!(&patched[0x3714..0x371a], &source[0x3714..0x371a]);
    assert_eq!(&patched[0x371c..0x3722], &[8, 10, 9, 10, 3, 5]);
    assert_eq!(
        read_instruction(&patched, 0x83c0),
        read_instruction(&source, 0x83c0)
    );
    assert!(
        report
            .descriptors
            .iter()
            .all(|descriptor| { descriptor.translation_id != "cooperative_mode_menu_heading" })
    );
    assert!(report.expected_write_ranges.iter().all(|[start, end]| {
        !(*start < 0x3624 && 0x3618 < *end)
            && !(*start < 0x371a && 0x3714 < *end)
            && !(*start <= 0x83c0 && 0x83c0 < *end)
    }));
}

#[test]
fn selection_help_uses_its_own_page_and_fills_both_native_consumers() {
    let mut source = vec![0_u8; 24_020];
    source[0x0474..0x0494].copy_from_slice(&[
        0x0d, 0x01, 0x11, 0x00, 0x06, 0x01, 0x11, 0x00, 0x0f, 0x00, 0x0e, 0x01, 0x0c, 0x07, 0x04,
        0x05, 0x0d, 0x06, 0x00, 0x04, 0x08, 0x04, 0x00, 0x04, 0x0e, 0x00, 0x0d, 0x00, 0x0f, 0x00,
        0x05, 0x01,
    ]);
    source[0x0494..0x04a3].copy_from_slice(&[
        0x07, 0x00, 0x03, 0x02, 0x01, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x03, 0x04, 0x00,
    ]);
    source[0x04a4..0x04b9].copy_from_slice(&[
        0x0a, 0x01, 0x02, 0x02, 0x02, 0x03, 0x02, 0x04, 0x02, 0x05, 0x02, 0x02, 0x01, 0x06, 0x02,
        0x07, 0x02, 0x04, 0x02, 0x00, 0x03,
    ]);
    for (offset, instruction) in [
        (
            0x1eb0,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x2474,
            },
        ),
        (
            0x1ed0,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::ZERO,
                immediate: 0x00a0,
            },
        ),
        (
            0x1ef0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0340,
            },
        ),
        (
            0x2024,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S5,
                immediate: 16,
            },
        ),
        (
            0x26b0,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x2494,
            },
        ),
        (
            0x270c,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0340,
            },
        ),
        (
            0x28a4,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 0x24a4,
            },
        ),
        (
            0x28f8,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0340,
            },
        ),
    ] {
        write_instruction(&mut source, offset, instruction);
    }

    let entries = vec![
        dynamic_entry("character_select_heading", "캐릭터 선택"),
        dynamic_entry("tournament_heading", "토너먼트전"),
        entry("selection_back_help", "SELECT 버튼 = BACK"),
    ];
    let mut heading_characters = "캐릭터 선택토너먼트전".chars().collect::<Vec<_>>();
    heading_characters.sort_unstable();
    heading_characters.dedup();
    let mut allocations = heading_characters
        .into_iter()
        .enumerate()
        .map(|(index, character)| allocation(character, index))
        .collect::<Vec<_>>();
    let mut help_characters = "SELECT 버튼 = BACK".chars().collect::<Vec<_>>();
    help_characters.sort_unstable();
    help_characters.dedup();
    for (index, character) in help_characters.into_iter().enumerate() {
        let u = (index * 12) as u8;
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::SelectionHelp,
            character,
            surface: CharacterSelectTextureSurface::SelectionHelpAtlas,
            tim_offset: 0x17800,
            texture_page_index: 3,
            texture_uv: [u, 96],
            cell: Cell {
                x: 768 + usize::from(u),
                y: 96,
                width: 12,
                height: 16,
            },
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
        });
    }

    let (patched, report) = rewrite_character_select_overlay(
        "DAT1/PLSEL4.BIN",
        &source,
        &allocations,
        &consumer_plan(),
        &entries,
    )
    .unwrap();

    let expected_descriptor = "SELECT 버튼 = BACK"
        .chars()
        .flat_map(|character| {
            let glyph = allocations
                .iter()
                .find(|glyph| {
                    glyph.font_role == CharacterSelectFontRole::SelectionHelp
                        && glyph.character == character
                })
                .unwrap();
            [glyph.texture_uv[0] / 12, (glyph.texture_uv[1] - 96) / 16]
        })
        .collect::<Vec<_>>();
    assert_eq!(&patched[0x0474..0x0494], expected_descriptor);
    assert_eq!(
        read_instruction(&patched, 0x1ed0),
        Instruction::Addiu {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 0x00a0,
        }
    );
    assert_eq!(
        read_instruction(&patched, 0x1ef0),
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x03c0,
        }
    );
    assert_eq!(
        read_instruction(&patched, 0x2024),
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::S5,
            immediate: 16,
        }
    );
    assert_eq!(
        report
            .descriptors
            .iter()
            .find(|descriptor| descriptor.translation_id == "selection_back_help")
            .unwrap()
            .logical_character_count,
        16
    );

    // The same source descriptor also exists in League, with a different reader.
    let mut league = vec![0_u8; 40_000];
    league[0x04a4..0x04c4].copy_from_slice(&source[0x0474..0x0494]);
    write_instruction(
        &mut league,
        0x2538,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 0x24a4,
        },
    );
    write_instruction(
        &mut league,
        0x2578,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x0340,
        },
    );
    let help = vec![entry("selection_back_help", "SELECT 버튼 = BACK")];
    let (league_patched, league_report) = rewrite_character_select_overlay(
        "DAT1/PLSEL3.BIN",
        &league,
        &allocations,
        &consumer_plan(),
        &help,
    )
    .unwrap();
    assert_eq!(&league_patched[0x04a4..0x04c4], expected_descriptor);
    assert_eq!(
        read_instruction(&league_patched, 0x2578),
        read_instruction(&patched, 0x1ef0)
    );
    assert_eq!(&league_patched[0x1bc0..0x1bc4], &league[0x1bc0..0x1bc4]);
    assert_eq!(league_report.descriptors[0].texture_page_indices, [3]);
    let short = vec![entry("selection_back_help", "SELECT 버튼")];
    assert!(
        rewrite_character_select_overlay(
            "DAT1/PLSEL3.BIN",
            &league,
            &allocations,
            &consumer_plan(),
            &short
        )
        .unwrap_err()
        .to_string()
        .contains("exactly 16 slots")
    );
}

fn dynamic_entry(id: &str, korean_text: &str) -> CharacterSelectLocalizedSource {
    entry(id, korean_text)
}

fn entry(id: &str, korean_text: &str) -> CharacterSelectLocalizedSource {
    CharacterSelectLocalizedSource {
        source_ui_id: id.to_string(),
        translation_id: id.to_string(),
        source_text: "source".to_string(),
        korean_text: korean_text.to_string(),
    }
}

fn allocation(character: char, index: usize) -> CharacterSelectGlyphAllocation {
    let u = (index % 8 * 32) as u8;
    let v = (index / 8 * 32 + 128) as u8;
    CharacterSelectGlyphAllocation {
        font_role: CharacterSelectFontRole::SelectHeading,
        character,
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        tim_offset: 0x17800,
        texture_page_index: 2,
        texture_uv: [u, v],
        cell: Cell {
            x: 512 + usize::from(u),
            y: usize::from(v),
            width: 32,
            height: 32,
        },
        source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
    }
}

fn consumer_plan() -> CharacterSelectConsumerPlan {
    CharacterSelectConsumerPlan {
        shared_atlas_vram_word_x: 0x0300,
        select_heading_texture_page_index: 2,
    }
}

fn write_instruction(target: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    target[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn read_instruction(source: &[u8], offset: usize) -> Instruction {
    let word = u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap());
    psx_r3000a::decode(word, OVERLAY_RUNTIME_BASE + offset as u32).unwrap()
}
