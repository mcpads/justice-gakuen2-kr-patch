use crate::pipeline::sha256_bytes;

use super::dialogue_development_runtime::validate_runtime_image_chain;
use super::dialogue_disc_build_model::{
    DialogueDiscAssetReadback, DialogueDiscBundleMemberReadback, DialogueDiscBundleReadback,
};

const SOURCE_PATH: &str = "DAT2/MGG04T.BIZ";

#[test]
fn runtime_image_chain_accepts_matching_record_bundle_member_and_ram() {
    let resident = b"decoded runtime image";
    let stored = b"compressed record";
    let (assets, bundles) = linked_readbacks(stored, stored, resident);

    let chain = validate_runtime_image_chain(&assets, &bundles, SOURCE_PATH, resident).unwrap();

    assert_eq!(chain.asset.path, SOURCE_PATH);
    assert_eq!(chain.bundle.path, "DAT2/MGG04.BZZ");
    assert_eq!(chain.member.source_path, SOURCE_PATH);
    assert_eq!(chain.resident_sha256, sha256_bytes(resident));
}

#[test]
fn runtime_image_chain_rejects_stale_bundle_member() {
    let resident = b"decoded runtime image";
    let (assets, bundles) = linked_readbacks(b"new compressed record", b"old member", resident);

    let error = validate_runtime_image_chain(&assets, &bundles, SOURCE_PATH, resident).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("individual dialogue record and loaded bundle member differ")
    );
}

#[test]
fn runtime_image_chain_rejects_ram_from_another_asset() {
    let expected_resident = b"decoded runtime image";
    let stored = b"compressed record";
    let (assets, bundles) = linked_readbacks(stored, stored, expected_resident);

    let error =
        validate_runtime_image_chain(&assets, &bundles, SOURCE_PATH, b"stale RAM").unwrap_err();

    assert!(
        error
            .to_string()
            .contains("emucap RAM dialogue image differs from disc readback")
    );
}

fn linked_readbacks(
    individual_stored: &[u8],
    member_stored: &[u8],
    decoded: &[u8],
) -> (
    Vec<DialogueDiscAssetReadback>,
    Vec<DialogueDiscBundleReadback>,
) {
    let individual_sha256 = sha256_bytes(individual_stored);
    let member_sha256 = sha256_bytes(member_stored);
    let decoded_sha256 = sha256_bytes(decoded);
    let bundle_sha256 = sha256_bytes(b"rebuilt bundle");
    (
        vec![DialogueDiscAssetReadback {
            path: SOURCE_PATH.to_string(),
            extent_lba: 1,
            sector_count: 1,
            expected_stored_sha256: individual_sha256.clone(),
            readback_stored_sha256: individual_sha256,
            expected_decoded_sha256: decoded_sha256.clone(),
            readback_decoded_sha256: decoded_sha256,
            readback_verified: true,
        }],
        vec![DialogueDiscBundleReadback {
            path: "DAT2/MGG04.BZZ".to_string(),
            extent_lba: 2,
            sector_count: 1,
            source_sha256: sha256_bytes(b"source bundle"),
            expected_rebuilt_sha256: bundle_sha256.clone(),
            readback_sha256: bundle_sha256,
            readback_verified: true,
            members: vec![DialogueDiscBundleMemberReadback {
                source_path: SOURCE_PATH.to_string(),
                bundle_offset: 2048,
                byte_count: member_stored.len(),
                source_sha256: sha256_bytes(b"source member"),
                expected_replacement_sha256: member_sha256.clone(),
                readback_replacement_sha256: member_sha256,
                readback_verified: true,
            }],
        }],
    )
}
