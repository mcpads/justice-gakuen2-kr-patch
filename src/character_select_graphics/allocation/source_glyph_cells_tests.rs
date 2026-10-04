use super::*;
use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectSourceCellOccupancy, CharacterSelectTextureSurface,
};
use crate::tim::Cell;

#[test]
fn source_glyph_conflicts_follow_physical_record_ownership() {
    let source_cell = CharacterSelectSourceGlyphCell {
        physical_cell_id: "shared-source-glyph-page-3-row-8-column-1".to_string(),
        surface: CharacterSelectTextureSurface::StageLabelAtlas,
        tim_offset: SHARED_ATLAS_OFFSET,
        texture_page_index: 3,
        texture_uv: [20, 160],
        cell: Cell {
            x: 788,
            y: 160,
            width: 20,
            height: 20,
        },
        source_indexed_sha256: "ff8b4328131c869d9f14e1bef83d28063fb601bbe22ab384fb404ee9917c44e3"
            .to_string(),
    };
    let overlapping_glyph = CharacterSelectGlyphAllocation {
        font_role: CharacterSelectFontRole::SelectionHelp,
        character: '뒤',
        surface: CharacterSelectTextureSurface::SelectionHelpAtlas,
        tim_offset: SHARED_ATLAS_OFFSET,
        texture_page_index: 3,
        texture_uv: [36, 176],
        cell: Cell {
            x: 804,
            y: 176,
            width: 12,
            height: 16,
        },
        source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
    };

    validate_source_glyph_cell_conflicts(
        std::slice::from_ref(&source_cell),
        std::slice::from_ref(&overlapping_glyph),
    )
    .unwrap();

    let shared_writer = CharacterSelectGlyphAllocation {
        font_role: CharacterSelectFontRole::SelectHeading,
        character: '선',
        surface: CharacterSelectTextureSurface::SharedSelectAtlas,
        tim_offset: SHARED_ATLAS_OFFSET,
        texture_page_index: 3,
        texture_uv: [0, 160],
        cell: Cell {
            x: 768,
            y: 160,
            width: 32,
            height: 32,
        },
        source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
    };

    let error = validate_source_glyph_cell_conflicts(&[source_cell], &[shared_writer]).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("overlaps a source-blank dynamic glyph allocation")
    );
}
