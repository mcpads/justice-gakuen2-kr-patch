use super::{
    EXPECTED_MGK_PATHS, expected_runtime_image_population, is_dialogue_runtime_image_path,
};

#[test]
fn runtime_population_includes_non_mgk_dialogue_images() {
    assert!(is_dialogue_runtime_image_path("DAT2/MGK04.BIZ"));
    assert!(is_dialogue_runtime_image_path("DAT2/MGG04T.BIZ"));
    assert!(!EXPECTED_MGK_PATHS.contains(&"DAT2/MGG04T.BIZ"));
}

#[test]
fn runtime_population_excludes_known_non_dialogue_and_foreign_paths() {
    assert!(!is_dialogue_runtime_image_path("DAT2/MGSTAFF1.BIZ"));
    assert!(!is_dialogue_runtime_image_path("DAT1/MGK04.BIZ"));
    assert!(!is_dialogue_runtime_image_path("DAT2/MGK04.BIN"));
}

#[test]
fn runtime_population_rejects_an_incomplete_path_manifest() {
    assert!(!expected_runtime_image_population(&[
        "DAT2/MGK04.BIZ",
        "DAT2/MGG04T.BIZ",
    ]));
}
