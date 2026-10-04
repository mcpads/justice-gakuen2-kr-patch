use super::*;
use crate::name_input::runtime_test_machine::execute_with_callbacks;

fn fixture() -> SelectorBootstrapLayout {
    // Probe addresses; the product installer has not admitted these allocations.
    SelectorBootstrapLayout {
        origin: 0x800dc340,
        byte_capacity: 192,
        payload_source: 0x800dc400,
        payload_destination: 0x800a8000,
        payload_byte_count: 392,
        payload_split: None,
        enter_critical_address: 0x80060058,
        flush_cache_address: 0x80060008,
        exit_critical_address: 0x80060068,
    }
}

#[test]
fn relocation_flushes_before_entry_and_retains_disabled_interrupts() {
    for origin in [0x800dc340, 0xa00dc340] {
        for initially_enabled in [false, true] {
            for count in [4, 392, 4096] {
                let splits = if count > 4 {
                    vec![None, Some(4), Some(count - 4)]
                } else {
                    vec![None]
                };
                for split in splits {
                    let layout = SelectorBootstrapLayout {
                        payload_byte_count: count,
                        payload_split: split.map(|first_byte_count| SelectorPayloadSplit {
                            first_byte_count,
                            second_source: 0x800f0000,
                        }),
                        origin,
                        ..fixture()
                    };
                    let bytes = build_selector_bootstrap(&layout).unwrap();
                    let offset = |address: u32| (address & 0x1fffffff) as usize;
                    let mut memory = vec![0x55; 0x200000];
                    let payload: Vec<_> = (0..count).map(|i| (i * 37 + i / 4) as u8).collect();
                    let source = offset(layout.payload_source);
                    let destination = offset(layout.payload_destination);
                    let first = split.unwrap_or(count);
                    memory[source..source + first].copy_from_slice(&payload[..first]);
                    if split.is_some() {
                        memory[0xf0000..0xf0000 + count - first].copy_from_slice(&payload[first..]);
                    }
                    let before = memory.clone();
                    let mut r = std::array::from_fn(|i| 0x12340000 + i as u32);
                    r[0] = 0;
                    r[29] = 0x801fe000;
                    r[31] = 0x800a2bb0;
                    let saved = r;
                    let mut enabled = initially_enabled;
                    let mut calls = Vec::new();
                    execute_with_callbacks(
                        &bytes,
                        layout.origin,
                        &mut r,
                        &mut memory,
                        None,
                        &mut |pc, r, m| {
                            if pc == layout.payload_destination {
                                assert_eq!(enabled, initially_enabled);
                                assert_eq!(r[29], saved[29]);
                                assert_eq!(r[31], saved[31]);
                                assert_eq!(&m[destination..destination + count], payload);
                                assert_eq!(
                                    calls,
                                    if initially_enabled {
                                        vec![
                                            layout.enter_critical_address,
                                            layout.flush_cache_address,
                                            layout.exit_critical_address,
                                        ]
                                    } else {
                                        vec![
                                            layout.enter_critical_address,
                                            layout.flush_cache_address,
                                        ]
                                    }
                                );
                                calls.push(pc);
                                return true;
                            }
                            if ![
                                layout.enter_critical_address,
                                layout.flush_cache_address,
                                layout.exit_critical_address,
                            ]
                            .contains(&pc)
                            {
                                return false;
                            }
                            assert_eq!(
                                &m[destination..destination + count],
                                payload,
                                "flush before complete copy"
                            );
                            calls.push(pc);
                            for register in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25]
                            {
                                r[register] = 0xdead0000 + register as u32;
                            }
                            let sp = offset(r[29]);
                            m[sp..sp + 16].fill(0xcc);
                            if pc == layout.enter_critical_address {
                                r[2] = u32::from(enabled);
                                enabled = false;
                            } else if pc == layout.flush_cache_address {
                                assert!(!enabled, "cache flush must run with IRQs disabled");
                            } else {
                                assert!(initially_enabled);
                                enabled = true;
                            }
                            true
                        },
                    );
                    assert_eq!(calls.last(), Some(&layout.payload_destination));
                    for register in (16..24).chain([28, 29, 30, 31]) {
                        assert_eq!(r[register], saved[register]);
                    }
                    let mut expected = before;
                    expected[destination..destination + count].copy_from_slice(&payload);
                    let stack = offset(saved[29]);
                    expected[stack - 24..stack].copy_from_slice(&memory[stack - 24..stack]);
                    assert_eq!(
                        memory, expected,
                        "unexpected write outside relocated code and owned stack"
                    );
                }
            }
        }
    }
}

