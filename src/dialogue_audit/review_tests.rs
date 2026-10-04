use super::review::{review_page_dimensions, review_slot_origin};

#[test]
fn review_grid_uses_stable_row_major_coordinates() {
    assert_eq!(review_page_dimensions(), (856, 856));
    assert_eq!(review_slot_origin(0), (24, 24));
    assert_eq!(review_slot_origin(7), (752, 24));
    assert_eq!(review_slot_origin(8), (24, 128));
    assert_eq!(review_slot_origin(63), (752, 752));
}
