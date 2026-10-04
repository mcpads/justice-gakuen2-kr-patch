use psx_r3000a::{Instruction, PsxR3000A, Register, decode};
use typed_isa_core::{ControlAction, StaticSemantics};

use super::ExecutableDomain;

const BRANCH_DISTANCES_BEFORE_SHIFT: [usize; 2] = [4, 8];
const MAXIMUM_BOUND_SEARCH_DISTANCE: usize = 0x20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedJumpTable {
    pub(crate) bound_instruction_offset: usize,
    pub(crate) transfer_instruction_offset: usize,
    pub(crate) selector_register: Register,
    pub(crate) table_address: u32,
    pub(crate) table_offset: usize,
    pub(crate) targets: Vec<u32>,
}

pub(crate) fn read_bounded_jump_table(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    transfer_instruction_offset: usize,
) -> Option<BoundedJumpTable> {
    if !executable_domain.matches_image_len(data.len()) {
        return None;
    }
    BRANCH_DISTANCES_BEFORE_SHIFT
        .into_iter()
        .find_map(|branch_distance_before_shift| {
            read_bounded_jump_table_with_branch_distance(
                data,
                instruction_base,
                executable_domain,
                transfer_instruction_offset,
                branch_distance_before_shift,
            )
        })
}

fn read_bounded_jump_table_with_branch_distance(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    transfer_instruction_offset: usize,
    branch_distance_before_shift: usize,
) -> Option<BoundedJumpTable> {
    let transfer_pc =
        instruction_base.checked_add(u32::try_from(transfer_instruction_offset).ok()?)?;
    let shift_instruction_offset = transfer_instruction_offset.checked_sub(0x14)?;
    let shift_pc = transfer_pc.checked_sub(0x14)?;
    let branch_instruction_offset =
        shift_instruction_offset.checked_sub(branch_distance_before_shift)?;
    let branch_pc = shift_pc.checked_sub(u32::try_from(branch_distance_before_shift).ok()?)?;

    let Instruction::Beq {
        rs: bound_register,
        rt: branch_zero,
        target: out_of_range_target,
    } = decode_instruction(
        data,
        executable_domain,
        branch_instruction_offset,
        branch_pc,
    )?
    else {
        return None;
    };
    if branch_zero != Register::ZERO {
        return None;
    }
    if branch_distance_before_shift == 8 {
        let delay_slot = decode_instruction(
            data,
            executable_domain,
            shift_instruction_offset - 4,
            shift_pc - 4,
        )?;
        if !continues_to_next_instruction(&delay_slot, shift_pc - 4) {
            return None;
        }
    }

    let (bound_instruction_offset, selector_register, entry_count) = find_unsigned_bound(
        data,
        instruction_base,
        executable_domain,
        branch_instruction_offset,
        bound_register,
    )?;
    if !register_is_unchanged(
        data,
        instruction_base,
        executable_domain,
        bound_instruction_offset + 4,
        shift_instruction_offset,
        selector_register,
    ) {
        return None;
    }

    read_bounded_jump_table_tail(
        data,
        instruction_base,
        executable_domain,
        transfer_instruction_offset,
        shift_instruction_offset,
        shift_pc,
        out_of_range_target,
        bound_instruction_offset,
        selector_register,
        entry_count,
    )
}

fn register_is_unchanged(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    start_offset: usize,
    end_offset: usize,
    register: Register,
) -> bool {
    (start_offset..end_offset)
        .step_by(4)
        .all(|instruction_offset| {
            let Some(pc) = u32::try_from(instruction_offset)
                .ok()
                .and_then(|offset| instruction_base.checked_add(offset))
            else {
                return false;
            };
            decode_instruction(data, executable_domain, instruction_offset, pc)
                .is_some_and(|instruction| instruction.written_gpr() != Some(register))
        })
}

