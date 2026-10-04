use anyhow::Result;

use crate::source_composition::{
    SourceChange, SourceChangeContribution, compose_disjoint_source_changes,
};

pub(crate) struct DecodedMenuChange<'a> {
    pub(crate) owner: &'a str,
    pub(crate) decoded: &'a [u8],
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ComposedDecodedMenu {
    pub(crate) decoded: Vec<u8>,
    pub(crate) changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) contributions: Vec<SourceChangeContribution>,
}

pub(crate) fn compose_disjoint_menu_changes(
    source: &[u8],
    changes: &[DecodedMenuChange<'_>],
) -> Result<ComposedDecodedMenu> {
    let source_changes = changes
        .iter()
        .map(|change| SourceChange {
            owner: change.owner,
            bytes: change.decoded,
        })
        .collect::<Vec<_>>();
    let composed = compose_disjoint_source_changes("decoded MENU", source, &source_changes)?;
    Ok(ComposedDecodedMenu {
        decoded: composed.bytes,
        changed_byte_ranges: composed.changed_byte_ranges,
        contributions: composed.contributions,
    })
}

#[cfg(test)]
#[path = "menu_composition_tests.rs"]
mod tests;
