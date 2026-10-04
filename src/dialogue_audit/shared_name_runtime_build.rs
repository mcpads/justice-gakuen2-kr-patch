use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, verify_placed_program};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::name_input::{
    MGAME_RELOAD_WRAPPER_ORIGIN, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
    NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN, NAME_DIALOGUE_RUNTIME_ORIGIN,
    NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
    NameDialogueRuntimeBootstrapProgram, NameDialogueRuntimeProgram, NameGlyphConsumerLayout,
    NameGlyphMaterializationBundle, NicknameHudGlyphLayout,
    SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
    SharedNameOutlineRuntimeProgram, build_name_dialogue_runtime_bootstrap_program,
    build_name_dialogue_runtime_program, build_shared_name_outline_runtime_program,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::source_disc::{
    MAIN_EXECUTABLE_PATH, MAIN_EXECUTABLE_SHA256, MAIN_TEXT_RUNTIME_BASE, MAIN_TEXT_SIZE,
    PSX_EXE_HEADER_SIZE,
};

use super::dialogue_runtime_entry_hook::{
    DialogueRuntimeEntryHookInstall, install_dialogue_runtime_entry_hook,
};
use super::shared_name_runtime_build_model::SharedNameRuntimeBuildReport;

use super::dialogue_name_runtime_build::profile_name_controls;

const BOOTSTRAP_TEXT_OFFSET: usize = 0x0000_0aa4;
const BOOTSTRAP_FILE_OFFSET: usize = PSX_EXE_HEADER_SIZE + BOOTSTRAP_TEXT_OFFSET;
const BOOTSTRAP_SOURCE_REGION_SHA256: &str =
    "7ffb21772afdf16b75c7e774fcef924f07dc104279aa2cc4f3b55ffda3d3a7bb";

const OUTLINE_RUNTIME_TEXT_OFFSET: usize =
    (SHARED_NAME_OUTLINE_RUNTIME_ORIGIN - MAIN_TEXT_RUNTIME_BASE) as usize;
const OUTLINE_RUNTIME_FILE_OFFSET: usize = PSX_EXE_HEADER_SIZE + OUTLINE_RUNTIME_TEXT_OFFSET;
const OUTLINE_SOURCE_REGION_SHA256: &str =
    "58f218aeb259309eeb545345a8f5c715987967b0ec724806d448394da578d0b0";

const DIALOGUE_STORAGE_TEXT_OFFSET: usize = 0x0008_7ad0;
const DIALOGUE_STORAGE_FILE_OFFSET: usize = PSX_EXE_HEADER_SIZE + DIALOGUE_STORAGE_TEXT_OFFSET;
const DIALOGUE_STORAGE_SOURCE_REGION_SHA256: &str =
    "6bb4877dfebc6d4f819999f0f8b65d06aa540746bc63b7c1261282636efcaa15";

const MAIN_EXECUTABLE_OWNER: &str = "shared-name main-executable runtime producer";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SharedNameMachineCodeWrite {
    id: &'static str,
    purpose: &'static str,
    range: std::ops::Range<usize>,
    runtime_address: u32,
    instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SharedNameRuntimeBuild {
    pub(super) bytes: Vec<u8>,
    pub(super) machine_code_writes: Vec<SharedNameMachineCodeWrite>,
    pub(super) outline_program: SharedNameOutlineRuntimeProgram,
    pub(super) bootstrap_program: NameDialogueRuntimeBootstrapProgram,
    pub(super) dialogue_program: NameDialogueRuntimeProgram,
    pub(super) report: SharedNameRuntimeBuildReport,
}

pub(super) fn register_shared_name_runtime_candidate(
    build: &SharedNameRuntimeBuild,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine_code_sources: &mut PsxMachineCodeSources,
) -> Result<()> {
    ensure!(
        sha256_bytes(&build.bytes) == build.report.patched_sha256,
        "shared-name main-executable producer candidate identity changed"
    );
    let mut claims = Vec::with_capacity(build.machine_code_writes.len());
    let data_end = DIALOGUE_STORAGE_FILE_OFFSET + build.dialogue_program.instruction_offset;
    claims.push(CandidateWriteClaim {
        id: "slps:shared-name:dialogue-runtime-data".to_string(),
        purpose: "store shared-name dialogue runtime tables".to_string(),
        range: DIALOGUE_STORAGE_FILE_OFFSET..data_end,
        intent: WriteIntent::Data,
    });
    claims.push(CandidateWriteClaim {
        id: "slps:shared-name:profile-controls".to_string(),
        purpose: "store persistent profile name control records".to_string(),
        range: BOOTSTRAP_FILE_OFFSET + profile_name_controls::STORAGE_OFFSET
            ..BOOTSTRAP_FILE_OFFSET
                + profile_name_controls::STORAGE_OFFSET
                + profile_name_controls::DATA_BYTES,
        intent: WriteIntent::Data,
    });
    for write in &build.machine_code_writes {
        let provenance = machine_code_sources.register(
            write.id,
            write.runtime_address,
            write.instructions.clone(),
        )?;
        claims.push(CandidateWriteClaim {
            id: write.id.to_string(),
            purpose: write.purpose.to_string(),
            range: write.range.clone(),
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: MAIN_EXECUTABLE_OWNER,
        source_sha256: &build.report.source_sha256,
        candidate: &build.bytes,
        claims,
    })
}

pub(super) fn build_shared_name_runtime(
    source: &[u8],
    layout: &NameGlyphConsumerLayout,
    materialization: &NameGlyphMaterializationBundle,
    nickname_hud_layout: &NicknameHudGlyphLayout,
) -> Result<SharedNameRuntimeBuild> {
    ensure!(
        source.len() == PSX_EXE_HEADER_SIZE + MAIN_TEXT_SIZE,
        "unexpected {MAIN_EXECUTABLE_PATH} size"
    );
    ensure!(
        sha256_bytes(source) == MAIN_EXECUTABLE_SHA256,
        "unexpected {MAIN_EXECUTABLE_PATH} SHA-256"
    );
    ensure!(
        source.starts_with(b"PS-X EXE"),
        "{MAIN_EXECUTABLE_PATH} lacks the PS-X EXE signature"
    );
    verify_storage_region(
        source,
        "bootstrap",
        BOOTSTRAP_TEXT_OFFSET,
        BOOTSTRAP_FILE_OFFSET,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
        BOOTSTRAP_SOURCE_REGION_SHA256,
    )?;
    verify_storage_region(
        source,
        "outline",
        OUTLINE_RUNTIME_TEXT_OFFSET,
        OUTLINE_RUNTIME_FILE_OFFSET,
        SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
        SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY,
        OUTLINE_SOURCE_REGION_SHA256,
    )?;
    verify_storage_region(
        source,
        "dialogue storage",
        DIALOGUE_STORAGE_TEXT_OFFSET,
        DIALOGUE_STORAGE_FILE_OFFSET,
        NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
        NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY,
        DIALOGUE_STORAGE_SOURCE_REGION_SHA256,
    )?;
    let mut outline_program = build_shared_name_outline_runtime_program()?;
    let outline_typed =
        verify_placed_program(&outline_program.bytes, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN)?;
    ensure!(
        outline_typed.len() == outline_program.report.typed_instruction_count,
        "shared-name outline typed readback changed"
    );
    materialization.validate()?;
    let mut dialogue_program = match &materialization.format {
        crate::name_input::NameGlyphMaterializationFormat::Components(report) => {
            let runtime_pack = crate::name_input::plan_name_input_runtime_pack(report)?;
            build_name_dialogue_runtime_program(
                layout,
                &runtime_pack,
                &outline_program,
                nickname_hud_layout,
            )?
        }
        crate::name_input::NameGlyphMaterializationFormat::Bands => {
            let pack =
                crate::name_input::NameGlyphBandPack::parse(materialization.pack_bytes.clone())?;
            crate::name_input::build_name_dialogue_band_runtime_program(
                layout,
                &pack,
                &outline_program,
                nickname_hud_layout,
            )?
        }
    };
    let dialogue_instruction_origin = NAME_DIALOGUE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(dialogue_program.instruction_offset)?)
        .context("dialogue runtime instruction origin overflow")?;
    let dialogue_typed = verify_placed_program(
        &dialogue_program.bytes[dialogue_program.instruction_offset..],
        dialogue_instruction_origin,
    )?;
    ensure!(
        dialogue_typed.len() == dialogue_program.report.typed_instruction_count,
        "persistent dialogue-name typed readback changed"
    );
    let mut bootstrap_program = build_name_dialogue_runtime_bootstrap_program(
        dialogue_program.bytes.len(),
        dialogue_program.nickname_hud_scaler_address,
        &dialogue_program.mgame_runtime_repair,
    )?;
    ensure!(
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN as usize + bootstrap_program.bytes.len()
            <= SHARED_NAME_OUTLINE_RUNTIME_ORIGIN as usize
            && SHARED_NAME_OUTLINE_RUNTIME_ORIGIN as usize + outline_program.bytes.len()
                <= MGAME_RELOAD_WRAPPER_ORIGIN as usize,
        "shared-name outline helper overlaps resident bootstrap programs"
    );
    let bootstrap_typed = verify_placed_program(
        &bootstrap_program.bytes,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
    )?;
    ensure!(
        bootstrap_typed.len() == bootstrap_program.report.typed_instruction_count,
        "dialogue runtime bootstrap typed readback changed"
    );
    let mgame_reload_wrapper_file_offset = contained_program_file_offset(
        BOOTSTRAP_FILE_OFFSET,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        dialogue_program.mgame_reload_wrapper_address,
    )?;
    ensure!(
        mgame_reload_wrapper_file_offset >= BOOTSTRAP_FILE_OFFSET + bootstrap_program.bytes.len()
            && mgame_reload_wrapper_file_offset + dialogue_program.mgame_reload_wrapper_bytes.len()
                <= BOOTSTRAP_FILE_OFFSET + NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
        "MGAME reload wrapper overlaps or leaves the bootstrap region"
    );
    let profile_data_start = profile_name_controls::STORAGE_OFFSET;
    let profile_data_end = profile_data_start + profile_name_controls::DATA_BYTES;
    ensure!(
        bootstrap_program
            .bytes
            .get(profile_data_start..profile_data_end)
            .is_some_and(|gap| gap.iter().all(|byte| *byte == 0)),
        "profile controls overlap bootstrap code"
    );
    let mut patched = source.to_vec();
    install_program(
        &mut patched,
        BOOTSTRAP_FILE_OFFSET,
        &bootstrap_program.bytes,
        "bootstrap",
    )?;
    install_program(
        &mut patched,
        mgame_reload_wrapper_file_offset,
        &dialogue_program.mgame_reload_wrapper_bytes,
        "MGAME reload wrapper",
    )?;
    install_program(
        &mut patched,
        OUTLINE_RUNTIME_FILE_OFFSET,
        &outline_program.bytes,
        "outline",
    )?;
    install_program(
        &mut patched,
        DIALOGUE_STORAGE_FILE_OFFSET,
        &dialogue_program.bytes,
        "dialogue storage",
    )?;
    install_program(
        &mut patched,
        BOOTSTRAP_FILE_OFFSET + profile_data_start,
        &profile_name_controls::control_data(),
        "profile name controls",
    )?;
    let DialogueRuntimeEntryHookInstall {
        report: entry_hook,
        file_range: entry_hook_file_range,
        runtime_address: entry_hook_runtime_address,
        replacement_instructions: entry_hook_instructions,
    } = install_dialogue_runtime_entry_hook(source, &mut patched)?;
    bootstrap_program.report.installed = true;
    outline_program.report.installed = true;
    dialogue_program.report.installed = true;
    dialogue_program.mgame_runtime_repair.report.installed = true;
    dialogue_program.report.mgame_runtime_repair.installed = true;
    let machine_code_writes = vec![
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:entry-hook",
            purpose: "call the typed shared-name runtime bootstrap from the PS-X EXE entry",
            range: entry_hook_file_range,
            runtime_address: entry_hook_runtime_address,
            instructions: entry_hook_instructions,
        },
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:bootstrap",
            purpose: "install the typed shared-name runtime bootstrap",
            range: BOOTSTRAP_FILE_OFFSET..BOOTSTRAP_FILE_OFFSET + profile_data_start,
            runtime_address: NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
            instructions: bootstrap_program.instructions[..profile_data_start / 4].to_vec(),
        },
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:bootstrap-after-profile-data",
            purpose: "install bootstrap code after the separately owned profile data",
            range: BOOTSTRAP_FILE_OFFSET + profile_data_end
                ..BOOTSTRAP_FILE_OFFSET + bootstrap_program.bytes.len(),
            runtime_address: NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN + profile_data_end as u32,
            instructions: bootstrap_program.instructions[profile_data_end / 4..].to_vec(),
        },
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:mgame-reload-wrapper",
            purpose: "repair the MGAME-clobbered dialogue runtime code before entry",
            range: mgame_reload_wrapper_file_offset
                ..mgame_reload_wrapper_file_offset
                    + dialogue_program.mgame_reload_wrapper_bytes.len(),
            runtime_address: MGAME_RELOAD_WRAPPER_ORIGIN,
            instructions: dialogue_program.mgame_reload_wrapper_instructions.clone(),
        },
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:outline-runtime",
            purpose: "install the typed shared-name outline runtime",
            range: OUTLINE_RUNTIME_FILE_OFFSET
                ..OUTLINE_RUNTIME_FILE_OFFSET + outline_program.bytes.len(),
            runtime_address: SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
            instructions: outline_program.instructions.clone(),
        },
        SharedNameMachineCodeWrite {
            id: "slps:shared-name:dialogue-runtime-code",
            purpose: "install the typed shared-name dialogue runtime",
            range: DIALOGUE_STORAGE_FILE_OFFSET + dialogue_program.instruction_offset
                ..DIALOGUE_STORAGE_FILE_OFFSET + dialogue_program.bytes.len(),
            runtime_address: dialogue_instruction_origin,
            instructions: dialogue_program.instructions.clone(),
        },
    ];
    Ok(SharedNameRuntimeBuild {
        report: SharedNameRuntimeBuildReport {
            source_path: MAIN_EXECUTABLE_PATH.to_string(),
            source_sha256: MAIN_EXECUTABLE_SHA256.to_string(),
            source_file_size: source.len(),
            entry_hook,
            bootstrap_file_byte_range: storage_file_range(
                BOOTSTRAP_FILE_OFFSET,
                NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
            ),
            bootstrap_address_range: storage_address_range(
                NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
                NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
            ),
            bootstrap_source_region_sha256: BOOTSTRAP_SOURCE_REGION_SHA256.to_string(),
            outline_runtime_file_byte_range: storage_file_range(
                OUTLINE_RUNTIME_FILE_OFFSET,
                SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY,
            ),
            outline_runtime_address_range: storage_address_range(
                SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
                SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY,
            ),
            outline_source_region_sha256: OUTLINE_SOURCE_REGION_SHA256.to_string(),
            dialogue_storage_file_byte_range: storage_file_range(
                DIALOGUE_STORAGE_FILE_OFFSET,
                NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY,
            ),
            dialogue_storage_address_range: storage_address_range(
                NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
                NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY,
            ),
            dialogue_storage_source_region_sha256: DIALOGUE_STORAGE_SOURCE_REGION_SHA256
                .to_string(),
            patched_sha256: sha256_bytes(&patched),
            outline_program: outline_program.report.clone(),
            bootstrap_program: bootstrap_program.report.clone(),
            dialogue_program: dialogue_program.report.clone(),
            runtime_execution_verified: false,
        },
        bytes: patched,
        machine_code_writes,
        outline_program,
        bootstrap_program,
        dialogue_program,
    })
}

