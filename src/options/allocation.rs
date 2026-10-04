use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::menu_glyph_slots::{OPTIONS_SMALL_GLYPH_CODE_CANDIDATES, TITLE_MENU_GLYPH_CODES};
use crate::text::{SKIP_GLYPH_CODE, atlas_position};
use crate::tim::Cell;

use super::model::{OptionsAuthoredUnit, OptionsFontRole, OptionsFontStyles};
use super::model::{OptionsDevelopmentStatus, OptionsTranslationUnit};
use super::records_main::HEADING_GLYPHS as RECORDS_MAIN_HEADING_GLYPHS;

const HEADING_GLYPHS: [(char, u16); 7] = [
    ('옵', 0x03a0),
    ('션', 0x03a2),
    ('키', 0x03a4),
    ('설', 0x03a6),
    ('정', 0x03a8),
    ('게', 0x0386),
    ('임', 0x038a),
];
pub(super) const TRANSPARENT_ADVANCE_CODE: u16 = SKIP_GLYPH_CODE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlyphAllocation {
    pub(super) role: OptionsFontRole,
    pub(super) character: char,
    pub(super) code: u16,
    pub(super) cell: Cell,
}

pub(crate) struct OptionsGlyphAllocation {
    pub(super) by_role_and_character: BTreeMap<(OptionsFontRole, char), GlyphAllocation>,
    pub(super) global_glyphs: Vec<GlyphAllocation>,
    pub(super) records_contextual_glyphs: Vec<GlyphAllocation>,
    pub(super) provisional_physical_codes: Vec<u16>,
}

impl OptionsGlyphAllocation {
    pub(super) fn code_for(&self, role: OptionsFontRole, character: char) -> Result<u16> {
        self.by_role_and_character
            .get(&(role, character))
            .map(|allocation| allocation.code)
            .with_context(|| {
                format!(
                    "no options glyph allocation for {} {character:?}",
                    role.key()
                )
            })
    }
}

#[cfg(test)]
pub(super) fn allocate_options_glyphs(
    units: &[OptionsAuthoredUnit],
    styles: &OptionsFontStyles,
) -> Result<OptionsGlyphAllocation> {
    allocate_options_glyphs_preserving(units, styles, &BTreeSet::new())
}

pub(crate) struct OptionsGlyphRequests {
    requested: BTreeSet<(OptionsFontRole, char)>,
    heading_glyphs: Vec<(char, Vec<u16>)>,
    records_main_heading: BTreeSet<(OptionsFontRole, char)>,
    small_groups: Vec<Vec<(OptionsFontRole, char)>>,
    small_candidates: Vec<u16>,
}

pub(super) fn collect_options_glyph_requests(
    units: &[OptionsAuthoredUnit],
    styles: &OptionsFontStyles,
    preserved: &BTreeSet<u16>,
) -> Result<OptionsGlyphRequests> {
    let requested = units
        .iter()
        .flat_map(|unit| {
            unit.korean_text
                .chars()
                .filter(|character| needs_rasterized_glyph(*character))
                .map(move |character| (unit.font_role, character))
        })
        .collect::<BTreeSet<_>>();
    let heading = requested
        .iter()
        .filter(|(role, _)| *role == OptionsFontRole::Heading)
        .copied()
        .collect::<BTreeSet<_>>();
    let heading_glyphs = heading_candidates(units, &heading, preserved)?;
    let records_main_heading = requested
        .iter()
        .filter(|(role, _)| *role == OptionsFontRole::RecordsMainHeading)
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        records_main_heading.is_empty()
            || records_main_heading
                == RECORDS_MAIN_HEADING_GLYPHS
                    .iter()
                    .map(|(character, _)| (OptionsFontRole::RecordsMainHeading, *character))
                    .collect(),
        "records main heading glyph repertoire changed"
    );
    let small_requested = requested
        .iter()
        .filter(|(role, _)| {
            !matches!(
                *role,
                OptionsFontRole::Heading | OptionsFontRole::RecordsMainHeading
            )
        })
        .copied()
        .collect::<Vec<_>>();
    let mut small_groups = Vec::<Vec<(OptionsFontRole, char)>>::new();
    for requested_glyph in &small_requested {
        if let Some(group) = small_groups.iter_mut().find(|group| {
            group[0].1 == requested_glyph.1
                && same_raster_style(styles, group[0].0, requested_glyph.0)
        }) {
            group.push(*requested_glyph);
        } else {
            small_groups.push(vec![*requested_glyph]);
        }
    }
    let small_candidates = reclaimed_small_cells(units, &HEADING_GLYPHS, preserved)?;
    Ok(OptionsGlyphRequests {
        requested,
        heading_glyphs,
        records_main_heading,
        small_groups,
        small_candidates,
    })
}

