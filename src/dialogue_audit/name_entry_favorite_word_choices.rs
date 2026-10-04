use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;

pub(super) fn favorite_word_choices_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "favorite-word-choices",
        translation_file: "favorite-word-choices.json",
        translation_kind: "Justice Gakuen 2 source-bound enrollment favorite-word graphic translation",
        build_kind: "Justice Gakuen 2 enrollment favorite-word fixed-graphic build",
        target: "enrollment favorite-word Korean text",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}
