use std::path::PathBuf;

use super::build::validate_contributors;
use super::model::{MenuCompositionContributorSpec, MenuContributorEncoding};

fn contributor(role: &str) -> MenuCompositionContributorSpec {
    MenuCompositionContributorSpec {
        role: role.to_string(),
        path: PathBuf::from("menu.biz"),
        sha256: "hash".to_string(),
        encoding: MenuContributorEncoding::CompressedRecord,
    }
}

#[test]
fn distinct_surface_roles_are_accepted() {
    validate_contributors(&[contributor("MODE SELECT"), contributor("options")]).unwrap();
}

#[test]
fn repeated_surface_role_is_rejected() {
    assert!(validate_contributors(&[contributor("options"), contributor("options")]).is_err());
}
