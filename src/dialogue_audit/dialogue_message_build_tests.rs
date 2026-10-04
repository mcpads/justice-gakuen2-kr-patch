use std::path::PathBuf;

use super::dialogue_message_build::materialize_dialogue_message_asset;
use super::dialogue_message_plan_model::DialogueMessageBuildPlanAsset;
use crate::compression::decompress;
use crate::paged_compression::{
    PagedCompressionProfile, compress_page_safe_image, profile_paged_compression,
};
use crate::pipeline::sha256_bytes;
use crate::test_support::temporary_directory;

#[test]
fn standalone_message_materialization_writes_roundtrip_verified_stored_image() {
    let output_dir = test_output_dir("standalone-materialization");
    std::fs::create_dir_all(&output_dir).unwrap();
    let mut rebuilt_decoded = vec![0_u8; 80_000];
    rebuilt_decoded.extend_from_slice(b"SCRIPT-DATA");
    if !rebuilt_decoded.len().is_multiple_of(2) {
        rebuilt_decoded.push(0);
    }
    let source_contract = PagedCompressionProfile::from_source_contract(1_073, 2_146, 0, 0x7000);
    let source_stream = compress_page_safe_image(&rebuilt_decoded, source_contract).unwrap();
    let source_compression_profile = profile_paged_compression(&source_stream).unwrap();
    let source_catalog_prefix: [u8; 4] = source_stream[..4].try_into().unwrap();
    let mut source_stored = source_stream;
    source_stored.resize(source_stored.len() + 64, 0);
    let plan = DialogueMessageBuildPlanAsset {
        source_path: "DAT2/TEST.BIZ".to_string(),
        original_decoded_sha256: sha256_bytes(&rebuilt_decoded),
        rebuilt_decoded_sha256: sha256_bytes(&rebuilt_decoded),
        original_stored_sha256: sha256_bytes(&source_stored),
        original_stored_byte_count: source_stored.len(),
        source_catalog_prefix,
        source_compression_profile,
        original_decoded: rebuilt_decoded.clone(),
        rebuilt_decoded: rebuilt_decoded.clone(),
        bank_count: 1,
        message_count: 1,
        translated_message_count: 1,
        rewritten_runtime_insertion_message_count: 0,
        preserved_untranslated_message_count: 0,
        preserved_unreferenced_message_count: 0,
        message_arena_used_byte_count: 12,
        message_arena_spare_byte_count: 4,
        parse_back_verified: true,
    };

    let report = materialize_dialogue_message_asset(&output_dir, &plan).unwrap();

    let written_decoded = std::fs::read(output_dir.join(&report.decoded_output_file)).unwrap();
    let written_stored = std::fs::read(output_dir.join(&report.stored_output_file)).unwrap();
    assert_eq!(written_decoded, rebuilt_decoded);
    assert_eq!(written_stored.len(), plan.original_stored_byte_count);
    assert_eq!(sha256_bytes(&written_stored), report.rebuilt_stored_sha256);
    assert_eq!(
        decompress(&written_stored[..report.compressed_byte_count], false).unwrap(),
        rebuilt_decoded
    );
    assert_eq!(written_stored[..4], plan.source_catalog_prefix);
    assert!(
        written_stored[report.compressed_byte_count..]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(report.parse_back_verified);
    assert!(report.compression_roundtrip_verified);
    assert!(report.compressed_within_original_extent);
    std::fs::remove_dir_all(output_dir).unwrap();
}

fn test_output_dir(label: &str) -> PathBuf {
    temporary_directory(&format!("dialogue-message-{label}"))
}
