use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, decode};

use crate::consumer_analysis::analyze_loaded_program;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::psx_static_analysis::memory_access::decoded_memory_access;
use crate::psx_static_analysis::value_flow::DerivedAddressKind;
use crate::source_disc::{LoadedImage, load_dat1_images, load_main_executable_image};

use super::name_companion_reference_model::{
    NameCompanionReferenceAuditConfig, NameCompanionReferenceAuditReport,
    NameCompanionStaticReference,
};

const NICKNAME_COMPANION_START: u32 = 0x801f_1886;
const NICKNAME_COMPANION_END: u32 = 0x801f_1896;

#[derive(Debug)]
pub(super) struct LoadedSourceReferenceScan {
    pub(super) seed_count: usize,
    pub(super) instruction_state_count: usize,
    pub(super) budget_exhausted_seed_count: usize,
    pub(super) candidates: Vec<NameCompanionStaticReference>,
}

pub fn audit_name_companion_references(
    config: &NameCompanionReferenceAuditConfig,
) -> Result<NameCompanionReferenceAuditReport> {
    ensure!(
        config.address_flow_state_budget > 0,
        "address-flow state budget must be positive"
    );
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let mut sources = load_dat1_images(&cue.image_path)?;
    sources.push(load_main_executable_image(&cue.image_path)?);
    sources.sort_by(|left, right| left.path.cmp(&right.path));

    let mut skipped_loaded_image_paths = Vec::new();
    let mut address_flow_seed_count = 0usize;
    let mut address_flow_instruction_state_count = 0usize;
    let mut address_flow_budget_exhausted_seed_count = 0usize;
    let mut candidates = Vec::new();
    for source in &sources {
        if source.runtime_base.is_none() {
            skipped_loaded_image_paths.push(source.path.clone());
            continue;
        }
        let scan = scan_loaded_source(source, config.address_flow_state_budget)?;
        address_flow_seed_count += scan.seed_count;
        address_flow_instruction_state_count += scan.instruction_state_count;
        address_flow_budget_exhausted_seed_count += scan.budget_exhausted_seed_count;
        candidates.extend(scan.candidates);
    }

    let static_read_site_count = candidates
        .iter()
        .filter(|candidate| candidate.operation == "read")
        .count();
    let static_write_site_count = candidates
        .iter()
        .filter(|candidate| candidate.operation == "write")
        .count();
    let report = NameCompanionReferenceAuditReport {
        kind: "Justice Gakuen 2 nickname companion static reference audit".to_string(),
        source_bin_sha256,
        target_runtime_byte_range: [
            format!("0x{NICKNAME_COMPANION_START:08x}"),
            format!("0x{NICKNAME_COMPANION_END:08x}"),
        ],
        scanned_loaded_image_count: sources.len() - skipped_loaded_image_paths.len(),
        skipped_loaded_image_paths,
        address_flow_state_budget: config.address_flow_state_budget,
        address_flow_seed_count,
        address_flow_instruction_state_count,
        address_flow_budget_exhausted_seed_count,
        all_address_flows_completed_within_budget: address_flow_budget_exhausted_seed_count == 0,
        static_memory_access_site_count: candidates.len(),
        static_read_site_count,
        static_write_site_count,
        runtime_confirmation_required: true,
        candidates,
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

pub(super) fn scan_loaded_source(
    source: &LoadedImage,
    address_flow_state_budget: usize,
) -> Result<LoadedSourceReferenceScan> {
    let runtime_base = source
        .runtime_base
        .context("static reference source lacks a runtime base")?;
    let analysis = analyze_loaded_program(source, address_flow_state_budget)?
        .with_context(|| format!("{} lacks a valid executable entrypoint", source.path))?;
    let flow = analysis.value_flow;
    let loaded_bytes_sha256 = sha256_bytes(&source.data);
    let mut grouped =
        BTreeMap::<(usize, &'static str, usize), (BTreeSet<usize>, BTreeSet<u32>)>::new();

    for reference in &flow.addresses {
        if reference.kind != DerivedAddressKind::MemoryAccess
            || !(NICKNAME_COMPANION_START..NICKNAME_COMPANION_END).contains(&reference.address)
        {
            continue;
        }
        let instruction =
            decode_instruction(&source.data, runtime_base, reference.instruction_offset)?;
        let Some(access) = decoded_memory_access(&instruction) else {
            continue;
        };
        let operation = access.operation.as_str();
        let width_bytes = access.width_bytes;
        let (seed_offsets, access_addresses) = grouped
            .entry((reference.instruction_offset, operation, width_bytes))
            .or_default();
        seed_offsets.insert(reference.seed_offset);
        access_addresses.insert(reference.address);
    }

    let candidates = grouped
        .into_iter()
        .map(
            |((instruction_offset, operation, width_bytes), (seed_offsets, access_addresses))| {
                NameCompanionStaticReference {
                    source_path: source.path.clone(),
                    loaded_bytes_sha256: loaded_bytes_sha256.clone(),
                    loaded_runtime_base: format!("0x{runtime_base:08x}"),
                    seed_loaded_offsets: seed_offsets
                        .into_iter()
                        .map(|offset| format!("0x{offset:04x}"))
                        .collect(),
                    instruction_loaded_offset: format!("0x{instruction_offset:04x}"),
                    instruction_runtime_address: format!(
                        "0x{:08x}",
                        runtime_base.wrapping_add(instruction_offset as u32)
                    ),
                    access_runtime_addresses: access_addresses
                        .into_iter()
                        .map(|address| format!("0x{address:08x}"))
                        .collect(),
                    operation: operation.to_string(),
                    width_bytes,
                }
            },
        )
        .collect();

    Ok(LoadedSourceReferenceScan {
        seed_count: flow.seed_count,
        instruction_state_count: flow.instruction_state_count,
        budget_exhausted_seed_count: flow.budget_exhausted_seed_count,
        candidates,
    })
}

fn decode_instruction(data: &[u8], runtime_base: u32, offset: usize) -> Result<Instruction> {
    let end = offset.checked_add(4).context("instruction end overflow")?;
    let bytes = data
        .get(offset..end)
        .context("static reference instruction is truncated")?;
    let pc = runtime_base.wrapping_add(offset as u32);
    decode(u32::from_le_bytes(bytes.try_into()?), pc)
        .with_context(|| format!("failed to decode static reference instruction at {pc:#010x}"))
}

fn write_report(path: &Path, report: &NameCompanionReferenceAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
