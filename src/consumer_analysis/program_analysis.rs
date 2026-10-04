use anyhow::{Context, Result, ensure};

use super::control_transfer_profiles::declared_control_transfers;
use super::model::StaticDeclaredIndirectJumpAudit;
use crate::psx_static_analysis::ExecutableDomain;
use crate::psx_static_analysis::reachable_control_flow::{
    ReachableInstructionClosure,
    close_reachable_control_transfers_from_entrypoints_with_declared_targets,
};
use crate::psx_static_analysis::value_flow::{
    DerivedAddressScan, scan_derived_address_flow_with_budget,
};
use crate::source_disc::LoadedImage;

const MAX_REACHABILITY_REFINEMENT_PASSES: usize = 16;

pub(crate) struct LoadedProgramAnalysis {
    pub(crate) executable_domain: ExecutableDomain,
    pub(crate) closure: ReachableInstructionClosure,
    pub(crate) value_flow: DerivedAddressScan,
    pub(crate) refinement_pass_count: usize,
    pub(crate) declared_indirect_jumps: Vec<StaticDeclaredIndirectJumpAudit>,
}

pub(crate) fn analyze_loaded_program(
    image: &LoadedImage,
    value_flow_state_budget: usize,
) -> Result<Option<LoadedProgramAnalysis>> {
    let Some(instruction_base) = image.runtime_base else {
        return Ok(None);
    };
    let candidate_domain = ExecutableDomain::full_image(image.data.len());
    let entrypoint_addresses = image
        .entrypoints
        .iter()
        .map(|entrypoint| entrypoint.runtime_address)
        .collect::<Vec<_>>();
    if image.entrypoints.is_empty()
        || entrypoint_addresses.iter().any(|&entrypoint| {
            candidate_domain
                .instruction_offset(instruction_base, entrypoint)
                .is_none()
        })
    {
        return Ok(None);
    }
    let declared_control_transfers = declared_control_transfers(image)?;

    let mut closure = close_reachable_control_transfers_from_entrypoints_with_declared_targets(
        &image.data,
        instruction_base,
        &candidate_domain,
        &entrypoint_addresses,
        &[],
        &declared_control_transfers.targets,
    );
    let mut refinement_pass_count = 0usize;
    let value_flow = loop {
        let executable_domain = ExecutableDomain::from_instruction_offsets(
            image.data.len(),
            closure.scan.instruction_offsets.iter().copied(),
        )
        .context("reachable instructions did not form an executable domain")?;
        let value_flow = scan_derived_address_flow_with_budget(
            &image.data,
            instruction_base,
            &executable_domain,
            value_flow_state_budget,
        );
        let next = close_reachable_control_transfers_from_entrypoints_with_declared_targets(
            &image.data,
            instruction_base,
            &candidate_domain,
            &entrypoint_addresses,
            &value_flow.resolved_register_transfers,
            &declared_control_transfers.targets,
        );
        refinement_pass_count += 1;
        if next.scan.instruction_offsets == closure.scan.instruction_offsets {
            closure = next;
            break value_flow;
        }
        ensure!(
            refinement_pass_count < MAX_REACHABILITY_REFINEMENT_PASSES,
            "{} reachability did not converge within {} refinement passes",
            image.path,
            MAX_REACHABILITY_REFINEMENT_PASSES
        );
        closure = next;
    };
    let executable_domain = ExecutableDomain::from_instruction_offsets(
        image.data.len(),
        closure.scan.instruction_offsets.iter().copied(),
    )
    .context("final reachable instructions did not form an executable domain")?;
    Ok(Some(LoadedProgramAnalysis {
        executable_domain,
        closure,
        value_flow,
        refinement_pass_count,
        declared_indirect_jumps: declared_control_transfers.audits,
    }))
}
