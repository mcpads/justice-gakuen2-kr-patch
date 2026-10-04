use std::path::Path;

use psx_r3000a::{Instruction, Register, verify_placed_program};

use crate::tim::Cell;

use super::{
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    build_name_input_redisplay_runtime_program, load_name_input_keyboard,
    plan_name_input_runtime_atlas,
};

const MATERIALIZER_ADDRESS: u32 = 0x8010_36cc;
const COMPONENT_RESOLVER_ADDRESS: u32 = 0x8010_34c4;

#[test]
#[ignore = "requires assets/"]
fn tagged_redisplay_runtime_owns_typed_code_lookup_and_stack_upload_packet() {
    let atlas = fixture_atlas();
    let runtime = build_name_input_redisplay_runtime_program(
        &atlas,
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let instructions = verify_placed_program(
        &runtime.bytes[..runtime.report.instruction_byte_count],
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    )
    .unwrap();

    assert_eq!(runtime.report.origin, "0x801012e0");
    assert_eq!(runtime.tagged_code_resolver_address, 0x8010_12e0);
    assert!(runtime.cache_cell_uploader_address > runtime.tagged_code_resolver_address);
    assert!(runtime.selected_key_handler_address > runtime.cache_cell_uploader_address);
    assert_eq!(
        runtime.report.compound_final_table_byte_range,
        [0x4b4, 0x4cc]
    );
    assert_eq!(runtime.report.simple_final_table_byte_range, [0x4cc, 0x4df]);
    assert_eq!(runtime.report.cache_cell_table_byte_range, [0x4e0, 0x520]);
    for (entry, expected_offset) in runtime.bytes[0x4e0..0x520]
        .as_chunks::<4>()
        .0
        .iter()
        .zip(&atlas.cache_cell_base_byte_offsets)
    {
        assert_eq!(
            u16::from_le_bytes(entry[..2].try_into().unwrap()),
            *expected_offset
        );
    }
    assert_eq!(runtime.report.upload_tim_stack_byte_count, 220);
    assert_eq!(runtime.report.upload_tim_pixel_byte_count, 200);
    assert_eq!(runtime.bytes.len(), 0x520);
    assert_eq!(runtime.report.typed_instruction_count, instructions.len());
    assert!(runtime.bytes.len() <= NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY);
    assert!(runtime.report.fits_runtime_region);
    assert!(!runtime.report.installed);
    assert!(!runtime.report.renderer_hook_installed);
    assert!(!runtime.report.selection_hook_installed);
    assert!(!runtime.report.runtime_execution_verified);
    assert!(instructions.contains(&Instruction::Jal {
        target: MATERIALIZER_ADDRESS,
    }));
    assert!(instructions.contains(&Instruction::Jalr {
        rd: Register::RA,
        rs: Register::T0,
    }));
    assert!(instructions.contains(&Instruction::Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -240,
    }));
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::ZERO,
        base: Register::SP,
        offset: 4,
    }));
    assert!(instructions.contains(&Instruction::Sh {
        rt: Register::T0,
        base: Register::SP,
        offset: 18,
    }));
}

