use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const RELATIONSHIP_BUILDER_JUMP_TABLE_ADDRESS: u32 = 0x800c_a700;
pub(super) const RELATIONSHIP_BUILDER_CASE_ADDRESSES: [u32; 7] = [
    0x800a_fbf0,
    0x800a_fc58,
    0x800a_fcd4,
    0x800a_fd50,
    0x800a_fdac,
    0x800a_fe14,
    0x800a_fbd0,
];
pub(super) const RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES: [u32; 7] = [
    0x800a_fbc4,
    0x800a_fc34,
    0x800a_fc9c,
    0x800a_fd18,
    0x800a_fd94,
    0x800a_fdf0,
    0x800a_fe58,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelationshipNameFieldCaptureSiteReport {
    pub role: String,
    pub byte_range: [usize; 2],
    pub address: String,
    pub source_sha256: String,
    pub source_instruction: String,
    pub replacement_sha256: String,
    pub replacement_instruction: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelationshipNameFieldCaptureReport {
    pub sites: [RelationshipNameFieldCaptureSiteReport; 7],
    pub relationship_control_codes: [String; 6],
    pub relationship_builder_case_addresses: [String; 6],
    pub captured_register: String,
    pub field_categories: [String; 3],
    pub given_name_is_the_default_category: bool,
    pub family_and_nickname_categories_captured_after_classification: bool,
    pub jump_table_source_verified: bool,
    pub source_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn relationship_name_field_capture_replacements() -> [(u32, Instruction, &'static str); 7]
{
    [
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[0],
            Instruction::Ori {
                rt: Register::T9,
                rs: Register::ZERO,
                immediate: 1,
            },
            "initialize the relationship field as the given-name category",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[1],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2003",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[2],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2004",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[3],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2005",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[4],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2006",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[5],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2007",
        ),
        (
            RELATIONSHIP_NAME_FIELD_CAPTURE_ADDRESSES[6],
            category_capture_instruction(),
            "capture family or nickname for relationship control 0x2008",
        ),
    ]
}

pub(super) fn install_relationship_name_field_capture(
    source: &[u8],
    patched: &mut [u8],
) -> Result<RelationshipNameFieldCaptureReport> {
    let source_instruction = Instruction::nop();
    let replacements = relationship_name_field_capture_replacements();
    let mut verified = Vec::with_capacity(replacements.len());

    let jump_table_offset = runtime_offset(RELATIONSHIP_BUILDER_JUMP_TABLE_ADDRESS)?;
    let jump_table_end = jump_table_offset + RELATIONSHIP_BUILDER_CASE_ADDRESSES.len() * 4;
    let jump_table = source
        .get(jump_table_offset..jump_table_end)
        .context("relationship-name builder jump table is truncated")?;
    for (word, expected) in jump_table
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .zip(RELATIONSHIP_BUILDER_CASE_ADDRESSES)
    {
        ensure!(
            word == expected,
            "relationship-name builder jump table changed"
        );
    }

    for (address, replacement, role) in &replacements {
        let offset = runtime_offset(*address)?;
        let end = offset + 4;
        let source_bytes = source
            .get(offset..end)
            .context("relationship-name field capture source is truncated")?;
        ensure!(
            patched.get(offset..end) == Some(source_bytes),
            "another MGAME writer changed a relationship-name field capture site"
        );
        ensure!(
            decode(u32::from_le_bytes(source_bytes.try_into()?), *address)? == source_instruction,
            "relationship-name field capture source changed at 0x{address:08x}"
        );
        let replacement_bytes = encode(replacement, *address)?.to_le_bytes();
        verified.push((
            offset,
            source_bytes.to_vec(),
            replacement_bytes,
            replacement.clone(),
            *role,
        ));
    }

    let mut sites = Vec::with_capacity(verified.len());
    for ((address, _, _), (offset, source_bytes, replacement_bytes, replacement, role)) in
        replacements.iter().zip(verified)
    {
        let end = offset + 4;
        patched
            .get_mut(offset..end)
            .context("relationship-name field capture destination is truncated")?
            .copy_from_slice(&replacement_bytes);
        ensure!(
            decode(
                u32::from_le_bytes(patched[offset..end].try_into()?),
                *address,
            )? == replacement,
            "relationship-name field capture replacement changed at 0x{address:08x}"
        );
        sites.push(RelationshipNameFieldCaptureSiteReport {
            role: role.to_string(),
            byte_range: [offset, end],
            address: hex_address(*address),
            source_sha256: sha256_bytes(&source_bytes),
            source_instruction: format!("{source_instruction:?}"),
            replacement_sha256: sha256_bytes(&replacement_bytes),
            replacement_instruction: format!("{replacement:?}"),
        });
    }

    Ok(RelationshipNameFieldCaptureReport {
        sites: sites
            .try_into()
            .map_err(|_| anyhow::anyhow!("relationship-name capture site count changed"))?,
        relationship_control_codes: [0x2003_u16, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008]
            .map(|code| format!("0x{code:04x}")),
        relationship_builder_case_addresses: [
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[0],
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[1],
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[2],
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[3],
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[4],
            RELATIONSHIP_BUILDER_CASE_ADDRESSES[5],
        ]
        .map(hex_address),
        captured_register: "t9".to_string(),
        field_categories: ["family", "given", "nickname"].map(str::to_string),
        given_name_is_the_default_category: true,
        family_and_nickname_categories_captured_after_classification: true,
        jump_table_source_verified: true,
        source_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

fn category_capture_instruction() -> Instruction {
    Instruction::Addu {
        rd: Register::T9,
        rs: Register::V1,
        rt: Register::ZERO,
    }
}

fn runtime_offset(address: u32) -> Result<usize> {
    usize::try_from(
        address
            .checked_sub(MGAME_RUNTIME_BASE)
            .context("relationship-name field capture precedes MGAME runtime")?,
    )
    .map_err(Into::into)
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