#[allow(clippy::too_many_arguments)]
fn verify_storage_region(
    source: &[u8],
    name: &str,
    text_offset: usize,
    file_offset: usize,
    storage_origin: u32,
    capacity: usize,
    source_sha256: &str,
) -> Result<()> {
    ensure!(
        storage_origin == MAIN_TEXT_RUNTIME_BASE + u32::try_from(text_offset)?,
        "shared-name {name} address differs from its executable offset"
    );
    let end = file_offset + capacity;
    let region = source
        .get(file_offset..end)
        .with_context(|| format!("shared-name {name} source region is truncated"))?;
    ensure!(
        region.iter().all(|byte| *byte == 0) && sha256_bytes(region) == source_sha256,
        "shared-name {name} source region changed"
    );
    Ok(())
}

fn install_program(
    patched: &mut [u8],
    file_offset: usize,
    program: &[u8],
    name: &str,
) -> Result<()> {
    let end = file_offset + program.len();
    patched
        .get_mut(file_offset..end)
        .with_context(|| format!("shared-name {name} install range is truncated"))?
        .copy_from_slice(program);
    ensure!(
        patched[file_offset..end] == *program,
        "shared-name {name} install readback changed"
    );
    Ok(())
}

fn contained_program_file_offset(
    container_file_offset: usize,
    container_origin: u32,
    program_origin: u32,
) -> Result<usize> {
    let relative_offset = program_origin
        .checked_sub(container_origin)
        .context("shared-name auxiliary program precedes its main-executable region")?;
    container_file_offset
        .checked_add(usize::try_from(relative_offset)?)
        .context("shared-name auxiliary program file offset overflow")
}

fn storage_file_range(file_offset: usize, capacity: usize) -> [usize; 2] {
    [file_offset, file_offset + capacity]
}

fn storage_address_range(origin: u32, capacity: usize) -> [String; 2] {
    [hex_address(origin), hex_address(origin + capacity as u32)]
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
