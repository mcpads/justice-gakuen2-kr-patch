use super::*;

// A dense inverse of the product keyboard codes; zero entries are not direct keys.
// This table follows the packed fixed glyphs in the already owned staging and
// persistent regions and is copied with them before either import hook runs.
pub(super) struct ImportedDirectNameMap {
    pub base: u16,
    pub bytes: Vec<u8>,
}

pub(super) const DIRECT_NAME_MAP_OFFSET: usize =
    FIXED_GLYPH_PAYLOAD_BYTE_CAPACITY + KANRI_RAW_ASCII_GLYPH_COUNT * KANRI_GLYPH_PAYLOAD_BYTES;
pub(super) const DIRECT_NAME_MAP_ADDRESS: u32 =
    STATIC_HANGUL_PAYLOAD_PERSISTENT_ORIGIN + DIRECT_NAME_MAP_OFFSET as u32;

impl ImportedDirectNameMap {
    pub fn build(layout: &NameGlyphConsumerLayout) -> Result<Self> {
        let keys = LATIN_KEYS
            .chars()
            .chain(DIGIT_KEYS.chars())
            .chain(SYMBOL_KEYS.chars())
            .map(|c| {
                Ok((
                    layout
                        .code_for_active_character(c)
                        .with_context(|| format!("KANRI import lost direct key {c:?}"))?,
                    c as u8,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let base = keys
            .iter()
            .map(|x| x.0)
            .min()
            .context("empty direct keyboard")?;
        let end = keys.iter().map(|x| x.0).max().unwrap();
        let mut bytes = vec![0; (usize::from(end - base) + 1).next_multiple_of(4)];
        for (code, character) in keys {
            let slot = &mut bytes[usize::from(code - base)];
            ensure!(*slot == 0, "KANRI direct keyboard codes overlap");
            *slot = character;
        }
        ensure!(
            DIRECT_NAME_MAP_OFFSET + bytes.len() <= STATIC_HANGUL_PAYLOAD_STAGING_BYTE_CAPACITY
                && DIRECT_NAME_MAP_OFFSET + bytes.len()
                    <= STATIC_HANGUL_PAYLOAD_PERSISTENT_BYTE_CAPACITY,
            "KANRI direct import table exceeds owned storage"
        );
        Ok(Self { base, bytes })
    }
}

// The native card buffer keeps the tagged nickname 16 bytes after its legacy
// display companion. Restore tagged and direct-key slots from the canonical
// record; other legacy words still need their original companion conversion.
// Bound the imported representation to the four native nickname slots.
pub(super) fn emit_imported_name_normalizers(
    a: &mut Assembler,
    origin: u32,
    direct: &ImportedDirectNameMap,
) -> Result<(u32, u32)> {
    use Instruction::*;
    let card = origin + u32::try_from(a.assemble(origin)?.bytes().len())?;
    a.emit(Addiu {
        rt: Register::A1,
        rs: Register::S5,
        immediate: 0x86,
    })
    .jump("normalize_imported_name")
    .emit(Instruction::nop());
    let registered = card + 12;
    // This hook replaces a load in a leaf copy routine. Preserve its caller's
    // return address and argument; the original shift still runs in the hook's
    // delay slot. Return directly after that pair with the original load result.
    a.emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -16,
    })
    .emit(Sw {
        rt: Register::RA,
        base: Register::SP,
        offset: 12,
    })
    .emit(Sw {
        rt: Register::A1,
        base: Register::SP,
        offset: 8,
    })
    .call("normalize_imported_name")
    .emit(Addiu {
        rt: Register::A1,
        rs: Register::A0,
        immediate: 0x86,
    })
    .emit(Lw {
        rt: Register::RA,
        base: Register::SP,
        offset: 12,
    })
    .emit(Lw {
        rt: Register::A1,
        base: Register::SP,
        offset: 8,
    })
    .emit(Lhu {
        rt: Register::V1,
        base: Register::A0,
        offset: 0x86,
    })
    .emit(J {
        target: OVERLAY_RUNTIME_BASE + REGISTER_NAME_LOAD_OFFSET as u32 + 8,
    })
    .emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: 16,
    });
    a.label("normalize_imported_name").emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -16,
    });
    for (r, offset) in [
        (Register::T0, 0),
        (Register::T1, 4),
        (Register::T2, 8),
        (Register::T3, 12),
    ] {
        a.emit(Sw {
            rt: r,
            base: Register::SP,
            offset,
        });
    }
    a.emit(Addu {
        rd: Register::T0,
        rs: Register::A1,
        rt: Register::ZERO,
    })
    .emit(Addiu {
        rt: Register::T3,
        rs: Register::A1,
        immediate: 8,
    })
    .label("normalize_imported_name_slot")
    .emit(Lhu {
        rt: Register::T1,
        base: Register::T0,
        offset: 16,
    })
    .emit(Addiu {
        rt: Register::T0,
        rs: Register::T0,
        immediate: 2,
    })
    .emit(Andi {
        rt: Register::T2,
        rs: Register::T1,
        immediate: TAG_MASK,
    })
    .beq(
        Register::T2,
        Register::ZERO,
        "normalize_imported_direct_name",
    )
    .emit(Xori {
        rt: Register::T2,
        rs: Register::T2,
        immediate: TAG_MASK,
    })
    .beq(Register::T2, Register::ZERO, "normalize_imported_name_next")
    .emit(Instruction::nop())
    .label("normalize_imported_name_store")
    .emit(Sh {
        rt: Register::T1,
        base: Register::T0,
        offset: -2,
    })
    .label("normalize_imported_name_next")
    .bne(Register::T0, Register::T3, "normalize_imported_name_slot")
    .emit(Instruction::nop());
    a.emit(Ori {
        rt: Register::T1,
        rs: Register::ZERO,
        immediate: MESSAGE_END_CODE,
    })
    .emit(Sh {
        rt: Register::T1,
        base: Register::T0,
        offset: 0,
    })
    .jump("normalize_imported_name_return")
    .emit(Instruction::nop())
    .label("normalize_imported_direct_name")
    .emit(Ori {
        rt: Register::T2,
        rs: Register::ZERO,
        immediate: MESSAGE_END_CODE,
    })
    .beq(Register::T1, Register::T2, "normalize_imported_name_store")
    .emit(Instruction::nop())
    .emit(Addiu {
        rt: Register::T2,
        rs: Register::T1,
        immediate: -(direct.base as i16),
    })
    .emit(Sltiu {
        rt: Register::T1,
        rs: Register::T2,
        immediate: i16::try_from(direct.bytes.len())?,
    })
    .beq(Register::T1, Register::ZERO, "normalize_imported_name_next")
    .emit(Instruction::nop())
    .emit_all(load_address(Register::T1, DIRECT_NAME_MAP_ADDRESS))
    .emit(Addu {
        rd: Register::T2,
        rs: Register::T2,
        rt: Register::T1,
    })
    .emit(Lbu {
        rt: Register::T1,
        base: Register::T2,
        offset: 0,
    })
    .emit(Instruction::nop())
    .beq(Register::T1, Register::ZERO, "normalize_imported_name_next")
    .emit(Ori {
        rt: Register::T1,
        rs: Register::T1,
        immediate: ASCII_NAME_TAG,
    })
    .jump("normalize_imported_name_store")
    .emit(Instruction::nop())
    .label("normalize_imported_name_return");
    for (r, offset) in [
        (Register::T0, 0),
        (Register::T1, 4),
        (Register::T2, 8),
        (Register::T3, 12),
    ] {
        a.emit(Lw {
            rt: r,
            base: Register::SP,
            offset,
        });
    }
    a.emit(Jr { rs: Register::RA }).emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: 16,
    });
    Ok((card, registered))
}

