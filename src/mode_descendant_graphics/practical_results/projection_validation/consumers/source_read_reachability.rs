//! Validates adopted loaded-image interface evidence for source-read consumers.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::super::super::projection_model::PracticalResultSourceReadReachabilityEvidence;
use super::super::footprint::SourceConsumerReachability;
use super::super::source_evidence::{
    ValidationCounts, address_and_span, checked_end, ensure_pointer_value, parse_hex, parse_hex_u32,
};

pub(super) fn validate_source_read_reachability(
    overlay: &[u8],
    runtime_base: u32,
    expected_consumer_runtime_addresses: &[&str],
    evidence: &PracticalResultSourceReadReachabilityEvidence,
    counts: &mut ValidationCounts,
) -> Result<SourceConsumerReachability> {
    match evidence {
        PracticalResultSourceReadReachabilityEvidence::AdoptedDeclaredEntrypointClosure {
            consumer_runtime_addresses,
        } => {
            ensure!(
                !expected_consumer_runtime_addresses.is_empty()
                    && consumer_runtime_addresses.len()
                        == expected_consumer_runtime_addresses.len()
                    && consumer_runtime_addresses
                        .iter()
                        .map(String::as_str)
                        .eq(expected_consumer_runtime_addresses.iter().copied()),
                "adopted entrypoint closure differs from the mechanism consumer addresses"
            );
            let loaded_end = runtime_base
                .checked_add(u32::try_from(overlay.len())?)
                .context("loaded-image runtime range overflow")?;
            let addresses = consumer_runtime_addresses
                .iter()
                .map(|address| parse_hex_u32(address, "adopted entrypoint-reachable consumer"))
                .collect::<Result<BTreeSet<_>>>()?;
            ensure!(
                addresses.len() == consumer_runtime_addresses.len()
                    && addresses.iter().all(|address| {
                        *address >= runtime_base && *address < loaded_end && address % 4 == 0
                    }),
                "adopted entrypoint closure has duplicate, unaligned, or external consumers"
            );
            Ok(SourceConsumerReachability::Closed)
        }
        PracticalResultSourceReadReachabilityEvidence::LoadedImageHeaderCallback {
            callback_pointer_offset,
            callback_pointer_runtime_address,
            callback_pointer_value,
            callback_pointer_sha256,
        } => {
            let pointer_offset = address_and_span(
                overlay,
                runtime_base,
                callback_pointer_offset,
                callback_pointer_runtime_address,
                4,
                callback_pointer_sha256,
                "loaded-image callback pointer",
                counts,
            )?;
            ensure!(
                expected_consumer_runtime_addresses.len() == 1,
                "header callback evidence requires one expected callback"
            );
            let expected_callback_address = parse_hex_u32(
                expected_consumer_runtime_addresses[0],
                "expected loaded-image callback address",
            )?;
            let pointer_value = parse_hex_u32(
                callback_pointer_value,
                "loaded-image callback pointer value",
            )?;
            ensure!(
                pointer_value == expected_callback_address,
                "loaded-image callback pointer does not name the declared callback"
            );
            ensure_pointer_value(
                overlay,
                pointer_offset,
                pointer_value,
                "loaded-image callback",
            )?;
            Ok(SourceConsumerReachability::Closed)
        }
        PracticalResultSourceReadReachabilityEvidence::DormantOutsideCompleteLoadedImageHeaderInterface {
            header_offset,
            header_runtime_address,
            header_size,
            header_sha256,
            code_callback_pointer_offsets,
            data_pointer_offsets,
        } => {
            ensure!(
                expected_consumer_runtime_addresses.len() == 1,
                "complete-header dormancy evidence requires one candidate"
            );
            let expected_candidate_address = parse_hex_u32(
                expected_consumer_runtime_addresses[0],
                "expected dormant candidate address",
            )?;
            let header_offset = address_and_span(
                overlay,
                runtime_base,
                header_offset,
                header_runtime_address,
                *header_size,
                header_sha256,
                "complete loaded-image header interface",
                counts,
            )?;
            ensure!(
                *header_size >= 8 && header_size.is_multiple_of(4),
                "loaded-image header interface is not a nontrivial word array"
            );
            let header_end = checked_end(
                header_offset,
                *header_size,
                "complete loaded-image header interface",
            )?;
            let code_offsets = parse_pointer_offsets(
                code_callback_pointer_offsets,
                "loaded-image code callback pointer offsets",
            )?;
            let data_offsets = parse_pointer_offsets(
                data_pointer_offsets,
                "loaded-image data pointer offsets",
            )?;
            ensure!(
                !code_offsets.is_empty() && !data_offsets.is_empty(),
                "loaded-image header interface must classify code and data pointers"
            );
            let classified_offsets = code_offsets
                .iter()
                .chain(&data_offsets)
                .copied()
                .collect::<BTreeSet<_>>();
            let expected_offsets = (header_offset..header_end).step_by(4).collect::<BTreeSet<_>>();
            ensure!(
                classified_offsets.len() == code_offsets.len() + data_offsets.len()
                    && classified_offsets == expected_offsets,
                "loaded-image header pointer classifications do not exactly partition the header"
            );

            let loaded_end = runtime_base
                .checked_add(u32::try_from(overlay.len())?)
                .context("loaded-image runtime range overflow")?;
            let code_targets = code_offsets
                .iter()
                .map(|&offset| read_loaded_pointer(overlay, offset, "code callback"))
                .collect::<Result<BTreeSet<_>>>()?;
            let data_targets = data_offsets
                .iter()
                .map(|&offset| read_loaded_pointer(overlay, offset, "data root"))
                .collect::<Result<BTreeSet<_>>>()?;
            ensure!(
                code_targets.len() == code_offsets.len()
                    && data_targets.len() == data_offsets.len()
                    && code_targets.is_disjoint(&data_targets)
                    && code_targets
                        .iter()
                        .chain(&data_targets)
                        .all(|target| {
                            *target >= runtime_base && *target < loaded_end && target % 4 == 0
                        }),
                "loaded-image header contains duplicate, unaligned, or external pointers"
            );
            ensure!(
                !code_targets.contains(&expected_candidate_address),
                "dormant candidate is exposed by the loaded-image code callback interface"
            );
            Ok(SourceConsumerReachability::DormantOutsideDeclaredEntrypoints)
        }
    }
}

