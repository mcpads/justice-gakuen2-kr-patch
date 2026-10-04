use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

#[test]
fn battle_loader_keeps_font_cells_and_legacy_prefix_through_native_loads() {
    let entry = BATTLE_NAME_LOAD_DESTINATION + BATTLE_NAME_LEGACY_STORAGE_BYTES as u32;
    let supplier_len = BATTLE_NAME_LEGACY_STORAGE_BYTES + 4096;
    let code = build_battle_name_loader(supplier_len, entry, 229168).unwrap();
    let off = |a: u32| (a & 0x1fff_ffff) as usize;
    for enabled in [false, true] {
        let mut memory = vec![0x55; 0x200000];
        memory[0x9a978..0x9a97c].copy_from_slice(&u32::from(enabled).to_le_bytes());
        let before = memory.clone();
        let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
        r[0] = 0;
        r[29] = 0x801fe000;
        r[31] = 0x80027f24;
        let saved = r;
        let mut calls = Vec::new();
        execute_with_callbacks(
            &code,
            BATTLE_NAME_GENERATOR_ORIGIN,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, m| {
                if ![NATIVE_LOADER, 0x80060058, 0x80060008, 0x80060068, entry].contains(&pc) {
                    return false;
                }
                calls.push(pc);
                match calls.len() {
                    1 => {
                        assert_eq!(
                            (pc, r[4], r[5]),
                            (NATIVE_LOADER, BATTLE_NAME_LOAD_DESTINATION, 718)
                        );
                        m[off(r[4])..off(r[4]) + 229168].fill(0x37);
                    }
                    2 => {
                        assert_eq!(
                            (pc, r[4], r[5]),
                            (NATIVE_LOADER, BATTLE_NAME_LOAD_DESTINATION, 49)
                        );
                        m[off(r[4])..off(r[4]) + supplier_len].fill(0xc9);
                    }
                    3 => assert_eq!(pc, 0x80060058),
                    4 => assert_eq!(pc, 0x80060008),
                    5 => assert_eq!(pc, 0x80060068),
                    6 => {
                        assert_eq!(pc, entry);
                        assert!(
                            m[off(BATTLE_NAME_LOAD_DESTINATION)
                                ..off(BATTLE_NAME_LOAD_DESTINATION) + supplier_len]
                                .iter()
                                .all(|&b| b == 0xc9)
                        );
                        assert!(
                            m[off(BATTLE_NAME_FONT_PIXELS)
                                ..off(BATTLE_NAME_LOAD_DESTINATION) + 229168]
                                .iter()
                                .all(|&b| b == 0x37)
                        );
                    }
                    _ => panic!("unexpected native call"),
                }
                for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                    r[reg] = 0xdead0000 + reg as u32;
                }
                let sp = off(r[29]);
                m[sp..sp + 16].fill(0xcc);
                true
            },
        );
        assert_eq!(calls.len(), if enabled { 6 } else { 0 });
        for reg in (16..24).chain([28, 29, 30, 31]) {
            assert_eq!(r[reg], saved[reg]);
        }
        let mut expected = before;
        if enabled {
            let start = off(BATTLE_NAME_LOAD_DESTINATION);
            expected[start..start + 229168].fill(0x37);
            expected[start..start + supplier_len].fill(0xc9);
            let sp = off(saved[29]);
            expected[sp - 24..sp].copy_from_slice(&memory[sp - 24..sp]);
        }
        assert_eq!(
            memory, expected,
            "loader changed memory outside native destinations and its stack"
        );
    }
}

#[test]
fn battle_loader_rejects_entries_outside_payload_and_font_overlap() {
    let entry = BATTLE_NAME_LOAD_DESTINATION + BATTLE_NAME_LEGACY_STORAGE_BYTES as u32;
    for (len, target) in [
        (BATTLE_NAME_LEGACY_STORAGE_BYTES, entry),
        (BATTLE_NAME_LEGACY_STORAGE_BYTES + 4, entry + 4),
        (BATTLE_NAME_LEGACY_STORAGE_BYTES + 8, entry + 1),
        (BATTLE_NAME_LEGACY_STORAGE_BYTES + 8, entry - 4),
        (
            (BATTLE_NAME_FONT_PIXELS - BATTLE_NAME_LOAD_DESTINATION) as usize + 4,
            entry,
        ),
        (usize::MAX, entry),
    ] {
        assert!(build_battle_name_loader(len, target, 229168).is_err());
    }
}
