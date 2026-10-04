//! Small, reviewed source-region, semantic-sink, and consumer-edge populations.

use psx_r3000a::{Instruction, Register};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticConsumerSinkProfile {
    pub(crate) id: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) image_path: &'static str,
    pub(crate) runtime_address: u32,
    pub(crate) additional_runtime_addresses: &'static [u32],
    pub(crate) disposition: StaticConsumerSinkDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StaticConsumerSinkDisposition {
    ActiveConsumer,
    DormantCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticConsumerEdgeProfile {
    pub(crate) id: &'static str,
    pub(crate) source_record_path: &'static str,
    pub(crate) source_region_id: &'static str,
    pub(crate) source_member_indices: &'static [u8],
    pub(crate) relationship: &'static str,
    pub(crate) sink_id: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticConsumerSourceRegionProfile {
    pub(crate) source_record_path: &'static str,
    pub(crate) source_region_id: &'static str,
    pub(crate) source_member_indices: &'static [u8],
    pub(crate) source_catalog_index: u16,
    pub(crate) unbound_consumer_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticPointerRunProfile {
    pub(crate) id: &'static str,
    pub(crate) image_path: &'static str,
    pub(crate) pointer_table_runtime_address: u32,
    pub(crate) pointer_count: usize,
    pub(crate) target_arena_runtime_address: u32,
    pub(crate) target_arena_size: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticPointerRunNonAddressFlowProfile {
    pub(crate) id: &'static str,
    pub(crate) pointer_run_id: &'static str,
    pub(crate) image_path: &'static str,
    pub(crate) analysis_state_budget: usize,
    pub(crate) seed_instruction_offset: usize,
    pub(crate) expected_seed_instruction: Instruction,
    pub(crate) scalar_load_instruction_offset: usize,
    pub(crate) expected_scalar_load_instruction: Instruction,
    pub(crate) scalar_storage_runtime_address: u32,
    pub(crate) expected_frontier_instructions: &'static [(usize, Instruction)],
}

pub(crate) const STATIC_POINTER_RUN_PROFILES: &[StaticPointerRunProfile] = &[
    StaticPointerRunProfile {
        id: "sikeng21_opaque_pointer_run_077c",
        image_path: "DAT1/SIKENG21.BIN",
        pointer_table_runtime_address: 0x8015_277c,
        pointer_count: 46,
        target_arena_runtime_address: 0x8015_2438,
        target_arena_size: 836,
    },
    StaticPointerRunProfile {
        id: "sikeng22_opaque_pointer_run_077c",
        image_path: "DAT1/SIKENG22.BIN",
        pointer_table_runtime_address: 0x8017_a77c,
        pointer_count: 46,
        target_arena_runtime_address: 0x8017_a438,
        target_arena_size: 836,
    },
];

pub(crate) const STATIC_POINTER_RUN_NON_ADDRESS_FLOW_PROFILES:
    &[StaticPointerRunNonAddressFlowProfile] = &[
    StaticPointerRunNonAddressFlowProfile {
        id: "sikeng21_sprite_coordinate_record_loop",
        pointer_run_id: "sikeng21_opaque_pointer_run_077c",
        image_path: "DAT1/SIKENG21.BIN",
        analysis_state_budget: 262_144,
        seed_instruction_offset: 0x1f80,
        expected_seed_instruction: Instruction::Lui {
            rt: Register::A3,
            immediate: 0x8015,
        },
        scalar_load_instruction_offset: 0x1f84,
        expected_scalar_load_instruction: Instruction::Lhu {
            rt: Register::A3,
            base: Register::A3,
            offset: 0x20b8,
        },
        scalar_storage_runtime_address: 0x8015_20b8,
        expected_frontier_instructions: &[
            (
                0x0ef8,
                Instruction::Addiu {
                    rt: Register::S2,
                    rs: Register::S2,
                    immediate: 1,
                },
            ),
            (
                0x1080,
                Instruction::Lw {
                    rt: Register::FP,
                    base: Register::SP,
                    offset: 0x38,
                },
            ),
        ],
    },
    StaticPointerRunNonAddressFlowProfile {
        id: "sikeng22_sprite_coordinate_record_loop",
        pointer_run_id: "sikeng22_opaque_pointer_run_077c",
        image_path: "DAT1/SIKENG22.BIN",
        analysis_state_budget: 262_144,
        seed_instruction_offset: 0x1f80,
        expected_seed_instruction: Instruction::Lui {
            rt: Register::A3,
            immediate: 0x8018,
        },
        scalar_load_instruction_offset: 0x1f84,
        expected_scalar_load_instruction: Instruction::Lhu {
            rt: Register::A3,
            base: Register::A3,
            offset: -0x5f48,
        },
        scalar_storage_runtime_address: 0x8017_a0b8,
        expected_frontier_instructions: &[
            (
                0x0ef8,
                Instruction::Addiu {
                    rt: Register::S2,
                    rs: Register::S2,
                    immediate: 1,
                },
            ),
            (
                0x1080,
                Instruction::Lw {
                    rt: Register::FP,
                    base: Register::SP,
                    offset: 0x38,
                },
            ),
        ],
    },
];

pub(crate) const MAIN_DISC_RECORD_LOADER_ADDRESS: u32 = 0x8001_5414;
pub(crate) const KANRI_MENU_TEXT_RENDERER_ADDRESS: u32 = 0x800a_3b3c;
pub(crate) const MGTIT_MENU_TEXT_RENDERER_ADDRESS: u32 = 0x800a_50b4;
pub(crate) const NEWOPT_DIRECT_STRING_WRITER_ADDRESS: u32 = 0x800a_4fe0;

pub(crate) const STATIC_CONSUMER_SINK_PROFILES: &[StaticConsumerSinkProfile] = &[
    sink(
        "main_disc_record_loader",
        "disc_record_loader",
        "SLPS_021.20",
        MAIN_DISC_RECORD_LOADER_ADDRESS,
    ),
    sink(
        "main_lz16_decoder",
        "compressed_record_decoder",
        "SLPS_021.20",
        0x8001_5eac,
    ),
    sink("main_tim_parser", "tim_parser", "SLPS_021.20", 0x8001_5fcc),
    sink(
        "main_gpu_image_uploader",
        "gpu_image_upload",
        "SLPS_021.20",
        0x8006_2630,
    ),
    sink(
        "kanri_menu_text_renderer",
        "menu_text_renderer",
        "DAT1/KANRI.BIN",
        KANRI_MENU_TEXT_RENDERER_ADDRESS,
    ),
    sink(
        "mgtit_menu_text_renderer",
        "menu_text_renderer",
        "DAT1/MGTIT.BIN",
        MGTIT_MENU_TEXT_RENDERER_ADDRESS,
    ),
    sink(
        "newopt_direct_string_writer",
        "menu_text_renderer",
        "DAT1/NEWOPT.BIN",
        NEWOPT_DIRECT_STRING_WRITER_ADDRESS,
    ),
    sink_group(
        "minisel_gorin_primitive_rendering_pipeline",
        "primitive_record_renderer_pipeline",
        "DAT1/MINISEL.BIN",
        0x800a_2cd0,
        &[0x800a_36b8, 0x800a_36e8],
    ),
    sink(
        "siken_testmj_member_selector",
        "archive_member_selector_call_site",
        "DAT1/SIKEN.BIN",
        0x800a_4b4c,
    ),
    sink(
        "siken2_testmj_member_selector",
        "archive_member_selector_call_site",
        "DAT1/SIKEN2.BIN",
        0x800a_413c,
    ),
    sink(
        "siken_battle_subject_title_renderer",
        "sprite_strip_renderer",
        "DAT1/SIKEN.BIN",
        0x800a_7768,
    ),
    sink(
        "siken_basics_record_sheet_renderer",
        "full_source_texture_renderer",
        "DAT1/SIKEN.BIN",
        0x800a_8de8,
    ),
    sink(
        "siken_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKEN.BIN",
        0x800a_6204,
    ),
    sink(
        "siken2_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKEN2.BIN",
        0x800a_592c,
    ),
    sink_group(
        "siken2_siken20_indexed_member_producer",
        "indexed_member_load_and_first_tim_upload_pipeline",
        "DAT1/SIKEN2.BIN",
        0x800a_56dc,
        &[0x800a_56f8],
    ),
    sink(
        "sikeng21_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKENG21.BIN",
        0x8015_4e8c,
    ),
    sink(
        "sikeng22_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKENG22.BIN",
        0x8017_ce8c,
    ),
    sink(
        "sikengo1_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKENGO1.BIN",
        0x8015_3998,
    ),
    sink(
        "sikengo2_practical_result_catalog_renderer",
        "catalog_descriptor_renderer",
        "DAT1/SIKENGO2.BIN",
        0x8017_b998,
    ),
    sink(
        "sikeng21_practical_result_outcome_badge_renderer",
        "direct_sprite_selector_renderer",
        "DAT1/SIKENG21.BIN",
        0x8015_3e9c,
    ),
    sink(
        "sikeng22_practical_result_outcome_badge_renderer",
        "direct_sprite_selector_renderer",
        "DAT1/SIKENG22.BIN",
        0x8017_be9c,
    ),
    sink(
        "sikengo1_practical_result_outcome_badge_renderer",
        "direct_sprite_selector_renderer",
        "DAT1/SIKENGO1.BIN",
        0x8015_3fcc,
    ),
    sink(
        "sikengo2_practical_result_outcome_badge_renderer",
        "direct_sprite_selector_renderer",
        "DAT1/SIKENGO2.BIN",
        0x8017_bfcc,
    ),
    sink_group(
        "sikeng21_practical_result_direct_numeric_sites",
        "direct_numeric_sprite_site_group",
        "DAT1/SIKENG21.BIN",
        0x8015_32f8,
        &[
            0x8015_35b0,
            0x8015_3794,
            0x8015_3938,
            0x8015_3af4,
            0x8015_3c9c,
        ],
    ),
    sink_group(
        "sikeng22_practical_result_direct_numeric_sites",
        "direct_numeric_sprite_site_group",
        "DAT1/SIKENG22.BIN",
        0x8017_b2f8,
        &[
            0x8017_b5b0,
            0x8017_b794,
            0x8017_b938,
            0x8017_baf4,
            0x8017_bc9c,
        ],
    ),
    sink(
        "sikengo1_practical_result_numeric_lookup_renderer",
        "numeric_lookup_renderer",
        "DAT1/SIKENGO1.BIN",
        0x8015_43e0,
    ),
    sink(
        "sikengo2_practical_result_numeric_lookup_renderer",
        "numeric_lookup_renderer",
        "DAT1/SIKENGO2.BIN",
        0x8017_c3e0,
    ),
    sink(
        "sikeng21_practical_result_static_tile_stream_renderer",
        "static_selector_tile_stream_renderer",
        "DAT1/SIKENG21.BIN",
        0x8015_6178,
    ),
    sink(
        "sikeng22_practical_result_static_tile_stream_renderer",
        "static_selector_tile_stream_renderer",
        "DAT1/SIKENG22.BIN",
        0x8017_e178,
    ),
    sink(
        "sikengo1_practical_result_static_tile_stream_renderer",
        "static_matrix_tile_stream_renderer",
        "DAT1/SIKENGO1.BIN",
        0x8015_6b60,
    ),
    sink(
        "sikengo2_practical_result_static_tile_stream_renderer",
        "static_matrix_tile_stream_renderer",
        "DAT1/SIKENGO2.BIN",
        0x8017_eb60,
    ),
    candidate_sink(
        "sikeng21_practical_result_dynamic_tile_stream_renderer",
        "dynamic_config_tile_stream_renderer",
        "DAT1/SIKENG21.BIN",
        0x8015_65d8,
    ),
    candidate_sink(
        "sikeng22_practical_result_dynamic_tile_stream_renderer",
        "dynamic_config_tile_stream_renderer",
        "DAT1/SIKENG22.BIN",
        0x8017_e5d8,
    ),
    candidate_sink(
        "sikengo1_practical_result_dynamic_tile_stream_renderer",
        "dynamic_config_tile_stream_renderer",
        "DAT1/SIKENGO1.BIN",
        0x8015_6fd0,
    ),
    candidate_sink(
        "sikengo2_practical_result_dynamic_tile_stream_renderer",
        "dynamic_config_tile_stream_renderer",
        "DAT1/SIKENGO2.BIN",
        0x8017_efd0,
    ),
];

const BASIC_PRACTICAL_MEMBER_INDICES: &[u8] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29,
];
const EXAM_PRACTICAL_MEMBER_INDICES: &[u8] = &[30, 31, 32];

pub(crate) const STATIC_CONSUMER_SOURCE_REGION_PROFILES: &[StaticConsumerSourceRegionProfile] = &[
    source_region(
        "DAT2/TESTMJ.TIZ",
        "primary_tim_members_0_through_29",
        BASIC_PRACTICAL_MEMBER_INDICES,
        695,
        None,
    ),
    source_region(
        "DAT2/TESTMJ.TIZ",
        "primary_tim_members_30_31_32",
        EXAM_PRACTICAL_MEMBER_INDICES,
        695,
        None,
    ),
    source_region(
        "DAT2/TESTMJ.TIZ",
        "battle_subject_title_strips_in_members_0_through_29",
        BASIC_PRACTICAL_MEMBER_INDICES,
        695,
        None,
    ),
    source_region(
        "DAT2/SIKEN1.BIZ",
        "tim_0x55000_indexed_8bpp",
        &[],
        690,
        None,
    ),
    source_region(
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        692,
        None,
    ),
    source_region(
        "DAT2/SIKEN20.BIZ",
        "indexed_members_0_1_tim_0x00000_indexed_4bpp",
        &[0, 1],
        693,
        None,
    ),
    source_region(
        "DAT2/MINITTL0.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        60,
        None,
    ),
    source_region(
        "DAT2/MINITTL1.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        61,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        62,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL3.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        63,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL4.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        64,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL5.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        65,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL6.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        66,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL7.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        67,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL8.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        68,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTL9.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        69,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTLA.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        70,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTLB.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        71,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
    source_region(
        "DAT2/MINITTLC.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        72,
        Some("runtime_route_and_variant_semantics_not_yet_source_bound"),
    ),
];

pub(crate) const STATIC_CONSUMER_EDGE_PROFILES: &[StaticConsumerEdgeProfile] = &[
    edge(
        "testmj_basic_member_selection",
        "DAT2/TESTMJ.TIZ",
        "primary_tim_members_0_through_29",
        BASIC_PRACTICAL_MEMBER_INDICES,
        "selects_archive_member",
        "siken_testmj_member_selector",
    ),
    edge(
        "testmj_exam_member_selection",
        "DAT2/TESTMJ.TIZ",
        "primary_tim_members_30_31_32",
        EXAM_PRACTICAL_MEMBER_INDICES,
        "selects_archive_member",
        "siken2_testmj_member_selector",
    ),
    edge(
        "testmj_battle_subject_title_rendering",
        "DAT2/TESTMJ.TIZ",
        "battle_subject_title_strips_in_members_0_through_29",
        BASIC_PRACTICAL_MEMBER_INDICES,
        "renders_member_texture_strip",
        "siken_battle_subject_title_renderer",
    ),
    edge(
        "minittl0_gorin_primitive_rendering",
        "DAT2/MINITTL0.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "supplies_gorin_primitive_texture",
        "minisel_gorin_primitive_rendering_pipeline",
    ),
    edge(
        "siken_basics_record_sheet_rendering",
        "DAT2/SIKEN1.BIZ",
        "tim_0x55000_indexed_8bpp",
        &[],
        "renders_full_source_texture",
        "siken_basics_record_sheet_renderer",
    ),
    edge(
        "siken_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "siken_practical_result_catalog_renderer",
    ),
    edge(
        "siken2_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "siken2_practical_result_catalog_renderer",
    ),
    edge(
        "siken20_indexed_result_atlas_production",
        "DAT2/SIKEN20.BIZ",
        "indexed_members_0_1_tim_0x00000_indexed_4bpp",
        &[0, 1],
        "loads_selected_member_and_uploads_first_tim",
        "siken2_siken20_indexed_member_producer",
    ),
    edge(
        "sikeng21_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "sikeng21_practical_result_catalog_renderer",
    ),
    edge(
        "sikeng22_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "sikeng22_practical_result_catalog_renderer",
    ),
    edge(
        "sikengo1_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "sikengo1_practical_result_catalog_renderer",
    ),
    edge(
        "sikengo2_result_atlas_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_source_atlas_domain",
        "sikengo2_practical_result_catalog_renderer",
    ),
    edge(
        "sikeng21_result_outcome_badge_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_outcome_badge_selector",
        "sikeng21_practical_result_outcome_badge_renderer",
    ),
    edge(
        "sikeng22_result_outcome_badge_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_outcome_badge_selector",
        "sikeng22_practical_result_outcome_badge_renderer",
    ),
    edge(
        "sikengo1_result_outcome_badge_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_outcome_badge_selector",
        "sikengo1_practical_result_outcome_badge_renderer",
    ),
    edge(
        "sikengo2_result_outcome_badge_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_outcome_badge_selector",
        "sikengo2_practical_result_outcome_badge_renderer",
    ),
    edge(
        "sikeng21_result_direct_numeric_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_direct_numeric_sprite_sites",
        "sikeng21_practical_result_direct_numeric_sites",
    ),
    edge(
        "sikeng22_result_direct_numeric_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_direct_numeric_sprite_sites",
        "sikeng22_practical_result_direct_numeric_sites",
    ),
    edge(
        "sikengo1_result_numeric_lookup_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_numeric_lookup",
        "sikengo1_practical_result_numeric_lookup_renderer",
    ),
    edge(
        "sikengo2_result_numeric_lookup_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_numeric_lookup",
        "sikengo2_practical_result_numeric_lookup_renderer",
    ),
    edge(
        "sikeng21_result_static_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_static_tile_stream",
        "sikeng21_practical_result_static_tile_stream_renderer",
    ),
    edge(
        "sikeng22_result_static_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_static_tile_stream",
        "sikeng22_practical_result_static_tile_stream_renderer",
    ),
    edge(
        "sikengo1_result_static_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_static_tile_stream",
        "sikengo1_practical_result_static_tile_stream_renderer",
    ),
    edge(
        "sikengo2_result_static_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_static_tile_stream",
        "sikengo2_practical_result_static_tile_stream_renderer",
    ),
    edge(
        "sikeng21_result_dynamic_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_dynamic_config_tile_stream",
        "sikeng21_practical_result_dynamic_tile_stream_renderer",
    ),
    edge(
        "sikeng22_result_dynamic_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_dynamic_config_tile_stream",
        "sikeng22_practical_result_dynamic_tile_stream_renderer",
    ),
    edge(
        "sikengo1_result_dynamic_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_dynamic_config_tile_stream",
        "sikengo1_practical_result_dynamic_tile_stream_renderer",
    ),
    edge(
        "sikengo2_result_dynamic_tile_stream_rendering",
        "DAT2/SIKEN2.BIZ",
        "tim_0x00000_indexed_4bpp",
        &[],
        "renders_dynamic_config_tile_stream",
        "sikengo2_practical_result_dynamic_tile_stream_renderer",
    ),
];

const fn sink(
    id: &'static str,
    kind: &'static str,
    image_path: &'static str,
    runtime_address: u32,
) -> StaticConsumerSinkProfile {
    StaticConsumerSinkProfile {
        id,
        kind,
        image_path,
        runtime_address,
        additional_runtime_addresses: &[],
        disposition: StaticConsumerSinkDisposition::ActiveConsumer,
    }
}

const fn candidate_sink(
    id: &'static str,
    kind: &'static str,
    image_path: &'static str,
    runtime_address: u32,
) -> StaticConsumerSinkProfile {
    StaticConsumerSinkProfile {
        id,
        kind,
        image_path,
        runtime_address,
        additional_runtime_addresses: &[],
        disposition: StaticConsumerSinkDisposition::DormantCandidate,
    }
}

const fn sink_group(
    id: &'static str,
    kind: &'static str,
    image_path: &'static str,
    runtime_address: u32,
    additional_runtime_addresses: &'static [u32],
) -> StaticConsumerSinkProfile {
    StaticConsumerSinkProfile {
        id,
        kind,
        image_path,
        runtime_address,
        additional_runtime_addresses,
        disposition: StaticConsumerSinkDisposition::ActiveConsumer,
    }
}

const fn edge(
    id: &'static str,
    source_record_path: &'static str,
    source_region_id: &'static str,
    source_member_indices: &'static [u8],
    relationship: &'static str,
    sink_id: &'static str,
) -> StaticConsumerEdgeProfile {
    StaticConsumerEdgeProfile {
        id,
        source_record_path,
        source_region_id,
        source_member_indices,
        relationship,
        sink_id,
    }
}

const fn source_region(
    source_record_path: &'static str,
    source_region_id: &'static str,
    source_member_indices: &'static [u8],
    source_catalog_index: u16,
    unbound_consumer_reason: Option<&'static str>,
) -> StaticConsumerSourceRegionProfile {
    StaticConsumerSourceRegionProfile {
        source_record_path,
        source_region_id,
        source_member_indices,
        source_catalog_index,
        unbound_consumer_reason,
    }
}
