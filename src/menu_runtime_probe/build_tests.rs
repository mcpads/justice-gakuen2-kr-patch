use super::build::validate_output_stem;

#[test]
fn output_stem_accepts_role_words_and_hyphens() {
    validate_output_stem("password-mode-select-only").unwrap();
}

#[test]
fn output_stem_rejects_paths_and_revision_labels() {
    assert!(validate_output_stem("../probe").is_err());
    assert!(validate_output_stem("ProbeV2").is_err());
}
