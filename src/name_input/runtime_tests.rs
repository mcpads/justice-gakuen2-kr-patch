use std::path::Path;

use psx_r3000a::{Instruction, Register, verify_placed_program};

use crate::tim::Cell;

use super::runtime::runtime_metadata_storage_cell_count;
use super::{
    NAME_INPUT_RUNTIME_BYTE_CAPACITY, NAME_INPUT_RUNTIME_ORIGIN, build_name_input_runtime_program,
    build_name_input_runtime_program_with_atlas,
    build_name_input_runtime_program_with_stored_atlas_and_outline, load_name_input_keyboard,
    plan_name_input_runtime_atlas,
};

#[test]
fn typed_runtime_places_pack_reader_and_selected_code_writer() {
    let runtime = build_name_input_runtime_program().unwrap();
    let decoded = verify_placed_program(&runtime.bytes, NAME_INPUT_RUNTIME_ORIGIN).unwrap();

    assert_eq!(runtime.report.origin, "0x80103340");
    assert_eq!(runtime.report.pack_byte_reader_address, "0x80103340");
    assert_eq!(runtime.report.selected_code_writer_address, "0x80103420");
    assert_eq!(runtime.selected_code_writer_address, 0x8010_3420);
    assert_eq!(runtime.report.byte_count, runtime.bytes.len());
    assert_eq!(runtime.report.typed_instruction_count, decoded.len());
    assert!(runtime.bytes.len() <= NAME_INPUT_RUNTIME_BYTE_CAPACITY);
    assert!(runtime.report.fits_runtime_region);
    assert!(!runtime.report.overlay_hook_installed);
    assert!(!runtime.report.runtime_execution_verified);
    assert!(decoded.iter().any(|instruction| {
        *instruction
            == Instruction::Divu {
                rs: Register::A0,
                rt: Register::T0,
            }
    }));
    let writer_index =
        usize::try_from((runtime.selected_code_writer_address - NAME_INPUT_RUNTIME_ORIGIN) / 4)
            .unwrap();
    assert_eq!(
        decoded[writer_index],
        Instruction::Sh {
            rt: Register::V1,
            base: Register::V0,
            offset: 0,
        }
    );
    assert_eq!(decoded.len() - writer_index, 3);
    assert_eq!(
        decoded.last(),
        Some(&Instruction::Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        })
    );
}

