use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::pipeline::sha256_bytes;
use crate::tim::Cell;

use super::name_entry::{OVERLAY_RUNTIME_BASE, bounded_slice, decode_instruction};
use super::name_entry_model::{
    DialogueNameEntryChoiceDescriptorAudit, DialogueNameEntryChoiceSurfaceAudit,
};

const TEXTURE_PAGE_START: u8 = 0x0c;
const TEXTURE_PAGE_END: u8 = 0x0e;
const TEXTURE_PAGE_WIDTH: usize = 256;
const ATLAS_WIDTH: usize = 768;
const ATLAS_HEIGHT: usize = 256;
const DESCRIPTOR_SIZE: usize = 6;

#[derive(Clone, Copy)]
struct AddressMaterialization {
    lui_offset: usize,
    addiu_offset: usize,
    register: Register,
    target_offset: usize,
}

#[derive(Clone, Copy)]
struct LoopBound {
    instruction_offset: usize,
    counter: Register,
    exclusive_limit: i16,
}

#[derive(Clone, Copy)]
struct ChoiceSurfaceSpec {
    id: &'static str,
    descriptor_table_offset: usize,
    descriptor_count: usize,
    logical_choice_count: usize,
    text_descriptor_indexes: &'static [usize],
    descriptor_table_sha256: &'static str,
    address_materializations: &'static [AddressMaterialization],
    loop_bounds: &'static [LoopBound],
}

const SCHOOL_TEXT_DESCRIPTOR_INDEXES: [usize; 5] = [1, 3, 5, 7, 9];
const SCHOOL_ADDRESS_MATERIALIZATIONS: [AddressMaterialization; 2] = [
    AddressMaterialization {
        lui_offset: 0x7df0,
        addiu_offset: 0x7df4,
        register: Register::S2,
        target_offset: 0x15f8,
    },
    AddressMaterialization {
        lui_offset: 0x8000,
        addiu_offset: 0x8004,
        register: Register::V0,
        target_offset: 0x15fe,
    },
];
const SCHOOL_LOOP_BOUNDS: [LoopBound; 2] = [
    LoopBound {
        instruction_offset: 0x7f74,
        counter: Register::S4,
        exclusive_limit: 2,
    },
    LoopBound {
        instruction_offset: 0x7f84,
        counter: Register::S7,
        exclusive_limit: 5,
    },
];

const SUBJECT_TEXT_DESCRIPTOR_INDEXES: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const SUBJECT_ADDRESS_MATERIALIZATIONS: [AddressMaterialization; 2] = [
    AddressMaterialization {
        lui_offset: 0x3f08,
        addiu_offset: 0x3f0c,
        register: Register::S4,
        target_offset: 0x1674,
    },
    AddressMaterialization {
        lui_offset: 0x4104,
        addiu_offset: 0x4108,
        register: Register::V0,
        target_offset: 0x1674,
    },
];
const SUBJECT_LOOP_BOUNDS: [LoopBound; 1] = [LoopBound {
    instruction_offset: 0x4088,
    counter: Register::S6,
    exclusive_limit: 8,
}];

const FAVORITE_WORD_TEXT_DESCRIPTOR_INDEXES: [usize; 16] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const FAVORITE_WORD_ADDRESS_MATERIALIZATIONS: [AddressMaterialization; 2] = [
    AddressMaterialization {
        lui_offset: 0x4834,
        addiu_offset: 0x4838,
        register: Register::S1,
        target_offset: 0x1724,
    },
    AddressMaterialization {
        lui_offset: 0x4a50,
        addiu_offset: 0x4a54,
        register: Register::V0,
        target_offset: 0x1724,
    },
];
const FAVORITE_WORD_LOOP_BOUNDS: [LoopBound; 1] = [LoopBound {
    instruction_offset: 0x49a0,
    counter: Register::S5,
    exclusive_limit: 16,
}];

const CHOICE_SURFACES: [ChoiceSurfaceSpec; 3] = [
    ChoiceSurfaceSpec {
        id: "school-choices",
        descriptor_table_offset: 0x15f8,
        descriptor_count: 10,
        logical_choice_count: 5,
        text_descriptor_indexes: &SCHOOL_TEXT_DESCRIPTOR_INDEXES,
        descriptor_table_sha256: "518096fc383bf7ef932371bf42eca5a4dd1971f22da01b86d59e6b21445adcfe",
        address_materializations: &SCHOOL_ADDRESS_MATERIALIZATIONS,
        loop_bounds: &SCHOOL_LOOP_BOUNDS,
    },
    ChoiceSurfaceSpec {
        id: "subject-choices",
        descriptor_table_offset: 0x1674,
        descriptor_count: 8,
        logical_choice_count: 8,
        text_descriptor_indexes: &SUBJECT_TEXT_DESCRIPTOR_INDEXES,
        descriptor_table_sha256: "55b4a193a4b0225407b92503362f8711b6cf900d0ff2586f01ab03d5aded56f8",
        address_materializations: &SUBJECT_ADDRESS_MATERIALIZATIONS,
        loop_bounds: &SUBJECT_LOOP_BOUNDS,
    },
    ChoiceSurfaceSpec {
        id: "favorite-word-choices",
        descriptor_table_offset: 0x1724,
        descriptor_count: 16,
        logical_choice_count: 16,
        text_descriptor_indexes: &FAVORITE_WORD_TEXT_DESCRIPTOR_INDEXES,
        descriptor_table_sha256: "e48eb7e9a8edeff529efb47cb0810072f3cc5d83a5580c048fa67bf4328e627a",
        address_materializations: &FAVORITE_WORD_ADDRESS_MATERIALIZATIONS,
        loop_bounds: &FAVORITE_WORD_LOOP_BOUNDS,
    },
];

