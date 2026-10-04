use psx_r3000a::{Instruction, Register, verify_placed_program};
use std::path::Path;

use crate::cue::CueSheet;
use crate::disc::rebuild::read_record;
use crate::name_input::{load_name_input_keyboard, plan_name_glyph_consumer_layout};

use super::name_entry::{OVERLAY_PATH, OVERLAY_RUNTIME_BASE};
use super::name_entry_nickname_companion::{
    build_nickname_companion_program, install_name_entry_nickname_companion,
    verify_nickname_companion_lookup_preserved,
};

const ROUTINE_ADDRESS: u32 = OVERLAY_RUNTIME_BASE + 0x6a10;

fn consumer_layout() -> crate::name_input::NameGlyphConsumerLayout {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    plan_name_glyph_consumer_layout(&keyboard).unwrap()
}

#[test]
#[ignore = "requires assets/"]
fn tagged_nickname_slots_use_their_persistent_cache_codes() {
    let program = build_nickname_companion_program(&consumer_layout()).unwrap();
    let instructions = verify_placed_program(&program.bytes, ROUTINE_ADDRESS).unwrap();

    assert_eq!(program.bytes.len(), 0x88);
    assert!(instructions.contains(&Instruction::Andi {
        rt: Register::T0,
        rs: Register::A2,
        immediate: 0xc000,
    }));
    assert!(instructions.contains(&Instruction::Bne {
        rs: Register::T0,
        rt: Register::ZERO,
        target: program.tagged_cache_address,
    }));
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::T4,
        rs: Register::ZERO,
        immediate: 0x0339,
    }));
    assert!(instructions.contains(&Instruction::Addu {
        rd: Register::V0,
        rs: Register::T4,
        rt: Register::ZERO,
    }));
    assert!(instructions.contains(&Instruction::Addiu {
        rt: Register::T4,
        rs: Register::T4,
        immediate: 1,
    }));
    assert!(instructions.contains(&Instruction::Sh {
        rt: Register::V0,
        base: Register::A3,
        offset: 0,
    }));
}

#[test]
#[ignore = "requires assets/"]
fn invalid_tags_and_unmapped_legacy_codes_stop_before_the_output_table() {
    let program = build_nickname_companion_program(&consumer_layout()).unwrap();
    let instructions = verify_placed_program(&program.bytes, ROUTINE_ADDRESS).unwrap();

    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::V0,
        rs: Register::ZERO,
        immediate: 0xc000,
    }));
    assert!(instructions.contains(&Instruction::Beq {
        rs: Register::T0,
        rt: Register::V0,
        target: program.finish_address,
    }));
    assert!(instructions.contains(&Instruction::Beq {
        rs: Register::V1,
        rt: Register::T3,
        target: program.finish_address,
    }));
    assert!(instructions.contains(&Instruction::Lhu {
        rt: Register::V0,
        base: Register::A1,
        offset: 0x0482,
    }));
    assert!(program.legacy_scan_address < program.store_address);
    assert!(program.store_address < program.finish_address);
}

#[test]
#[ignore = "requires assets/ and the original disc in roms/"]
fn final_overlay_must_leave_the_legacy_nickname_lookup_unchanged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, source) = read_record(&cue.image_path, OVERLAY_PATH).unwrap();
    let mut patched = source.clone();
    let report =
        install_name_entry_nickname_companion(&source, &mut patched, &consumer_layout()).unwrap();

    assert!(report.legacy_mapping_preserved);
    assert!(verify_nickname_companion_lookup_preserved(&source, &patched).unwrap());

    patched[0x1198] ^= 1;
    assert!(verify_nickname_companion_lookup_preserved(&source, &patched).is_err());

    let mut patched = source.clone();
    install_name_entry_nickname_companion(&source, &mut patched, &consumer_layout()).unwrap();
    patched[0x1363] ^= 1;
    assert!(verify_nickname_companion_lookup_preserved(&source, &patched).is_err());
}

// Native direct-key names must reach the original companion lookup, including
// its high-bit address; tagged slots keep their position-specific cache codes.
#[test]
#[ignore = "requires assets/ and the original disc in roms/"]
fn nickname_companion_executes_direct_mixed_empty_and_invalid_records() {
    use crate::name_input::runtime_test_machine::execute;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, source) = read_record(&cue.image_path, OVERLAY_PATH).unwrap();
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let source =
        super::name_entry_keyboard_build::patch_korean_name_keyboard_overlay(&source, &keyboard)
            .unwrap()
            .bytes;
    let program = build_nickname_companion_program(&consumer_layout()).unwrap();
    let mut cases = vec![
        (
            vec![0x0359, 0x03b8, 0x03c1, 0x03ab, 0x3001],
            vec![0x57, 0x0e, 0x17, 0x0a, 0x3001],
        ),
        (
            vec![0x8000, 0x0359, 0x4030, 0x8001, 0x3001],
            vec![0x339, 0x57, 0x33b, 0x33c, 0x3001],
        ),
        (vec![0x3001], vec![0x3001]),
        (vec![0xc001, 0x3001], vec![0x3001]),
        (vec![0x2222, 0x3001], vec![0x3001]),
    ];
    // The product keyboard changes the code table while retaining the source
    // companion map. Duplicate padding codes resolve to their first entry.
    let mut seen = std::collections::BTreeSet::new();
    for (i, code) in source[0xd14..].as_chunks::<2>().0.iter().enumerate() {
        let code = u16::from_le_bytes(*code);
        if code == 0xffff {
            break;
        }
        if !seen.insert(code) {
            continue;
        }
        let offset = 0x1198 + i * 2;
        let companion = u16::from_le_bytes(source[offset..offset + 2].try_into().unwrap());
        cases.push((vec![code, 0x3001], vec![companion, 0x3001]));
    }
    for (input, expected) in cases {
        let mut memory = vec![0xee; 0x20_0000];
        memory[0x17a000..0x17a000 + source.len()].copy_from_slice(&source);
        // The accidentally sign-extended address must never be consulted.
        memory[0x16ad14..0x16ad16].copy_from_slice(&0xffffu16.to_le_bytes());
        for (i, word) in input.iter().enumerate() {
            memory[0x1f1896 + i * 2..0x1f1898 + i * 2].copy_from_slice(&u16::to_le_bytes(*word));
        }
        let before = memory.clone();
        let mut registers = [0u32; 32];
        registers[4] = 0x801f1864;
        registers[29] = 0x801df000;
        registers[31] = 0x80010000;
        execute(&program.bytes, ROUTINE_ADDRESS, &mut registers, &mut memory);
        let output: Vec<_> = memory[0x1f1886..0x1f1886 + expected.len() * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|x| u16::from_le_bytes(*x))
            .collect();
        assert_eq!(output, expected, "input {input:04x?}");
        assert_eq!(&memory[..0x1f1886], &before[..0x1f1886]);
        let end = 0x1f1886 + expected.len() * 2;
        assert_eq!(&memory[end..], &before[end..]);
        assert_eq!(registers[29], 0x801df000);
        assert_eq!(registers[31], 0x80010000);
    }
}
