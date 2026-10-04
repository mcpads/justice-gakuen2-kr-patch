use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::name_input::{
    NAME_GLYPH_CODE_COUNT, NAME_GLYPH_CODE_START, NameInputKeyboardPlan, NameNavigationMap,
    build_sparse_name_navigation,
};
use crate::pipeline::sha256_bytes;

use super::name_entry::{
    OVERLAY_SHA256, PADDING_CODE, PAGE_SPECS, SELECTABLE_CODE_SEQUENCE_OFFSET,
};
use super::name_entry_input::{NAME_NAVIGATION_SOURCE_SPECS, load_name_navigation_source_maps};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct KoreanNameKeyboardOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) navigation_maps: Vec<NameNavigationMap>,
    pub(super) active_positions_by_page: Vec<BTreeSet<u8>>,
    pub(super) report: KoreanNameKeyboardOverlayReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KoreanNameKeyboardOverlayReport {
    pub kind: String,
    pub active_key_count: usize,
    pub cache_code_count: usize,
    pub pack_storage_cell_count: usize,
    pub code_lookup_entry_count: usize,
    pub code_lookup_sha256: String,
    pub navigation_map_ids: Vec<String>,
    pub navigation_map_sha256: Vec<String>,
    pub pages: Vec<KoreanNameKeyboardPageReport>,
    pub runtime_navigation_consumer_installed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KoreanNameKeyboardPageReport {
    pub source_page: String,
    pub physical_cell_count: usize,
    pub source_padding_cell_count: usize,
    pub active_key_count: usize,
    pub inactive_navigation_position_count: usize,
    pub redirected_navigation_edge_count: usize,
}

pub(super) fn patch_korean_name_keyboard_overlay(
    source: &[u8],
    keyboard: &NameInputKeyboardPlan,
) -> Result<KoreanNameKeyboardOverlay> {
    ensure!(
        keyboard.source_overlay_sha256 == OVERLAY_SHA256,
        "Korean name keyboard targets a different source overlay"
    );
    ensure!(
        keyboard.pages.len() == PAGE_SPECS.len(),
        "Korean name keyboard page count changed"
    );

    let active_sequence_positions = keyboard
        .pages
        .iter()
        .scan(0_usize, |page_start, page| {
            let start = *page_start;
            *page_start += page.source_selectable_capacity;
            Some((start..start + page.active_key_count).collect::<BTreeSet<_>>())
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let cache_sequence_positions = keyboard
        .cache
        .slots
        .iter()
        .map(|slot| slot.selectable_sequence_position)
        .collect::<BTreeSet<_>>();
    let pack_sequence_positions = keyboard
        .glyph_pack_storage
        .cells
        .iter()
        .map(|cell| cell.selectable_sequence_position)
        .collect::<BTreeSet<_>>();
    ensure!(
        active_sequence_positions.is_disjoint(&cache_sequence_positions)
            && active_sequence_positions.is_disjoint(&pack_sequence_positions)
            && cache_sequence_positions.is_disjoint(&pack_sequence_positions),
        "Korean name keyboard code ownership overlaps"
    );
    ensure!(
        active_sequence_positions.len()
            + cache_sequence_positions.len()
            + pack_sequence_positions.len()
            == NAME_GLYPH_CODE_COUNT,
        "Korean name keyboard does not partition the source code lookup"
    );

    let mut patched = source.to_vec();
    let mut active_positions_by_page = Vec::with_capacity(PAGE_SPECS.len());
    let mut pages = Vec::with_capacity(PAGE_SPECS.len());
    let mut page_start = 0_usize;
    for ((page_name, page_offset, physical_cell_count, source_padding_cell_count), page) in
        PAGE_SPECS.into_iter().zip(&keyboard.pages)
    {
        ensure!(
            page.source_page == page_name,
            "Korean name keyboard source page order changed"
        );
        let mut source_selectable_position = 0_usize;
        let mut active_physical_positions = BTreeSet::new();
        let mut observed_source_padding = 0_usize;
        for physical_position in 0..physical_cell_count {
            let offset = page_offset + physical_position * 2;
            let source_code = read_u16(source, offset)?;
            if source_code == PADDING_CODE {
                observed_source_padding += 1;
                continue;
            }
            let sequence_position = page_start + source_selectable_position;
            let replacement = if source_selectable_position < page.active_key_count {
                active_physical_positions.insert(u8::try_from(physical_position)?);
                NAME_GLYPH_CODE_START + u16::try_from(sequence_position)?
            } else {
                PADDING_CODE
            };
            write_u16(&mut patched, offset, replacement)?;
            source_selectable_position += 1;
        }
        ensure!(
            observed_source_padding == source_padding_cell_count
                && source_selectable_position == page.source_selectable_capacity,
            "Korean name keyboard source page geometry changed"
        );
        pages.push(KoreanNameKeyboardPageReport {
            source_page: page_name.to_string(),
            physical_cell_count,
            source_padding_cell_count,
            active_key_count: active_physical_positions.len(),
            inactive_navigation_position_count: 90 - active_physical_positions.len(),
            redirected_navigation_edge_count: 0,
        });
        active_positions_by_page.push(active_physical_positions);
        page_start += page.source_selectable_capacity;
    }
    ensure!(
        page_start == NAME_GLYPH_CODE_COUNT,
        "Korean name keyboard code lookup length changed"
    );

    let source_navigation_maps = load_name_navigation_source_maps(source)?;
    let mut navigation_maps = Vec::with_capacity(NAME_NAVIGATION_SOURCE_SPECS.len());
    for (spec, source_navigation) in source_navigation_maps {
        let navigation = build_sparse_name_navigation(
            source_navigation,
            &active_positions_by_page[spec.source_page_index],
        )?;
        patched[spec.file_offset..spec.file_offset + navigation.bytes.len()]
            .copy_from_slice(&navigation.bytes);
        pages[spec.source_page_index].redirected_navigation_edge_count +=
            navigation.redirected_edge_count;
        navigation_maps.push(navigation);
    }

    for sequence_position in 0..NAME_GLYPH_CODE_COUNT {
        let code = if active_sequence_positions.contains(&sequence_position)
            || cache_sequence_positions.contains(&sequence_position)
        {
            NAME_GLYPH_CODE_START + u16::try_from(sequence_position)?
        } else {
            ensure!(
                pack_sequence_positions.contains(&sequence_position),
                "Korean name keyboard lookup entry has no owner"
            );
            PADDING_CODE
        };
        write_u16(
            &mut patched,
            SELECTABLE_CODE_SEQUENCE_OFFSET + sequence_position * 2,
            code,
        )?;
    }
    let lookup = patched
        .get(
            SELECTABLE_CODE_SEQUENCE_OFFSET
                ..SELECTABLE_CODE_SEQUENCE_OFFSET + NAME_GLYPH_CODE_COUNT * 2,
        )
        .context("Korean name keyboard lookup is truncated")?;

    Ok(KoreanNameKeyboardOverlay {
        report: KoreanNameKeyboardOverlayReport {
            kind: "Justice Gakuen 2 Korean name keyboard overlay".to_string(),
            active_key_count: active_sequence_positions.len(),
            cache_code_count: cache_sequence_positions.len(),
            pack_storage_cell_count: pack_sequence_positions.len(),
            code_lookup_entry_count: NAME_GLYPH_CODE_COUNT,
            code_lookup_sha256: sha256_bytes(lookup),
            navigation_map_ids: NAME_NAVIGATION_SOURCE_SPECS
                .iter()
                .map(|spec| spec.id.to_string())
                .collect(),
            navigation_map_sha256: navigation_maps
                .iter()
                .map(|navigation| sha256_bytes(&navigation.bytes))
                .collect(),
            pages,
            runtime_navigation_consumer_installed: true,
        },
        bytes: patched,
        navigation_maps,
        active_positions_by_page,
    })
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .context("Korean name keyboard source u16 is truncated")?;
    Ok(u16::from_le_bytes(bytes.try_into()?))
}

fn write_u16(data: &mut [u8], offset: usize, value: u16) -> Result<()> {
    let destination = data
        .get_mut(offset..offset + 2)
        .context("Korean name keyboard destination u16 is truncated")?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
