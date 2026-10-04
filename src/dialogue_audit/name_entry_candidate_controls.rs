use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;

pub(super) fn candidate_controls_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "candidate-controls",
        translation_file: "candidate-controls.json",
        translation_kind: "Justice Gakuen 2 source-bound name-entry candidate-control graphic translation",
        build_kind: "Justice Gakuen 2 name-entry candidate-control fixed-graphic build",
        target: "name-entry candidate-control Korean text",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}
