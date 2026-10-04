use std::path::Path;

use crate::development_build_spec::load_development_build_spec;
use crate::tim::{
    parse_8bpp_prefix, read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
};

use super::build::build_bonus_menu;
use super::heading::HEADING_CLEANUP_CELL;
use super::model::{BonusMenuBuildConfig, BonusMenuFontRole};
use super::source::load_source;

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_builds_centered_heading_and_preserves_8bpp_background() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let spec = load_development_build_spec(&root.join("assets/build/development.json")).unwrap();
    let output_dir = std::env::temp_dir().join(format!(
        "justice-bonus-heading-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);

    let build = build_bonus_menu(&BonusMenuBuildConfig {
        cue: cue.clone(),
        assets: spec.assets.bonus_menu,
        fonts: spec.fonts.bonus_menu,
        build_spec_sha256: spec.sha256,
        output_dir: output_dir.clone(),
        force: true,
    })
    .unwrap();

    assert_eq!(build.report.authored_unit_count, 4);
    assert_eq!(build.report.untranslated_unit_count, 0);
    assert!(build.report.complete_surface_localized);
    assert!(build.report.source_palette_unchanged);
    assert!(build.report.heading_background_preserved_outside_cleanup);
    assert!(build.report.changed_bytes_confined_to_owned_cells);
    let heading = build
        .report
        .units
        .iter()
        .find(|unit| unit.id == "heading")
        .unwrap();
    let [left, top, right, bottom] = heading.ink_bounds.unwrap();
    assert!(left.abs_diff(heading.cell.width - right) <= 1);
    assert!(left >= 16 && right <= 176 && top >= 4 && bottom <= 92);
    let heading_font = build
        .report
        .fonts
        .iter()
        .find(|font| font.role == BonusMenuFontRole::Heading)
        .unwrap();
    assert_eq!(heading_font.font_name, "Maplestory Bold");
    assert_eq!(heading_font.font_px, 44.0);

    let source = load_source(&cue).unwrap();
    let tim = parse_8bpp_prefix(&source.decoded).unwrap();
    assert_eq!(
        &build.decoded[..tim.pixel_offset],
        &source.decoded[..tim.pixel_offset]
    );
    assert_pixels_outside_heading_cell_unchanged(&source.decoded, &build.decoded);
    let source_heading =
        read_8bpp_indexed_cell_in_prefix(&source.decoded, 0, heading.cell).unwrap();
    let output_heading = read_8bpp_indexed_cell_in_prefix(&build.decoded, 0, heading.cell).unwrap();
    assert_ne!(source_heading, output_heading);
    assert_background_outside_cleanup_unchanged(&source_heading, &output_heading);

    let palette = read_8bpp_palette_words_in_prefix(&build.decoded, 0, 0).unwrap();
    let preview = root.join("work/bonus-heading-preview/bonus-main-heading-static.png");
    write_indexed_preview(
        &preview,
        heading.cell.width,
        heading.cell.height,
        &output_heading,
        &palette,
    );
    let surface_preview = root.join("work/bonus-heading-preview/bonus-main-surface-static.png");
    let full_surface =
        &build.decoded[tim.pixel_offset..tim.pixel_offset + tim.row_bytes() * tim.image_height];
    write_indexed_preview(
        &surface_preview,
        tim.pixel_width(),
        tim.image_height,
        full_surface,
        &palette,
    );
    eprintln!(
        "heading font={}px advance={:.2} bounds={:?} preview={} surface_preview={}",
        heading_font.font_px,
        heading.measured_advance_px.unwrap(),
        heading.ink_bounds.unwrap(),
        preview.display(),
        surface_preview.display()
    );

    std::fs::remove_dir_all(output_dir).unwrap();
}

fn assert_pixels_outside_heading_cell_unchanged(source: &[u8], output: &[u8]) {
    let tim = parse_8bpp_prefix(source).unwrap();
    for y in 0..tim.image_height {
        for x in 0..tim.pixel_width() {
            let inside_heading = (160..352).contains(&x) && (48..144).contains(&y);
            if inside_heading {
                continue;
            }
            let offset = tim.pixel_offset + y * tim.row_bytes() + x;
            assert_eq!(output[offset], source[offset]);
        }
    }
}

fn assert_background_outside_cleanup_unchanged(source: &[u8], output: &[u8]) {
    for y in 0..96 {
        for x in 0..192 {
            let inside_cleanup = (HEADING_CLEANUP_CELL.x
                ..HEADING_CLEANUP_CELL.x + HEADING_CLEANUP_CELL.width)
                .contains(&x)
                && (HEADING_CLEANUP_CELL.y..HEADING_CLEANUP_CELL.y + HEADING_CLEANUP_CELL.height)
                    .contains(&y);
            if !inside_cleanup {
                assert_eq!(output[y * 192 + x], source[y * 192 + x]);
            }
        }
    }
}

fn write_indexed_preview(
    path: &Path,
    width: usize,
    height: usize,
    pixels: &[u8],
    palette: &[u16; 256],
) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rgba = Vec::with_capacity(width * height * 4);
    for index in pixels {
        let word = palette[usize::from(*index)];
        let expand = |component: u16| {
            let value = component as u8;
            (value << 3) | (value >> 2)
        };
        rgba.extend_from_slice(&[
            expand(word & 0x1f),
            expand((word >> 5) & 0x1f),
            expand((word >> 10) & 0x1f),
            255,
        ]);
    }
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
}
