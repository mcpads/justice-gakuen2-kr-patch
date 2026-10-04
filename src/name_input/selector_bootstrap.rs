//! Relocate selector code before font loading overwrites its staging resource.
//! The installer must separately prove staging execution and RAM ownership.
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

use super::selector_loader::{overlaps, ram_range};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorPayloadSplit {
    pub first_byte_count: usize,
    pub second_source: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorBootstrapLayout {
    pub origin: u32,
    pub byte_capacity: usize,
    pub payload_source: u32,
    pub payload_destination: u32,
    pub payload_byte_count: usize,
    #[serde(default)]
    pub payload_split: Option<SelectorPayloadSplit>,
    pub enter_critical_address: u32,
    pub flush_cache_address: u32,
    pub exit_critical_address: u32,
}

/// Normalize only the two main-RAM segments used for staged instructions.
/// Comparing normalized ranges prevents a cached/uncached alias hiding overlap.
pub(super) fn staging_ram_range(address: u32, count: usize) -> Result<std::ops::Range<u32>> {
    let cached = if (0xa000_0000..0xa020_0000).contains(&address) {
        address - 0x2000_0000
    } else {
        address
    };
    ram_range(cached, count)
}

/// PLSEL1's post-SELP load site. An indirect call is needed because JAL cannot
/// cross from KSEG0 into the uncached KSEG1 staging address.
pub const SELECTOR_BOOTSTRAP_CALL_SITE: u32 = 0x800a2bb0;
pub const SELECTOR_BOOTSTRAP_RETURN: u32 = 0x800a2bc0;

/// Validate the displaced source setup and emit its four-instruction entry.
/// The resident loader must replay that setup before returning to 800a2bc0.
/// This does not install a write or establish source-record ownership.
pub fn build_selector_bootstrap_entry(source: &[u8], target: u32) -> Result<Vec<u8>> {
    ensure!(
        (0xa0000000..0xa0200000).contains(&target) && target.is_multiple_of(4),
        "selector staging entry must be aligned uncached main RAM"
    );
    let expected: Vec<u8> = [0x3c02801fu32, 0x8c426360, 0x3c04800d, 0x8c420150]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    ensure!(
        source == expected,
        "PLSEL1 post-load source instructions changed"
    );
    let mut a = Assembler::new();
    a.emit_all(load_address(Register::T6, target))
        .emit(Instruction::Jalr {
            rd: Register::RA,
            rs: Register::T6,
        })
        .emit(Instruction::nop());
    let bytes = a.assemble(SELECTOR_BOOTSTRAP_CALL_SITE)?.bytes().to_vec();
    ensure!(
        bytes.len() == source.len(),
        "selector entry changed source extent"
    );
    verify_placed_program(&bytes, SELECTOR_BOOTSTRAP_CALL_SITE)?;
    Ok(bytes)
}

/// A source-owned data slice relocated before font loading destroys staging.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorDataCopy {
    pub source: u32,
    pub destination: u32,
    pub byte_count: usize,
}

/// Copy a word-aligned payload, flush its instruction cache with IRQs disabled,
/// and tail-call its first instruction with the original stack and return address.
/// ExitCriticalSection is called only when EnterCriticalSection returned nonzero.
/// The payload supplies its own arguments; caller-saved registers are scratch.
pub fn build_selector_bootstrap(layout: &SelectorBootstrapLayout) -> Result<Vec<u8>> {
    build_selector_bootstrap_with_data(layout, &[])
}

