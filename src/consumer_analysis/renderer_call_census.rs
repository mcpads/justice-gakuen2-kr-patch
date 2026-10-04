use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::model::{StaticRendererCallCensusAudit, StaticRendererCallSiteAudit};
use crate::source_disc::LoadedImage;

const KANRI_PATH: &str = "DAT1/KANRI.BIN";
const KANRI_RUNTIME_BASE: u32 = 0x800a_2000;
const KANRI_RENDERER_ADDRESS: u32 = 0x800a_3b3c;

#[derive(Clone, Copy)]
struct RendererCallSiteProfile {
    instruction_offset: usize,
    argument_source_class: &'static str,
    source_domain_status: &'static str,
    evidence: &'static str,
}

const fn call(
    instruction_offset: usize,
    argument_source_class: &'static str,
    source_domain_status: &'static str,
    evidence: &'static str,
) -> RendererCallSiteProfile {
    RendererCallSiteProfile {
        instruction_offset,
        argument_source_class,
        source_domain_status,
        evidence,
    }
}

const DIRECT_RECORD: (&str, &str, &str) = (
    "direct_overlay_record",
    "finite_source_domain",
    "the call loads A1 from one exact source-overlay placement record",
);
const BRANCH_RECORD: (&str, &str, &str) = (
    "branch_selected_overlay_record",
    "finite_source_domain",
    "the call selects A1 from an explicit finite set of source-overlay placement records",
);
const BOUNDED_RECORD_SEQUENCE: (&str, &str, &str) = (
    "bounded_overlay_record_sequence",
    "finite_source_domain",
    "the call advances through an explicitly validated finite source-overlay placement sequence",
);
const STACK_RECORD: (&str, &str, &str) = (
    "stack_assembled_overlay_text_record",
    "finite_source_domain",
    "the call receives a stack record whose text pointer is selected from an explicit source-overlay table",
);
const RUNTIME_INDEXED_RECORD_TABLE: (&str, &str, &str) = (
    "runtime_indexed_overlay_record_table",
    "dynamic_index_domain",
    "the source-overlay record-table base and stride are fixed, but this census has not closed the runtime index domain",
);

const fn direct(instruction_offset: usize) -> RendererCallSiteProfile {
    call(
        instruction_offset,
        DIRECT_RECORD.0,
        DIRECT_RECORD.1,
        DIRECT_RECORD.2,
    )
}

const fn branch(instruction_offset: usize) -> RendererCallSiteProfile {
    call(
        instruction_offset,
        BRANCH_RECORD.0,
        BRANCH_RECORD.1,
        BRANCH_RECORD.2,
    )
}

const fn bounded(instruction_offset: usize) -> RendererCallSiteProfile {
    call(
        instruction_offset,
        BOUNDED_RECORD_SEQUENCE.0,
        BOUNDED_RECORD_SEQUENCE.1,
        BOUNDED_RECORD_SEQUENCE.2,
    )
}

const fn stack(instruction_offset: usize) -> RendererCallSiteProfile {
    call(
        instruction_offset,
        STACK_RECORD.0,
        STACK_RECORD.1,
        STACK_RECORD.2,
    )
}

const fn runtime_indexed(instruction_offset: usize) -> RendererCallSiteProfile {
    call(
        instruction_offset,
        RUNTIME_INDEXED_RECORD_TABLE.0,
        RUNTIME_INDEXED_RECORD_TABLE.1,
        RUNTIME_INDEXED_RECORD_TABLE.2,
    )
}

