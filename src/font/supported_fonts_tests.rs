use super::raster::supported_font_identity;

#[test]
fn exact_maplestory_weights_are_distinct_supported_inputs() {
    let light =
        supported_font_identity("6d51d8e576f77b01914095aa1f69f9d37c16d93fe940d748962867f218442ba9")
            .unwrap();
    let bold =
        supported_font_identity("d57eaff48a793ff872a0f33bba2943d058d07c81ed64c68054858a287b85811a")
            .unwrap();

    assert_eq!(light.name, "Maplestory Light");
    assert_eq!(light.slug, "maplestory-light");
    assert_eq!(bold.name, "Maplestory Bold");
    assert_eq!(bold.slug, "maplestory-bold");
    assert_ne!(light, bold);
}

#[test]
fn unpinned_font_input_is_rejected() {
    assert!(supported_font_identity(&"0".repeat(64)).is_none());
}