impl OptionsGlyphRequests {
    pub(crate) fn requests(&self) -> Vec<crate::menu_atlas_plan::MenuAtlasRequest> {
        use crate::menu_atlas_plan::MenuAtlasRequest;
        let contexts = BTreeSet::from(["options_records".to_owned()]);
        let protected = super::contextual_glyphs::PRESERVED_SOURCE_GRAPHIC_CODES;
        let mut requests = self
            .small_groups
            .iter()
            .map(|group| {
                let records_only = group
                    .iter()
                    .all(|(role, _)| super::contextual_glyphs::is_records_role(*role));
                let mut candidates = self
                    .small_candidates
                    .iter()
                    .copied()
                    .filter(|code| records_only || !protected.contains(code))
                    .collect::<Vec<_>>();
                if records_only {
                    candidates.sort_by_key(|code| !protected.contains(code));
                }
                MenuAtlasRequest {
                    key: key(group[0].0, group[0].1),
                    contexts: contexts.clone(),
                    candidates,
                    width: 20,
                    height: 20,
                }
            })
            .collect::<Vec<_>>();
        for (character, candidates) in &self.heading_glyphs {
            requests.push(MenuAtlasRequest {
                key: key(OptionsFontRole::Heading, *character),
                contexts: contexts.clone(),
                candidates: candidates.clone(),
                width: 40,
                height: 40,
            });
        }
        if !self.records_main_heading.is_empty() {
            for (character, code) in RECORDS_MAIN_HEADING_GLYPHS {
                requests.push(MenuAtlasRequest {
                    key: key(OptionsFontRole::RecordsMainHeading, character),
                    contexts: contexts.clone(),
                    candidates: vec![code],
                    width: 40,
                    height: 40,
                });
            }
        }
        for character in '0'..='9' {
            requests.push(MenuAtlasRequest {
                key: format!("options:native_digit:{character}"),
                contexts: contexts.clone(),
                candidates: vec![
                    crate::text::fixed_menu_glyph_code(character).expect("native digit"),
                ],
                width: 20,
                height: 20,
            });
        }
        requests
    }

    pub(crate) fn resolve(
        &self,
        plan: &crate::menu_atlas_plan::MenuAtlasPlan,
    ) -> Result<OptionsGlyphAllocation> {
        let mut glyphs = Vec::new();
        let mut by_role_and_character = BTreeMap::new();
        for group in &self.small_groups {
            let (role, character) = group[0];
            let planned = plan.glyph(&key(role, character))?;
            let allocation = GlyphAllocation {
                role,
                character,
                code: planned.code,
                cell: planned.cell,
            };
            glyphs.push(allocation);
            for &alias in group {
                by_role_and_character.insert(alias, allocation);
            }
        }
        for (character, _) in &self.heading_glyphs {
            let character = *character;
            let planned = plan.glyph(&key(OptionsFontRole::Heading, character))?;
            let glyph = GlyphAllocation {
                role: OptionsFontRole::Heading,
                character,
                code: planned.code,
                cell: planned.cell,
            };
            glyphs.push(glyph);
            by_role_and_character.insert((glyph.role, character), glyph);
        }
        if !self.records_main_heading.is_empty() {
            for (character, _) in RECORDS_MAIN_HEADING_GLYPHS {
                let planned = plan.glyph(&key(OptionsFontRole::RecordsMainHeading, character))?;
                let glyph = GlyphAllocation {
                    role: OptionsFontRole::RecordsMainHeading,
                    character,
                    code: planned.code,
                    cell: planned.cell,
                };
                glyphs.push(glyph);
                by_role_and_character.insert((glyph.role, character), glyph);
            }
        }
        ensure!(
            by_role_and_character.len() == self.requested.len(),
            "central MENU plan omitted an Options role"
        );
        for character in '0'..='9' {
            let planned = plan.glyph(&format!("options:native_digit:{character}"))?;
            glyphs.push(GlyphAllocation {
                role: OptionsFontRole::Value,
                character,
                code: planned.code,
                cell: planned.cell,
            });
        }
        glyphs.sort_by_key(|glyph| (glyph.role, glyph.character));
        let mut records_contextual_glyphs = Vec::new();
        let mut provisional_physical_codes = Vec::new();
        for glyph in &glyphs {
            if glyph.cell.width == 40 {
                provisional_physical_codes.extend([
                    glyph.code,
                    glyph.code + 1,
                    glyph.code + 0x10,
                    glyph.code + 0x11,
                ]);
            } else {
                provisional_physical_codes.push(glyph.code);
            }
        }
        glyphs.retain(|glyph| {
            if super::contextual_glyphs::PRESERVED_SOURCE_GRAPHIC_CODES.contains(&glyph.code) {
                records_contextual_glyphs.push(*glyph);
                false
            } else {
                true
            }
        });
        ensure!(
            records_contextual_glyphs
                .iter()
                .all(|glyph| super::contextual_glyphs::is_records_role(glyph.role)),
            "central MENU plan overwrote protected graphics outside Records"
        );
        Ok(OptionsGlyphAllocation {
            by_role_and_character,
            global_glyphs: glyphs,
            records_contextual_glyphs,
            provisional_physical_codes,
        })
    }
}

