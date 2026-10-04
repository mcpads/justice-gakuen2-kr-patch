use std::path::{Path, PathBuf};

use super::{
    NAME_GLYPH_PACK_STORAGE_BYTES, NameGlyphPackDecoder, encode_name_glyph_pack_cells,
    install_name_glyph_pack_cells_in_tim, install_name_glyph_pack_runtime_lookup,
    load_name_input_keyboard, read_name_glyph_pack_cells,
};
use crate::tim::Cell;

fn maplestory_light() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf")
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn runtime_lookup_occupies_only_the_reserved_trailing_storage_cells() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = super::build_default_name_glyph_pack(&maplestory_light()).unwrap();
    let mut stored =
        encode_name_glyph_pack_cells(&pack.bytes, &keyboard.glyph_pack_storage).unwrap();
    let lookup = (0..222).map(|value| value as u8).collect::<Vec<_>>();

    let report = install_name_glyph_pack_runtime_lookup(&mut stored, &lookup).unwrap();

    assert_eq!(report.first_storage_cell_index, 93);
    assert_eq!(report.storage_cell_count, 2);
    assert_eq!(report.storage_byte_range, [18_600, 18_822]);
    assert_eq!(report.byte_count, lookup.len());
    assert!(report.readback_verified);
    assert_eq!(read_name_glyph_pack_cells(&stored).unwrap(), pack.bytes);
    assert!(install_name_glyph_pack_runtime_lookup(&mut stored, &lookup).is_err());
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn runtime_lookup_and_coordinate_list_share_the_two_reserved_tail_cells() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = super::build_default_name_glyph_pack(&maplestory_light()).unwrap();
    let mut stored =
        encode_name_glyph_pack_cells(&pack.bytes, &keyboard.glyph_pack_storage).unwrap();
    let mut runtime_metadata = vec![0x5a; 222];
    runtime_metadata.extend_from_slice(&pack.runtime_coordinate_list);

    let report = install_name_glyph_pack_runtime_lookup(&mut stored, &runtime_metadata).unwrap();

    assert_eq!(runtime_metadata.len(), 362);
    assert_eq!(report.first_storage_cell_index, 93);
    assert_eq!(report.storage_cell_count, 2);
    assert_eq!(report.storage_byte_range, [18_600, 18_962]);
    assert!(report.readback_verified);
    assert_eq!(read_name_glyph_pack_cells(&stored).unwrap(), pack.bytes);
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn component_pack_roundtrips_through_scattered_four_bit_cells() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = super::build_default_name_glyph_pack(&maplestory_light()).unwrap();

    let stored = encode_name_glyph_pack_cells(&pack.bytes, &keyboard.glyph_pack_storage).unwrap();
    let readback = read_name_glyph_pack_cells(&stored).unwrap();

    assert_eq!(readback, pack.bytes);
    assert_eq!(stored.cells.len(), 95);
    assert_eq!(stored.cells[0].pixels.len(), 400);
    assert_eq!(
        stored
            .cells
            .iter()
            .map(|cell| cell.pixels.len())
            .sum::<usize>()
            / 2,
        NAME_GLYPH_PACK_STORAGE_BYTES
    );
    assert!(NameGlyphPackDecoder::new(&readback).unwrap().supports('한'));
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn nonzero_trailing_storage_is_rejected() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = super::build_default_name_glyph_pack(&maplestory_light()).unwrap();
    let mut stored =
        encode_name_glyph_pack_cells(&pack.bytes, &keyboard.glyph_pack_storage).unwrap();
    let last = stored.cells.last_mut().unwrap();
    *last.pixels.last_mut().unwrap() = 1;

    assert!(read_name_glyph_pack_cells(&stored).is_err());
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn component_pack_roundtrips_through_the_physical_tim_stride() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = super::build_default_name_glyph_pack(&maplestory_light()).unwrap();
    let stored = encode_name_glyph_pack_cells(&pack.bytes, &keyboard.glyph_pack_storage).unwrap();
    let tim_offset = 16;
    let mut decoded = vec![0x5a; tim_offset];
    decoded.extend(synthetic_name_font_tim());
    let prefix = decoded[..tim_offset].to_vec();

    let report = install_name_glyph_pack_cells_in_tim(
        &mut decoded,
        tim_offset,
        &synthetic_name_layout_cells(),
        &stored,
    )
    .unwrap();

    assert_eq!(decoded[..tim_offset], prefix);
    assert_eq!(report.payload_byte_count, pack.bytes.len());
    assert_eq!(report.storage_cell_count, 95);
    assert!(report.changed_decoded_byte_count > 0);
    assert_eq!(report.payload_sha256, report.tim_readback_sha256);
    assert!(report.tim_readback_verified);
}