fn parse_pointer_offsets(offsets: &[String], role: &str) -> Result<Vec<usize>> {
    ensure!(!offsets.is_empty(), "{role} are empty");
    offsets
        .iter()
        .map(|offset| parse_hex(offset, role))
        .collect()
}

fn read_loaded_pointer(data: &[u8], offset: usize, role: &str) -> Result<u32> {
    let end = checked_end(offset, 4, role)?;
    Ok(u32::from_le_bytes(
        data.get(offset..end)
            .with_context(|| format!("{role} pointer is truncated"))?
            .try_into()
            .expect("four-byte slice"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::sha256_bytes;

    const BASE: u32 = 0x8015_2000;

    #[test]
    fn adopted_closure_requires_the_exact_mechanism_consumer_population() {
        let overlay = vec![0; 0x80];
        let evidence =
            PracticalResultSourceReadReachabilityEvidence::AdoptedDeclaredEntrypointClosure {
                consumer_runtime_addresses: vec!["0x80152020".to_string()],
            };

        assert_eq!(
            validate_source_read_reachability(
                &overlay,
                BASE,
                &["0x80152020"],
                &evidence,
                &mut ValidationCounts::default(),
            )
            .unwrap(),
            SourceConsumerReachability::Closed
        );
        assert!(
            validate_source_read_reachability(
                &overlay,
                BASE,
                &["0x80152024"],
                &evidence,
                &mut ValidationCounts::default(),
            )
            .unwrap_err()
            .to_string()
            .contains("differs")
        );
    }

    #[test]
    fn complete_header_partition_classifies_an_unexposed_candidate_as_dormant() {
        let mut overlay = vec![0; 0x80];
        for (index, target_offset) in [0x20_u32, 0x30, 0x40, 0x50].into_iter().enumerate() {
            overlay[index * 4..index * 4 + 4]
                .copy_from_slice(&(BASE + target_offset).to_le_bytes());
        }
        let evidence = PracticalResultSourceReadReachabilityEvidence::DormantOutsideCompleteLoadedImageHeaderInterface {
            header_offset: "0x0".to_string(),
            header_runtime_address: format!("{BASE:#x}"),
            header_size: 16,
            header_sha256: sha256_bytes(&overlay[..16]),
            code_callback_pointer_offsets: vec!["0x0".to_string(), "0x4".to_string()],
            data_pointer_offsets: vec!["0x8".to_string(), "0xc".to_string()],
        };

        assert_eq!(
            validate_source_read_reachability(
                &overlay,
                BASE,
                &["0x80152060"],
                &evidence,
                &mut ValidationCounts::default(),
            )
            .unwrap(),
            SourceConsumerReachability::DormantOutsideDeclaredEntrypoints
        );
    }

    #[test]
    fn dormant_classification_rejects_a_candidate_present_in_a_code_slot() {
        let mut overlay = vec![0; 0x80];
        for (index, target_offset) in [0x20_u32, 0x30, 0x40, 0x50].into_iter().enumerate() {
            overlay[index * 4..index * 4 + 4]
                .copy_from_slice(&(BASE + target_offset).to_le_bytes());
        }
        let evidence = PracticalResultSourceReadReachabilityEvidence::DormantOutsideCompleteLoadedImageHeaderInterface {
            header_offset: "0x0".to_string(),
            header_runtime_address: format!("{BASE:#x}"),
            header_size: 16,
            header_sha256: sha256_bytes(&overlay[..16]),
            code_callback_pointer_offsets: vec!["0x0".to_string(), "0x4".to_string()],
            data_pointer_offsets: vec!["0x8".to_string(), "0xc".to_string()],
        };

        let error = validate_source_read_reachability(
            &overlay,
            BASE,
            &["0x80152020"],
            &evidence,
            &mut ValidationCounts::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("exposed"));
    }
}
