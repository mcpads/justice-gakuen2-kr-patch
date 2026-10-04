use std::path::Path;

use anyhow::Result;

use super::address_flow_seed_exhaustion_audit;
use super::model::SharedReferenceSourceAudit;
use super::scanner::{ReferenceIndex, index_references_from_value_flow};
use crate::consumer_analysis::analyze_loaded_program;
use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    LoadedImage, MAIN_EXECUTABLE_PATH, MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE,
    loaded_main_executable, read_dat1_images,
};

pub(super) struct AuditInputs {
    pub(super) overlays: Vec<LoadedImage>,
    pub(super) shared_references: ReferenceIndex,
    pub(super) shared_reference_source: SharedReferenceSourceAudit,
}

pub(super) fn load_audit_inputs(
    image_path: &Path,
    address_flow_state_budget: usize,
) -> Result<AuditInputs> {
    let mut track = RawTrack::open(image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let overlays = read_dat1_images(&mut iso)?;
    let maximum_overlay_size = overlays
        .iter()
        .map(|overlay| overlay.data.len())
        .max()
        .expect("validated non-empty overlay set");

    let executable_record = iso.find(MAIN_EXECUTABLE_PATH)?;
    let executable = iso.read_record(&executable_record)?;
    let executable_sha256 = sha256_bytes(&executable);
    let runtime_base = MAIN_TEXT_RUNTIME_BASE;
    let executable_image = loaded_main_executable(&executable)?;
    let text = &executable_image.data;
    let executable_analysis = analyze_loaded_program(&executable_image, address_flow_state_budget)?
        .expect("validated main executable has a runtime base and entrypoint");
    let shared_references = index_references_from_value_flow(
        text,
        super::scanner::OVERLAY_BASE,
        maximum_overlay_size,
        PSX_EXE_HEADER_SIZE,
        &executable_analysis.value_flow,
    );
    let shared_reference_source = SharedReferenceSourceAudit {
        path: MAIN_EXECUTABLE_PATH.to_string(),
        file_size: executable.len(),
        sha256: executable_sha256,
        loaded_text_file_offset: format!("0x{PSX_EXE_HEADER_SIZE:04x}"),
        loaded_text_runtime_base: format!("0x{runtime_base:08x}"),
        loaded_text_size: text.len(),
        address_flow_seed_count: shared_references.address_flow_seed_count(),
        address_flow_instruction_state_count: shared_references
            .address_flow_instruction_state_count(),
        address_flow_budget_exhausted_seed_count: shared_references
            .address_flow_budget_exhausted_seed_count(),
        address_flow_budget_exhausted_seed_offsets: shared_references
            .address_flow_budget_exhausted_seed_offsets()
            .iter()
            .copied()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        address_flow_budget_exhausted_seeds: shared_references
            .address_flow_budget_exhausted_seeds()
            .iter()
            .map(address_flow_seed_exhaustion_audit)
            .collect(),
        overlay_window_pointer_reference_count: shared_references.pointer_reference_count(),
        overlay_window_address_materialization_reference_count: shared_references
            .address_materialization_reference_count(),
        overlay_window_memory_access_reference_count: shared_references
            .memory_access_reference_count(),
        overlay_window_loaded_word_reference_count: shared_references.loaded_word_reference_count(),
    };

    Ok(AuditInputs {
        overlays,
        shared_references,
        shared_reference_source,
    })
}
