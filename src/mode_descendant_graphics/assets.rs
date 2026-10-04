use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;

use crate::menu_audit::kanri_school_label_consumers;
use crate::pipeline::sha256_bytes;
use crate::tim::cells_overlap;

use super::model::{
    ModeDescendantEntry, ModeDescendantManifest, ModeDescendantPlacement, ModeDescendantSurface,
    ModeDescendantUnit,
};

const MANIFEST_KIND: &str = "justice_gakuen2_mode_descendant_manifest";
const UNIT_KIND: &str = "justice_gakuen2_mode_descendant_unit";

#[derive(Debug)]
pub(super) struct ModeDescendantAssets {
    pub(super) manifest_sha256: String,
    pub(super) entries: Vec<ModeDescendantEntry>,
    pub(super) illustrated_panel_paths: Vec<std::path::PathBuf>,
    pub(super) edit_command_sheets_path: std::path::PathBuf,
    pub(super) edit_technique_names_path: std::path::PathBuf,
    pub(super) training_menu_path: Option<std::path::PathBuf>,
    pub(super) gorin_hud_labels_path: Option<std::path::PathBuf>,
    pub(super) gorin_dance_intro_path: Option<std::path::PathBuf>,
    pub(super) gorin_gauge_labels_path: Option<std::path::PathBuf>,
    pub(super) gorin_home_run_path: Option<std::path::PathBuf>,
    pub(super) gorin_sprint_announcements_path: Option<std::path::PathBuf>,
    pub(super) gorin_dance_announcements_path: Option<std::path::PathBuf>,
    pub(super) gorin_dance_logo_path: Option<std::path::PathBuf>,
    pub(super) gorin_retry_path: Option<std::path::PathBuf>,
    pub(super) gorin_announcements_path: Option<std::path::PathBuf>,
    pub(super) main_title_path: Option<std::path::PathBuf>,
    pub(super) battle_announcements_path: Option<std::path::PathBuf>,
    pub(super) continue_schools_path: Option<std::path::PathBuf>,
    pub(super) gorin_selector_names_path: Option<std::path::PathBuf>,
}

