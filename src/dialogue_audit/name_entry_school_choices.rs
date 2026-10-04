use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;

pub(super) fn school_choices_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "school-choices",
        translation_file: "school-choices.json",
        translation_kind: "Justice Gakuen 2 source-bound enrollment school-choice graphic translation",
        build_kind: "Justice Gakuen 2 enrollment school-choice fixed-graphic build",
        target: "enrollment school-choice Korean text",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}
