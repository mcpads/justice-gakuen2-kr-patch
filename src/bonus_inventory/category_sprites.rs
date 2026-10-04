use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::catalog::expected_occurrences;

const TABLE: usize = 0x1528;
const CATEGORIES: [&str; 7] = [
    "card_category",
    "poster_category",
    "cd_category",
    "video_category",
    "illustrations_category",
    "pocketstation_category",
    "return",
];
// Each source sprite stores screen y, texture u/v, width/height and CLUT.
const SOURCE_ROWS: [[u16; 6]; 14] = [
    [120, 0, 48, 101, 24, 6],
    [160, 0, 72, 120, 24, 6],
    [200, 0, 96, 80, 24, 6],
    [240, 0, 120, 101, 24, 6],
    [280, 0, 144, 141, 24, 6],
    [320, 0, 168, 200, 24, 6],
    [380, 142, 96, 48, 24, 6],
    [116, 0, 0, 120, 32, 0],
    [156, 0, 32, 128, 32, 0],
    [196, 0, 64, 99, 32, 0],
    [236, 0, 96, 120, 32, 0],
    [276, 0, 128, 138, 32, 0],
    [316, 0, 160, 248, 32, 0],
    [376, 200, 0, 56, 32, 0],
];

pub(crate) fn apply_category_sprite_geometry(
    overlay: &mut [u8],
    build: &super::model::BonusInventoryBuild,
) -> Result<Vec<[usize; 2]>> {
    let authored = build
        .report
        .units
        .iter()
        .filter(|unit| unit.development_status == super::model::DevelopmentStatus::Authored)
        .map(|unit| unit.id.as_str())
        .collect::<Vec<_>>();
    apply_authored_category_geometry(overlay, &authored)
}

fn apply_authored_category_geometry(
    overlay: &mut [u8],
    authored: &[&str],
) -> Result<Vec<[usize; 2]>> {
    validate_consumers(overlay)?;
    for (index, row) in SOURCE_ROWS.iter().enumerate() {
        let expected = row.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
        let offset = TABLE + index * 12;
        ensure!(
            overlay.get(offset..offset + 12) == Some(expected.as_slice()),
            "bonus category source sprite changed at row {index}"
        );
    }
    let mut ranges = Vec::new();
    for (index, id) in CATEGORIES.iter().enumerate() {
        if !authored.contains(id) {
            continue;
        }
        for (variant, occurrence) in expected_occurrences(id).into_iter().enumerate() {
            let cell = occurrence.cell;
            let page_origin = if variant == 0 { 0 } else { 256 };
            ensure!(
                cell.x >= page_origin && cell.x + cell.width <= page_origin + 256,
                "bonus category sprite crosses its texture page"
            );
            let values = [cell.x - page_origin, cell.y, cell.width, cell.height];
            let bytes = values
                .into_iter()
                .flat_map(|v| (v as u16).to_le_bytes())
                .collect::<Vec<_>>();
            let offset = TABLE + (index + variant * 7) * 12 + 2;
            overlay[offset..offset + 8].copy_from_slice(&bytes);
            ranges.push([offset, offset + 8]);
        }
    }
    ranges.sort_unstable();
    Ok(ranges)
}

fn validate_consumers(source: &[u8]) -> Result<()> {
    for (offset, expected) in consumer_instructions() {
        let bytes = source
            .get(offset..offset + 4)
            .ok_or_else(|| anyhow::anyhow!("truncated bonus category consumer"))?;
        let actual = decode(
            u32::from_le_bytes(bytes.try_into().unwrap()),
            0x800a2000 + offset as u32,
        )?;
        ensure!(
            actual == expected,
            "bonus category consumer changed at +0x{offset:x}"
        );
    }
    Ok(())
}

