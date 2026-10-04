use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

fn layout() -> RosterAdapterLayout {
    RosterAdapterLayout {
        origin: 0x800b7000,
        capacity: 384,
        renderer: 0x800af000,
        legacy: 0x800a4dc8,
        next: 0x800a4f30,
        packet_base: 0x48,
        packet_extra: None,
        packet_row: None,
        length: R::S2,
        x: RosterValue::StackWord(0x20),
        y: RosterValue::StackWord(0x60),
        y_bias: 73,
        ot_offset: 0x1068,
    }
}

#[test]
fn roster_adapters_keep_native_context_for_both_buffers_and_all_name_lengths() {
    let layouts = [
        layout(),
        RosterAdapterLayout {
            packet_base: 0x28,
            packet_row: Some([0x48, 0x18]),
            length: R::S3,
            y: RosterValue::StackHalf(0x50),
            y_bias: 0,
            ot_offset: 0x1060,
            ..layout()
        },
        RosterAdapterLayout {
            packet_base: 0x38,
            packet_extra: Some(0x50),
            length: R::S3,
            x: RosterValue::Register(R::S7),
            y: RosterValue::Register(R::FP),
            y_bias: 0,
            ..layout()
        },
        RosterAdapterLayout {
            packet_base: 0x78,
            length: R::S3,
            x: RosterValue::Register(R::FP),
            y: RosterValue::Constant(0x60),
            ..layout()
        },
    ];
    for (variant, l) in layouts.into_iter().enumerate() {
        let code = build_roster_adapter(&l).unwrap();
        for frame in 0..2u32 {
            for length in 1..=4u32 {
                for index in 0..length {
                    let mut memory = vec![0x55; 0x200000];
                    let sp = 0x1fe000;
                    let mut put =
                        |p: usize, v: u32| memory[p..p + 4].copy_from_slice(&v.to_le_bytes());
                    put(0x1f608c, frame);
                    put(0x1f6090, 0x801d0000);
                    put(sp + 0x18, 1);
                    put(sp + 0x20, 60);
                    put(sp + 0x28, 0x801c0000);
                    put(sp + 0x38, 0x801c0000);
                    put(sp + 0x48, if variant == 1 { 3 } else { 0x801c0000 });
                    put(sp + 0x50, 120);
                    put(sp + 0x60, 120);
                    put(sp + 0x78, 0x801c0000);
                    let before = memory.clone();
                    let mut r = std::array::from_fn(|i| i as u32 * 41);
                    r[0] = 0;
                    r[18] = length;
                    r[19] = length;
                    r[20] = 0x8000 + index;
                    r[21] = index * 56;
                    r[22] = index * 14;
                    r[23] = 60;
                    r[30] = if variant == 3 { 60 } else { 120 };
                    r[29] = 0x801fe000;
                    r[31] = l.next;
                    let saved = r;
                    let mut calls = 0;
                    execute_with_callbacks(
                        &code,
                        l.origin,
                        &mut r,
                        &mut memory,
                        None,
                        &mut |pc, r, m| {
                            if pc != l.renderer {
                                return false;
                            }
                            calls += 1;
                            assert_eq!(r[4], saved[20]);
                            let extra = if variant == 1 {
                                64
                            } else if variant == 2 {
                                120
                            } else {
                                0
                            };
                            assert_eq!(r[5], 0x801c0000 + extra + index * 56 + frame * 28);
                            let x = 60i32 + (48 - length as i32 * 14) / 2 + index as i32 * 14;
                            let y = match variant {
                                0 => 193,
                                3 => 169,
                                _ => 120,
                            };
                            assert_eq!(r[6], (y << 16) | x as u32);
                            assert_eq!(r[7], 0x801d0000 + l.ot_offset as u32);
                            for reg in (2..16).chain([24, 25]) {
                                r[reg] = 0xdead0000 + reg as u32;
                            }
                            let at = (r[29] & 0x1fffffff) as usize;
                            m[at..at + 16].fill(0xa5);
                            true
                        },
                    );
                    assert_eq!(calls, 1);
                    for reg in (16..24).chain([28, 29, 30, 31]) {
                        assert_eq!(r[reg], saved[reg]);
                    }
                    assert_eq!(&memory[..sp - 24], &before[..sp - 24]);
                    assert_eq!(&memory[sp..], &before[sp..]);
                }
            }
        }
    }
}

#[test]
fn roster_adapter_preserves_the_native_legacy_branch() {
    let l = layout();
    let code = build_roster_adapter(&l).unwrap();
    for word in 0..256u32 {
        let mut memory = vec![0x55; 0x200000];
        let before = memory.clone();
        let mut r = std::array::from_fn(|i| i as u32 * 31);
        r[0] = 0;
        r[20] = word;
        r[31] = l.legacy;
        let mut expected = r;
        expected[2] = 1;
        expected[5] = 0;
        execute_with_callbacks(
            &code,
            l.origin,
            &mut r,
            &mut memory,
            None,
            &mut |_, _, _| false,
        );
        assert_eq!(r, expected);
        assert_eq!(memory, before);
    }
}
