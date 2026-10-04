use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use super::model::MenuNonTextEvidence;

#[path = "known_non_text/kanri.rs"]
mod kanri;
#[path = "known_non_text/koubai2.rs"]
mod koubai2;
#[path = "known_non_text/minisel.rs"]
mod minisel;
#[path = "known_non_text/plsel5.rs"]
mod plsel5;

pub(super) fn identify_known_non_text_evidence(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    let mut evidence = BTreeMap::new();
    for identified in [
        kanri::identify_counted_object_record_candidates(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
        )?,
        koubai2::identify_command_byte_sequence_candidates(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
        )?,
        minisel::identify_primitive_record_candidates(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
        )?,
        plsel5::identify_counted_parameter_pointer_candidates(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
        )?,
    ] {
        for (target, non_text_evidence) in identified {
            evidence
                .entry(target)
                .or_insert_with(Vec::new)
                .extend(non_text_evidence);
        }
    }
    Ok(evidence)
}

#[cfg(test)]
#[path = "known_non_text/kanri_tests.rs"]
mod kanri_tests;
#[cfg(test)]
#[path = "known_non_text/koubai2_tests.rs"]
mod koubai2_tests;
#[cfg(test)]
#[path = "known_non_text/minisel_tests.rs"]
mod minisel_tests;
#[cfg(test)]
#[path = "known_non_text/plsel5_tests.rs"]
mod plsel5_tests;
