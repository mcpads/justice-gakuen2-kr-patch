use super::corpus_codec::{reconstruct_words, semantic_sha256, source_markup};
use std::collections::BTreeSet;

use super::corpus_model::{
    DialogueCorpusAsset, DialogueCorpusBank, DialogueCorpusEntry, DialogueCorpusToken,
    DialogueSourceCorpus,
};
use super::corpus_writer::write_dialogue_source_corpus_shards;
use crate::pipeline::sha256_bytes;

fn glyph(code: &str, pixel: &str, text: &str) -> DialogueCorpusToken {
    DialogueCorpusToken::Glyph {
        code: code.to_string(),
        pixel_sha256: pixel.to_string(),
        text: text.to_string(),
        semantic_id: None,
        codebook_status: "source_pixel_verified".to_string(),
    }
}

#[test]
fn corpus_words_roundtrip_control_arguments_and_padding() {
    let tokens = vec![
        glyph("0x0001", "first", "あ"),
        DialogueCorpusToken::Control {
            code: "0x3003".to_string(),
            semantic_name: "palette_style".to_string(),
            arguments: vec!["0x0012".to_string()],
        },
        DialogueCorpusToken::Control {
            code: "0x3001".to_string(),
            semantic_name: "message_end".to_string(),
            arguments: Vec::new(),
        },
    ];

    assert_eq!(
        reconstruct_words(&tokens, 1).unwrap(),
        [0x0001, 0x3003, 0x0012, 0x3001, 0x0000]
    );
}

#[test]
fn source_markup_escapes_glyph_delimiters_and_names_controls() {
    let tokens = vec![
        glyph("0x0001", "first", "{\\"),
        DialogueCorpusToken::Control {
            code: "0x2003".to_string(),
            semantic_name: "relationship_name_plain".to_string(),
            arguments: vec!["0x0004".to_string()],
        },
    ];

    assert_eq!(
        source_markup(&tokens),
        "\\{\\\\{#relationship_name_plain:0x0004}"
    );
}

#[test]
fn semantic_hash_ignores_storage_identity_but_preserves_controls() {
    let left = vec![glyph("0x0001", "first", "あ")];
    let right = vec![glyph("0x0123", "second", "あ")];
    let controlled = vec![
        glyph("0x0123", "second", "あ"),
        DialogueCorpusToken::Control {
            code: "0x3000".to_string(),
            semantic_name: "line_break".to_string(),
            arguments: Vec::new(),
        },
    ];

    assert_eq!(semantic_sha256(&left), semantic_sha256(&right));
    assert_ne!(semantic_sha256(&left), semantic_sha256(&controlled));
}

#[test]
fn sharded_corpus_preserves_each_coordinate_once_and_binds_content_hashes() {
    let output_dir = std::env::temp_dir().join(format!(
        "justice-dialogue-corpus-shards-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);
    let corpus = DialogueSourceCorpus {
        kind: "test corpus".to_string(),
        implementation: "test extraction".to_string(),
        source_bin_sha256: "source".to_string(),
        codebook_sha256: "codebook".to_string(),
        coordinate_count: 3,
        raw_unique_entry_count: 3,
        semantic_shared_group_count: 3,
        glyph_occurrence_count: 3,
        control_occurrence_count: 0,
        alignment_padding_word_count: 0,
        roundtrip_verified_coordinate_count: 3,
        ready_for_translation: true,
        markup_contract: "test markup".to_string(),
        assets: vec![DialogueCorpusAsset {
            source_path: "DAT2/MGTEST.BIZ".to_string(),
            stored_sha256: "stored".to_string(),
            decoded_sha256: "decoded".to_string(),
            fixed_cell_count: 1,
            banks: vec![DialogueCorpusBank {
                selector_index: 2,
                entries: (0..3).map(test_corpus_entry).collect(),
            }],
        }],
    };

    let report = write_dialogue_source_corpus_shards(&output_dir, &corpus).unwrap();
    assert!(report.coordinate_partition_complete);
    assert_eq!(report.coordinate_count, 3);

    let manifest_bytes = std::fs::read(output_dir.join("dialogue-source-corpus.json")).unwrap();
    assert_eq!(report.manifest_sha256, sha256_bytes(&manifest_bytes));
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let shards = manifest["shards"].as_array().unwrap();
    assert_eq!(
        shards
            .iter()
            .map(|shard| shard["entry_count"].as_u64().unwrap())
            .sum::<u64>(),
        3
    );

    let mut coordinates = BTreeSet::new();
    for shard in shards {
        let relative_path = shard["path"].as_str().unwrap();
        let bytes = std::fs::read(output_dir.join(relative_path)).unwrap();
        assert_eq!(
            shard["content_sha256"].as_str().unwrap(),
            sha256_bytes(&bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for entry in body["entries"].as_array().unwrap() {
            assert!(coordinates.insert(entry["coordinate_id"].as_str().unwrap().to_string()));
        }
    }
    assert_eq!(
        coordinates,
        BTreeSet::from([
            "DAT2/MGTEST.BIZ#bank-2-entry-0000".to_string(),
            "DAT2/MGTEST.BIZ#bank-2-entry-0001".to_string(),
            "DAT2/MGTEST.BIZ#bank-2-entry-0002".to_string(),
        ])
    );
    std::fs::remove_dir_all(output_dir).unwrap();
}

fn test_corpus_entry(entry_index: usize) -> DialogueCorpusEntry {
    DialogueCorpusEntry {
        coordinate_id: format!("DAT2/MGTEST.BIZ#bank-2-entry-{entry_index:04}"),
        entry_index,
        decoded_offset: format!("0x{entry_index:08x}"),
        runtime_address: format!("0x{:08x}", 0x800d0000 + entry_index),
        raw_entry_sha256: format!("raw-{entry_index}"),
        semantic_source_sha256: format!("semantic-{entry_index}"),
        semantic_shared_coordinate_count: 1,
        raw_words: vec![format!("0x{entry_index:04x}")],
        alignment_padding_word_count: 0,
        source_markup: format!("text-{entry_index}"),
        tokens: vec![glyph(
            &format!("0x{entry_index:04x}"),
            &format!("pixel-{entry_index}"),
            "글",
        )],
    }
}