// This is a small source-bound set, so the call sites stay literal. Do not infer
// membership from neighboring offsets or filenames. The audit independently
// scans every aligned word in the exact loaded image and requires set equality.
const KANRI_RENDERER_CALL_SITES: [RendererCallSiteProfile; 33] = [
    bounded(0x24b4),
    stack(0x2ab8),
    branch(0x2b48),
    bounded(0x2bd4),
    direct(0x3b48),
    direct(0x3b5c),
    direct(0x3cec),
    direct(0x3d18),
    direct(0x3e6c),
    direct(0x3e80),
    direct(0x3ea8),
    direct(0x3ebc),
    direct(0x3ee0),
    direct(0x3ef4),
    direct(0x3f0c),
    direct(0x4d6c),
    bounded(0x59d8),
    stack(0x5a28),
    bounded(0x5a44),
    bounded(0x5aac),
    bounded(0x5be4),
    bounded(0x7414),
    direct(0x74b4),
    direct(0x7694),
    direct(0x76ac),
    bounded(0x76e8),
    runtime_indexed(0x78d8),
    bounded(0x7bd8),
    bounded(0x7ce8),
    bounded(0x7da8),
    direct(0x7fa0),
    direct(0x7fec),
    direct(0x80b4),
];

pub(super) fn audit_declared_renderer_calls(
    image: &LoadedImage,
    reachable_instruction_offsets: Option<&BTreeSet<usize>>,
) -> Result<Vec<StaticRendererCallCensusAudit>> {
    if image.path != KANRI_PATH {
        return Ok(Vec::new());
    }
    ensure!(
        image.runtime_base == Some(KANRI_RUNTIME_BASE),
        "KANRI renderer-call census moved from its source-bound runtime base"
    );
    validate_profiles()?;
    validate_sixteen_record_loop(image)?;

    let observed_offsets =
        direct_call_offsets(&image.data, KANRI_RUNTIME_BASE, KANRI_RENDERER_ADDRESS);
    let declared_offsets = KANRI_RENDERER_CALL_SITES
        .iter()
        .map(|profile| profile.instruction_offset)
        .collect::<BTreeSet<_>>();
    ensure!(
        observed_offsets == declared_offsets,
        "KANRI renderer direct-call census changed; missing declared calls: {}; unclassified calls: {}",
        display_offsets(declared_offsets.difference(&observed_offsets)),
        display_offsets(observed_offsets.difference(&declared_offsets)),
    );

    let call_sites = KANRI_RENDERER_CALL_SITES
        .iter()
        .map(|profile| StaticRendererCallSiteAudit {
            instruction_offset: hex_offset(profile.instruction_offset),
            instruction_runtime_address: hex_address(
                KANRI_RUNTIME_BASE + profile.instruction_offset as u32,
            ),
            entrypoint_reachable: reachable_instruction_offsets
                .is_some_and(|reachable| reachable.contains(&profile.instruction_offset)),
            argument_source_class: profile.argument_source_class.to_string(),
            source_domain_status: profile.source_domain_status.to_string(),
            evidence: profile.evidence.to_string(),
            dynamic_source_boundary: None,
        })
        .collect::<Vec<_>>();
    let finite_source_domain_call_site_count = call_sites
        .iter()
        .filter(|site| site.source_domain_status == "finite_source_domain")
        .count();
    let dynamic_source_domain_call_site_count =
        call_sites.len() - finite_source_domain_call_site_count;

    Ok(vec![StaticRendererCallCensusAudit {
        id: "kanri_menu_text_renderer_direct_calls".to_string(),
        renderer_runtime_address: hex_address(KANRI_RENDERER_ADDRESS),
        direct_call_site_count: call_sites.len(),
        entrypoint_reachable_call_site_count: call_sites
            .iter()
            .filter(|site| site.entrypoint_reachable)
            .count(),
        classified_call_site_count: call_sites.len(),
        finite_source_domain_call_site_count,
        dynamic_source_domain_call_site_count,
        call_site_classification_complete: true,
        source_domain_complete: dynamic_source_domain_call_site_count == 0,
        call_sites,
        limitations: vec![
            "Call-site set equality classifies every aligned direct JAL to this renderer; it does not discover indirect calls through registers or prove that every reachable call executes on a captured route.".to_string(),
            "A runtime-indexed call is complete only after a separate cross-image audit has established its machine bounds and the point where semantic values enter from native save or runtime state; neighboring static records are not used to infer that domain.".to_string(),
        ],
    }])
}