#[test]
fn rejects_bootstrap_aliases_empty_unaligned_and_overflowing_payloads() {
    let good = fixture();
    for bad in [
        SelectorBootstrapLayout {
            origin: 0xa0000000 | (good.payload_source & 0x1fffffff),
            ..good.clone()
        },
        SelectorBootstrapLayout {
            origin: 0xa01ffffc,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            origin: 0xc00dc340,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_byte_count: 0,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_byte_count: 3,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_byte_count: usize::MAX,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_destination: good.payload_source,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_destination: good.origin,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_source: good.origin,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_source: 0x801ffffc,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            payload_destination: good.payload_destination + 2,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            flush_cache_address: good.payload_destination,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            exit_critical_address: good.enter_critical_address,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            byte_capacity: 4,
            ..good.clone()
        },
        SelectorBootstrapLayout {
            byte_capacity: usize::MAX,
            ..good
        },
    ] {
        assert!(build_selector_bootstrap(&bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn selector_entry_reaches_uncached_staging_and_returns_after_displaced_setup() {
    let source: Vec<u8> = [0x3c02801fu32, 0x8c426360, 0x3c04800d, 0x8c420150]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let target = 0xa00dc340;
    let bytes = build_selector_bootstrap_entry(&source, target).unwrap();
    let mut r = std::array::from_fn(|i| 0x10000000 + i as u32);
    r[0] = 0;
    r[29] = 0x801fe000;
    r[31] = 0x800a4000;
    let saved = r;
    let mut memory = vec![0x55; 0x200000];
    let mut entered = false;
    execute_with_callbacks(
        &bytes,
        SELECTOR_BOOTSTRAP_CALL_SITE,
        &mut r,
        &mut memory,
        None,
        &mut |pc, r, _| {
            if pc != target {
                return false;
            }
            assert_eq!(r[31], SELECTOR_BOOTSTRAP_RETURN);
            entered = true;
            r[31] = saved[31];
            true
        },
    );
    assert!(entered);
    for reg in 0..32 {
        if reg != 14 {
            assert_eq!(r[reg], saved[reg]);
        }
    }
    assert!(memory.iter().all(|&b| b == 0x55));
    for i in 0..source.len() {
        let mut changed = source.clone();
        changed[i] ^= 1;
        assert!(build_selector_bootstrap_entry(&changed, target).is_err());
    }
    for target in [0x800dc340, 0xa00dc342, 0xa0200000, 0xc00dc340] {
        assert!(build_selector_bootstrap_entry(&source, target).is_err());
    }
}

#[test]
fn rejects_split_sources_that_alias_dependencies_or_leave_ram() {
    let base = fixture();
    for (count, source) in [
        (0, 0x800f0000),
        (392, 0x800f0000),
        (3, 0x800f0000),
        (4, 0x800f0002),
        (4, base.payload_source),
        (4, base.origin),
        (4, base.payload_destination),
        (4, base.flush_cache_address),
        (4, 0x801ffffc),
    ] {
        let layout = SelectorBootstrapLayout {
            payload_split: Some(SelectorPayloadSplit {
                first_byte_count: count,
                second_source: source,
            }),
            ..base.clone()
        };
        assert!(
            build_selector_bootstrap(&layout).is_err(),
            "accepted {layout:?}"
        );
    }
}

#[test]
fn split_ascii_is_relocated_before_the_loader_overwrites_staging() {
    let layout = SelectorBootstrapLayout {
        origin: 0xa00dc340,
        byte_capacity: 0x1c0,
        payload_source: 0x800eb020,
        payload_byte_count: 64,
        ..fixture()
    };
    let copies = [
        SelectorDataCopy {
            source: 0x800d4050,
            destination: 0x800b4c00,
            byte_count: 1968,
        },
        SelectorDataCopy {
            source: 0x800dc500,
            destination: 0x800b53b0,
            byte_count: 316,
        },
    ];
    let code = build_selector_bootstrap_with_data(&layout, &copies).unwrap();
    let mut memory = vec![0x55; 0x200000];
    let ascii: Vec<u8> = (0..2284).map(|i| (i * 37 + i / 7) as u8).collect();
    memory[0xd4050..0xd4800].copy_from_slice(&ascii[..1968]);
    memory[0xdc500..0xdc63c].copy_from_slice(&ascii[1968..]);
    let mut r = [0; 32];
    r[29] = 0x801fe000;
    r[31] = 0x800a2bb0;
    let mut loaded = false;
    execute_with_callbacks(
        &code,
        layout.origin,
        &mut r,
        &mut memory,
        None,
        &mut |pc, r, m| {
            if pc == layout.payload_destination {
                // The staged resource is then replaced by the native font load.
                m[0xc0000..0xf3800].fill(0x99);
                assert_eq!(&m[0xb4c00..0xb54ec], &ascii);
                loaded = true;
                return true;
            }
            if [
                layout.enter_critical_address,
                layout.flush_cache_address,
                layout.exit_critical_address,
            ]
            .contains(&pc)
            {
                assert_eq!(&m[0xb4c00..0xb54ec], &ascii);
                r[2] = 0;
                return true;
            }
            false
        },
    );
    assert!(loaded);
    for bad in [
        SelectorDataCopy {
            source: 0x800eb020,
            ..copies[0].clone()
        },
        SelectorDataCopy {
            destination: layout.payload_destination,
            ..copies[0].clone()
        },
        SelectorDataCopy {
            destination: 0x80060008,
            ..copies[0].clone()
        },
        SelectorDataCopy {
            destination: 0x800dc340,
            ..copies[0].clone()
        },
        SelectorDataCopy {
            byte_count: 0,
            ..copies[0].clone()
        },
        SelectorDataCopy {
            byte_count: 3,
            ..copies[0].clone()
        },
    ] {
        assert!(build_selector_bootstrap_with_data(&layout, &[bad]).is_err());
    }
    let cross = SelectorDataCopy {
        destination: copies[0].source,
        ..copies[1].clone()
    };
    assert!(build_selector_bootstrap_with_data(&layout, &[copies[0].clone(), cross]).is_err());
}
