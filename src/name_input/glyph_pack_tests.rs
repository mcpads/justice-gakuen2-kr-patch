use std::path::{Path, PathBuf};

use super::{
    KS_X_1001_HANGUL_COUNT, NAME_GLYPH_PACK_STORAGE_BYTES, NameGlyphPackDecoder,
    build_default_name_glyph_pack, load_ks_x_1001_hangul,
};

fn maplestory_light() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf")
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn component_pack_fits_hidden_cells_and_decodes_the_supported_repertoire() {
    let pack = build_default_name_glyph_pack(&maplestory_light()).unwrap();
    let decoder = NameGlyphPackDecoder::new(&pack.bytes).unwrap();
    let repertoire = load_ks_x_1001_hangul().unwrap();

    assert_eq!(pack.report.supported_syllable_count, KS_X_1001_HANGUL_COUNT);
    assert_eq!(pack.report.unsupported_modern_syllable_count, 8_822);
    assert_eq!(pack.report.crop, [4, 6, 12, 13]);
    assert_eq!(pack.report.occupied_coordinate_count, 140);
    assert_eq!(pack.report.bytes_per_component_mask, 18);
    assert_eq!(pack.report.no_final_base_component_count, 349);
    assert_eq!(pack.report.final_bearing_base_component_count, 328);
    assert_eq!(pack.report.final_component_count, 249);
    assert_eq!(pack.report.component_count, 926);
    assert_eq!(pack.report.no_final_rank_prefix_entry_bytes, 2);
    assert_eq!(pack.report.final_rank_prefix_entry_bytes, 1);
    assert_eq!(pack.report.no_final_rank_prefix_byte_range, [1_612, 1_712]);
    assert_eq!(
        pack.report.final_bearing_rank_prefix_byte_range,
        [1_712, 1_812]
    );
    assert_eq!(pack.report.final_rank_prefix_byte_range, [1_812, 1_883]);
    assert_eq!(pack.report.runtime_coordinate_list_byte_count, 140);
    assert_eq!(pack.report.component_mask_byte_range, [1_883, 18_551]);
    assert_eq!(pack.report.pack_bytes, 18_551);
    assert_eq!(
        pack.report.storage_capacity_bytes,
        NAME_GLYPH_PACK_STORAGE_BYTES
    );
    assert_eq!(pack.report.storage_bytes_remaining, 449);
    assert!(pack.report.fits_storage);
    assert!(!pack.report.runtime_consumer_installed);
    assert!(pack.report.exact_reference_glyph_count >= 349);
    assert_eq!(pack.report.synthesized_blank_count, 0);
    assert!(decoder.supports('한'));
    assert!(!decoder.supports('갂'));
    assert!(decoder.render('갂').is_err());
    assert_eq!(pack.runtime_coordinate_list.len(), 140);

    for character in repertoire {
        let pixels = decoder.render(character).unwrap();
        assert_eq!(pixels.len(), 400);
        assert!(pixels.contains(&13));
        assert!(pixels.contains(&3));
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn pack_decoder_rejects_header_corruption() {
    let mut pack = build_default_name_glyph_pack(&maplestory_light())
        .unwrap()
        .bytes;
    pack[0] ^= 0xff;

    assert!(NameGlyphPackDecoder::new(&pack).is_err());
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn pack_decoder_rejects_rank_prefix_corruption() {
    let mut pack = build_default_name_glyph_pack(&maplestory_light())
        .unwrap()
        .bytes;
    let first_rank_prefix = 24 + 20 + 1_397 + 50 + 50 + 71;
    pack[first_rank_prefix] = 1;

    assert!(NameGlyphPackDecoder::new(&pack).is_err());
}
