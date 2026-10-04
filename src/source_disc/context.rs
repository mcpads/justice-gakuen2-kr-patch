use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, ensure};

use crate::cue::CueSheet;
use crate::disc::iso9660::FileRecord;
use crate::disc::rebuild;
use crate::pipeline::sha256_file;

use super::profile::{SOURCE_BIN_BYTE_COUNT, SOURCE_BIN_SHA256};

#[derive(Clone)]
pub(crate) struct SupportedSourceDisc {
    inner: Arc<SupportedSourceDiscInner>,
}

struct SupportedSourceDiscInner {
    cue: CueSheet,
    source_bin_sha256: String,
    records: Mutex<BTreeMap<String, CachedSourceRecord>>,
}

#[derive(Clone)]
struct CachedSourceRecord {
    record: FileRecord,
    bytes: Vec<u8>,
}

impl SupportedSourceDisc {
    pub(crate) fn open(cue_path: &Path) -> Result<Self> {
        let cue = CueSheet::parse(cue_path)?;
        let metadata = std::fs::metadata(&cue.image_path).with_context(|| {
            format!("failed to inspect source BIN: {}", cue.image_path.display())
        })?;
        ensure!(
            metadata.is_file() && metadata.len() == SOURCE_BIN_BYTE_COUNT,
            "unsupported source BIN size: expected {}, got {}",
            SOURCE_BIN_BYTE_COUNT,
            metadata.len()
        );
        let source_bin_sha256 = sha256_file(&cue.image_path)?;
        ensure!(
            source_bin_sha256 == SOURCE_BIN_SHA256,
            "unsupported source BIN SHA-256: {source_bin_sha256}"
        );
        Ok(Self::from_verified_parts(cue, source_bin_sha256))
    }

    pub(super) fn from_verified_parts(cue: CueSheet, source_bin_sha256: String) -> Self {
        Self {
            inner: Arc::new(SupportedSourceDiscInner {
                cue,
                source_bin_sha256,
                records: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    pub(crate) fn cue(&self) -> &CueSheet {
        &self.inner.cue
    }

    pub(crate) fn image_path(&self) -> &Path {
        &self.inner.cue.image_path
    }

    pub(crate) fn source_bin_sha256(&self) -> &str {
        &self.inner.source_bin_sha256
    }

    pub(crate) fn read_record(&self, path: &str) -> Result<(FileRecord, Vec<u8>)> {
        let mut records = self
            .inner
            .records
            .lock()
            .map_err(|_| anyhow!("supported source record cache lock poisoned"))?;
        if let Some(cached) = records.get(path) {
            return Ok((cached.record.clone(), cached.bytes.clone()));
        }

        let (record, bytes) = rebuild::read_record(self.image_path(), path)
            .with_context(|| format!("failed to read verified source record {path}"))?;
        records.insert(
            path.to_string(),
            CachedSourceRecord {
                record: record.clone(),
                bytes: bytes.clone(),
            },
        );
        Ok((record, bytes))
    }
}
