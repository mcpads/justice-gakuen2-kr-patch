use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use super::model::MenuConsumerEvidence;
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;

#[path = "known_consumers/kanri.rs"]
mod kanri;
#[path = "known_consumers/mgtit.rs"]
mod mgtit;
#[path = "known_consumers/newopt.rs"]
mod newopt;

pub(crate) use kanri::{
    school_label_consumers as kanri_school_label_consumers,
    source_menu_codes as kanri_source_menu_codes,
};

pub(super) fn identify_known_consumer_evidence(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
    let mut evidence = BTreeMap::new();
    for identified in [
        kanri::identify_placement_record_consumers(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
            reachable_lui_seed_offsets,
            direct_call_arguments,
        )?,
        mgtit::identify_placement_record_consumers(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
            reachable_lui_seed_offsets,
            direct_call_arguments,
        )?,
        newopt::identify_direct_string_writer_consumer(
            overlay_path,
            data,
            runtime_base,
            reachable_instruction_offsets,
        )?,
    ] {
        for (target, consumer_evidence) in identified {
            evidence
                .entry(target)
                .or_insert_with(Vec::new)
                .extend(consumer_evidence);
        }
    }
    Ok(evidence)
}

#[cfg(test)]
#[path = "known_consumers/kanri_tests.rs"]
mod kanri_tests;
#[cfg(test)]
#[path = "known_consumers/mgtit_tests.rs"]
mod mgtit_tests;
#[cfg(test)]
#[path = "known_consumers/newopt_tests.rs"]
mod newopt_tests;

pub(crate) use mgtit::validate_mgtit_placement_renderer;
