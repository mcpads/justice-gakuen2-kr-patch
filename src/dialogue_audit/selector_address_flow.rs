use anyhow::{Context, Result};
use psx_r3000a::{Instruction, PROFILE_ID, decode};

use crate::cue::CueSheet;
use crate::pipeline::sha256_bytes;
use crate::psx_static_analysis::ExecutableDomain;
use crate::psx_static_analysis::value_flow::{
    DerivedAddress, DerivedAddressKind, ResolvedRegisterTransfer, scan_derived_address_flow,
};

use super::diary_save_slot_marker_flow::validate_diary_save_slot_marker_flow;
use super::format::hex_address;
use super::script_source::load_mgame;
use super::selector_address_flow_model::{
    DialogueFunctionCaller, DialogueMessageConstructorCall, DialogueResolvedMessageConstructorCall,
    DialogueSelectorAddressFlowAuditConfig, DialogueSelectorAddressFlowAuditReport,
    DialogueSelectorPointerLoad, DialogueSelectorRegionSeed,
    DialogueSelectorTableBaseMaterialization, DialogueTypedInstruction,
};
use super::translation_workspace_io::json_bytes;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const SELECTOR_TABLE_RUNTIME_START: u32 = 0x8010_1000;
const SELECTOR_COUNT: usize = 8;
const MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS: u32 = 0x800b_84d4;
const MESSAGE_BANK_CONSTRUCTOR_RUNTIME_ADDRESS: u32 = 0x800b_8534;

pub fn audit_dialogue_selector_address_flow(
    config: &DialogueSelectorAddressFlowAuditConfig,
) -> Result<DialogueSelectorAddressFlowAuditReport> {
    let cue = CueSheet::parse(&config.cue)?;
    let mgame = load_mgame(&cue)?;
    let executable_domain = ExecutableDomain::full_image(mgame.len());
    let address_flow = scan_derived_address_flow(&mgame, MGAME_RUNTIME_BASE, &executable_domain);
    let selector_region_high_half_seeds = selector_region_high_half_seeds(&mgame)?;
    let materializations = selector_table_base_materializations(&address_flow.addresses)?;
    let message_entry_constructor_calls = direct_calls_to(
        &mgame,
        MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS,
        &address_flow.addresses,
    )?;
    let resolved_register_message_entry_constructor_calls = resolved_register_calls_to(
        &address_flow.resolved_register_transfers,
        MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS,
    )?;
    let message_bank_constructor_calls = direct_calls_to(
        &mgame,
        MESSAGE_BANK_CONSTRUCTOR_RUNTIME_ADDRESS,
        &address_flow.addresses,
    )?;
    let resolved_register_message_bank_constructor_calls = resolved_register_calls_to(
        &address_flow.resolved_register_transfers,
        MESSAGE_BANK_CONSTRUCTOR_RUNTIME_ADDRESS,
    )?;
    let diary_save_slot_marker_flow = validate_diary_save_slot_marker_flow(&mgame)?;
    let report = DialogueSelectorAddressFlowAuditReport {
        kind: "Justice Gakuen 2 dialogue selector address-flow audit".to_string(),
        source_mgame_sha256: sha256_bytes(&mgame),
        typed_isa_profile: PROFILE_ID.to_string(),
        selector_table_runtime_start: hex_address(SELECTOR_TABLE_RUNTIME_START),
        selector_count: SELECTOR_COUNT,
        selector_region_high_half_seed_count: selector_region_high_half_seeds.len(),
        selector_region_high_half_seeds,
        table_base_materialization_count: materializations.len(),
        table_base_materializations: materializations,
        message_entry_constructor_runtime_address: hex_address(
            MESSAGE_ENTRY_CONSTRUCTOR_RUNTIME_ADDRESS,
        ),
        direct_message_entry_constructor_call_count: message_entry_constructor_calls.len(),
        direct_message_entry_constructor_calls: message_entry_constructor_calls,
        resolved_register_message_entry_constructor_call_count:
            resolved_register_message_entry_constructor_calls.len(),
        resolved_register_message_entry_constructor_calls,
        message_bank_constructor_runtime_address: hex_address(
            MESSAGE_BANK_CONSTRUCTOR_RUNTIME_ADDRESS,
        ),
        direct_message_bank_constructor_call_count: message_bank_constructor_calls.len(),
        direct_message_bank_constructor_calls: message_bank_constructor_calls,
        resolved_register_message_bank_constructor_call_count:
            resolved_register_message_bank_constructor_calls.len(),
        resolved_register_message_bank_constructor_calls,
        diary_save_slot_marker_flow,
    };
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&config.output, json_bytes(&report)?)
        .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

