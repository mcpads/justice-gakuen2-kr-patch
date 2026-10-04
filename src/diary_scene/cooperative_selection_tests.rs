use super::*;
use std::{fs, path::Path};

#[test]
#[ignore = "requires the private original disc and selected font; emits no ROM"]
fn source_clear_selection_replaces_all_names_and_preserves_other_pixels() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let disc = crate::source_disc::SupportedSourceDisc::open(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )?;
    let assets =
        crate::diary_scene::assets::load_diary_scene_assets(&root.join("assets/diary/scenes"))?;
    let family = assets
        .fixed_presentations
        .iter()
        .find(|family| family.id == "diary_clear_cooperative_selection")
        .unwrap();
    let (_, archive) = disc.read_record("DAT2/MGBGE03.BZZ")?;
    let binding = assets
        .runtime_bundles
        .iter()
        .find(|b| b.path == "DAT2/MGBGE03.BZZ")
        .unwrap();
    assert_eq!(sha256_bytes(&archive), binding.archive_sha256);
    let slot = crate::tzz::parse_tzz(&archive)?[22];
    let stored = &archive[slot.compressed_range()];
    let source = crate::compression::decompress(stored, false)?;
    let spec = crate::development_build_spec::load_development_build_spec(
        &root.join("assets/build/development.json"),
    )?;
    let contribution =
        build_fixed_presentation_contribution(&source, family, &spec.fonts.diary_scene)?;
    let mut plan = crate::decoded_record_write_plan::DecodedRecordWritePlan::new(
        "clear selection",
        &source,
        &family.source_decoded_sha256,
    )?;
    plan.register_data_candidate(
        "clear selection",
        &family.source_decoded_sha256,
        &contribution.candidate,
        &contribution.claims,
    )?;
    assert_eq!(plan.apply(None)?, contribution.candidate);
    let old = read_4bpp_indexed_image_in_prefix(&source, family.tim_offset)?;
    let new = read_4bpp_indexed_image_in_prefix(&contribution.candidate, family.tim_offset)?;
    assert_eq!((old.width, old.height), (1024, 256));
    assert_eq!(family.translated_regions.len(), 23); // 22 selectable moves + one line packed into two heading strips.
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut report = Vec::new();
    for region in &family.translated_regions {
        let c = region.cell;
        let style = font_for_role(&spec.fonts.diary_scene, region.font_role);
        let expected = render_region(
            rasterizers.for_font(&style.path)?,
            style,
            c,
            &region.korean_text,
            region.rendering,
        )?;
        assert_eq!(
            read_indexed_cell_in_prefix(&contribution.candidate, family.tim_offset, c)?,
            expected
        );
        assert!(
            expected[..c.width]
                .iter()
                .chain(&expected[(c.height - 1) * c.width..])
                .all(|p| *p == 0)
        );
        if matches!(
            region.rendering,
            DiarySceneFixedPresentationRendering::Outlined { .. }
        ) {
            assert!((0..c.height).all(|y| expected[y*c.width] == 0 && expected[y*c.width+c.width-1] == 0));
        } else if let DiarySceneFixedPresentationRendering::SplitLine { second_width, .. } =
            region.rendering
        {
            let line_height = c.height / 2;
            let line = render_region(
                rasterizers.for_font(&style.path)?,
                style,
                Cell {
                    x: 0,
                    y: 0,
                    width: c.width + second_width,
                    height: line_height,
                },
                &region.korean_text,
                DiarySceneFixedPresentationRendering::Outlined {
                    clear_index: 0,
                    outline_index: 3,
                    fill_index: 14,
                    horizontal_alignment: HorizontalTextAlignment::Left,
                },
            )?;
            for y in 0..line_height {
                let mut rejoined = expected[y * c.width..(y + 1) * c.width].to_vec();
                let start = (line_height + y) * c.width;
                rejoined.extend_from_slice(&expected[start..start + second_width]);
                assert_eq!(
                    rejoined,
                    line[y * (c.width + second_width)..(y + 1) * (c.width + second_width)]
                );
            }
        }
        // Every source tail up to the next texture page is blank, unlike EDITCM's three tails.
        for y in c.y..c.y + c.height {
            assert!(
                old.pixels[y * old.width + c.x + c.width..y * old.width + c.x + 256]
                    .iter()
                    .all(|p| *p == 0)
            );
        }
        report.push(
            serde_json::json!({"id":region.id,"source_text":region.source_text,
            "korean_text":region.korean_text,"cell":c,"font_px":style.font_px}),
        );
    }
    for (i, (before, after)) in old.pixels.iter().zip(&new.pixels).enumerate() {
        let (x, y) = (i % old.width, i / old.width);
        if !family.translated_regions.iter().any(|r| {
            let c = r.cell;
            x >= c.x && x < c.x + c.width && y >= c.y && y < c.y + c.height
        }) {
            assert_eq!(before, after, "changed neighboring pixel {x},{y}");
        }
    }
    let mut drifted = source.clone();
    drifted[family.tim_offset + 24] ^= 1;
    assert!(
        build_fixed_presentation_contribution(&drifted, family, &spec.fonts.diary_scene).is_err()
    );
    let encoding = crate::diary_scene::runtime_bundle_build::encode_runtime_member(
        stored,
        &source,
        &contribution.candidate,
    )?;
    assert!(encoding.compression_requirement_satisfied);
    assert!(encoding.bytes.len() <= stored.len());
    assert_eq!(
        crate::compression::decompress(&encoding.bytes, true)?,
        contribution.candidate
    );
    let output = root.join("work/edit-consumer-batch/static");
    fs::create_dir_all(&output)?;
    fs::write(output.join("clear-selection-indexed.bin"), &new.pixels)?;
    fs::write(
        output.join("clear-selection.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "source_decoded_sha256":family.source_decoded_sha256,
            "patched_decoded_sha256":sha256_bytes(&contribution.candidate),
            "translations_sha256":family.team_up_names_sha256,
            "font_sha256":crate::pipeline::sha256_bytes(&fs::read(&spec.fonts.diary_scene.cooperative_selection.path)?),
            "stored_size":stored.len(),"rebuilt_size":encoding.bytes.len(),
            "compression_requirement_satisfied":true,"outside_regions_preserved":true,
            "runtime_verified":false,"regions":report,
        }))?,
    )?;
    Ok(())
}