pub fn build_selector_bootstrap_with_data(
    layout: &SelectorBootstrapLayout,
    data_copies: &[SelectorDataCopy],
) -> Result<Vec<u8>> {
    let code = staging_ram_range(layout.origin, layout.byte_capacity)?;
    let first_count = layout
        .payload_split
        .as_ref()
        .map_or(layout.payload_byte_count, |s| s.first_byte_count);
    let mut sources = vec![ram_range(layout.payload_source, first_count)?];
    if let Some(split) = &layout.payload_split {
        ensure!(
            split.first_byte_count > 0
                && split.first_byte_count < layout.payload_byte_count
                && split.first_byte_count.is_multiple_of(4)
                && split.second_source.is_multiple_of(4),
            "selector split must contain two nonempty word-aligned pieces"
        );
        sources.push(ram_range(
            split.second_source,
            layout.payload_byte_count - split.first_byte_count,
        )?);
    }
    let destination = ram_range(layout.payload_destination, layout.payload_byte_count)?;
    ensure!(
        [
            layout.origin,
            layout.payload_source,
            layout.payload_destination,
            layout.enter_critical_address,
            layout.flush_cache_address,
            layout.exit_critical_address
        ]
        .iter()
        .all(|address| address.is_multiple_of(4))
            && layout.payload_byte_count.is_multiple_of(4),
        "selector bootstrap requires word-aligned code and payload"
    );
    ensure!(
        !overlaps(&code, &destination)
            && sources
                .iter()
                .all(|s| !overlaps(&code, s) && !overlaps(s, &destination))
            && (sources.len() == 1 || !overlaps(&sources[0], &sources[1])),
        "selector bootstrap code and payload ranges overlap"
    );
    let mut callees = Vec::new();
    for address in [
        layout.enter_critical_address,
        layout.flush_cache_address,
        layout.exit_critical_address,
    ] {
        let callee = ram_range(address, 16)?;
        ensure!(
            [&code, &destination]
                .into_iter()
                .chain(sources.iter())
                .all(|range| !overlaps(range, &callee))
                && callees.iter().all(|prior| !overlaps(prior, &callee)),
            "selector bootstrap overwrites or aliases a native service stub"
        );
        callees.push(callee);
    }
    let mut transfers = Vec::new();
    let mut output = destination.start;
    for source in &sources {
        transfers.push((source.clone(), output));
        output += source.end - source.start;
    }
    let mut data_destinations = Vec::new();
    for copy in data_copies {
        ensure!(
            copy.source.is_multiple_of(4)
                && copy.destination.is_multiple_of(4)
                && copy.byte_count.is_multiple_of(4),
            "unaligned selector data copy"
        );
        let source = ram_range(copy.source, copy.byte_count)?;
        let target = ram_range(copy.destination, copy.byte_count)?;
        ensure!(
            !overlaps(&source, &code)
                && !overlaps(&target, &code)
                && !overlaps(&source, &destination)
                && !overlaps(&target, &destination)
                && !overlaps(&source, &target)
                && callees
                    .iter()
                    .all(|r| !overlaps(r, &source) && !overlaps(r, &target))
                && sources
                    .iter()
                    .all(|r| !overlaps(r, &source) && !overlaps(r, &target))
                && data_destinations
                    .iter()
                    .all(|r| !overlaps(r, &source) && !overlaps(r, &target)),
            "selector data copy aliases code, source, destination or service"
        );
        sources.push(source.clone());
        data_destinations.push(target);
        transfers.push((source, copy.destination));
    }
    use Instruction::*;
    let mut a = Assembler::new();
    a.emit(Addiu {
        rt: Register::SP,
        rs: Register::SP,
        immediate: -24,
    })
    .emit(Sw {
        rt: Register::RA,
        base: Register::SP,
        offset: 20,
    });
    for (index, (source, output)) in transfers.iter().enumerate() {
        let label = format!("copy_selector_payload_{index}");
        a.emit_all(load_address(Register::T0, source.start))
            .emit_all(load_address(Register::T1, *output))
            .emit_all(load_address(Register::T2, source.end))
            .label(&label)
            .emit(Lw {
                rt: Register::T3,
                base: Register::T0,
                offset: 0,
            })
            .emit(Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 4,
            })
            .emit(Sw {
                rt: Register::T3,
                base: Register::T1,
                offset: 0,
            })
            .bne(Register::T0, Register::T2, &label)
            .emit(Addiu {
                rt: Register::T1,
                rs: Register::T1,
                immediate: 4,
            });
    }
    a.emit_all(load_address(Register::T6, layout.enter_critical_address))
        .emit(Jalr {
            rd: Register::RA,
            rs: Register::T6,
        })
        .emit(Instruction::nop())
        .emit(Sw {
            rt: Register::V0,
            base: Register::SP,
            offset: 16,
        })
        .emit_all(load_address(Register::T6, layout.flush_cache_address))
        .emit(Jalr {
            rd: Register::RA,
            rs: Register::T6,
        })
        .emit(Instruction::nop())
        .emit(Lw {
            rt: Register::T0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::nop())
        .beq(Register::T0, Register::ZERO, "selector_payload_ready")
        .emit(Instruction::nop())
        .emit_all(load_address(Register::T6, layout.exit_critical_address))
        .emit(Jalr {
            rd: Register::RA,
            rs: Register::T6,
        })
        .emit(Instruction::nop())
        .label("selector_payload_ready")
        .emit(Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 20,
        })
        .emit(Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 24,
        })
        .emit_all(load_address(Register::T6, destination.start))
        .emit(Jr { rs: Register::T6 })
        .emit(Instruction::nop());
    let bytes = a.assemble(layout.origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= layout.byte_capacity,
        "selector bootstrap exceeds staging capacity"
    );
    let instructions = verify_placed_program(&bytes, layout.origin)?;
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &instructions,
        layout.origin,
        "selector bootstrap",
    )?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "selector_bootstrap_tests.rs"]
mod tests;