fn key(role: OptionsFontRole, character: char) -> String {
    format!("options:{}:{:04x}", role.key(), character as u32)
}

#[cfg(test)]
pub(super) fn allocate_options_glyphs_preserving(
    units: &[OptionsAuthoredUnit],
    styles: &OptionsFontStyles,
    preserved: &BTreeSet<u16>,
) -> Result<OptionsGlyphAllocation> {
    let requests = collect_options_glyph_requests(units, styles, preserved)?;
    let plan = crate::menu_atlas_plan::MenuAtlasPlan::build(&requests.requests(), &[])?;
    requests.resolve(&plan)
}

fn source_cells(unit: &OptionsAuthoredUnit) -> Result<Vec<u16>> {
    unit.source_codes
        .iter()
        .filter(|code| code.as_str() != "0x0fff")
        .map(|code| crate::menu_atlas::parse_menu_code(code))
        .collect()
}

fn heading_candidates(
    units: &[OptionsAuthoredUnit],
    requested: &BTreeSet<(OptionsFontRole, char)>,
    preserved: &BTreeSet<u16>,
) -> Result<Vec<(char, Vec<u16>)>> {
    ensure!(
        HEADING_GLYPHS
            .iter()
            .all(|(c, _)| requested.contains(&(OptionsFontRole::Heading, *c))),
        "required Options heading characters missing"
    );
    let mut candidates = Vec::new();
    for unit in units
        .iter()
        .filter(|u| u.font_role == OptionsFontRole::Heading)
    {
        candidates.extend(source_cells(unit)?);
    }
    let owned = reclaimed_small_cells(units, &HEADING_GLYPHS, preserved)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    candidates.extend(owned.iter().copied().filter(|code| {
        [*code, *code + 1, *code + 0x10, *code + 0x11]
            .iter()
            .all(|part| owned.contains(part))
    }));
    candidates.retain(|code| {
        let Ok(p) = atlas_position(*code) else {
            return false;
        };
        p.x % 256 + 40 <= 256
            && p.y + 40 <= 256
            && preserved
                .iter()
                .chain(super::contextual_glyphs::PRESERVED_SOURCE_GRAPHIC_CODES.iter())
                .chain(TITLE_MENU_GLYPH_CODES.iter())
                .all(|occupied| {
                    !crate::menu_audit::wrapped_cells_overlap_sized(*code, 40, *occupied, 20)
                })
            && HEADING_GLYPHS
                .iter()
                .chain(RECORDS_MAIN_HEADING_GLYPHS.iter())
                .all(|(_, occupied)| {
                    !crate::menu_audit::wrapped_cells_overlap_sized(*code, 40, *occupied, 40)
                })
    });
    Ok(requested
        .iter()
        .map(|(_, character)| {
            let domain = HEADING_GLYPHS
                .iter()
                .find(|(c, _)| c == character)
                .map_or_else(|| candidates.clone(), |(_, code)| vec![*code]);
            (*character, domain)
        })
        .collect())
}