#[test]
#[ignore = "requires assets/"]
fn simple_final_table_maps_keyboard_initials_to_modern_hangul_finals() {
    let runtime = build_name_input_redisplay_runtime_program(
        &fixture_atlas(),
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();

    assert_eq!(
        &runtime.bytes[0x4cc..0x4df],
        &[
            1, 2, 4, 7, 0xff, 8, 16, 17, 0xff, 19, 20, 21, 22, 0xff, 23, 24, 25, 26, 27
        ]
    );
}

#[test]
#[ignore = "requires assets/"]
fn compound_final_table_covers_every_modern_two_consonant_final() {
    let runtime = build_name_input_redisplay_runtime_program(
        &fixture_atlas(),
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let entries = runtime.bytes[0x4b4..0x4cc]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| u16::from_le_bytes(*bytes))
        .collect::<Vec<_>>();
    let decoded = entries[..11]
        .iter()
        .map(|entry| {
            (
                u8::try_from(entry & 0x1f).unwrap(),
                u8::try_from((entry >> 5) & 0x1f).unwrap(),
                u8::try_from((entry >> 10) & 0x1f).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        decoded,
        [
            (1, 9, 3),
            (4, 12, 5),
            (4, 18, 6),
            (8, 0, 9),
            (8, 6, 10),
            (8, 7, 11),
            (8, 9, 12),
            (8, 16, 13),
            (8, 17, 14),
            (8, 18, 15),
            (17, 9, 18),
        ]
    );
    assert_eq!(entries[11], 0);
}

#[test]
#[ignore = "requires assets/"]
fn cache_upload_table_binds_source_offsets_to_vram_word_coordinates() {
    let atlas = fixture_atlas();
    let runtime = build_name_input_redisplay_runtime_program(
        &atlas,
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let table = &runtime.bytes[0x4e0..0x520];

    for (entry, offset) in table
        .as_chunks::<4>()
        .0
        .iter()
        .zip(atlas.cache_cell_base_byte_offsets)
    {
        let stored_offset = u16::from_le_bytes(entry[0..2].try_into().unwrap());
        let vram_x = 768 + u16::from(entry[2]);
        let vram_y = u16::from(entry[3]);
        assert_eq!(stored_offset, offset);
        assert_eq!(vram_x, 768 + (offset % 384) / 2);
        assert_eq!(vram_y, offset / 384);
    }
}

#[test]
#[ignore = "requires assets/"]
fn cache_copy_observes_the_r3000a_load_delay() {
    let runtime = build_name_input_redisplay_runtime_program(
        &fixture_atlas(),
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let instructions = verify_placed_program(
        &runtime.bytes[..runtime.report.instruction_byte_count],
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    )
    .unwrap();
    let load = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Lbu {
                    rt: Register::T8,
                    base: Register::T1,
                    offset: 0,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[load + 1],
        Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 1,
        }
    );
    assert_eq!(
        instructions[load + 2],
        Instruction::Sb {
            rt: Register::T8,
            base: Register::T5,
            offset: 0,
        }
    );
}

#[test]
#[ignore = "requires assets/"]
fn tagged_resolver_restores_the_source_lookup_index_contract() {
    let runtime = build_name_input_redisplay_runtime_program(
        &fixture_atlas(),
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let instructions = verify_placed_program(
        &runtime.bytes[..runtime.report.instruction_byte_count],
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    )
    .unwrap();
    let resolver_return = instructions
        .iter()
        .position(|instruction| *instruction == Instruction::Jr { rs: Register::RA })
        .unwrap();
    let lookup_index_reset = instructions[..resolver_return]
        .iter()
        .rposition(|instruction| {
            *instruction
                == Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                }
        })
        .unwrap();

    assert!(resolver_return - lookup_index_reset < 12);
}

#[test]
#[ignore = "requires assets/"]
fn tagged_resolver_accepts_every_sprite_record_in_each_name_field() {
    let runtime = build_name_input_redisplay_runtime_program(
        &fixture_atlas(),
        MATERIALIZER_ADDRESS,
        COMPONENT_RESOLVER_ADDRESS,
    )
    .unwrap();
    let instructions = verify_placed_program(
        &runtime.bytes[..runtime.report.instruction_byte_count],
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    )
    .unwrap();
    let accepted_spans = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::Sltiu {
                rt: Register::T0,
                rs: Register::T1,
                immediate,
            } if matches!(*immediate, 0x150 | 0xe0) => Some(*immediate),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(accepted_spans, [0x150, 0x150, 0xe0]);
}

pub(super) fn fixture_atlas() -> super::NameInputRuntimeAtlasLayout {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let cells = (0..228)
        .map(|index| {
            let page = index / 84;
            let position = index % 84;
            Cell {
                x: page * 256 + position % 12 * 20,
                y: position / 12 * 20,
                width: 20,
                height: 20,
            }
        })
        .collect::<Vec<_>>();
    plan_name_input_runtime_atlas(&cells, &keyboard).unwrap()
}
