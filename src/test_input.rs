//! Locally supplied inputs for tests that the public tree cannot run alone.
//!
//! Tests that call these readers are marked `#[ignore = "requires ..."]` and run
//! with `cargo test -- --ignored` once the named file is in place.

use std::path::Path;

pub(crate) fn read(path: &str) -> Vec<u8> {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read(&full)
        .unwrap_or_else(|error| panic!("required test input {path} is unavailable: {error}"))
}

/// Reads a required input once and keeps it for the test process.
pub(crate) fn read_bytes(path: &str) -> &'static [u8] {
    read(path).leak()
}

/// Reads a required UTF-8 input once and keeps it for the test process.
pub(crate) fn read_str(path: &str) -> &'static str {
    String::from_utf8(read(path))
        .unwrap_or_else(|error| panic!("required test input {path} is not UTF-8: {error}"))
        .leak()
}
