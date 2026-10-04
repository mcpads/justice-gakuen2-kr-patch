//! Compose selector loading and glyph materialization into one relocatable payload.
use super::selector_loader::{overlaps, ram_range};
use super::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectorRuntimeLayout {
    pub loader: SelectorNameLoaderLayout,
    pub scratch_address: u32,
    pub cache_x_words: u16,
    pub cache_y: u16,
    pub cache_slot_count: u16,
    pub clut_x_words: u16,
    pub clut_y: u16,
    pub bootstrap_origin: u32,
    pub bootstrap_capacity: usize,
    pub payload_source: u32,
    pub payload_capacity: usize,
    #[serde(default)]
    pub extra_payload_source: Option<u32>,
    #[serde(default)]
    pub extra_payload_capacity: usize,
    #[serde(default)]
    pub ascii_source: Option<u32>,
    #[serde(default)]
    pub data_copies: Vec<SelectorDataCopy>,
    pub enter_critical_address: u32,
    pub flush_cache_address: u32,
    pub exit_critical_address: u32,
}

#[derive(Clone, Debug)]
pub struct SelectorRuntimeProgram {
    pub resident_bytes: Vec<u8>,
    pub roster_source_hooks: Option<RosterSourceHooks>,
    pub bootstrap_bytes: Vec<u8>,
    pub materializer_address: u32,
    pub uploader_address: u32,
    pub sprite_address: u32,
    pub dispatch_address: u32,
    pub ascii_materializer_address: u32,
    pub ascii_storage_offset: Option<usize>,
    pub ascii_source_address: u32,
    pub ascii_bytes: Vec<u8>,
    pub glyph_rectangle: [usize; 4],
    pub bootstrap_layout: SelectorBootstrapLayout,
}

/// Emits only: source byte ownership and the native call-site hook belong to
/// the installer. Both staging ranges must be inside the selector texture load.
pub fn build_selector_runtime(
    layout: &SelectorRuntimeLayout,
    atlas: &NameInputRuntimeAtlasLayout,
    pack: &NameGlyphBandPack,
    ascii: &SelectorAsciiGlyphs,
    outline: &SharedNameOutlineRuntimeProgram,
) -> Result<SelectorRuntimeProgram> {
    build_runtime(layout, atlas, pack, ascii, outline, None)
}

/// Compose the same suppliers for all source-bound PLSEL2..5 roster loops.
/// The mode installer still owns physical storage and lifetime reservations.
pub fn build_roster_runtime(
    layout: &SelectorRuntimeLayout,
    atlas: &NameInputRuntimeAtlasLayout,
    pack: &NameGlyphBandPack,
    ascii: &SelectorAsciiGlyphs,
    outline: &SharedNameOutlineRuntimeProgram,
    number: u8,
    overlay: &[u8],
) -> Result<SelectorRuntimeProgram> {
    build_runtime(layout, atlas, pack, ascii, outline, Some((number, overlay)))
}

