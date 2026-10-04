use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;

pub(super) fn application_form_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "application-form",
        translation_file: "application-form.json",
        translation_kind: "Justice Gakuen 2 source-bound application-form graphic translation",
        build_kind: "Justice Gakuen 2 application-form fixed-graphic build",
        target: "enrollment application-form Korean text",
        tim_offset: 0,
        image_x: 512,
        image_y: 0,
        image_width: 448,
        image_height: 464,
        clut_width: 16,
        clut_height: 1,
    }
}
