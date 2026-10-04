//! Allocate source-audited glyph requests before any consumer emits bytes.
//! Contexts describe simultaneous texture ownership, not build order. Two
//! entry-uploaded overlays may reuse pixels; requests live in the same context
//! may not. Source-specific adapters supply domains, never final placements.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::menu_atlas::logical_regions_overlap;
use crate::text::atlas_position;
use crate::tim::Cell;

#[path = "menu_atlas_plan/development.rs"]
mod development;
pub(crate) use development::prepare_development_plan;

#[derive(Debug, Clone)]
pub(crate) struct MenuAtlasRequest {
    pub key: String,
    pub contexts: BTreeSet<String>,
    pub candidates: Vec<u16>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MenuAtlasReservation {
    pub owner: String,
    /// Exact fixed consumers allowed to redraw this native source footprint.
    /// Dynamic requests still see the complete reservation, including aliases.
    pub replacement_keys: BTreeSet<String>,
    pub contexts: BTreeSet<String>,
    pub code: u16,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlannedMenuGlyph {
    pub code: u16,
    pub cell: Cell,
    pub contexts: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MenuAtlasPlan {
    allocations: BTreeMap<String, PlannedMenuGlyph>,
    source_reservations: Vec<MenuAtlasReservation>,
}

impl MenuAtlasPlan {
    pub fn build(
        requests: &[MenuAtlasRequest],
        reservations: &[MenuAtlasReservation],
    ) -> Result<Self> {
        let mut requests = requests.to_vec();
        let mut keys = BTreeSet::new();
        for reservation in reservations {
            ensure!(
                reservation.code < 0x400 && reservation.width > 0 && reservation.height > 0,
                "invalid source MENU reservation"
            );
            ensure!(
                !reservation.owner.is_empty() && !reservation.contexts.is_empty(),
                "MENU reservation requires an owner and live context"
            );
        }
        for request in &mut requests {
            ensure!(
                !request.key.is_empty() && keys.insert(request.key.clone()),
                "duplicate or empty MENU request key {:?}",
                request.key
            );
            ensure!(
                !request.contexts.is_empty(),
                "MENU request {} has no live context",
                request.key
            );
            let mut seen = BTreeSet::new();
            request.candidates.retain(|code| seen.insert(*code));
            for code in &request.candidates {
                validate_cell(*code, request.width, request.height)?;
            }
            request.candidates.retain(|code| {
                reservations.iter().all(|reserved| {
                    request.contexts.is_disjoint(&reserved.contexts)
                        || reserved.replacement_keys.contains(&request.key)
                        || !logical_regions_overlap(
                            *code,
                            request.width,
                            request.height,
                            reserved.code,
                            reserved.width,
                            reserved.height,
                        )
                })
            });
            ensure!(
                !request.candidates.is_empty(),
                "no source-audited MENU cell remains for {} in {:?}",
                request.key,
                request.contexts
            );
        }
        for reserved in reservations {
            ensure!(
                reserved
                    .replacement_keys
                    .iter()
                    .all(|key| requests.iter().any(|request| {
                        &request.key == key
                            && request.candidates.len() == 1
                            && !request.contexts.is_disjoint(&reserved.contexts)
                    })),
                "source replacement must name a fixed consumer in its live context"
            );
        }
        // Restrictive domains first, stable identity breaks ties. Neither source
        // file traversal nor parallel build completion controls placement.
        requests.sort_by(|a, b| {
            a.candidates
                .len()
                .cmp(&b.candidates.len())
                .then((b.width * b.height).cmp(&(a.width * a.height)))
                .then(a.key.cmp(&b.key))
        });
        let mut assigned = vec![None; requests.len()];
        for index in 0..requests.len() {
            ensure!(
                place(index, &requests, &mut assigned, &mut BTreeSet::new()),
                "MENU atlas capacity cannot satisfy {} in {:?}",
                requests[index].key,
                requests[index].contexts
            );
        }
        let mut allocations = BTreeMap::new();
        for (request, code) in requests.iter().zip(assigned) {
            let code = code.context("central MENU plan omitted a requested glyph")?;
            let position = atlas_position(code)?;
            allocations.insert(
                request.key.clone(),
                PlannedMenuGlyph {
                    code,
                    cell: Cell {
                        x: position.x,
                        y: position.y,
                        width: request.width,
                        height: request.height,
                    },
                    contexts: request.contexts.clone(),
                },
            );
        }
        let plan = Self {
            allocations,
            source_reservations: reservations.to_vec(),
        };
        plan.validate()?;
        Ok(plan)
    }

    pub fn glyph(&self, key: &str) -> Result<&PlannedMenuGlyph> {
        self.allocations
            .get(key)
            .with_context(|| format!("unplanned MENU glyph {key:?}"))
    }

    fn validate(&self) -> Result<()> {
        let rows = self.allocations.iter().collect::<Vec<_>>();
        for (index, (key, glyph)) in rows.iter().enumerate() {
            for (other_key, other) in &rows[index + 1..] {
                ensure!(
                    glyph.contexts.is_disjoint(&other.contexts)
                        || !logical_regions_overlap(
                            glyph.code,
                            glyph.cell.width,
                            glyph.cell.height,
                            other.code,
                            other.cell.width,
                            other.cell.height
                        ),
                    "central MENU plan overlaps {key} and {other_key}"
                );
            }
        }
        Ok(())
    }
}

fn validate_cell(code: u16, width: usize, height: usize) -> Result<()> {
    ensure!(
        matches!(width, 20 | 40) && matches!(height, 20 | 40),
        "unsupported MENU glyph footprint"
    );
    let p = atlas_position(code)?;
    ensure!(
        code < 0x400 && p.x % 256 + width <= 256 && p.y + height <= 256,
        "MENU cell 0x{code:04x}/{width}x{height} escapes its native texture page"
    );
    Ok(())
}

fn place(
    index: usize,
    requests: &[MenuAtlasRequest],
    assigned: &mut Vec<Option<u16>>,
    locked: &mut BTreeSet<usize>,
) -> bool {
    if !locked.insert(index) {
        return false;
    }
    let request = &requests[index];
    // Prefer unused space before moving an existing assignment. Most domains
    // are sparse, so this avoids a relocation chain for every new character.
    if let Some(code) = request.candidates.iter().copied().find(|code| {
        assigned.iter().enumerate().all(|(other, placed)| {
            other == index
                || placed.is_none_or(|placed| {
                    request.contexts.is_disjoint(&requests[other].contexts)
                        || !logical_regions_overlap(
                            *code,
                            request.width,
                            request.height,
                            placed,
                            requests[other].width,
                            requests[other].height,
                        )
                })
        })
    }) {
        assigned[index] = Some(code);
        return true;
    }
    for &code in &request.candidates {
        let conflicts = assigned
            .iter()
            .enumerate()
            .filter_map(|(other, assigned_code)| {
                let assigned_code = (*assigned_code)?;
                (other != index
                    && !request.contexts.is_disjoint(&requests[other].contexts)
                    && logical_regions_overlap(
                        code,
                        request.width,
                        request.height,
                        assigned_code,
                        requests[other].width,
                        requests[other].height,
                    ))
                .then_some(other)
            })
            .collect::<Vec<_>>();
        if conflicts.iter().any(|other| locked.contains(other)) {
            continue;
        }
        let before = assigned.clone();
        assigned[index] = Some(code);
        for &other in &conflicts {
            assigned[other] = None;
        }
        if conflicts
            .into_iter()
            .all(|other| place(other, requests, assigned, locked))
        {
            return true;
        }
        *assigned = before;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(key: &str, context: &str, candidates: &[u16]) -> MenuAtlasRequest {
        MenuAtlasRequest {
            key: key.into(),
            contexts: BTreeSet::from([context.into()]),
            candidates: candidates.to_vec(),
            width: 20,
            height: 20,
        }
    }
    #[test]
    fn unrelated_notice_text_does_not_reallocate_options() {
        let options = request("options:A", "options", &[0x301, 0x302]);
        let base = MenuAtlasPlan::build(std::slice::from_ref(&options), &[]).unwrap();
        let combined =
            MenuAtlasPlan::build(&[options, request("notice:C", "title", &[0x301])], &[]).unwrap();
        assert_eq!(
            base.glyph("options:A").unwrap().code,
            combined.glyph("options:A").unwrap().code
        );
        assert_eq!(combined.glyph("notice:C").unwrap().code, 0x301);
    }
    #[test]
    fn constrained_requests_displace_flexible_requests_without_collision() {
        let requests = vec![
            request("a", "title", &[0x301, 0x302]),
            request("b", "title", &[0x301, 0x303]),
            request("c", "title", &[0x301, 0x303]),
        ];
        let plan = MenuAtlasPlan::build(&requests, &[]).unwrap();
        assert_eq!(plan.glyph("a").unwrap().code, 0x302);
        assert_ne!(plan.glyph("b").unwrap().code, plan.glyph("c").unwrap().code);
        let reversed =
            MenuAtlasPlan::build(&requests.into_iter().rev().collect::<Vec<_>>(), &[]).unwrap();
        assert_eq!(
            serde_json::to_value(plan).unwrap(),
            serde_json::to_value(reversed).unwrap()
        );
    }
    #[test]
    fn source_reservations_protect_every_pixel_of_large_glyphs() {
        let reserved = MenuAtlasReservation {
            owner: "native icon".into(),
            replacement_keys: BTreeSet::new(),
            contexts: BTreeSet::from(["records".into()]),
            code: 0x300,
            width: 40,
            height: 40,
        };
        let plan =
            MenuAtlasPlan::build(&[request("label", "records", &[0x311, 0x302])], &[reserved])
                .unwrap();
        assert_eq!(plan.glyph("label").unwrap().code, 0x302);
    }
    #[test]
    fn rectangular_pairs_and_large_headings_reserve_all_covered_cells() {
        let mut heading = request("heading", "menu", &[0x300]);
        heading.width = 40;
        heading.height = 40;
        let mut pair = request("pair", "menu", &[0x310, 0x302]);
        pair.width = 40;
        let plan = MenuAtlasPlan::build(
            &[heading, pair, request("digit", "menu", &[0x303, 0x312])],
            &[],
        )
        .unwrap();
        assert_eq!(plan.glyph("pair").unwrap().code, 0x302);
        assert_eq!(plan.glyph("digit").unwrap().code, 0x312);
    }

    #[test]
    fn wrapped_native_codes_and_multi_context_owners_are_protected() {
        let reservation = MenuAtlasReservation {
            owner: "native wrapped glyph".into(),
            replacement_keys: BTreeSet::new(),
            contexts: BTreeSet::from(["title".into()]),
            code: 0x30c,
            width: 20,
            height: 20,
        };
        let mut shared = request("shared", "options", &[0x300, 0x302]);
        shared.contexts.insert("title".into());
        let plan = MenuAtlasPlan::build(&[shared], &[reservation]).unwrap();
        assert_eq!(plan.glyph("shared").unwrap().code, 0x302);
    }

    #[test]
    fn fixed_source_replacement_does_not_release_native_aliases_to_dynamic_text() {
        let reservation = MenuAtlasReservation {
            owner: "native digit and wrapped alias".into(),
            replacement_keys: BTreeSet::from(["digit".into()]),
            contexts: BTreeSet::from(["edit".into()]),
            code: 0x30c,
            width: 20,
            height: 20,
        };
        let plan = MenuAtlasPlan::build(
            &[
                request("digit", "edit", &[0x300]),
                request("name", "edit", &[0x30b, 0x302]),
            ],
            &[reservation],
        )
        .unwrap();
        assert_eq!(plan.glyph("digit").unwrap().code, 0x300);
        assert_eq!(plan.glyph("name").unwrap().code, 0x30b);
        // The wrapped region is still unavailable to an unnamed writer.
        let blocked = MenuAtlasReservation {
            owner: "alias".into(),
            replacement_keys: BTreeSet::new(),
            contexts: BTreeSet::from(["edit".into()]),
            code: 0x30c,
            width: 20,
            height: 20,
        };
        assert!(MenuAtlasPlan::build(&[request("name", "edit", &[0x300])], &[blocked]).is_err());
    }

    #[test]
    fn malformed_and_duplicate_requests_fail_before_placement() {
        let item = request("same", "title", &[0x300]);
        assert!(MenuAtlasPlan::build(&[item.clone(), item], &[]).is_err());
        let mut invalid = request("edge", "title", &[0x30b]);
        invalid.width = 40;
        assert!(MenuAtlasPlan::build(&[invalid], &[]).is_err());
    }

    #[test]
    fn insufficient_capacity_fails_before_any_builder_can_emit_bytes() {
        assert!(
            MenuAtlasPlan::build(
                &[
                    request("a", "title", &[0x301]),
                    request("b", "title", &[0x301])
                ],
                &[]
            )
            .is_err()
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuGlyphAllocation {
    pub(crate) character: char,
    pub(crate) code: u16,
    pub(crate) cell: Cell,
    pub(crate) reused: bool,
}
