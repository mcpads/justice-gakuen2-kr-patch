//! Explicit analysis output consumed by the font/disc builders. Loading never
//! falls back to translation discovery, auditing, or code allocation.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::compression::decompress;
use crate::development_build_spec::load_development_build_spec;
use crate::name_input::NameGlyphConsumerLayout;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::dialogue_code_allocation_model::DialogueCodeAllocationAsset;
use super::dialogue_fixed_code_consumers_model::DialogueFixedCodeConsumerAssetAudit;
use super::dialogue_font_build_model::DialogueFontBuildConfig;
use super::dialogue_message_build_model::DialogueMessageBuildConfig;
use super::dialogue_message_plan::plan_dialogue_messages;
use super::dialogue_message_plan_model::DialogueMessageBuildPlan;
use super::translation_model::DialogueDevelopmentInputPolicy;

const MANIFEST: &str = "prepared-dialogue.json";
const KIND: &str = "justice_gakuen2_prepared_dialogue";

#[derive(Serialize, Deserialize)]
pub(super) struct PreparedGlyphAllocation {
    pub static_allocation_complete: bool,
    pub fixed_code_consumer_ownership_complete: bool,
    pub name_glyph_layout: NameGlyphConsumerLayout,
    pub fixed_code_consumers: Vec<DialogueFixedCodeConsumerAssetAudit>,
    pub assets: Vec<DialogueCodeAllocationAsset>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedDialogue {
    kind: String,
    input_bindings: BTreeMap<String, String>,
    message_plan: DialogueMessageBuildPlan,
    allocation: PreparedGlyphAllocation,
}

/// Run the analysis explicitly. Pixel rendering and compression belong to build.
pub fn prepare_dialogue_inputs(
    cue: &Path,
    spec: &Path,
    input_policy: DialogueDevelopmentInputPolicy,
    output_dir: &Path,
    force: bool,
) -> Result<()> {
    let _lock = crate::output_lock::OutputLock::writer(output_dir)?;
    let spec = load_development_build_spec(spec)?;
    let config = DialogueMessageBuildConfig {
        cue: cue.to_owned(),
        codebook: spec.assets.dialogue_codebook,
        translation: spec.assets.dialogue_translations,
        selector_translation: spec.assets.dialogue_selector_translations,
        name_input_keyboard: spec.assets.name_entry_candidates.join("keyboard.json"),
        translation_audit_output: output_dir.join("dialogue-translation-audit.json"),
        selector_translation_audit_output: output_dir
            .join("dialogue-selector-translation-audit.json"),
        code_allocation_output: output_dir.join("dialogue-code-allocation.json"),
        output_dir: output_dir.to_owned(),
        force,
        input_policy,
    };
    ensure!(
        force || !output_dir.join(MANIFEST).exists(),
        "prepared dialogue exists; use --force to replace it"
    );
    let bindings = input_bindings(
        &config.codebook,
        &config.translation,
        &config.selector_translation,
        &config.name_input_keyboard,
    )?;
    std::fs::create_dir_all(output_dir)?;
    let source = SupportedSourceDisc::open(cue)?;
    let (message_plan, allocation) = plan_dialogue_messages(&config, &source)?;
    ensure!(
        bindings
            == input_bindings(
                &config.codebook,
                &config.translation,
                &config.selector_translation,
                &config.name_input_keyboard
            )?,
        "dialogue inputs changed during preparation"
    );
    validate_source_population(
        message_plan
            .assets
            .iter()
            .map(|asset| asset.source_path.as_str()),
    )?;
    for asset in &message_plan.assets {
        write_message_member(
            output_dir,
            &asset.rebuilt_decoded_sha256,
            &asset.rebuilt_decoded,
        )?;
    }
    let prepared = PreparedDialogue {
        kind: KIND.into(),
        input_bindings: bindings,
        message_plan,
        allocation: PreparedGlyphAllocation {
            static_allocation_complete: allocation.static_allocation_complete,
            fixed_code_consumer_ownership_complete: allocation
                .fixed_code_consumer_ownership_complete,
            name_glyph_layout: allocation.name_glyph_layout,
            fixed_code_consumers: allocation.fixed_code_consumers.assets,
            assets: allocation.assets,
        },
    };
    publish_manifest(output_dir, &serde_json::to_vec(&prepared)?)
}

pub(super) fn load_prepared_dialogue(
    config: &DialogueFontBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<(DialogueMessageBuildPlan, PreparedGlyphAllocation, String)> {
    let _lock = crate::output_lock::OutputLock::reader(&config.prepared_dialogue)?;
    let (mut prepared, manifest_sha256) = read_prepared(
        &config.prepared_dialogue,
        input_bindings(
            &config.codebook,
            &config.translation,
            &config.selector_translation,
            &config.name_input_keyboard,
        )?,
        config.input_policy,
    )?;
    ensure!(
        prepared.message_plan.source_bin_sha256 == source.source_bin_sha256(),
        "prepared dialogue source differs from build input"
    );
    for asset in &mut prepared.message_plan.assets {
        let (_, stored) = source.read_record(&asset.source_path)?;
        ensure!(
            sha256_bytes(&stored) == asset.original_stored_sha256,
            "prepared message source changed: {}",
            asset.source_path
        );
        asset.original_decoded = decompress(&stored, true)?;
        asset.rebuilt_decoded = std::fs::read(
            config
                .prepared_dialogue
                .join(message_member_path(&asset.rebuilt_decoded_sha256)?),
        )?;
        ensure!(
            sha256_bytes(&asset.original_decoded) == asset.original_decoded_sha256
                && sha256_bytes(&asset.rebuilt_decoded) == asset.rebuilt_decoded_sha256,
            "prepared message bytes changed: {}",
            asset.source_path
        );
    }
    Ok((prepared.message_plan, prepared.allocation, manifest_sha256))
}

pub(super) fn preflight_prepared_dialogue(
    directory: &Path,
    assets: &crate::development_build_spec::DevelopmentAssetPaths,
    policy: DialogueDevelopmentInputPolicy,
) -> Result<()> {
    let _lock = crate::output_lock::OutputLock::reader(directory)?;
    read_prepared(
        directory,
        input_bindings(
            &assets.dialogue_codebook,
            &assets.dialogue_translations,
            &assets.dialogue_selector_translations,
            &assets.name_entry_candidates.join("keyboard.json"),
        )?,
        policy,
    )?;
    Ok(())
}

fn read_prepared(
    directory: &Path,
    bindings: BTreeMap<String, String>,
    policy: DialogueDevelopmentInputPolicy,
) -> Result<(PreparedDialogue, String)> {
    let path = directory.join(MANIFEST);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "missing prepared dialogue {}; run prepare-dialogue-inputs explicitly",
            path.display()
        )
    })?;
    let manifest_sha256 = sha256_bytes(&bytes);
    let prepared: PreparedDialogue = serde_json::from_slice(&bytes)?;
    ensure!(prepared.kind == KIND, "unsupported prepared dialogue input");
    ensure!(
        prepared.input_bindings == bindings,
        "prepared dialogue inputs are stale; run prepare-dialogue-inputs explicitly"
    );
    ensure!(
        prepared.message_plan.input_policy == policy.label()
            && prepared.allocation.static_allocation_complete,
        "prepared dialogue policy or allocation differs from build inputs"
    );
    validate_source_population(
        prepared
            .message_plan
            .assets
            .iter()
            .map(|asset| asset.source_path.as_str()),
    )?;
    validate_source_population(
        prepared
            .allocation
            .assets
            .iter()
            .map(|asset| asset.source_path.as_str()),
    )?;
    validate_source_population(
        prepared
            .allocation
            .fixed_code_consumers
            .iter()
            .map(|asset| asset.source_path.as_str()),
    )?;
    for asset in &prepared.message_plan.assets {
        let member = directory.join(message_member_path(&asset.rebuilt_decoded_sha256)?);
        ensure!(
            member.is_file(),
            "missing prepared message member: {}",
            member.display()
        );
    }
    Ok((prepared, manifest_sha256))
}

