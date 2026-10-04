use std::path::Path;

use psx_r3000a::{Instruction, Register, decode};

use super::record_build::first_complete_control_block;
use super::source::{
    load_character_select_auxiliary_source_from_disc, load_character_select_source,
};
use super::{CharacterSelectAtlasBuildConfig, build_character_select_atlas};
use crate::compression::decompress;
use crate::development_build_spec::load_development_build_spec;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::test_support::temporary_directory;

#[test]
fn source_first_control_block_is_reused_as_one_complete_unit() {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&0x8000_u16.to_le_bytes());
    encoded.extend_from_slice(&0x1001_u16.to_le_bytes());
    for literal in 0_u16..15 {
        encoded.extend_from_slice(&literal.to_le_bytes());
    }
    let expected_length = encoded.len();
    encoded.extend_from_slice(b"following block");

    let block = first_complete_control_block(&encoded).unwrap();

    assert_eq!(block.len(), expected_length);
    assert_eq!(block, &encoded[..expected_length]);
}

#[test]
fn source_terminator_cannot_be_used_as_an_incomplete_seed_block() {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&0x8000_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());

    let error = first_complete_control_block(&encoded).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("terminates inside its first control block")
    );
}

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_build_respects_patch_boundaries_and_roundtrips() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = load_development_build_spec(&root.join("assets/build/development.json")).unwrap();
    let output = temporary_directory("character-select-build");
    let build = build_character_select_atlas(&CharacterSelectAtlasBuildConfig {
        native_text_font: spec.fonts.shared_menu_numerals.clone(),
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        assets: spec.assets.character_select,
        fonts: spec.fonts.character_select.clone(),
        build_spec_sha256: spec.sha256,
        output_dir: output.clone(),
        force: false,
    })
    .unwrap();

    assert!(build.report.changed_bytes_confined_to_allocated_cells);
    assert!(build.report.compression_roundtrip_verified);
    assert!(build.report.catalog_prefixes_preserved);
    assert!(build.report.development_input_available);
    assert_eq!(build.overlays.len(), 5);
    assert_eq!(build.report.overlays.len(), 5);
    assert!(
        build
            .report
            .glyphs
            .iter()
            .filter(|glyph| { glyph.font_role == super::CharacterSelectFontRole::ModeMenuHeading })
            .all(|glyph| {
                glyph.clear_index == 0 && glyph.outline_index == 1 && glyph.fill_index == 10
            })
    );
    assert!(
        build
            .report
            .fixed_strips
            .iter()
            .filter(|strip| { strip.font_role == super::CharacterSelectFontRole::ModeMenuLabel })
            .all(|strip| {
                strip.clear_index == 0 && strip.outline_index == 2 && strip.fill_index == 15
            })
    );
    assert!(
        build
            .report
            .fully_routed_source_ui_ids
            .iter()
            .any(|source_ui_id| source_ui_id == "cooperative_mode_menu_heading")
    );
    assert!(
        build
            .report
            .fully_routed_source_ui_ids
            .iter()
            .any(|source_ui_id| source_ui_id == "cooperative_background_gedo_emblem")
    );
    for source_ui_id in [
        "tournament_champion_label",
        "tournament_certificate_title",
        "tournament_certificate_body",
        "solo_challenge_prompt",
        "solo_press_start_prompt",
        "solo_wait_prompt",
        "solo_episode_card_1",
        "solo_episode_card_2",
        "solo_episode_card_3",
        "solo_episode_card_4",
        "solo_episode_card_5",
        "solo_episode_card_6",
        "solo_episode_card_7",
        "solo_episode_card_8",
    ] {
        assert!(
            build
                .report
                .fully_routed_source_ui_ids
                .iter()
                .any(|bound| bound == source_ui_id)
        );
    }
    assert!(
        build
            .report
            .route_census
            .unclassified_pending_occurrence_ids
            .is_empty()
    );
    // This is a source-to-record/consumer binding assertion. Screen
    // presentation remains an artifact-bound runtime gate, so do not freeze a
    // progress snapshot such as the current untranslated-card list here.
    let episode_card_route = build
        .report
        .route_census
        .occurrences
        .iter()
        .find(|occurrence| occurrence.occurrence_id == "cdemo-solo-episode-card-1")
        .unwrap();
    assert_eq!(episode_card_route.producer_targets.len(), 1);
    assert_eq!(
        episode_card_route.producer_targets[0].source_record,
        "DAT2/TITLE.BIN"
    );
    let title_load = episode_card_route
        .consumer_targets
        .iter()
        .find(|target| {
            target.consumer_record == "DAT1/CDEMO.BIN"
                && target.reference_kind
                    == super::model::CharacterSelectConsumerReferenceKind::ResourceLoad
        })
        .unwrap();
    assert_eq!(title_load.byte_offsets, [0x0a40, 0x0a44]);
    assert_eq!(
        title_load.runtime_states,
        ["solo-story/episode-card/title-member-0/loader-selector-ram-0x1f64be"]
    );
    let primitive_layout = episode_card_route
        .consumer_targets
        .iter()
        .find(|target| {
            target.consumer_record == "DAT1/CDEMO.BIN"
                && target.reference_kind
                    == super::model::CharacterSelectConsumerReferenceKind::PrimitiveLayout
        })
        .unwrap();
    for offset in [0x0010, 0x07b0, 0x0870, 0x0a00, 0x0a24] {
        assert!(primitive_layout.byte_offsets.contains(&offset));
    }
    assert_eq!(
        primitive_layout.runtime_states,
        ["solo-story/episode-card/title-member-0/source-layout-reference-not-presentation-proof"]
    );
    assert!(
        build
            .report
            .route_census
            .partially_routed_source_ui_ids
            .is_empty()
    );
    let bracket_route = build
        .report
        .route_census
        .occurrences
        .iter()
        .find(|occurrence| occurrence.occurrence_id == "plsel4-completed-bracket-champion-label")
        .unwrap();
    assert_eq!(
        bracket_route.producer_binding_status,
        super::model::CharacterSelectProducerBindingStatus::Bound
    );
    assert!(build.report.overlays.iter().all(|overlay| {
        overlay.changed_byte_ranges.iter().all(|[start, end]| {
            (*start..*end).all(|offset| {
                overlay
                    .expected_write_ranges
                    .iter()
                    .any(|[allowed_start, allowed_end]| {
                        *allowed_start <= offset && offset < *allowed_end
                    })
            })
        })
    }));
    let (_, sources) = load_character_select_source(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    // Rank sprites consume 40x16 cells from this source UV table. The fifth
    // and sixth cells cross a row boundary; a guessed 56px erase corrupts
    // adjacent glyphs. The bracket VS instead consumes a complete 64x32 burst.
    let label_asset: serde_json::Value = serde_json::from_slice(crate::test_input::read_bytes(
        "assets/menu/common/selection-retained-sprites.json",
    ))
    .unwrap();
    for (index, unit) in label_asset["units"].as_array().unwrap()[3..9]
        .iter()
        .enumerate()
    {
        let table = &sources[2].overlay[0x4f4 + index * 4..0x4f8 + index * 4];
        assert_eq!(
            unit["cell"]["x"],
            512 + u16::from_le_bytes([table[0], table[1]])
        );
        assert_eq!(unit["cell"]["y"], u16::from_le_bytes([table[2], table[3]]));
    }
    for (supplier, address, value) in [
        (2, 0x621c, 40),
        (2, 0x6224, 16),
        (3, 0x68b4, 104),
        (3, 0x68bc, 224),
        (3, 0x68c4, 64),
        (3, 0x68cc, 32),
    ] {
        assert_eq!(
            decode_overlay_instruction(&sources[supplier].overlay, address),
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: value
            }
        );
    }
    // Relocated ready text must be sampled from the allocator's actual page
    // in every independent reader, rather than the old page-2 constant.
    let ready_pages = build
        .report
        .glyphs
        .iter()
        .filter(|g| g.surface == super::model::CharacterSelectTextureSurface::BattleReadyAtlas)
        .map(|g| g.texture_page_index)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ready_pages.len(), 1);
    let ready_word_x = 768 + 64 * i16::from(*ready_pages.first().unwrap());
    for (index, address) in [(2, 0x8658), (3, 0x9b2c), (4, 0xb304)] {
        assert_eq!(
            decode_overlay_instruction(&sources[index].overlay, address),
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0380
            }
        );
        assert_eq!(
            decode_overlay_instruction(&build.overlays[index], address),
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: ready_word_x
            }
        );
    }
    let tournament_overlay = &build.overlays[3];
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x1ed0),
        decode_overlay_instruction(&sources[3].overlay, 0x1ed0)
    );
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x1ef0),
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x03c0,
        }
    );
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x2024),
        decode_overlay_instruction(&sources[3].overlay, 0x2024)
    );
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x7edc),
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 279,
        }
    );
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x7ee4),
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 176,
        }
    );
    assert_eq!(
        decode_overlay_instruction(tournament_overlay, 0x7f78),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: -0x118
        }
    );

    for required_guard in [
        (
            "tournament certificate 446x393 slice layout",
            [0x0780, 0x07b0],
        ),
        ("tournament A-H team marker UV table", [0x07b0, 0x07d0]),
        (
            "completed bracket left champion texture page",
            [0x6990, 0x6998],
        ),
        ("completed bracket left champion sprite", [0x6a44, 0x6a80]),
        (
            "completed bracket right champion texture page",
            [0x6ac4, 0x6ac8],
        ),
        ("completed bracket right champion sprite", [0x6b54, 0x6b80]),
    ] {
        assert!(
            build.report.overlays[3]
                .source_guards
                .iter()
                .any(|guard| { (guard.role.as_str(), guard.byte_range) == required_guard })
        );
    }
    // All native text glyphs change. Their contribution must not touch
    // the unrelated MENU slash position, which other SELP writers may translate.
    for ((source, stored), report) in sources
        .iter()
        .zip(&build.records)
        .zip(&build.report.records)
    {
        let patched = decompress(stored, true).unwrap();
        // Check the composed bytes of every supplier, after all atlas writers.
        // A clear intermediate raster alone cannot rule out a later overwrite.
        let blank = build
            .report
            .glyphs
            .iter()
            .find(|glyph| {
                glyph.font_role == super::CharacterSelectFontRole::SelectHeading
                    && glyph.character == ' '
            })
            .unwrap();
        let blank_pixels =
            crate::tim::read_indexed_cell_in_prefix(&patched, blank.tim_offset, blank.cell)
                .unwrap();
        assert!(
            blank_pixels.iter().all(|pixel| *pixel == 0),
            "{} heading space contains ink",
            source.path
        );
        // Transparent cursor centres must not become storage for Korean glyphs.
        super::native_regions::validate_retained(&source.decoded, &patched).unwrap();
        let mut contaminated = source.decoded.clone();
        crate::tim::write_indexed_cell_in_prefix_with_report(
            &mut contaminated,
            super::texture_targets::SHARED_ATLAS_OFFSET,
            crate::tim::Cell {
                x: 532,
                y: 140,
                width: 1,
                height: 1,
            },
            &[14],
        )
        .unwrap();
        assert!(super::native_regions::validate_retained(&source.decoded, &contaminated).is_err());
        let offset = super::texture_targets::SHARED_ATLAS_OFFSET;
        let tim = crate::tim::parse_4bpp_prefix(&source.decoded[offset..]).unwrap();
        assert_eq!(
            &patched[offset..offset + tim.pixel_offset],
            &source.decoded[offset..offset + tim.pixel_offset]
        );
        let units = report.native_text["units"].as_array().unwrap();
        assert_eq!(
            units
                .iter()
                .map(|u| u["character"].as_str().unwrap())
                .collect::<String>(),
            "1234567890PH()/?ABCDEFGHABCDUOK!CPUMAN12345678901234567890ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz@#%?&()/+*><$!-=:"
        );
        for (index, unit) in units.iter().enumerate() {
            let cell: crate::tim::Cell = serde_json::from_value(unit["cell"].clone()).unwrap();
            let old =
                crate::tim::read_indexed_cell_in_prefix(&source.decoded, offset, cell).unwrap();
            let new = crate::tim::read_indexed_cell_in_prefix(&patched, offset, cell).unwrap();
            assert_ne!(old, new, "native text glyph {index} remained unchanged");
            assert_eq!(
                sha256_bytes(&new),
                unit["output_pixel_sha256"].as_str().unwrap()
            );
            assert!(new.iter().all(|p| [0, 3, 14].contains(p)));
        }
        let unrelated = crate::tim::Cell {
            x: 160,
            y: 100,
            width: 20,
            height: 20,
        };
        let mut native_text_only = source.decoded.clone();
        crate::menu_atlas::native_text::apply_selection_text(
            &source.decoded,
            &mut native_text_only,
            offset,
            &spec.fonts.shared_menu_numerals,
            &spec.fonts.character_select.label,
        )
        .unwrap();
        assert_eq!(
            crate::tim::read_indexed_cell_in_prefix(&source.decoded, offset, unrelated).unwrap(),
            crate::tim::read_indexed_cell_in_prefix(&native_text_only, offset, unrelated).unwrap()
        );
    }
    let cooperative_overlay = &build.overlays[4];
    let source_cooperative_overlay = &sources[4].overlay;
    assert_eq!(
        decode_overlay_instruction(cooperative_overlay, 0x83c0),
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x0300,
        }
    );
    assert_eq!(
        decode_overlay_instruction(cooperative_overlay, 0x83c4),
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: 0x0100,
        }
    );
    assert_eq!(
        &cooperative_overlay[0x3714..0x371a],
        &source_cooperative_overlay[0x3714..0x371a]
    );
    assert_eq!(
        &cooperative_overlay[0x3618..0x3624],
        &source_cooperative_overlay[0x3618..0x3624]
    );
    let patched_selp5 = decompress(&build.records[4], true).unwrap();
    let background_text_regions = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| {
            strip.font_role == super::CharacterSelectFontRole::CooperativeEmblemCharacter
        })
        .collect::<Vec<_>>();
    assert_eq!(background_text_regions.len(), 2);
    assert_eq!(
        background_text_regions
            .iter()
            .map(|strip| strip.text_selection)
            .collect::<Vec<_>>(),
        [
            super::model::CharacterSelectTextSelection::Character { index: 0 },
            super::model::CharacterSelectTextSelection::Character { index: 1 },
        ]
    );
    assert!(background_text_regions.iter().all(|strip| {
        strip.clear_index == 10 && strip.outline_index == 12 && strip.fill_index == 15
    }));
    for cell in [
        crate::tim::Cell {
            x: 2,
            y: 0,
            width: 31,
            height: 96,
        },
        crate::tim::Cell {
            x: 109,
            y: 0,
            width: 33,
            height: 96,
        },
    ] {
        let pixels = crate::tim::read_indexed_cell_in_prefix(&patched_selp5, 0x6800, cell).unwrap();
        assert!(pixels.iter().all(|pixel| [10, 12, 15].contains(pixel)));
    }
    assert_eq!(
        build.report.records[4]
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        [0x6800, 0x17800]
    );
    let preserved_emblem_cell = crate::tim::Cell {
        x: 33,
        y: 0,
        width: 76,
        height: 96,
    };
    assert_eq!(
        crate::tim::read_indexed_cell_in_prefix(
            &patched_selp5,
            0x6800,
            preserved_emblem_cell,
        )
        .unwrap(),
        crate::tim::read_indexed_cell_in_prefix(
            &sources[4].decoded,
            0x6800,
            preserved_emblem_cell,
        )
        .unwrap()
    );
    let source_portrait_tim = &sources[4].decoded[0x8800..];
    let portrait_tim_size = crate::tim::parse_4bpp_prefix(source_portrait_tim)
        .unwrap()
        .total_size;
    assert_eq!(
        &patched_selp5[0x8800..0x8800 + portrait_tim_size],
        &source_portrait_tim[..portrait_tim_size]
    );
    let source_disc = SupportedSourceDisc::open(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let source_auxiliary = load_character_select_auxiliary_source_from_disc(&source_disc).unwrap();
    for source_path in [
        "DAT2/AISYOU.TIZ",
        "DAT2/OVER.TIZ",
        "DAT2/OP01.BIZ",
        "DAT2/TITLE.BIN",
        "DAT2/TOROFY.BIZ",
    ] {
        assert!(
            build
                .report
                .auxiliary_records
                .iter()
                .any(|record| record.source_path == source_path)
        );
    }
    let source_op01 = source_auxiliary
        .iter()
        .find(|record| record.path == "DAT2/OP01.BIZ")
        .unwrap();
    let op01_index = build
        .report
        .auxiliary_records
        .iter()
        .position(|record| record.source_path == "DAT2/OP01.BIZ")
        .unwrap();
    let patched_op01 = &build.auxiliary_records[op01_index];
    let op01_report = &build.report.auxiliary_records[op01_index];
    let source_op01_stream = source_op01.compression_streams.first().unwrap();
    let op01_stream = op01_report.compressed_streams.first().unwrap();
    assert_eq!(source_op01.compression_streams.len(), 1);
    assert_eq!(op01_report.compressed_streams.len(), 1);
    assert_eq!(source_op01_stream.storage.stream_offset(), 0x800);
    assert_eq!(source_op01_stream.storage.slot_capacity(), 0x32800);
    assert_eq!(op01_stream.encoded_stream_offset, 0x800);
    assert_eq!(op01_stream.encoded_stream_capacity, 0x32800);
    assert_eq!(op01_stream.decoded_range, [0, 0x4d000]);
    assert!(op01_stream.changed);
    assert_eq!(&patched_op01[..4], &source_op01.stored[..4]);
    assert_eq!(
        u32::from_le_bytes(patched_op01[4..8].try_into().unwrap()) as usize,
        op01_stream.unpadded_stored_size
    );
    assert_eq!(&patched_op01[8..0x800], &source_op01.stored[8..0x800]);
    assert_eq!(&patched_op01[0x33000..], &source_op01.stored[0x33000..]);
    assert!(patched_op01[0x33000..].iter().any(|byte| *byte != 0));
    assert!(
        op01_report
            .changed_stored_byte_ranges
            .iter()
            .all(|[start, end]| {
                (4 <= *start && *end <= 8) || (0x800 <= *start && *end <= 0x33000)
            })
    );
    let patched_op01_stream_size =
        u32::from_le_bytes(patched_op01[4..8].try_into().unwrap()) as usize;
    let patched_op01_decoded = decompress(
        &patched_op01[0x800..0x800 + patched_op01_stream_size],
        false,
    )
    .unwrap();
    assert_eq!(patched_op01_decoded.len(), 0x4d000);
    assert_eq!(
        build.report.auxiliary_records[op01_index]
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        [0x3c800]
    );
    let source_title = source_auxiliary
        .iter()
        .find(|record| record.path == "DAT2/TITLE.BIN")
        .unwrap();
    let title_index = build
        .report
        .auxiliary_records
        .iter()
        .position(|record| record.source_path == "DAT2/TITLE.BIN")
        .unwrap();
    let patched_title = &build.auxiliary_records[title_index];
    let title_report = &build.report.auxiliary_records[title_index];
    let stored_offsets = [
        0x0800, 0x3000, 0x5000, 0x7000, 0x9000, 0xb000, 0xe000, 0x10000,
    ];
    let stored_capacities = [
        0x2800, 0x2000, 0x2000, 0x2000, 0x2000, 0x3000, 0x2000, 0x2000,
    ];
    let decoded_offsets = [
        0x00000, 0x04840, 0x08080, 0x0c0c0, 0x0f900, 0x13940, 0x19180, 0x1d1c0, 0x20a00,
    ];
    assert_eq!(source_title.compression_streams.len(), 8);
    assert_eq!(title_report.compressed_streams.len(), 8);
    for (index, ((source_stream, report_stream), (&stored_offset, &stored_capacity))) in
        source_title
            .compression_streams
            .iter()
            .zip(&title_report.compressed_streams)
            .zip(stored_offsets.iter().zip(&stored_capacities))
            .enumerate()
    {
        assert_eq!(source_stream.stream_index, index);
        assert_eq!(report_stream.stream_index, index);
        assert_eq!(source_stream.storage.stream_offset(), stored_offset);
        assert_eq!(source_stream.storage.slot_capacity(), stored_capacity);
        assert_eq!(report_stream.encoded_stream_offset, stored_offset);
        assert_eq!(report_stream.encoded_stream_capacity, stored_capacity);
        assert_eq!(
            report_stream.decoded_range,
            [decoded_offsets[index], decoded_offsets[index + 1]]
        );
        assert!(report_stream.changed);
        assert_eq!(
            &patched_title[index * 8..index * 8 + 4],
            &source_title.stored[index * 8..index * 8 + 4]
        );
        assert_eq!(
            u32::from_le_bytes(
                patched_title[index * 8 + 4..index * 8 + 8]
                    .try_into()
                    .unwrap()
            ) as usize,
            report_stream.unpadded_stored_size
        );
        assert!(
            patched_title[stored_offset + report_stream.unpadded_stored_size
                ..stored_offset + stored_capacity]
                .iter()
                .all(|byte| *byte == 0)
        );
    }
    assert_eq!(
        &patched_title[0x40..0x800],
        &source_title.stored[0x40..0x800]
    );
    assert!(
        title_report
            .changed_stored_byte_ranges
            .iter()
            .all(|[start, end]| {
                (0..8).any(|index| {
                    let size_field_offset = index * 8 + 4;
                    (size_field_offset <= *start && *end <= size_field_offset + 4)
                        || (stored_offsets[index] <= *start
                            && *end <= stored_offsets[index] + stored_capacities[index])
                })
            })
    );
    let patched_title_decoded = super::compressed_record::decode_built_record_streams(
        patched_title,
        title_report
            .compressed_streams
            .iter()
            .map(|stream| (stream.encoded_stream_offset, stream.encoded_stream_capacity)),
    )
    .unwrap();
    assert_eq!(patched_title_decoded.len(), 0x20a00);
    assert_eq!(
        sha256_bytes(&patched_title_decoded),
        title_report.patched_decoded_sha256
    );
    assert_eq!(
        title_report
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        decoded_offsets[..8]
    );
    let episode_card_strips = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| strip.font_role == super::CharacterSelectFontRole::SoloEpisodeCard)
        .collect::<Vec<_>>();
    assert_eq!(episode_card_strips.len(), 30);
    assert!(episode_card_strips.iter().all(|strip| {
        strip.surface == super::model::CharacterSelectTextureSurface::SoloEpisodeCardAtlas
            && strip.clear_index == 15
            && strip.outline_index == 14
            && strip.fill_index == 1
            && strip.changed_decoded_byte_count > 0
    }));
    let translated_episode_cards = episode_card_strips
        .iter()
        .flat_map(|strip| strip.source_ui_ids.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        translated_episode_cards,
        (1..=8)
            .map(|episode| format!("solo_episode_card_{episode}"))
            .collect::<std::collections::BTreeSet<_>>()
    );
    for strip in episode_card_strips {
        let source_pixels = crate::tim::read_indexed_cell_in_prefix(
            &source_title.decoded,
            strip.tim_offset,
            strip.cell,
        )
        .unwrap();
        let patched_pixels = crate::tim::read_indexed_cell_in_prefix(
            &patched_title_decoded,
            strip.tim_offset,
            strip.cell,
        )
        .unwrap();
        assert_ne!(patched_pixels, source_pixels);
        assert!(
            patched_pixels
                .iter()
                .all(|pixel| [1, 14, 15].contains(pixel))
        );
    }
    let source_aisyou = source_auxiliary
        .iter()
        .find(|record| record.path == "DAT2/AISYOU.TIZ")
        .unwrap();
    let aisyou_index = build
        .report
        .auxiliary_records
        .iter()
        .position(|record| record.source_path == "DAT2/AISYOU.TIZ")
        .unwrap();
    let patched_aisyou = decompress(&build.auxiliary_records[aisyou_index], true).unwrap();
    assert_eq!(
        build.report.auxiliary_records[aisyou_index]
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        [0, 0x20800, 0x45000]
    );
    let mode_menu_offset = 0x45000;
    let mode_menu_size = crate::tim::parse_4bpp_prefix(&source_aisyou.decoded[mode_menu_offset..])
        .unwrap()
        .total_size;
    assert_ne!(
        &patched_aisyou[mode_menu_offset..mode_menu_offset + mode_menu_size],
        &source_aisyou.decoded[mode_menu_offset..mode_menu_offset + mode_menu_size]
    );
    let source_over = source_auxiliary
        .iter()
        .find(|record| record.path == "DAT2/OVER.TIZ")
        .unwrap();
    let over_index = build
        .report
        .auxiliary_records
        .iter()
        .position(|record| record.source_path == "DAT2/OVER.TIZ")
        .unwrap();
    let patched_over = decompress(&build.auxiliary_records[over_index], true).unwrap();
    assert_eq!(
        build.report.auxiliary_records[over_index]
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        [0]
    );
    let solo_prompt_strips = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| strip.font_role == super::CharacterSelectFontRole::SoloStatePrompt)
        .collect::<Vec<_>>();
    assert_eq!(solo_prompt_strips.len(), 3);
    assert!(solo_prompt_strips.iter().all(|strip| {
        strip.surface == super::model::CharacterSelectTextureSurface::SoloStatePromptAtlas
            && strip.clear_index == 0
            && strip.outline_index == 7
            && strip.fill_index == 3
    }));
    for cell in [
        crate::tim::Cell {
            x: 0,
            y: 100,
            width: 88,
            height: 12,
        },
        crate::tim::Cell {
            x: 0,
            y: 116,
            width: 112,
            height: 12,
        },
        crate::tim::Cell {
            x: 0,
            y: 196,
            width: 80,
            height: 12,
        },
    ] {
        let source_pixels =
            crate::tim::read_indexed_cell_in_prefix(&source_over.decoded, 0, cell).unwrap();
        let patched_pixels =
            crate::tim::read_indexed_cell_in_prefix(&patched_over, 0, cell).unwrap();
        assert_ne!(patched_pixels, source_pixels);
        assert!(patched_pixels.iter().all(|pixel| [0, 3, 7].contains(pixel)));
    }
    let torofy_index = build
        .report
        .auxiliary_records
        .iter()
        .position(|record| record.source_path == "DAT2/TOROFY.BIZ")
        .unwrap();
    let source_torofy = source_auxiliary
        .iter()
        .find(|record| record.path == "DAT2/TOROFY.BIZ")
        .unwrap();
    let patched_torofy = decompress(&build.auxiliary_records[torofy_index], true).unwrap();
    assert_eq!(
        build.report.auxiliary_records[torofy_index]
            .target_tims
            .iter()
            .map(|tim| tim.target_tim_offset)
            .collect::<Vec<_>>(),
        [0x00000, 0x4a000]
    );
    assert_eq!(build.report.source_ink_cleanup_count, 18);
    assert_eq!(
        build
            .report
            .source_ink_cleanups
            .iter()
            .map(|cleanup| cleanup.source_ink_count)
            .sum::<usize>(),
        2 * (1_173 + 664 + 998 + 540 + 1_251 + 1_966 + 1_258 + 860 + 1_147)
    );
    let certificate_strips = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| {
            strip.surface == super::model::CharacterSelectTextureSurface::TournamentCertificate
        })
        .collect::<Vec<_>>();
    assert_eq!(certificate_strips.len(), 16);
    assert!(certificate_strips.iter().all(|strip| {
        strip.text_flow == super::model::CharacterSelectTextFlow::Horizontal
            && strip.write_mode
                == super::model::CharacterSelectFixedStripWriteMode::OverlayNonClearPixels
            && strip.clear_index == 15
            && strip.outline_index == 14
            && strip.fill_index == 1
    }));
    let bracket_strips = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| strip.font_role == super::CharacterSelectFontRole::TournamentBracketLabel)
        .collect::<Vec<_>>();
    assert_eq!(bracket_strips.len(), 2);
    assert_eq!(
        bracket_strips
            .iter()
            .map(|strip| (strip.text_selection, strip.cell))
            .collect::<Vec<_>>(),
        [
            (
                super::model::CharacterSelectTextSelection::Character { index: 0 },
                crate::tim::Cell {
                    x: 328,
                    y: 224,
                    width: 32,
                    height: 32,
                },
            ),
            (
                super::model::CharacterSelectTextSelection::Character { index: 1 },
                crate::tim::Cell {
                    x: 256,
                    y: 0,
                    width: 32,
                    height: 32,
                },
            ),
        ]
    );
    let dynamic_team_atlas = crate::tim::Cell {
        x: 448,
        y: 0,
        width: 32,
        height: 128,
    };
    for offset in [0x00000, 0x4a000] {
        assert_eq!(
            crate::tim::read_indexed_cell_in_prefix(&patched_torofy, offset, dynamic_team_atlas,)
                .unwrap(),
            crate::tim::read_indexed_cell_in_prefix(
                &source_torofy.decoded,
                offset,
                dynamic_team_atlas,
            )
            .unwrap()
        );
    }
    for untouched_offset in [0x40800, 0x49000] {
        let size = crate::tim::parse_4bpp_prefix(&source_torofy.decoded[untouched_offset..])
            .unwrap()
            .total_size;
        assert_eq!(
            &patched_torofy[untouched_offset..untouched_offset + size],
            &source_torofy.decoded[untouched_offset..untouched_offset + size]
        );
    }
    let fixed_certificate = crate::tim::Cell {
        x: 0,
        y: 0,
        width: 446,
        height: 393,
    };
    for offset in [0, 0x4a000] {
        let before = crate::tim::read_indexed_cell_in_prefix(
            &source_torofy.decoded,
            offset,
            fixed_certificate,
        )
        .unwrap();
        let after =
            crate::tim::read_indexed_cell_in_prefix(&patched_torofy, offset, fixed_certificate)
                .unwrap();
        assert!(
            before
                .iter()
                .zip(&after)
                .all(|(before, after)| *after != 0 || *before == 0),
            "certificate lettering must not punch transparent holes into the paper"
        );
    }
    assert_eq!(
        crate::tim::read_indexed_cell_in_prefix(&patched_torofy, 0, fixed_certificate).unwrap(),
        crate::tim::read_indexed_cell_in_prefix(&patched_torofy, 0x4a000, fixed_certificate)
            .unwrap()
    );
    std::fs::remove_dir_all(output).unwrap();
}

fn decode_overlay_instruction(source: &[u8], offset: usize) -> Instruction {
    let word = u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap());
    decode(word, 0x800a_2000 + offset as u32).unwrap()
}
