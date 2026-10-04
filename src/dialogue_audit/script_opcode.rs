use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptFlow {
    Fallthrough,
    ConditionalRouteTransfer,
    RouteTransfer,
    Terminal,
    OuterStateYield,
}

impl ScriptFlow {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Fallthrough => "fallthrough",
            Self::ConditionalRouteTransfer => "conditional_route_transfer",
            Self::RouteTransfer => "route_transfer",
            Self::Terminal => "terminal",
            Self::OuterStateYield => "outer_state_yield",
        }
    }

    pub(super) const fn ends_linear_stream(self) -> bool {
        matches!(
            self,
            Self::RouteTransfer | Self::Terminal | Self::OuterStateYield
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OpcodeSpec {
    pub(super) width: usize,
    pub(super) flow: ScriptFlow,
    pub(super) message_index_bytes: Option<(usize, usize)>,
}

pub(super) fn opcode_spec(opcode: u8) -> Result<OpcodeSpec> {
    if opcode > 0x76 {
        bail!("opcode 0x{opcode:02x} is outside the MGAME dispatch table");
    }
    let width = match opcode {
        0x10 | 0x2f..=0x31 | 0x39..=0x3b | 0x45 | 0x63 | 0x64 => 1,
        0x4b => 7,
        0x15
        | 0x17
        | 0x19
        | 0x1e
        | 0x3e
        | 0x3f
        | 0x51
        | 0x57..=0x59
        | 0x65
        | 0x69
        | 0x6b..=0x6d => 8,
        0x20 | 0x21 | 0x25 | 0x52 | 0x54 => 12,
        0x24 => 16,
        _ => 4,
    };
    let flow = match opcode {
        0x63 | 0x64 => ScriptFlow::OuterStateYield,
        0x1d | 0x4f | 0x59 | 0x6a => ScriptFlow::ConditionalRouteTransfer,
        0x3e
        | 0x3f
        | 0x43
        | 0x44
        | 0x47
        | 0x4e
        | 0x51..=0x53
        | 0x55
        | 0x57
        | 0x58
        | 0x65
        | 0x68
        | 0x69
        | 0x6b..=0x6d => ScriptFlow::RouteTransfer,
        0x4a..=0x4c => ScriptFlow::Terminal,
        _ => ScriptFlow::Fallthrough,
    };
    let message_index_bytes = match opcode {
        0x1e | 0x1f | 0x22 | 0x24 => Some((2, 3)),
        0x20 | 0x21 | 0x25 => Some((1, 2)),
        _ => None,
    };
    Ok(OpcodeSpec {
        width,
        flow,
        message_index_bytes,
    })
}

pub(super) fn command_kind(opcode: u8) -> &'static str {
    match opcode {
        0x1e | 0x1f | 0x22 => "show_dialogue_message",
        0x20 => "show_dialogue_message_with_presentation",
        0x21 => "show_dialogue_choice",
        0x24 => "show_dialogue_message_with_actor_state",
        0x25 => "show_dialogue_message_with_presentation",
        0x4a..=0x4c => "end_script_stream",
        0x1d
        | 0x3e
        | 0x3f
        | 0x43
        | 0x44
        | 0x47
        | 0x4e
        | 0x4f
        | 0x51..=0x53
        | 0x55
        | 0x57..=0x59
        | 0x65
        | 0x68..=0x6d => "select_route_segment",
        _ => "opaque_runtime_command",
    }
}
