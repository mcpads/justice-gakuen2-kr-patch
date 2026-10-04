//! Read-only source-record view used by allocation planning.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::source::{CharacterSelectAuxiliarySourceRecord, CharacterSelectSourceRecord};

pub(super) struct CharacterSelectPlanningSources<'a> {
    shared_atlas_record: &'a [u8],
    records: BTreeMap<&'a str, &'a [u8]>,
}

impl<'a> CharacterSelectPlanningSources<'a> {
    pub(super) fn shared_only(shared_atlas_record: &'a [u8]) -> Self {
        Self {
            shared_atlas_record,
            records: BTreeMap::new(),
        }
    }

    pub(super) fn from_loaded_records(
        primary: &'a [CharacterSelectSourceRecord],
        auxiliary: &'a [CharacterSelectAuxiliarySourceRecord],
    ) -> Result<Self> {
        let shared_atlas_record = primary
            .first()
            .map(|record| record.decoded.as_slice())
            .context("character-select source record inventory is empty")?;
        let mut records = BTreeMap::new();
        for record in primary {
            ensure!(
                records
                    .insert(record.path, record.decoded.as_slice())
                    .is_none(),
                "duplicate character-select planning source {}",
                record.path
            );
        }
        for record in auxiliary {
            ensure!(
                records
                    .insert(record.path, record.decoded.as_slice())
                    .is_none(),
                "duplicate character-select planning source {}",
                record.path
            );
        }
        Ok(Self {
            shared_atlas_record,
            records,
        })
    }

    pub(super) fn shared_atlas_record(&self) -> &'a [u8] {
        self.shared_atlas_record
    }

    pub(super) fn record(&self, path: &str) -> Option<&'a [u8]> {
        self.records.get(path).copied()
    }

    pub(super) fn has_record_context(&self) -> bool {
        !self.records.is_empty()
    }
}
