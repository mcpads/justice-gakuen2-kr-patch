//! KANRI/PASS expose one entry-owned glyph pool. Pair geometry and name-cache
//! capacity are requests to the shared planner, never post-allocation swaps.
use super::build::{KanriDisplayAllocation, text_and_characters};
use super::model::KanriDisplayAllocationEvidence;
use crate::menu_atlas_plan::MenuGlyphAllocation;
use crate::menu_atlas_plan::{MenuAtlasPlan, MenuAtlasRequest, MenuAtlasReservation};
use crate::name_input::{
    DIGIT_KEYS, KANRI_DISPLAY_CODE_CAPACITY, KANRI_INITIAL_GLYPH_CAPACITY,
    KANRI_NAME_CACHE_SLOT_COUNT, LATIN_KEYS, SYMBOL_KEYS,
};
use crate::source_disc::SupportedSourceDisc;
use crate::text::atlas_position;
use crate::tim::Cell;
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(super) struct EditGlyphRequests {
    requested: BTreeSet<char>,
    groups: Vec<Vec<char>>,
    source_codes: BTreeSet<u16>,
    fixed_codes: BTreeSet<u16>,
    password_codes: BTreeMap<char, u16>,
}

pub(crate) fn collect_menu_requests(
    root: &Path,
    source: &SupportedSourceDisc,
) -> Result<(Vec<MenuAtlasRequest>, Vec<MenuAtlasReservation>)> {
    let source = super::source::load_source(source)?;
    let assets = super::assets::load_assets(root, &source)?;
    let password = super::password_alphabet::PasswordAlphabet::load(root)?;
    let requests = collect_edit_requests(&assets, &source, &password)?;
    Ok((requests.requests(), requests.reservations()))
}

