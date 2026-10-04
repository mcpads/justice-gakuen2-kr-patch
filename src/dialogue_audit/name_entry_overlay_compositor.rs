use anyhow::Result;

use crate::name_input::{
    NameGlyphConsumerLayout, NameInputKeyboardPlan, NameInputRedisplayRuntimeProgram,
};
use crate::pipeline::difference_ranges;

use super::name_entry::{OVERLAY_PATH, OVERLAY_RUNTIME_BASE};
use super::name_entry_confirmation_hook::{
    HOOK_OFFSET as CONFIRMATION_HOOK_OFFSET, NameEntryConfirmationHookReport,
    build_name_entry_confirmation_replacement, install_name_entry_confirmation_hook,
};
use super::name_entry_delete_handler::{
    DELETE_ROUTINE_BYTE_COUNT, DELETE_ROUTINE_OFFSET, NameEntryDeleteHandlerReport,
    build_name_entry_delete_handler_program, confirmation_entry_address,
    install_name_entry_delete_handler, routine_runtime_address as delete_routine_runtime_address,
};
use super::name_entry_keyboard_build::{
    KoreanNameKeyboardOverlayReport, patch_korean_name_keyboard_overlay,
};
use super::name_entry_nickname_companion::{
    NameEntryNicknameCompanionReport, ROUTINE_OFFSET as NICKNAME_COMPANION_ROUTINE_OFFSET,
    build_nickname_companion_program, install_name_entry_nickname_companion,
    routine_address as nickname_companion_routine_address,
    verify_nickname_companion_lookup_preserved,
};
use super::name_entry_nickname_hangul_page::{
    NameEntryNicknameHangulPageReport, install_name_entry_nickname_hangul_page,
    name_entry_nickname_hangul_page_replacements,
};
use super::name_entry_record_compositor::{
    NameEntryRecordClaim, NameEntryRecordContribution, compose_name_entry_record,
    effective_data_claims,
};
use super::name_entry_redisplay_hook::{
    HOOK_START_OFFSET as REDISPLAY_HOOK_OFFSET, NameEntryRedisplayHookReport,
    build_name_entry_redisplay_replacement, install_name_entry_redisplay_hook,
};
use super::name_entry_selection_hook::{
    HOOK_START_OFFSET as SELECTION_HOOK_OFFSET, NameEntrySelectionHookReport,
    build_name_entry_selection_replacement, install_name_entry_selection_hook,
};
use super::name_entry_slot_navigation_guard::{
    NameEntrySlotNavigationGuardReport, ROUTINE_BYTE_COUNT as SLOT_NAVIGATION_ROUTINE_BYTE_COUNT,
    ROUTINE_OFFSET as SLOT_NAVIGATION_ROUTINE_OFFSET,
    build_name_entry_slot_navigation_guard_program, install_name_entry_slot_navigation_guard,
    previous_slot_entry_address,
};

pub(super) struct ComposedNameEntryOverlayBuild {
    pub(super) bytes: Vec<u8>,
    pub(super) keyboard: KoreanNameKeyboardOverlayReport,
    pub(super) nickname_hangul_page: NameEntryNicknameHangulPageReport,
    pub(super) nickname_companion: NameEntryNicknameCompanionReport,
    pub(super) selection_hook: NameEntrySelectionHookReport,
    pub(super) delete_handler: NameEntryDeleteHandlerReport,
    pub(super) slot_navigation_guard: NameEntrySlotNavigationGuardReport,
    pub(super) confirmation_hook: NameEntryConfirmationHookReport,
    pub(super) redisplay_hook: NameEntryRedisplayHookReport,
}