// The Diary preparation path copies a display companion into temporary record
// 16. Normalize a stack-local copy, leaving the live Diary record untouched.
pub(super) fn emit_diary_name_transfer(
    a: &mut Assembler,
    origin: u32,
    normalizer: u32,
) -> Result<u32> {
    use Instruction::*;
    let entry = origin + u32::try_from(a.assemble(origin)?.bytes().len())?;
    a.emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -48,
    })
    .emit(Sw {
        rt: Register::RA,
        base: Register::SP,
        offset: 44,
    })
    .emit_all(load_address(Register::T0, PRIMARY_NICKNAME_ADDRESS - 16));
    for offset in (0..10).step_by(2) {
        a.emit(Lhu {
            rt: Register::T1,
            base: Register::T0,
            offset,
        })
        .emit(Lhu {
            rt: Register::T2,
            base: Register::T0,
            offset: offset + 16,
        })
        .emit(Sh {
            rt: Register::T1,
            base: Register::SP,
            offset,
        })
        .emit(Sh {
            rt: Register::T2,
            base: Register::SP,
            offset: offset + 16,
        });
    }
    a.emit(Sw {
        rt: Register::S5,
        base: Register::SP,
        offset: 40,
    })
    .emit(Jal { target: normalizer })
    .emit(Addiu {
        rt: Register::S5,
        rs: Register::SP,
        immediate: -0x86,
    })
    .emit(Lw {
        rt: Register::S5,
        base: Register::SP,
        offset: 40,
    });
    for (i, r) in [
        Register::V1,
        Register::A0,
        Register::A1,
        Register::A2,
        Register::A3,
    ]
    .into_iter()
    .enumerate()
    {
        a.emit(Lhu {
            rt: r,
            base: Register::SP,
            offset: i as i16 * 2,
        });
    }
    a.emit(Lw {
        rt: Register::RA,
        base: Register::SP,
        offset: 44,
    })
    .emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: 48,
    })
    .emit(J {
        target: OVERLAY_RUNTIME_BASE + 0x6120,
    })
    .emit(Instruction::nop());
    Ok(entry)
}