fn publish_manifest(directory: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = directory.join("prepared-dialogue.json.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, directory.join(MANIFEST))?;
    Ok(())
}

fn validate_source_population<'a>(paths: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut paths = paths.collect::<Vec<_>>();
    paths.sort_unstable();
    ensure!(
        super::sources::expected_runtime_image_population(&paths),
        "prepared dialogue must cover the exact supported runtime source population"
    );
    Ok(())
}

fn message_member_path(digest: &str) -> Result<PathBuf> {
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "invalid prepared message digest"
    );
    Ok(Path::new("messages").join(format!("{digest}.bin")))
}

fn write_message_member(directory: &Path, digest: &str, bytes: &[u8]) -> Result<()> {
    ensure!(
        sha256_bytes(bytes) == digest,
        "prepared message producer digest differs"
    );
    let path = directory.join(message_member_path(digest)?);
    std::fs::create_dir_all(path.parent().context("message member has no parent")?)?;
    if path.exists() {
        ensure!(
            std::fs::read(&path)? == bytes,
            "existing prepared member is corrupt"
        );
        return Ok(());
    }
    let staged = path.with_extension("bin.tmp");
    std::fs::write(&staged, bytes)?;
    std::fs::rename(staged, path)?;
    Ok(())
}

fn input_bindings(
    codebook: &Path,
    primary: &Path,
    selector: &Path,
    keyboard: &Path,
) -> Result<BTreeMap<String, String>> {
    let mut bindings = BTreeMap::new();
    // Layout semantics are preparation inputs too; reject old space-free counts.
    bindings.insert("backup-slot-layout".into(), "contiguous-cells-v2".into());
    bindings.insert(
        "choice-columns".into(),
        super::choice_columns::asset_sha256()?,
    );
    bindings.insert(
        "stat-result-layout".into(),
        super::stat_result_layout::asset_sha256()?,
    );
    for (role, path) in [
        ("codebook", codebook),
        ("primary", primary),
        ("selector", selector),
        ("keyboard", keyboard),
    ] {
        collect_input_files(path, path, role, &mut bindings)?;
    }
    Ok(bindings)
}

