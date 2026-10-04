use psx_r3000a::{Instruction, Register};

use super::scanner::{
    OVERLAY_BASE, ReferenceIndex, scan_overlay_with_shared_references_report, scan_references,
    wrapped_cells_overlap,
};

const MAIN_TEXT_BASE: u32 = 0x8001_0000;

#[test]
fn direct_pointer_scan_deduplicates_targets_and_preserves_control_nibbles() {
    let mut data = vec![0u8; 0x80];
    let pointer = (OVERLAY_BASE + 0x40).to_le_bytes();
    data[0..4].copy_from_slice(&pointer);
    data[4..8].copy_from_slice(&pointer);
    data[0x40..0x42].copy_from_slice(&3u16.to_le_bytes());
    data[0x42..0x48].copy_from_slice(&[0x18, 0x12, 0xff, 0x0f, 0x95, 0x01]);

    let candidates = scan_overlay(&data);
    assert_eq!(candidates.len(), 1);
    let candidate = candidates.get(&0x40).unwrap();
    assert_eq!(candidate.pointer_offsets, [0, 4]);
    assert!(candidate.address_materialization_references.is_empty());
    assert!(candidate.memory_access_references.is_empty());
    assert!(candidate.loaded_word_references.is_empty());
    assert_eq!(candidate.raw_codes, [0x1218, 0x0fff, 0x0195]);
}

#[test]
fn shared_executable_reference_admits_an_overlay_string_without_local_references() {
    let mut overlay = vec![0u8; 0x80];
    overlay[0x40..0x42].copy_from_slice(&2u16.to_le_bytes());
    overlay[0x42..0x46].copy_from_slice(&[0x47, 0x00, 0x69, 0x00]);

    let mut executable_text = vec![0u8; 0x20];
    install_instructions_at_base(
        &mut executable_text,
        &[
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800a,
            },
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x2040,
            },
        ],
        MAIN_TEXT_BASE,
    );
    let shared_references = scan_references(
        &executable_text,
        MAIN_TEXT_BASE,
        OVERLAY_BASE,
        overlay.len(),
        0x800,
        super::DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET,
    );

    assert!(scan_overlay(&overlay).is_empty());
    let (candidates, _) = scan_overlay_with_shared_references_report(
        &overlay,
        &shared_references,
        super::DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET,
    );
    let candidate = candidates.get(&0x40).unwrap();
    assert!(candidate.pointer_offsets.is_empty());
    assert!(candidate.address_materialization_references.is_empty());
    assert!(candidate.memory_access_references.is_empty());
    assert!(candidate.shared_pointer_offsets.is_empty());
    assert_eq!(candidate.shared_address_materialization_references.len(), 1);
    assert_eq!(
        candidate.shared_address_materialization_references[0].seed_offset,
        0x800
    );
    assert!(candidate.shared_memory_access_references.is_empty());
    assert!(candidate.shared_loaded_word_references.is_empty());
    assert_eq!(candidate.raw_codes, [0x0047, 0x0069]);
}

#[test]
fn memory_access_admits_a_string_without_a_pointer_constant() {
    let mut overlay = vec![0u8; 0x80];
    install_instructions_at_base(
        &mut overlay,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::S0,
                offset: 0x2040,
            },
        ],
        OVERLAY_BASE,
    );
    overlay[0x40..0x42].copy_from_slice(&2u16.to_le_bytes());
    overlay[0x42..0x46].copy_from_slice(&[0x47, 0x00, 0x69, 0x00]);

    let candidates = scan_overlay(&overlay);
    let candidate = candidates.get(&0x40).unwrap();
    assert!(candidate.pointer_offsets.is_empty());
    assert!(candidate.address_materialization_references.is_empty());
    assert_eq!(candidate.memory_access_references.len(), 1);
    assert_eq!(candidate.memory_access_references[0].seed_offset, 0);
    assert_eq!(candidate.memory_access_references[0].instruction_offset, 4);
    assert!(candidate.loaded_word_references.is_empty());
    assert_eq!(candidate.raw_codes, [0x0047, 0x0069]);
}

