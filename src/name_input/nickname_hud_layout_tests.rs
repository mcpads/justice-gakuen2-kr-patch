use super::{NameInputRuntimePackLayout, NicknameHudGlyphStyle, plan_nickname_hud_glyph_layout};

#[test]
fn nickname_hud_scale_enlarges_only_the_persistent_consumer_box() {
    let runtime_pack = runtime_pack_with_crop([4, 6, 12, 13]);

    let layout = plan_nickname_hud_glyph_layout(
        &runtime_pack,
        NicknameHudGlyphStyle {
            scale_percent: 115,
            vertical_shift_px: -1,
        },
    )
    .unwrap();

    assert_eq!(layout.source_bounds, [3, 5, 14, 15]);
    assert_eq!(layout.target_bounds, [2, 2, 16, 17]);
    assert_eq!(runtime_pack.crop, [4, 6, 12, 13]);
}

#[test]
fn nickname_hud_scale_rejects_a_target_that_would_clip() {
    let runtime_pack = runtime_pack_with_crop([4, 6, 12, 13]);

    let result = plan_nickname_hud_glyph_layout(
        &runtime_pack,
        NicknameHudGlyphStyle {
            scale_percent: 150,
            vertical_shift_px: 0,
        },
    );

    assert!(result.is_err());
}

fn runtime_pack_with_crop(crop: [usize; 4]) -> NameInputRuntimePackLayout {
    NameInputRuntimePackLayout {
        crop,
        occupied_coordinate_count: 0,
        bytes_per_component_mask: 0,
        coordinate_membership_byte_range: [0, 0],
        repertoire_membership_byte_range: [0, 0],
        no_final_membership_byte_range: [0, 0],
        final_bearing_membership_byte_range: [0, 0],
        final_membership_byte_range: [0, 0],
        no_final_rank_prefix_byte_range: [0, 0],
        final_bearing_rank_prefix_byte_range: [0, 0],
        final_rank_prefix_byte_range: [0, 0],
        runtime_coordinate_list_byte_count: 0,
        component_mask_byte_range: [0, 0],
        no_final_base_component_count: 0,
        final_bearing_base_component_count: 0,
        final_component_count: 0,
        pack_bytes: 0,
    }
}
