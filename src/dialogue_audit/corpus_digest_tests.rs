use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::corpus_digest::pretty_json_sha256_for_test;

#[derive(Serialize)]
struct DigestFixture {
    label: &'static str,
    values: Vec<u16>,
}

#[test]
fn streamed_digest_matches_canonical_pretty_json_bytes() {
    let fixture = DigestFixture {
        label: "source corpus",
        values: vec![0x0001, 0x0203, 0xffff],
    };
    let materialized = format!("{}\n", serde_json::to_string_pretty(&fixture).unwrap());

    assert_eq!(
        pretty_json_sha256_for_test(&fixture).unwrap(),
        sha256_bytes(materialized.as_bytes())
    );
}
