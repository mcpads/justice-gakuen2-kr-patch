use serde_json::json;

use crate::pipeline::sha256_bytes;

use super::apply_review_bytes;

fn fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let codebook = serde_json::to_vec_pretty(&json!({
        "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
        "source_bin_sha256": "source",
        "evidence_sources": {"existing": "Existing evidence."},
        "entries": []
    }))
    .unwrap();
    let context = serde_json::to_vec_pretty(&json!({
        "kind": "Dialogue runtime-image glyph review context shard",
        "page_index": 1,
        "slot_count": 1,
        "slots": [{
            "slot_index": 0,
            "pixel_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }]
    }))
    .unwrap();
    let png = b"review pixels".to_vec();
    let review = serde_json::to_vec_pretty(&json!({
        "kind": "Dialogue glyph source review decision shard",
        "source_bin_sha256": "source",
        "codebook_before_sha256": sha256_bytes(&codebook),
        "context_shard_sha256": sha256_bytes(&context),
        "review_png_sha256": sha256_bytes(&png),
        "batch_index": 1,
        "page_index": 1,
        "evidence_id": "full-runtime-review-page-001-manual",
        "evidence_description": "Exact source pixels and runtime dialogue contexts.",
        "reviewed_at": "2026-08-11",
        "entries": [{
            "slot_index": 0,
            "pixel_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "text": "告"
        }]
    }))
    .unwrap();
    (codebook, review, context, png)
}

#[test]
fn applies_a_source_bound_review_decision() {
    let (codebook, review, context, png) = fixture();
    let (output, report) = apply_review_bytes(&codebook, &review, &context, &png).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(report.applied_entry_count, 1);
    assert_eq!(report.codebook_entry_count, 1);
    assert_eq!(report.batch_index, 1);
    assert_eq!(value["entries"][0]["meaning"]["text"], "告");
    assert_eq!(value["entries"][0]["status"], "source_pixel_verified");
}

#[test]
fn rejects_a_review_with_different_pixel_evidence() {
    let (codebook, review, context, mut png) = fixture();
    png.push(b'!');

    let error = apply_review_bytes(&codebook, &review, &context, &png).unwrap_err();
    assert!(error.to_string().contains("review PNG SHA-256 mismatch"));
}
