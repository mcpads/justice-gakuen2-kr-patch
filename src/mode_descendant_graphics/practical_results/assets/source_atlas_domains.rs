//! Static catalog joins for exact JP source-atlas identities.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::super::model::{
    PracticalResultManifest, PracticalResultPhysicalRegionCatalog,
    PracticalResultSourceGlyphCatalog,
};
use super::super::source_atlas_domain_model::{
    PracticalResultSourceAtlasConsumerProjectionDenominatorStatus,
    PracticalResultSourceAtlasDomainCatalog, PracticalResultSourceAtlasDomainCatalogKind,
};
use super::parse_hex_offset;

pub(super) fn validate_source_atlas_domain_catalog(
    catalog: &PracticalResultSourceAtlasDomainCatalog,
    physical_regions: &PracticalResultPhysicalRegionCatalog,
    source_glyphs: &PracticalResultSourceGlyphCatalog,
    manifest: &PracticalResultManifest,
) -> Result<()> {
    ensure!(
        matches!(
            catalog.kind,
            PracticalResultSourceAtlasDomainCatalogKind::JusticeGakuen2PracticalResultSourceAtlasDomainCatalog
        ) && !catalog.domains.is_empty(),
        "unsupported or empty practical-result source-atlas domain catalog"
    );
    let source_paths = manifest
        .source_catalog
        .iter()
        .map(|source| source.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut source_keys = BTreeSet::new();
    for domain in &catalog.domains {
        ensure!(
            !domain.id.is_empty()
                && ids.insert(domain.id.as_str())
                && source_paths.contains(domain.source_path.as_str())
                && matches!(domain.bpp, 4 | 8)
                && domain.source_tim_size > 0
                && domain.pixel_width > 0
                && domain.pixel_height > 0
                && source_keys.insert((
                    domain.source_path.as_str(),
                    parse_hex_offset(&domain.tim_offset)?,
                    domain.bpp,
                )),
            "invalid or duplicate practical-result source-atlas domain {}",
            domain.id
        );
        validate_sha256(&domain.source_tim_sha256, "source-atlas TIM")?;
        match domain.consumer_projection_denominator_status {
            PracticalResultSourceAtlasConsumerProjectionDenominatorStatus::Complete => ensure!(
                domain
                    .consumer_projection_denominator_incomplete_reason
                    .is_none(),
                "source-atlas domain {} has a complete consumer-projection denominator and an incomplete reason",
                domain.id
            ),
            PracticalResultSourceAtlasConsumerProjectionDenominatorStatus::Incomplete => ensure!(
                domain
                    .consumer_projection_denominator_incomplete_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "source-atlas domain {} has an incomplete consumer-projection denominator without a reason",
                domain.id
            ),
        }
    }
    let domains = catalog
        .domains
        .iter()
        .map(|domain| (domain.id.as_str(), domain))
        .collect::<BTreeMap<_, _>>();
    let domains_by_source_key = catalog
        .domains
        .iter()
        .map(|domain| {
            Ok((
                (
                    domain.source_path.as_str(),
                    parse_hex_offset(&domain.tim_offset)?,
                    domain.bpp,
                ),
                domain,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    for region in &physical_regions.regions {
        let matching_domain = domains_by_source_key.get(&(
            region.source_path.as_str(),
            parse_hex_offset(&region.tim_offset)?,
            region.bpp,
        ));
        ensure!(
            region.source_atlas_domain_id.as_ref() == matching_domain.map(|domain| &domain.id),
            "physical region {} source-atlas reverse join is incomplete or crosses domains",
            region.region_id
        );
    }
    for domain in &catalog.domains {
        ensure!(
            physical_regions
                .regions
                .iter()
                .any(|region| region.source_atlas_domain_id.as_ref() == Some(&domain.id)),
            "source-atlas domain {} has no physical-region evidence",
            domain.id
        );
    }
    for bank in &source_glyphs.banks {
        let domain = domains
            .get(bank.source_atlas_domain_id.as_str())
            .with_context(|| {
                format!(
                    "source-glyph bank {} names unknown source-atlas domain {}",
                    bank.id, bank.source_atlas_domain_id
                )
            })?;
        ensure!(
            bank.source_path == domain.source_path
                && bank.tim_offset == domain.tim_offset
                && bank.bpp == domain.bpp,
            "source-glyph bank {} crosses its source-atlas domain",
            bank.id
        );
    }
    for protected in &manifest.protected_content.regions {
        let domain = domains
            .get(protected.source_atlas_domain_id.as_str())
            .with_context(|| {
                format!(
                    "protected region {} names unknown source-atlas domain {}",
                    protected.id, protected.source_atlas_domain_id
                )
            })?;
        ensure!(
            protected.source_path == domain.source_path
                && protected.tim_offset == domain.tim_offset
                && protected.bpp == domain.bpp,
            "protected region {} crosses its source-atlas domain",
            protected.id
        );
    }
    Ok(())
}

fn validate_sha256(value: &str, role: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid {role} SHA-256"
    );
    Ok(())
}
