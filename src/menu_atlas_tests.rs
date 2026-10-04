use super::*;

#[test]
fn logical_positions_wrap_renderer_uv_bytes_inside_each_page() {
    let horizontal = logical_code_position(0x000d).unwrap();
    assert_eq!(
        (horizontal.page, horizontal.column, horizontal.x),
        (0, 13, 4)
    );

    let vertical = logical_code_position(0x00d0).unwrap();
    assert_eq!((vertical.row, vertical.y), (13, 4));

    let paged = logical_code_position(0x02fd).unwrap();
    assert_eq!((paged.page, paged.column, paged.row), (2, 13, 15));
    assert_eq!((paged.x, paged.y), (516, 44));
}

#[test]
fn wrapped_footprint_is_split_without_crossing_a_texture_page() {
    let fragments = logical_code_fragments(0x00cc, 20, 20).unwrap();
    assert_eq!(
        fragments,
        [
            Cell {
                x: 240,
                y: 240,
                width: 16,
                height: 16,
            },
            Cell {
                x: 0,
                y: 240,
                width: 4,
                height: 16,
            },
            Cell {
                x: 240,
                y: 0,
                width: 16,
                height: 4,
            },
            Cell {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
        ]
    );
    assert!(fragments.iter().all(|fragment| {
        fragment.x + fragment.width <= MENU_ATLAS_PAGE_WIDTH
            && fragment.y + fragment.height <= MENU_ATLAS_HEIGHT
    }));
}

#[test]
fn overlap_uses_wrapped_physical_fragments_instead_of_nominal_grid_cells() {
    assert!(logical_regions_overlap(0x000c, 20, 20, 0x0000, 20, 20));
    assert!(!logical_regions_overlap(0x000c, 20, 20, 0x0100, 20, 20));
    assert!(logical_regions_overlap(0x00cc, 40, 40, 0x0000, 20, 20));
}

#[test]
fn shared_atlas_gate_rejects_unproven_global_writes() {
    let error = require_proven_shared_atlas_writes("Options glyph", 137, false).unwrap_err();
    assert!(error.to_string().contains("reclaimability is not proven"));
}

#[test]
fn shared_atlas_gate_allows_noop_or_proven_migration() {
    require_proven_shared_atlas_writes("context-only glyph", 0, false).unwrap();
    require_proven_shared_atlas_writes("migrated glyph", 137, true).unwrap();
}
