use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::sha256_bytes;

use super::corpus::extract_all_dialogue_source_corpus;
use super::corpus_digest::source_corpus_sha256;
use super::format::hex_address;
use super::parser::{DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE};
use super::script_message_contract::validate_compact_message_opcode_contract;
use super::script_model::{
    DialogueSceneAuditConfig, DialogueSceneAuditReport, DialogueScriptAssetSummary,
    DialogueUnresolvedPrimaryScriptAsset,
};
use super::script_runtime::audit_admitted_runtime_scene;
use super::script_source::load_mgame;
use super::script_topology::{audit_script_asset, serialized_asset_bytes};
use super::sources::load_dialogue_runtime_image_sources;

const SCENE_ASSET_PATH: &str = "DAT2/MGK04.BIZ";
const SCRIPT_RUNTIME_BASE: u32 = DECODED_RUNTIME_BASE + 0x4a000;
const OPCODE_DISPATCH_RUNTIME_ADDRESS: u32 = 0x800a_256c;
const SCRIPT_INTERPRETER_RUNTIME_ADDRESS: u32 = 0x800a_9d24;
const LAST_ESTABLISHED_SCRIPT_BOUNDARY: &str = "reversible_source_corpus";
const FIRST_UNESTABLISHED_SCRIPT_BOUNDARY: &str = "mgame_primary_script_topology";
const EXPECTED_RESOLVED_PRIMARY_SCRIPT_ASSET_COUNT: usize = 78;
const EXPECTED_UNRESOLVED_PRIMARY_SCRIPT_ASSET_PATHS: [&str; 0] = [];

