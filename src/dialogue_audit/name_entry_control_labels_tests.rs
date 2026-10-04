use super::name_entry_control_labels::{END, OFFSET, build_control_labels};
use super::name_entry_slot_navigation_guard::build_name_entry_slot_navigation_guard_program;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

fn fixture() -> Vec<u8> {
    let words: &[u32] = &[
        0x27bdffb8, 0xafbe0040, 0x0080f021, 0xafb7003c, 0x3c17801c, 0xafbf0044, 0xafb60038,
        0xafb50034, 0xafb40030, 0xafb3002c, 0xafb20028, 0xafb10024, 0xafb00020, 0x93c3000f,
        0x24020002, 0x10620007, 0x36f7a148, 0x3c158018, 0x26b5aaf8, 0x3c148018, 0x2694b5cc,
        0x0806070a, 0x24090007, 0x3c158018, 0x26b5aafc, 0x3c148018, 0x2694b5d2, 0x24090006,
        0xafa90010, 0x8fa90010, 0x00000000, 0x1120005c, 0x0000b021, 0x86a30000, 0x26b50002,
        0x86a80000, 0x26b50002, 0x92860000, 0x26940001, 0x00002021, 0x00002821, 0x00003821,
        0x26d60001, 0x3c02801f, 0x8c42608c, 0x87d20084, 0x87d10086, 0x00063180, 0x000280c0,
        0x02028023, 0x00108080, 0x02f08021, 0x3c02801f, 0x8c426370, 0x02439021, 0x8c420018,
        0x00000000, 0x0040f809, 0x02288821, 0x02002021, 0x00002821, 0x3c03801f, 0x8c636360,
        0x24060001, 0x8c630034, 0x00000000, 0x0060f809, 0x3047ffff, 0x3c02801f, 0x8c426360,
        0x26130008, 0x8c420068, 0x00000000, 0x0040f809, 0x02602021, 0x92860000, 0x26940001,
        0x3c02801f, 0x8c426370, 0x240501e3, 0x8c420014, 0x00000000, 0x0040f809, 0x00062100,
        0xa6020016, 0x24020080, 0xa202000c, 0xa202000d, 0xa202000e, 0x92860000, 0x26940001,
        0x92840000, 0x26940001, 0x92830000, 0x26940001, 0x92820000, 0x26f70038, 0xa6120010,
        0xa6110012, 0xa2060014, 0xa2040015, 0xa6030018, 0xa602001a, 0x3c02801f, 0x8c426360,
        0x02602821, 0x8c420020, 0x00000000, 0x0040f809, 0x02002021, 0x02002821, 0x3c02801f,
        0x8c426360, 0x3c04801f, 0x8c846090, 0x8c420000, 0x00000000, 0x0040f809, 0x24841070,
        0x8fa90010, 0x00000000, 0x02c9102a, 0x1440ffa6, 0x26940001, 0x8fbf0044, 0x8fbe0040,
        0x8fb7003c, 0x8fb60038, 0x8fb50034, 0x8fb40030, 0x8fb3002c, 0x8fb20028, 0x8fb10024,
        0x8fb00020, 0x27bd0048, 0x03e00008, 0x00000000,
    ];
    let mut source = vec![0; END];
    for (i, w) in words.iter().enumerate() {
        source[OFFSET + i * 4..OFFSET + i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    source
}

fn render(code: &[u8], field: u8, page: u8, slot: u8, value: u16) -> Vec<Vec<u8>> {
    const ORIGIN: u32 = 0x8018129c;
    let mut program = vec![0; (0x80181ddc - ORIGIN) as usize];
    let guard = build_name_entry_slot_navigation_guard_program().unwrap();
    program[8..8 + guard.bytes.len()].copy_from_slice(&guard.bytes);
    let start = (0x80181bb8 - ORIGIN) as usize;
    program[start..start + code.len()].copy_from_slice(code);
    program[..4].copy_from_slice(&0x080606eeu32.to_le_bytes()); // J 80181bb8
    let mut memory = vec![0; 0x200000];
    let write32 = |m: &mut [u8], at: usize, v: u32| m[at..at + 4].copy_from_slice(&v.to_le_bytes());
    write32(&mut memory, 0x1f6360, 0x801e0000);
    write32(&mut memory, 0x1f6370, 0x801e0200);
    write32(&mut memory, 0x1f6090, 0x801e0400);
    for (offset, pc) in [
        (0, 0x801e1000),
        (0x20, 0x801e1004),
        (0x34, 0x801e1008),
        (0x68, 0x801e100c),
    ] {
        write32(&mut memory, 0x1e0000 + offset, pc);
    }
    write32(&mut memory, 0x1e0200 + 0x14, 0x801e1010);
    write32(&mut memory, 0x1e0200 + 0x18, 0x801e1014);
    for (i, y) in [200u16, 225, 250, 325, 350, 375, 400].iter().enumerate() {
        memory[0x17aaf8 + i * 4..0x17aafa + i * 4].copy_from_slice(&61u16.to_le_bytes());
        memory[0x17aafa + i * 4..0x17aafc + i * 4].copy_from_slice(&y.to_le_bytes());
    }
    let descriptors: [[u8; 6]; 7] = [
        [14, 18, 48, 200, 48, 24],
        [14, 18, 96, 200, 48, 24],
        [14, 18, 144, 200, 48, 24],
        [14, 19, 0, 224, 48, 24],
        [14, 19, 48, 224, 48, 24],
        [14, 19, 96, 224, 48, 24],
        [14, 17, 144, 224, 48, 24],
    ];
    for (i, d) in descriptors.iter().enumerate() {
        memory[0x17b5cc + i * 6..0x17b5d2 + i * 6].copy_from_slice(d);
    }
    memory[0x1d0009] = page;
    memory[0x1d000a] = slot;
    memory[0x1d000f] = field;
    let at = 0x1d0012 + field as usize * 16 + slot as usize * 2;
    memory[at..at + 2].copy_from_slice(&value.to_le_bytes());
    let mut r = [0u32; 32];
    for i in 16..24 {
        r[i] = 0xbeef0000 + i as u32;
    }
    r[30] = 0xbeef0030;
    r[4] = 0x801d0000;
    r[29] = 0x801fe000;
    r[31] = 0x800f0000;
    let saved = r;
    let mut packets = Vec::new();
    execute_with_callbacks(
        &program,
        ORIGIN,
        &mut r,
        &mut memory,
        None,
        &mut |pc, r, m| {
            if !(0x801e1000..=0x801e1014).contains(&pc) {
                return false;
            }
            let result = if pc == 0x801e1010 {
                0x600 + r[4]
            } else {
                0x123
            };
            if pc == 0x801e1000 {
                assert_eq!(r[4], 0x801e1470);
                let at = (r[5] & 0x1fffffff) as usize;
                packets.push(m[at..at + 28].to_vec());
            }
            if pc == 0x801e1004 {
                assert_eq!(r[5], r[4] + 8);
            }
            for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                r[reg] = 0xdead0000 + reg as u32;
            }
            let sp = (r[29] & 0x1fffffff) as usize;
            m[sp..sp + 16].fill(0xa5);
            r[2] = result;
            true
        },
    );
    for reg in 16..24 {
        assert_eq!(r[reg], saved[reg]);
    }
    assert_eq!(r[29], saved[29]);
    assert_eq!(r[30], saved[30]);
    assert_eq!(r[31], saved[31]);
    packets
}

#[test]
fn name_entry_control_labels_preserve_native_packets_except_incomplete_slot_cue() {
    let source = fixture();
    let (patched, instructions) = build_control_labels(&source).unwrap();
    assert_eq!(patched.len(), END - OFFSET);
    assert_eq!(instructions.len() * 4, patched.len());
    let mut original = source[OFFSET..END].to_vec();
    // Previously adopted all-fields Hangul label visibility.
    original[0x7bf4 - OFFSET..0x7bf8 - OFFSET].fill(0);
    for field in 0..3 {
        for page in 0..3 {
            for slot in [0, if field == 2 { 3 } else { 5 }] {
                for value in [0x0fff, 0x8000, 0x81a4, 0x0359, 0xc000, 0xc012] {
                    let mut expected = render(&original, field, page, slot, value);
                    assert_eq!(expected.len(), 7);
                    if value & 0xc000 == 0xc000 {
                        expected[0][0x14] = 192;
                    }
                    assert_eq!(
                        render(&patched, field, page, slot, value),
                        expected,
                        "field {field} page {page} slot {slot} value {value:x}"
                    );
                }
            }
        }
    }
}

#[test]
fn name_entry_control_labels_reject_changed_source() {
    let mut source = fixture();
    source[OFFSET] ^= 1;
    assert!(build_control_labels(&source).is_err());
}
