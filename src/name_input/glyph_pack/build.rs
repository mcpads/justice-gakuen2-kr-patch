use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::font::{RasterizedMenuGlyph, rasterize_menu_glyphs};

use super::decode::NameGlyphPackDecoder;
use super::format::{
    BASE_KEY_COUNT, BASE_MEMBERSHIP_BYTES, FILL_PALETTE_INDEX, FINAL_COUNT, FINAL_KEY_COUNT,
    FINAL_MEMBERSHIP_BYTES, HEADER_BYTES, INITIAL_COUNT, MEDIAL_COUNT, MODERN_HANGUL_COUNT,
    OUTLINE_PALETTE_INDEX, PackHeader, REPERTOIRE_MEMBERSHIP_BYTES, has_membership,
    membership_count, rank_prefix_u8, rank_prefix_u16, set_membership,
};
use super::{MODERN_HANGUL_START, NameGlyphPack, NameGlyphPackReport};
use crate::name_input::repertoire::load_ks_x_1001_hangul;

const CELL_WIDTH: usize = 20;
const CELL_HEIGHT: usize = 20;

#[derive(Clone, Debug)]
struct SyllableRaster {
    index: usize,
    initial: usize,
    medial: usize,
    final_consonant: usize,
    pixels: Vec<u8>,
}

