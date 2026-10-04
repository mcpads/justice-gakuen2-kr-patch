pub(crate) fn execute(bytes: &[u8], origin: u32, r: &mut [u32; 32], memory: &mut [u8]) {
    execute_with_reader(bytes, origin, r, memory, None);
}

// Decode the emitted bytes independently and model both branch and load delay
// slots. Unhandled instructions fail rather than silently approximating them.
pub(super) fn execute_with_reader(
    bytes: &[u8],
    origin: u32,
    r: &mut [u32; 32],
    memory: &mut [u8],
    reader: Option<(u32, &[u8])>,
) {
    execute_with_callbacks(bytes, origin, r, memory, reader, &mut |_, _, _| false);
}

pub(crate) fn execute_with_callbacks(
    bytes: &[u8],
    origin: u32,
    r: &mut [u32; 32],
    memory: &mut [u8],
    reader: Option<(u32, &[u8])>,
    callback: &mut impl FnMut(u32, &mut [u32; 32], &mut [u8]) -> bool,
) {
    execute_with_callbacks_limit(bytes, origin, r, memory, reader, 100_000, callback);
}

// Whole-loader fixtures execute many glyphs and a scattered 19 KB pack copy.
// Keep their budget explicit instead of weakening the small-program default.
pub(crate) fn execute_with_callbacks_limit(
    bytes: &[u8],
    origin: u32,
    r: &mut [u32; 32],
    memory: &mut [u8],
    reader: Option<(u32, &[u8])>,
    instruction_limit: usize,
    callback: &mut impl FnMut(u32, &mut [u32; 32], &mut [u8]) -> bool,
) {
    assert!((1..=2_000_000).contains(&instruction_limit));
    let stop = r[31];
    let mut pc = origin;
    let mut lo = 0;
    let mut hi = 0;
    let mut branch = None;
    let mut load: Option<(usize, u32)> = None;
    for _ in 0..instruction_limit {
        if pc == stop {
            return;
        }
        if callback(pc, r, memory) {
            assert!(branch.is_none() && load.is_none());
            pc = r[31];
            continue;
        }
        if let Some((address, pack)) = reader
            && pc == address
        {
            assert!(branch.is_none() && load.is_none());
            let value = pack[r[4] as usize];
            // Model the reader's permitted clobbers, not its implementation.
            for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                r[reg] = 0xdead0000 + reg as u32;
            }
            let sp = (r[29] & 0x1fff_ffff) as usize;
            memory[sp..sp + 16].fill(0xa5);
            lo = 0xdeadbeef;
            r[2] = u32::from(value);
            pc = r[31];
            continue;
        }
        let offset = (pc - origin) as usize;
        let word = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let rs = ((word >> 21) & 31) as usize;
        let rt = ((word >> 16) & 31) as usize;
        let rd = ((word >> 11) & 31) as usize;
        let immediate = word as u16 as i16 as i32 as u32;
        let address = (r[rs].wrapping_add(immediate) & 0x1fff_ffff) as usize;
        let prior_branch = branch.take();
        let prior_load = load.take();
        match word >> 26 {
            0 => match word & 63 {
                0 => r[rd] = r[rt] << ((word >> 6) & 31),
                2 => r[rd] = r[rt] >> ((word >> 6) & 31),
                3 => r[rd] = ((r[rt] as i32) >> ((word >> 6) & 31)) as u32,
                4 => r[rd] = r[rt] << (r[rs] & 31),
                6 => r[rd] = r[rt] >> (r[rs] & 31),
                16 => r[rd] = hi,
                18 => r[rd] = lo,
                27 => {
                    assert_ne!(r[rt], 0);
                    lo = r[rs] / r[rt];
                    hi = r[rs] % r[rt];
                }
                25 => {
                    let product = u64::from(r[rs]) * u64::from(r[rt]);
                    lo = product as u32;
                    hi = (product >> 32) as u32;
                }
                8 => branch = Some(r[rs]),
                9 => {
                    let target = r[rs];
                    r[rd] = pc + 8;
                    branch = Some(target);
                }
                35 => r[rd] = r[rs].wrapping_sub(r[rt]),
                33 => r[rd] = r[rs].wrapping_add(r[rt]),
                37 => r[rd] = r[rs] | r[rt],
                38 => r[rd] = r[rs] ^ r[rt],
                42 => r[rd] = u32::from((r[rs] as i32) < (r[rt] as i32)),
                43 => r[rd] = u32::from(r[rs] < r[rt]),
                _ => panic!("unsupported instruction {word:08x}"),
            },
            2 => branch = Some(((pc + 4) & 0xf0000000) | ((word & 0x3ffffff) << 2)),
            3 => {
                r[31] = pc + 8;
                branch = Some(((pc + 4) & 0xf0000000) | ((word & 0x3ffffff) << 2));
            }
            4 | 5 => {
                if (r[rs] == r[rt]) == (word >> 26 == 4) {
                    branch = Some((pc + 4).wrapping_add(immediate.wrapping_mul(4)));
                }
            }
            7 => {
                if (r[rs] as i32) > 0 {
                    branch = Some((pc + 4).wrapping_add(immediate.wrapping_mul(4)));
                }
            }
            9 => r[rt] = r[rs].wrapping_add(immediate),
            10 => r[rt] = u32::from((r[rs] as i32) < (immediate as i32)),
            11 => r[rt] = u32::from(r[rs] < immediate),
            12 => r[rt] = r[rs] & (word & 65535),
            13 => r[rt] = r[rs] | (word & 65535),
            14 => r[rt] = r[rs] ^ (word & 65535),
            15 => r[rt] = (word & 65535) << 16,
            33 => {
                assert_eq!(address % 2, 0);
                load = Some((
                    rt,
                    i16::from_le_bytes(memory[address..address + 2].try_into().unwrap()) as i32
                        as u32,
                ));
            }
            37 => {
                assert_eq!(address % 2, 0);
                load = Some((
                    rt,
                    u32::from(u16::from_le_bytes(
                        memory[address..address + 2].try_into().unwrap(),
                    )),
                ));
            }
            35 => {
                assert_eq!(address % 4, 0);
                load = Some((
                    rt,
                    u32::from_le_bytes(memory[address..address + 4].try_into().unwrap()),
                ));
            }
            41 => {
                assert_eq!(address % 2, 0);
                memory[address..address + 2].copy_from_slice(&(r[rt] as u16).to_le_bytes());
            }
            43 => {
                assert_eq!(address % 4, 0);
                memory[address..address + 4].copy_from_slice(&r[rt].to_le_bytes());
            }
            32 => load = Some((rt, memory[address] as i8 as i32 as u32)),
            36 => load = Some((rt, u32::from(memory[address]))),
            40 => memory[address] = r[rt] as u8,
            _ => panic!("unsupported instruction {word:08x}"),
        }
        if let Some((register, value)) = prior_load {
            r[register] = value;
        }
        r[0] = 0;
        pc = prior_branch.unwrap_or(pc + 4);
    }
    panic!("name program failed to return");
}