fn collect_input_files(
    root: &Path,
    path: &Path,
    role: &str,
    bindings: &mut BTreeMap<String, String>,
) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("missing dialogue input {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "dialogue preparation input must not be a symlink: {}",
        path.display()
    );
    if metadata.is_dir() {
        let mut children = std::fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<PathBuf>>>()?;
        children.sort();
        for child in children {
            collect_input_files(root, &child, role, bindings)?;
        }
    } else {
        ensure!(
            metadata.is_file(),
            "unsupported dialogue input {}",
            path.display()
        );
        let relative = path.strip_prefix(root)?;
        bindings.insert(
            format!("{role}/{}", relative.display()),
            sha256_bytes(&std::fs::read(path)?),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_manifest_write_preserves_previous_manifest_and_members() {
        let root = crate::test_support::temporary_directory("prepared-publication");
        std::fs::create_dir_all(&root).unwrap();
        let old = b"previous message";
        let old_hash = sha256_bytes(old);
        write_message_member(&root, &old_hash, old).unwrap();
        publish_manifest(&root, b"previous manifest").unwrap();
        let new = b"new message";
        write_message_member(&root, &sha256_bytes(new), new).unwrap();
        std::fs::create_dir(root.join("prepared-dialogue.json.tmp")).unwrap();
        assert!(publish_manifest(&root, b"new manifest").is_err());
        assert_eq!(
            std::fs::read(root.join(MANIFEST)).unwrap(),
            b"previous manifest"
        );
        assert_eq!(
            std::fs::read(root.join(message_member_path(&old_hash).unwrap())).unwrap(),
            old
        );
        std::fs::remove_dir(root.join("prepared-dialogue.json.tmp")).unwrap();
        publish_manifest(&root, b"new manifest").unwrap();
        assert_eq!(std::fs::read(root.join(MANIFEST)).unwrap(), b"new manifest");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_or_corrupt_members_cannot_overwrite_previous_content() {
        let root = crate::test_support::temporary_directory("prepared-members");
        let bytes = b"good";
        let hash = sha256_bytes(bytes);
        write_message_member(&root, &hash, bytes).unwrap();
        assert!(write_message_member(&root, &hash, b"different").is_err());
        let path = root.join(message_member_path(&hash).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(write_message_member(&root, &hash, bytes).is_err());
        assert!(message_member_path("../../elsewhere").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_and_fabricated_source_populations_are_rejected() {
        assert!(validate_source_population(std::iter::empty()).is_err());
        assert!(validate_source_population(std::iter::repeat_n("DAT2/MGT13.BIZ", 78)).is_err());
    }

    #[test]
    fn input_binding_detects_changed_added_and_removed_translation_files() {
        let root = std::env::temp_dir().join(format!("justice-preparation-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("first.json"), b"original").unwrap();
        let bind = || {
            let mut map = BTreeMap::new();
            collect_input_files(&root, &root, "primary", &mut map).unwrap();
            map
        };
        let original = bind();
        std::fs::write(root.join("first.json"), b"changed").unwrap();
        assert_ne!(original, bind());
        std::fs::write(root.join("first.json"), b"original").unwrap();
        assert_eq!(original, bind());
        std::fs::write(root.join("added.json"), b"new").unwrap();
        assert_ne!(original, bind());
        std::fs::remove_file(root.join("first.json")).unwrap();
        assert_ne!(original, bind());
        std::fs::remove_dir_all(root).unwrap();
    }
}