pub(super) fn load_mode_descendant_assets(directory: &Path) -> Result<ModeDescendantAssets> {
    let manifest_path = directory.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ModeDescendantManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported mode-descendant manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.units.is_empty(),
        "mode-descendant manifest has no units"
    );
    validate_relative_path(&manifest.edit_command_sheets)?;
    validate_relative_path(&manifest.edit_technique_names)?;
    if let Some(path) = &manifest.battle_announcements {
        validate_relative_path(path)?;
        ensure!(
            manifest.training_menu.is_some(),
            "battle announcements require the shared CEFT3 composition"
        );
    }
    if let Some(path) = &manifest.gorin_hud_labels {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "small Gorin labels require the shared CEFT5 composition"
        );
    }
    if let Some(path) = &manifest.gorin_dance_logo {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "dance logo requires shared DANCE composition"
        );
    }
    if let Some(path) = &manifest.gorin_dance_announcements {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "dance announcements require shared DANCE composition"
        );
    }
    if let Some(path) = &manifest.gorin_sprint_announcements {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "sprint announcements require the shared SPRINT composition"
        );
    }
    if let Some(path) = &manifest.gorin_home_run {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "home-run lettering requires the shared CEFT5 composition"
        );
    }
    if let Some(path) = &manifest.gorin_gauge_labels {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "gauge labels require the shared CEFT5 composition"
        );
    }
    if let Some(path) = &manifest.gorin_dance_intro {
        validate_relative_path(path)?;
        ensure!(
            manifest.gorin_retry.is_some(),
            "dance intro requires the shared DANCE texture composition"
        );
    }
    if let Some(path) = &manifest.gorin_retry {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.gorin_announcements {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.main_title {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.training_menu {
        validate_relative_path(path)?;
    }
    if let Some(path) = &manifest.continue_schools {
        validate_relative_path(path)?;
    }

    if let Some(path) = &manifest.gorin_selector_names {
        validate_relative_path(path)?;
    }
    for path in &manifest.illustrated_panels {
        validate_relative_path(path)?;
    }
    let mut paths = BTreeSet::new();
    let mut entries = Vec::new();
    for relative_path in &manifest.units {
        validate_relative_path(relative_path)?;
        ensure!(
            paths.insert(relative_path.clone()),
            "duplicate mode-descendant unit {}",
            relative_path.display()
        );
        let unit: ModeDescendantUnit = read_json(&directory.join(relative_path))?;
        ensure!(
            unit.kind == UNIT_KIND,
            "unsupported mode-descendant unit kind in {}",
            relative_path.display()
        );
        ensure!(
            !unit.entries.is_empty(),
            "mode-descendant unit {} is empty",
            relative_path.display()
        );
        entries.extend(unit.entries);
    }
    validate_entries(&entries)?;
    Ok(ModeDescendantAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        illustrated_panel_paths: manifest
            .illustrated_panels
            .iter()
            .map(|path| directory.join(path))
            .collect(),
        entries,
        edit_command_sheets_path: directory.join(manifest.edit_command_sheets),
        edit_technique_names_path: directory.join(manifest.edit_technique_names),
        training_menu_path: manifest.training_menu.map(|path| directory.join(path)),
        gorin_retry_path: manifest.gorin_retry.map(|path| directory.join(path)),
        main_title_path: manifest.main_title.map(|path| directory.join(path)),
        gorin_announcements_path: manifest
            .gorin_announcements
            .map(|path| directory.join(path)),
        gorin_dance_intro_path: manifest.gorin_dance_intro.map(|path| directory.join(path)),
        gorin_gauge_labels_path: manifest.gorin_gauge_labels.map(|path| directory.join(path)),
        gorin_home_run_path: manifest.gorin_home_run.map(|path| directory.join(path)),
        gorin_dance_logo_path: manifest.gorin_dance_logo.map(|path| directory.join(path)),
        gorin_dance_announcements_path: manifest
            .gorin_dance_announcements
            .map(|path| directory.join(path)),
        gorin_sprint_announcements_path: manifest
            .gorin_sprint_announcements
            .map(|path| directory.join(path)),
        gorin_hud_labels_path: manifest.gorin_hud_labels.map(|path| directory.join(path)),
        battle_announcements_path: manifest
            .battle_announcements
            .map(|path| directory.join(path)),
        continue_schools_path: manifest.continue_schools.map(|path| directory.join(path)),
        gorin_selector_names_path: manifest
            .gorin_selector_names
            .map(|path| directory.join(path)),
    })
}

fn validate_entries(entries: &[ModeDescendantEntry]) -> Result<()> {
    ensure!(
        !entries.is_empty(),
        "mode-descendant assets have no entries"
    );
    let mut ids = BTreeSet::new();
    for entry in entries {
        if let Some(records) = &entry.records {
            ensure!(
                !records.is_empty()
                    && records.iter().collect::<BTreeSet<_>>().len() == records.len(),
                "mode-descendant entry {} requires unique, nonempty consumers",
                entry.id
            );
            let specs =
                super::catalog::fixed_record_specs_for_surfaces(&BTreeSet::from([entry.surface]));
            ensure!(
                !matches!(entry.placement, ModeDescendantPlacement::Dynamic)
                    && records
                        .iter()
                        .all(|record| specs.iter().any(|spec| spec.record == *record)),
                "mode-descendant entry {} targets a record outside its fixed surface",
                entry.id
            );
        }
        ensure!(
            ids.insert(entry.id.as_str()),
            "duplicate mode-descendant id {}",
            entry.id
        );
        ensure!(
            !entry.source_text.trim().is_empty(),
            "mode-descendant entry {} has no source text",
            entry.id
        );
        ensure!(
            !entry.korean_text.trim().is_empty(),
            "mode-descendant entry {} has no Korean text",
            entry.id
        );
        match &entry.placement {
            ModeDescendantPlacement::Fixed {
                bits_per_pixel,
                tim_offset,
                cell,
                source_region_sha256,
                palette_roles,
                vertical_shift_px,
                ..
            } => {
                ensure!(
                    entry.surface != ModeDescendantSurface::PracticalExamSharedUi,
                    "practical exam entry {} must use dynamic atlas placement",
                    entry.id
                );
                ensure!(
                    matches!(bits_per_pixel, 4 | 8),
                    "mode-descendant entry {} has unsupported TIM depth {}",
                    entry.id,
                    bits_per_pixel
                );
                if let Some(roles) = palette_roles {
                    ensure!(
                        *bits_per_pixel == 4,
                        "explicit fixed palette roles require 4-bpp pixels"
                    );
                    validate_fixed_palette_roles(roles)?;
                }
                ensure!(
                    *bits_per_pixel == 4 || *vertical_shift_px == 0,
                    "fixed vertical text shifts require 4-bpp pixels"
                );
                parse_hex_offset(tim_offset).with_context(|| {
                    format!(
                        "mode-descendant entry {} has an invalid TIM offset",
                        entry.id
                    )
                })?;
                ensure!(
                    cell.width > 0 && cell.height > 0,
                    "mode-descendant entry {} has an empty cell",
                    entry.id
                );
                ensure!(
                    source_region_sha256.len() == 64
                        && source_region_sha256
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit()),
                    "mode-descendant entry {} has an invalid source-region SHA-256",
                    entry.id
                );
            }
            ModeDescendantPlacement::Fixed4bppWithoutClut {
                tim_offset,
                cell,
                clear_index,
                outline_index,
                fill_index,
                source_region_sha256,
                ..
            } => {
                ensure!(
                    entry.surface == ModeDescendantSurface::EditSharedUi,
                    "CLUT-less fixed mode-descendant entry {} is outside EDIT shared UI",
                    entry.id
                );
                parse_hex_offset(tim_offset).with_context(|| {
                    format!(
                        "mode-descendant entry {} has an invalid TIM offset",
                        entry.id
                    )
                })?;
                ensure!(
                    cell.width > 0 && cell.height > 0,
                    "mode-descendant entry {} has an empty cell",
                    entry.id
                );
                ensure!(
                    *clear_index < 16
                        && *fill_index < 16
                        && outline_index.is_none_or(|index| index < 16)
                        && clear_index != fill_index
                        && outline_index
                            .is_none_or(|index| index != *clear_index && index != *fill_index),
                    "CLUT-less mode-descendant entry {} has invalid 4-bpp palette roles",
                    entry.id
                );
                ensure!(
                    source_region_sha256.len() == 64
                        && source_region_sha256
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit()),
                    "mode-descendant entry {} has an invalid source-region SHA-256",
                    entry.id
                );
            }
            ModeDescendantPlacement::Dynamic => ensure!(
                entry.surface == ModeDescendantSurface::PracticalExamSharedUi,
                "mode-descendant entry {} uses dynamic placement on a fixed surface",
                entry.id
            ),
        }
    }
    validate_edit_school_label_domain(entries)?;
    for surface in ModeDescendantSurface::ALL {
        let cells = entries
            .iter()
            .filter(|entry| entry.surface == surface)
            .filter_map(|entry| match &entry.placement {
                ModeDescendantPlacement::Fixed {
                    bits_per_pixel,
                    tim_offset,
                    cell,
                    ..
                } => Some((
                    entry.id.as_str(),
                    *bits_per_pixel,
                    parse_hex_offset(tim_offset).expect("validated TIM offset"),
                    *cell,
                )),
                ModeDescendantPlacement::Fixed4bppWithoutClut {
                    tim_offset, cell, ..
                } => Some((
                    entry.id.as_str(),
                    4,
                    parse_hex_offset(tim_offset).expect("validated TIM offset"),
                    *cell,
                )),
                ModeDescendantPlacement::Dynamic => None,
            })
            .collect::<Vec<_>>();
        for (index, (left_id, left_bpp, left_offset, left)) in cells.iter().enumerate() {
            for (right_id, right_bpp, right_offset, right) in &cells[index + 1..] {
                ensure!(
                    left_bpp != right_bpp
                        || left_offset != right_offset
                        || !cells_overlap(*left, *right),
                    "mode-descendant cells {left_id} and {right_id} overlap"
                );
            }
        }
    }
    Ok(())
}