pub(super) fn selector_region_high_half_seeds(
    mgame: &[u8],
) -> Result<Vec<DialogueSelectorRegionSeed>> {
    let mut seeds = Vec::new();
    for instruction_offset in (0..mgame.len().saturating_sub(3)).step_by(4) {
        let pc = runtime_address(instruction_offset)?;
        if matches!(
            decode(read_word(mgame, instruction_offset), pc),
            Ok(Instruction::Lui { rt, immediate })
                if rt != psx_r3000a::Register::ZERO && immediate == 0x8010
        ) {
            seeds.push(DialogueSelectorRegionSeed {
                seed_instruction_runtime_address: hex_address(pc),
                diagnostic_use_window: diagnostic_forward_window(mgame, instruction_offset)?,
            });
        }
    }
    Ok(seeds)
}

pub(super) fn resolved_register_calls_to(
    transfers: &[ResolvedRegisterTransfer],
    target_runtime_address: u32,
) -> Result<Vec<DialogueResolvedMessageConstructorCall>> {
    transfers
        .iter()
        .filter(|transfer| transfer.target == target_runtime_address)
        .map(|transfer| {
            Ok(DialogueResolvedMessageConstructorCall {
                seed_instruction_runtime_address: hex_address(runtime_address(
                    transfer.seed_offset,
                )?),
                call_instruction_runtime_address: hex_address(runtime_address(
                    transfer.instruction_offset,
                )?),
            })
        })
        .collect()
}

pub(super) fn direct_calls_to(
    mgame: &[u8],
    target_runtime_address: u32,
    derived_addresses: &[DerivedAddress],
) -> Result<Vec<DialogueMessageConstructorCall>> {
    let mut calls = Vec::new();
    for instruction_offset in direct_call_instruction_offsets(mgame, target_runtime_address)? {
        let pc = runtime_address(instruction_offset)?;
        let enclosing_function_offset = enclosing_function_offset(mgame, instruction_offset)?;
        let direct_enclosing_function_calls = enclosing_function_offset
            .map(|function_offset| direct_function_callers(mgame, function_offset))
            .transpose()?
            .unwrap_or_default();
        let enclosing_function_selector_pointer_loads = selector_pointer_loads_between(
            mgame,
            derived_addresses,
            enclosing_function_offset.unwrap_or(instruction_offset),
            instruction_offset,
        )?;
        calls.push(DialogueMessageConstructorCall {
            call_instruction_runtime_address: hex_address(pc),
            diagnostic_setup_window: diagnostic_setup_window(mgame, instruction_offset)?,
            enclosing_function_runtime_address: enclosing_function_offset
                .map(runtime_address)
                .transpose()?
                .map(hex_address),
            direct_enclosing_function_call_count: direct_enclosing_function_calls.len(),
            direct_enclosing_function_calls,
            enclosing_function_selector_pointer_load_count:
                enclosing_function_selector_pointer_loads.len(),
            enclosing_function_selector_pointer_loads,
        });
    }
    Ok(calls)
}

fn selector_pointer_loads_between(
    mgame: &[u8],
    derived_addresses: &[DerivedAddress],
    start_offset: usize,
    end_offset: usize,
) -> Result<Vec<DialogueSelectorPointerLoad>> {
    let selector_table_runtime_end = SELECTOR_TABLE_RUNTIME_START
        .checked_add(u32::try_from(SELECTOR_COUNT * 4)?)
        .context("selector table address overflow")?;
    let mut loads = Vec::new();
    for reference in derived_addresses {
        if reference.kind != DerivedAddressKind::MemoryAccess
            || !(start_offset..=end_offset).contains(&reference.source_offset)
            || !(SELECTOR_TABLE_RUNTIME_START..selector_table_runtime_end)
                .contains(&reference.address)
        {
            continue;
        }
        let relative = reference.address - SELECTOR_TABLE_RUNTIME_START;
        if !relative.is_multiple_of(4) {
            continue;
        }
        let pc = runtime_address(reference.source_offset)?;
        if !matches!(
            decode(read_word(mgame, reference.source_offset), pc),
            Ok(Instruction::Lw { .. })
        ) {
            continue;
        }
        loads.push(DialogueSelectorPointerLoad {
            selector_index: usize::try_from(relative / 4)?,
            pointer_storage_runtime_address: hex_address(reference.address),
            load_instruction_runtime_address: hex_address(pc),
        });
    }
    loads.sort_by(|left, right| {
        left.load_instruction_runtime_address
            .cmp(&right.load_instruction_runtime_address)
            .then_with(|| left.selector_index.cmp(&right.selector_index))
    });
    loads.dedup();
    Ok(loads)
}

fn direct_call_instruction_offsets(mgame: &[u8], target: u32) -> Result<Vec<usize>> {
    let mut calls = Vec::new();
    for instruction_offset in (0..mgame.len().saturating_sub(3)).step_by(4) {
        let pc = runtime_address(instruction_offset)?;
        if matches!(decode(read_word(mgame, instruction_offset), pc), Ok(Instruction::Jal { target: decoded }) if decoded == target)
        {
            calls.push(instruction_offset);
        }
    }
    Ok(calls)
}

