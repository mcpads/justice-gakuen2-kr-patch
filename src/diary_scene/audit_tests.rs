use super::audit::{is_diary_runtime_bundle, location_palette_roles, matching_catalogue_indices};

#[test]
fn diary_runtime_bundle_filter_excludes_unrelated_archives() {
    assert!(is_diary_runtime_bundle("DAT2/MGBGK04.BZZ"));
    assert!(is_diary_runtime_bundle("DAT2/MGBGT13.BZZ"));
    assert!(!is_diary_runtime_bundle("DAT2/MGK04.BZZ"));
    assert!(!is_diary_runtime_bundle("DAT2/MGBG.TZZ"));
    assert!(!is_diary_runtime_bundle("DAT1/MGBGK04.BZZ"));
}

#[test]
fn runtime_member_binding_reports_every_matching_catalogue_prefix() {
    let runtime = [1, 2, 3, 4, 5, 6];
    let first = [1, 2];
    let second = [7, 8];
    let third = [1, 2, 3, 4];
    let catalogue = [&first[..], &second[..], &third[..]];

    assert_eq!(
        matching_catalogue_indices(&runtime, catalogue.into_iter()),
        vec![0, 2]
    );
}

#[test]
fn location_palette_roles_follow_source_usage_and_luminance() {
    let pixels = [0, 0, 0, 0, 1, 1, 2, 7, 7];
    let mut palette = [0u16; 256];
    palette[1] = 0x0000;
    palette[2] = 0x4210;
    palette[7] = 0x7fff;

    let roles = location_palette_roles(&pixels, &palette).unwrap();

    assert_eq!(roles.clear_index, 0);
    assert_eq!(roles.outline_index, 1);
    assert_eq!(roles.fill_index, 7);
}
