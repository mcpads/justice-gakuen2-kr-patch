use anyhow::{Result, ensure};

use crate::pipeline::difference_ranges;

pub(crate) struct SourceChange<'a> {
    pub(crate) owner: &'a str,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourceChangeContribution {
    pub(crate) owner: String,
    pub(crate) changed_byte_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ComposedSource {
    pub(crate) bytes: Vec<u8>,
    pub(crate) changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) contributions: Vec<SourceChangeContribution>,
}

pub(crate) fn compose_disjoint_source_changes(
    subject: &str,
    source: &[u8],
    changes: &[SourceChange<'_>],
) -> Result<ComposedSource> {
    ensure!(!changes.is_empty(), "no {subject} changes requested");

    let mut composed = source.to_vec();
    let mut claimed_by: Vec<Option<usize>> = vec![None; source.len()];
    let mut contributions = Vec::with_capacity(changes.len());

    for (change_index, change) in changes.iter().enumerate() {
        ensure!(
            change.bytes.len() == source.len(),
            "{} {subject} length changed from {} to {} bytes",
            change.owner,
            source.len(),
            change.bytes.len()
        );
        let changed_byte_ranges = difference_ranges(source, change.bytes);
        ensure!(
            !changed_byte_ranges.is_empty(),
            "{} changed no {subject} bytes",
            change.owner
        );

        for [start, end] in &changed_byte_ranges {
            for offset in *start..*end {
                if let Some(previous_index) = claimed_by[offset] {
                    ensure!(
                        false,
                        "{subject} changes overlap at 0x{offset:x}: {} and {}",
                        changes[previous_index].owner,
                        change.owner
                    );
                }
                claimed_by[offset] = Some(change_index);
                composed[offset] = change.bytes[offset];
            }
        }
        contributions.push(SourceChangeContribution {
            owner: change.owner.to_string(),
            changed_byte_ranges,
        });
    }

    let changed_byte_ranges = difference_ranges(source, &composed);
    ensure!(
        !changed_byte_ranges.is_empty(),
        "composed {subject} changed no bytes"
    );
    Ok(ComposedSource {
        bytes: composed,
        changed_byte_ranges,
        contributions,
    })
}

#[cfg(test)]
#[path = "source_composition_tests.rs"]
mod tests;