fn enclosing_function_offset(
    mgame: &[u8],
    call_instruction_offset: usize,
) -> Result<Option<usize>> {
    const MAX_PRECEDING_INSTRUCTION_COUNT: usize = 192;

    let start = call_instruction_offset.saturating_sub(MAX_PRECEDING_INSTRUCTION_COUNT * 4);
    let mut instruction_offset = call_instruction_offset;
    loop {
        let pc = runtime_address(instruction_offset)?;
        if matches!(
            decode(read_word(mgame, instruction_offset), pc),
            Ok(Instruction::Addiu { rt, rs, immediate })
                if rt == psx_r3000a::Register::SP
                    && rs == psx_r3000a::Register::SP
                    && immediate < 0
        ) {
            return Ok(Some(instruction_offset));
        }
        if instruction_offset == start {
            break;
        }
        instruction_offset -= 4;
    }
    Ok(None)
}

fn direct_function_callers(
    mgame: &[u8],
    function_offset: usize,
) -> Result<Vec<DialogueFunctionCaller>> {
    let target = runtime_address(function_offset)?;
    direct_call_instruction_offsets(mgame, target)?
        .into_iter()
        .map(|instruction_offset| {
            Ok(DialogueFunctionCaller {
                call_instruction_runtime_address: hex_address(runtime_address(instruction_offset)?),
                diagnostic_setup_window: diagnostic_setup_window(mgame, instruction_offset)?,
            })
        })
        .collect()
}

fn diagnostic_setup_window(
    mgame: &[u8],
    call_instruction_offset: usize,
) -> Result<Vec<DialogueTypedInstruction>> {
    const PRECEDING_INSTRUCTION_COUNT: usize = 192;
    const FOLLOWING_INSTRUCTION_COUNT: usize = 1;

    let start = call_instruction_offset.saturating_sub(PRECEDING_INSTRUCTION_COUNT * 4);
    let end = call_instruction_offset
        .checked_add((FOLLOWING_INSTRUCTION_COUNT + 1) * 4)
        .context("message constructor diagnostic window overflow")?
        .min(mgame.len());
    let mut instructions = Vec::new();
    for instruction_offset in (start..end.saturating_sub(3)).step_by(4) {
        let pc = runtime_address(instruction_offset)?;
        let word = read_word(mgame, instruction_offset);
        if let Ok(instruction) = decode(word, pc) {
            instructions.push(DialogueTypedInstruction {
                runtime_address: hex_address(pc),
                instruction: format!("{instruction:?}"),
            });
        }
    }
    Ok(instructions)
}

fn diagnostic_forward_window(
    mgame: &[u8],
    seed_instruction_offset: usize,
) -> Result<Vec<DialogueTypedInstruction>> {
    const FOLLOWING_INSTRUCTION_COUNT: usize = 96;

    let end = seed_instruction_offset
        .checked_add((FOLLOWING_INSTRUCTION_COUNT + 1) * 4)
        .context("selector-region diagnostic window overflow")?
        .min(mgame.len());
    let mut instructions = Vec::new();
    for instruction_offset in (seed_instruction_offset..end.saturating_sub(3)).step_by(4) {
        let pc = runtime_address(instruction_offset)?;
        let word = read_word(mgame, instruction_offset);
        if let Ok(instruction) = decode(word, pc) {
            instructions.push(DialogueTypedInstruction {
                runtime_address: hex_address(pc),
                instruction: format!("{instruction:?}"),
            });
        }
    }
    Ok(instructions)
}

fn read_word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

pub(super) fn selector_table_base_materializations(
    references: &[DerivedAddress],
) -> Result<Vec<DialogueSelectorTableBaseMaterialization>> {
    let mut materializations = references
        .iter()
        .filter(|reference| {
            reference.kind == DerivedAddressKind::RegisterValue
                && reference.address == SELECTOR_TABLE_RUNTIME_START
        })
        .map(|reference| {
            Ok(DialogueSelectorTableBaseMaterialization {
                table_base_runtime_address: hex_address(reference.address),
                seed_instruction_runtime_address: hex_address(runtime_address(
                    reference.seed_offset,
                )?),
                materialization_instruction_runtime_address: hex_address(runtime_address(
                    reference.instruction_offset,
                )?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    materializations.dedup_by(|left, right| {
        left.table_base_runtime_address == right.table_base_runtime_address
            && left.seed_instruction_runtime_address == right.seed_instruction_runtime_address
    });
    Ok(materializations)
}

fn runtime_address(offset: usize) -> Result<u32> {
    MGAME_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("selector address-flow runtime address overflow")
}
