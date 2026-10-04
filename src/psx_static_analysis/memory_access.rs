use psx_r3000a::Instruction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemoryAccessOperation {
    Read,
    Write,
}

impl MemoryAccessOperation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedMemoryAccess {
    pub(crate) operation: MemoryAccessOperation,
    pub(crate) width_bytes: usize,
}

pub(crate) fn decoded_memory_access(instruction: &Instruction) -> Option<DecodedMemoryAccess> {
    let (operation, width_bytes) = match instruction {
        Instruction::Lb { .. } | Instruction::Lbu { .. } => (MemoryAccessOperation::Read, 1),
        Instruction::Lh { .. } | Instruction::Lhu { .. } => (MemoryAccessOperation::Read, 2),
        Instruction::Lw { .. }
        | Instruction::Lwl { .. }
        | Instruction::Lwr { .. }
        | Instruction::Lwc2 { .. } => (MemoryAccessOperation::Read, 4),
        Instruction::Sb { .. } => (MemoryAccessOperation::Write, 1),
        Instruction::Sh { .. } => (MemoryAccessOperation::Write, 2),
        Instruction::Sw { .. }
        | Instruction::Swl { .. }
        | Instruction::Swr { .. }
        | Instruction::Swc2 { .. } => (MemoryAccessOperation::Write, 4),
        _ => return None,
    };
    Some(DecodedMemoryAccess {
        operation,
        width_bytes,
    })
}
