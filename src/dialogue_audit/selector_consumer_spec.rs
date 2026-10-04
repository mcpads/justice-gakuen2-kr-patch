use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, PROFILE_ID, Register, decode};

use crate::pipeline::sha256_bytes;

use super::format::hex_address;
use super::script_source::{MGAME_SHA256, MGAME_SIZE};
use super::selector_consumers_model::{
    DialogueSelectorConsumerAudit, DialogueSelectorConsumerReport,
    DialogueSelectorEntryConsumerAudit,
};

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const SELECTOR_TABLE_RUNTIME_START: u32 = 0x8010_1000;
const SELECTOR_COUNT: usize = 8;
const ADOPTED_REPORT_SHA256: &str =
    "e69c9ed37471570b12596e1e3f42fb8ef28b20ba9359fa204118a04b796c58cb";

#[derive(Clone, Copy)]
struct EntryLoadSpec {
    entry_index: usize,
    runtime_address: u32,
}

#[derive(Clone, Copy)]
struct SelectorConsumerSpec {
    selector_index: usize,
    load_runtime_address: u32,
    entry_loads: &'static [EntryLoadSpec],
}

const RUNTIME_INSERTION_ENTRY_LOADS: [EntryLoadSpec; 4] = [
    EntryLoadSpec {
        entry_index: 5,
        runtime_address: 0x800a_fcc8,
    },
    EntryLoadSpec {
        entry_index: 6,
        runtime_address: 0x800a_fd44,
    },
    EntryLoadSpec {
        entry_index: 7,
        runtime_address: 0x800a_fe90,
    },
    EntryLoadSpec {
        entry_index: 8,
        runtime_address: 0x800a_feac,
    },
];

const SELECTOR_FIVE_ENTRY_LOADS: [EntryLoadSpec; 1] = [EntryLoadSpec {
    entry_index: 30,
    runtime_address: 0x800c_1048,
}];

const ADOPTED_CONSUMERS: [SelectorConsumerSpec; 15] = [
    selector_consumer(2, 0x800a_8474, &[]),
    selector_consumer(2, 0x800a_86f0, &[]),
    selector_consumer(2, 0x800a_8858, &[]),
    selector_consumer(2, 0x800a_a6b8, &[]),
    selector_consumer(2, 0x800a_b310, &[]),
    selector_consumer(2, 0x800a_b4f0, &[]),
    selector_consumer(6, 0x800a_b74c, &[]),
    selector_consumer(6, 0x800a_b924, &[]),
    selector_consumer(2, 0x800a_c214, &[]),
    selector_consumer(2, 0x800a_fba4, &RUNTIME_INSERTION_ENTRY_LOADS),
    selector_consumer(2, 0x800b_a3f0, &[]),
    selector_consumer(5, 0x800c_1040, &SELECTOR_FIVE_ENTRY_LOADS),
    selector_consumer(5, 0x800c_2428, &[]),
    selector_consumer(2, 0x800c_787c, &[]),
    selector_consumer(2, 0x800c_7c10, &[]),
];

const fn selector_consumer(
    selector_index: usize,
    load_runtime_address: u32,
    entry_loads: &'static [EntryLoadSpec],
) -> SelectorConsumerSpec {
    SelectorConsumerSpec {
        selector_index,
        load_runtime_address,
        entry_loads,
    }
}

pub(super) fn validate_adopted_selector_consumers(
    mgame: &[u8],
) -> Result<DialogueSelectorConsumerReport> {
    ensure!(
        mgame.len() == MGAME_SIZE && sha256_bytes(mgame) == MGAME_SHA256,
        "adopted selector consumers require the supported MGAME.BIN"
    );

    let consumers = ADOPTED_CONSUMERS
        .iter()
        .map(|spec| validate_consumer(mgame, *spec))
        .collect::<Result<Vec<_>>>()?;
    let direct_entry_load_count = consumers
        .iter()
        .map(|consumer| consumer.entry_pointer_loads.len())
        .sum();
    let report = DialogueSelectorConsumerReport {
        typed_isa_profile: PROFILE_ID.to_string(),
        selector_table_runtime_start: hex_address(SELECTOR_TABLE_RUNTIME_START),
        selector_count: SELECTOR_COUNT,
        direct_load_count: consumers.len(),
        direct_entry_load_count,
        consumers,
    };
    let report_sha256 = sha256_bytes(&serde_json::to_vec(&report)?);
    ensure!(
        report_sha256 == ADOPTED_REPORT_SHA256,
        "adopted selector-consumer report changed: {report_sha256}"
    );
    Ok(report)
}

fn validate_consumer(
    mgame: &[u8],
    spec: SelectorConsumerSpec,
) -> Result<DialogueSelectorConsumerAudit> {
    ensure!(
        spec.selector_index < SELECTOR_COUNT,
        "adopted selector index is out of range"
    );
    let instruction = decode_at(mgame, spec.load_runtime_address)?;
    let Instruction::Lw {
        rt: table_register, ..
    } = instruction
    else {
        anyhow::bail!(
            "adopted selector consumer at {:#010x} is not an LW",
            spec.load_runtime_address
        );
    };
    ensure!(
        table_register != Register::ZERO,
        "adopted selector consumer loads into the zero register"
    );

    let entry_pointer_loads = spec
        .entry_loads
        .iter()
        .map(|entry| validate_entry_load(mgame, table_register, *entry))
        .collect::<Result<Vec<_>>>()?;
    let pointer_storage_runtime_address = SELECTOR_TABLE_RUNTIME_START
        .checked_add(u32::try_from(spec.selector_index * 4)?)
        .context("adopted selector pointer address overflow")?;
    Ok(DialogueSelectorConsumerAudit {
        selector_index: spec.selector_index,
        pointer_storage_runtime_address: hex_address(pointer_storage_runtime_address),
        load_instruction_runtime_address: hex_address(spec.load_runtime_address),
        entry_pointer_loads,
    })
}

fn validate_entry_load(
    mgame: &[u8],
    table_register: Register,
    spec: EntryLoadSpec,
) -> Result<DialogueSelectorEntryConsumerAudit> {
    let instruction = decode_at(mgame, spec.runtime_address)?;
    let expected_displacement = i16::try_from(
        spec.entry_index
            .checked_mul(4)
            .context("adopted selector entry displacement overflow")?,
    )?;
    ensure!(
        matches!(
            instruction,
            Instruction::Lw { base, offset, .. }
                if base == table_register && offset == expected_displacement
        ),
        "adopted selector entry load changed at {:#010x}",
        spec.runtime_address
    );
    Ok(DialogueSelectorEntryConsumerAudit {
        entry_index: spec.entry_index,
        load_instruction_runtime_address: hex_address(spec.runtime_address),
    })
}

fn decode_at(mgame: &[u8], runtime_address: u32) -> Result<Instruction> {
    let offset = runtime_address
        .checked_sub(MGAME_RUNTIME_BASE)
        .context("adopted selector instruction precedes MGAME")?;
    let offset = usize::try_from(offset)?;
    let bytes = mgame
        .get(offset..offset + 4)
        .with_context(|| format!("truncated MGAME instruction at {runtime_address:#010x}"))?;
    decode(
        u32::from_le_bytes(bytes.try_into().unwrap()),
        runtime_address,
    )
    .with_context(|| format!("failed to decode MGAME instruction at {runtime_address:#010x}"))
}
