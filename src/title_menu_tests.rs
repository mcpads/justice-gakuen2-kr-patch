use crate::menu_audit::wrapped_cells_overlap;
use crate::menu_glyph_slots::{OPTIONS_SMALL_GLYPH_CODE_CANDIDATES, TITLE_MENU_GLYPH_CODES};
use crate::text::read_length_prefixed_codes;

use super::build::write_length_prefixed_record;

#[test]
fn title_menu_runtime_replacement_targets_the_compressed_consumer_record() {
    assert_eq!(super::TITLE_MENU_OVERLAY_PATH, "DAT1/MGTIT.BIZ");
    assert_ne!(
        super::source::OVERLAY_PATH,
        super::source::OVERLAY_DECODED_PATH
    );
}

#[test]
fn shorter_title_menu_text_clears_the_owned_source_tail() {
    let mut record = vec![0xff; 20];

    write_length_prefixed_record(&mut record, 0, 20, &[0x0398, 0x0399, 0x03aa, 0x03ab]).unwrap();

    assert_eq!(
        read_length_prefixed_codes(&record, 0).unwrap(),
        [0x0398, 0x0399, 0x03aa, 0x03ab]
    );
    assert!(record[10..].iter().all(|byte| *byte == 0));
}

#[test]
fn title_menu_text_must_fit_its_source_owned_record() {
    let mut record = vec![0; 8];

    let error = write_length_prefixed_record(&mut record, 0, 8, &[1, 2, 3, 4]).unwrap_err();

    assert!(error.to_string().contains("owns 8"));
}

#[test]
fn title_menu_glyph_slots_do_not_overlap_options_slots_after_uv_wrapping() {
    for title_code in TITLE_MENU_GLYPH_CODES {
        assert!(
            OPTIONS_SMALL_GLYPH_CODE_CANDIDATES
                .iter()
                .all(|options_code| !wrapped_cells_overlap(title_code, *options_code))
        );
    }
}