pub(super) fn audit_name_entry_choice_descriptors(
    overlay: &[u8],
) -> Result<Vec<DialogueNameEntryChoiceSurfaceAudit>> {
    CHOICE_SURFACES
        .iter()
        .copied()
        .map(|spec| audit_surface(overlay, spec))
        .collect()
}

fn audit_surface(
    overlay: &[u8],
    spec: ChoiceSurfaceSpec,
) -> Result<DialogueNameEntryChoiceSurfaceAudit> {
    let table_size = spec.descriptor_count * DESCRIPTOR_SIZE;
    let table = bounded_slice(overlay, spec.descriptor_table_offset, table_size)?;
    ensure!(
        sha256_bytes(table) == spec.descriptor_table_sha256,
        "{} descriptor table changed",
        spec.id
    );
    ensure!(
        spec.text_descriptor_indexes.len() == spec.logical_choice_count,
        "{} text descriptor population changed",
        spec.id
    );
    ensure!(
        spec.text_descriptor_indexes
            .iter()
            .all(|index| *index < spec.descriptor_count),
        "{} text descriptor index is outside its table",
        spec.id
    );

    validate_address_materializations(overlay, spec)?;
    validate_loop_bounds(overlay, spec)?;

    let descriptors = table
        .as_chunks::<DESCRIPTOR_SIZE>()
        .0
        .iter()
        .enumerate()
        .map(|(index, descriptor)| parse_descriptor(index, descriptor))
        .collect::<Result<Vec<_>>>()?;

    Ok(DialogueNameEntryChoiceSurfaceAudit {
        id: spec.id.to_string(),
        descriptor_table_file_offset: format!("0x{:04x}", spec.descriptor_table_offset),
        descriptor_table_runtime_address: format!(
            "0x{:08x}",
            OVERLAY_RUNTIME_BASE + u32::try_from(spec.descriptor_table_offset)?
        ),
        descriptor_table_sha256: spec.descriptor_table_sha256.to_string(),
        descriptor_size: DESCRIPTOR_SIZE,
        descriptor_count: spec.descriptor_count,
        logical_choice_count: spec.logical_choice_count,
        text_descriptor_indexes: spec.text_descriptor_indexes.to_vec(),
        producer_address_materialization_file_offsets: spec
            .address_materializations
            .iter()
            .flat_map(|reference| [reference.lui_offset, reference.addiu_offset])
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        producer_loop_bound_file_offsets: spec
            .loop_bounds
            .iter()
            .map(|bound| format!("0x{:04x}", bound.instruction_offset))
            .collect(),
        descriptors,
    })
}

fn validate_address_materializations(overlay: &[u8], spec: ChoiceSurfaceSpec) -> Result<()> {
    for reference in spec.address_materializations {
        let address = OVERLAY_RUNTIME_BASE + u32::try_from(reference.target_offset)?;
        let adjusted_high = u16::try_from((address + 0x8000) >> 16)?;
        let low = address as u16 as i16;
        ensure!(
            decode_instruction(overlay, reference.lui_offset)?
                == Instruction::Lui {
                    rt: reference.register,
                    immediate: adjusted_high,
                }
                && decode_instruction(overlay, reference.addiu_offset)?
                    == Instruction::Addiu {
                        rt: reference.register,
                        rs: reference.register,
                        immediate: low,
                    },
            "{} descriptor producer changed at {:#x}",
            spec.id,
            reference.lui_offset
        );
    }
    Ok(())
}

fn validate_loop_bounds(overlay: &[u8], spec: ChoiceSurfaceSpec) -> Result<()> {
    for bound in spec.loop_bounds {
        ensure!(
            decode_instruction(overlay, bound.instruction_offset)?
                == Instruction::Slti {
                    rt: Register::V0,
                    rs: bound.counter,
                    immediate: bound.exclusive_limit,
                },
            "{} descriptor loop bound changed at {:#x}",
            spec.id,
            bound.instruction_offset
        );
    }
    Ok(())
}

fn parse_descriptor(
    index: usize,
    descriptor: &[u8],
) -> Result<DialogueNameEntryChoiceDescriptorAudit> {
    let [texture_page, palette, u, v, width, height]: [u8; DESCRIPTOR_SIZE] =
        descriptor.try_into()?;
    ensure!(
        (TEXTURE_PAGE_START..=TEXTURE_PAGE_END).contains(&texture_page),
        "name-entry choice descriptor has an unknown texture page"
    );
    ensure!(
        width > 0 && height > 0,
        "empty name-entry choice descriptor"
    );
    let x = usize::from(texture_page - TEXTURE_PAGE_START) * TEXTURE_PAGE_WIDTH + usize::from(u);
    let y = usize::from(v);
    let cell = Cell {
        x,
        y,
        width: usize::from(width),
        height: usize::from(height),
    };
    ensure!(
        cell.x + cell.width <= ATLAS_WIDTH && cell.y + cell.height <= ATLAS_HEIGHT,
        "name-entry choice descriptor is outside the atlas"
    );
    Ok(DialogueNameEntryChoiceDescriptorAudit {
        index,
        texture_page,
        palette,
        cell,
    })
}
