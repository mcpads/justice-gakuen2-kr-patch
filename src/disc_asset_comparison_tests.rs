use std::collections::BTreeSet;
use std::path::Path;

use crate::compression::compress;
use crate::test_support::temporary_directory;

use super::{
    ComparisonStatus, FixedPresentationTarget, FixedPresentationTargetIndex,
    RuntimeFourBitSpritePacket, RuntimeFourBitTextureTarget, byte_difference_summary,
    compare_decoded_layers, fixed_presentation_atlas_families, fixed_presentation_source_class,
    indexed_difference_image, path_slug, pixel_difference_count, prepare_output_directory,
    runtime_sprite_cells, runtime_texture_source_origins, tim_image_payload_matches_vram,
};
use crate::embedded_tim::EmbeddedTimAudit;

use super::model::{
    FixedPresentationGlyphRuntimeUse, FixedPresentationGlyphSourceMatch,
    FixedPresentationGlyphTargetAudit,
};

fn four_bit_tim(pixel_byte: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&0x08u32.to_le_bytes());
    bytes.extend_from_slice(&44u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    for color in 0..16u16 {
        let value: u16 = if color == 0 { 0 } else { 0x7fff };
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[pixel_byte, 0, 0, 0]);
    bytes
}

fn presentation_match(
    record_path: &str,
    decoded_layer_id: &str,
    tim_sha256: &str,
) -> FixedPresentationGlyphSourceMatch {
    FixedPresentationGlyphSourceMatch {
        record_path: record_path.to_string(),
        decoded_layer_id: decoded_layer_id.to_string(),
        decoded_layer_sha256: format!("decoded-{record_path}-{decoded_layer_id}"),
        tim_offset: 0xe000,
        tim_sha256: tim_sha256.to_string(),
        tim_bits_per_pixel: 4,
        tim_pixel_width: 768,
        tim_pixel_height: 256,
        tim_image_vram_word_x: 768,
        tim_image_vram_y: 0,
        pixel_x: 184,
        pixel_y: 176,
        source_class: "presentation_candidate".to_string(),
        preview_file: Some(format!("{record_path}-{decoded_layer_id}.png")),
        preview_sha256: Some(format!("preview-{record_path}-{decoded_layer_id}")),
    }
}

#[test]
fn byte_difference_summary_reports_content_and_length_changes() {
    let summary = byte_difference_summary(&[0, 1, 2, 3], &[0, 9, 2, 8, 7]);

    assert_eq!(summary.changed_byte_count, 3);
    assert_eq!(summary.changed_range_count, 2);
    assert_eq!(summary.changed_byte_ranges_preview, [[1, 2], [3, 5]]);
    assert!(!summary.ranges_truncated);
}

#[test]
fn changed_compression_padding_does_not_become_a_visual_change() {
    let output = temporary_directory("disc-asset-comparison-padding");
    std::fs::create_dir_all(&output).unwrap();
    let tim = four_bit_tim(0);
    let mut source = compress(&tim, 0).unwrap();
    let mut patched = source.clone();
    source.extend_from_slice(&[0, 0]);
    patched.extend_from_slice(&[0xaa, 0x55]);

    let (layers, issues) =
        compare_decoded_layers(0, "DAT2/IMAGE.TIZ", Some(&source), Some(&patched), &output)
            .unwrap();

    assert!(issues.is_empty());
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].status, ComparisonStatus::Unchanged);
    assert!(!layers[0].embedded_tim_scan_performed);
    assert!(layers[0].embedded_tims.is_empty());

    std::fs::remove_dir_all(output).unwrap();
}

#[test]
fn changed_tim_produces_original_patched_and_difference_previews() {
    let output = temporary_directory("disc-asset-comparison-previews");
    std::fs::create_dir_all(&output).unwrap();
    let source = compress(&four_bit_tim(0), 0).unwrap();
    let patched = compress(&four_bit_tim(1), 0).unwrap();

    let (layers, issues) =
        compare_decoded_layers(0, "DAT2/IMAGE.TIZ", Some(&source), Some(&patched), &output)
            .unwrap();

    assert!(issues.is_empty());
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].status, ComparisonStatus::Changed);
    assert_eq!(layers[0].embedded_tims.len(), 1);
    let tim = &layers[0].embedded_tims[0];
    assert_eq!(tim.status, ComparisonStatus::Changed);
    assert_eq!(tim.indexed_pixel_difference_count, Some(1));
    assert_eq!(tim.clut_word_difference_count, Some(0));
    assert_eq!(tim.preview_palette_index, Some(0));
    for relative in [
        tim.source_preview_file.as_ref().unwrap(),
        tim.patched_preview_file.as_ref().unwrap(),
        tim.difference_preview_file.as_ref().unwrap(),
    ] {
        assert!(output.join(relative).is_file(), "missing {relative}");
    }

    std::fs::remove_dir_all(output).unwrap();
}