fn synthetic_name_layout_cells() -> Vec<Cell> {
    [84_usize, 82, 62]
        .into_iter()
        .enumerate()
        .flat_map(|(page, count)| {
            (0..count).map(move |position| Cell {
                x: page * 256 + position % 12 * 20,
                y: position / 12 * 20,
                width: 20,
                height: 20,
            })
        })
        .collect()
}

fn synthetic_name_font_tim() -> Vec<u8> {
    const PIXEL_BYTES: usize = 768 * 256 / 2;
    let mut data = Vec::with_capacity(64 + PIXEL_BYTES);
    data.extend_from_slice(&0x10_u32.to_le_bytes());
    data.extend_from_slice(&0x08_u32.to_le_bytes());
    data.extend_from_slice(&44_u32.to_le_bytes());
    data.extend_from_slice(&0_u16.to_le_bytes());
    data.extend_from_slice(&483_u16.to_le_bytes());
    data.extend_from_slice(&16_u16.to_le_bytes());
    data.extend_from_slice(&1_u16.to_le_bytes());
    data.extend_from_slice(&[0_u8; 32]);
    data.extend_from_slice(&u32::try_from(12 + PIXEL_BYTES).unwrap().to_le_bytes());
    data.extend_from_slice(&768_u16.to_le_bytes());
    data.extend_from_slice(&0_u16.to_le_bytes());
    data.extend_from_slice(&192_u16.to_le_bytes());
    data.extend_from_slice(&256_u16.to_le_bytes());
    data.extend(std::iter::repeat_n(0xff, PIXEL_BYTES));
    data
}

#[test]
#[ignore = "requires assets/"]
fn auxiliary_font_data_survives_tim_install_without_changing_hangul_or_lookup() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let pack = vec![0x5a; 16937];
    let mut stored = encode_name_glyph_pack_cells(&pack, &keyboard.glyph_pack_storage).unwrap();
    let lookup = vec![0x7b; 190];
    install_name_glyph_pack_runtime_lookup(&mut stored, &lookup).unwrap();
    let payload: Vec<_> = (0..1514).map(|v| (v * 37) as u8).collect();
    super::install_name_glyph_pack_auxiliary(&mut stored, 16940, &payload).unwrap();
    assert_eq!(read_name_glyph_pack_cells(&stored).unwrap(), pack);
    let mut decoded = synthetic_name_font_tim();
    assert!(
        install_name_glyph_pack_cells_in_tim(
            &mut decoded,
            0,
            &synthetic_name_layout_cells(),
            &stored
        )
        .unwrap()
        .tim_readback_verified
    );
    for (start, length) in [
        (16936, 4),
        (16940, 1),
        (18799, 2),
        (18999, 2),
        (usize::MAX, 2),
        (18454, 0),
    ] {
        let before = stored.clone();
        assert!(
            super::install_name_glyph_pack_auxiliary(&mut stored, start, &vec![0; length]).is_err()
        );
        assert_eq!(stored, before);
    }
    // Corruption of the new data must be detected even though the returned pack is unchanged.
    let cell = 16940 / 200;
    let pixel = (16940 % 200) * 2;
    stored.cells[cell].pixels[pixel] ^= 1;
    assert!(read_name_glyph_pack_cells(&stored).is_err());
}
