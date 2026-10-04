//! Source-atlas identity and write-allocation gates.
//!
//! A source atlas domain describes one exact JP TIM and the consumers known to
//! read it. It is deliberately not a Korean destination-slot or write
//! authority.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(super) struct SourceAtlasDomainId(String);

impl SourceAtlasDomainId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for SourceAtlasDomainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceAtlasDomainCatalog {
    pub(super) kind: PracticalResultSourceAtlasDomainCatalogKind,
    pub(super) domains: Vec<PracticalResultSourceAtlasDomain>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultSourceAtlasDomainCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_source_atlas_domain_catalog")]
    JusticeGakuen2PracticalResultSourceAtlasDomainCatalog,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceAtlasDomain {
    pub(super) id: SourceAtlasDomainId,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) source_tim_size: usize,
    pub(super) source_tim_sha256: String,
    pub(super) pixel_width: usize,
    pub(super) pixel_height: usize,
    pub(super) consumer_projection_denominator_status:
        PracticalResultSourceAtlasConsumerProjectionDenominatorStatus,
    #[serde(default)]
    pub(super) consumer_projection_denominator_incomplete_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceAtlasConsumerProjectionDenominatorStatus {
    Complete,
    Incomplete,
}
