use std::path::Path;

use crate::name_input::{
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, build_name_input_redisplay_runtime_program,
    load_name_input_keyboard, plan_name_input_runtime_atlas,
};
use crate::tim::Cell;

use super::name_entry_redisplay_runtime::{
    NEXT_EMBEDDED_TIM_OFFSET, REDISPLAY_RUNTIME_REGION_OFFSET,
    audit_name_entry_redisplay_runtime_region, install_name_entry_redisplay_runtime_program,
};
#[test]
#[ignore = "requires assets/"]
fn exact_inter_tim_padding_accepts_and_reads_back_typed_redisplay_runtime() {
    let source = source_decoded_fixture();
    let font_tim_size = REDISPLAY_RUNTIME_REGION_OFFSET - 0x19000;
    let mut patched = source.clone();
    let atlas = runtime_atlas_fixture();
    let program =
        build_name_input_redisplay_runtime_program(&atlas, 0x8010_36cc, 0x8010_34c4).unwrap();
    let mut report = audit_name_entry_redisplay_runtime_region(&source, font_tim_size).unwrap();

    install_name_entry_redisplay_runtime_program(&source, &mut patched, &program.bytes).unwrap();
    report.typed_code_installed = true;

    assert_eq!(
        report.decoded_byte_range,
        [REDISPLAY_RUNTIME_REGION_OFFSET, NEXT_EMBEDDED_TIM_OFFSET]
    );
    assert_eq!(report.runtime_address_range, ["0x801012e0", "0x80101800"]);
    assert_eq!(
        report.byte_count,
        NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY
    );
    assert!(report.source_bytes_are_zero);
    assert!(report.follows_font_tim_exactly);
    assert!(report.typed_code_installed);
    assert_eq!(
        &patched[REDISPLAY_RUNTIME_REGION_OFFSET
            ..REDISPLAY_RUNTIME_REGION_OFFSET + program.bytes.len()],
        program.bytes
    );
}

#[test]
#[ignore = "requires assets/"]
fn changed_boundary_or_preclaimed_inter_tim_padding_is_rejected() {
    let source = source_decoded_fixture();
    let font_tim_size = REDISPLAY_RUNTIME_REGION_OFFSET - 0x19000;
    assert!(audit_name_entry_redisplay_runtime_region(&source, font_tim_size - 4).is_err());

    let mut patched = source.clone();
    patched[REDISPLAY_RUNTIME_REGION_OFFSET] = 1;
    let atlas = runtime_atlas_fixture();
    let program =
        build_name_input_redisplay_runtime_program(&atlas, 0x8010_36cc, 0x8010_34c4).unwrap();
    assert!(
        install_name_entry_redisplay_runtime_program(&source, &mut patched, &program.bytes)
            .is_err()
    );
}

fn source_decoded_fixture() -> Vec<u8> {
    let mut decoded = vec![0_u8; NEXT_EMBEDDED_TIM_OFFSET + 66];
    let tim = &mut decoded[NEXT_EMBEDDED_TIM_OFFSET..];
    tim[0..4].copy_from_slice(&0x10_u32.to_le_bytes());
    tim[4..8].copy_from_slice(&8_u32.to_le_bytes());
    tim[8..12].copy_from_slice(&44_u32.to_le_bytes());
    tim[16..18].copy_from_slice(&16_u16.to_le_bytes());
    tim[18..20].copy_from_slice(&1_u16.to_le_bytes());
    tim[52..56].copy_from_slice(&14_u32.to_le_bytes());
    tim[60..62].copy_from_slice(&1_u16.to_le_bytes());
    tim[62..64].copy_from_slice(&1_u16.to_le_bytes());
    decoded
}

fn runtime_atlas_fixture() -> crate::name_input::NameInputRuntimeAtlasLayout {
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
