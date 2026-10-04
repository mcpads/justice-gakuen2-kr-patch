use super::*;
use crate::name_input::{
    DIGIT_KEYS, LATIN_KEYS, NameSlotCode, SYMBOL_KEYS, load_name_input_hangul,
};
const MEMBERSHIP: u32 = 0x800b_0040;
const ASCII: u32 = 0x800b_1000;
const RECORD: usize = 0x1f5804;
const DECODED: usize = RECORD + 16 * 40;
const INPUT: usize = 0x1f1a80;

#[test]
fn codec_fits_owned_source_and_has_valid_load_delays() {
    let (p, decoder) = program(MEMBERSHIP, ASCII).unwrap();
    assert!(decoder > BASE + START as u32 && decoder < BASE + END as u32);
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &p,
        BASE + START as u32,
        "password codec",
    )
    .unwrap();
}

struct Machine {
    code: Vec<u8>,
    memory: Vec<u8>,
    decoder: u32,
}
impl Machine {
    fn new() -> Self {
        let cue = crate::cue::CueSheet::parse(std::path::Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))
        .unwrap();
        let (_, source) =
            crate::disc::rebuild::read_record(&cue.image_path, "DAT1/PASS.BIN").unwrap();
        let (_, kanri) =
            crate::disc::rebuild::read_record(&cue.image_path, "DAT1/KANRI.BIN").unwrap();
        let hash = sha256_bytes(&source);
        let mut plan = DecodedRecordWritePlan::new("DAT1/PASS.BIN", &source, &hash).unwrap();
        let mut sources = PsxMachineCodeSources::default();
        register(&source, &kanri, &mut plan, &mut sources, MEMBERSHIP, ASCII).unwrap();
        let code = plan.apply(Some(&sources)).unwrap();
        let (_, decoder) = program(MEMBERSHIP, ASCII).unwrap();
        let mut memory = vec![0; 2 * 1024 * 1024];
        memory[0x17a000..0x17a000 + code.len()].copy_from_slice(&code);
        for c in load_name_input_hangul().unwrap() {
            let n = c as usize - 0xac00;
            memory[0xb0040 + n / 8] |= 1 << (n % 8);
        }
        memory[0xb1000..0xb1100].fill(255);
        for c in LATIN_KEYS
            .chars()
            .chain(DIGIT_KEYS.chars())
            .chain(SYMBOL_KEYS.chars())
        {
            memory[0xb1000 + 2 * c as usize..0xb1000 + 2 * c as usize + 2]
                .copy_from_slice(&0u16.to_le_bytes());
        }
        Self {
            code,
            memory,
            decoder,
        }
    }
    fn run(&mut self, entry: u32, slot: u32) -> u32 {
        let mut r = [0; 32];
        r[4] = slot;
        r[5] = 0x8000_0000 + INPUT as u32;
        r[29] = 0x8016_0000;
        r[31] = 0x8000_0400;
        crate::name_input::runtime_test_machine::execute_with_callbacks(
            &self.code[(entry - BASE) as usize..],
            entry,
            &mut r,
            &mut self.memory,
            None,
            &mut |_, _, _| false,
        );
        assert_eq!(r[29], 0x8016_0000);
        r[2]
    }
    fn encode(&mut self, record: &[u8; 40]) -> Vec<u8> {
        self.memory[RECORD..RECORD + 40].copy_from_slice(record);
        self.memory[INPUT - 4..INPUT + 28].fill(0xcc);
        assert_eq!(self.run(0x8018_1160, 0), 0);
        assert_eq!(&self.memory[INPUT - 4..INPUT], &[0xcc; 4]);
        assert_eq!(&self.memory[INPUT + 24..INPUT + 28], &[0xcc; 4]);
        self.memory[INPUT..INPUT + 24].to_vec()
    }
    fn decode(&mut self, values: &[u8]) -> u32 {
        self.memory[INPUT..INPUT + 24].fill(255);
        self.memory[INPUT..INPUT + values.len()].copy_from_slice(values);
        self.memory[DECODED..DECODED + 40].fill(0xa5);
        self.run(self.decoder, 16)
    }
}
fn record() -> [u8; 40] {
    let mut r = [0; 40];
    r[..16].copy_from_slice(&[1, 15, 3, 0, 0, 0, 0, 2, 10, 5, 4, 4, 3, 1, 7, 3]);
    r[16..26].copy_from_slice(&[0, 0x80, 1, 0x30, 0, 0, 0, 0, 1, 0x30]);
    r[36..40].copy_from_slice(&[1, 0, 1, 0]);
    r
}
// Independent byte model of the observed retail stat fields, with raw names.
fn expected(record: &[u8; 40]) -> Vec<u8> {
    let r = record;
    let mut p = r[16..24].to_vec();
    p.extend([
        (r[0] & 15) << 4 | r[12] & 15,
        (r[8] & 15) << 4 | r[9] & 15,
        (r[10] & 7) << 5 | r[4] & 31,
        (r[6] & 31) << 3 | r[36] & 7,
        (r[38] & 7) << 5 | r[5] & 31,
        (r[37] & 1) << 6 | r[1] & 31 | 0xa0,
        (r[2] & 7) << 5 | r[13] & 31,
        (r[11] & 7) << 5 | r[15] & 31,
        (r[3] & 1) << 7 | (r[14] & 31) << 2 | r[7] & 3,
    ]);
    let checksum = p.iter().fold(0u8, |a, b| a.wrapping_add(*b));
    p.push(checksum);
    p.as_chunks::<3>()
        .0
        .iter()
        .flat_map(|b| {
            [
                b[0] >> 2,
                ((b[0] & 3) << 4) | (b[1] >> 4),
                ((b[1] & 15) << 2) | (b[2] >> 6),
                b[2] & 63,
            ]
        })
        .collect()
}
#[test]
#[ignore = "requires the source disc; executes original encoder prefix and native validation"]
fn native_password_name_words_and_rejection_domain() {
    let mut m = Machine::new();
    let mut r = record();
    // Every 16-bit word is classified, including all unsupported tags/syllables.
    for word in 0..=u16::MAX {
        let position = (word as usize % 4) * 2 + 16;
        r[16..24].fill(0);
        r[position..position + 2].copy_from_slice(&word.to_le_bytes());
        let values = m.encode(&r);
        assert_eq!(values, expected(&r), "encode {word:04x}");
        let valid = word == 0x3001 || NameSlotCode::decode(word).is_ok();
        assert_eq!(m.decode(&values) == 0, valid, "decode {word:04x}");
        if valid {
            assert_eq!(&m.memory[DECODED + 16..DECODED + 24], &r[16..24]);
            assert_eq!(&m.memory[DECODED..DECODED + 16], &r[..16]);
        } else {
            assert_eq!(&m.memory[DECODED..DECODED + 40], &[0xa5; 40]);
        }
        assert_eq!(&m.memory[RECORD..RECORD + 40], &r);
    }
    // Byte-boundary extremes and every supported Hangul syllable at each slot.
    for c in load_name_input_hangul().unwrap() {
        for pos in 0..4 {
            let mut r = record();
            r[16 + pos * 2..18 + pos * 2]
                .copy_from_slice(&(0x8000 + (c as u32 - 0xac00) as u16).to_le_bytes());
            let v = m.encode(&r);
            assert_eq!(m.decode(&v), 0);
            assert_eq!(&m.memory[DECODED + 16..DECODED + 24], &r[16..24]);
        }
    }
    let r = record();
    let good = m.encode(&r);
    for pos in 0..24 {
        for bit in 0..6 {
            let mut v = good.clone();
            v[pos] ^= 1 << bit;
            assert_ne!(m.decode(&v), 0, "checksum {pos}/{bit}");
        }
    }
    for n in 0..24 {
        assert_ne!(m.decode(&good[..n]), 0, "partial {n}");
    }
    for pos in 0..24 {
        for value in 64..=255 {
            let mut v = good.clone();
            v[pos] = value;
            assert_ne!(m.decode(&v), 0, "invalid symbol {pos}/{value}");
        }
    }
    let jp = [
        3, 64, 43, 3, 22, 9, 3, 16, 30, 24, 17, 59, 26, 70, 3, 5, 66, 31, 61, 28,
    ];
    assert_eq!(m.decode(&jp), 0);
    assert_eq!(&m.memory[DECODED..DECODED + 16], &r[..16]);
    assert_eq!(&m.memory[DECODED + 16..DECODED + 18], &[0, 0]);
}