#[test]
fn changed_clut_is_reported_separately_from_indexed_pixels() {
    let output = temporary_directory("disc-asset-comparison-clut");
    std::fs::create_dir_all(&output).unwrap();
    let source = compress(&four_bit_tim(0x21), 0).unwrap();
    let mut patched_tim = four_bit_tim(0x21);
    patched_tim[22] = 0x1f;
    patched_tim[23] = 0;
    let patched = compress(&patched_tim, 0).unwrap();

    let (layers, issues) =
        compare_decoded_layers(0, "DAT2/IMAGE.TIZ", Some(&source), Some(&patched), &output)
            .unwrap();

    assert!(issues.is_empty());
    let tim = &layers[0].embedded_tims[0];
    assert_eq!(tim.indexed_pixel_difference_count, Some(0));
    assert_eq!(tim.clut_word_difference_count, Some(1));
    assert!(tim.palette_zero_rgba_difference_count.unwrap() > 0);

    std::fs::remove_dir_all(output).unwrap();
}

#[test]
fn difference_preview_marks_only_changed_indexed_pixels() {
    let source = super::IndexedPixels {
        width: 2,
        height: 1,
        pixels: vec![1, 2],
    };
    let patched = super::IndexedPixels {
        width: 2,
        height: 1,
        pixels: vec![1, 3],
    };

    assert_eq!(
        pixel_difference_count(Some(&source), Some(&patched)),
        Some(1)
    );
    let difference = indexed_difference_image(Some(&source), Some(&patched)).unwrap();
    assert_ne!(&difference.pixels[0..4], &[255, 32, 180, 255]);
    assert_eq!(&difference.pixels[4..8], &[255, 32, 180, 255]);
}

#[test]
fn preview_directories_use_stable_path_slugs() {
    assert_eq!(
        path_slug("DAT2/MG BG.TZZ:member-001"),
        "dat2-mg-bg-tzz-member-001"
    );
    assert!(!Path::new(&path_slug("DAT2/MENU.BIZ")).is_absolute());
}

#[test]
fn fixed_presentation_scan_finds_a_twenty_pixel_glyph_off_any_nominal_grid() {
    let mut image = super::IndexedPixels {
        width: 47,
        height: 43,
        pixels: vec![0; 47 * 43],
    };
    let mut target = vec![0u8; 20 * 20];
    for y in 0..20 {
        for x in 0..20 {
            target[y * 20 + x] = u8::try_from((x * 3 + y * 5) % 15 + 1).unwrap();
            image.pixels[(y + 11) * image.width + x + 7] = target[y * 20 + x];
        }
    }

    let targets = [FixedPresentationTarget {
        pixel_sha256: "target".to_string(),
        pixels: target,
        source_codes: BTreeSet::new(),
        source_texts: BTreeSet::new(),
        source_semantic_ids: BTreeSet::new(),
        runtime_uses: BTreeSet::new(),
    }];
    let index = FixedPresentationTargetIndex::new(&targets);

    assert_eq!(index.find(&image, &targets), [(0, 7, 11)]);
}

#[test]
fn fixed_presentation_scan_separates_runtime_fonts_and_standalone_glyph_strips() {
    let allocation_paths = BTreeSet::from(["DAT2/MGK04.BIZ".to_string()]);
    let mut tim = EmbeddedTimAudit {
        offset: 0,
        total_size: 0,
        source_tim_sha256: String::new(),
        bits_per_pixel: 4,
        pixel_width: 20,
        pixel_height: 14_920,
        image_vram_word_x: 0,
        image_vram_y: 0,
        clut_vram_x: 0,
        clut_vram_y: 0,
        palette_count: 1,
        preview_file: String::new(),
        preview_sha256: String::new(),
    };

    assert_eq!(
        fixed_presentation_source_class("DAT2/MGK04.BIZ", &tim, &allocation_paths),
        "dialogue_runtime_font"
    );
    assert_eq!(
        fixed_presentation_source_class("DAT2/MGG04.BZZ", &tim, &allocation_paths),
        "standalone_glyph_strip"
    );
    tim.pixel_width = 768;
    tim.pixel_height = 256;
    assert_eq!(
        fixed_presentation_source_class("DAT2/MGBGK04.BZZ", &tim, &allocation_paths),
        "presentation_candidate"
    );
}

#[test]
fn runtime_residency_requires_the_complete_tim_image_at_its_declared_vram_position() {
    let mut tim_bytes = four_bit_tim(0x21);
    let embedded = EmbeddedTimAudit {
        offset: 0,
        total_size: tim_bytes.len(),
        source_tim_sha256: String::new(),
        bits_per_pixel: 4,
        pixel_width: 8,
        pixel_height: 1,
        image_vram_word_x: 0,
        image_vram_y: 0,
        clut_vram_x: 0,
        clut_vram_y: 0,
        palette_count: 1,
        preview_file: String::new(),
        preview_sha256: String::new(),
    };
    let image_x = 13u16;
    let image_y = 7u16;
    tim_bytes[56..58].copy_from_slice(&image_x.to_le_bytes());
    tim_bytes[58..60].copy_from_slice(&image_y.to_le_bytes());
    let mut gpu = vec![0u8; super::PSX_VRAM_BYTE_COUNT];
    let vram_start =
        usize::from(image_y) * super::PSX_VRAM_WIDTH_WORDS * 2 + usize::from(image_x) * 2;
    gpu[vram_start..vram_start + 4].copy_from_slice(&tim_bytes[64..68]);

    assert!(tim_image_payload_matches_vram(&tim_bytes, &embedded, &gpu).unwrap());
    gpu[vram_start + 3] ^= 1;
    assert!(!tim_image_payload_matches_vram(&tim_bytes, &embedded, &gpu).unwrap());
}