#[cfg(test)]
mod imported_name_tests {
    use super::*;

    // Execute the emitted hook bytes, including branch delay slots. Load-delay
    // legality is independently checked by place_program's R3000A verifier.
    fn execute(bytes: &[u8], origin: u32, entry: u32, stop: u32, r: &mut [u32; 32], m: &mut [u8]) {
        let mut pc = entry;
        let mut delayed = None;
        let mut pending_load = None;
        for _ in 0..400 {
            if pc == stop {
                return;
            }
            let o = (pc - origin) as usize;
            let w = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            let rs = ((w >> 21) & 31) as usize;
            let rt = ((w >> 16) & 31) as usize;
            let rd = ((w >> 11) & 31) as usize;
            let imm = w as u16 as i16 as i32 as u32;
            let addr = (r[rs].wrapping_add(imm) & 0x1fffffff) as usize;
            let prior = delayed.take();
            // Sources of this instruction see the register file before the
            // preceding load commits. All emitted load hazards are rejected
            // separately by the R3000A placement verifier.
            let previous_load = pending_load.take();
            match w >> 26 {
                0 => match w & 63 {
                    0 => r[rd] = r[rt] << ((w >> 6) & 31),
                    8 => delayed = Some(r[rs]),
                    33 => r[rd] = r[rs].wrapping_add(r[rt]),
                    _ => panic!("unsupported emitted instruction {w:08x}"),
                },
                2 | 3 => {
                    if w >> 26 == 3 {
                        r[31] = pc + 8;
                    }
                    delayed = Some(((pc + 4) & 0xf0000000) | ((w & 0x3ffffff) << 2));
                }
                4 | 5 => {
                    if (r[rs] == r[rt]) == (w >> 26 == 4) {
                        delayed = Some((pc + 4).wrapping_add(imm.wrapping_mul(4)));
                    }
                }
                9 => r[rt] = r[rs].wrapping_add(imm),
                11 => r[rt] = u32::from(r[rs] < imm),
                13 => r[rt] = r[rs] | (w & 65535),
                15 => r[rt] = (w & 65535) << 16,
                36 => pending_load = Some((rt, u32::from(m[addr]))),
                12 => r[rt] = r[rs] & (w & 65535),
                14 => r[rt] = r[rs] ^ (w & 65535),
                35 => {
                    pending_load = Some((
                        rt,
                        u32::from_le_bytes(m[addr..addr + 4].try_into().unwrap()),
                    ))
                }
                37 => {
                    pending_load = Some((
                        rt,
                        u16::from_le_bytes(m[addr..addr + 2].try_into().unwrap()) as u32,
                    ))
                }
                41 => m[addr..addr + 2].copy_from_slice(&(r[rt] as u16).to_le_bytes()),
                43 => m[addr..addr + 4].copy_from_slice(&r[rt].to_le_bytes()),
                _ => panic!("unsupported emitted instruction {w:08x}"),
            }
            if let Some((register, value)) = previous_load {
                r[register] = value;
            }
            r[0] = 0;
            pc = prior.unwrap_or(pc + 4);
        }
        panic!("imported-name hook failed to return");
    }

