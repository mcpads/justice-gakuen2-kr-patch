use crate::text::read_length_prefixed_codes;

use super::records_main::{
    UNIT_IDS, expected_write_range, main_executable_expected_write_ranges, rebuild_main_executable,
    rebuild_pool,
};

#[test]
fn records_main_strings_are_repacked_within_their_contiguous_source_pool() {
    let mut overlay = vec![0xff; 0x700];
    let records = [
        (UNIT_IDS[0], &[0x0368, 0x0fff, 0x036a][..]),
        (UNIT_IDS[1], &[0x0107, 0x0109, 0x010b, 0x0113][..]),
        (UNIT_IDS[2], &[0x0115, 0x0117][..]),
        (UNIT_IDS[3], &[0x0119, 0x011b][..]),
    ];

    let offsets = rebuild_pool(&mut overlay, &records).unwrap();

    assert_eq!(expected_write_range(), [0x05f0, 0x0610]);
    assert_eq!(offsets[UNIT_IDS[0]], 0x05f0);
    assert_eq!(offsets[UNIT_IDS[1]], 0x05f8);
    assert_eq!(offsets[UNIT_IDS[2]], 0x0602);
    assert_eq!(offsets[UNIT_IDS[3]], 0x0608);
    assert_eq!(&overlay[0x060e..0x0610], &[0, 0]);
}

#[test]
fn records_main_pool_rejects_an_unowned_record() {
    let mut overlay = vec![0; 0x700];
    let error = rebuild_pool(&mut overlay, &[("records_heading", &[0x0368])]).unwrap_err();

    assert!(error.to_string().contains("membership changed"));
}

#[test]
fn records_main_repoints_the_consumed_main_executable_records() {
    let source = source_main_executable();
    let records = authored_records();

    let output = rebuild_main_executable(&source, &records).unwrap();

    assert_eq!(
        main_executable_expected_write_ranges(),
        [
            [0x080678, 0x080686],
            [0x0807e0, 0x080800],
            [0x080c00, 0x080c0c],
            [0x080e28, 0x080e38]
        ]
    );
    assert_eq!(
        read_length_prefixed_codes(&output, 0x0807e8).unwrap(),
        [0x0107, 0x0109, 0x010b, 0x0113]
    );
    assert_eq!(
        read_length_prefixed_codes(&output, 0x0807f2).unwrap(),
        [0x0115, 0x0117]
    );
    assert_eq!(read_u32(&output, 0x080e30), 0x8008_fff2);
    assert_eq!(
        read_length_prefixed_codes(&output, 0x080678).unwrap(),
        [0x035a, 0x0fff, 0x0389]
    );
    assert_eq!(
        read_length_prefixed_codes(&output, 0x080680).unwrap(),
        [0x0389]
    );
    assert_eq!(
        read_length_prefixed_codes(&output, 0x080c00).unwrap(),
        [0x0392, 0x0271, 0x02a1]
    );
}

fn authored_records() -> [(&'static str, &'static [u16]); 7] {
    [
        (UNIT_IDS[0], &[0x0368, 0x0fff, 0x036a]),
        (UNIT_IDS[1], &[0x0107, 0x0109, 0x010b, 0x0113]),
        (UNIT_IDS[2], &[0x0115, 0x0117]),
        (UNIT_IDS[3], &[0x0119, 0x011b]),
        ("disabled", &[0x035a, 0x0fff, 0x0389]),
        ("enabled", &[0x0389]),
        ("exit", &[0x0392, 0x0271, 0x02a1]),
    ]
}

fn source_main_executable() -> Vec<u8> {
    let mut source = vec![0; 0x91800];
    source[..8].copy_from_slice(b"PS-X EXE");
    write_record(&mut source, 0x0807e0, &[0x0368, 0x0fff, 0x036a]);
    write_record(&mut source, 0x0807e8, &[0x0156, 0x0071, 0x0171]);
    write_record(&mut source, 0x0807f0, &[0x0131, 0x0071, 0x0174]);
    write_record(&mut source, 0x0807f8, &[0x0124, 0x0071, 0x0137]);
    write_record(&mut source, 0x080678, &[0x0083, 0x0090, 0x0075]);
    write_record(&mut source, 0x080680, &[0x0084, 0x00a8]);
    write_record(&mut source, 0x080c00, &[0x0012, 0x0029, 0x0016, 0x0025]);
    for (offset, pointer) in [
        (0x080e28, 0x8008_ffe0_u32),
        (0x080e2c, 0x8008_ffe8),
        (0x080e30, 0x8008_fff0),
        (0x080e34, 0x8008_fff8),
        (0x080dc4, 0x8008_fe78),
        (0x080dc8, 0x8008_fe80),
        (0x080f40, 0x8009_0400),
    ] {
        source[offset..offset + 4].copy_from_slice(&pointer.to_le_bytes());
    }
    source
}

fn write_record(output: &mut [u8], offset: usize, codes: &[u16]) {
    output[offset..offset + 2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let start = offset + 2 + index * 2;
        output[start..start + 2].copy_from_slice(&code.to_le_bytes());
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
