//! Composes the independent options writes to `SLPS_021.20` from its immutable source.

use std::ops::Range;

use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::source_disc::MAIN_EXECUTABLE_PATH;

use super::record_build::OptionsRecordBuild;
use super::records_main;
use super::runtime_glyph_upload::RecordsHookInstall;

const RECORDS_MAIN_OWNER: &str = "options records-main producer";
const RECORDS_RUNTIME_OWNER: &str = "options records-runtime producer";
const RECORDS_CONTEXT_HOOKS_OWNER: &str = "options records-context hook producer";

pub(super) struct OptionsMainExecutableContributions {
    pub(super) source_sha256: String,
    pub(super) records_main: Vec<u8>,
    pub(super) records_runtime: Vec<u8>,
    pub(super) records_context_hooks: Vec<u8>,
    pub(super) records_context_hook_writes: Vec<RecordsHookInstall>,
}

pub(super) fn compose_options_main_executable(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
) -> Result<Vec<u8>> {
    let mut machine_code_sources = PsxMachineCodeSources::default();
    let mut plan =
        DecodedRecordWritePlan::new(MAIN_EXECUTABLE_PATH, source, &contributions.source_sha256)?;
    register_contributions(source, contributions, &mut plan, &mut machine_code_sources)?;
    plan.apply(Some(&machine_code_sources))
}

pub(crate) fn register_main_executable_candidates(
    build: &OptionsRecordBuild,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine_code_sources: &mut PsxMachineCodeSources,
) -> Result<()> {
    ensure!(
        sha256_bytes(&build.source_main_executable) == build.report.source_main_executable_sha256,
        "options main-executable source identity changed"
    );
    ensure!(
        sha256_bytes(&build.main_executable) == build.report.output_main_executable_sha256,
        "composed options main-executable identity changed"
    );
    ensure!(
        build.main_executable_contributions.source_sha256
            == build.report.source_main_executable_sha256,
        "options main-executable contributors consumed a different source identity"
    );
    register_contributions(
        &build.source_main_executable,
        &build.main_executable_contributions,
        plan,
        machine_code_sources,
    )
}

fn register_contributions(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine_code_sources: &mut PsxMachineCodeSources,
) -> Result<()> {
    register_records_main(source, contributions, plan)?;
    register_records_runtime(source, contributions, plan)?;
    register_records_context_hooks(source, contributions, plan, machine_code_sources)
}

fn register_records_main(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
    plan: &mut DecodedRecordWritePlan<'_>,
) -> Result<()> {
    ensure_contribution_source(source, contributions)?;
    let declared_claims = [
        (
            "slps:options:records-main:status-text",
            "store localized Records status text",
            WriteIntent::Data,
        ),
        (
            "slps:options:records-main:text-pool",
            "store the localized Records text pool",
            WriteIntent::Data,
        ),
        (
            "slps:options:records-main:exit-text",
            "store localized Records exit text",
            WriteIntent::Data,
        ),
        (
            "slps:options:records-main:pointers",
            "repoint Records text consumers to the rebuilt pool",
            WriteIntent::Metadata,
        ),
    ];
    let mut claims = Vec::new();
    for ((id, purpose, intent), [start, end]) in declared_claims
        .into_iter()
        .zip(records_main::main_executable_expected_write_ranges())
    {
        if range_changes(source, &contributions.records_main, start..end)? {
            claims.push(CandidateWriteClaim {
                id: id.to_string(),
                purpose: purpose.to_string(),
                range: start..end,
                intent,
            });
        }
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: RECORDS_MAIN_OWNER,
        source_sha256: &contributions.source_sha256,
        candidate: &contributions.records_main,
        claims,
    })
}

fn register_records_runtime(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
    plan: &mut DecodedRecordWritePlan<'_>,
) -> Result<()> {
    ensure_contribution_source(source, contributions)?;
    let mut claims = Vec::new();
    for (unit_id, [start, end]) in super::records_runtime::main_executable_write_claims() {
        if range_changes(source, &contributions.records_runtime, start..end)? {
            claims.push(CandidateWriteClaim {
                id: format!("slps:options:records-runtime:{unit_id}"),
                purpose: format!("store localized Records runtime mirror {unit_id}"),
                range: start..end,
                intent: WriteIntent::Data,
            });
        }
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: RECORDS_RUNTIME_OWNER,
        source_sha256: &contributions.source_sha256,
        candidate: &contributions.records_runtime,
        claims,
    })
}

fn register_records_context_hooks(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine_code_sources: &mut PsxMachineCodeSources,
) -> Result<()> {
    ensure_contribution_source(source, contributions)?;
    let mut claims = Vec::with_capacity(contributions.records_context_hook_writes.len());
    for write in &contributions.records_context_hook_writes {
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
        owner: RECORDS_CONTEXT_HOOKS_OWNER,
        source_sha256: &contributions.source_sha256,
        candidate: &contributions.records_context_hooks,
        claims,
    })
}

fn ensure_contribution_source(
    source: &[u8],
    contributions: &OptionsMainExecutableContributions,
) -> Result<()> {
    ensure!(
        sha256_bytes(source) == contributions.source_sha256
            && contributions.records_main.len() == source.len()
            && contributions.records_runtime.len() == source.len()
            && contributions.records_context_hooks.len() == source.len(),
        "options main-executable contributors consumed a different source main executable"
    );
    Ok(())
}

fn range_changes(source: &[u8], candidate: &[u8], range: Range<usize>) -> Result<bool> {
    let source = source
        .get(range.clone())
        .context("options main-executable claim leaves its immutable source")?;
    let candidate = candidate
        .get(range)
        .context("options main-executable claim leaves its candidate")?;
    Ok(source != candidate)
}
