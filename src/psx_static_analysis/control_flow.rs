use psx_r3000a::{IndirectTarget, Instruction, PsxR3000A, Register};
use typed_isa_core::{ControlAction, ControlTarget, StaticSemantics};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct FlowSuccessors {
    pub(crate) delay_slot: Option<u32>,
    pub(crate) target: Option<FlowTarget>,
    pub(crate) fallthrough: Option<u32>,
    pub(crate) return_site: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum FlowTarget {
    Direct(u32),
    Register(Register),
}

pub(crate) fn flow_successors(instruction: &Instruction, pc: u32) -> FlowSuccessors {
    let semantics = PsxR3000A::semantics(instruction, &pc)
        .expect("aligned decoded R3000A instruction has static semantics");
    let (target, return_site) = match semantics.control_flow.action {
        ControlAction::Transfer { target } => (Some(target), None),
        ControlAction::LinkedTransfer {
            target,
            return_site,
        } => (Some(target), Some(return_site)),
        ControlAction::Continue
        | ControlAction::Return { .. }
        | ControlAction::ExceptionReturn { .. }
        | ControlAction::Boundary(_) => (None, None),
    };
    let target = match target {
        Some(ControlTarget::Direct(target)) => Some(FlowTarget::Direct(target)),
        Some(ControlTarget::Indirect(IndirectTarget::Register(register))) => {
            Some(FlowTarget::Register(register))
        }
        Some(ControlTarget::Indirect(IndirectTarget::GeneralExceptionVector)) | None => None,
    };
    FlowSuccessors {
        delay_slot: semantics
            .control_flow
            .delay_slot
            .map(|delay_slot| delay_slot.point),
        target,
        fallthrough: semantics.control_flow.fallthrough,
        return_site,
    }
}
