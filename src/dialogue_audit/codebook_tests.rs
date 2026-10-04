use std::collections::{BTreeMap, BTreeSet};

use super::codebook::{
    SourceGlyphUsage, audit_codebook_bytes, dialogue_codebook_projection_sha256,
    resolved_dialogue_glyphs, validate_codebook_for_usage,
};
use crate::pipeline::sha256_bytes;

const SOURCE_SHA256: &str = "62dbc6ca47ec8d9dfbb5797f35d4e720d63e00b5a8e0edd080b149c3ff4f1823";
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn source_inventory() -> (BTreeSet<String>, BTreeMap<String, SourceGlyphUsage>) {
    let source_hashes = BTreeSet::from([HASH_A.to_string(), HASH_B.to_string()]);
    let usage = BTreeMap::from([
        (
            HASH_A.to_string(),
            SourceGlyphUsage {
                occurrence_count: 3,
                references: BTreeMap::from([(("A".to_string(), 1), 3)]),
            },
        ),
        (
            HASH_B.to_string(),
            SourceGlyphUsage {
                occurrence_count: 2,
                references: BTreeMap::from([(("B".to_string(), 2), 2)]),
            },
        ),
    ]);
    (source_hashes, usage)
}

#[test]
fn candidate_entries_remain_unresolved() {
    let codebook = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{
    "known-message-a": "Known message evidence.",
    "ocr-candidate": "Untrusted OCR candidate."
  }},
  "entries": [
    {{
      "pixel_sha256": "{HASH_A}",
      "meaning": {{"kind": "character", "text": "あ"}},
      "status": "known_text_verified",
      "evidence": ["known-message-a"],
      "reviewed_by": null,
      "reviewed_at": null
    }},
    {{
      "pixel_sha256": "{HASH_B}",
      "meaning": {{"kind": "character", "text": "い"}},
      "status": "candidate",
      "evidence": ["ocr-candidate"],
      "reviewed_by": null,
      "reviewed_at": null
    }}
  ]
}}"#
    );
    let (source_hashes, usage) = source_inventory();

    let report =
        audit_codebook_bytes(codebook.as_bytes(), SOURCE_SHA256, &source_hashes, &usage).unwrap();

    assert_eq!(report.verified_entry_count, 1);
    assert_eq!(report.candidate_entry_count, 1);
    assert_eq!(report.resolved_used_source_pixel_hash_count, 1);
    assert_eq!(report.unresolved_used_source_pixel_hash_count, 1);
    assert_eq!(report.unresolved_glyph_occurrence_count, 2);
    assert!(!report.ready_for_glyph_decode);
    assert_eq!(report.unresolved[0].source_references[0].code, "0x0002");
}

#[test]
fn human_approval_requires_reviewer_metadata() {
    let codebook = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"review-sheet": "Manual review sheet."}},
  "entries": [{{
    "pixel_sha256": "{HASH_A}",
    "meaning": {{"kind": "character", "text": "あ"}},
    "status": "human_approved",
    "evidence": ["review-sheet"],
    "reviewed_by": null,
    "reviewed_at": null
  }}]
}}"#
    );
    let (source_hashes, usage) = source_inventory();

    let error = audit_codebook_bytes(codebook.as_bytes(), SOURCE_SHA256, &source_hashes, &usage)
        .unwrap_err();

    assert!(error.to_string().contains("reviewed_by"));
}

#[test]
fn rejects_codebook_hashes_absent_from_source_media() {
    let codebook = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"known-message-a": "Known message evidence."}},
  "entries": [{{
    "pixel_sha256": "{HASH_A}",
    "meaning": {{"kind": "character", "text": "あ"}},
    "status": "known_text_verified",
    "evidence": ["known-message-a"],
    "reviewed_by": null,
    "reviewed_at": null
  }}]
}}"#
    );
    let usage = BTreeMap::new();

    let error = audit_codebook_bytes(codebook.as_bytes(), SOURCE_SHA256, &BTreeSet::new(), &usage)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("absent from the selected source media")
    );
}

#[test]
fn subset_usage_does_not_reject_valid_codebook_superset_entries() {
    let codebook = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"known-message-a": "Known message evidence."}},
  "entries": [
    {{
      "pixel_sha256": "{HASH_A}",
      "meaning": {{"kind": "character", "text": "あ"}},
      "status": "known_text_verified",
      "evidence": ["known-message-a"]
    }},
    {{
      "pixel_sha256": "{HASH_B}",
      "meaning": {{"kind": "character", "text": "い"}},
      "status": "known_text_verified",
      "evidence": ["known-message-a"]
    }}
  ]
}}"#
    );
    let usage = BTreeMap::from([(
        HASH_A.to_string(),
        SourceGlyphUsage {
            occurrence_count: 1,
            references: BTreeMap::new(),
        },
    )]);

    validate_codebook_for_usage(codebook.as_bytes(), SOURCE_SHA256, &usage).unwrap();
}

#[test]
fn subset_projection_ignores_entries_outside_selected_source_pixels() {
    let codebook_a = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"known-message-a": "Known message evidence."}},
  "entries": [{{
    "pixel_sha256": "{HASH_A}",
    "meaning": {{"kind": "character", "text": "あ"}},
    "status": "known_text_verified",
    "evidence": ["known-message-a"]
  }}]
}}"#
    );
    let codebook_ab = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"known-message-a": "Known message evidence."}},
  "entries": [
    {{
      "pixel_sha256": "{HASH_A}",
      "meaning": {{"kind": "character", "text": "あ"}},
      "status": "known_text_verified",
      "evidence": ["known-message-a"]
    }},
    {{
      "pixel_sha256": "{HASH_B}",
      "meaning": {{"kind": "character", "text": "い"}},
      "status": "known_text_verified",
      "evidence": ["known-message-a"]
    }}
  ]
}}"#
    );
    let selected = BTreeSet::from([HASH_A.to_string()]);
    let glyphs_a = resolved_dialogue_glyphs(codebook_a.as_bytes()).unwrap();
    let glyphs_ab = resolved_dialogue_glyphs(codebook_ab.as_bytes()).unwrap();

    assert_eq!(
        dialogue_codebook_projection_sha256(&selected, &glyphs_a).unwrap(),
        dialogue_codebook_projection_sha256(&selected, &glyphs_ab).unwrap()
    );
}

#[test]
fn projection_digest_is_distinct_from_the_codebook_file_digest() {
    let codebook = format!(
        r#"{{
  "kind": "Justice Gakuen 2 asset-scoped dialogue glyph codebook",
  "source_bin_sha256": "{SOURCE_SHA256}",
  "evidence_sources": {{"known-message-a": "Known message evidence."}},
  "entries": [{{
    "pixel_sha256": "{HASH_A}",
    "meaning": {{"kind": "character", "text": "あ"}},
    "status": "known_text_verified",
    "evidence": ["known-message-a"]
  }}]
}}"#
    );
    let selected = BTreeSet::from([HASH_A.to_string()]);
    let glyphs = resolved_dialogue_glyphs(codebook.as_bytes()).unwrap();

    assert_ne!(
        dialogue_codebook_projection_sha256(&selected, &glyphs).unwrap(),
        sha256_bytes(codebook.as_bytes())
    );
}