    #[test]
    #[ignore = "requires assets/"]
    fn imported_names_preserve_tags_legacy_conversion_and_callers() {
        let origin = RESOLVER_PROGRAM_ORIGIN;
        let keyboard = crate::name_input::load_name_input_keyboard(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/dialogue/name-entry/keyboard.json"),
        )
        .unwrap();
        let layout = crate::name_input::plan_name_glyph_consumer_layout(&keyboard).unwrap();
        let direct = ImportedDirectNameMap::build(&layout).unwrap();
        let mut a = Assembler::new();
        let (card, registration) = emit_imported_name_normalizers(&mut a, origin, &direct).unwrap();
        let placed = place_program(&a, origin, "imported names").unwrap();
        let mut cases = vec![
            [0x8000u16, 0x4041, 0x0020, 0x061e],
            [0x8001, 0x8042, 0x4042, 0x407f],
            [0x3001, 0x061e, 0x061e, 0x061e],
            [0x0359, 0x03b8, 0x03c1, 0x03ab],
        ];
        // Every selectable direct key must survive an old empty companion, in
        // every nickname position, alongside tagged and native legacy words.
        for cell in &layout.active_glyphs {
            if cell.character.len() == 1 && cell.character.is_ascii() {
                for slot in 0..4 {
                    let mut raw = [0x8000, 0x4041, 0x0020, 0x3001];
                    raw[slot] = cell.code;
                    cases.push(raw);
                }
            }
        }
        cases.extend([
            [0x038d, 0x03aa, 0xffff, 0xc041],
            [0, 0x3001, 0x061e, 0x3001],
        ]);
        for raw in cases {
            for register in [false, true] {
                let mut m = vec![0x55; 0x200000];
                let table = (DIRECT_NAME_MAP_ADDRESS & 0x1fffffff) as usize;
                m[table..table + direct.bytes.len()].copy_from_slice(&direct.bytes);
                let companion = if raw == [0x0359, 0x03b8, 0x03c1, 0x03ab] {
                    [0x3001u16, 0, 0, 0, 0]
                } else {
                    [0x0339u16, 0x033a, 0x0210, 0x3001, 0x3001]
                };
                for (i, v) in companion.iter().enumerate() {
                    m[0x186 + i * 2..0x188 + i * 2].copy_from_slice(&v.to_le_bytes());
                }
                for (i, v) in raw.iter().enumerate() {
                    m[0x196 + i * 2..0x198 + i * 2].copy_from_slice(&v.to_le_bytes());
                }
                let before = m.clone();
                let mut r = std::array::from_fn(|i| i as u32 * 7);
                r[0] = 0;
                r[4] = 0x100;
                r[21] = 0x100;
                r[29] = 0xf00;
                r[31] = 0x800afffc;
                let saved = r;
                let stop = if register {
                    OVERLAY_RUNTIME_BASE + REGISTER_NAME_LOAD_OFFSET as u32 + 8
                } else {
                    r[31]
                };
                execute(
                    &placed.bytes,
                    origin,
                    if register { registration } else { card },
                    stop,
                    &mut r,
                    &mut m,
                );
                let mut expected = before;
                for (i, v) in raw.iter().enumerate() {
                    let mapped = layout
                        .active_glyphs
                        .iter()
                        .find(|cell| {
                            cell.code == *v
                                && cell.character.len() == 1
                                && cell.character.is_ascii()
                        })
                        .map(|cell| cell.character.as_bytes()[0]);
                    let v = if let Some(c) = mapped {
                        ASCII_NAME_TAG | u16::from(c)
                    } else {
                        *v
                    };
                    if v == MESSAGE_END_CODE
                        || matches!(v & TAG_MASK, ASCII_NAME_TAG | HANGUL_NAME_TAG)
                    {
                        expected[0x186 + i * 2..0x188 + i * 2].copy_from_slice(&v.to_le_bytes());
                    }
                }
                expected[0x18e..0x190].copy_from_slice(&MESSAGE_END_CODE.to_le_bytes());
                // Only companion words and the hook's private stack may change.
                assert_eq!(&m[..0xee0], &expected[..0xee0]);
                assert_eq!(&m[0xf00..], &expected[0xf00..]);
                for i in 0..32 {
                    if (register && i == 3) || (!register && i == 5) {
                        continue;
                    }
                    assert_eq!(r[i], saved[i], "register {i} corrupted");
                }
                if register {
                    assert_eq!(
                        r[3],
                        u16::from_le_bytes(m[0x186..0x188].try_into().unwrap()) as u32
                    );
                } else {
                    assert_eq!(r[5], 0x186);
                }
            }
        }
    }
    #[test]
    #[ignore = "requires assets/"]
    fn diary_transfer_preserves_profile_and_normalizes_every_direct_key_in_every_slot() {
        let keyboard = crate::name_input::load_name_input_keyboard(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/dialogue/name-entry/keyboard.json"),
        )
        .unwrap();
        let layout = crate::name_input::plan_name_glyph_consumer_layout(&keyboard).unwrap();
        let direct = ImportedDirectNameMap::build(&layout).unwrap();
        let origin = RESOLVER_PROGRAM_ORIGIN;
        let mut a = Assembler::new();
        let (card, _) = emit_imported_name_normalizers(&mut a, origin, &direct).unwrap();
        let entry = emit_diary_name_transfer(&mut a, origin, card).unwrap();
        let placed = place_program(&a, origin, "Diary name transfer").unwrap();
        let mut cases = vec![(vec![0x3001, 0, 0, 0], vec![0x3001, 0, 0, 0])];
        for cell in &layout.active_glyphs {
            if cell.character.len() != 1 || !cell.character.is_ascii() {
                continue;
            }
            for slot in 0..4 {
                let mut raw = vec![0x8000, 0x4041, 0x8042, 0x407e];
                let mut expected = raw.clone();
                raw[slot] = cell.code;
                expected[slot] = ASCII_NAME_TAG | cell.character.as_bytes()[0] as u16;
                cases.push((raw, expected));
            }
        }
        for (raw, expected) in cases {
            let mut m = vec![0u8; 0x200000];
            let table = (DIRECT_NAME_MAP_ADDRESS & 0x1fffffff) as usize;
            m[table..table + direct.bytes.len()].copy_from_slice(&direct.bytes);
            for (i, word) in raw
                .iter()
                .chain(std::iter::once(&MESSAGE_END_CODE))
                .enumerate()
            {
                m[0x1f1896 + i * 2..0x1f1898 + i * 2].copy_from_slice(&word.to_le_bytes());
            }
            let before = m.clone();
            let mut r = std::array::from_fn(|i| i as u32 * 7);
            r[0] = 0;
            r[29] = 0x801df000;
            r[31] = 0x80010000;
            let saved = r;
            execute(
                &placed.bytes,
                origin,
                entry,
                OVERLAY_RUNTIME_BASE + 0x6120,
                &mut r,
                &mut m,
            );
            for (i, reg) in [3, 4, 5, 6].into_iter().enumerate() {
                assert_eq!(r[reg], expected[i] as u32, "{raw:04x?}");
            }
            assert_eq!(r[7], MESSAGE_END_CODE as u32);
            assert_eq!(r[2], saved[2]);
            assert_eq!(&r[16..], &saved[16..]);
            assert_eq!(&m[..0x1defc0], &before[..0x1defc0]);
            assert_eq!(&m[0x1df000..], &before[0x1df000..]);
        }
    }
}