fn validate_sixteen_record_loop(image: &LoadedImage) -> Result<()> {
    let instructions = [
        (
            0x2b98,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x2bb0,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x2bc4,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x800a,
            },
        ),
        (
            0x2bc8,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x2a9c,
            },
        ),
        (
            0x2bcc,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::S2,
                rt: Register::A1,
            },
        ),
        (
            0x2bd4,
            Instruction::Jal {
                target: KANRI_RENDERER_ADDRESS,
            },
        ),
        (
            0x2c4c,
            Instruction::Addiu {
                rt: Register::S2,
                rs: Register::S2,
                immediate: 12,
            },
        ),
        (
            0x2c50,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 1,
            },
        ),
        (
            0x2c54,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 16,
            },
        ),
        (
            0x2c58,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: KANRI_RUNTIME_BASE + 0x2bc0,
            },
        ),
    ];
    for (offset, expected) in instructions {
        let bytes: [u8; 4] = image
            .data
            .get(offset..offset + 4)
            .with_context(|| format!("KANRI instruction +0x{offset:04x} is truncated"))?
            .try_into()?;
        let actual = decode(
            u32::from_le_bytes(bytes),
            KANRI_RUNTIME_BASE + offset as u32,
        )
        .with_context(|| format!("failed to decode KANRI instruction +0x{offset:04x}"))?;
        ensure!(
            actual == expected,
            "KANRI sixteen-record renderer loop grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn validate_profiles() -> Result<()> {
    let offsets = KANRI_RENDERER_CALL_SITES
        .iter()
        .map(|profile| profile.instruction_offset)
        .collect::<BTreeSet<_>>();
    ensure!(
        offsets.len() == KANRI_RENDERER_CALL_SITES.len(),
        "KANRI renderer-call profile repeats an instruction offset"
    );
    ensure!(
        KANRI_RENDERER_CALL_SITES.iter().all(|profile| {
            matches!(
                profile.source_domain_status,
                "finite_source_domain" | "dynamic_index_domain"
            )
        }),
        "KANRI renderer-call profile contains an unsupported source-domain status"
    );
    Ok(())
}

fn direct_call_offsets(data: &[u8], runtime_base: u32, target: u32) -> BTreeSet<usize> {
    data.as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(word_index, bytes)| {
            let word = u32::from_le_bytes(*bytes);
            let instruction_offset = word_index * 4;
            let pc = runtime_base.wrapping_add(instruction_offset as u32);
            (direct_jump_target(word, pc) == Some(target)).then_some(instruction_offset)
        })
        .collect()
}

fn direct_jump_target(word: u32, pc: u32) -> Option<u32> {
    (word >> 26 == 0x03).then_some((pc.wrapping_add(4) & 0xf000_0000) | ((word & 0x03ff_ffff) << 2))
}

fn display_offsets<'a>(offsets: impl Iterator<Item = &'a usize>) -> String {
    offsets
        .map(|offset| hex_offset(*offset))
        .collect::<Vec<_>>()
        .join(", ")
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:08x}")
}

#[cfg(test)]
mod tests {
    use super::{direct_call_offsets, direct_jump_target};

    #[test]
    fn scans_only_aligned_direct_calls_to_the_requested_target() {
        let runtime_base = 0x800a_2000u32;
        let target = 0x800a_3b3cu32;
        let call_word = 0x0c00_0000 | ((target >> 2) & 0x03ff_ffff);
        let other_target = 0x800a_4000u32;
        let other_call_word = 0x0c00_0000 | ((other_target >> 2) & 0x03ff_ffff);
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&call_word.to_le_bytes());
        data.extend_from_slice(&other_call_word.to_le_bytes());

        assert_eq!(
            direct_jump_target(call_word, runtime_base + 4),
            Some(target)
        );
        assert_eq!(
            direct_call_offsets(&data, runtime_base, target),
            [4].into_iter().collect()
        );
    }
}
