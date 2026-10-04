//! Offline candidate census. This never selects product writes or runs in a build.
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::psx_static_analysis::{
    memory_access::decoded_memory_access, value_flow::DerivedAddressKind,
};
use crate::source_disc::{load_dat1_images, load_main_executable_image};
use anyhow::{Context, Result, ensure};
use psx_r3000a::decode;
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct MemoryReferenceAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
    pub ranges: Vec<[u32; 2]>,
    pub state_budget: usize,
}

fn physical(address: u32) -> u32 {
    address & 0x1fff_ffff
}
fn overlaps(ranges: &[[u32; 2]], address: u32, bytes: usize) -> bool {
    let a = u64::from(physical(address));
    ranges
        .iter()
        .any(|r| a < u64::from(physical(r[1])) && u64::from(physical(r[0])) < a + bytes as u64)
}

pub fn audit_memory_references(config: &MemoryReferenceAuditConfig) -> Result<Value> {
    ensure!(
        !config.ranges.is_empty()
            && config
                .ranges
                .iter()
                .all(|r| physical(r[0]) < physical(r[1])),
        "invalid memory ranges"
    );
    ensure!(config.state_budget > 0, "empty value-flow budget");
    let cue = CueSheet::parse(&config.cue)?;
    let source_sha = sha256_file(&cue.image_path)?;
    ensure!(source_sha == BASELINE_BIN_SHA256, "unsupported source disc");
    let mut sources = load_dat1_images(&cue.image_path)?;
    sources.push(load_main_executable_image(&cue.image_path)?);
    let mut images = Vec::new();
    for source in sources {
        eprintln!("memory census: {}", source.path);
        let Some(analysis) = super::analyze_loaded_program(&source, config.state_budget)? else {
            images.push(json!({"path":source.path,"status":"no_executable_profile"}));
            continue;
        };
        let base = source.runtime_base.context("missing source base")?;
        // Also enumerate dormant code candidates: indirect task dispatch can hide
        // real writers from the declared entrypoint closure. Keep this distinct
        // from reachable proof and retain budget exhaustion.
        let candidate_domain =
            crate::psx_static_analysis::ExecutableDomain::full_image(source.data.len());
        let candidate_flow =
            crate::psx_static_analysis::value_flow::scan_derived_address_flow_with_budget(
                &source.data,
                base,
                &candidate_domain,
                config.state_budget,
            );
        let flow = &candidate_flow;
        let mut accesses = Vec::new();
        for reference in &flow.addresses {
            if reference.kind != DerivedAddressKind::MemoryAccess {
                continue;
            }
            let o = reference.instruction_offset;
            let instruction = decode(
                u32::from_le_bytes(source.data[o..o + 4].try_into()?),
                base + o as u32,
            )?;
            let Some(access) = decoded_memory_access(&instruction) else {
                continue;
            };
            if overlaps(&config.ranges, reference.address, access.width_bytes) {
                accesses.push(json!({"pc":format!("0x{:08x}",base+o as u32),"address":format!("0x{:08x}",reference.address),"bytes":access.width_bytes,"reachable":analysis.closure.scan.instruction_offsets.contains(&o),"operation":access.operation.as_str(),"instruction":format!("{instruction:?}")}));
            }
        }
        accesses.sort_by_key(Value::to_string);
        accesses.dedup();
        let calls = flow.resolved_direct_call_arguments.iter().filter(|a| overlaps(&config.ranges,a.value,1)).map(|a| json!({"pc":format!("0x{:08x}",base+a.instruction_offset as u32),"target":format!("0x{:08x}",a.target),"argument":format!("{:?}",a.argument_register),"value":format!("0x{:08x}",a.value)})).collect::<Vec<_>>();
        images.push(json!({"path":source.path,"sha256":sha256_bytes(&source.data),"runtime_base":format!("0x{base:08x}"),"entrypoints":source.entrypoints.iter().map(|e|format!("0x{:08x}",e.runtime_address)).collect::<Vec<_>>(),"reachable_instruction_count":analysis.closure.scan.instruction_offsets.len(),"unresolved_indirect_transfers":analysis.closure.scan.unresolved_indirect_transfer_offsets,"outside_image_targets":analysis.closure.scan.outside_image_transfer_targets,"decode_failures":analysis.closure.scan.decode_failure_offsets,"flow_seed_count":flow.seed_count,"flow_state_count":flow.instruction_state_count,"budget_exhausted_seeds":flow.budget_exhausted_seed_offsets,"memory_accesses":accesses,"pointer_call_arguments":calls}));
    }
    let report = json!({"kind":"runtime memory reference candidates","source_bin_sha256":source_sha,"ranges":config.ranges,"state_budget":config.state_budget,"completeness":"full-image candidates are not code/reachability proof; unknown pointers, gp-only addressing and unresolved bulk ranges require separate review","images":images});
    if let Some(parent) = config.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config.output, serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_access_and_cached_alias_share_the_same_ram_boundary() {
        let r = [[0x8009_a400, 0x8009_ad00]];
        assert!(overlaps(&r, 0xa009_a3fe, 4));
        assert!(overlaps(&r, 0x0009_acff, 1));
        assert!(!overlaps(&r, 0x8009_a3fc, 4));
        assert!(!overlaps(&r, 0x8009_ad00, 4));
    }
}
