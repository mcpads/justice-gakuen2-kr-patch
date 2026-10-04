use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::super::model::{LoadedImageConsumerAudit, StaticProfiledStateAccessAudit};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DeclaredSelectorDomain {
    pub(super) known_values: BTreeSet<u8>,
    pub(super) unresolved_runtime_sources: BTreeSet<String>,
}

impl DeclaredSelectorDomain {
    pub(super) fn is_finite(&self) -> bool {
        !self.known_values.is_empty() && self.unresolved_runtime_sources.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StateLayer {
    Final,
    Upstream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeSourceTransform {
    WrappingDecrement,
}

impl RuntimeSourceTransform {
    fn apply(self, value: u8) -> u8 {
        match self {
            Self::WrappingDecrement => value.wrapping_sub(1),
        }
    }
}

pub(super) fn declared_selector_domain(
    reports: &[LoadedImageConsumerAudit],
    state_runtime_address: u32,
) -> Result<DeclaredSelectorDomain> {
    let mut domain = DeclaredSelectorDomain::default();
    let mut stack = BTreeSet::new();
    collect_state_writes(
        reports,
        StateLayer::Final,
        state_runtime_address,
        &mut stack,
        &mut domain,
    )?;
    Ok(domain)
}

pub(super) fn declared_upstream_state_domain(
    reports: &[LoadedImageConsumerAudit],
    state_runtime_address: u32,
) -> Result<DeclaredSelectorDomain> {
    let mut domain = DeclaredSelectorDomain::default();
    let mut stack = BTreeSet::new();
    collect_state_writes(
        reports,
        StateLayer::Upstream,
        state_runtime_address,
        &mut stack,
        &mut domain,
    )?;
    Ok(domain)
}

fn collect_state_writes(
    reports: &[LoadedImageConsumerAudit],
    layer: StateLayer,
    state_runtime_address: u32,
    stack: &mut BTreeSet<(StateLayer, u32)>,
    domain: &mut DeclaredSelectorDomain,
) -> Result<()> {
    if !stack.insert((layer, state_runtime_address)) {
        domain.unresolved_runtime_sources.insert(format!(
            "cyclic_{layer:?}_state_source@0x{state_runtime_address:08x}"
        ));
        return Ok(());
    }
    let mut found = false;
    for report in reports {
        let accesses = match layer {
            StateLayer::Final => &report.disc_record_loader_selector_state_accesses,
            StateLayer::Upstream => &report.disc_record_loader_selector_upstream_state_accesses,
        };
        for access in accesses.iter().filter(|access| access.operation == "write") {
            let access_addresses = access
                .access_runtime_addresses
                .iter()
                .map(|address| parse_address(address))
                .collect::<Result<BTreeSet<_>>>()?;
            if !access_addresses.contains(&state_runtime_address) {
                continue;
            }
            found = true;
            collect_access_values(
                reports,
                report,
                access,
                state_runtime_address,
                stack,
                domain,
            )?;
        }
    }
    if !found {
        domain.unresolved_runtime_sources.insert(format!(
            "unobserved_{layer:?}_state_source@0x{state_runtime_address:08x}"
        ));
    }
    stack.remove(&(layer, state_runtime_address));
    Ok(())
}

fn collect_access_values(
    reports: &[LoadedImageConsumerAudit],
    report: &LoadedImageConsumerAudit,
    access: &StaticProfiledStateAccessAudit,
    state_runtime_address: u32,
    stack: &mut BTreeSet<(StateLayer, u32)>,
    domain: &mut DeclaredSelectorDomain,
) -> Result<()> {
    let Some(evidence) = &access.writer_evidence else {
        domain.unresolved_runtime_sources.insert(format!(
            "{}:{}:unclassified_writer",
            report.path, access.instruction_runtime_address
        ));
        return Ok(());
    };
    match evidence.value_resolution.as_str() {
        "exact_candidates" => {
            domain
                .known_values
                .extend(evidence.exact_value_candidates.iter().copied());
        }
        "bounded_range" => {
            let [start, end] = evidence
                .bounded_value_range
                .context("bounded selector writer lacks its value range")?;
            ensure!(start <= end, "bounded selector writer range is reversed");
            domain.known_values.extend(start..=end);
        }
        "runtime_source" if preserves_source_byte(&evidence.classification) => {
            domain
                .known_values
                .extend(evidence.exact_value_candidates.iter().copied());
            let source_runtime_address = identity_source_address(
                &access.access_runtime_addresses,
                &evidence.source_runtime_byte_ranges,
                state_runtime_address,
            )?;
            if let Some(upstream_runtime_address) =
                upstream_identity_source(state_runtime_address, source_runtime_address)
            {
                collect_state_writes(
                    reports,
                    StateLayer::Upstream,
                    upstream_runtime_address,
                    stack,
                    domain,
                )?;
            }
        }
        "runtime_source" if runtime_source_transform(&evidence.classification).is_some() => {
            domain
                .known_values
                .extend(evidence.exact_value_candidates.iter().copied());
            let transform = runtime_source_transform(&evidence.classification)
                .expect("guard established a known runtime-source transform");
            let source_runtime_address = transformed_source_address(
                &evidence.source_runtime_byte_ranges,
                &evidence.classification,
            )?;
            let mut source_domain = DeclaredSelectorDomain::default();
            collect_state_writes(
                reports,
                StateLayer::Upstream,
                source_runtime_address,
                stack,
                &mut source_domain,
            )?;
            domain.known_values.extend(
                source_domain
                    .known_values
                    .into_iter()
                    .map(|value| transform.apply(value)),
            );
            domain
                .unresolved_runtime_sources
                .extend(source_domain.unresolved_runtime_sources);
        }
        "runtime_source" => {
            domain
                .known_values
                .extend(evidence.exact_value_candidates.iter().copied());
            let source_ranges = evidence
                .source_runtime_byte_ranges
                .iter()
                .map(|range| format!("{}..{}", range[0], range[1]))
                .collect::<Vec<_>>();
            domain.unresolved_runtime_sources.insert(format!(
                "{}:{}:{}{}",
                report.path,
                access.instruction_runtime_address,
                evidence.classification,
                if source_ranges.is_empty() {
                    String::new()
                } else {
                    format!(" from {}", source_ranges.join(","))
                }
            ));
        }
        other => {
            domain.unresolved_runtime_sources.insert(format!(
                "{}:{}:unknown_value_resolution:{other}",
                report.path, access.instruction_runtime_address
            ));
        }
    }
    Ok(())
}

fn upstream_identity_source(
    destination_runtime_address: u32,
    source_runtime_address: u32,
) -> Option<u32> {
    (source_runtime_address != destination_runtime_address).then_some(source_runtime_address)
}

fn transformed_source_address(source_ranges: &[[String; 2]], classification: &str) -> Result<u32> {
    ensure!(
        source_ranges.len() == 1,
        "{classification} must have one contiguous source range"
    );
    let source_start = parse_address(&source_ranges[0][0])?;
    let source_end = parse_address(&source_ranges[0][1])?;
    ensure!(
        source_start < source_end,
        "{classification} source range is empty or reversed"
    );
    Ok(source_start)
}

fn identity_source_address(
    destination_addresses: &[String],
    source_ranges: &[[String; 2]],
    destination_runtime_address: u32,
) -> Result<u32> {
    ensure!(
        source_ranges.len() == 1,
        "identity-preserving copy must have one contiguous source range"
    );
    let destinations = destination_addresses
        .iter()
        .map(|address| parse_address(address))
        .collect::<Result<BTreeSet<_>>>()?;
    let first_destination = destinations
        .first()
        .copied()
        .context("identity-preserving copy has no destination addresses")?;
    let last_destination = destinations
        .last()
        .copied()
        .context("identity-preserving copy has no destination addresses")?;
    ensure!(
        last_destination - first_destination + 1 == u32::try_from(destinations.len())?,
        "identity-preserving copy destination addresses are not contiguous"
    );
    ensure!(
        destinations.contains(&destination_runtime_address),
        "identity-preserving copy does not contain requested destination"
    );
    let source_start = parse_address(&source_ranges[0][0])?;
    let source_end = parse_address(&source_ranges[0][1])?;
    ensure!(
        source_end - source_start == u32::try_from(destinations.len())?,
        "identity-preserving copy source and destination widths differ"
    );
    source_start
        .checked_add(destination_runtime_address - first_destination)
        .context("identity-preserving copy source address overflow")
}

fn preserves_source_byte(classification: &str) -> bool {
    matches!(
        classification,
        "configuration_byte_copy"
            | "global_runtime_byte"
            | "forwarded_global_runtime_byte"
            | "forwarded_halfword_low_byte"
    )
}

fn runtime_source_transform(classification: &str) -> Option<RuntimeSourceTransform> {
    match classification {
        "decremented_runtime_byte" | "decremented_runtime_word_low_byte" => {
            Some(RuntimeSourceTransform::WrappingDecrement)
        }
        _ => None,
    }
}

fn parse_address(value: &str) -> Result<u32> {
    let digits = value
        .strip_prefix("0x")
        .context("runtime address lacks 0x prefix")?;
    u32::from_str_radix(digits, 16).with_context(|| format!("invalid runtime address {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_copy_maps_each_destination_to_the_corresponding_source_byte() {
        let destinations = (0x801f_6494_u32..=0x801f_6497_u32)
            .map(|address| format!("0x{address:08x}"))
            .collect::<Vec<_>>();
        let sources = [["0x801f6702".to_string(), "0x801f6706".to_string()]];

        assert_eq!(
            identity_source_address(&destinations, &sources, 0x801f_6496).unwrap(),
            0x801f_6704
        );
    }

    #[test]
    fn unresolved_runtime_sources_prevent_finite_domain_promotion() {
        let mut domain = DeclaredSelectorDomain::default();
        domain.known_values.insert(0);
        assert!(domain.is_finite());

        domain
            .unresolved_runtime_sources
            .insert("runtime byte".to_string());
        assert!(!domain.is_finite());
    }

    #[test]
    fn identity_self_copy_does_not_create_an_upstream_dependency() {
        assert_eq!(upstream_identity_source(0x801f_180a, 0x801f_180a), None);
        assert_eq!(
            upstream_identity_source(0x801f_1a0e, 0x801f_180a),
            Some(0x801f_180a)
        );
    }

    #[test]
    fn only_identity_preserving_runtime_sources_are_followed() {
        assert!(preserves_source_byte("configuration_byte_copy"));
        assert!(preserves_source_byte("global_runtime_byte"));
        assert!(preserves_source_byte("forwarded_global_runtime_byte"));
        assert!(preserves_source_byte("forwarded_halfword_low_byte"));
        assert!(!preserves_source_byte("incremented_runtime_state"));
        assert!(!preserves_source_byte("runtime_buffer_byte_copy"));
    }

    #[test]
    fn wrapping_decrement_transform_preserves_finite_values_and_wraps_zero() {
        let transform = runtime_source_transform("decremented_runtime_byte").unwrap();
        let source = BTreeSet::from([0, 1, 3]);
        let transformed = source
            .into_iter()
            .map(|value| transform.apply(value))
            .collect::<BTreeSet<_>>();

        assert_eq!(transformed, BTreeSet::from([0, 2, 255]));
        assert_eq!(
            runtime_source_transform("decremented_runtime_word_low_byte"),
            Some(RuntimeSourceTransform::WrappingDecrement)
        );
        assert_eq!(runtime_source_transform("incremented_runtime_state"), None);
    }
}
