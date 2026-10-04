//! Memory-operand support for the shared PSX value-flow analyzer.

use psx_r3000a::{Instruction, Register};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MemoryOperand {
    pub(super) base: Register,
    pub(super) displacement: i16,
    pub(super) load_destination: Option<Register>,
    pub(super) scalar_load: Option<ScalarLoad>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScalarLoad {
    pub(super) destination: Register,
    pub(super) width_bytes: u8,
    pub(super) sign_extended: bool,
}

pub(super) fn memory_operand(instruction: &Instruction) -> Option<MemoryOperand> {
    let (base, displacement, load_destination, scalar_load) = match instruction {
        Instruction::Lb { rt, base, offset } => (
            *base,
            *offset,
            Some(*rt),
            Some(ScalarLoad {
                destination: *rt,
                width_bytes: 1,
                sign_extended: true,
            }),
        ),
        Instruction::Lbu { rt, base, offset } => (
            *base,
            *offset,
            Some(*rt),
            Some(ScalarLoad {
                destination: *rt,
                width_bytes: 1,
                sign_extended: false,
            }),
        ),
        Instruction::Lh { rt, base, offset } => (
            *base,
            *offset,
            Some(*rt),
            Some(ScalarLoad {
                destination: *rt,
                width_bytes: 2,
                sign_extended: true,
            }),
        ),
        Instruction::Lhu { rt, base, offset } => (
            *base,
            *offset,
            Some(*rt),
            Some(ScalarLoad {
                destination: *rt,
                width_bytes: 2,
                sign_extended: false,
            }),
        ),
        Instruction::Lwl { rt, base, offset } | Instruction::Lwr { rt, base, offset } => {
            (*base, *offset, Some(*rt), None)
        }
        Instruction::Sb { base, offset, .. }
        | Instruction::Sh { base, offset, .. }
        | Instruction::Sw { base, offset, .. }
        | Instruction::Swl { base, offset, .. }
        | Instruction::Swr { base, offset, .. } => (*base, *offset, None, None),
        Instruction::Lw { rt, base, offset } => (
            *base,
            *offset,
            Some(*rt),
            Some(ScalarLoad {
                destination: *rt,
                width_bytes: 4,
                sign_extended: false,
            }),
        ),
        Instruction::Lwc2 { base, offset, .. } | Instruction::Swc2 { base, offset, .. } => {
            (*base, *offset, None, None)
        }
        _ => return None,
    };
    Some(MemoryOperand {
        base,
        displacement,
        load_destination,
        scalar_load,
    })
}

pub(super) fn read_scalar_value(
    data: &[u8],
    runtime_base: u32,
    address: u32,
    load: ScalarLoad,
) -> Option<u32> {
    if !address.is_multiple_of(u32::from(load.width_bytes)) {
        return None;
    }
    let offset = usize::try_from(address.checked_sub(runtime_base)?).ok()?;
    let end = offset.checked_add(usize::from(load.width_bytes))?;
    let bytes = data.get(offset..end)?;
    match (load.width_bytes, load.sign_extended) {
        (1, false) => Some(u32::from(bytes[0])),
        (1, true) => Some(i32::from(i8::from_le_bytes([bytes[0]])) as u32),
        (2, false) => Some(u32::from(u16::from_le_bytes(bytes.try_into().ok()?))),
        (2, true) => Some(i32::from(i16::from_le_bytes(bytes.try_into().ok()?)) as u32),
        (4, false) => Some(u32::from_le_bytes(bytes.try_into().ok()?)),
        _ => None,
    }
}
