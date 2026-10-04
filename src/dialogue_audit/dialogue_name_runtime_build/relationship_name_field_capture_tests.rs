use psx_r3000a::{Instruction, Register, decode};

use super::relationship_name_field_capture::{
    RELATIONSHIP_BUILDER_CASE_ADDRESSES, RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES,
    install_relationship_name_field_capture, relationship_name_field_capture_replacements,
};

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const MGAME_SIZE: usize = 0x0002_e000;

#[test]
fn every_relationship_builder_path_preserves_its_selected_name_field() {
    let source = synthetic_mgame();
    let mut patched = source.clone();

    let report = install_relationship_name_field_capture(&source, &mut patched).unwrap();

    assert_eq!(
        report.sites.each_ref().map(|site| site.address.as_str()),
        [
            "0x800afbc4",
            "0x800afc34",
            "0x800afc9c",
            "0x800afd18",
            "0x800afd94",
            "0x800afdf0",
            "0x800afe58",
        ]
    );
    assert_eq!(report.captured_register, "t9");
    assert_eq!(report.field_categories, ["family", "given", "nickname"]);
    assert_eq!(
        report.relationship_control_codes,
        ["0x2003", "0x2004", "0x2005", "0x2006", "0x2007", "0x2008"]
    );
    assert_eq!(
        report.relationship_builder_case_addresses,
        [
            "0x800afbf0",
            "0x800afc58",
            "0x800afcd4",
            "0x800afd50",
            "0x800afdac",
            "0x800afe14",
        ]
    );
    assert!(report.given_name_is_the_default_category);
    assert!(report.family_and_nickname_categories_captured_after_classification);
    assert!(report.jump_table_source_verified);
    assert!(report.source_verified);
    assert!(report.installed);
    for (address, expected, _) in relationship_name_field_capture_replacements() {
        assert_eq!(
            decode(read_word(&patched, runtime_offset(address)), address).unwrap(),
            expected
        );
    }
    assert_eq!(
        decode(
            read_word(
                &patched,
                runtime_offset(RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[0])
            ),
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[0],
        )
        .unwrap(),
        Instruction::Ori {
            rt: Register::T9,
            rs: Register::ZERO,
            immediate: 1,
        }
    );
}

#[test]
fn changed_relationship_builder_is_rejected_before_any_capture_is_written() {
    let mut source = synthetic_mgame();
    let changed_offset = runtime_offset(RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[6]);
    source[changed_offset] = 1;
    let mut patched = source.clone();

    assert!(install_relationship_name_field_capture(&source, &mut patched).is_err());
    assert_eq!(patched, source);
}

fn synthetic_mgame() -> Vec<u8> {
    let mut source = vec![0_u8; MGAME_SIZE];
    let jump_table_offset = runtime_offset(0x800c_a700);
    for (index, address) in RELATIONSHIP_BUILDER_CASE_ADDRESSES.into_iter().enumerate() {
        let offset = jump_table_offset + index * 4;
        source[offset..offset + 4].copy_from_slice(&address.to_le_bytes());
    }
    source
}

fn runtime_offset(address: u32) -> usize {
    usize::try_from(address - MGAME_RUNTIME_BASE).unwrap()
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