pub fn build_name_glyph_pack(
    font_path: &Path,
    font_px: f32,
    storage_capacity_bytes: usize,
) -> Result<NameGlyphPack> {
    let characters = load_ks_x_1001_hangul()?;
    let rasterized = rasterize_menu_glyphs(
        font_path,
        &characters.iter().collect::<String>(),
        font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    ensure!(
        rasterized.glyphs.len() == characters.len(),
        "name glyph raster population changed"
    );
    let glyphs = rasterized
        .glyphs
        .into_iter()
        .map(syllable_raster)
        .collect::<Result<Vec<_>>>()?;
    let crop = fill_bounds(&glyphs)?;
    let occupied_coordinates = occupied_coordinates(&glyphs, crop);
    let bytes_per_mask = occupied_coordinates.len().div_ceil(8);

    let mut repertoire_membership = vec![0_u8; REPERTOIRE_MEMBERSHIP_BYTES];
    let mut no_final_membership = vec![0_u8; BASE_MEMBERSHIP_BYTES];
    let mut final_bearing_membership = vec![0_u8; BASE_MEMBERSHIP_BYTES];
    let mut final_membership = vec![0_u8; FINAL_MEMBERSHIP_BYTES];
    for glyph in &glyphs {
        set_membership(&mut repertoire_membership, glyph.index);
        let base_key = glyph.initial * MEDIAL_COUNT + glyph.medial;
        if glyph.final_consonant == 0 {
            set_membership(&mut no_final_membership, base_key);
        } else {
            set_membership(&mut final_bearing_membership, base_key);
            set_membership(
                &mut final_membership,
                glyph.medial * (FINAL_COUNT - 1) + glyph.final_consonant - 1,
            );
        }
    }

    let no_final_masks = exact_no_final_masks(
        &glyphs,
        &no_final_membership,
        crop,
        &occupied_coordinates,
        bytes_per_mask,
    )?;
    let FactoredFinalMasks {
        base_masks: final_bearing_masks,
        final_masks,
    } = factor_final_bearing_masks(
        &glyphs,
        &final_bearing_membership,
        &final_membership,
        crop,
        &occupied_coordinates,
        bytes_per_mask,
    )?;
    let component_count = no_final_masks.len() + final_bearing_masks.len() + final_masks.len();
    let no_final_rank_prefix = rank_prefix_u16(&no_final_membership);
    let final_bearing_rank_prefix = rank_prefix_u16(&final_bearing_membership);
    let final_rank_prefix = rank_prefix_u8(&final_membership);
    let coordinate_membership_bytes = (crop[2] * crop[3]).div_ceil(8);
    let total_bytes = HEADER_BYTES
        + coordinate_membership_bytes
        + repertoire_membership.len()
        + no_final_membership.len()
        + final_bearing_membership.len()
        + final_membership.len()
        + no_final_rank_prefix.len()
        + final_bearing_rank_prefix.len()
        + final_rank_prefix.len()
        + component_count * bytes_per_mask;
    ensure!(
        total_bytes <= storage_capacity_bytes,
        "name glyph pack requires {total_bytes} bytes but owns {storage_capacity_bytes}"
    );
    let header = PackHeader {
        crop_x: crop[0],
        crop_y: crop[1],
        crop_width: crop[2],
        crop_height: crop[3],
        occupied_coordinate_count: occupied_coordinates.len(),
        bytes_per_mask,
        supported_syllable_count: glyphs.len(),
        no_final_base_count: no_final_masks.len(),
        final_bearing_base_count: final_bearing_masks.len(),
        final_component_count: final_masks.len(),
        total_bytes,
    };
    let mut bytes = Vec::with_capacity(total_bytes);
    bytes.extend(header.encode()?);
    bytes.extend(coordinate_membership(crop, &occupied_coordinates));
    bytes.extend(&repertoire_membership);
    bytes.extend(&no_final_membership);
    bytes.extend(&final_bearing_membership);
    bytes.extend(&final_membership);
    let no_final_rank_prefix_start = bytes.len();
    bytes.extend(&no_final_rank_prefix);
    let final_bearing_rank_prefix_start = bytes.len();
    bytes.extend(&final_bearing_rank_prefix);
    let final_rank_prefix_start = bytes.len();
    bytes.extend(&final_rank_prefix);
    let component_mask_start = bytes.len();
    for mask in no_final_masks
        .iter()
        .chain(&final_bearing_masks)
        .chain(&final_masks)
    {
        bytes.extend(mask);
    }
    ensure!(bytes.len() == total_bytes, "name glyph pack size changed");

    let decoder = NameGlyphPackDecoder::new(&bytes)?;
    let mut exact_reference_glyph_count = 0_usize;
    let mut missing_fill_pixel_count = 0_usize;
    let mut unexpected_fill_pixel_count = 0_usize;
    let mut synthesized_blank_count = 0_usize;
    for glyph in &glyphs {
        let character = char::from_u32(MODERN_HANGUL_START + u32::try_from(glyph.index)?)
            .context("name glyph index is not Unicode")?;
        let synthesized = decoder.render(character)?;
        exact_reference_glyph_count += usize::from(synthesized == glyph.pixels);
        synthesized_blank_count +=
            usize::from(synthesized.iter().all(|pixel| *pixel != FILL_PALETTE_INDEX));
        for (&reference, &actual) in glyph.pixels.iter().zip(&synthesized) {
            missing_fill_pixel_count +=
                usize::from(reference == FILL_PALETTE_INDEX && actual != FILL_PALETTE_INDEX);
            unexpected_fill_pixel_count +=
                usize::from(reference != FILL_PALETTE_INDEX && actual == FILL_PALETTE_INDEX);
        }
    }
    ensure!(
        synthesized_blank_count == 0,
        "name glyph pack synthesized a blank supported syllable"
    );

    let runtime_coordinate_list = encode_occupied_coordinates(crop, &occupied_coordinates)?;
    Ok(NameGlyphPack {
        bytes,
        runtime_coordinate_list,
        report: NameGlyphPackReport {
            kind: "Justice Gakuen 2 deterministic name glyph component pack".to_string(),
            font_name: rasterized.font_name,
            font_sha256: rasterized.font_sha256,
            font_px,
            repertoire: "KS X 1001 Hangul".to_string(),
            supported_syllable_count: glyphs.len(),
            unsupported_modern_syllable_count: MODERN_HANGUL_COUNT - glyphs.len(),
            crop: [crop[0], crop[1], crop[2], crop[3]],
            occupied_coordinate_count: occupied_coordinates.len(),
            bytes_per_component_mask: bytes_per_mask,
            no_final_base_component_count: no_final_masks.len(),
            final_bearing_base_component_count: final_bearing_masks.len(),
            final_component_count: final_masks.len(),
            component_count,
            no_final_rank_prefix_entry_bytes: 2,
            final_rank_prefix_entry_bytes: 1,
            no_final_rank_prefix_byte_range: [
                no_final_rank_prefix_start,
                final_bearing_rank_prefix_start,
            ],
            final_bearing_rank_prefix_byte_range: [
                final_bearing_rank_prefix_start,
                final_rank_prefix_start,
            ],
            final_rank_prefix_byte_range: [final_rank_prefix_start, component_mask_start],
            runtime_coordinate_list_byte_count: occupied_coordinates.len(),
            component_mask_byte_range: [component_mask_start, total_bytes],
            pack_bytes: total_bytes,
            storage_capacity_bytes,
            storage_bytes_remaining: storage_capacity_bytes - total_bytes,
            fits_storage: true,
            exact_reference_glyph_count,
            missing_fill_pixel_count,
            unexpected_fill_pixel_count,
            synthesized_blank_count,
            runtime_consumer_installed: false,
        },
    })
}

fn encode_occupied_coordinates(
    crop: [usize; 4],
    occupied_coordinates: &[usize],
) -> Result<Vec<u8>> {
    ensure!(
        crop[2] <= 16 && crop[3] <= 16,
        "name glyph runtime coordinate list exceeds one-byte entries"
    );
    occupied_coordinates
        .iter()
        .copied()
        .map(|coordinate| {
            let x = coordinate % crop[2];
            let y = coordinate / crop[2];
            Ok(u8::try_from((y << 4) | x)?)
        })
        .collect()
}

fn syllable_raster(glyph: RasterizedMenuGlyph) -> Result<SyllableRaster> {
    let scalar = u32::from(glyph.character);
    ensure!(
        scalar >= MODERN_HANGUL_START,
        "name glyph repertoire contains a non-Hangul character"
    );
    let index = usize::try_from(scalar - MODERN_HANGUL_START)?;
    ensure!(
        index < MODERN_HANGUL_COUNT,
        "name glyph is outside modern Hangul"
    );
    Ok(SyllableRaster {
        index,
        initial: index / (MEDIAL_COUNT * FINAL_COUNT),
        medial: index / FINAL_COUNT % MEDIAL_COUNT,
        final_consonant: index % FINAL_COUNT,
        pixels: glyph.pixels,
    })
}

fn fill_bounds(glyphs: &[SyllableRaster]) -> Result<[usize; 4]> {
    let mut min_x = CELL_WIDTH;
    let mut min_y = CELL_HEIGHT;
    let mut max_x = 0_usize;
    let mut max_y = 0_usize;
    for glyph in glyphs {
        for (index, pixel) in glyph.pixels.iter().enumerate() {
            if *pixel != FILL_PALETTE_INDEX {
                continue;
            }
            min_x = min_x.min(index % CELL_WIDTH);
            min_y = min_y.min(index / CELL_WIDTH);
            max_x = max_x.max(index % CELL_WIDTH);
            max_y = max_y.max(index / CELL_WIDTH);
        }
    }
    ensure!(
        min_x <= max_x && min_y <= max_y,
        "name glyph repertoire is blank"
    );
    Ok([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1])
}

fn occupied_coordinates(glyphs: &[SyllableRaster], crop: [usize; 4]) -> Vec<usize> {
    let mut occupied = vec![false; crop[2] * crop[3]];
    for glyph in glyphs {
        for y in 0..crop[3] {
            for x in 0..crop[2] {
                if glyph.pixels[(crop[1] + y) * CELL_WIDTH + crop[0] + x] == FILL_PALETTE_INDEX {
                    occupied[y * crop[2] + x] = true;
                }
            }
        }
    }
    occupied
        .into_iter()
        .enumerate()
        .filter_map(|(coordinate, occupied)| occupied.then_some(coordinate))
        .collect()
}

fn coordinate_membership(crop: [usize; 4], occupied_coordinates: &[usize]) -> Vec<u8> {
    let mut membership = vec![0_u8; (crop[2] * crop[3]).div_ceil(8)];
    for coordinate in occupied_coordinates {
        set_membership(&mut membership, *coordinate);
    }
    membership
}

fn exact_no_final_masks(
    glyphs: &[SyllableRaster],
    membership: &[u8],
    crop: [usize; 4],
    occupied_coordinates: &[usize],
    bytes_per_mask: usize,
) -> Result<Vec<Vec<u8>>> {
    let mut masks = vec![None; BASE_KEY_COUNT];
    for glyph in glyphs.iter().filter(|glyph| glyph.final_consonant == 0) {
        let key = glyph.initial * MEDIAL_COUNT + glyph.medial;
        ensure!(masks[key].is_none(), "duplicate no-final name glyph base");
        masks[key] = Some(pack_fill_mask(
            &glyph.pixels,
            crop,
            occupied_coordinates,
            bytes_per_mask,
        ));
    }
    ensure!(
        masks.iter().flatten().count() == membership_count(membership),
        "no-final name glyph membership disagrees with its masks"
    );
    Ok(masks.into_iter().flatten().collect())
}

struct FactoredFinalMasks {
    base_masks: Vec<Vec<u8>>,
    final_masks: Vec<Vec<u8>>,
}

fn factor_final_bearing_masks(
    glyphs: &[SyllableRaster],
    base_membership: &[u8],
    final_membership: &[u8],
    crop: [usize; 4],
    occupied_coordinates: &[usize],
    bytes_per_mask: usize,
) -> Result<FactoredFinalMasks> {
    let mut bases = vec![vec![0_u8; bytes_per_mask]; BASE_KEY_COUNT];
    let mut finals = vec![vec![0_u8; bytes_per_mask]; FINAL_KEY_COUNT];
    for medial in 0..MEDIAL_COUNT {
        let row_keys = (0..INITIAL_COUNT)
            .map(|initial| initial * MEDIAL_COUNT + medial)
            .filter(|key| has_membership(base_membership, *key))
            .collect::<Vec<_>>();
        let column_keys = (1..FINAL_COUNT)
            .map(|final_consonant| medial * (FINAL_COUNT - 1) + final_consonant - 1)
            .filter(|key| has_membership(final_membership, *key))
            .collect::<Vec<_>>();
        for bit in 0..occupied_coordinates.len() {
            let observations = glyphs
                .iter()
                .filter(|glyph| glyph.medial == medial && glyph.final_consonant != 0)
                .map(|glyph| {
                    let row_key = glyph.initial * MEDIAL_COUNT + medial;
                    let column_key = medial * (FINAL_COUNT - 1) + glyph.final_consonant - 1;
                    let row = row_keys
                        .binary_search(&row_key)
                        .expect("observed name glyph base lost its membership");
                    let column = column_keys
                        .binary_search(&column_key)
                        .expect("observed name glyph final lost its membership");
                    let coordinate = occupied_coordinates[bit];
                    let x = crop[0] + coordinate % crop[2];
                    let y = crop[1] + coordinate / crop[2];
                    let expected = glyph.pixels[y * CELL_WIDTH + x] == FILL_PALETTE_INDEX;
                    (row, column, expected)
                })
                .collect::<Vec<_>>();
            let (row_bits, column_bits) =
                factor_or_matrix(row_keys.len(), column_keys.len(), &observations);
            for (row, enabled) in row_bits.into_iter().enumerate() {
                if enabled {
                    bases[row_keys[row]][bit / 8] |= 1 << (bit % 8);
                }
            }
            for (column, enabled) in column_bits.into_iter().enumerate() {
                if enabled {
                    finals[column_keys[column]][bit / 8] |= 1 << (bit % 8);
                }
            }
        }
    }
    let bases = bases
        .into_iter()
        .enumerate()
        .filter_map(|(key, mask)| has_membership(base_membership, key).then_some(mask))
        .collect::<Vec<_>>();
    let finals = finals
        .into_iter()
        .enumerate()
        .filter_map(|(key, mask)| has_membership(final_membership, key).then_some(mask))
        .collect::<Vec<_>>();
    ensure!(
        bases.len() == membership_count(base_membership)
            && finals.len() == membership_count(final_membership),
        "factored name glyph membership disagrees with its masks"
    );
    Ok(FactoredFinalMasks {
        base_masks: bases,
        final_masks: finals,
    })
}

fn pack_fill_mask(
    pixels: &[u8],
    crop: [usize; 4],
    occupied_coordinates: &[usize],
    bytes_per_mask: usize,
) -> Vec<u8> {
    let mut mask = vec![0_u8; bytes_per_mask];
    for (bit, coordinate) in occupied_coordinates.iter().copied().enumerate() {
        let x = crop[0] + coordinate % crop[2];
        let y = crop[1] + coordinate / crop[2];
        if pixels[y * CELL_WIDTH + x] == FILL_PALETTE_INDEX {
            mask[bit / 8] |= 1 << (bit % 8);
        }
    }
    mask
}

fn factor_or_matrix(
    row_count: usize,
    column_count: usize,
    observations: &[(usize, usize, bool)],
) -> (Vec<bool>, Vec<bool>) {
    let mut best = None;
    for seed in [0.0, 0.5, 0.75, 1.0] {
        let mut rows = vec![false; row_count];
        let mut columns = vec![false; column_count];
        if seed > 0.0 && seed < 1.0 {
            for (row, enabled) in rows.iter_mut().enumerate() {
                let relevant = observations
                    .iter()
                    .filter(|observation| observation.0 == row);
                let (ones, count) = relevant.fold((0_usize, 0_usize), |(ones, count), value| {
                    (ones + usize::from(value.2), count + 1)
                });
                *enabled = count != 0 && ones as f64 / count as f64 >= seed;
            }
        } else if seed == 1.0 {
            columns.fill(true);
        }
        for _ in 0..16 {
            let previous = (rows.clone(), columns.clone());
            for (row, enabled) in rows.iter_mut().enumerate() {
                let disabled_error = observations
                    .iter()
                    .filter(|observation| observation.0 == row)
                    .filter(|observation| columns[observation.1] != observation.2)
                    .count();
                let enabled_error = observations
                    .iter()
                    .filter(|observation| observation.0 == row && !observation.2)
                    .count();
                *enabled = enabled_error < disabled_error;
            }
            for (column, enabled) in columns.iter_mut().enumerate() {
                let disabled_error = observations
                    .iter()
                    .filter(|observation| observation.1 == column)
                    .filter(|observation| rows[observation.0] != observation.2)
                    .count();
                let enabled_error = observations
                    .iter()
                    .filter(|observation| observation.1 == column && !observation.2)
                    .count();
                *enabled = enabled_error < disabled_error;
            }
            if previous == (rows.clone(), columns.clone()) {
                break;
            }
        }
        let error = observations
            .iter()
            .filter(|observation| (rows[observation.0] || columns[observation.1]) != observation.2)
            .count();
        if best
            .as_ref()
            .is_none_or(|(best_error, _, _)| error < *best_error)
        {
            best = Some((error, rows, columns));
        }
    }
    let (_, rows, columns) = best.expect("component factorization has no deterministic seed");
    (rows, columns)
}
