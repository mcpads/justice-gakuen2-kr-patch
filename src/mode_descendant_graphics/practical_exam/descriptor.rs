use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, cells_overlap};

use super::catalog::PracticalExamDescriptorBinding;

#[path = "direct_census.rs"]
mod direct_census;
#[path = "direct_texture_reads.rs"]
mod direct_texture_reads;

use super::super::model::{
    PracticalExamDirectTextureCensusAudit, PracticalExamSecondaryDescriptorAudit,
    PracticalExamSecondaryDescriptorCensusAudit, PracticalExamSecondaryDescriptorFragmentAudit,
    PracticalExamSecondaryTextureBankAudit,
};

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const FIRST_TEXTURE_PAGE: u8 = 0x0c;
const TEXTURE_PAGE_COUNT: u8 = 3;
const TEXTURE_PAGE_WIDTH: usize = 256;
const TEXTURE_WIDTH: usize = 768;
const TEXTURE_HEIGHT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub(in crate::mode_descendant_graphics) enum PracticalExamConsumer {
    BasicsReview,
    Exam1999,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PracticalExamTextureRegion {
    pub(super) id: &'static str,
    pub(super) cell: Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PracticalExamFragment {
    pub(super) texture_page: u8,
    pub(super) u: u8,
    pub(super) v: u8,
    pub(super) width: u8,
    pub(super) height: u8,
}

impl PracticalExamFragment {
    pub(super) fn cell(self) -> Cell {
        Cell {
            x: usize::from(self.texture_page - FIRST_TEXTURE_PAGE) * TEXTURE_PAGE_WIDTH
                + usize::from(self.u),
            y: usize::from(self.v),
            width: usize::from(self.width),
            height: usize::from(self.height),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PracticalExamCountedDescriptor {
    pub(super) index: usize,
    pub(super) offset: usize,
    pub(super) capacity: usize,
    pub(super) source_sha256: String,
    pub(super) fragments: Vec<PracticalExamFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mode_descendant_graphics) enum PracticalExamSecondaryTextureBank {
    SharedProducer,
    External,
}

impl PracticalExamSecondaryTextureBank {
    fn parse(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::SharedProducer),
            1 => Ok(Self::External),
            _ => anyhow::bail!("practical-exam secondary descriptor has unknown texture bank"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mode_descendant_graphics) struct PracticalExamSecondaryDescriptorFragment {
    pub(in crate::mode_descendant_graphics) texture_bank: PracticalExamSecondaryTextureBank,
    pub(in crate::mode_descendant_graphics) texture_page: u8,
    pub(in crate::mode_descendant_graphics) clut_x_index: u8,
    pub(in crate::mode_descendant_graphics) clut_y_offset: u8,
    pub(in crate::mode_descendant_graphics) u: u8,
    pub(in crate::mode_descendant_graphics) v: u8,
    pub(in crate::mode_descendant_graphics) width: u8,
    pub(in crate::mode_descendant_graphics) height: u8,
}

impl PracticalExamSecondaryDescriptorFragment {
    fn fragment(self) -> PracticalExamFragment {
        PracticalExamFragment {
            texture_page: self.texture_page,
            u: self.u,
            v: self.v,
            width: self.width,
            height: self.height,
        }
    }

    fn shared_producer_region(self) -> Option<PracticalExamTextureRegion> {
        (self.texture_bank == PracticalExamSecondaryTextureBank::SharedProducer).then(|| {
            PracticalExamTextureRegion {
                id: "retained-secondary-descriptor",
                cell: self.fragment().cell(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::mode_descendant_graphics) struct PracticalExamSecondaryDescriptor {
    pub(in crate::mode_descendant_graphics) descriptor_index_aliases: Vec<usize>,
    pub(in crate::mode_descendant_graphics) offset: usize,
    pub(in crate::mode_descendant_graphics) capacity: usize,
    pub(in crate::mode_descendant_graphics) source_sha256: String,
    pub(in crate::mode_descendant_graphics) fragments:
        Vec<PracticalExamSecondaryDescriptorFragment>,
}

pub(super) struct PracticalExamConsumerAudit {
    pub(super) consumer: PracticalExamConsumer,
    pub(super) descriptors: Vec<PracticalExamCountedDescriptor>,
    secondary_descriptors: Vec<PracticalExamSecondaryDescriptor>,
    pub(super) direct_regions: Vec<PracticalExamTextureRegion>,
    pub(super) direct_texture_reads_complete: bool,
}

struct ConsumerLayout {
    consumer: PracticalExamConsumer,
    source_sha256: &'static str,
    pointer_table_offset: usize,
    descriptor_count: usize,
    pointer_table_sha256: &'static str,
    renderer_offset: usize,
    renderer_size: usize,
    renderer_sha256: &'static str,
    caller_offset: usize,
    caller_size: usize,
    caller_sha256: &'static str,
    secondary_table: Option<SecondaryDescriptorLayout>,
    unresolved_dynamic_tiles: Option<UnresolvedDynamicTileLayout>,
}

#[derive(Clone, Copy)]
struct SecondaryDescriptorLayout {
    pointer_table_offset: usize,
    descriptor_count: usize,
    pointer_table_sha256: &'static str,
    descriptor_arena_start: usize,
    descriptor_arena_end: usize,
    renderer_offset: usize,
    renderer_size: usize,
    renderer_sha256: &'static str,
    lookup_offset: usize,
    lookup_size: usize,
    lookup_sha256: &'static str,
}

#[derive(Clone, Copy)]
struct UnresolvedDynamicTileLayout {
    renderer_offset: usize,
    renderer_size: usize,
    renderer_sha256: &'static str,
    setup_offset: usize,
    setup_size: usize,
    setup_sha256: &'static str,
}

const BASICS_LAYOUT: ConsumerLayout = ConsumerLayout {
    consumer: PracticalExamConsumer::BasicsReview,
    source_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
    pointer_table_offset: 0x0420,
    descriptor_count: 49,
    pointer_table_sha256: "b3375d17687e64de361dc6efd6660f46bb116a9126188ce8e605ad6998997425",
    renderer_offset: 0x1f74,
    renderer_size: 0x1f4,
    renderer_sha256: "a14ce534f52cc26986243e5cf3f9d5bb68828070d42678dc1188290a7e08021b",
    caller_offset: 0x7540,
    caller_size: 0x9c,
    caller_sha256: "605fa2422ec8172cd050300fc8648f83509ee8c98be8ed007b1128d4feab9ef4",
    secondary_table: Some(SecondaryDescriptorLayout {
        pointer_table_offset: 0x07ec,
        descriptor_count: 60,
        pointer_table_sha256: "3ff7c00b2534c68aa66098710f8d09b982a0cbe3480ee40ade1ec9c20c869afb",
        descriptor_arena_start: 0x04e4,
        descriptor_arena_end: 0x07ec,
        renderer_offset: 0x4204,
        renderer_size: 548,
        renderer_sha256: "1b55b29dea6173847119ed4aa2ee10896b9d8f0c89bbd6fd3bab2ff8b811809b",
        lookup_offset: 0x4244,
        lookup_size: 20,
        lookup_sha256: "dc6db154fed57b6c372fda6a36197bdf140f8491de25f0653acfb22973d6ccf5",
    }),
    unresolved_dynamic_tiles: Some(UnresolvedDynamicTileLayout {
        renderer_offset: 0x4b10,
        renderer_size: 696,
        renderer_sha256: "cb21963b6e5c3a4b6eab460f9093ed9f8dc6892916731b0cb244ae754a7f07cf",
        setup_offset: 0x4c1c,
        setup_size: 24,
        setup_sha256: "e4fbbbde5743feba21e816e3f7121595e63aa4907dcfb230f1cd918d861bb407",
    }),
};

const EXAM_1999_LAYOUT: ConsumerLayout = ConsumerLayout {
    consumer: PracticalExamConsumer::Exam1999,
    source_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
    pointer_table_offset: 0x0210,
    descriptor_count: 8,
    pointer_table_sha256: "f83999abc42b002579cb4f60f7dda055b3d0eaa5578fccade145f57991cf863e",
    renderer_offset: 0x1890,
    renderer_size: 0x1f4,
    renderer_sha256: "3a510490dfb0bf87b309669a4b6bf7cf233d2351fb7fb07431edfb47d141aa99",
    caller_offset: 0x760c,
    caller_size: 0x9c,
    caller_sha256: "822365f47677d3dd7d7d7c628bf5302592c92cc8566a3d7d5a13d6522d74fab1",
    secondary_table: Some(SecondaryDescriptorLayout {
        pointer_table_offset: 0x0538,
        descriptor_count: 60,
        pointer_table_sha256: "5b8aa264c49a65f55cee3b665eceeb7b75b8ed4b5cca995ccaecac15db18e7d4",
        descriptor_arena_start: 0x0230,
        descriptor_arena_end: 0x0538,
        renderer_offset: 0x392c,
        renderer_size: 548,
        renderer_sha256: "14f91e490caa2e9a9953d0f58ae972127cb95e66fc856c72096e9ef9da661468",
        lookup_offset: 0x396c,
        lookup_size: 20,
        lookup_sha256: "eff4daab8b03e57c2d66c2c48a39ebc5d9ca21d122bdca7a58cd80cd4d383ae2",
    }),
    unresolved_dynamic_tiles: Some(UnresolvedDynamicTileLayout {
        renderer_offset: 0x4238,
        renderer_size: 0x2b8,
        renderer_sha256: "c26a5246caef2a5e6929bd8611c10822c6db9277fab4535a4f9db0ee42448e0c",
        setup_offset: 0x4344,
        setup_size: 0x18,
        setup_sha256: "e52422a4c4f410a4151ab5afeb860c1930a8bb25eb1035f5d9214eaaef2758ce",
    }),
};

pub(super) fn audit_practical_exam_consumer(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<PracticalExamConsumerAudit> {
    let layout = layout(consumer);
    ensure!(
        sha256_bytes(overlay) == layout.source_sha256,
        "{consumer:?} practical-exam consumer source changed"
    );
    ensure_guarded_region(
        overlay,
        layout.pointer_table_offset,
        layout.descriptor_count * 4,
        layout.pointer_table_sha256,
        "descriptor pointer table",
    )?;
    ensure_guarded_region(
        overlay,
        layout.renderer_offset,
        layout.renderer_size,
        layout.renderer_sha256,
        "counted-fragment renderer",
    )?;
    ensure_guarded_region(
        overlay,
        layout.caller_offset,
        layout.caller_size,
        layout.caller_sha256,
        "counted-fragment caller",
    )?;
    if let Some(dynamic) = layout.unresolved_dynamic_tiles {
        ensure_guarded_region(
            overlay,
            dynamic.renderer_offset,
            dynamic.renderer_size,
            dynamic.renderer_sha256,
            "unresolved dynamic-tile renderer",
        )?;
        ensure_guarded_region(
            overlay,
            dynamic.setup_offset,
            dynamic.setup_size,
            dynamic.setup_sha256,
            "unresolved dynamic-tile setup",
        )?;
        validate_unresolved_upload_target(overlay, dynamic)?;
    }
    let mut offsets = Vec::with_capacity(layout.descriptor_count);
    for index in 0..layout.descriptor_count {
        let pointer_offset = layout.pointer_table_offset + index * 4;
        let runtime_address = u32::from_le_bytes(
            overlay[pointer_offset..pointer_offset + 4]
                .try_into()
                .context("truncated practical-exam descriptor pointer")?,
        );
        ensure!(
            runtime_address >= OVERLAY_RUNTIME_BASE,
            "{consumer:?} descriptor {index} points below its overlay"
        );
        let offset = usize::try_from(runtime_address - OVERLAY_RUNTIME_BASE)?;
        ensure!(
            offset < layout.pointer_table_offset,
            "{consumer:?} descriptor {index} points outside the descriptor arena"
        );
        offsets.push(offset);
    }
    ensure!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{consumer:?} descriptor pointers are not strictly increasing"
    );

    let mut descriptors = Vec::with_capacity(layout.descriptor_count);
    let mut protected_regions = Vec::new();
    for (index, offset) in offsets.iter().copied().enumerate() {
        let count =
            usize::from(*overlay.get(offset).with_context(|| {
                format!("{consumer:?} descriptor {index} is outside its overlay")
            })?);
        ensure!(count > 0, "{consumer:?} descriptor {index} is empty");
        let used = 1usize
            .checked_add(count.checked_mul(5).context("descriptor size overflow")?)
            .context("descriptor size overflow")?;
        let capacity = used.next_multiple_of(4);
        let next_offset = offsets
            .get(index + 1)
            .copied()
            .unwrap_or(layout.pointer_table_offset);
        ensure!(
            offset + capacity <= next_offset,
            "{consumer:?} descriptor {index} overlaps the next descriptor"
        );
        let bytes = overlay
            .get(offset..offset + capacity)
            .with_context(|| format!("{consumer:?} descriptor {index} is truncated"))?;
        ensure!(
            bytes[used..].iter().all(|byte| *byte == 0),
            "{consumer:?} descriptor {index} has nonzero alignment padding"
        );
        let mut fragments = Vec::with_capacity(count);
        for fragment_index in 0..count {
            let fragment_offset = 1 + fragment_index * 5;
            let fragment = PracticalExamFragment {
                texture_page: bytes[fragment_offset],
                u: bytes[fragment_offset + 1],
                v: bytes[fragment_offset + 2],
                width: bytes[fragment_offset + 3],
                height: bytes[fragment_offset + 4],
            };
            ensure!(
                (FIRST_TEXTURE_PAGE..FIRST_TEXTURE_PAGE + TEXTURE_PAGE_COUNT)
                    .contains(&fragment.texture_page),
                "{consumer:?} descriptor {index} has an unknown texture page"
            );
            ensure!(
                fragment.width > 0
                    && fragment.height > 0
                    && usize::from(fragment.u) + usize::from(fragment.width) <= TEXTURE_PAGE_WIDTH
                    && usize::from(fragment.v) + usize::from(fragment.height) <= TEXTURE_HEIGHT,
                "{consumer:?} descriptor {index} has invalid fragment geometry"
            );
            protected_regions.push(PracticalExamTextureRegion {
                id: "counted-fragment-descriptor",
                cell: fragment.cell(),
            });
            fragments.push(fragment);
        }
        descriptors.push(PracticalExamCountedDescriptor {
            index,
            offset,
            capacity,
            source_sha256: sha256_bytes(bytes),
            fragments,
        });
    }
    let secondary_descriptors = match layout.secondary_table {
        Some(secondary) => parse_secondary_descriptors(overlay, consumer, secondary)?,
        None => Vec::new(),
    };
    let retained_secondary_regions = secondary_descriptors
        .iter()
        .flat_map(|descriptor| descriptor.fragments.iter().copied())
        .filter_map(PracticalExamSecondaryDescriptorFragment::shared_producer_region)
        .collect::<Vec<_>>();
    protected_regions.extend_from_slice(&retained_secondary_regions);
    let direct_texture_reads =
        direct_texture_reads::load_direct_texture_reads(overlay, consumer, layout.source_sha256)?;
    let direct_regions = direct_texture_reads.regions;
    protected_regions.extend_from_slice(&direct_regions);
    validate_protected_regions(&protected_regions)?;
    Ok(PracticalExamConsumerAudit {
        consumer,
        descriptors,
        secondary_descriptors,
        direct_regions,
        direct_texture_reads_complete: true,
    })
}

pub(in crate::mode_descendant_graphics) fn audit_practical_exam_direct_texture_census(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    source_path: &str,
) -> Result<PracticalExamDirectTextureCensusAudit> {
    let expected_source_sha256 = layout(consumer).source_sha256;
    let census = direct_census::audit_practical_exam_direct_census(
        overlay,
        consumer,
        expected_source_sha256,
    )?;
    ensure!(
        census.complete,
        "{consumer:?} practical-exam direct-texture census is incomplete"
    );
    let adopted =
        direct_texture_reads::load_direct_texture_reads(overlay, consumer, expected_source_sha256)?;
    let census_cells = canonical_texture_cells(&census.direct_regions);
    let adopted_cells = canonical_texture_cells(&adopted.regions);
    ensure!(
        census_cells == adopted_cells,
        "{consumer:?} practical-exam frozen direct-texture footprint differs from its full census: census {}, adopted {} cells",
        census_cells.len(),
        adopted_cells.len()
    );
    Ok(PracticalExamDirectTextureCensusAudit {
        consumer: match consumer {
            PracticalExamConsumer::BasicsReview => "basics_review",
            PracticalExamConsumer::Exam1999 => "exam_1999",
        }
        .to_string(),
        source_path: source_path.to_string(),
        source_sha256: expected_source_sha256.to_string(),
        full_census_complete: true,
        census_region_count: census.direct_regions.len(),
        adopted_region_count: adopted.regions.len(),
        canonical_cell_count: adopted_cells.len(),
        canonical_cell_set_matches: true,
        canonical_cells: adopted_cells
            .into_iter()
            .map(|(x, y, width, height)| Cell {
                x,
                y,
                width,
                height,
            })
            .collect(),
    })
}

pub(in crate::mode_descendant_graphics) fn audit_practical_exam_secondary_descriptors(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    source_path: &str,
) -> Result<PracticalExamSecondaryDescriptorCensusAudit> {
    let consumer_layout = layout(consumer);
    let secondary_layout = consumer_layout
        .secondary_table
        .context("practical-exam consumer has no secondary descriptor table")?;
    let audit = audit_practical_exam_consumer(overlay, consumer)?;
    secondary_descriptor_census(
        consumer,
        source_path,
        consumer_layout.source_sha256,
        secondary_layout,
        &audit.secondary_descriptors,
    )
}

fn secondary_descriptor_census(
    consumer: PracticalExamConsumer,
    source_path: &str,
    source_sha256: &str,
    layout: SecondaryDescriptorLayout,
    descriptors: &[PracticalExamSecondaryDescriptor],
) -> Result<PracticalExamSecondaryDescriptorCensusAudit> {
    let alias_count = descriptors
        .iter()
        .map(|descriptor| descriptor.descriptor_index_aliases.len())
        .sum::<usize>();
    let aliases = descriptors
        .iter()
        .flat_map(|descriptor| descriptor.descriptor_index_aliases.iter().copied())
        .collect::<BTreeSet<_>>();
    ensure!(
        alias_count == layout.descriptor_count && aliases == (0..layout.descriptor_count).collect(),
        "{consumer:?} secondary descriptor census lost a pointer-table alias"
    );
    Ok(PracticalExamSecondaryDescriptorCensusAudit {
        consumer: match consumer {
            PracticalExamConsumer::BasicsReview => "basics_review",
            PracticalExamConsumer::Exam1999 => "exam_1999",
        }
        .to_string(),
        source_path: source_path.to_string(),
        source_sha256: source_sha256.to_string(),
        pointer_table_offset: layout.pointer_table_offset,
        pointer_table_entry_count: layout.descriptor_count,
        pointer_table_sha256: layout.pointer_table_sha256.to_string(),
        descriptor_arena_start: layout.descriptor_arena_start,
        descriptor_arena_end: layout.descriptor_arena_end,
        physical_descriptor_count: descriptors.len(),
        alias_partition_complete: true,
        descriptors: descriptors
            .iter()
            .map(|descriptor| PracticalExamSecondaryDescriptorAudit {
                aliases: descriptor.descriptor_index_aliases.clone(),
                source_offset: descriptor.offset,
                encoded_size: descriptor.capacity,
                source_sha256: descriptor.source_sha256.clone(),
                fragments: descriptor
                    .fragments
                    .iter()
                    .map(|fragment| PracticalExamSecondaryDescriptorFragmentAudit {
                        texture_bank: match fragment.texture_bank {
                            PracticalExamSecondaryTextureBank::SharedProducer => {
                                PracticalExamSecondaryTextureBankAudit::SharedProducer
                            }
                            PracticalExamSecondaryTextureBank::External => {
                                PracticalExamSecondaryTextureBankAudit::External
                            }
                        },
                        texture_page: fragment.texture_page,
                        clut_x_index: fragment.clut_x_index,
                        clut_y_offset: fragment.clut_y_offset,
                        source_cell: Cell {
                            x: usize::from(fragment.u),
                            y: usize::from(fragment.v),
                            width: usize::from(fragment.width),
                            height: usize::from(fragment.height),
                        },
                    })
                    .collect(),
            })
            .collect(),
    })
}

fn canonical_texture_cells(
    regions: &[PracticalExamTextureRegion],
) -> BTreeSet<(usize, usize, usize, usize)> {
    regions
        .iter()
        .map(|region| {
            (
                region.cell.x,
                region.cell.y,
                region.cell.width,
                region.cell.height,
            )
        })
        .collect()
}

pub(in crate::mode_descendant_graphics) fn parse_practical_exam_secondary_descriptor(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    descriptor_offset: usize,
    expected_aliases: &[usize],
) -> Result<PracticalExamSecondaryDescriptor> {
    let consumer_layout = layout(consumer);
    ensure!(
        sha256_bytes(overlay) == consumer_layout.source_sha256,
        "{consumer:?} practical-exam secondary descriptor source changed"
    );
    let secondary_layout = consumer_layout
        .secondary_table
        .context("practical-exam consumer has no secondary descriptor table")?;
    ensure_guarded_region(
        overlay,
        secondary_layout.pointer_table_offset,
        secondary_layout.descriptor_count * 4,
        secondary_layout.pointer_table_sha256,
        "secondary descriptor pointer table",
    )?;

    let descriptor_offsets =
        read_secondary_descriptor_offsets(overlay, consumer, secondary_layout)?;
    let actual_aliases = descriptor_offsets
        .iter()
        .enumerate()
        .filter_map(|(index, offset)| (*offset == descriptor_offset).then_some(index))
        .collect::<Vec<_>>();
    ensure!(
        !actual_aliases.is_empty() && actual_aliases == expected_aliases,
        "{consumer:?} secondary descriptor at +0x{descriptor_offset:04x} aliases changed"
    );
    let next_offset = descriptor_offsets
        .iter()
        .copied()
        .filter(|offset| *offset > descriptor_offset)
        .min()
        .unwrap_or(secondary_layout.descriptor_arena_end);

    parse_secondary_descriptor(
        overlay,
        consumer,
        descriptor_offset,
        next_offset,
        actual_aliases,
    )
}

pub(in crate::mode_descendant_graphics) fn external_secondary_descriptors_avoid_source_cell(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    source_cell: Cell,
) -> Result<bool> {
    let audit = audit_practical_exam_consumer(overlay, consumer)?;
    Ok(audit.secondary_descriptors.iter().all(|descriptor| {
        descriptor.fragments.iter().all(|fragment| {
            fragment.texture_bank != PracticalExamSecondaryTextureBank::External
                || fragment.texture_page != 0x0e
                || !cells_overlap(
                    source_cell,
                    Cell {
                        x: usize::from(fragment.u),
                        y: usize::from(fragment.v),
                        width: usize::from(fragment.width),
                        height: usize::from(fragment.height),
                    },
                )
        })
    }))
}

fn validate_unresolved_upload_target(
    overlay: &[u8],
    layout: UnresolvedDynamicTileLayout,
) -> Result<()> {
    const ZERO: Register = Register::ZERO;
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;

    let expected = [
        Instruction::Addu {
            rd: A0,
            rs: ZERO,
            rt: ZERO,
        },
        Instruction::Addu {
            rd: A1,
            rs: ZERO,
            rt: ZERO,
        },
        Instruction::Addiu {
            rt: A2,
            rs: ZERO,
            immediate: 0x0380,
        },
        Instruction::Addiu {
            rt: A3,
            rs: ZERO,
            immediate: 0x0100,
        },
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let offset = layout.setup_offset + index * 4;
        let bytes = overlay
            .get(offset..offset + 4)
            .context("practical-exam unresolved upload setup is truncated")?;
        let actual = decode(
            u32::from_le_bytes(bytes.try_into()?),
            OVERLAY_RUNTIME_BASE + u32::try_from(offset)?,
        )?;
        ensure!(
            actual == expected,
            "practical-exam unresolved upload setup changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

pub(super) fn validate_bound_descriptor(
    audit: &PracticalExamConsumerAudit,
    binding: PracticalExamDescriptorBinding,
) -> Result<&PracticalExamCountedDescriptor> {
    ensure!(
        audit.consumer == binding.consumer,
        "practical-exam descriptor binding targets the wrong consumer"
    );
    let descriptor = audit
        .descriptors
        .get(binding.descriptor_index)
        .context("practical-exam descriptor binding index is outside its table")?;
    ensure!(
        descriptor.offset == binding.descriptor_offset
            && descriptor.capacity == binding.descriptor_capacity
            && descriptor.source_sha256 == binding.source_descriptor_sha256,
        "practical-exam descriptor binding {} source identity changed",
        binding.descriptor_index
    );
    Ok(descriptor)
}

pub(super) fn retained_protected_union(
    audits: &[&PracticalExamConsumerAudit],
    rewritten_descriptors: &BTreeSet<(PracticalExamConsumer, usize)>,
) -> Vec<PracticalExamTextureRegion> {
    audits
        .iter()
        .flat_map(|audit| {
            audit
                .descriptors
                .iter()
                .filter(|descriptor| {
                    !rewritten_descriptors.contains(&(audit.consumer, descriptor.index))
                })
                .flat_map(|descriptor| {
                    descriptor.fragments.iter().copied().map(|fragment| {
                        PracticalExamTextureRegion {
                            id: "retained-counted-fragment",
                            cell: fragment.cell(),
                        }
                    })
                })
                .chain(
                    audit
                        .secondary_descriptors
                        .iter()
                        .flat_map(|descriptor| descriptor.fragments.iter().copied())
                        .filter_map(
                            PracticalExamSecondaryDescriptorFragment::shared_producer_region,
                        ),
                )
                .chain(audit.direct_regions.iter().copied())
        })
        .collect()
}

pub(super) fn reclaimed_source_regions(
    audits: &[&PracticalExamConsumerAudit],
    rewritten_descriptors: &BTreeSet<(PracticalExamConsumer, usize)>,
) -> Vec<PracticalExamTextureRegion> {
    audits
        .iter()
        .flat_map(|audit| {
            audit
                .descriptors
                .iter()
                .filter(|descriptor| {
                    rewritten_descriptors.contains(&(audit.consumer, descriptor.index))
                })
                .flat_map(|descriptor| {
                    descriptor.fragments.iter().copied().map(|fragment| {
                        PracticalExamTextureRegion {
                            id: "rewritten-counted-fragment",
                            cell: fragment.cell(),
                        }
                    })
                })
        })
        .collect()
}

pub(super) fn cell_avoids_protected_regions(
    cell: Cell,
    protected: &[PracticalExamTextureRegion],
) -> bool {
    protected
        .iter()
        .all(|region| !cells_overlap(cell, region.cell))
}

fn layout(consumer: PracticalExamConsumer) -> &'static ConsumerLayout {
    let layout = match consumer {
        PracticalExamConsumer::BasicsReview => &BASICS_LAYOUT,
        PracticalExamConsumer::Exam1999 => &EXAM_1999_LAYOUT,
    };
    debug_assert_eq!(layout.consumer, consumer);
    layout
}

pub(super) fn practical_exam_consumer_source_sha256(
    consumer: PracticalExamConsumer,
) -> &'static str {
    layout(consumer).source_sha256
}

fn ensure_guarded_region(
    overlay: &[u8],
    offset: usize,
    size: usize,
    expected_sha256: &str,
    role: &str,
) -> Result<()> {
    let bytes = overlay
        .get(offset..offset + size)
        .with_context(|| format!("practical-exam {role} is truncated"))?;
    let found_sha256 = sha256_bytes(bytes);
    ensure!(
        found_sha256 == expected_sha256,
        "practical-exam {role} changed: found {found_sha256}"
    );
    Ok(())
}

fn parse_secondary_descriptors(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    layout: SecondaryDescriptorLayout,
) -> Result<Vec<PracticalExamSecondaryDescriptor>> {
    ensure_guarded_region(
        overlay,
        layout.pointer_table_offset,
        layout.descriptor_count * 4,
        layout.pointer_table_sha256,
        "secondary descriptor pointer table",
    )?;
    ensure_guarded_region(
        overlay,
        layout.renderer_offset,
        layout.renderer_size,
        layout.renderer_sha256,
        "secondary descriptor renderer",
    )?;
    ensure_guarded_region(
        overlay,
        layout.lookup_offset,
        layout.lookup_size,
        layout.lookup_sha256,
        "secondary descriptor lookup",
    )?;
    parse_secondary_descriptor_arena(overlay, consumer, layout)
}

fn parse_secondary_descriptor_arena(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    layout: SecondaryDescriptorLayout,
) -> Result<Vec<PracticalExamSecondaryDescriptor>> {
    let descriptor_offsets = read_secondary_descriptor_offsets(overlay, consumer, layout)?;
    let mut aliases_by_offset = BTreeMap::<usize, Vec<usize>>::new();
    for (descriptor_index, descriptor_offset) in descriptor_offsets.iter().copied().enumerate() {
        aliases_by_offset
            .entry(descriptor_offset)
            .or_default()
            .push(descriptor_index);
    }

    let physical_offsets = aliases_by_offset.keys().copied().collect::<Vec<_>>();
    let mut descriptors = Vec::with_capacity(physical_offsets.len());
    for (physical_index, offset) in physical_offsets.iter().copied().enumerate() {
        let next_offset = physical_offsets
            .get(physical_index + 1)
            .copied()
            .unwrap_or(layout.descriptor_arena_end);
        let descriptor_index_aliases = aliases_by_offset
            .remove(&offset)
            .expect("physical offset came from the alias map");
        descriptors.push(parse_secondary_descriptor(
            overlay,
            consumer,
            offset,
            next_offset,
            descriptor_index_aliases,
        )?);
    }
    validate_secondary_descriptor_aliases(&descriptor_offsets, &descriptors, consumer)?;
    Ok(descriptors)
}

fn read_secondary_descriptor_offsets(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    layout: SecondaryDescriptorLayout,
) -> Result<Vec<usize>> {
    let mut descriptor_offsets = Vec::with_capacity(layout.descriptor_count);
    for index in 0..layout.descriptor_count {
        let pointer_offset = layout.pointer_table_offset + index * 4;
        let runtime_address = u32::from_le_bytes(
            overlay[pointer_offset..pointer_offset + 4]
                .try_into()
                .context("truncated practical-exam secondary descriptor pointer")?,
        );
        ensure!(
            runtime_address >= OVERLAY_RUNTIME_BASE,
            "{consumer:?} secondary descriptor {index} points below its overlay"
        );
        let offset = usize::try_from(runtime_address - OVERLAY_RUNTIME_BASE)?;
        ensure!(
            (layout.descriptor_arena_start..layout.descriptor_arena_end).contains(&offset),
            "{consumer:?} secondary descriptor {index} points outside its arena"
        );
        descriptor_offsets.push(offset);
    }
    Ok(descriptor_offsets)
}

fn parse_secondary_descriptor(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    offset: usize,
    next_offset: usize,
    descriptor_index_aliases: Vec<usize>,
) -> Result<PracticalExamSecondaryDescriptor> {
    let identity = descriptor_index_aliases
        .first()
        .copied()
        .context("secondary descriptor has no pointer-table alias")?;
    let count = usize::from(*overlay.get(offset).with_context(|| {
        format!("{consumer:?} secondary descriptor {identity} is outside its overlay")
    })?);
    ensure!(
        count > 0,
        "{consumer:?} secondary descriptor {identity} is empty"
    );
    let used = 1usize
        .checked_add(
            count
                .checked_mul(8)
                .context("secondary descriptor size overflow")?,
        )
        .context("secondary descriptor size overflow")?;
    let capacity = used.next_multiple_of(4);
    let end = offset
        .checked_add(capacity)
        .context("secondary descriptor range overflow")?;
    ensure!(
        end <= next_offset,
        "{consumer:?} secondary descriptor {identity} at +0x{offset:04x} overlaps the next physical descriptor"
    );
    let bytes = overlay
        .get(offset..end)
        .with_context(|| format!("{consumer:?} secondary descriptor {identity} is truncated"))?;
    ensure!(
        bytes[used..].iter().all(|byte| *byte == 0),
        "{consumer:?} secondary descriptor {identity} has nonzero alignment padding"
    );

    let mut fragments = Vec::with_capacity(count);
    for fragment_index in 0..count {
        let fragment_offset = 1 + fragment_index * 8;
        let fragment = PracticalExamSecondaryDescriptorFragment {
            texture_page: bytes[fragment_offset],
            texture_bank: PracticalExamSecondaryTextureBank::parse(bytes[fragment_offset + 1])?,
            clut_x_index: bytes[fragment_offset + 2],
            clut_y_offset: bytes[fragment_offset + 3],
            u: bytes[fragment_offset + 4],
            v: bytes[fragment_offset + 5],
            width: bytes[fragment_offset + 6],
            height: bytes[fragment_offset + 7],
        };
        ensure!(
            (FIRST_TEXTURE_PAGE..FIRST_TEXTURE_PAGE + TEXTURE_PAGE_COUNT)
                .contains(&fragment.texture_page),
            "{consumer:?} secondary descriptor {identity} has an unknown texture page"
        );
        ensure!(
            fragment.width > 0
                && fragment.height > 0
                && usize::from(fragment.u) + usize::from(fragment.width) <= TEXTURE_PAGE_WIDTH
                && usize::from(fragment.v) + usize::from(fragment.height) <= TEXTURE_HEIGHT,
            "{consumer:?} secondary descriptor {identity} has invalid fragment geometry"
        );
        fragments.push(fragment);
    }

    Ok(PracticalExamSecondaryDescriptor {
        descriptor_index_aliases,
        offset,
        capacity,
        source_sha256: sha256_bytes(bytes),
        fragments,
    })
}

fn validate_secondary_descriptor_aliases(
    descriptor_offsets: &[usize],
    descriptors: &[PracticalExamSecondaryDescriptor],
    consumer: PracticalExamConsumer,
) -> Result<()> {
    let mut covered_indices = BTreeSet::new();
    for descriptor in descriptors {
        ensure!(
            !descriptor.descriptor_index_aliases.is_empty()
                && descriptor
                    .descriptor_index_aliases
                    .windows(2)
                    .all(|indices| indices[0] < indices[1]),
            "{consumer:?} secondary descriptor at +0x{:04x} has inconsistent aliases",
            descriptor.offset
        );
        for descriptor_index in descriptor.descriptor_index_aliases.iter().copied() {
            ensure!(
                covered_indices.insert(descriptor_index)
                    && descriptor_offsets.get(descriptor_index) == Some(&descriptor.offset),
                "{consumer:?} secondary descriptor alias {descriptor_index} changed physical identity"
            );
        }
    }
    ensure!(
        covered_indices.len() == descriptor_offsets.len(),
        "{consumer:?} secondary descriptor aliases do not cover the pointer table"
    );
    Ok(())
}

fn validate_protected_regions(regions: &[PracticalExamTextureRegion]) -> Result<()> {
    ensure!(
        !regions.is_empty(),
        "practical-exam protected texture union is empty"
    );
    let mut direct_regions = BTreeSet::new();
    for region in regions {
        ensure!(
            region.cell.width > 0
                && region.cell.height > 0
                && region.cell.x + region.cell.width <= TEXTURE_WIDTH
                && region.cell.y + region.cell.height <= TEXTURE_HEIGHT,
            "practical-exam protected region {} has invalid geometry",
            region.id
        );
        if !matches!(
            region.id,
            "counted-fragment-descriptor" | "retained-secondary-descriptor"
        ) {
            ensure!(
                direct_regions.insert((
                    region.id,
                    region.cell.x,
                    region.cell.y,
                    region.cell.width,
                    region.cell.height,
                )),
                "duplicate practical-exam direct region {} at ({},{}) {}x{}",
                region.id,
                region.cell.x,
                region.cell.y,
                region.cell.width,
                region.cell.height
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "descriptor_tests.rs"]
mod tests;