pub fn audit_dialogue_scenes(
    config: &DialogueSceneAuditConfig,
) -> Result<DialogueSceneAuditReport> {
    let cue = CueSheet::parse(&config.cue)?;
    let corpus = extract_all_dialogue_source_corpus(&config.cue, &config.codebook)?;
    let source_corpus_sha256 = source_corpus_sha256(&corpus)?;
    let mgame = load_mgame(&cue)?;
    validate_compact_message_opcode_contract(&mgame)?;

    let asset_shard_directory = asset_shard_directory(&config.output)?;
    std::fs::create_dir_all(&asset_shard_directory).with_context(|| {
        format!(
            "failed to create script audit shard directory {}",
            asset_shard_directory.display()
        )
    })?;

    let mut totals = ScriptInventoryTotals::default();
    let mut asset_summaries = Vec::new();
    let mut unresolved_primary_script_assets = Vec::new();
    let mut admitted_decoded = None;
    let sources = load_dialogue_runtime_image_sources(&cue.image_path)?;
    let source_asset_paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        ensure!(
            decoded.len() == DECODED_IMAGE_SIZE,
            "unexpected {} decoded size",
            source.path
        );
        let corpus_asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} corpus asset disappeared", source.path))?;
        if source.path == SCENE_ASSET_PATH {
            admitted_decoded = Some(decoded.clone());
        }
        let audit = match audit_script_asset(&decoded, corpus_asset, &mgame) {
            Ok(audit) => audit,
            Err(error) => {
                unresolved_primary_script_assets.push(DialogueUnresolvedPrimaryScriptAsset {
                    source_path: source.path,
                    last_established_boundary: LAST_ESTABLISHED_SCRIPT_BOUNDARY.to_string(),
                    first_unestablished_boundary: FIRST_UNESTABLISHED_SCRIPT_BOUNDARY.to_string(),
                    detail: format!("{error:#}"),
                });
                continue;
            }
        };
        let shard_bytes = serialized_asset_bytes(&audit)?;
        let shard_path = asset_shard_directory.join(asset_shard_name(&source.path)?);
        std::fs::write(&shard_path, &shard_bytes)
            .with_context(|| format!("failed to write {}", shard_path.display()))?;

        totals.include(&audit);
        let route_binding_count = audit
            .bank_tables
            .iter()
            .map(|bank| bank.route_bindings.len())
            .sum();
        asset_summaries.push(DialogueScriptAssetSummary {
            source_path: source.path.clone(),
            decoded_sha256: audit.decoded_sha256.clone(),
            primary_region_end: audit.primary_region_end.clone(),
            bank_table_count: audit.bank_tables.len(),
            route_binding_count,
            unique_route_table_count: audit.unique_route_table_count,
            route_entry_reference_count: audit.route_entry_reference_count,
            unique_route_entry_count: audit.unique_route_entry_count,
            route_slot_count: audit.route_slot_count,
            unique_entrypoint_count: audit.unique_entrypoint_count,
            reachable_command_count: audit.reachable_command_count,
            contextual_message_reference_count: audit.contextual_message_reference_count,
            referenced_coordinate_count: audit.referenced_coordinate_count,
            referenced_semantic_group_count: audit.referenced_semantic_group_count,
            shard_path: shard_path.display().to_string(),
            shard_sha256: sha256_bytes(&shard_bytes),
        });
    }
    validate_script_population_partition(
        &source_asset_paths,
        &asset_summaries,
        &unresolved_primary_script_assets,
        &totals,
    )?;

    let decoded = admitted_decoded.context("MGK04 dialogue source disappeared")?;
    let admitted_asset = corpus
        .assets
        .iter()
        .find(|asset| asset.source_path == SCENE_ASSET_PATH)
        .context("MGK04 corpus asset disappeared")?;
    let (runtime_evidence, scene) =
        audit_admitted_runtime_scene(config, &decoded, admitted_asset, &mgame)?;
    let admitted_message_reference_count = scene.message_reference_count;

    let all_semantic_groups: BTreeSet<_> = corpus
        .assets
        .iter()
        .flat_map(|asset| &asset.banks)
        .flat_map(|bank| &bank.entries)
        .map(|entry| entry.semantic_source_sha256.clone())
        .collect();
    let unreferenced_coordinate_count = corpus
        .coordinate_count
        .checked_sub(totals.referenced_coordinates.len())
        .context("referenced coordinate denominator exceeds source corpus")?;
    let unreferenced_semantic_group_count = all_semantic_groups
        .len()
        .checked_sub(totals.referenced_semantic_groups.len())
        .context("referenced semantic denominator exceeds source corpus")?;

    let report = DialogueSceneAuditReport {
        kind: "Justice Gakuen 2 evidence-bound dialogue scene audit".to_string(),
        implementation: "Rust original-media script traversal joined to the reversible dialogue corpus and exact emucap evidence"
            .to_string(),
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        codebook_sha256: corpus.codebook_sha256.clone(),
        source_corpus_sha256,
        script_runtime_base: hex_address(SCRIPT_RUNTIME_BASE),
        script_interpreter: hex_address(SCRIPT_INTERPRETER_RUNTIME_ADDRESS),
        opcode_dispatch_table: hex_address(OPCODE_DISPATCH_RUNTIME_ADDRESS),
        primary_script_inventory_complete: unresolved_primary_script_assets.is_empty(),
        source_asset_count: source_asset_paths.len(),
        resolved_primary_script_asset_count: asset_summaries.len(),
        unresolved_primary_script_asset_count: unresolved_primary_script_assets.len(),
        bank_table_count: totals.bank_table_count,
        route_binding_count: totals.route_binding_count,
        unique_route_table_count: totals.unique_route_table_count,
        route_entry_reference_count: totals.route_entry_reference_count,
        unique_route_entry_count: totals.unique_route_entry_count,
        route_slot_count: totals.route_slot_count,
        unique_entrypoint_count: totals.unique_entrypoint_count,
        reachable_command_count: totals.reachable_command_count,
        contextual_message_reference_count: totals.contextual_message_reference_count,
        referenced_coordinate_count: totals.referenced_coordinates.len(),
        referenced_semantic_group_count: totals.referenced_semantic_groups.len(),
        unreferenced_coordinate_count,
        unreferenced_semantic_group_count,
        asset_shard_directory: asset_shard_directory.display().to_string(),
        resolved_primary_script_assets: asset_summaries,
        unresolved_primary_script_assets,
        admitted_scene_count: 1,
        admitted_message_reference_count,
        runtime_evidence,
        scenes: vec![scene],
        limitations: vec![
            "The current MGAME primary-script profile resolves all 78 admitted runtime images. This closes the primary-script partition only; selector banks and other consumer families remain separately scoped."
                .to_string(),
            "All primary MGK command roots and contextual message coordinates are recovered, but only the MGK04 bank-0 variant-0 enrollment route currently has emucap-backed semantic scene admission."
                .to_string(),
            "Opaque presentation opcodes retain exact bytes and handler identities; they do not silently assign character names, speaker identities, or listener identities."
                .to_string(),
            "Segments preserve script graph order. A segment table is not flattened into a single unconditional dialogue sequence."
                .to_string(),
        ],
    };
    write_json(&config.output, &report)?;
    Ok(report)
}

#[derive(Default)]
struct ScriptInventoryTotals {
    bank_table_count: usize,
    route_binding_count: usize,
    unique_route_table_count: usize,
    route_entry_reference_count: usize,
    unique_route_entry_count: usize,
    route_slot_count: usize,
    unique_entrypoint_count: usize,
    reachable_command_count: usize,
    contextual_message_reference_count: usize,
    referenced_coordinates: BTreeSet<String>,
    referenced_semantic_groups: BTreeSet<String>,
}

impl ScriptInventoryTotals {
    fn include(&mut self, audit: &super::script_model::DialogueScriptAssetAudit) {
        self.bank_table_count += audit.bank_tables.len();
        self.route_binding_count += audit
            .bank_tables
            .iter()
            .map(|bank| bank.route_bindings.len())
            .sum::<usize>();
        self.unique_route_table_count += audit.unique_route_table_count;
        self.route_entry_reference_count += audit.route_entry_reference_count;
        self.unique_route_entry_count += audit.unique_route_entry_count;
        self.route_slot_count += audit.route_slot_count;
        self.unique_entrypoint_count += audit.unique_entrypoint_count;
        self.reachable_command_count += audit.reachable_command_count;
        self.contextual_message_reference_count += audit.contextual_message_reference_count;
        self.referenced_coordinates.extend(
            audit
                .message_bindings
                .iter()
                .map(|binding| binding.coordinate_id.clone()),
        );
        self.referenced_semantic_groups.extend(
            audit
                .message_bindings
                .iter()
                .map(|binding| binding.semantic_source_sha256.clone()),
        );
    }
}

