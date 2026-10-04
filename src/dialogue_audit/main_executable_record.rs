//! Composes all `SLPS_021.20` feature candidates from one immutable source.

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedRecordWritePlan;
use crate::disc::rebuild::DiscRecordSourceIdentity;
use crate::options::{OptionsRecordBuild, register_main_executable_candidates};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::source_disc::{MAIN_EXECUTABLE_PATH, MAIN_EXECUTABLE_SHA256};

use super::shared_name_runtime_build::{
    SharedNameRuntimeBuild, register_shared_name_runtime_candidate,
};

pub(super) const MAIN_EXECUTABLE_RECORD_OWNER: &str = "main-executable record composer";

struct DecodedMainExecutableCandidate {
    data: Vec<u8>,
    source: DiscRecordSourceIdentity,
    sha256: String,
}

pub(super) struct StoredMainExecutableRecord {
    pub(super) data: Vec<u8>,
    pub(super) source: DiscRecordSourceIdentity,
    pub(super) stored_sha256: String,
}

pub(super) fn compose_main_executable_record(
    source: &[u8],
    shared_name_runtime: &SharedNameRuntimeBuild,
    options: &OptionsRecordBuild,
    character_select: &crate::character_select_graphics::CharacterSelectAtlasBuild,
    plain_loading: Option<&crate::loading_art::PlainLoadingBuild>,
    mode_descendants: &crate::mode_descendant_graphics::ModeDescendantGraphicsBuild,
    battle_names: &super::battle_name_build::BattleNameBuild,
) -> Result<StoredMainExecutableRecord> {
    battle_names.validate_texture(mode_descendants)?;
    let decoded = compose_decoded_main_executable(
        source,
        shared_name_runtime,
        options,
        character_select,
        plain_loading,
        mode_descendants,
        battle_names,
    )?;
    identity_store_main_executable(decoded)
}

fn compose_decoded_main_executable(
    source: &[u8],
    shared_name_runtime: &SharedNameRuntimeBuild,
    options: &OptionsRecordBuild,
    character_select: &crate::character_select_graphics::CharacterSelectAtlasBuild,
    plain_loading: Option<&crate::loading_art::PlainLoadingBuild>,
    mode_descendants: &crate::mode_descendant_graphics::ModeDescendantGraphicsBuild,
    battle_names: &super::battle_name_build::BattleNameBuild,
) -> Result<DecodedMainExecutableCandidate> {
    let mut machine_code_sources = PsxMachineCodeSources::default();
    let mut plan =
        DecodedRecordWritePlan::new(MAIN_EXECUTABLE_PATH, source, MAIN_EXECUTABLE_SHA256)?;
    register_shared_name_runtime_candidate(
        shared_name_runtime,
        &mut plan,
        &mut machine_code_sources,
    )?;
    register_main_executable_candidates(options, &mut plan, &mut machine_code_sources)?;
    if let Some(loading) = plain_loading {
        loading.register_executable(&mut plan, &mut machine_code_sources)?;
    }
    crate::character_select_graphics::register_battle_pause_return(
        source,
        character_select,
        &mut plan,
    )?;
    crate::mode_descendant_graphics::register_battle_lettering_layout(
        source,
        mode_descendants,
        &mut plan,
    )?;
    battle_names.register_executable(source, &mut plan, &mut machine_code_sources)?;
    let data = plan.apply(Some(&machine_code_sources))?;
    validate_battle_floor_tables_preserved(source, &data)?;
    let sha256 = sha256_bytes(&data);
    Ok(DecodedMainExecutableCandidate {
        data,
        source: DiscRecordSourceIdentity::new(source.len(), MAIN_EXECUTABLE_SHA256)?,
        sha256,
    })
}

fn identity_store_main_executable(
    decoded: DecodedMainExecutableCandidate,
) -> Result<StoredMainExecutableRecord> {
    let stored_sha256 = sha256_bytes(&decoded.data);
    ensure!(
        stored_sha256 == decoded.sha256,
        "{MAIN_EXECUTABLE_PATH} identity stored transform changed bytes"
    );
    Ok(StoredMainExecutableRecord {
        data: decoded.data,
        source: decoded.source,
        stored_sha256,
    })
}

// The dispatcher at 0x8004ef18..0x8004effc selects eight 12x12 tile
// descriptor arrays. The floor renderer consumes each (texture, palette) byte
// pair, including zero-valued entries. These are live data, not code padding.
fn validate_battle_floor_tables_preserved(source: &[u8], candidate: &[u8]) -> Result<()> {
    let range = 0x7ca30..0x7d330; // PS-X EXE file offsets for 0x8008c230..0x8008cb30.
    let original = source
        .get(range.clone())
        .context("battle floor tables are truncated")?;
    ensure!(
        candidate.get(range) == Some(original),
        "main-executable patch overwrites native battle floor tile/palette tables"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battle_floor_zero_entries_are_protected_from_code_insertion() {
        let source = vec![0; 0x7d330];
        let mut candidate = source.clone();
        validate_battle_floor_tables_preserved(&source, &candidate).unwrap();
        candidate[0x7ca30] = 0x24;
        assert!(validate_battle_floor_tables_preserved(&source, &candidate).is_err());
        candidate[0x7ca30] = 0;
        candidate[0x7d32f] = 1;
        assert!(validate_battle_floor_tables_preserved(&source, &candidate).is_err());
    }
}
