use std::sync::OnceLock;

use anyhow::{Result, ensure};

use super::{RAW_SECTOR_SIZE, SYNC};

const USER_DATA_END: usize = 0x818;
const EDC_START: usize = 0x818;
const ECC_P_START: usize = 0x81c;
const ECC_Q_START: usize = 0x8c8;

struct ChecksumTables {
    forward: [u8; 256],
    backward: [u8; 256],
    edc: [u32; 256],
}

static CHECKSUM_TABLES: OnceLock<ChecksumTables> = OnceLock::new();

pub fn verify(sector: &[u8; RAW_SECTOR_SIZE]) -> bool {
    expected_checksums(sector).is_ok_and(|(edc, p, q)| {
        sector[EDC_START..ECC_P_START] == edc
            && sector[ECC_P_START..ECC_Q_START] == p
            && sector[ECC_Q_START..] == q
    })
}

pub fn regenerate(sector: &mut [u8; RAW_SECTOR_SIZE]) -> Result<()> {
    let (edc, p, q) = expected_checksums(sector)?;
    sector[EDC_START..ECC_P_START].copy_from_slice(&edc);
    sector[ECC_P_START..ECC_Q_START].copy_from_slice(&p);
    sector[ECC_Q_START..].copy_from_slice(&q);
    Ok(())
}

fn expected_checksums(sector: &[u8; RAW_SECTOR_SIZE]) -> Result<([u8; 4], [u8; 172], [u8; 104])> {
    validate(sector)?;
    let tables = CHECKSUM_TABLES.get_or_init(make_tables);
    let mut work = *sector;
    let edc = compute_edc(&work[0x10..USER_DATA_END], &tables.edc).to_le_bytes();
    work[EDC_START..ECC_P_START].copy_from_slice(&edc);
    work[0x0c..0x10].fill(0);

    let p = compute_ecc_block::<172>(
        &work[0x0c..],
        86,
        24,
        2,
        86,
        &tables.forward,
        &tables.backward,
    )?;
    work[ECC_P_START..ECC_Q_START].copy_from_slice(&p);
    let q = compute_ecc_block::<104>(
        &work[0x0c..],
        52,
        43,
        86,
        88,
        &tables.forward,
        &tables.backward,
    )?;
    Ok((edc, p, q))
}

fn validate(sector: &[u8; RAW_SECTOR_SIZE]) -> Result<()> {
    ensure!(sector[..12] == SYNC, "invalid sync pattern");
    ensure!(sector[0x0f] == 2, "expected Mode 2 sector");
    ensure!(
        sector[0x10..0x14] == sector[0x14..0x18],
        "subheader mismatch"
    );
    ensure!(sector[0x12] & 0x20 == 0, "Mode 2 Form 2 has no P/Q ECC");
    Ok(())
}

fn make_tables() -> ChecksumTables {
    let mut forward = [0u8; 256];
    let mut backward = [0u8; 256];
    let mut edc = [0u32; 256];
    for value in 0..256usize {
        let doubled = (((value as u32) << 1) ^ if value & 0x80 != 0 { 0x11d } else { 0 }) & 0xff;
        forward[value] = doubled as u8;
        backward[value ^ doubled as usize] = value as u8;

        let mut remainder = value as u32;
        for _ in 0..8 {
            remainder = if remainder & 1 != 0 {
                (remainder >> 1) ^ 0xd8018001
            } else {
                remainder >> 1
            };
        }
        edc[value] = remainder;
    }
    ChecksumTables {
        forward,
        backward,
        edc,
    }
}

fn compute_edc(data: &[u8], table: &[u32; 256]) -> u32 {
    let mut value = 0u32;
    for &byte in data {
        value = (value >> 8) ^ table[((value ^ u32::from(byte)) & 0xff) as usize];
    }
    value
}

fn compute_ecc_block<const OUTPUT_SIZE: usize>(
    source: &[u8],
    major_count: usize,
    minor_count: usize,
    major_multiplier: usize,
    minor_increment: usize,
    forward: &[u8; 256],
    backward: &[u8; 256],
) -> Result<[u8; OUTPUT_SIZE]> {
    let size = major_count * minor_count;
    ensure!(source.len() >= size, "ECC source block is too short");
    ensure!(
        OUTPUT_SIZE == major_count * 2,
        "ECC output has the wrong fixed size"
    );
    let mut result = [0u8; OUTPUT_SIZE];
    for major in 0..major_count {
        let mut index = (major >> 1) * major_multiplier + (major & 1);
        let mut ecc_a = 0u8;
        let mut ecc_b = 0u8;
        for _ in 0..minor_count {
            let value = source[index];
            index += minor_increment;
            if index >= size {
                index -= size;
            }
            ecc_a ^= value;
            ecc_b ^= value;
            ecc_a = forward[ecc_a as usize];
        }
        ecc_a = backward[(forward[ecc_a as usize] ^ ecc_b) as usize];
        result[major] = ecc_a;
        result[major + major_count] = ecc_a ^ ecc_b;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "mode2_tests.rs"]
mod tests;
