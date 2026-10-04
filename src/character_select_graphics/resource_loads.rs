//! Source-bound resource loads shared by character-select producer families.

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;

struct ResourceLoadSpec {
    source_record: &'static str,
    consumer_record: &'static str,
    catalog_index: i16,
    byte_offsets: &'static [usize],
}

const RESOURCE_LOADS: &[ResourceLoadSpec] = &[
    ResourceLoadSpec {
        source_record: "DAT2/SELP1.BIZ",
        consumer_record: "DAT1/PLSEL1.BIN",
        catalog_index: 0x035,
        byte_offsets: &[0x0ba8, 0x0bac],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SELP2.BIZ",
        consumer_record: "DAT1/PLSEL2.BIN",
        catalog_index: 0x036,
        byte_offsets: &[0x0824, 0x0828, 0x287c, 0x2880, 0x3e9c, 0x3ea0],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SELP3.BIZ",
        consumer_record: "DAT1/PLSEL3.BIN",
        catalog_index: 0x037,
        byte_offsets: &[0x074c, 0x0750, 0x3348, 0x334c],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SELP4.BIZ",
        consumer_record: "DAT1/PLSEL4.BIN",
        catalog_index: 0x038,
        byte_offsets: &[0x09e8, 0x09ec, 0x333c, 0x3340],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SELP5.BIZ",
        consumer_record: "DAT1/PLSEL5.BIN",
        catalog_index: 0x039,
        byte_offsets: &[0x4210, 0x4214, 0x4574, 0x4578],
    },
    ResourceLoadSpec {
        source_record: "DAT2/AISYOU.TIZ",
        consumer_record: "DAT1/PLSEL5.BIN",
        catalog_index: 0x2dd,
        byte_offsets: &[0x3ce8, 0x3cec, 0x547c, 0x5480],
    },
    ResourceLoadSpec {
        source_record: "DAT2/OP01.BIZ",
        consumer_record: "DAT1/ODEMO.BIN",
        catalog_index: 0x24b,
        byte_offsets: &[0x0298, 0x029c],
    },
    ResourceLoadSpec {
        source_record: "DAT2/TITLE.BIN",
        consumer_record: "DAT1/CDEMO.BIN",
        catalog_index: 0x24a,
        byte_offsets: &[0x0a40, 0x0a44],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SIKEN1.BIZ",
        consumer_record: "DAT1/SIKEN.BIN",
        catalog_index: 0x2b2,
        byte_offsets: &[0x153c, 0x1540, 0x1840, 0x1844],
    },
    ResourceLoadSpec {
        source_record: "DAT2/SIKEN10.BIZ",
        consumer_record: "DAT1/SIKEN2.BIN",
        catalog_index: 0x2b3,
        byte_offsets: &[0x1294, 0x1298],
    },
];

pub(super) fn resource_load_byte_offsets(
    source_record: &str,
    consumer_record: &str,
) -> Option<&'static [usize]> {
    RESOURCE_LOADS
        .iter()
        .find(|spec| spec.source_record == source_record && spec.consumer_record == consumer_record)
        .map(|spec| spec.byte_offsets)
}

pub(crate) fn validate_consumer_resource_loads(
    consumer_record: &str,
    overlay: &[u8],
) -> Result<()> {
    for spec in RESOURCE_LOADS
        .iter()
        .filter(|spec| spec.consumer_record == consumer_record)
    {
        ensure!(
            spec.byte_offsets.len().is_multiple_of(2),
            "{} resource-load evidence has an incomplete call pair",
            spec.source_record
        );
        for pair in spec.byte_offsets.as_chunks::<2>().0 {
            let call = decode_instruction(overlay, pair[0])?;
            let argument = decode_instruction(overlay, pair[1])?;
            ensure!(
                call == (Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                }) && argument
                    == (Instruction::Addiu {
                        rt: Register::A1,
                        rs: Register::ZERO,
                        immediate: spec.catalog_index,
                    }),
                "{} no longer loads {} at +0x{:04x}/+0x{:04x}: found {call:?} / {argument:?}",
                consumer_record,
                spec.source_record,
                pair[0],
                pair[1]
            );
        }
    }
    Ok(())
}

fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let word = u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated resource-load instruction at +0x{offset:04x}"))?
            .try_into()?,
    );
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode resource-load instruction at +0x{offset:04x}"))
}