#[test]
#[ignore = "requires assets/"]
fn table_driven_pack_reader_keeps_more_typed_code_capacity() {
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
    let atlas = plan_name_input_runtime_atlas(&cells, &keyboard).unwrap();
    let table_runtime = build_name_input_runtime_program_with_atlas(&atlas).unwrap();
    let arithmetic_runtime = build_name_input_runtime_program().unwrap();
    let instruction_origin =
        NAME_INPUT_RUNTIME_ORIGIN + u32::try_from(table_runtime.instruction_offset).unwrap();
    let decoded = verify_placed_program(
        &table_runtime.bytes[table_runtime.instruction_offset..],
        instruction_origin,
    )
    .unwrap();

    let expected_instruction_offset = atlas.lookup_table_bytes.len().next_multiple_of(16);
    let expected_instruction_origin =
        NAME_INPUT_RUNTIME_ORIGIN + u32::try_from(expected_instruction_offset).unwrap();
    assert_eq!(
        table_runtime.instruction_offset,
        expected_instruction_offset
    );
    assert_eq!(
        table_runtime.report.instruction_origin,
        format!("0x{expected_instruction_origin:08x}")
    );
    assert_eq!(table_runtime.report.atlas_lookup_storage, "runtime_prefix");
    assert_eq!(
        table_runtime.report.pack_byte_reader_address,
        format!("0x{expected_instruction_origin:08x}")
    );
    assert_eq!(
        table_runtime.report.atlas_lookup_table_byte_count,
        atlas.lookup_table_bytes.len()
    );
    assert!(table_runtime.report.pack_reader_uses_atlas_lookup);
    assert_eq!(
        &table_runtime.bytes[..atlas.lookup_table_bytes.len()],
        atlas.lookup_table_bytes
    );
    assert!(
        table_runtime.report.typed_instruction_count
            < arithmetic_runtime.report.typed_instruction_count
    );
    assert_eq!(decoded.len(), table_runtime.report.typed_instruction_count);
    assert!(table_runtime.bytes.len() <= NAME_INPUT_RUNTIME_BYTE_CAPACITY);
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn stored_atlas_lookup_preserves_runtime_origin_and_frees_code_capacity() {
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
    let atlas = plan_name_input_runtime_atlas(&cells, &keyboard).unwrap();
    let stored_runtime = build_name_input_runtime_program_with_stored_atlas_and_outline(
        &atlas,
        &fixture_pack(),
        0x800c_b000,
        0x800c_b100,
        0x800c_b200,
    )
    .unwrap();
    let decoded = verify_placed_program(&stored_runtime.bytes, NAME_INPUT_RUNTIME_ORIGIN).unwrap();

    assert_eq!(stored_runtime.instruction_offset, 0);
    assert_eq!(
        runtime_metadata_storage_cell_count(
            &atlas,
            fixture_pack().runtime_coordinate_list_byte_count
        ),
        2
    );
    assert_eq!(stored_runtime.report.instruction_origin, "0x80103340");
    assert_eq!(stored_runtime.report.pack_byte_reader_address, "0x80103340");
    assert_eq!(
        stored_runtime.report.atlas_lookup_table_byte_count,
        atlas.pack_storage_cell_base_byte_offsets.len() * 2
    );
    assert_eq!(
        stored_runtime.report.atlas_lookup_storage,
        "glyph_pack_tail_cells"
    );
    let clearer_address = stored_runtime.cache_cell_clearer_address.unwrap();
    assert_eq!(
        stored_runtime.report.cache_cell_clearer_address,
        Some(format!("0x{clearer_address:08x}"))
    );
    assert!(stored_runtime.selected_code_writer_address > clearer_address);
    let clearer_index = usize::try_from((clearer_address - NAME_INPUT_RUNTIME_ORIGIN) / 4).unwrap();
    let writer_index = usize::try_from(
        (stored_runtime.selected_code_writer_address - NAME_INPUT_RUNTIME_ORIGIN) / 4,
    )
    .unwrap();
    assert_eq!(
        decoded[clearer_index],
        Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: 16,
        }
    );
    assert!(
        decoded[clearer_index..writer_index].contains(&Instruction::Sb {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
    );
    let resolver_address = stored_runtime.component_resolver_address.unwrap();
    assert_eq!(
        stored_runtime.report.component_resolver_address,
        Some(format!("0x{resolver_address:08x}"))
    );
    assert!(resolver_address > clearer_address);
    assert!(stored_runtime.selected_code_writer_address > resolver_address);
    let materializer_address = stored_runtime.glyph_fill_materializer_address.unwrap();
    let materializer_index =
        usize::try_from((materializer_address - NAME_INPUT_RUNTIME_ORIGIN) / 4).unwrap();
    assert!(
        decoded[..materializer_index]
            .iter()
            .all(|instruction| !matches!(
                instruction.written_gpr(),
                Some(Register::T8 | Register::T9)
            )),
        "component resolution and its callees must preserve the redisplay handler's pointer and return registers"
    );
    assert!(stored_runtime.report.pack_reader_uses_atlas_lookup);
    assert!(decoded.contains(&Instruction::Jal {
        target: 0x800c_b000,
    }));
    assert!(decoded.contains(&Instruction::Jal {
        target: 0x800c_b100,
    }));
    assert!(decoded.contains(&Instruction::Jal {
        target: 0x800c_b200,
    }));
    assert_eq!(decoded.len(), stored_runtime.report.typed_instruction_count);
    assert!(stored_runtime.bytes.len() <= NAME_INPUT_RUNTIME_BYTE_CAPACITY);
}

fn fixture_pack() -> super::NameInputRuntimePackLayout {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf");
    let pack = super::build_default_name_glyph_pack(&font).unwrap();
    super::plan_name_input_runtime_pack(&pack.report).unwrap()
}
