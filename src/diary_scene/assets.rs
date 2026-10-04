use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::pipeline::BASELINE_BIN_SHA256;

use super::model::{
    DIARY_SCENE_RUNTIME_CATALOGUE_KIND, DIARY_SCENE_SOURCE_CATALOGUE_KIND, DiarySceneEntry,
    DiarySceneFixedPresentationFamily, DiarySceneFixedPresentationFamilySet, DiarySceneManifest,
    DiarySceneRuntimeBundleBinding, DiarySceneRuntimeCatalogue, DiarySceneSourceCatalogue,
    DiarySceneUnit,
};
use super::source::{
    DIARY_SCENE_PATH, SOURCE_ARCHIVE_SHA256, SOURCE_MEMBER_COUNT, SOURCE_RUNTIME_BUNDLE_COUNT,
    SOURCE_RUNTIME_MEMBER_COUNT,
};

const MANIFEST_KIND: &str = "justice_gakuen2_diary_scene_manifest";
const UNIT_KIND: &str = "justice_gakuen2_diary_scene_unit";
const FIXED_PRESENTATION_KIND: &str = "justice_gakuen2_diary_scene_fixed_presentation_family";
const FIXED_PRESENTATION_SET_KIND: &str =
    "justice_gakuen2_diary_scene_fixed_presentation_family_set";

#[derive(Deserialize)]
struct AssetKind {
    kind: String,
}

pub(super) struct DiarySceneAssets {
    pub(super) source: DiarySceneSourceCatalogue,
    pub(super) runtime_bundles: Vec<DiarySceneRuntimeBundleBinding>,
    pub(super) fixed_presentations: Vec<DiarySceneFixedPresentationFamily>,
    pub(super) entries: Vec<DiarySceneEntry>,
    pub(super) portraits: Option<std::path::PathBuf>,
    pub(super) calendar_backgrounds: Option<std::path::PathBuf>,
    pub(super) month_overview: Option<std::path::PathBuf>,
    pub(super) scene_backgrounds: Option<std::path::PathBuf>,
}