fn validate_edit_school_label_domain(entries: &[ModeDescendantEntry]) -> Result<()> {
    if !entries
        .iter()
        .any(|entry| entry.surface == ModeDescendantSurface::EditSharedUi)
    {
        return Ok(());
    }
    let consumers = kanri_school_label_consumers();
    let school_entries = entries
        .iter()
        .filter(|entry| entry.font_role == super::model::ModeDescendantFontRole::EditSchoolLabel)
        .collect::<Vec<_>>();
    ensure!(
        school_entries.len() == consumers.len(),
        "EDIT school-label asset domain must cover all {} KANRI selectors",
        consumers.len()
    );
    for consumer in consumers {
        let entry = school_entries
            .iter()
            .find(|entry| entry.id == consumer.id)
            .with_context(|| {
                format!(
                    "EDIT school-label asset is missing KANRI selector {} ({})",
                    consumer.selector, consumer.id
                )
            })?;
        let placement_matches = matches!(
            &entry.placement,
            ModeDescendantPlacement::Fixed {
                bits_per_pixel: 4,
                tim_offset,
                cell,
                alignment: super::model::TextAlignment::Center,
                ..
            } if tim_offset == "0x0" && *cell == consumer.cell
        );
        ensure!(
            entry.surface == ModeDescendantSurface::EditSharedUi
                && entry.source_text == consumer.source_text
                && placement_matches,
            "EDIT school-label asset {} no longer matches KANRI selector {}",
            entry.id,
            consumer.selector
        );
    }
    Ok(())
}

pub(super) fn parse_hex_offset(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("TIM offset is not hexadecimal")?,
        16,
    )
    .context("invalid hexadecimal TIM offset")
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

pub(super) fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "mode-descendant asset path must be a normalized relative path: {}",
        path.display()
    );
    Ok(())
}

fn validate_fixed_palette_roles(roles: &super::model::FixedPaletteRoles) -> Result<()> {
    ensure!(
        roles.clear_index < 16
            && roles.fill_index < 16
            && roles.clear_index != roles.fill_index
            && roles.outline_index.is_none_or(|index| {
                index < 16 && index != roles.clear_index && index != roles.fill_index
            }),
        "invalid fixed 4-bpp palette roles"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_fixed_palette_roles_reject_invisible_or_out_of_palette_ink() {
        for (clear_index, outline_index, fill_index) in [
            (10, None, 10),
            (10, Some(10), 1),
            (10, Some(1), 1),
            (0, None, 16),
        ] {
            assert!(
                validate_fixed_palette_roles(&super::super::model::FixedPaletteRoles {
                    clear_index,
                    outline_index,
                    fill_index,
                })
                .is_err()
            );
        }
        assert!(
            validate_fixed_palette_roles(&super::super::model::FixedPaletteRoles {
                clear_index: 10,
                outline_index: None,
                fill_index: 1,
            })
            .is_ok()
        );
    }
}
