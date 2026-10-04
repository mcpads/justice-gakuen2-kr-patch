use super::*;

fn sector() -> [u8; RAW_SECTOR_SIZE] {
    let mut sector = [0u8; RAW_SECTOR_SIZE];
    sector[..12].copy_from_slice(&SYNC);
    sector[0x0f] = 2;
    sector[0x10..0x14].copy_from_slice(&[1, 2, 0x08, 0]);
    sector[0x14..0x18].copy_from_slice(&[1, 2, 0x08, 0]);
    for (index, byte) in sector[0x18..0x818].iter_mut().enumerate() {
        *byte = index as u8;
    }
    sector
}

#[test]
fn regenerates_and_detects_corruption() {
    let mut raw = sector();
    regenerate(&mut raw).unwrap();
    assert!(verify(&raw));
    raw[0x200] ^= 1;
    assert!(!verify(&raw));
}