#[test]
fn runtime_sprite_cell_inventory_omits_transparent_cells() {
    let packet = RuntimeFourBitSpritePacket {
        packet_address: 0x8010_0000,
        screen_x: 32,
        screen_y: 48,
        width: 20,
        height: 20,
        texture_page_word_x: 0,
        texture_page_y: 0,
        texture_u: 0,
        texture_v: 0,
        clut: 0,
        draw_mode: 0xe100_0200,
    };
    let mut gpu = vec![0u8; super::PSX_VRAM_BYTE_COUNT];

    assert!(
        runtime_sprite_cells(std::slice::from_ref(&packet), &gpu)
            .unwrap()
            .is_empty()
    );

    gpu[0] = 1;
    let cells = runtime_sprite_cells(&[packet], &gpu).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].nonzero_pixel_count, 1);
}

#[test]
fn runtime_texture_source_search_uses_exact_pixels_at_any_tim_location() {
    let width = 40;
    let height = 20;
    let pixels = (0..width * height)
        .map(|index| u8::try_from((index * 7 + index / width * 3) % 15 + 1).unwrap())
        .collect::<Vec<_>>();
    let target = RuntimeFourBitTextureTarget {
        pixel_sha256: "runtime-texture".to_string(),
        width,
        height,
        pixels: pixels.clone(),
        packets: Vec::new(),
    };
    let mut source = super::IndexedPixels {
        width: 96,
        height: 64,
        pixels: vec![0; 96 * 64],
    };
    let expected = (13, 17);
    for row in 0..height {
        let source_start = (expected.1 + row) * source.width + expected.0;
        let target_start = row * width;
        source.pixels[source_start..source_start + width]
            .copy_from_slice(&pixels[target_start..target_start + width]);
    }

    assert_eq!(runtime_texture_source_origins(&source, &target), [expected]);

    source.pixels[expected.1 * source.width + expected.0] ^= 1;
    assert!(runtime_texture_source_origins(&source, &target).is_empty());
}

#[test]
fn fixed_presentation_atlas_summary_groups_duplicate_consumers_and_compares_each_surface() {
    let source_tim = "source-atlas";
    let target = FixedPresentationGlyphTargetAudit {
        pixel_sha256: "question-glyph".to_string(),
        source_codes: vec!["0x0121".to_string()],
        source_texts: vec!["問".to_string()],
        source_semantic_ids: Vec::new(),
        runtime_uses: vec![FixedPresentationGlyphRuntimeUse {
            screen_x: 84,
            screen_y: 108,
            sprite_width_pixels: 40,
            sprite_height_pixels: 20,
            texture_vram_word_x: 814,
            texture_vram_y: 176,
        }],
        source_matches: vec![
            presentation_match("DAT2/MGBGK04.BZZ", "member-002", source_tim),
            presentation_match("DAT2/MGBGK05.BZZ", "member-001", source_tim),
        ],
        patched_matches: vec![
            presentation_match("DAT2/MGBGK04.BZZ", "member-002", source_tim),
            presentation_match("DAT2/MGBGK05.BZZ", "member-001", "changed-atlas"),
        ],
        source_presentation_candidate_match_count: 2,
        patched_presentation_candidate_match_count: 2,
        unchanged_presentation_candidate_locations: Vec::new(),
    };

    let families = fixed_presentation_atlas_families(&[target]).unwrap();

    assert_eq!(families.len(), 1);
    let family = &families[0];
    assert_eq!(family.source_tim_sha256.as_deref(), Some(source_tim));
    assert_eq!(family.source_surface_count, 2);
    assert_eq!(family.patched_surface_count, 2);
    assert_eq!(family.unchanged_surface_count, 1);
    assert_eq!(family.changed_surface_count, 1);
    assert_eq!(family.source_target_count, 1);
    assert_eq!(family.patched_target_count, 1);
    assert_eq!(family.source_runtime_observed_target_count, 1);
    assert_eq!(family.patched_runtime_observed_target_count, 1);
    assert_eq!(family.source_texts, ["問"]);
}

#[test]
fn force_refuses_to_delete_a_directory_without_its_ownership_marker() {
    let output = temporary_directory("disc-asset-comparison-unowned-output");
    std::fs::create_dir_all(&output).unwrap();
    let preserved = output.join("preserve.txt");
    std::fs::write(&preserved, "user data").unwrap();

    let error = prepare_output_directory(&output, true).unwrap_err();

    assert!(error.to_string().contains("unowned output directory"));
    assert!(preserved.is_file());
    std::fs::remove_dir_all(output).unwrap();
}