#[test]
fn loaded_word_preserves_consumer_and_storage_provenance() {
    let mut overlay = vec![0u8; 0x80];
    install_instructions_at_base(
        &mut overlay,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
        ],
        OVERLAY_BASE,
    );
    overlay[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());
    overlay[0x40..0x42].copy_from_slice(&2u16.to_le_bytes());
    overlay[0x42..0x46].copy_from_slice(&[0x47, 0x00, 0x69, 0x00]);

    let candidates = scan_overlay(&overlay);
    let candidate = candidates.get(&0x40).unwrap();
    assert_eq!(candidate.pointer_offsets, [0x20]);
    assert_eq!(candidate.loaded_word_references.len(), 1);
    assert_eq!(candidate.loaded_word_references[0].instruction_offset, 4);
    assert_eq!(
        candidate.loaded_word_references[0].load_instruction_offset,
        4
    );
    assert_eq!(
        candidate.loaded_word_references[0].storage_address,
        OVERLAY_BASE + 0x20
    );
    assert_eq!(candidate.raw_codes, [0x0047, 0x0069]);
}

#[test]
fn loaded_pointer_derivation_admits_a_string_after_the_delay_slot() {
    let mut overlay = vec![0u8; 0x90];
    install_instructions_at_base(
        &mut overlay,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
            Instruction::nop(),
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x20,
            },
        ],
        OVERLAY_BASE,
    );
    overlay[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());
    overlay[0x60..0x62].copy_from_slice(&2u16.to_le_bytes());
    overlay[0x62..0x66].copy_from_slice(&[0x47, 0x00, 0x69, 0x00]);

    let candidates = scan_overlay(&overlay);
    let candidate = candidates.get(&0x60).unwrap();
    assert!(candidate.pointer_offsets.is_empty());
    assert_eq!(candidate.loaded_word_references.len(), 1);
    assert_eq!(candidate.loaded_word_references[0].instruction_offset, 12);
    assert_eq!(
        candidate.loaded_word_references[0].load_instruction_offset,
        4
    );
    assert_eq!(
        candidate.loaded_word_references[0].storage_address,
        OVERLAY_BASE + 0x20
    );
    assert_eq!(candidate.raw_codes, [0x0047, 0x0069]);
}

#[test]
fn menu_domain_rejects_out_of_range_codes() {
    let mut data = vec![0u8; 0x80];
    data[0..4].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());
    data[0x40..0x42].copy_from_slice(&1u16.to_le_bytes());
    data[0x42..0x44].copy_from_slice(&0x0400u16.to_le_bytes());
    assert!(scan_overlay(&data).is_empty());
}

#[test]
fn physical_cell_overlap_accounts_for_byte_sized_texture_coordinates() {
    assert!(wrapped_cells_overlap(0x0031, 0x003d));
    assert!(!wrapped_cells_overlap(0x0032, 0x003d));
    assert!(!wrapped_cells_overlap(0x0034, 0x0035));
    assert!(wrapped_cells_overlap(0x0000, 0x000c));
}

fn install_instructions_at_base(data: &mut [u8], instructions: &[Instruction], base: u32) {
    for (index, instruction) in instructions.iter().enumerate() {
        let pc = base + (index * 4) as u32;
        let encoded = psx_r3000a::encode(instruction, pc).unwrap();
        data[index * 4..index * 4 + 4].copy_from_slice(&encoded.to_le_bytes());
    }
}

fn scan_overlay(data: &[u8]) -> std::collections::BTreeMap<usize, super::scanner::CandidateString> {
    scan_overlay_with_shared_references_report(
        data,
        &ReferenceIndex::default(),
        super::DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET,
    )
    .0
}
