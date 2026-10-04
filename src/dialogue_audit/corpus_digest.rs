use std::io::Write;

use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::corpus_model::DialogueSourceCorpus;

pub(super) fn source_corpus_sha256(corpus: &DialogueSourceCorpus) -> Result<String> {
    pretty_json_sha256(corpus)
}

fn pretty_json_sha256(value: &impl Serialize) -> Result<String> {
    let mut writer = Sha256Writer(Sha256::new());
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    Ok(format!("{:x}", writer.0.finalize()))
}

struct Sha256Writer(Sha256);

impl Write for Sha256Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn pretty_json_sha256_for_test(value: &impl Serialize) -> Result<String> {
    pretty_json_sha256(value)
}
