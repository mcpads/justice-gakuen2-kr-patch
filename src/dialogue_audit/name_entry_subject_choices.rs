use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;

pub(super) fn subject_choices_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "subject-choices",
        translation_file: "subject-choices.json",
        translation_kind: "Justice Gakuen 2 source-bound enrollment subject-choice graphic translation",
        build_kind: "Justice Gakuen 2 enrollment subject-choice fixed-graphic build",
        target: "enrollment subject-choice Korean text",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}