fn build_runtime(
    layout: &SelectorRuntimeLayout,
    atlas: &NameInputRuntimeAtlasLayout,
    pack: &NameGlyphBandPack,
    ascii: &SelectorAsciiGlyphs,
    outline: &SharedNameOutlineRuntimeProgram,
    roster: Option<(u8, &[u8])>,
) -> Result<SelectorRuntimeProgram> {
    ensure!(
        pack.bytes().len() <= NAME_GLYPH_PACK_STORAGE_BYTES,
        "selector band pack exceeds the loader's physical cell storage"
    );
    let loader_layout = &layout.loader;
    ensure!(
        loader_layout.restore_selector_service_table,
        "PLSEL1 runtime must restore the displaced service-table setup"
    );
    if let Some(copy) = loader_layout.font_copies.first() {
        ensure!(
            copy.byte_count == pack.bytes().len().next_multiple_of(4),
            "selector font copy does not cover the exact padded pack"
        );
        ensure!(
            layout.data_copies.is_empty() && layout.ascii_source.is_some(),
            "selector font copies require one explicit ASCII supplier"
        );
    }
    let loader = build_selector_name_loader(loader_layout, atlas)?;
    let remaining = layout
        .loader
        .byte_capacity
        .checked_sub(loader.bytes.len())
        .ok_or_else(|| anyhow::anyhow!("selector resident payload has no materializer capacity"))?;
    let materializer = build_selector_materializer(
        &SelectorMaterializerLayout {
            origin: loader_layout
                .origin
                .checked_add(u32::try_from(loader.bytes.len())?)
                .ok_or_else(|| anyhow::anyhow!("selector materializer address overflow"))?,
            byte_capacity: remaining,
            pack_address: loader_layout.pack_destination,
            scratch_address: layout.scratch_address,
        },
        pack,
        outline,
    )?;
    let mut resident_bytes = loader.bytes;
    resident_bytes.extend_from_slice(&materializer.bytes);
    let ascii_storage_offset = layout
        .ascii_source
        .is_none()
        .then(|| pack.bytes().len().next_multiple_of(4));
    let ascii_source_address = if let Some(source) = layout.ascii_source {
        let data = ram_range(source, ascii.bytes.len().next_multiple_of(4))?;
        let textures = ram_range(
            loader_layout.textures.destination,
            loader_layout.textures.decoded_byte_count,
        )?;
        if !loader_layout.font_copies.is_empty() {
            ensure!(
                loader_layout.font_copies.len() == 2,
                "selector font transport must contain pack and ASCII only"
            );
            let copy = &loader_layout.font_copies[1];
            ensure!(
                copy.destination == source
                    && copy.byte_count == ascii.bytes.len().next_multiple_of(4),
                "selector font ASCII copy does not cover exact padded supplier"
            );
        } else if layout.data_copies.is_empty() {
            ensure!(
                data.start >= textures.start && data.end <= textures.end,
                "selector ASCII is outside its resident texture load"
            );
        } else {
            let mut copies = layout.data_copies.iter().collect::<Vec<_>>();
            copies.sort_by_key(|copy| copy.destination);
            let mut cursor = source;
            for copy in copies {
                ensure!(
                    copy.destination == cursor,
                    "selector ASCII copies leave a gap or overlap"
                );
                let input = ram_range(copy.source, copy.byte_count)?;
                ensure!(
                    input.start >= textures.start && input.end <= textures.end,
                    "selector ASCII staging leaves texture resource"
                );
                cursor = ram_range(copy.destination, copy.byte_count)?.end;
            }
            ensure!(
                cursor == source + u32::try_from(ascii.bytes.len().next_multiple_of(4))?,
                "selector ASCII copies do not cover the complete padded supplier"
            );
        }
        for (address, capacity) in [
            (layout.bootstrap_origin, layout.bootstrap_capacity),
            (layout.payload_source, layout.payload_capacity),
        ]
        .into_iter()
        .chain(
            layout
                .extra_payload_source
                .map(|address| (address, layout.extra_payload_capacity)),
        ) {
            ensure!(
                !overlaps(
                    &data,
                    &super::selector_bootstrap::staging_ram_range(address, capacity)?
                ),
                "selector ASCII aliases staged code"
            );
        }
        source
    } else {
        ensure!(
            layout.data_copies.is_empty(),
            "selector data copies require an explicit ASCII destination"
        );
        let offset = ascii_storage_offset.unwrap();
        ensure!(
            offset + ascii.bytes.len() <= NAME_GLYPH_PACK_STORAGE_BYTES - 200,
            "selector ASCII storage overlaps the font coordinate table"
        );
        loader_layout.pack_destination + u32::try_from(offset)?
    };
    let ascii_materializer_address = loader_layout.origin + u32::try_from(resident_bytes.len())?;
    let ascii_code = ascii.build_render_program(
        ascii_materializer_address,
        loader_layout
            .byte_capacity
            .checked_sub(resident_bytes.len())
            .ok_or_else(|| anyhow::anyhow!("no ASCII code capacity"))?,
        ascii_source_address,
        layout.scratch_address,
        outline,
    )?;
    resident_bytes.extend_from_slice(&ascii_code);
    let [hx, hy, hw, hh] = materializer.glyph_rectangle;
    let [ax, ay, aw, ah] = ascii.crop;
    let x = hx.min(ax - 1);
    let y = hy.min(ay - 1);
    let glyph_rectangle = [
        x,
        y,
        (hx + hw).max(ax + aw + 1) - x,
        (hy + hh).max(ay + ah + 1) - y,
    ];
    let uploader_address = loader_layout
        .origin
        .checked_add(u32::try_from(resident_bytes.len())?)
        .ok_or_else(|| anyhow::anyhow!("selector uploader address overflow"))?;
    let upload_capacity = loader_layout
        .byte_capacity
        .checked_sub(resident_bytes.len())
        .ok_or_else(|| anyhow::anyhow!("selector payload has no uploader capacity"))?;
    let uploader = build_selector_upload(&SelectorUploadLayout {
        origin: uploader_address,
        byte_capacity: upload_capacity,
        scratch_address: layout.scratch_address,
        x_words: layout.cache_x_words,
        y: layout.cache_y,
        slot_count: layout.cache_slot_count,
    })?;
    resident_bytes.extend_from_slice(&uploader);
    let sprite_address = loader_layout
        .origin
        .checked_add(u32::try_from(resident_bytes.len())?)
        .ok_or_else(|| anyhow::anyhow!("selector sprite address overflow"))?;
    let sprite_capacity = loader_layout
        .byte_capacity
        .checked_sub(resident_bytes.len())
        .ok_or_else(|| anyhow::anyhow!("selector payload has no sprite capacity"))?;
    let sprite = build_selector_sprite(&SelectorSpriteLayout {
        origin: sprite_address,
        byte_capacity: sprite_capacity,
        x_words: layout.cache_x_words,
        y: layout.cache_y,
        slot_count: layout.cache_slot_count,
        glyph_rectangle,
        clut_x_words: layout.clut_x_words,
        clut_y: layout.clut_y,
    })?;
    resident_bytes.extend_from_slice(&sprite);
    ensure!(
        layout.cache_slot_count >= if roster.is_some() { 68 } else { 16 }
            && glyph_rectangle[2] <= SELECTOR_NAME_ADVANCE as usize,
        "selector dispatch needs all name cells and glyphs fitting the declared advance"
    );
    let dispatch_address = loader_layout
        .origin
        .checked_add(u32::try_from(resident_bytes.len())?)
        .ok_or_else(|| anyhow::anyhow!("selector dispatch address overflow"))?;
    let dispatch_capacity = loader_layout
        .byte_capacity
        .checked_sub(resident_bytes.len())
        .ok_or_else(|| anyhow::anyhow!("selector payload has no dispatch capacity"))?;
    let dispatch = if roster.is_some() {
        build_roster_glyph(&RosterGlyphLayout {
            origin: dispatch_address,
            capacity: dispatch_capacity,
            names: 0x801f5814,
            hangul: materializer.entry_address,
            ascii: ascii_materializer_address,
            uploader: uploader_address,
            sprite: sprite_address,
        })?
    } else {
        build_selector_name_dispatch(
            dispatch_address,
            dispatch_capacity,
            materializer.entry_address,
            ascii_materializer_address,
            uploader_address,
            sprite_address,
        )?
    };
    resident_bytes.extend_from_slice(&dispatch);
    let roster_source_hooks = if let Some((number, source)) = roster {
        let hooks = build_roster_source_hooks(
            number,
            source,
            loader_layout.origin + u32::try_from(resident_bytes.len())?,
            loader_layout
                .byte_capacity
                .checked_sub(resident_bytes.len())
                .ok_or_else(|| anyhow::anyhow!("roster adapters exceed resident reservation"))?,
            dispatch_address,
            layout.bootstrap_origin,
        )?;
        resident_bytes.extend_from_slice(&hooks.adapters);
        Some(hooks)
    } else {
        None
    };
    let payload_split = if resident_bytes.len() > layout.payload_capacity {
        ensure!(
            resident_bytes.len() - layout.payload_capacity <= layout.extra_payload_capacity,
            "selector resident payload exceeds both staging capacities"
        );
        Some(SelectorPayloadSplit {
            first_byte_count: layout.payload_capacity,
            second_source: layout
                .extra_payload_source
                .ok_or_else(|| anyhow::anyhow!("missing second staging source"))?,
        })
    } else {
        None
    };
    let bootstrap_layout = SelectorBootstrapLayout {
        origin: layout.bootstrap_origin,
        byte_capacity: layout.bootstrap_capacity,
        payload_source: layout.payload_source,
        payload_destination: loader_layout.origin,
        payload_byte_count: resident_bytes.len(),
        payload_split,
        enter_critical_address: layout.enter_critical_address,
        flush_cache_address: layout.flush_cache_address,
        exit_critical_address: layout.exit_critical_address,
    };
    let bootstrap_bytes =
        build_selector_bootstrap_with_data(&bootstrap_layout, &layout.data_copies)?;
    let scratch = ram_range(layout.scratch_address, 200)?;
    let copied_pack = ram_range(
        loader_layout.pack_destination,
        NAME_GLYPH_PACK_STORAGE_BYTES,
    )?;
    let resident = ram_range(loader_layout.origin, layout.loader.byte_capacity)?;
    let font = ram_range(
        loader_layout.font.destination,
        loader_layout.font.decoded_byte_count,
    )?;
    let textures = ram_range(
        loader_layout.textures.destination,
        loader_layout.textures.decoded_byte_count,
    )?;
    let bootstrap = super::selector_bootstrap::staging_ram_range(
        layout.bootstrap_origin,
        layout.bootstrap_capacity,
    )?;
    let staging = ram_range(layout.payload_source, layout.payload_capacity)?;
    let outline_range = ram_range(outline.outline_pixel_address, outline.bytes.len())?;
    for copy in layout
        .data_copies
        .iter()
        .chain(loader_layout.font_copies.iter().skip(1))
    {
        let target = ram_range(copy.destination, copy.byte_count)?;
        ensure!(
            [
                &scratch,
                &copied_pack,
                &resident,
                &font,
                &textures,
                &outline_range
            ]
            .iter()
            .all(|region| !overlaps(region, &target)),
            "relocated ASCII aliases a retained resource or native load"
        );
    }
    let services = [
        loader_layout.native_loader_address,
        layout.enter_critical_address,
        layout.flush_cache_address,
        layout.exit_critical_address,
    ];
    let extra_staging = layout
        .extra_payload_source
        .map(|address| ram_range(address, layout.extra_payload_capacity))
        .transpose()?;
    ensure!(
        layout.extra_payload_source.is_some() || layout.extra_payload_capacity == 0,
        "extra staging capacity has no source"
    );
    for region in [&bootstrap, &staging]
        .into_iter()
        .chain(extra_staging.iter())
    {
        ensure!(
            region.start >= textures.start && region.end <= textures.end,
            "selector staging is outside its decoded texture resource"
        );
    }
    ensure!(
        !overlaps(&bootstrap, &staging)
            && extra_staging
                .as_ref()
                .is_none_or(|range| !overlaps(range, &bootstrap) && !overlaps(range, &staging)),
        "selector staging reservations overlap"
    );
    ensure!(
        !overlaps(&scratch, &copied_pack)
            && !overlaps(&scratch, &resident)
            && !overlaps(&scratch, &font)
            && !overlaps(&scratch, &textures)
            && !overlaps(&outline_range, &font)
            && !overlaps(&outline_range, &textures)
            && !overlaps(&outline_range, &copied_pack)
            && !overlaps(&outline_range, &resident),
        "selector load or scratch would destroy a resident dependency"
    );
    for address in services {
        let service = ram_range(address, 16)?;
        for copy in layout
            .data_copies
            .iter()
            .chain(loader_layout.font_copies.iter())
        {
            ensure!(
                !overlaps(&service, &ram_range(copy.destination, copy.byte_count)?),
                "selector copied data overwrites a native service"
            );
        }
        ensure!(
            [&scratch, &copied_pack, &resident, &font, &textures]
                .iter()
                .all(|range| !overlaps(&service, range)),
            "selector load or allocation overwrites a native service"
        );
    }
    Ok(SelectorRuntimeProgram {
        resident_bytes,
        roster_source_hooks,
        bootstrap_bytes,
        materializer_address: materializer.entry_address,
        uploader_address,
        sprite_address,
        dispatch_address,
        ascii_materializer_address,
        ascii_storage_offset,
        ascii_source_address,
        ascii_bytes: ascii.bytes.clone(),
        glyph_rectangle,
        bootstrap_layout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::rasterize_menu_glyphs;
    use crate::name_input::runtime_test_machine::execute_with_callbacks;

    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn combined_payload_survives_staging_overwrite_and_protects_dependencies() {
        verify_selector_sources(None, None, false);
    }

    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn selector_ascii_survives_the_native_texture_reload() {
        verify_selector_sources(Some(0x8012_2c00), None, false);
    }

    #[test]
    #[ignore = "requires original source disc"]
    fn roster_payload_survives_loader_overwrites_for_all_four_overlays() {
        for number in 2..=5 {
            let source = crate::source_disc::test_source_disc()
                .read_record(&format!("DAT1/PLSEL{number}.BIN"))
                .unwrap()
                .1;
            verify_selector_sources(Some(0x8012_2c00), Some((number, &source)), false);
        }
    }

    #[test]
    #[ignore = "requires fonts in ../fonts/"]
    fn relocated_ascii_survives_both_native_resource_loads() {
        verify_selector_sources(Some(0x800b4c00), None, false);
    }

    #[test]
    #[ignore = "requires installed name font"]
    fn selector_renders_contiguous_font_suppliers_after_both_native_loads() {
        verify_selector_sources(Some(0x800b4c00), None, true);
    }

    fn verify_selector_sources(
        ascii_source: Option<u32>,
        roster: Option<(u8, &[u8])>,
        contiguous: bool,
    ) {
        let atlas = NameInputRuntimeAtlasLayout {
            font_atlas_row_bytes: 384,
            glyph_cell_width: 20,
            glyph_cell_height: 20,
            pack_storage_cell_base_byte_offsets: (0..95)
                .map(|i| ((i / 19) * 20 * 384 + (i % 19) * 10) as u16)
                .collect(),
            lookup_table_bytes: Vec::new(),
            cache_cell_base_byte_offsets: Vec::new(),
        };
        let atlas = NameInputRuntimeAtlasLayout {
            lookup_table_bytes: atlas
                .pack_storage_cell_base_byte_offsets
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            ..atlas
        };
        let font_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/galmuri/Galmuri11.ttf");
        let reference = rasterize_menu_glyphs(&font_path, "가각잠체력", 12.0, 3, 13).unwrap();
        let pack = NameGlyphBandPack::build(&reference, 19000).unwrap();
        let ascii_reference = rasterize_menu_glyphs(
            &font_path,
            &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
            12.0,
            3,
            13,
        )
        .unwrap();
        let ascii = SelectorAsciiGlyphs::build(&ascii_reference, 1860).unwrap();
        let outline = build_shared_name_outline_runtime_program().unwrap();
        let layout = SelectorRuntimeLayout {
            loader: SelectorNameLoaderLayout {
                origin: 0x800a8000,
                byte_capacity: 3008,
                native_loader_address: 0x80015414,
                font: NameAssetLoad {
                    catalog_index: 718,
                    destination: 0x800c0000,
                    decoded_byte_count: 210944,
                },
                textures: NameAssetLoad {
                    catalog_index: 53,
                    destination: 0x800d4000,
                    decoded_byte_count: 328192,
                },
                font_pixel_address: 0x800d92e0,
                pack_destination: 0x800b0000,
                font_copies: Vec::new(),
                restore_selector_service_table: true,
                restored_zero_ranges: Vec::new(),
            },
            scratch_address: 0x800b4a38,
            cache_x_words: 512,
            cache_y: 0,
            cache_slot_count: 16,
            clut_x_words: 0,
            clut_y: 482,
            bootstrap_origin: 0xa00dc340,
            bootstrap_capacity: 1216,
            payload_source: 0x800eb020,
            payload_capacity: 1536,
            extra_payload_source: Some(0x8010bc20),
            extra_payload_capacity: 992,
            ascii_source,
            data_copies: Vec::new(),
            enter_critical_address: 0x80060058,
            flush_cache_address: 0x80060008,
            exit_critical_address: 0x80060068,
        };
        let mut layout = layout;
        if ascii_source == Some(0x800b4c00) {
            let first = (ascii.bytes.len() / 2) & !3;
            layout.bootstrap_capacity = 0x1c0;
            layout.loader.restored_zero_ranges = vec![[0x50, 0x800]];
            layout.data_copies = vec![
                SelectorDataCopy {
                    source: 0x800d4050,
                    destination: 0x800b4c00,
                    byte_count: first,
                },
                SelectorDataCopy {
                    source: 0x800dc500,
                    destination: 0x800b4c00 + first as u32,
                    byte_count: ascii.bytes.len().next_multiple_of(4) - first,
                },
            ];
        }
        if contiguous {
            layout.data_copies.clear();
            layout.loader.font.decoded_byte_count = 229168;
            layout.loader.font_copies = vec![
                SelectorDataCopy {
                    source: 0x800f3800,
                    destination: layout.loader.pack_destination,
                    byte_count: pack.bytes().len().next_multiple_of(4),
                },
                SelectorDataCopy {
                    source: 0x800f7a2c,
                    destination: layout.ascii_source.unwrap(),
                    byte_count: ascii.bytes.len().next_multiple_of(4),
                },
            ];
        }
        if roster.is_some() {
            // Fixture reservations, not product placement claims.
            layout.loader.origin = 0x800b5000;
            layout.loader.byte_capacity = 4096;
            layout.payload_capacity = 4096;
            layout.cache_slot_count = 68;
        }
        if contiguous {
            let mut invalid = layout.clone();
            invalid.loader.font_copies[0].byte_count -= 4;
            assert!(build_selector_runtime(&invalid, &atlas, &pack, &ascii, &outline).is_err());
            let mut invalid = layout.clone();
            invalid.loader.font_copies[1].byte_count -= 4;
            assert!(build_selector_runtime(&invalid, &atlas, &pack, &ascii, &outline).is_err());
            let mut invalid = layout.clone();
            invalid.ascii_source = Some(layout.scratch_address);
            invalid.loader.font_copies[1].destination = layout.scratch_address;
            assert!(build_selector_runtime(&invalid, &atlas, &pack, &ascii, &outline).is_err());
            let mut invalid = layout.clone();
            invalid
                .data_copies
                .push(invalid.loader.font_copies[1].clone());
            assert!(build_selector_runtime(&invalid, &atlas, &pack, &ascii, &outline).is_err());
        }
        let p = if let Some((number, source)) = roster {
            build_roster_runtime(&layout, &atlas, &pack, &ascii, &outline, number, source).unwrap()
        } else {
            build_selector_runtime(&layout, &atlas, &pack, &ascii, &outline).unwrap()
        };
        let off = |a: u32| (a & 0x1fffffff) as usize;
        let mut memory = vec![0x55; 0x200000];
        memory[0x1f6360..0x1f6364].copy_from_slice(&0x80089a7cu32.to_le_bytes());
        memory[0x89bcc..0x89bd0].copy_from_slice(&0x80015fccu32.to_le_bytes());
        let source = off(layout.payload_source);
        let destination = off(layout.loader.origin);
        let first = layout.payload_capacity.min(p.resident_bytes.len());
        memory[source..source + first].copy_from_slice(&p.resident_bytes[..first]);
        if let Some(split) = &p.bootstrap_layout.payload_split {
            let extra = off(split.second_source);
            memory[extra..extra + p.resident_bytes.len() - first]
                .copy_from_slice(&p.resident_bytes[first..]);
        }
        if !layout.data_copies.is_empty() {
            let mut padded = ascii.bytes.clone();
            padded.resize(padded.len().next_multiple_of(4), 0);
            let mut offset = 0;
            for copy in &layout.data_copies {
                let start = off(copy.source);
                memory[start..start + copy.byte_count]
                    .copy_from_slice(&padded[offset..offset + copy.byte_count]);
                offset += copy.byte_count;
            }
        }
        let mut font = vec![0xa5; layout.loader.font.decoded_byte_count];
        let mut cells = vec![0; 19000];
        cells[..pack.bytes().len()].copy_from_slice(pack.bytes());
        if let Some(offset) = p.ascii_storage_offset {
            cells[offset..offset + ascii.bytes.len()].copy_from_slice(&ascii.bytes);
        }
        for (cell, &offset) in atlas.pack_storage_cell_base_byte_offsets.iter().enumerate() {
            for row in 0..20 {
                let at = (layout.loader.font_pixel_address - layout.loader.font.destination)
                    as usize
                    + usize::from(offset)
                    + row * 384;
                font[at..at + 10]
                    .copy_from_slice(&cells[cell * 200 + row * 10..cell * 200 + row * 10 + 10]);
            }
        }
        if contiguous {
            for (copy, bytes) in layout
                .loader
                .font_copies
                .iter()
                .zip([pack.bytes(), ascii.bytes.as_slice()])
            {
                let start = (copy.source - layout.loader.font.destination) as usize;
                font[start..start + copy.byte_count].fill(0);
                font[start..start + bytes.len()].copy_from_slice(bytes);
            }
        }
        let mut r = std::array::from_fn(|i| i as u32 * 37);
        r[0] = 0;
        r[29] = 0x801fe000;
        r[31] = SELECTOR_BOOTSTRAP_RETURN;
        let saved = r;
        let mut native_loads = 0;
        let mut services = Vec::new();
        execute_with_callbacks(
            &p.bootstrap_bytes,
            layout.bootstrap_origin,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, m| {
                if pc == layout.loader.origin {
                    let copied = m[destination..destination + p.resident_bytes.len()].to_vec();
                    assert_eq!(copied, p.resident_bytes);
                    assert_eq!(
                        services,
                        vec![
                            layout.enter_critical_address,
                            layout.flush_cache_address,
                            layout.exit_critical_address
                        ]
                    );
                    execute_with_callbacks(&copied, pc, r, m, None, &mut |pc, r, m| {
                        if pc != layout.loader.native_loader_address {
                            return false;
                        }
                        native_loads += 1;
                        if native_loads == 1 {
                            assert_eq!((r[4], r[5]), (layout.loader.font.destination, 718));
                            m[off(r[4])..off(r[4]) + font.len()].copy_from_slice(&font);
                            assert_ne!(&m[source..source + copied.len()], copied);
                        } else {
                            assert_eq!(native_loads, 2);
                            if !layout.data_copies.is_empty() || contiguous {
                                let start = off(layout.ascii_source.unwrap());
                                assert_eq!(&m[start..start + ascii.bytes.len()], &ascii.bytes);
                            }
                            assert_eq!((r[4], r[5]), (layout.loader.textures.destination, 53));
                            m[off(r[4])..off(r[4]) + layout.loader.textures.decoded_byte_count]
                                .fill(0x77);
                            if let Some(source) = layout.ascii_source
                                && layout.data_copies.is_empty()
                                && !contiguous
                            {
                                m[off(source)..off(source) + ascii.bytes.len()]
                                    .copy_from_slice(&ascii.bytes);
                            }
                        }
                        r[2] = 221538;
                        true
                    });
                    return true;
                }
                if [
                    layout.enter_critical_address,
                    layout.flush_cache_address,
                    layout.exit_critical_address,
                ]
                .contains(&pc)
                {
                    services.push(pc);
                    r[2] = 1;
                    return true;
                }
                false
            },
        );
        assert_eq!(native_loads, 2);
        assert_eq!((r[2], r[4]), (0x80015fcc, 0x800d0000));
        assert_eq!(
            &memory[destination..destination + p.resident_bytes.len()],
            p.resident_bytes
        );
        let retained = if contiguous {
            pack.bytes().len()
        } else {
            19000
        };
        assert_eq!(
            &memory[off(layout.loader.pack_destination)
                ..off(layout.loader.pack_destination) + retained],
            &cells[..retained]
        );
        for i in (16..24).chain([28, 29, 30, 31]) {
            assert_eq!(r[i], saved[i]);
        }
        // Execute the relocated materializer after both native loads destroyed staging.
        let origin = 0x80010000;
        let mut code = vec![0; (layout.loader.origin - origin) as usize + p.resident_bytes.len()];
        let mut jump = psx_r3000a::Assembler::new();
        jump.emit(psx_r3000a::Instruction::J {
            target: p.materializer_address,
        })
        .emit(psx_r3000a::Instruction::nop());
        code[..8].copy_from_slice(jump.assemble(origin).unwrap().bytes());
        let helper = (outline.outline_pixel_address - origin) as usize;
        code[helper..helper + outline.bytes.len()].copy_from_slice(&outline.bytes);
        code[(layout.loader.origin - origin) as usize..]
            .copy_from_slice(&memory[destination..destination + p.resident_bytes.len()]);
        r[4] = '가' as u32 - 0xac00;
        crate::name_input::runtime_test_machine::execute(&code, origin, &mut r, &mut memory);
        assert_eq!(r[2], layout.scratch_address);
        let glyph = reference
            .glyphs
            .iter()
            .find(|g| g.character == '가')
            .unwrap();
        let expected: Vec<_> = glyph
            .pixels
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| p[0] | p[1] << 4)
            .collect();
        assert_eq!(
            &memory[off(layout.scratch_address)..off(layout.scratch_address) + 200],
            expected
        );
        let mut jump = psx_r3000a::Assembler::new();
        jump.emit(psx_r3000a::Instruction::J {
            target: p.uploader_address,
        })
        .emit(psx_r3000a::Instruction::nop());
        code[..8].copy_from_slice(jump.assemble(origin).unwrap().bytes());
        r[4] = 15;
        let mut uploaded = false;
        execute_with_callbacks(&code, origin, &mut r, &mut memory, None, &mut |pc, r, m| {
            if pc == crate::contextual_texture_upload::VRAM_UPLOAD_ROUTINE_ADDRESS {
                assert_eq!(r[5], layout.scratch_address);
                assert_eq!(&m[off(r[5])..off(r[5]) + 200], expected);
                let at = off(r[4]);
                let rect: Vec<_> = m[at..at + 8]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|v| u16::from_le_bytes(*v))
                    .collect();
                assert_eq!(rect, [527, 20, 5, 20]);
                uploaded = true;
                r[2] = 0;
                return true;
            }
            if pc == crate::contextual_texture_upload::VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS {
                assert!(uploaded);
                assert_eq!(r[4], 0);
                return true;
            }
            false
        });
        assert!(uploaded);
        let mut jump = psx_r3000a::Assembler::new();
        jump.emit(psx_r3000a::Instruction::J {
            target: p.ascii_materializer_address,
        })
        .emit(psx_r3000a::Instruction::nop());
        code[..8].copy_from_slice(jump.assemble(origin).unwrap().bytes());
        for character in "Ag_:0 ".chars() {
            r[4] = character as u32;
            crate::name_input::runtime_test_machine::execute(&code, origin, &mut r, &mut memory);
            assert_eq!(r[2], layout.scratch_address);
            let pixels = ascii_reference
                .glyphs
                .iter()
                .find(|g| g.character == character)
                .map_or_else(|| vec![0; 400], |g| g.pixels.clone());
            let expected: Vec<_> = pixels
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| p[0] | p[1] << 4)
                .collect();
            assert_eq!(
                &memory[off(layout.scratch_address)..off(layout.scratch_address) + 200],
                expected
            );
        }
        for &[start, end] in &layout.loader.restored_zero_ranges {
            let base = off(layout.loader.textures.destination);
            assert!(memory[base + start..base + end].iter().all(|&b| b == 0));
            assert_eq!(memory[base + start - 1], 0x77);
            assert_eq!(memory[base + end], 0x77);
        }
        assert_eq!(p.glyph_rectangle, [3, 3, 13, 15]);
        // Previously independent constructors cannot detect these cross-component collisions.
        for bad in [
            SelectorRuntimeLayout {
                loader: SelectorNameLoaderLayout {
                    restored_zero_ranges: vec![[0x51, 0x800]],
                    ..layout.loader.clone()
                },
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                loader: SelectorNameLoaderLayout {
                    restored_zero_ranges: vec![[0, layout.loader.textures.decoded_byte_count + 4]],
                    ..layout.loader.clone()
                },
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                loader: SelectorNameLoaderLayout {
                    restored_zero_ranges: vec![[0x50, 0x800], [0x100, 0x200]],
                    ..layout.loader.clone()
                },
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                scratch_address: layout.loader.pack_destination + 18000,
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                scratch_address: layout.loader.font.destination,
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                flush_cache_address: layout.loader.font.destination,
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                payload_capacity: 32,
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                payload_source: 0x801f0000,
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                loader: SelectorNameLoaderLayout {
                    byte_capacity: 392,
                    ..layout.loader.clone()
                },
                ..layout.clone()
            },
            SelectorRuntimeLayout {
                payload_source: layout.bootstrap_origin,
                ..layout
            },
        ] {
            assert!(build_selector_runtime(&bad, &atlas, &pack, &ascii, &outline).is_err());
        }
    }
}