fn consumer_instructions() -> Vec<(usize, Instruction)> {
    const S0: Register = Register::S0;
    const S1: Register = Register::S1;
    const S3: Register = Register::S3;
    const S5: Register = Register::S5;
    const V0: Register = Register::V0;
    const ZERO: Register = Register::ZERO;
    vec![
        (
            0x5f04,
            Instruction::Lui {
                rt: S5,
                immediate: 0x800a,
            },
        ),
        (
            0x5f08,
            Instruction::Addiu {
                rt: S5,
                rs: S5,
                immediate: 0x3528,
            },
        ),
        (
            0x5f78,
            Instruction::Addiu {
                rt: S3,
                rs: ZERO,
                immediate: 8,
            },
        ),
        (
            0x5f7c,
            Instruction::Addiu {
                rt: S0,
                rs: S0,
                immediate: 7,
            },
        ),
        (
            0x5f80,
            Instruction::Addiu {
                rt: S3,
                rs: ZERO,
                immediate: 9,
            },
        ),
        (
            0x5fbc,
            Instruction::Lhu {
                rt: V0,
                base: V0,
                offset: 2,
            },
        ),
        (
            0x5fcc,
            Instruction::Lhu {
                rt: V0,
                base: V0,
                offset: 4,
            },
        ),
        (
            0x5fdc,
            Instruction::Lhu {
                rt: V0,
                base: V0,
                offset: 6,
            },
        ),
        (
            0x5fec,
            Instruction::Lhu {
                rt: V0,
                base: V0,
                offset: 8,
            },
        ),
        (
            0x60a0,
            Instruction::Slti {
                rt: V0,
                rs: S1,
                immediate: 7,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source_fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x6100];
        for (i, row) in SOURCE_ROWS.iter().enumerate() {
            for (j, value) in row.iter().enumerate() {
                let offset = TABLE + i * 12 + j * 2;
                bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        for (offset, insn) in consumer_instructions() {
            bytes[offset..offset + 4].copy_from_slice(
                &psx_r3000a::encode(&insn, 0x800a2000 + offset as u32)
                    .unwrap()
                    .to_le_bytes(),
            );
        }
        bytes
    }
    #[test]
    fn category_sprites_sample_complete_authored_cells_without_moving_screen_positions() {
        let source = source_fixture();
        let mut output = source.clone();
        let ranges = apply_authored_category_geometry(&mut output, &CATEGORIES).unwrap();
        assert_eq!(
            u16::from_le_bytes(output[0x15b2..0x15b4].try_into().unwrap()),
            192
        );
        for (i, id) in CATEGORIES.iter().enumerate() {
            for (v, occurrence) in expected_occurrences(id).into_iter().enumerate() {
                let offset = TABLE + (i + v * 7) * 12;
                let sample = (1..5)
                    .map(|j| {
                        u16::from_le_bytes(
                            output[offset + j * 2..offset + j * 2 + 2]
                                .try_into()
                                .unwrap(),
                        ) as usize
                    })
                    .collect::<Vec<_>>();
                let cell = occurrence.cell;
                assert_eq!(sample, [cell.x % 256, cell.y, cell.width, cell.height]);
                assert_eq!(&output[offset..offset + 2], &source[offset..offset + 2]);
                assert_eq!(
                    &output[offset + 10..offset + 12],
                    &source[offset + 10..offset + 12]
                );
            }
        }
        assert!(
            source
                .iter()
                .zip(&output)
                .enumerate()
                .all(|(i, (a, b))| a == b || ranges.iter().any(|r| r[0] <= i && i < r[1]))
        );
    }
    #[test]
    fn untranslated_categories_keep_their_native_sprite_geometry() {
        let source = source_fixture();
        let mut output = source.clone();
        let ranges =
            apply_authored_category_geometry(&mut output, &["illustrations_category"]).unwrap();
        assert_eq!(ranges, [[0x155a, 0x1562], [0x15ae, 0x15b6]]);
        for index in 0..14 {
            if index == 4 || index == 11 {
                continue;
            }
            let start = TABLE + index * 12;
            assert_eq!(&source[start..start + 12], &output[start..start + 12]);
        }
    }

    #[test]
    fn changed_source_geometry_and_selected_page_are_rejected_before_writing() {
        for offset in [0x15b2, 0x5f80] {
            let mut bytes = source_fixture();
            bytes[offset] ^= 1;
            let before = bytes.clone();
            assert!(apply_authored_category_geometry(&mut bytes, &CATEGORIES).is_err());
            assert_eq!(bytes, before);
        }
    }
}