fn validate_script_population_partition(
    source_asset_paths: &[String],
    resolved_assets: &[DialogueScriptAssetSummary],
    unresolved_assets: &[DialogueUnresolvedPrimaryScriptAsset],
    totals: &ScriptInventoryTotals,
) -> Result<()> {
    let resolved_asset_paths = resolved_assets
        .iter()
        .map(|asset| asset.source_path.clone())
        .collect::<Vec<_>>();
    let unresolved_asset_paths = unresolved_assets
        .iter()
        .map(|asset| asset.source_path.clone())
        .collect::<Vec<_>>();
    validate_unique_script_partition(
        source_asset_paths,
        &resolved_asset_paths,
        &unresolved_asset_paths,
    )?;
    let unresolved_paths = unresolved_asset_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        source_asset_paths.len() == 78,
        "dialogue script source asset count changed"
    );
    let expected_unresolved_paths = EXPECTED_UNRESOLVED_PRIMARY_SCRIPT_ASSET_PATHS
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();
    let unresolved_details = unresolved_assets
        .iter()
        .map(|asset| format!("{}: {}", asset.source_path, asset.detail))
        .collect::<Vec<_>>();
    ensure!(
        resolved_assets.len() == EXPECTED_RESOLVED_PRIMARY_SCRIPT_ASSET_COUNT
            && unresolved_paths == expected_unresolved_paths,
        "primary script resolution denominator changed: resolved={}, unresolved={:?}, details={:?}",
        resolved_assets.len(),
        unresolved_paths,
        unresolved_details,
    );
    ensure!(
        unresolved_assets.iter().all(|asset| {
            asset.last_established_boundary == LAST_ESTABLISHED_SCRIPT_BOUNDARY
                && asset.first_unestablished_boundary == FIRST_UNESTABLISHED_SCRIPT_BOUNDARY
                && !asset.detail.is_empty()
        }),
        "unresolved primary script boundary evidence is incomplete"
    );
    ensure!(
        totals.bank_table_count == 151
            && totals.route_binding_count == 2_098
            && totals.unique_route_table_count == 2_064
            && totals.route_entry_reference_count == 6_809
            && totals.unique_route_entry_count == 6_731
            && totals.route_slot_count == 6_810
            && totals.unique_entrypoint_count == 6_698
            && totals.reachable_command_count == 110_862
            && totals.contextual_message_reference_count == 40_518
            && totals.referenced_coordinates.len() == 40_026
            && totals.referenced_semantic_groups.len() == 18_221,
        "resolved primary script topology denominator changed: banks={}, route_bindings={}, unique_routes={}, route_entry_references={}, unique_route_entries={}, route_slots={}, entrypoints={}, commands={}, message_references={}, coordinates={}, semantic_groups={}",
        totals.bank_table_count,
        totals.route_binding_count,
        totals.unique_route_table_count,
        totals.route_entry_reference_count,
        totals.unique_route_entry_count,
        totals.route_slot_count,
        totals.unique_entrypoint_count,
        totals.reachable_command_count,
        totals.contextual_message_reference_count,
        totals.referenced_coordinates.len(),
        totals.referenced_semantic_groups.len(),
    );
    Ok(())
}

pub(super) fn validate_unique_script_partition(
    source_asset_paths: &[String],
    resolved_asset_paths: &[String],
    unresolved_asset_paths: &[String],
) -> Result<()> {
    let source_paths = source_asset_paths.iter().cloned().collect::<BTreeSet<_>>();
    let resolved_paths = resolved_asset_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved_paths = unresolved_asset_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        source_paths.len() == source_asset_paths.len()
            && resolved_paths.len() == resolved_asset_paths.len()
            && unresolved_paths.len() == unresolved_asset_paths.len(),
        "dialogue script classification contains a duplicate asset"
    );
    ensure!(
        resolved_paths.is_disjoint(&unresolved_paths)
            && resolved_paths
                .union(&unresolved_paths)
                .cloned()
                .collect::<BTreeSet<_>>()
                == source_paths,
        "dialogue script classification does not partition the source population"
    );
    Ok(())
}

fn asset_shard_directory(output: &Path) -> Result<PathBuf> {
    let stem = output
        .file_stem()
        .context("script audit output path has no file stem")?
        .to_string_lossy();
    Ok(output.with_file_name(format!("{stem}.assets")))
}

fn asset_shard_name(source_path: &str) -> Result<String> {
    Ok(format!(
        "{}.json",
        Path::new(source_path)
            .file_stem()
            .context("script source path has no stem")?
            .to_string_lossy()
            .to_ascii_lowercase()
    ))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("failed to write {}", path.display()))
}