pub(super) fn load_diary_scene_assets(directory: &Path) -> Result<DiarySceneAssets> {
    let manifest_path = directory.join("manifest.json");
    let manifest: DiarySceneManifest = read_json(&manifest_path)?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported diary scene manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.units.is_empty(),
        "diary scene manifest has no units"
    );
    ensure!(
        !manifest.runtime_catalogues.is_empty(),
        "diary scene manifest has no runtime catalogues"
    );
    validate_relative_path(&manifest.source_catalogue)?;
    let source: DiarySceneSourceCatalogue = read_json(&directory.join(&manifest.source_catalogue))?;
    validate_source_catalogue(&source)?;

    let mut catalogue_paths = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut runtime_bundles = Vec::new();
    for relative_path in &manifest.runtime_catalogues {
        validate_relative_path(relative_path)?;
        ensure!(
            catalogue_paths.insert(relative_path.clone()),
            "duplicate diary scene runtime catalogue {}",
            relative_path.display()
        );
        let catalogue: DiarySceneRuntimeCatalogue = read_json(&directory.join(relative_path))?;
        ensure!(
            catalogue.kind == DIARY_SCENE_RUNTIME_CATALOGUE_KIND,
            "unsupported diary scene runtime catalogue kind in {}",
            relative_path.display()
        );
        ensure!(
            families.insert(catalogue.family.clone()),
            "duplicate diary scene runtime family {}",
            catalogue.family
        );
        ensure!(
            !catalogue.bundles.is_empty(),
            "diary scene runtime catalogue {} has no bundles",
            relative_path.display()
        );
        runtime_bundles.extend(catalogue.bundles);
    }
    validate_runtime_bundles(&runtime_bundles, source.members.len())?;

    ensure!(
        !manifest.fixed_presentations.is_empty(),
        "diary scene manifest has no fixed-presentation families"
    );
    let mut fixed_presentation_paths = BTreeSet::new();
    let mut fixed_presentation_set_ids = BTreeSet::<String>::new();
    let mut fixed_presentations = Vec::new();
    for relative_path in &manifest.fixed_presentations {
        validate_relative_path(relative_path)?;
        ensure!(
            fixed_presentation_paths.insert(relative_path.clone()),
            "duplicate diary scene fixed-presentation path {}",
            relative_path.display()
        );
        let document_path = directory.join(relative_path);
        let document_kind: AssetKind = read_json(&document_path)?;
        match document_kind.kind.as_str() {
            FIXED_PRESENTATION_KIND => {
                let mut family: DiarySceneFixedPresentationFamily = read_json(&document_path)?;
                if let Some(path) = &family.team_up_names_path {
                    let names = crate::team_up_names::TeamUpNames::load(&directory.join(path))?;
                    for region in &mut family.translated_regions {
                        if region.korean_text.is_empty() {
                            region.korean_text =
                                names.translation(&region.source_text)?.to_string();
                        } else {
                            ensure!(
                                names.translation(&region.source_text).is_err(),
                                "cooperative name {} must use shared wording, not a local copy",
                                region.id
                            );
                        }
                    }
                    family.team_up_names_sha256 = Some(names.sha256);
                }
                ensure!(
                    family.kind == FIXED_PRESENTATION_KIND,
                    "fixed-presentation family kind changed while loading {}",
                    relative_path.display()
                );
                fixed_presentations.push(family);
            }
            FIXED_PRESENTATION_SET_KIND => {
                let set: DiarySceneFixedPresentationFamilySet = read_json(&document_path)?;
                ensure!(
                    set.kind == FIXED_PRESENTATION_SET_KIND,
                    "fixed-presentation set kind changed while loading {}",
                    relative_path.display()
                );
                ensure!(
                    !set.id.trim().is_empty() && fixed_presentation_set_ids.insert(set.id.clone()),
                    "duplicate or empty diary scene fixed-presentation set id"
                );
                ensure!(
                    !set.variants.is_empty(),
                    "diary scene fixed-presentation set {} has no variants",
                    set.id
                );
                fixed_presentations.extend(expand_fixed_presentation_set(set));
            }
            kind => bail!(
                "unsupported diary scene fixed-presentation kind {kind:?} in {}",
                relative_path.display()
            ),
        }
    }
    validate_fixed_presentations(&fixed_presentations, &runtime_bundles)?;

    let mut unit_paths = BTreeSet::new();
    let mut entries = Vec::new();
    for relative_path in &manifest.units {
        validate_relative_path(relative_path)?;
        ensure!(
            unit_paths.insert(relative_path.clone()),
            "duplicate diary scene unit {}",
            relative_path.display()
        );
        let unit: DiarySceneUnit = read_json(&directory.join(relative_path))?;
        ensure!(
            unit.kind == UNIT_KIND,
            "unsupported diary scene unit kind in {}",
            relative_path.display()
        );
        ensure!(!unit.entries.is_empty(), "diary scene unit is empty");
        entries.extend(unit.entries);
    }
    validate_entries(&entries, &source)?;
    if let Some(path) = &manifest.calendar_backgrounds {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.scene_backgrounds {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.month_overview {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.portraits {
        validate_relative_path(path)?;
    }
    Ok(DiarySceneAssets {
        scene_backgrounds: manifest.scene_backgrounds.map(|path| directory.join(path)),
        month_overview: manifest.month_overview.map(|path| directory.join(path)),
        portraits: manifest.portraits.map(|path| directory.join(path)),
        calendar_backgrounds: manifest
            .calendar_backgrounds
            .map(|path| directory.join(path)),
        source,
        runtime_bundles,
        fixed_presentations,
        entries,
    })
}

fn expand_fixed_presentation_set(
    set: DiarySceneFixedPresentationFamilySet,
) -> Vec<DiarySceneFixedPresentationFamily> {
    set.variants
        .into_iter()
        .map(|variant| {
            let mut translated_regions = variant.translated_regions;
            translated_regions.extend(set.shared_translated_regions.iter().cloned());
            DiarySceneFixedPresentationFamily {
                kind: FIXED_PRESENTATION_KIND.to_string(),
                id: variant.id,
                source_decoded_sha256: variant.source_decoded_sha256,
                tim_offset: set.tim_offset,
                source_tim_sha256: variant.source_tim_sha256,
                team_up_names_path: None,
                team_up_names_sha256: None,
                transparent_index: set.transparent_index,
                consumers: variant.consumers,
                translated_regions,
                preserved_regions: Vec::new(),
                preserved_remainder: Some(set.preserved_remainder.clone()),
            }
        })
        .collect()
}

fn validate_fixed_presentations(
    families: &[DiarySceneFixedPresentationFamily],
    runtime_bundles: &[DiarySceneRuntimeBundleBinding],
) -> Result<()> {
    let runtime_members = runtime_bundles
        .iter()
        .flat_map(|bundle| {
            bundle
                .members
                .iter()
                .map(move |member| ((bundle.path.as_str(), member.index), member))
        })
        .collect::<BTreeMap<_, _>>();
    let mut family_ids = BTreeSet::new();
    let mut owned_consumers = BTreeSet::new();
    for family in families {
        ensure!(
            !family.id.trim().is_empty() && family_ids.insert(family.id.as_str()),
            "duplicate or empty diary scene fixed-presentation family id"
        );
        ensure!(
            family.transparent_index < 16,
            "diary scene fixed-presentation family {} has an invalid transparent index",
            family.id
        );
        ensure!(
            !family.consumers.is_empty()
                && !family.translated_regions.is_empty()
                && (!family.preserved_regions.is_empty() || family.preserved_remainder.is_some()),
            "diary scene fixed-presentation family {} has an incomplete ownership map",
            family.id
        );
        let mut family_consumers = BTreeSet::new();
        for consumer in &family.consumers {
            ensure!(
                family_consumers.insert((consumer.path.as_str(), consumer.member_index)),
                "fixed-presentation family {} repeats consumer {} member {}",
                family.id,
                consumer.path,
                consumer.member_index
            );
            ensure!(
                owned_consumers.insert((consumer.path.as_str(), consumer.member_index)),
                "fixed-presentation consumer {} member {} has multiple owning families",
                consumer.path,
                consumer.member_index
            );
            let member = runtime_members
                .get(&(consumer.path.as_str(), consumer.member_index))
                .with_context(|| {
                    format!(
                        "fixed-presentation family {} names unknown consumer {} member {}",
                        family.id, consumer.path, consumer.member_index
                    )
                })?;
            ensure!(
                member.decoded_sha256 == family.source_decoded_sha256,
                "fixed-presentation family {} consumer {} member {} has a different decoded source",
                family.id,
                consumer.path,
                consumer.member_index
            );
        }
        let mut region_ids = BTreeSet::new();
        let mut cells = Vec::new();
        for region in &family.translated_regions {
            ensure!(
                !region.id.trim().is_empty() && region_ids.insert(region.id.as_str()),
                "fixed-presentation family {} has duplicate or empty region ids",
                family.id
            );
            ensure!(
                !region.source_text.is_empty() && !region.korean_text.is_empty(),
                "fixed-presentation region {} has empty text",
                region.id
            );
            cells.push((region.id.as_str(), region.cell));
        }
        for region in &family.preserved_regions {
            ensure!(
                !region.id.trim().is_empty() && region_ids.insert(region.id.as_str()),
                "fixed-presentation family {} has duplicate or empty region ids",
                family.id
            );
            ensure!(
                !region.purpose.trim().is_empty(),
                "fixed-presentation preserved region {} has no purpose",
                region.id
            );
            cells.push((region.id.as_str(), region.cell));
        }
        if let Some(remainder) = &family.preserved_remainder {
            ensure!(
                !remainder.id.trim().is_empty() && region_ids.insert(remainder.id.as_str()),
                "fixed-presentation family {} has duplicate or empty region ids",
                family.id
            );
            ensure!(
                !remainder.purpose.trim().is_empty(),
                "fixed-presentation preserved remainder {} has no purpose",
                remainder.id
            );
        }
        for (index, (id, cell)) in cells.iter().enumerate() {
            ensure!(
                cell.width > 0 && cell.height > 0,
                "fixed-presentation region {id} is empty"
            );
            for (other_id, other) in &cells[index + 1..] {
                ensure!(
                    !crate::tim::cells_overlap(*cell, *other),
                    "fixed-presentation regions {id} and {other_id} overlap"
                );
            }
        }
    }
    Ok(())
}

fn validate_source_catalogue(source: &DiarySceneSourceCatalogue) -> Result<()> {
    ensure!(
        source.kind == DIARY_SCENE_SOURCE_CATALOGUE_KIND,
        "unsupported diary scene source catalogue kind {:?}",
        source.kind
    );
    ensure!(
        source.path == DIARY_SCENE_PATH,
        "diary scene source path changed"
    );
    ensure!(
        source.source_bin_sha256 == BASELINE_BIN_SHA256,
        "diary scene source BIN binding changed"
    );
    ensure!(
        source.archive_sha256 == SOURCE_ARCHIVE_SHA256,
        "diary scene source archive binding changed"
    );
    ensure!(
        source.members.len() == SOURCE_MEMBER_COUNT,
        "diary scene source catalogue must bind all {SOURCE_MEMBER_COUNT} members"
    );
    ensure!(
        source
            .members
            .iter()
            .enumerate()
            .all(|(index, member)| member.index == index),
        "diary scene source catalogue indices are not contiguous"
    );
    ensure!(
        source
            .members
            .windows(2)
            .all(|pair| pair[0].offset < pair[1].offset),
        "diary scene source catalogue offsets are not strictly increasing"
    );
    ensure!(
        source.location_block.width > 0 && source.location_block.height > 0,
        "diary scene location block is empty"
    );
    Ok(())
}

fn validate_runtime_bundles(
    bundles: &[DiarySceneRuntimeBundleBinding],
    catalogue_member_count: usize,
) -> Result<()> {
    ensure!(
        bundles.len() == SOURCE_RUNTIME_BUNDLE_COUNT,
        "diary scene runtime catalogues must bind all {SOURCE_RUNTIME_BUNDLE_COUNT} bundles"
    );
    let mut paths = BTreeSet::new();
    let mut runtime_member_count = 0usize;
    for bundle in bundles {
        ensure!(
            paths.insert(bundle.path.as_str()),
            "duplicate diary scene runtime bundle {}",
            bundle.path
        );
        ensure!(
            bundle.member_count == bundle.members.len(),
            "diary scene runtime bundle {} has incomplete member bindings",
            bundle.path
        );
        ensure!(
            bundle
                .members
                .iter()
                .enumerate()
                .all(|(index, member)| member.index == index),
            "diary scene runtime member indices are not contiguous in {}",
            bundle.path
        );
        ensure!(
            bundle
                .members
                .windows(2)
                .all(|pair| pair[0].offset < pair[1].offset),
            "diary scene runtime member offsets are not strictly increasing in {}",
            bundle.path
        );
        for member in &bundle.members {
            ensure!(
                !member.catalogue_member_indices.is_empty(),
                "diary scene runtime member {} in {} has no catalogue owner",
                member.index,
                bundle.path
            );
            ensure!(
                member
                    .catalogue_member_indices
                    .iter()
                    .all(|index| *index < catalogue_member_count),
                "diary scene runtime member {} in {} names an unknown catalogue member",
                member.index,
                bundle.path
            );
        }
        runtime_member_count += bundle.members.len();
    }
    ensure!(
        runtime_member_count == SOURCE_RUNTIME_MEMBER_COUNT,
        "diary scene runtime catalogues must bind all {SOURCE_RUNTIME_MEMBER_COUNT} members"
    );
    Ok(())
}

fn validate_entries(entries: &[DiarySceneEntry], source: &DiarySceneSourceCatalogue) -> Result<()> {
    let expected_members = source
        .members
        .iter()
        .filter(|member| member.location_block_sha256.is_some())
        .map(|member| member.index)
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut member_indices = BTreeSet::new();
    let mut translations_by_source_hash = BTreeMap::<&str, &str>::new();
    for entry in entries {
        ensure!(!entry.id.trim().is_empty(), "diary scene entry id is empty");
        ensure!(
            ids.insert(entry.id.as_str()),
            "duplicate diary scene entry id {}",
            entry.id
        );
        ensure!(
            member_indices.insert(entry.member_index),
            "duplicate diary scene catalogue member {}",
            entry.member_index
        );
        ensure!(
            !entry.source_text.is_empty() && !entry.korean_text.is_empty(),
            "diary scene entry {} has empty text",
            entry.id
        );
        let member = source
            .members
            .get(entry.member_index)
            .with_context(|| format!("diary scene entry {} names an unknown member", entry.id))?;
        let source_region_sha256 = member.location_block_sha256.as_deref().with_context(|| {
            format!(
                "diary scene entry {} names a member without a location block",
                entry.id
            )
        })?;
        let palette_roles = member.location_palette_roles.with_context(|| {
            format!(
                "diary scene entry {} names a member without location palette roles",
                entry.id
            )
        })?;
        ensure!(
            palette_roles.clear_index != palette_roles.outline_index
                && palette_roles.clear_index != palette_roles.fill_index
                && palette_roles.outline_index != palette_roles.fill_index,
            "diary scene entry {} has collapsed location palette roles",
            entry.id
        );
        ensure!(
            entry.cell.width > 0 && entry.cell.height > 0,
            "diary scene entry {} has an empty text cell",
            entry.id
        );
        ensure!(
            entry.cell.x >= source.location_block.x
                && entry.cell.y >= source.location_block.y
                && entry.cell.x + entry.cell.width
                    <= source.location_block.x + source.location_block.width
                && entry.cell.y + entry.cell.height
                    <= source.location_block.y + source.location_block.height,
            "diary scene entry {} text cell leaves the owned location block",
            entry.id
        );
        if let Some(existing) =
            translations_by_source_hash.insert(source_region_sha256, entry.korean_text.as_str())
        {
            ensure!(
                existing == entry.korean_text,
                "identical diary scene source graphics have different Korean text"
            );
        }
    }
    ensure!(
        member_indices == expected_members,
        "diary scene translations do not cover every source catalogue location block"
    );
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "diary scene asset path must stay inside its directory: {}",
        path.display()
    );
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires assets/"]
    fn every_catalogue_location_requires_a_translation_including_the_tail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/diary/scenes");
        let mut assets = load_diary_scene_assets(&root).unwrap();
        let tail = assets
            .entries
            .iter()
            .position(|entry| entry.member_index == 92)
            .unwrap();
        assets.entries.remove(tail);
        let error = validate_entries(&assets.entries, &assets.source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("every source catalogue location block")
        );
    }
}
