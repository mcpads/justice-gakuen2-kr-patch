use anyhow::{Context, Result, ensure};

use crate::compression::decompress;

use super::corpus_model::DialogueSourceCorpus;
use super::script_model::DialogueScriptAssetAudit;
use super::script_topology::audit_script_asset;
use super::sources::load_dialogue_runtime_image_sources;

const EXPECTED_RESOLVED_ASSET_COUNT: usize = 78;
const EXPECTED_UNRESOLVED_ASSET_PATHS: [&str; 0] = [];

pub(super) struct TranslationPrimaryScriptSet {
    pub(super) source_asset_count: usize,
    pub(super) audits: Vec<DialogueScriptAssetAudit>,
    pub(super) unresolved_asset_paths: Vec<String>,
}

pub(super) fn extract_translation_primary_scripts(
    image_path: &std::path::Path,
    corpus: &DialogueSourceCorpus,
    mgame: &[u8],
) -> Result<TranslationPrimaryScriptSet> {
    let sources = load_dialogue_runtime_image_sources(image_path)?;
    let source_asset_count = sources.len();
    let mut audits = Vec::new();
    let mut unresolved_asset_paths = Vec::new();
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let corpus_asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} corpus asset disappeared", source.path))?;
        match audit_script_asset(&decoded, corpus_asset, mgame) {
            Ok(audit) => audits.push(audit),
            Err(_) => unresolved_asset_paths.push(source.path),
        }
    }
    ensure!(
        source_asset_count == 78
            && audits.len() == EXPECTED_RESOLVED_ASSET_COUNT
            && unresolved_asset_paths == EXPECTED_UNRESOLVED_ASSET_PATHS,
        "translation primary-script population changed: source={source_asset_count}, resolved={}, unresolved={unresolved_asset_paths:?}",
        audits.len(),
    );
    Ok(TranslationPrimaryScriptSet {
        source_asset_count,
        audits,
        unresolved_asset_paths,
    })
}