fn find_unsigned_bound(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    branch_instruction_offset: usize,
    bound_register: Register,
) -> Option<(usize, Register, usize)> {
    for distance in (4..=MAXIMUM_BOUND_SEARCH_DISTANCE).step_by(4) {
        let instruction_offset = branch_instruction_offset.checked_sub(distance)?;
        let pc = instruction_base.checked_add(u32::try_from(instruction_offset).ok()?)?;
        let instruction = decode_instruction(data, executable_domain, instruction_offset, pc)?;
        if let Instruction::Sltiu {
            rt,
            rs: selector_register,
            immediate,
        } = instruction
            && rt == bound_register
        {
            let entry_count = usize::from(u16::try_from(immediate).ok()?);
            return (entry_count > 0).then_some((
                instruction_offset,
                selector_register,
                entry_count,
            ));
        }
        if instruction.written_gpr() == Some(bound_register)
            || !continues_to_next_instruction(&instruction, pc)
        {
            return None;
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn read_bounded_jump_table_tail(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    transfer_instruction_offset: usize,
    shift_instruction_offset: usize,
    shift_pc: u32,
    out_of_range_target: u32,
    bound_instruction_offset: usize,
    selector_register: Register,
    entry_count: usize,
) -> Option<BoundedJumpTable> {
    let transfer_pc =
        instruction_base.checked_add(u32::try_from(transfer_instruction_offset).ok()?)?;

    let Instruction::Sll {
        rd: index_register,
        rt: shifted_selector_register,
        shift: 2,
    } = decode_instruction(data, executable_domain, shift_instruction_offset, shift_pc)?
    else {
        return None;
    };
    if shifted_selector_register != selector_register {
        return None;
    }

    let Instruction::Lui {
        rt: table_register,
        immediate: table_high,
    } = decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset - 0x10,
        transfer_pc - 0x10,
    )?
    else {
        return None;
    };
    let Instruction::Addu {
        rd: address_register,
        rs: address_left,
        rt: address_right,
    } = decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset - 0x0c,
        transfer_pc - 0x0c,
    )?
    else {
        return None;
    };
    if address_register != table_register
        || address_left != table_register
        || address_right != index_register
    {
        return None;
    }

    let Instruction::Lw {
        rt: target_register,
        base: load_base,
        offset: table_low,
    } = decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset - 8,
        transfer_pc - 8,
    )?
    else {
        return None;
    };
    if load_base != table_register {
        return None;
    }
    if decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset - 4,
        transfer_pc - 4,
    )? != Instruction::nop()
    {
        return None;
    }
    if decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset,
        transfer_pc,
    )? != (Instruction::Jr {
        rs: target_register,
    }) {
        return None;
    }
    if decode_instruction(
        data,
        executable_domain,
        transfer_instruction_offset + 4,
        transfer_pc + 4,
    )? != Instruction::nop()
    {
        return None;
    }

    let dispatch_end = transfer_pc.checked_add(4)?;
    if (shift_pc..=dispatch_end).contains(&out_of_range_target) {
        return None;
    }

    let table_address = (u32::from(table_high) << 16).wrapping_add_signed(i32::from(table_low));
    let table_offset = image_offset(data, instruction_base, table_address)?;
    let table_end = table_offset.checked_add(entry_count.checked_mul(4)?)?;
    let table_bytes = data.get(table_offset..table_end)?;
    let mut targets = Vec::with_capacity(entry_count);
    for entry in table_bytes.as_chunks::<4>().0 {
        let target = u32::from_le_bytes(*entry);
        executable_domain.instruction_offset(instruction_base, target)?;
        targets.push(target);
    }

    Some(BoundedJumpTable {
        bound_instruction_offset,
        transfer_instruction_offset,
        selector_register,
        table_address,
        table_offset,
        targets,
    })
}

fn continues_to_next_instruction(instruction: &Instruction, pc: u32) -> bool {
    PsxR3000A::semantics(instruction, &pc).is_ok_and(|semantics| {
        semantics.control_flow.action == ControlAction::Continue
            && semantics.control_flow.delay_slot.is_none()
            && semantics.control_flow.fallthrough == pc.checked_add(4)
    })
}

fn decode_instruction(
    data: &[u8],
    executable_domain: &ExecutableDomain,
    instruction_offset: usize,
    pc: u32,
) -> Option<Instruction> {
    if !executable_domain.contains_instruction_offset(instruction_offset) {
        return None;
    }
    let bytes: [u8; 4] = data
        .get(instruction_offset..instruction_offset.checked_add(4)?)?
        .try_into()
        .ok()?;
    decode(u32::from_le_bytes(bytes), pc).ok()
}

fn image_offset(data: &[u8], instruction_base: u32, address: u32) -> Option<usize> {
    address
        .checked_sub(instruction_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .filter(|offset| offset.is_multiple_of(4) && offset + 4 <= data.len())
}

#[cfg(test)]
#[path = "bounded_jump_tables_tests.rs"]
mod tests;
