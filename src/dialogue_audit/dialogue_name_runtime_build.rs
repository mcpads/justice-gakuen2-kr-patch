use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::verify_placed_program;
use serde::Serialize;

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::name_input::{
    NAME_DIALOGUE_RUNTIME_ORIGIN, NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY,
    NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN, NameDialogueRuntimeProgram,
    NameDialogueRuntimeProgramReport,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;

use super::script_source::{MGAME_PATH, MGAME_SHA256, MGAME_SIZE};

#[path = "dialogue_name_runtime_build/mgame_reload_entry_hook.rs"]
pub(super) mod mgame_reload_entry_hook;
#[cfg(test)]
#[path = "dialogue_name_runtime_build/mgame_reload_entry_hook_tests.rs"]
mod mgame_reload_entry_hook_tests;
#[path = "dialogue_name_runtime_build/name_consumer_hooks.rs"]
mod name_consumer_hooks;
#[cfg(test)]
#[path = "dialogue_name_runtime_build/name_consumer_hooks_tests.rs"]
mod name_consumer_hooks_tests;
#[path = "dialogue_name_runtime_build/nickname_hud_lookup.rs"]
mod nickname_hud_lookup;
#[path = "dialogue_name_runtime_build/nickname_hud_render_hook.rs"]
mod nickname_hud_render_hook;
#[cfg(test)]
#[path = "dialogue_name_runtime_build/nickname_hud_render_hook_tests.rs"]
mod nickname_hud_render_hook_tests;
#[path = "dialogue_name_runtime_build/relationship_name_field_capture.rs"]
mod relationship_name_field_capture;
#[cfg(test)]
#[path = "dialogue_name_runtime_build/relationship_name_field_capture_tests.rs"]
mod relationship_name_field_capture_tests;

use mgame_reload_entry_hook::{
    MgameReloadEntryHookReport, build_mgame_reload_entry_replacement,
    install_mgame_reload_entry_hook,
};
use name_consumer_hooks::{
    DialogueNameConsumerHookReport, dialogue_name_consumer_replacements,
    install_dialogue_name_consumer_hooks,
};
use nickname_hud_lookup::{NicknameHudLookupReport, verify_nickname_hud_lookup};
use nickname_hud_render_hook::{
    NicknameHudRenderHookReport, build_nickname_hud_render_replacement,
    install_nickname_hud_render_hook,
};
use relationship_name_field_capture::{
    RelationshipNameFieldCaptureReport, install_relationship_name_field_capture,
    relationship_name_field_capture_replacements,
};

#[path = "dialogue_name_runtime_build/backup_names.rs"]
mod backup_names;
#[path = "dialogue_name_runtime_build/profile_name_controls.rs"]
pub(super) mod profile_name_controls;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const MGAME_RUNTIME_TAIL_OFFSET: usize = 0x0002_8d94;
const MGAME_RUNTIME_TAIL_SOURCE_SHA256: &str =
    "270d7c32555268f578b5c79e665937b2bbe3eee44f88a39c5961c9ce3982a1eb";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DialogueNameRuntimeBuild {
    pub(super) bytes: Vec<u8>,
    pub(super) report: DialogueNameRuntimeBuildReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DialogueNameRuntimeBuildReport {
    pub source_path: String,
    pub source_size: usize,
    pub source_sha256: String,
    pub patched_sha256: String,
    pub runtime_bootstrap_owner_path: String,
    pub runtime_repair_owner_path: String,
    pub runtime_reload_source_owner_path: String,
    pub program_reload_byte_range: [usize; 2],
    pub program_reload_source_address_range: [String; 2],
    pub program_reload_source_sha256: String,
    pub program_reload_installed_byte_count: usize,
    pub program_reload_installed: bool,
    pub name_consumer_hooks: [DialogueNameConsumerHookReport; 2],
    pub relationship_name_field_capture: RelationshipNameFieldCaptureReport,
    pub mgame_reload_entry_hook: MgameReloadEntryHookReport,
    pub nickname_hud_render_hook: NicknameHudRenderHookReport,
    pub nickname_hud_lookup: NicknameHudLookupReport,
    pub profile_name_controls: profile_name_controls::ProfileNameControlsReport,
    pub backup_names: backup_names::BackupNamesReport,
    pub stat_result_layout: super::stat_result_layout::StatResultLayout,
    pub program: NameDialogueRuntimeProgramReport,
    pub runtime_execution_verified: bool,
}

pub(super) fn build_dialogue_name_runtime_hook(
    source: &[u8],
    program: &NameDialogueRuntimeProgram,
    runtime_bootstrap_owner_path: &str,
    backup_text: super::backup_slot_text::BackupSlotTextCounts,
    stat_result_layout: super::stat_result_layout::StatResultLayout,
) -> Result<DialogueNameRuntimeBuild> {
    ensure!(source.len() == MGAME_SIZE, "unexpected {MGAME_PATH} size");
    ensure!(
        sha256_bytes(source) == MGAME_SHA256,
        "unexpected {MGAME_PATH} SHA-256"
    );
    ensure!(
        program.report.installed
            && program.report.origin == hex_address(NAME_DIALOGUE_RUNTIME_ORIGIN),
        "dialogue-name hook requires an installed persistent runtime"
    );
    let instruction_origin = NAME_DIALOGUE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(program.instruction_offset)?)
        .context("persistent dialogue-name instruction address overflow")?;
    let typed = verify_placed_program(
        &program.bytes[program.instruction_offset..],
        instruction_origin,
    )?;
    ensure!(
        typed.len() == program.report.typed_instruction_count,
        "persistent dialogue-name typed readback changed"
    );
    ensure!(
        program.mgame_runtime_repair.report.installed
            && program
                .mgame_runtime_repair
                .report
                .first_entry_copy_roundtrip_verified
            && program
                .mgame_runtime_repair
                .report
                .destination_repair_roundtrip_verified,
        "dialogue-name hook requires the persistent MGAME runtime repair program"
    );
    let program_offset = runtime_offset(NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN)?;
    ensure!(
        program_offset == MGAME_RUNTIME_TAIL_OFFSET,
        "dialogue-name runtime no longer begins at the MGAME reload tail"
    );
    let program_region_end = program_offset
        .checked_add(NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY)
        .context("dialogue-name MGAME reload region overflow")?;
    let program_source_region = source
        .get(program_offset..program_region_end)
        .context("dialogue-name MGAME reload region is truncated")?;
    ensure!(
        program_source_region.iter().all(|byte| *byte == 0)
            && sha256_bytes(program_source_region) == MGAME_RUNTIME_TAIL_SOURCE_SHA256,
        "dialogue-name MGAME reload source region changed"
    );
    ensure!(
        program.bytes.len() <= NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY,
        "dialogue-name runtime exceeds the MGAME reload tail"
    );
    let nickname_hud_lookup = verify_nickname_hud_lookup(source, &program.report)?;

    let mut consumer_hooks_candidate = source.to_vec();
    let name_consumer_hooks = install_dialogue_name_consumer_hooks(
        source,
        &mut consumer_hooks_candidate,
        program.name_consumer_address,
    )?;
    let name_consumer_replacements =
        dialogue_name_consumer_replacements(program.name_consumer_address);

    let mut relationship_field_capture_candidate = source.to_vec();
    let relationship_name_field_capture =
        install_relationship_name_field_capture(source, &mut relationship_field_capture_candidate)?;
    let relationship_field_capture_replacements = relationship_name_field_capture_replacements();

    let mut reload_entry_candidate = source.to_vec();
    let mgame_reload_entry_hook = install_mgame_reload_entry_hook(
        source,
        &mut reload_entry_candidate,
        program.mgame_reload_wrapper_address,
    )?;
    let (reload_entry_address, reload_entry_instructions) =
        build_mgame_reload_entry_replacement(program.mgame_reload_wrapper_address)?;

    let mut render_hook_candidate = source.to_vec();
    let nickname_hud_render_hook = install_nickname_hud_render_hook(
        source,
        &mut render_hook_candidate,
        program.nickname_hud_render_wrapper_address,
    )?;
    let (render_hook_address, render_hook_instructions) =
        build_nickname_hud_render_replacement(program.nickname_hud_render_wrapper_address)?;

    let mut machine_code_sources = PsxMachineCodeSources::default();
    let mut write_plan = DecodedRecordWritePlan::new(MGAME_PATH, source, MGAME_SHA256)?;
    super::stat_result_layout::register(source, stat_result_layout, &mut write_plan)?;
    let mut program_candidate = source.to_vec();
    let program_end = program_offset + program.bytes.len();
    program_candidate[program_offset..program_end].copy_from_slice(&program.bytes);
    let program_instruction_offset = program_offset + program.instruction_offset;
    let program_provenance = machine_code_sources.register(
        "mgame:shared-name:reload-source-runtime",
        instruction_origin,
        program.instructions.clone(),
    )?;
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "MGAME reload-source shared-name runtime",
        source_sha256: MGAME_SHA256,
        candidate: &program_candidate,
        claims: vec![
            CandidateWriteClaim {
                id: "mgame:shared-name:reload-source-runtime-data".to_string(),
                purpose: "stage the reloadable dialogue-name runtime tables".to_string(),
                range: program_offset..program_instruction_offset,
                intent: WriteIntent::Data,
            },
            CandidateWriteClaim {
                id: "mgame:shared-name:reload-source-runtime-code".to_string(),
                purpose: "stage the repairable dialogue-name runtime code".to_string(),
                range: program_instruction_offset..program_end,
                intent: WriteIntent::MachineCode(program_provenance),
            },
        ],
    })?;
    let mut consumer_hook_claims = Vec::with_capacity(name_consumer_hooks.len());
    for ((address, replacement), report) in name_consumer_replacements
        .into_iter()
        .zip(&name_consumer_hooks)
    {
        let id = format!("mgame:shared-name:consumer-hook:{address:08x}");
        let provenance = machine_code_sources.register(id.clone(), address, vec![replacement])?;
        consumer_hook_claims.push(CandidateWriteClaim {
            id,
            purpose: "redirect a dialogue name consumer call to the tagged-name runtime"
                .to_string(),
            range: report.byte_range[0]..report.byte_range[1],
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "shared-name dialogue consumer hooks",
        source_sha256: MGAME_SHA256,
        candidate: &consumer_hooks_candidate,
        claims: consumer_hook_claims,
    })?;
    let mut relationship_field_capture_claims =
        Vec::with_capacity(relationship_field_capture_replacements.len());
    for ((address, replacement, _), site) in relationship_field_capture_replacements
        .into_iter()
        .zip(&relationship_name_field_capture.sites)
    {
        let id = format!("mgame:relationship-name:field-capture:{address:08x}");
        let provenance = machine_code_sources.register(id.clone(), address, vec![replacement])?;
        relationship_field_capture_claims.push(CandidateWriteClaim {
            id,
            purpose:
                "preserve the relationship-selected name field for tagged-name materialization"
                    .to_string(),
            range: site.byte_range[0]..site.byte_range[1],
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "relationship-name field capture",
        source_sha256: MGAME_SHA256,
        candidate: &relationship_field_capture_candidate,
        claims: relationship_field_capture_claims,
    })?;
    let reload_entry_provenance = machine_code_sources.register(
        "mgame:shared-name:reload-entry-hook",
        reload_entry_address,
        reload_entry_instructions,
    )?;
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "MGAME reload entry hook",
        source_sha256: MGAME_SHA256,
        candidate: &reload_entry_candidate,
        claims: vec![CandidateWriteClaim {
            id: "mgame:shared-name:reload-entry-hook".to_string(),
            purpose: "copy the valid staged source or repair the resident dialogue runtime before MGAME entry"
                .to_string(),
            range: mgame_reload_entry_hook.byte_range[0]..mgame_reload_entry_hook.byte_range[1],
            intent: WriteIntent::MachineCode(reload_entry_provenance),
        }],
    })?;
    let render_provenance = machine_code_sources.register(
        "mgame:nickname-hud:render-hook",
        render_hook_address,
        render_hook_instructions,
    )?;
    write_plan.register_candidate(CandidateRecordWrite {
        owner: "nickname HUD render hook",
        source_sha256: MGAME_SHA256,
        candidate: &render_hook_candidate,
        claims: vec![CandidateWriteClaim {
            id: "mgame:nickname-hud:render-hook".to_string(),
            purpose: "provide a lazy nickname materialization fallback at the HUD renderer"
                .to_string(),
            range: nickname_hud_render_hook.byte_range[0]..nickname_hud_render_hook.byte_range[1],
            intent: WriteIntent::MachineCode(render_provenance),
        }],
    })?;
    let profile_name_controls = profile_name_controls::register_profile_name_controls(
        source,
        MGAME_SHA256,
        &mut write_plan,
        &mut machine_code_sources,
    )?;
    let backup_names = backup_names::register(
        source,
        MGAME_SHA256,
        program.name_consumer_address,
        backup_text,
        &mut write_plan,
        &mut machine_code_sources,
    )?;
    crate::diary_header::action_spacing::register(
        source,
        MGAME_SHA256,
        &mut write_plan,
        &mut machine_code_sources,
    )?;
    let patched = write_plan.apply(Some(&machine_code_sources))?;

    Ok(DialogueNameRuntimeBuild {
        report: DialogueNameRuntimeBuildReport {
            source_path: MGAME_PATH.to_string(),
            source_size: source.len(),
            source_sha256: MGAME_SHA256.to_string(),
            patched_sha256: sha256_bytes(&patched),
            runtime_bootstrap_owner_path: runtime_bootstrap_owner_path.to_string(),
            runtime_repair_owner_path: runtime_bootstrap_owner_path.to_string(),
            runtime_reload_source_owner_path: MGAME_PATH.to_string(),
            program_reload_byte_range: [program_offset, program_region_end],
            program_reload_source_address_range: [
                hex_address(NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN),
                hex_address(
                    NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN
                        + u32::try_from(NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_BYTE_CAPACITY)?,
                ),
            ],
            program_reload_source_sha256: MGAME_RUNTIME_TAIL_SOURCE_SHA256.to_string(),
            program_reload_installed_byte_count: program.bytes.len(),
            program_reload_installed: true,
            name_consumer_hooks,
            relationship_name_field_capture,
            mgame_reload_entry_hook,
            nickname_hud_render_hook,
            nickname_hud_lookup,
            profile_name_controls,
            backup_names,
            stat_result_layout,
            program: program.report.clone(),
            runtime_execution_verified: false,
        },
        bytes: patched,
    })
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}

fn runtime_offset(address: u32) -> Result<usize> {
    usize::try_from(
        address
            .checked_sub(MGAME_RUNTIME_BASE)
            .context("MGAME address precedes its runtime base")?,
    )
    .map_err(Into::into)
}