pub(super) fn collect_edit_requests(
    assets: &super::assets::LoadedEditRuntimeTextAssets,
    source: &super::source::EditRuntimeTextSource,
    password: &super::password_alphabet::PasswordAlphabet,
) -> Result<EditGlyphRequests> {
    let (text, requested) = text_and_characters(assets)?;
    ensure!(
        requested.len() <= KANRI_INITIAL_GLYPH_CAPACITY,
        "EDIT requests {} initial glyphs, exceeding the KANRI capacity of {KANRI_INITIAL_GLYPH_CAPACITY}",
        requested.len()
    );
    let groups = super::pass_title_renderer::glyph_groups(&text["pass_password_title"])?;
    ensure!(
        groups
            .iter()
            .flatten()
            .all(|character| requested.contains(character)),
        "PASS title contains an unrequested glyph"
    );
    let mut source_codes = crate::menu_audit::kanri_source_menu_codes(&source.overlay)?;
    source_codes
        .extend(super::pass_fixed_presentation::preserved_pass_consumer_codes(&source.pass)?);
    ensure!(
        source_codes.iter().all(|code| *code < 0x400),
        "KANRI source consumer escaped the shared MENU atlas"
    );
    let fixed_codes = LATIN_KEYS
        .chars()
        .chain(DIGIT_KEYS.chars())
        .chain(SYMBOL_KEYS.chars())
        .chain("()".chars())
        .filter_map(|character| {
            crate::text::common_menu_ascii_glyph_code(character)
                .or_else(|| crate::text::fixed_menu_glyph_code(character))
        })
        .filter(|code| *code < 0x400)
        .collect();
    let password_codes = password
        .characters()
        .map(|character| {
            Ok((
                character,
                super::password_alphabet::PasswordAlphabet::glyph_code(character)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(EditGlyphRequests {
        requested,
        groups,
        source_codes,
        fixed_codes,
        password_codes,
    })
}

fn candidates(width: usize) -> Vec<u16> {
    (0u16..4)
        .flat_map(|page| {
            (0u16..12)
                .flat_map(move |row| (0u16..12).map(move |col| (page << 8) | (row << 4) | col))
        })
        .filter(|code| usize::from(code & 15) * 20 + width <= 240)
        .collect()
}
fn key(character: char) -> String {
    format!("edit:static:{:04x}", character as u32)
}
fn cache_key(index: usize) -> String {
    format!("edit:cache:{index:03}")
}
fn pair_key(index: usize) -> String {
    format!("edit:title:{index:03}")
}
fn contexts() -> BTreeSet<String> {
    BTreeSet::from(["kanri_pass".to_owned()])
}

impl EditGlyphRequests {
    fn requests(&self) -> Vec<MenuAtlasRequest> {
        let paired = self
            .groups
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut result = self
            .requested
            .iter()
            .filter(|c| !paired.contains(c))
            .map(|character| MenuAtlasRequest {
                key: key(*character),
                contexts: contexts(),
                candidates: candidates(20),
                width: 20,
                height: 20,
            })
            .collect::<Vec<_>>();
        for (index, group) in self.groups.iter().enumerate() {
            let width = group.len() * 20;
            result.push(MenuAtlasRequest {
                key: pair_key(index),
                contexts: contexts(),
                candidates: candidates(width),
                width,
                height: 20,
            });
        }
        for index in 0..KANRI_NAME_CACHE_SLOT_COUNT {
            result.push(MenuAtlasRequest {
                key: cache_key(index),
                contexts: contexts(),
                candidates: candidates(20),
                width: 20,
                height: 20,
            });
        }
        result.extend(
            self.password_codes
                .iter()
                .map(|(character, code)| MenuAtlasRequest {
                    key: password_key(*character),
                    contexts: contexts(),
                    candidates: vec![*code],
                    width: 20,
                    height: 20,
                }),
        );
        result
    }
    fn reservations(&self) -> Vec<MenuAtlasReservation> {
        self.source_codes
            .union(&self.fixed_codes)
            .map(|code| MenuAtlasReservation {
                owner: format!("KANRI/PASS source glyph 0x{code:04x}"),
                replacement_keys: self
                    .password_codes
                    .iter()
                    .filter(|(_, native)| {
                        crate::menu_audit::wrapped_cells_overlap_sized(*code, 20, **native, 20)
                    })
                    .map(|(character, _)| password_key(*character))
                    .collect(),
                contexts: contexts(),
                code: *code,
                width: 20,
                height: 20,
            })
            .collect()
    }
    pub(super) fn resolve(&self, plan: &MenuAtlasPlan) -> Result<KanriDisplayAllocation> {
        let mut static_glyphs = BTreeMap::new();
        for (index, group) in self.groups.iter().enumerate() {
            let first = plan.glyph(&pair_key(index))?.code;
            for (offset, character) in group.iter().enumerate() {
                insert(
                    &mut static_glyphs,
                    *character,
                    first + u16::try_from(offset)?,
                )?;
            }
        }
        for character in &self.requested {
            if !static_glyphs.contains_key(character) {
                insert(
                    &mut static_glyphs,
                    *character,
                    plan.glyph(&key(*character))?.code,
                )?;
            }
        }
        let dynamic_display_codes = (0..KANRI_NAME_CACHE_SLOT_COUNT)
            .map(|index| Ok(plan.glyph(&cache_key(index))?.code))
            .collect::<Result<Vec<_>>>()?;
        let codes = static_glyphs
            .values()
            .map(|g| g.code)
            .chain(dynamic_display_codes.iter().copied())
            .collect::<Vec<_>>();
        let overlap = crate::menu_audit::wrapped_cells_overlap_sized;
        let pairwise_disjoint = codes
            .iter()
            .enumerate()
            .all(|(i, c)| codes[i + 1..].iter().all(|d| !overlap(*c, 20, *d, 20)));
        let disjoint_from_source_consumers = codes
            .iter()
            .all(|c| self.source_codes.iter().all(|d| !overlap(*c, 20, *d, 20)));
        let disjoint_from_fixed_menu_codes = codes
            .iter()
            .all(|c| self.fixed_codes.iter().all(|d| !overlap(*c, 20, *d, 20)));
        ensure!(
            codes.len() == self.requested.len() + KANRI_NAME_CACHE_SLOT_COUNT
                && codes.len() <= KANRI_DISPLAY_CODE_CAPACITY
                && pairwise_disjoint
                && disjoint_from_source_consumers
                && disjoint_from_fixed_menu_codes,
            "central KANRI plan violates source ownership or cache capacity"
        );
        Ok(KanriDisplayAllocation {
            static_glyphs,
            dynamic_display_codes,
            evidence: KanriDisplayAllocationEvidence {
                source_consumer_code_count: self.source_codes.len(),
                fixed_menu_code_count: self.fixed_codes.len(),
                allocated_display_code_count: codes.len(),
                pairwise_disjoint,
                disjoint_from_source_consumers,
                disjoint_from_fixed_menu_codes,
            },
        })
    }
}
fn insert(map: &mut BTreeMap<char, MenuGlyphAllocation>, character: char, code: u16) -> Result<()> {
    let p = atlas_position(code)?;
    ensure!(
        map.insert(
            character,
            MenuGlyphAllocation {
                character,
                code,
                cell: Cell {
                    x: p.x,
                    y: p.y,
                    width: 20,
                    height: 20
                },
                reused: false
            }
        )
        .is_none(),
        "duplicate planned KANRI character"
    );
    Ok(())
}

#[cfg(test)]
pub(crate) fn validate_menu_plan(
    root: &Path,
    source: &SupportedSourceDisc,
    plan: &MenuAtlasPlan,
) -> Result<()> {
    let source = super::source::load_source(source)?;
    let assets = super::assets::load_assets(root, &source)?;
    let password = super::password_alphabet::PasswordAlphabet::load(root)?;
    let requests = collect_edit_requests(&assets, &source, &password)?;
    let allocation = requests.resolve(plan)?;
    ensure!(
        allocation.dynamic_display_codes.len() == KANRI_NAME_CACHE_SLOT_COUNT,
        "name cache lost capacity"
    );
    for group in &requests.groups {
        for pair in group.windows(2) {
            ensure!(
                allocation.static_glyphs[&pair[1]].code
                    == allocation.static_glyphs[&pair[0]].code + 1,
                "PASS title pair lost native adjacency"
            );
        }
    }
    Ok(())
}

pub(super) fn password_key(character: char) -> String {
    format!("edit:password:{:04x}", character as u32)
}
