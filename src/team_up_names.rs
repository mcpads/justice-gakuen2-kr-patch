//! Shared wording for cooperative moves; consumers own their own sprite geometry.
use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalogue {
    kind: String,
    translation_status: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source_text: String,
    korean_text: String,
}

pub(crate) struct TeamUpNames {
    pub(crate) sha256: String,
    entries: BTreeMap<String, String>,
}

impl TeamUpNames {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read cooperative names {}", path.display()))?;
        let catalogue: Catalogue = serde_json::from_slice(&bytes)?;
        ensure!(
            catalogue.kind == "justice_gakuen2_team_up_names"
                && catalogue.translation_status == "draft",
            "unsupported cooperative-name catalogue"
        );
        let mut entries = BTreeMap::new();
        for entry in catalogue.entries {
            ensure!(
                !entry.source_text.trim().is_empty()
                    && !entry.korean_text.trim().is_empty()
                    && !entry.korean_text.contains(['\n', '\r'])
                    && entries
                        .insert(entry.source_text, entry.korean_text)
                        .is_none(),
                "empty, multiline or duplicate cooperative name"
            );
        }
        ensure!(!entries.is_empty(), "empty cooperative-name catalogue");
        Ok(Self {
            sha256: crate::pipeline::sha256_bytes(&bytes),
            entries,
        })
    }

    pub(crate) fn translation(&self, source_text: &str) -> Result<&str> {
        self.entries
            .get(source_text)
            .map(String::as_str)
            .with_context(|| format!("missing cooperative-name translation for {source_text:?}"))
    }
}
