use std::path::Path;

use psx_r3000a::{Instruction, Register, verify_placed_program};

use crate::tim::Cell;

use super::{
    NAME_INPUT_RUNTIME_BYTE_CAPACITY, NAME_INPUT_RUNTIME_ORIGIN,
    build_name_input_runtime_program_with_stored_atlas_and_outline, load_name_input_keyboard,
    plan_name_input_runtime_atlas,
};

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn fill_materializer_occupies_its_own_typed_runtime_range() {
    let runtime = stored_atlas_runtime();
    let decoded = verify_placed_program(&runtime.bytes, NAME_INPUT_RUNTIME_ORIGIN).unwrap();
    let resolver = runtime.component_resolver_address.unwrap();
    let materializer = runtime.glyph_fill_materializer_address.unwrap();
    let writer = runtime.selected_code_writer_address;
    let materializer_index = instruction_index(materializer);
    let writer_index = instruction_index(writer);

    assert!(materializer > resolver);
    assert!(writer > materializer);
    assert_eq!(
        runtime.report.glyph_fill_materializer_address,
        Some(format!("0x{materializer:08x}"))
    );
    assert_eq!(
        decoded[materializer_index],
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -16,
        }
    );
    assert!(
        decoded[materializer_index..writer_index].contains(&Instruction::Sb {
            rt: Register::T2,
            base: Register::A3,
            offset: 0,
        })
    );
    assert!(
        decoded[materializer_index..writer_index].contains(&Instruction::Ori {
            rt: Register::T8,
            rs: Register::ZERO,
            immediate: 384,
        })
    );
    assert!(runtime.bytes.len() <= NAME_INPUT_RUNTIME_BYTE_CAPACITY);
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn cache_clearer_returns_the_resolved_pixel_cell_address() {
    let runtime = stored_atlas_runtime();
    let decoded = verify_placed_program(&runtime.bytes, NAME_INPUT_RUNTIME_ORIGIN).unwrap();
    let clearer_index = instruction_index(runtime.cache_cell_clearer_address.unwrap());
    let resolver_index = instruction_index(runtime.component_resolver_address.unwrap());
    let clearer = &decoded[clearer_index..resolver_index];
    let return_pointer = Instruction::Addu {
        rd: Register::V0,
        rs: Register::T0,
        rt: Register::ZERO,
    };
    let clear_byte = Instruction::Sb {
        rt: Register::ZERO,
        base: Register::T0,
        offset: 0,
    };

    assert!(clearer.contains(&return_pointer));
    assert!(clearer.contains(&clear_byte));
    assert!(
        clearer
            .iter()
            .position(|instruction| *instruction == return_pointer)
            < clearer
                .iter()
                .position(|instruction| *instruction == clear_byte)
    );
}

fn stored_atlas_runtime() -> super::NameInputRuntimeProgram {
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

    build_name_input_runtime_program_with_stored_atlas_and_outline(
        &atlas,
        &fixture_pack(),
        0x800c_b000,
        0x800c_b100,
        0x800c_b200,
    )
    .unwrap()
}

fn instruction_index(address: u32) -> usize {
    usize::try_from((address - NAME_INPUT_RUNTIME_ORIGIN) / 4).unwrap()
}

fn fixture_pack() -> super::NameInputRuntimePackLayout {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf");
    let pack = super::build_default_name_glyph_pack(&font).unwrap();
    super::plan_name_input_runtime_pack(&pack.report).unwrap()
}