fn reclaimed_small_cells(
    units: &[OptionsAuthoredUnit],
    headings: &[(char, u16)],
    preserved: &BTreeSet<u16>,
) -> Result<Vec<u16>> {
    let mut candidates = OPTIONS_SMALL_GLYPH_CODE_CANDIDATES.to_vec();
    for unit in units {
        for code in source_cells(unit)? {
            // Page zero supplies fixed Latin characters, digits and punctuation.
            if code < 0x0100 {
                continue;
            }
            candidates.push(code);
            if matches!(
                unit.font_role,
                OptionsFontRole::Heading | OptionsFontRole::RecordsMainHeading
            ) {
                candidates.extend([code + 1, code + 0x10, code + 0x11]);
            }
        }
    }
    let mut selected = Vec::new();
    for code in candidates {
        if preserved.contains(&code) {
            continue;
        }
        let p = atlas_position(code)?;
        if p.x % 256 + 20 > 256 || p.y + 20 > 256 {
            continue;
        }
        if headings
            .iter()
            .chain(RECORDS_MAIN_HEADING_GLYPHS.iter())
            .any(|(_, occupied)| {
                crate::menu_audit::wrapped_cells_overlap_sized(code, 20, *occupied, 40)
            })
            || TITLE_MENU_GLYPH_CODES
                .iter()
                .any(|occupied| crate::menu_audit::wrapped_cells_overlap(code, *occupied))
            || selected
                .iter()
                .any(|occupied| crate::menu_audit::wrapped_cells_overlap(code, *occupied))
        {
            continue;
        }
        selected.push(code);
    }
    Ok(selected)
}

fn same_raster_style(
    styles: &OptionsFontStyles,
    left: OptionsFontRole,
    right: OptionsFontRole,
) -> bool {
    let left = font_style(styles, left);
    let right = font_style(styles, right);
    left.font == right.font && left.font_px.to_bits() == right.font_px.to_bits()
}

fn font_style(
    styles: &OptionsFontStyles,
    role: OptionsFontRole,
) -> &super::model::OptionsFontStyle {
    match role {
        OptionsFontRole::Heading => &styles.heading,
        OptionsFontRole::Help => &styles.help,
        OptionsFontRole::Label => &styles.label,
        OptionsFontRole::Value => &styles.value,
        OptionsFontRole::Action => &styles.action,
        OptionsFontRole::RecordsMainHeading => &styles.records_main.heading,
        OptionsFontRole::RecordsMainItem => &styles.records_main.item,
        OptionsFontRole::RecordsPrompt => &styles.records_prompt,
        OptionsFontRole::RecordsStatusHeading => &styles.records_status.heading,
        OptionsFontRole::RecordsStatusMessage => &styles.records_status.message,
    }
}

/// Physical cells retained for still-untranslated source text. Build this once
/// so reservation and the later overwrite guard use the same parsed ownership.
pub(super) struct UntranslatedGlyphCells<'a> {
    conflicts: BTreeMap<(u16, u16), BTreeSet<&'a str>>,
}

impl<'a> UntranslatedGlyphCells<'a> {
    pub(super) fn from_units(
        units: &'a [OptionsTranslationUnit],
        large_text_source_offsets: &[usize],
    ) -> Result<Self> {
        let mut source_owners = BTreeMap::<(u16, usize), BTreeSet<&str>>::new();
        for unit in units
            .iter()
            .filter(|unit| unit.development_status == OptionsDevelopmentStatus::Untranslated)
        {
            let source_offset =
                super::assets::parse_hex_usize(&unit.source_offset, "source offset")?;
            let size = if large_text_source_offsets.contains(&source_offset) {
                40
            } else {
                20
            };
            for code in &unit.source_codes {
                let code = super::assets::parse_hex_u16(code, "source code")? & 0x0fff;
                if code != TRANSPARENT_ADVANCE_CODE {
                    source_owners
                        .entry((code, size))
                        .or_default()
                        .insert(&unit.id);
                }
            }
        }
        let mut conflicts = BTreeMap::<(u16, u16), BTreeSet<&str>>::new();
        for ((source, size), owners) in source_owners {
            for candidate in 0..crate::menu_atlas::MENU_GLYPH_CODE_COUNT as u16 {
                if crate::menu_audit::wrapped_cells_overlap_sized(candidate, 20, source, size) {
                    conflicts
                        .entry((candidate, source))
                        .or_default()
                        .extend(&owners);
                }
            }
        }
        Ok(Self { conflicts })
    }

    pub(super) fn preserved_codes(&self) -> BTreeSet<u16> {
        self.conflicts
            .keys()
            .map(|(candidate, _)| *candidate)
            .collect()
    }

    pub(super) fn conflicts_for(&self, provisional_codes: &[u16]) -> Vec<String> {
        self.conflicts
            .iter()
            .filter(|((candidate, _), _)| provisional_codes.contains(candidate))
            .map(|((provisional, source), unit_ids)| {
                format!(
                    "0x{provisional:04x} overlaps source 0x{source:04x} used by {}",
                    unit_ids.iter().copied().collect::<Vec<_>>().join(",")
                )
            })
            .collect()
    }
}

fn needs_rasterized_glyph(character: char) -> bool {
    character != ' '
}