#[test]
#[ignore = "requires the source disc; executes the CPU menu's encoder call"]
fn cpu_menu_exports_to_the_shared_input_buffer() {
    let mut m = Machine::new();
    let mut record = record();
    record[26..36].fill(0x55);
    m.memory[RECORD..RECORD + 40].copy_from_slice(&record);
    m.memory[INPUT..INPUT + 24].fill(0xcc);
    m.memory[0x1f1a2e..0x1f1a42].fill(0xdd);
    let mut r = [0; 32];
    r[16] = 0x801f_1a00;
    r[29] = 0x8016_0000;
    r[31] = 0x8017_fc88;
    crate::name_input::runtime_test_machine::execute_with_callbacks(
        &m.code,
        BASE,
        &mut r,
        &mut m.memory,
        None,
        &mut |pc, regs, _| {
            if pc == BASE {
                regs[31] = 0x8017_fc7c;
                true
            } else {
                false
            }
        },
    );
    assert_eq!(
        &m.memory[INPUT..INPUT + 14],
        &[6, 70, 35, 35, 70, 5, 5, 35, 6, 70, 35, 5, 6, 15]
    );
    assert_eq!(&m.memory[INPUT + 14..INPUT + 24], &[0xcc; 10]);
    assert_eq!(&m.memory[0x1f1a2e..0x1f1a42], &[0xdd; 20]);
    assert_eq!(&m.memory[RECORD..RECORD + 40], &record);
}