pub(super) fn compose_name_entry_overlay(
    source: &[u8],
    source_sha256: &str,
    keyboard: &NameInputKeyboardPlan,
    consumer_layout: &NameGlyphConsumerLayout,
    redisplay_runtime: &mut NameInputRedisplayRuntimeProgram,
) -> Result<ComposedNameEntryOverlayBuild> {
    let keyboard_overlay = patch_korean_name_keyboard_overlay(source, keyboard)?;
    let (control_labels, control_instructions) = super::name_entry_control_labels::build_control_labels(source)?;
    let mut control_candidate = source.to_vec();
    let control_range = super::name_entry_control_labels::OFFSET..super::name_entry_control_labels::END;
    control_candidate[control_range.clone()].copy_from_slice(&control_labels);
    let default_names = super::name_entry_defaults::patch_name_entry_defaults(source)?;
    let mut nickname_hangul_page_candidate = source.to_vec();
    let nickname_hangul_page =
        install_name_entry_nickname_hangul_page(source, &mut nickname_hangul_page_candidate)?;
    let mut nickname_companion_candidate = source.to_vec();
    let mut nickname_companion = install_name_entry_nickname_companion(
        source,
        &mut nickname_companion_candidate,
        consumer_layout,
    )?;
    let nickname_companion_program = build_nickname_companion_program(consumer_layout)?;
    let mut selection_hook_candidate = source.to_vec();
    let selection_hook = install_name_entry_selection_hook(
        &mut selection_hook_candidate,
        redisplay_runtime.selected_key_handler_address,
    )?;
    redisplay_runtime.report.selection_hook_installed = true;
    let mut delete_handler_candidate = source.to_vec();
    let delete_handler = install_name_entry_delete_handler(
        source,
        &mut delete_handler_candidate,
        redisplay_runtime.compound_final_backspace_address,
    )?;
    let delete_handler_program = build_name_entry_delete_handler_program(
        redisplay_runtime.compound_final_backspace_address,
    )?;
    let mut slot_navigation_guard_candidate = source.to_vec();
    let slot_navigation_guard =
        install_name_entry_slot_navigation_guard(source, &mut slot_navigation_guard_candidate)?;
    let slot_navigation_guard_program = build_name_entry_slot_navigation_guard_program()?;
    let mut confirmation_hook_candidate = source.to_vec();
    let confirmation_hook = install_name_entry_confirmation_hook(
        source,
        &mut confirmation_hook_candidate,
        confirmation_entry_address(),
    )?;
    let mut redisplay_hook_candidate = source.to_vec();
    let redisplay_hook = install_name_entry_redisplay_hook(
        &mut redisplay_hook_candidate,
        redisplay_runtime.tagged_code_resolver_address,
    )?;
    redisplay_runtime.report.renderer_hook_installed = true;

    let nickname_hangul_page_replacements = name_entry_nickname_hangul_page_replacements();
    let selection_hook_replacement =
        build_name_entry_selection_replacement(redisplay_runtime.selected_key_handler_address)?;
    let confirmation_hook_replacement =
        build_name_entry_confirmation_replacement(confirmation_entry_address());
    let redisplay_hook_replacement =
        build_name_entry_redisplay_replacement(redisplay_runtime.tagged_code_resolver_address);
    let bytes = compose_name_entry_record(
        OVERLAY_PATH,
        source,
        source_sha256,
        vec![
            NameEntryRecordContribution {
                owner: "canonical empty-name defaults",
                candidate: &default_names,
                claims: effective_data_claims(
                    "name-entry-overlay:defaults",
                    "use canonical Hangul identities for native empty-field defaults",
                    source,
                    &default_names,
                    difference_ranges(source, &default_names),
                )?,
            },
            NameEntryRecordContribution {
                owner: "Korean name-entry keyboard",
                candidate: &keyboard_overlay.bytes,
                claims: effective_data_claims(
                    "name-entry-overlay:keyboard",
                    "install Korean name-entry pages, navigation, and lookup data",
                    source,
                    &keyboard_overlay.bytes,
                    difference_ranges(source, &keyboard_overlay.bytes),
                )?,
            },
            NameEntryRecordContribution {
                owner: "name-entry control labels and composition cue",
                candidate: &control_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:control-labels".to_string(),
                    purpose: "show all seven controls and cue vowel completion for unfinished initials".to_string(),
                    runtime_address: overlay_runtime_address(control_range.start),
                    range: control_range,
                    instructions: control_instructions,
                }],
            },
            NameEntryRecordContribution {
                owner: "nickname Hangul page routing",
                candidate: &nickname_hangul_page_candidate,
                claims: nickname_hangul_page_replacements
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, (offset, instruction))| NameEntryRecordClaim::MachineCode {
                            id: format!("name-entry-overlay:nickname-page:{index}"),
                            purpose: "include the Hangul page in nickname input".to_string(),
                            range: offset..offset + 4,
                            runtime_address: overlay_runtime_address(offset),
                            instructions: vec![instruction],
                        },
                    )
                    .collect(),
            },
            NameEntryRecordContribution {
                owner: "nickname companion producer",
                candidate: &nickname_companion_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:nickname-companion".to_string(),
                    purpose: "produce nickname companion codes from persistent name records"
                        .to_string(),
                    range: NICKNAME_COMPANION_ROUTINE_OFFSET
                        ..NICKNAME_COMPANION_ROUTINE_OFFSET
                            + nickname_companion_program.bytes.len(),
                    runtime_address: nickname_companion_routine_address(),
                    instructions: nickname_companion_program.instructions,
                }],
            },
            NameEntryRecordContribution {
                owner: "name-entry selection hook",
                candidate: &selection_hook_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:selection-hook".to_string(),
                    purpose: "route selected keys through the composed-name runtime".to_string(),
                    range: SELECTION_HOOK_OFFSET
                        ..SELECTION_HOOK_OFFSET + selection_hook_replacement.len() * 4,
                    runtime_address: overlay_runtime_address(SELECTION_HOOK_OFFSET),
                    instructions: selection_hook_replacement.to_vec(),
                }],
            },
            NameEntryRecordContribution {
                owner: "name-entry delete handler",
                candidate: &delete_handler_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:delete-handler".to_string(),
                    purpose: "delete composed-name stages without corrupting the record"
                        .to_string(),
                    range: DELETE_ROUTINE_OFFSET..DELETE_ROUTINE_OFFSET + DELETE_ROUTINE_BYTE_COUNT,
                    runtime_address: delete_routine_runtime_address(),
                    instructions: delete_handler_program.instructions,
                }],
            },
            NameEntryRecordContribution {
                owner: "name-entry confirmation hook",
                candidate: &confirmation_hook_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:confirmation-hook".to_string(),
                    purpose: "validate incomplete Hangul composition before confirmation"
                        .to_string(),
                    range: CONFIRMATION_HOOK_OFFSET
                        ..CONFIRMATION_HOOK_OFFSET + confirmation_hook_replacement.len() * 4,
                    runtime_address: overlay_runtime_address(CONFIRMATION_HOOK_OFFSET),
                    instructions: confirmation_hook_replacement.to_vec(),
                }],
            },
            NameEntryRecordContribution {
                owner: "name-entry slot navigation guard",
                candidate: &slot_navigation_guard_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:slot-navigation-guard".to_string(),
                    purpose:
                        "prevent slot changes while the current Hangul composition is incomplete"
                            .to_string(),
                    range: SLOT_NAVIGATION_ROUTINE_OFFSET
                        ..SLOT_NAVIGATION_ROUTINE_OFFSET + SLOT_NAVIGATION_ROUTINE_BYTE_COUNT,
                    runtime_address: previous_slot_entry_address(),
                    instructions: slot_navigation_guard_program.instructions,
                }],
            },
            NameEntryRecordContribution {
                owner: "name-entry redisplay hook",
                candidate: &redisplay_hook_candidate,
                claims: vec![NameEntryRecordClaim::MachineCode {
                    id: "name-entry-overlay:redisplay-hook".to_string(),
                    purpose: "resolve tagged composed-name codes before sprite rendering"
                        .to_string(),
                    range: REDISPLAY_HOOK_OFFSET
                        ..REDISPLAY_HOOK_OFFSET + redisplay_hook_replacement.len() * 4,
                    runtime_address: overlay_runtime_address(REDISPLAY_HOOK_OFFSET),
                    instructions: redisplay_hook_replacement.to_vec(),
                }],
            },
        ],
    )?;
    nickname_companion.legacy_mapping_preserved =
        verify_nickname_companion_lookup_preserved(source, &bytes)?;

    Ok(ComposedNameEntryOverlayBuild {
        bytes,
        keyboard: keyboard_overlay.report,
        nickname_hangul_page,
        nickname_companion,
        selection_hook,
        delete_handler,
        slot_navigation_guard,
        confirmation_hook,
        redisplay_hook,
    })
}

fn overlay_runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + u32::try_from(offset).expect("MGENT offsets fit u32")
}
