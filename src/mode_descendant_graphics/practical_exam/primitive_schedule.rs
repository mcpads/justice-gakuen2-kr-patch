use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode_bytes, encode_bytes};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::write_scope::changed_ranges_are_within;

use super::descriptor::PracticalExamConsumer;
use super::primitive_pool::{
    FIRST_SLOT_OFFSET as PRIMITIVE_FIRST_SLOT_OFFSET,
    LOGICAL_SLOT_CAPACITY as LOGICAL_PRIMITIVE_SLOT_CAPACITY, NEXT_FIXED_PRIMITIVE_OFFSET,
    SLOT_STRIDE as PRIMITIVE_SLOT_STRIDE,
};
use super::stream::DescriptorGlyphStream;

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const HINT_CALLER_VERTICAL_ADJUSTMENT: i16 = -2;

const BASICS_RENDERER_CALL_OFFSETS: [usize; 21] = [
    0x25f0, 0x260c, 0x2624, 0x263c, 0x2654, 0x266c, 0x2684, 0x269c, 0x26b8, 0x2710, 0x272c, 0x275c,
    0x2794, 0x27ac, 0x27d8, 0x280c, 0x2828, 0x7560, 0x757c, 0x7594, 0x75b0,
];

const EXAM_1999_RENDERER_CALL_OFFSETS: [usize; 10] = [
    0x1d70, 0x1d8c, 0x1da4, 0x1dbc, 0x1dd4, 0x1df0, 0x762c, 0x7648, 0x7660, 0x767c,
];

const BASICS_HINT_CALL_OFFSETS: [usize; 3] = [0x26b8, 0x2828, 0x75b0];
const EXAM_1999_HINT_CALL_OFFSETS: [usize; 2] = [0x1df0, 0x767c];

const BASICS_ZERO_START_GUARDS: [usize; 3] = [0x25d8, 0x26ec, 0x7548];
const EXAM_1999_ZERO_START_GUARDS: [usize; 2] = [0x1d58, 0x7614];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PrimitiveBatchRole {
    BasicsExamSelection,
    BasicsSubjectSelection,
    BasicsReviewMenu,
    Exam1999TermSelection,
    Exam1999MainMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduledDescriptorSelection {
    Exact {
        descriptor_index: usize,
    },
    RuntimeChoice {
        first_descriptor_index: usize,
        last_descriptor_index: usize,
    },
    UnitStrideSequence {
        first_descriptor_index: usize,
        last_descriptor_index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScheduledPrimitiveAllocation {
    pub(super) selection: ScheduledDescriptorSelection,
    pub(super) start_slot: usize,
    pub(super) reserved_slot_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrimitiveBatchScheduleReport {
    pub(super) role: PrimitiveBatchRole,
    pub(super) allocations: Vec<ScheduledPrimitiveAllocation>,
    pub(super) used_slot_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PrimitiveScheduleWriteRole {
    AllocationStart {
        batch: PrimitiveBatchRole,
        selection: ScheduledDescriptorSelection,
    },
    HintVerticalPosition {
        batch: PrimitiveBatchRole,
        descriptor_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrimitiveScheduleInstructionWrite {
    pub(super) role: PrimitiveScheduleWriteRole,
    pub(super) offset: usize,
    pub(super) runtime_address: u32,
    pub(super) expected_instruction: Instruction,
    pub(super) replacement_instruction: Instruction,
    pub(super) expected_bytes: [u8; 4],
    pub(super) replacement_bytes: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrimitiveScheduleReadbackReport {
    pub(super) planned_instruction_count: usize,
    pub(super) byte_readback_count: usize,
    pub(super) typed_instruction_readback_count: usize,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PracticalExamPrimitiveScheduleReport {
    pub(super) consumer: PracticalExamConsumer,
    pub(super) logical_slot_capacity: usize,
    pub(super) primitive_pool_footprint_end: usize,
    pub(super) next_fixed_primitive_offset: usize,
    pub(super) renderer_call_count: usize,
    pub(super) maximum_batch_slot_count: usize,
    pub(super) batches: Vec<PrimitiveBatchScheduleReport>,
    pub(super) instruction_writes: Vec<PrimitiveScheduleInstructionWrite>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) readback: PrimitiveScheduleReadbackReport,
}

#[derive(Debug, Clone, Copy)]
struct CallerLayout {
    consumer: PracticalExamConsumer,
    source_sha256: &'static str,
    overlay_size: usize,
    descriptor_count: usize,
    renderer_runtime_address: u32,
    renderer_call_offsets: &'static [usize],
    descriptor_call_guards: &'static [DescriptorCallGuard],
    hint_call_offsets: &'static [usize],
    zero_start_guard_offsets: &'static [usize],
}

#[derive(Debug, Clone, Copy)]
struct DescriptorCallGuard {
    writer_offset: usize,
    selection: ScheduledDescriptorSelection,
}

#[derive(Debug, Clone, Copy)]
struct PrimitivePoolBoundaryWitness {
    active_buffer_lui_offset: usize,
    active_buffer_load_offset: usize,
    pool_lui_offset: usize,
    pool_load_offset: usize,
    buffer_scale_first_offset: usize,
    buffer_scale_add_offset: usize,
    buffer_scale_second_offset: usize,
    fixed_base_offset: usize,
    fixed_address_offset: usize,
    followup_call_offset: usize,
    followup_target: u32,
    pool_pointer_load_offset: i16,
}

const BASICS_POOL_BOUNDARY_WITNESSES: [PrimitivePoolBoundaryWitness; 3] = [
    PrimitivePoolBoundaryWitness {
        active_buffer_lui_offset: 0x22b0,
        active_buffer_load_offset: 0x22b4,
        pool_lui_offset: 0x22b8,
        pool_load_offset: 0x22bc,
        buffer_scale_first_offset: 0x22d0,
        buffer_scale_add_offset: 0x22d4,
        buffer_scale_second_offset: 0x22d8,
        fixed_base_offset: 0x22e4,
        fixed_address_offset: 0x22f4,
        followup_call_offset: 0x26c8,
        followup_target: 0x800a_42a0,
        pool_pointer_load_offset: -0x6870,
    },
    PrimitivePoolBoundaryWitness {
        active_buffer_lui_offset: 0x2444,
        active_buffer_load_offset: 0x2448,
        pool_lui_offset: 0x244c,
        pool_load_offset: 0x2450,
        buffer_scale_first_offset: 0x2464,
        buffer_scale_add_offset: 0x2468,
        buffer_scale_second_offset: 0x246c,
        fixed_base_offset: 0x2478,
        fixed_address_offset: 0x2488,
        followup_call_offset: 0x2838,
        followup_target: 0x800a_4434,
        pool_pointer_load_offset: -0x6870,
    },
    PrimitivePoolBoundaryWitness {
        active_buffer_lui_offset: 0x73c8,
        active_buffer_load_offset: 0x73cc,
        pool_lui_offset: 0x73d0,
        pool_load_offset: 0x73d4,
        buffer_scale_first_offset: 0x73e8,
        buffer_scale_add_offset: 0x73ec,
        buffer_scale_second_offset: 0x73f0,
        fixed_base_offset: 0x73fc,
        fixed_address_offset: 0x740c,
        followup_call_offset: 0x75c0,
        followup_target: 0x800a_93b8,
        pool_pointer_load_offset: -0x6870,
    },
];

const EXAM_1999_POOL_BOUNDARY_WITNESSES: [PrimitivePoolBoundaryWitness; 2] = [
    PrimitivePoolBoundaryWitness {
        active_buffer_lui_offset: 0x1bcc,
        active_buffer_load_offset: 0x1bd0,
        pool_lui_offset: 0x1bd4,
        pool_load_offset: 0x1bd8,
        buffer_scale_first_offset: 0x1bec,
        buffer_scale_add_offset: 0x1bf0,
        buffer_scale_second_offset: 0x1bf4,
        fixed_base_offset: 0x1c00,
        fixed_address_offset: 0x1c10,
        followup_call_offset: 0x1e00,
        followup_target: 0x800a_3bbc,
        pool_pointer_load_offset: -0x6854,
    },
    PrimitivePoolBoundaryWitness {
        active_buffer_lui_offset: 0x7494,
        active_buffer_load_offset: 0x7498,
        pool_lui_offset: 0x749c,
        pool_load_offset: 0x74a0,
        buffer_scale_first_offset: 0x74b4,
        buffer_scale_add_offset: 0x74b8,
        buffer_scale_second_offset: 0x74bc,
        fixed_base_offset: 0x74c8,
        fixed_address_offset: 0x74d8,
        followup_call_offset: 0x768c,
        followup_target: 0x800a_9484,
        pool_pointer_load_offset: -0x6854,
    },
];

const fn exact_descriptor(writer_offset: usize, descriptor_index: usize) -> DescriptorCallGuard {
    DescriptorCallGuard {
        writer_offset,
        selection: ScheduledDescriptorSelection::Exact { descriptor_index },
    }
}

const fn runtime_descriptor_choice(
    writer_offset: usize,
    first_descriptor_index: usize,
    last_descriptor_index: usize,
) -> DescriptorCallGuard {
    DescriptorCallGuard {
        writer_offset,
        selection: ScheduledDescriptorSelection::RuntimeChoice {
            first_descriptor_index,
            last_descriptor_index,
        },
    }
}

const fn unit_stride_descriptors(
    writer_offset: usize,
    first_descriptor_index: usize,
    last_descriptor_index: usize,
) -> DescriptorCallGuard {
    DescriptorCallGuard {
        writer_offset,
        selection: ScheduledDescriptorSelection::UnitStrideSequence {
            first_descriptor_index,
            last_descriptor_index,
        },
    }
}

const BASICS_DESCRIPTOR_CALL_GUARDS: [DescriptorCallGuard; 21] = [
    exact_descriptor(0x25d4, 0),
    exact_descriptor(0x25f8, 1),
    exact_descriptor(0x2614, 2),
    exact_descriptor(0x262c, 3),
    exact_descriptor(0x2644, 4),
    exact_descriptor(0x265c, 5),
    exact_descriptor(0x2674, 6),
    exact_descriptor(0x268c, 7),
    exact_descriptor(0x26a4, 48),
    exact_descriptor(0x26e8, 0),
    exact_descriptor(0x2718, 8),
    runtime_descriptor_choice(0x2760, 0, 48),
    exact_descriptor(0x2764, 15),
    unit_stride_descriptors(0x27a0, 10, 14),
    exact_descriptor(0x27c4, 9),
    runtime_descriptor_choice(0x2810, 0, 48),
    exact_descriptor(0x2814, 48),
    exact_descriptor(0x7544, 0),
    exact_descriptor(0x7568, 47),
    exact_descriptor(0x7584, 46),
    exact_descriptor(0x759c, 48),
];

const EXAM_1999_DESCRIPTOR_CALL_GUARDS: [DescriptorCallGuard; 10] = [
    exact_descriptor(0x1d54, 0),
    exact_descriptor(0x1d78, 4),
    exact_descriptor(0x1d94, 5),
    exact_descriptor(0x1dac, 6),
    exact_descriptor(0x1dc4, 7),
    exact_descriptor(0x1ddc, 3),
    exact_descriptor(0x7610, 0),
    exact_descriptor(0x7634, 1),
    exact_descriptor(0x7650, 2),
    exact_descriptor(0x7668, 3),
];

#[derive(Debug, Clone)]
struct PendingInstructionWrite {
    role: PrimitiveScheduleWriteRole,
    offset: usize,
    expected_instruction: Instruction,
    replacement_instruction: Instruction,
}

struct PrimitiveBatchBuilder<'a> {
    consumer: PracticalExamConsumer,
    role: PrimitiveBatchRole,
    stream_counts: &'a [usize],
    next_slot: usize,
    allocations: Vec<ScheduledPrimitiveAllocation>,
}

impl<'a> PrimitiveBatchBuilder<'a> {
    fn new(
        consumer: PracticalExamConsumer,
        role: PrimitiveBatchRole,
        stream_counts: &'a [usize],
    ) -> Self {
        Self {
            consumer,
            role,
            stream_counts,
            next_slot: 0,
            allocations: Vec::new(),
        }
    }

    fn exact(&mut self, descriptor_index: usize) -> Result<ScheduledPrimitiveAllocation> {
        let count = self.descriptor_count(descriptor_index)?;
        self.reserve(
            ScheduledDescriptorSelection::Exact { descriptor_index },
            count,
        )
    }

    fn runtime_choice(
        &mut self,
        first_descriptor_index: usize,
        last_descriptor_index: usize,
    ) -> Result<ScheduledPrimitiveAllocation> {
        ensure!(
            first_descriptor_index <= last_descriptor_index,
            "{:?} {:?} practical-exam runtime descriptor choice is empty",
            self.consumer,
            self.role
        );
        let reserved_slot_count = (first_descriptor_index..=last_descriptor_index)
            .map(|descriptor_index| self.descriptor_count(descriptor_index))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max()
            .context("practical-exam runtime descriptor choice has no stream counts")?;
        self.reserve(
            ScheduledDescriptorSelection::RuntimeChoice {
                first_descriptor_index,
                last_descriptor_index,
            },
            reserved_slot_count,
        )
    }

    fn unit_stride_sequence(
        &mut self,
        first_descriptor_index: usize,
        last_descriptor_index: usize,
    ) -> Result<ScheduledPrimitiveAllocation> {
        ensure!(
            first_descriptor_index <= last_descriptor_index,
            "{:?} {:?} practical-exam unit-stride descriptor sequence is empty",
            self.consumer,
            self.role
        );
        for descriptor_index in first_descriptor_index..=last_descriptor_index {
            ensure!(
                self.descriptor_count(descriptor_index)? == 1,
                "{:?} {:?} practical-exam descriptor {descriptor_index} must contain exactly one glyph because its caller advances A1 by one slot",
                self.consumer,
                self.role
            );
        }
        self.reserve(
            ScheduledDescriptorSelection::UnitStrideSequence {
                first_descriptor_index,
                last_descriptor_index,
            },
            last_descriptor_index - first_descriptor_index + 1,
        )
    }

    fn descriptor_count(&self, descriptor_index: usize) -> Result<usize> {
        self.stream_counts
            .get(descriptor_index)
            .copied()
            .with_context(|| {
                format!(
                    "{:?} {:?} practical-exam descriptor {descriptor_index} has no generated stream count",
                    self.consumer, self.role
                )
            })
    }

    fn reserve(
        &mut self,
        selection: ScheduledDescriptorSelection,
        reserved_slot_count: usize,
    ) -> Result<ScheduledPrimitiveAllocation> {
        ensure!(
            reserved_slot_count > 0,
            "{:?} {:?} practical-exam {selection:?} reserves no primitive slots",
            self.consumer,
            self.role
        );
        let allocation = ScheduledPrimitiveAllocation {
            selection,
            start_slot: self.next_slot,
            reserved_slot_count,
        };
        self.next_slot = self
            .next_slot
            .checked_add(reserved_slot_count)
            .context("practical-exam primitive schedule slot count overflow")?;
        ensure!(
            self.next_slot <= LOGICAL_PRIMITIVE_SLOT_CAPACITY,
            "{:?} {:?} practical-exam batch needs {} logical primitive slots, exceeding the {}-slot pool",
            self.consumer,
            self.role,
            self.next_slot,
            LOGICAL_PRIMITIVE_SLOT_CAPACITY
        );
        self.allocations.push(allocation);
        Ok(allocation)
    }

    fn finish(self) -> PrimitiveBatchScheduleReport {
        PrimitiveBatchScheduleReport {
            role: self.role,
            allocations: self.allocations,
            used_slot_count: self.next_slot,
        }
    }
}

pub(super) fn install_practical_exam_primitive_schedule(
    source_overlay: &[u8],
    composed_overlay: &mut [u8],
    consumer: PracticalExamConsumer,
    descriptor_streams: &[DescriptorGlyphStream],
) -> Result<PracticalExamPrimitiveScheduleReport> {
    let layout = caller_layout(consumer);
    ensure!(
        layout.consumer == consumer,
        "practical-exam primitive caller layout and consumer do not match"
    );
    ensure!(
        source_overlay.len() == layout.overlay_size
            && composed_overlay.len() == source_overlay.len(),
        "{consumer:?} practical-exam primitive schedule requires an exact {}-byte source and same-sized composed overlay",
        layout.overlay_size
    );
    ensure!(
        sha256_bytes(source_overlay) == layout.source_sha256,
        "{consumer:?} practical-exam primitive caller source changed"
    );

    let renderer_call_offsets = renderer_call_offsets(source_overlay, layout)?;
    let primitive_pool_footprint_end = ensure_primitive_pool_capacity(source_overlay, consumer)?;
    ensure_zero_start_guards(source_overlay, composed_overlay, layout)?;

    let stream_counts = validated_stream_counts(descriptor_streams, layout)?;
    let (batches, pending_writes) = match consumer {
        PracticalExamConsumer::BasicsReview => basics_schedule(&stream_counts)?,
        PracticalExamConsumer::Exam1999 => exam_1999_schedule(&stream_counts)?,
    };
    let maximum_batch_slot_count = batches
        .iter()
        .map(|batch| batch.used_slot_count)
        .max()
        .context("practical-exam primitive schedule has no caller batches")?;
    ensure!(
        maximum_batch_slot_count <= LOGICAL_PRIMITIVE_SLOT_CAPACITY,
        "{consumer:?} practical-exam primitive schedule exceeds its logical slot capacity"
    );

    let instruction_writes =
        compile_instruction_writes(source_overlay, composed_overlay, consumer, pending_writes)?;
    ensure_renderer_call_schedule_coverage(
        source_overlay,
        layout,
        &renderer_call_offsets,
        &batches,
        &instruction_writes,
    )?;
    let expected_write_ranges = instruction_writes
        .iter()
        .map(|write| [write.offset, write.offset + 4])
        .collect::<Vec<_>>();
    ensure_non_overlapping_write_ranges(&expected_write_ranges, consumer)?;

    let before = composed_overlay.to_vec();
    for write in &instruction_writes {
        composed_overlay[write.offset..write.offset + 4].copy_from_slice(&write.replacement_bytes);
    }
    let readback = verify_instruction_readback(
        &before,
        composed_overlay,
        consumer,
        &instruction_writes,
        &expected_write_ranges,
    )?;

    Ok(PracticalExamPrimitiveScheduleReport {
        consumer,
        logical_slot_capacity: LOGICAL_PRIMITIVE_SLOT_CAPACITY,
        primitive_pool_footprint_end,
        next_fixed_primitive_offset: NEXT_FIXED_PRIMITIVE_OFFSET,
        renderer_call_count: renderer_call_offsets.len(),
        maximum_batch_slot_count,
        batches,
        instruction_writes,
        expected_write_ranges,
        readback,
    })
}

fn ensure_primitive_pool_capacity(
    source_overlay: &[u8],
    consumer: PracticalExamConsumer,
) -> Result<usize> {
    let footprint_end = PRIMITIVE_FIRST_SLOT_OFFSET
        .checked_add(
            LOGICAL_PRIMITIVE_SLOT_CAPACITY
                .checked_mul(PRIMITIVE_SLOT_STRIDE)
                .context("practical-exam primitive pool footprint overflow")?,
        )
        .context("practical-exam primitive pool footprint overflow")?;
    let next_capacity_end = footprint_end
        .checked_add(PRIMITIVE_SLOT_STRIDE)
        .context("practical-exam next primitive pool footprint overflow")?;
    ensure!(
        footprint_end <= NEXT_FIXED_PRIMITIVE_OFFSET
            && next_capacity_end > NEXT_FIXED_PRIMITIVE_OFFSET,
        "{consumer:?} practical-exam logical primitive capacity is not the maximal source-owned prefix"
    );

    let witnesses = match consumer {
        PracticalExamConsumer::BasicsReview => &BASICS_POOL_BOUNDARY_WITNESSES[..],
        PracticalExamConsumer::Exam1999 => &EXAM_1999_POOL_BOUNDARY_WITNESSES[..],
    };
    for witness in witnesses {
        ensure_primitive_pool_boundary_witness(source_overlay, consumer, *witness)?;
    }
    Ok(footprint_end)
}

fn ensure_primitive_pool_boundary_witness(
    source_overlay: &[u8],
    consumer: PracticalExamConsumer,
    witness: PrimitivePoolBoundaryWitness,
) -> Result<()> {
    let expected = [
        (
            witness.active_buffer_lui_offset,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            witness.active_buffer_load_offset,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x608c,
            },
        ),
        (
            witness.pool_lui_offset,
            Instruction::Lui {
                rt: Register::T0,
                immediate: 0x800b,
            },
        ),
        (
            witness.pool_load_offset,
            Instruction::Lw {
                rt: Register::T0,
                base: Register::T0,
                offset: witness.pool_pointer_load_offset,
            },
        ),
        (
            witness.buffer_scale_first_offset,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V1,
                shift: 1,
            },
        ),
        (
            witness.buffer_scale_add_offset,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            witness.buffer_scale_second_offset,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            witness.fixed_base_offset,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: i16::try_from(NEXT_FIXED_PRIMITIVE_OFFSET)?,
            },
        ),
        (
            witness.fixed_address_offset,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::T0,
                rt: Register::V0,
            },
        ),
    ];
    for (offset, expected) in expected {
        let actual = decode_instruction(source_overlay, offset, consumer)?;
        ensure!(
            actual == expected,
            "{consumer:?} practical-exam fixed-primitive boundary witness +0x{offset:04x} changed: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        decode_instruction(source_overlay, witness.followup_call_offset, consumer)?
            == (Instruction::Jal {
                target: witness.followup_target,
            }),
        "{consumer:?} practical-exam fixed-primitive followup call +0x{:04x} changed",
        witness.followup_call_offset
    );
    Ok(())
}

fn basics_schedule(
    stream_counts: &[usize],
) -> Result<(
    Vec<PrimitiveBatchScheduleReport>,
    Vec<PendingInstructionWrite>,
)> {
    let consumer = PracticalExamConsumer::BasicsReview;
    let mut batches = Vec::with_capacity(3);
    let mut writes = Vec::with_capacity(21);

    let mut exam_selection = PrimitiveBatchBuilder::new(
        consumer,
        PrimitiveBatchRole::BasicsExamSelection,
        stream_counts,
    );
    exam_selection.exact(0)?;
    let exam_prompt = exam_selection.exact(1)?;
    let test_one = exam_selection.exact(2)?;
    let test_two = exam_selection.exact(3)?;
    let test_three = exam_selection.exact(4)?;
    let test_four = exam_selection.exact(5)?;
    let test_five = exam_selection.exact(6)?;
    let test_six = exam_selection.exact(7)?;
    let selection_hint = exam_selection.exact(48)?;
    for (offset, expected_start, allocation) in [
        (0x25fc, 1, exam_prompt),
        (0x2618, 6, test_one),
        (0x2630, 9, test_two),
        (0x2648, 12, test_three),
        (0x2660, 15, test_four),
        (0x2678, 18, test_five),
        (0x2690, 21, test_six),
        (0x26a8, 24, selection_hint),
    ] {
        writes.push(start_slot_write(
            PrimitiveBatchRole::BasicsExamSelection,
            offset,
            Register::ZERO,
            expected_start,
            allocation,
        )?);
    }
    writes.push(hint_vertical_write(
        PrimitiveBatchRole::BasicsExamSelection,
        48,
        0x26b0,
    )?);
    batches.push(exam_selection.finish());

    let mut subject_selection = PrimitiveBatchBuilder::new(
        consumer,
        PrimitiveBatchRole::BasicsSubjectSelection,
        stream_counts,
    );
    subject_selection.exact(0)?;
    let subject_prompt = subject_selection.exact(8)?;
    // Both runtime selectors are seeded from an external global whose domain
    // is not closed inside this overlay. Reserve the longest stream in the
    // entire pointer table instead of assuming the observed 2..=7 range.
    let selected_test = subject_selection.runtime_choice(0, 48)?;
    let retained_answer_marker = subject_selection.exact(15)?;
    let retained_grade_markers = subject_selection.unit_stride_sequence(10, 14)?;
    let subject_label = subject_selection.exact(9)?;
    let selected_subject = subject_selection.runtime_choice(0, 48)?;
    let subject_hint = subject_selection.exact(48)?;
    for (offset, source_register, expected_start, allocation) in [
        (0x271c, Register::ZERO, 1, subject_prompt),
        (0x2734, Register::ZERO, 5, selected_test),
        (0x2770, Register::ZERO, 11, retained_answer_marker),
        (0x27a4, Register::S1, 12, retained_grade_markers),
        (0x27c8, Register::ZERO, 17, subject_label),
        (0x27e0, Register::ZERO, 18, selected_subject),
        (0x2818, Register::ZERO, 30, subject_hint),
    ] {
        writes.push(start_slot_write(
            PrimitiveBatchRole::BasicsSubjectSelection,
            offset,
            source_register,
            expected_start,
            allocation,
        )?);
    }
    writes.push(hint_vertical_write(
        PrimitiveBatchRole::BasicsSubjectSelection,
        48,
        0x2820,
    )?);
    batches.push(subject_selection.finish());

    let mut review_menu = PrimitiveBatchBuilder::new(
        consumer,
        PrimitiveBatchRole::BasicsReviewMenu,
        stream_counts,
    );
    review_menu.exact(0)?;
    let basics_review = review_menu.exact(47)?;
    let results = review_menu.exact(46)?;
    let review_hint = review_menu.exact(48)?;
    for (offset, expected_start, allocation) in [
        (0x756c, 9, basics_review),
        (0x7588, 1, results),
        (0x75a0, 20, review_hint),
    ] {
        writes.push(start_slot_write(
            PrimitiveBatchRole::BasicsReviewMenu,
            offset,
            Register::ZERO,
            expected_start,
            allocation,
        )?);
    }
    writes.push(hint_vertical_write(
        PrimitiveBatchRole::BasicsReviewMenu,
        48,
        0x75a8,
    )?);
    batches.push(review_menu.finish());

    Ok((batches, writes))
}

fn exam_1999_schedule(
    stream_counts: &[usize],
) -> Result<(
    Vec<PrimitiveBatchScheduleReport>,
    Vec<PendingInstructionWrite>,
)> {
    let consumer = PracticalExamConsumer::Exam1999;
    let mut batches = Vec::with_capacity(2);
    let mut writes = Vec::with_capacity(10);

    let mut term_selection = PrimitiveBatchBuilder::new(
        consumer,
        PrimitiveBatchRole::Exam1999TermSelection,
        stream_counts,
    );
    term_selection.exact(0)?;
    let exam_prompt = term_selection.exact(4)?;
    let first_term = term_selection.exact(5)?;
    let second_term = term_selection.exact(6)?;
    let school_year = term_selection.exact(7)?;
    let selection_hint = term_selection.exact(3)?;
    for (offset, expected_start, allocation) in [
        (0x1d7c, 2, exam_prompt),
        (0x1d98, 12, first_term),
        (0x1db0, 22, second_term),
        (0x1dc8, 28, school_year),
        (0x1de0, 32, selection_hint),
    ] {
        writes.push(start_slot_write(
            PrimitiveBatchRole::Exam1999TermSelection,
            offset,
            Register::ZERO,
            expected_start,
            allocation,
        )?);
    }
    writes.push(hint_vertical_write(
        PrimitiveBatchRole::Exam1999TermSelection,
        3,
        0x1de8,
    )?);
    batches.push(term_selection.finish());

    let mut main_menu = PrimitiveBatchBuilder::new(
        consumer,
        PrimitiveBatchRole::Exam1999MainMenu,
        stream_counts,
    );
    main_menu.exact(0)?;
    let exam_start = main_menu.exact(1)?;
    let results = main_menu.exact(2)?;
    let menu_hint = main_menu.exact(3)?;
    for (offset, expected_start, allocation) in [
        (0x7638, 2, exam_start),
        (0x7654, 10, results),
        (0x766c, 21, menu_hint),
    ] {
        writes.push(start_slot_write(
            PrimitiveBatchRole::Exam1999MainMenu,
            offset,
            Register::ZERO,
            expected_start,
            allocation,
        )?);
    }
    writes.push(hint_vertical_write(
        PrimitiveBatchRole::Exam1999MainMenu,
        3,
        0x7674,
    )?);
    batches.push(main_menu.finish());

    Ok((batches, writes))
}

fn start_slot_write(
    batch: PrimitiveBatchRole,
    offset: usize,
    source_register: Register,
    expected_start_slot: i16,
    allocation: ScheduledPrimitiveAllocation,
) -> Result<PendingInstructionWrite> {
    let replacement_start_slot = i16::try_from(allocation.start_slot)
        .context("practical-exam primitive start slot does not fit an ADDIU immediate")?;
    Ok(PendingInstructionWrite {
        role: PrimitiveScheduleWriteRole::AllocationStart {
            batch,
            selection: allocation.selection,
        },
        offset,
        expected_instruction: Instruction::Addiu {
            rt: Register::A1,
            rs: source_register,
            immediate: expected_start_slot,
        },
        replacement_instruction: Instruction::Addiu {
            rt: Register::A1,
            rs: source_register,
            immediate: replacement_start_slot,
        },
    })
}

fn hint_vertical_write(
    batch: PrimitiveBatchRole,
    descriptor_index: usize,
    offset: usize,
) -> Result<PendingInstructionWrite> {
    const SOURCE_VERTICAL_POSITION: i16 = 0x0184;
    let replacement_vertical_position = SOURCE_VERTICAL_POSITION
        .checked_add(HINT_CALLER_VERTICAL_ADJUSTMENT)
        .context("practical-exam hint caller vertical position overflow")?;
    Ok(PendingInstructionWrite {
        role: PrimitiveScheduleWriteRole::HintVerticalPosition {
            batch,
            descriptor_index,
        },
        offset,
        expected_instruction: Instruction::Addiu {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: SOURCE_VERTICAL_POSITION,
        },
        replacement_instruction: Instruction::Addiu {
            rt: Register::A3,
            rs: Register::ZERO,
            immediate: replacement_vertical_position,
        },
    })
}

fn compile_instruction_writes(
    source_overlay: &[u8],
    composed_overlay: &[u8],
    consumer: PracticalExamConsumer,
    pending_writes: Vec<PendingInstructionWrite>,
) -> Result<Vec<PrimitiveScheduleInstructionWrite>> {
    let mut seen_offsets = BTreeSet::new();
    let mut writes = Vec::with_capacity(pending_writes.len());
    for pending in pending_writes {
        ensure!(
            seen_offsets.insert(pending.offset),
            "{consumer:?} practical-exam primitive schedule writes caller +0x{:04x} twice",
            pending.offset
        );
        let runtime_address = runtime_address(pending.offset)?;
        let source_bytes = instruction_bytes(source_overlay, pending.offset, consumer)?;
        let composed_bytes = instruction_bytes(composed_overlay, pending.offset, consumer)?;
        let source_instruction = decode_bytes(&source_bytes, runtime_address).with_context(|| {
            format!(
                "{consumer:?} practical-exam caller +0x{:04x} is not a typed R3000A instruction",
                pending.offset
            )
        })?;
        ensure!(
            source_instruction == pending.expected_instruction,
            "{consumer:?} practical-exam caller +0x{:04x} changed: expected {:?}, found {source_instruction:?}",
            pending.offset,
            pending.expected_instruction
        );
        let expected_bytes = encode_bytes(&pending.expected_instruction, runtime_address)?;
        ensure!(
            source_bytes == expected_bytes,
            "{consumer:?} practical-exam caller +0x{:04x} typed source encoding changed",
            pending.offset
        );
        ensure!(
            composed_bytes == expected_bytes,
            "{consumer:?} practical-exam caller +0x{:04x} was changed before primitive scheduling",
            pending.offset
        );
        let replacement_bytes = encode_bytes(&pending.replacement_instruction, runtime_address)?;
        ensure!(
            decode_bytes(&replacement_bytes, runtime_address)? == pending.replacement_instruction,
            "{consumer:?} practical-exam caller +0x{:04x} replacement failed typed preflight readback",
            pending.offset
        );
        writes.push(PrimitiveScheduleInstructionWrite {
            role: pending.role,
            offset: pending.offset,
            runtime_address,
            expected_instruction: pending.expected_instruction,
            replacement_instruction: pending.replacement_instruction,
            expected_bytes,
            replacement_bytes,
        });
    }
    Ok(writes)
}

fn verify_instruction_readback(
    before: &[u8],
    composed_overlay: &[u8],
    consumer: PracticalExamConsumer,
    instruction_writes: &[PrimitiveScheduleInstructionWrite],
    expected_write_ranges: &[[usize; 2]],
) -> Result<PrimitiveScheduleReadbackReport> {
    let mut byte_readback_count = 0usize;
    let mut typed_instruction_readback_count = 0usize;
    for write in instruction_writes {
        let bytes = instruction_bytes(composed_overlay, write.offset, consumer)?;
        ensure!(
            bytes == write.replacement_bytes,
            "{consumer:?} practical-exam caller +0x{:04x} byte readback changed",
            write.offset
        );
        byte_readback_count += 1;
        let instruction = decode_bytes(&bytes, write.runtime_address)?;
        ensure!(
            instruction == write.replacement_instruction,
            "{consumer:?} practical-exam caller +0x{:04x} typed readback changed",
            write.offset
        );
        typed_instruction_readback_count += 1;
    }

    let changed_byte_ranges = difference_ranges(before, composed_overlay);
    ensure!(
        changed_ranges_are_within(&changed_byte_ranges, expected_write_ranges),
        "{consumer:?} practical-exam primitive schedule escaped its Expected Writes"
    );
    ensure!(
        instruction_writes.iter().all(|write| {
            write.expected_bytes == write.replacement_bytes
                || changed_byte_ranges
                    .iter()
                    .any(|[start, end]| *start < write.offset + 4 && write.offset < *end)
        }),
        "{consumer:?} practical-exam primitive schedule omitted an effective replacement from readback"
    );

    Ok(PrimitiveScheduleReadbackReport {
        planned_instruction_count: instruction_writes.len(),
        byte_readback_count,
        typed_instruction_readback_count,
        changed_byte_ranges,
        verified: byte_readback_count == instruction_writes.len()
            && typed_instruction_readback_count == instruction_writes.len(),
    })
}

fn validated_stream_counts(
    descriptor_streams: &[DescriptorGlyphStream],
    layout: CallerLayout,
) -> Result<Vec<usize>> {
    ensure!(
        descriptor_streams.len() == layout.descriptor_count,
        "{:?} practical-exam primitive schedule requires exactly {} descriptor streams, found {}",
        layout.consumer,
        layout.descriptor_count,
        descriptor_streams.len()
    );
    let mut counts = vec![None; layout.descriptor_count];
    for stream in descriptor_streams {
        ensure!(
            stream.descriptor_index < layout.descriptor_count,
            "{:?} practical-exam primitive schedule descriptor {} is outside its caller table",
            layout.consumer,
            stream.descriptor_index
        );
        ensure!(
            !stream.codes.is_empty(),
            "{:?} practical-exam primitive schedule descriptor {} has an empty generated stream",
            layout.consumer,
            stream.descriptor_index
        );
        ensure!(
            counts[stream.descriptor_index]
                .replace(stream.codes.len())
                .is_none(),
            "{:?} practical-exam primitive schedule descriptor {} was supplied more than once",
            layout.consumer,
            stream.descriptor_index
        );
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(descriptor_index, count)| {
            count.with_context(|| {
                format!(
                    "{:?} practical-exam primitive schedule descriptor {descriptor_index} stream is missing",
                    layout.consumer
                )
            })
        })
        .collect()
}

fn renderer_call_offsets(source_overlay: &[u8], layout: CallerLayout) -> Result<Vec<usize>> {
    let mut offsets = Vec::new();
    for (instruction_index, bytes) in source_overlay.as_chunks::<4>().0.iter().enumerate() {
        let offset = instruction_index * 4;
        let runtime_address = runtime_address(offset)?;
        if let Ok(Instruction::Jal { target }) = decode_bytes(bytes, runtime_address)
            && target == layout.renderer_runtime_address
        {
            offsets.push(offset);
        }
    }
    ensure!(
        offsets.len() == layout.renderer_call_offsets.len(),
        "{:?} practical-exam renderer-call denominator changed: expected {}, found {}",
        layout.consumer,
        layout.renderer_call_offsets.len(),
        offsets.len()
    );
    ensure!(
        offsets == layout.renderer_call_offsets,
        "{:?} practical-exam renderer call sites changed: expected {:x?}, found {:x?}",
        layout.consumer,
        layout.renderer_call_offsets,
        offsets
    );
    Ok(offsets)
}

fn ensure_zero_start_guards(
    source_overlay: &[u8],
    composed_overlay: &[u8],
    layout: CallerLayout,
) -> Result<()> {
    let expected = Instruction::Addu {
        rd: Register::A1,
        rs: Register::ZERO,
        rt: Register::ZERO,
    };
    for offset in layout.zero_start_guard_offsets {
        let runtime_address = runtime_address(*offset)?;
        let expected_bytes = encode_bytes(&expected, runtime_address)?;
        let source_bytes = instruction_bytes(source_overlay, *offset, layout.consumer)?;
        let composed_bytes = instruction_bytes(composed_overlay, *offset, layout.consumer)?;
        ensure!(
            decode_bytes(&source_bytes, runtime_address)? == expected
                && source_bytes == expected_bytes,
            "{:?} practical-exam zero-based caller +0x{offset:04x} changed",
            layout.consumer
        );
        ensure!(
            composed_bytes == expected_bytes,
            "{:?} practical-exam zero-based caller +0x{offset:04x} was changed before primitive scheduling",
            layout.consumer
        );
    }
    Ok(())
}

fn ensure_renderer_call_schedule_coverage(
    source_overlay: &[u8],
    layout: CallerLayout,
    renderer_call_offsets: &[usize],
    batches: &[PrimitiveBatchScheduleReport],
    instruction_writes: &[PrimitiveScheduleInstructionWrite],
) -> Result<()> {
    ensure_descriptor_call_coverage(source_overlay, layout, renderer_call_offsets, batches)?;
    let expected_start_writers = layout
        .zero_start_guard_offsets
        .iter()
        .copied()
        .chain(instruction_writes.iter().filter_map(|write| {
            matches!(
                write.role,
                PrimitiveScheduleWriteRole::AllocationStart { .. }
            )
            .then_some(write.offset)
        }))
        .collect::<BTreeSet<_>>();
    ensure!(
        expected_start_writers.len() == renderer_call_offsets.len(),
        "{:?} practical-exam primitive schedule has {} A1 setup authorities for {} renderer calls",
        layout.consumer,
        expected_start_writers.len(),
        renderer_call_offsets.len()
    );

    let found_start_writers = renderer_call_offsets
        .iter()
        .enumerate()
        .map(|(index, call_offset)| {
            let lower_bound = index
                .checked_sub(1)
                .map_or(0, |previous| renderer_call_offsets[previous] + 8);
            nearest_argument_writer(
                source_overlay,
                *call_offset,
                lower_bound,
                Register::A1,
                layout.consumer,
            )
        })
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        found_start_writers == expected_start_writers,
        "{:?} practical-exam renderer calls are not covered one-for-one by the audited A1 setup authorities: expected {:x?}, found {:x?}",
        layout.consumer,
        expected_start_writers,
        found_start_writers
    );

    let hint_writers = instruction_writes
        .iter()
        .filter_map(|write| {
            matches!(
                write.role,
                PrimitiveScheduleWriteRole::HintVerticalPosition { .. }
            )
            .then_some(write.offset)
        })
        .collect::<BTreeSet<_>>();
    let found_hint_writers = layout
        .hint_call_offsets
        .iter()
        .map(|call_offset| {
            let call_index = renderer_call_offsets
                .binary_search(call_offset)
                .map_err(|_| anyhow::anyhow!(
                    "{:?} practical-exam hint call +0x{call_offset:04x} is outside the renderer-call denominator",
                    layout.consumer
                ))?;
            let lower_bound = call_index
                .checked_sub(1)
                .map_or(0, |previous| renderer_call_offsets[previous] + 8);
            nearest_argument_writer(
                source_overlay,
                *call_offset,
                lower_bound,
                Register::A3,
                layout.consumer,
            )
        })
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        found_hint_writers == hint_writers && hint_writers.len() == layout.hint_call_offsets.len(),
        "{:?} practical-exam hint calls are not covered one-for-one by their audited A3 setup authorities",
        layout.consumer
    );
    Ok(())
}

fn ensure_descriptor_call_coverage(
    source_overlay: &[u8],
    layout: CallerLayout,
    renderer_call_offsets: &[usize],
    batches: &[PrimitiveBatchScheduleReport],
) -> Result<()> {
    let scheduled_selections = batches
        .iter()
        .flat_map(|batch| {
            batch
                .allocations
                .iter()
                .map(|allocation| allocation.selection)
        })
        .collect::<Vec<_>>();
    ensure!(
        layout.descriptor_call_guards.len() == renderer_call_offsets.len()
            && scheduled_selections.len() == renderer_call_offsets.len(),
        "{:?} practical-exam descriptor-selection denominator changed",
        layout.consumer
    );
    for (index, ((call_offset, guard), scheduled_selection)) in renderer_call_offsets
        .iter()
        .zip(layout.descriptor_call_guards)
        .zip(scheduled_selections)
        .enumerate()
    {
        ensure!(
            guard.selection == scheduled_selection,
            "{:?} practical-exam renderer call {index} +0x{call_offset:04x} schedule selection changed: source guard {:?}, schedule {:?}",
            layout.consumer,
            guard.selection,
            scheduled_selection
        );
        let lower_bound = index
            .checked_sub(1)
            .map_or(0, |previous| renderer_call_offsets[previous] + 8);
        let writer = nearest_argument_writer(
            source_overlay,
            *call_offset,
            lower_bound,
            Register::A0,
            layout.consumer,
        )?;
        ensure!(
            writer == guard.writer_offset,
            "{:?} practical-exam renderer call +0x{call_offset:04x} A0 writer changed: expected +0x{:04x}, found +0x{writer:04x}",
            layout.consumer,
            guard.writer_offset
        );
        validate_descriptor_writer_grammar(source_overlay, layout.consumer, *guard)?;
    }
    Ok(())
}

fn validate_descriptor_writer_grammar(
    source_overlay: &[u8],
    consumer: PracticalExamConsumer,
    guard: DescriptorCallGuard,
) -> Result<()> {
    let actual = decode_instruction(source_overlay, guard.writer_offset, consumer)?;
    match guard.selection {
        ScheduledDescriptorSelection::Exact { descriptor_index } => {
            let expected = if descriptor_index == 0 {
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                }
            } else {
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: i16::try_from(descriptor_index)?,
                }
            };
            ensure!(
                actual == expected,
                "{consumer:?} practical-exam exact descriptor writer +0x{:04x} changed: expected {expected:?}, found {actual:?}",
                guard.writer_offset
            );
        }
        ScheduledDescriptorSelection::RuntimeChoice {
            first_descriptor_index: 0,
            last_descriptor_index: 48,
        } => match guard.writer_offset {
            0x2760 => ensure!(
                actual
                    == (Instruction::Addiu {
                        rt: Register::A0,
                        rs: Register::A0,
                        immediate: 2,
                    })
                    && decode_instruction(source_overlay, 0x2750, consumer)?
                        == (Instruction::Lbu {
                            rt: Register::A0,
                            base: Register::V0,
                            offset: 0x000c,
                        }),
                "{consumer:?} practical-exam first runtime descriptor grammar changed"
            ),
            0x2810 => {
                let expected = [
                    (
                        0x27f0,
                        Instruction::Lbu {
                            rt: Register::V1,
                            base: Register::V0,
                            offset: 0x000c,
                        },
                    ),
                    (
                        0x27f4,
                        Instruction::Lbu {
                            rt: Register::V0,
                            base: Register::V0,
                            offset: 0x000d,
                        },
                    ),
                    (
                        0x2800,
                        Instruction::Sll {
                            rd: Register::A0,
                            rt: Register::V1,
                            shift: 2,
                        },
                    ),
                    (
                        0x2804,
                        Instruction::Addu {
                            rd: Register::A0,
                            rs: Register::A0,
                            rt: Register::V1,
                        },
                    ),
                    (
                        0x2808,
                        Instruction::Addiu {
                            rt: Register::V0,
                            rs: Register::V0,
                            immediate: 16,
                        },
                    ),
                    (
                        0x2810,
                        Instruction::Addu {
                            rd: Register::A0,
                            rs: Register::A0,
                            rt: Register::V0,
                        },
                    ),
                ];
                ensure!(
                    expected.into_iter().all(|(offset, expected)| {
                        decode_instruction(source_overlay, offset, consumer)
                            .is_ok_and(|actual| actual == expected)
                    }),
                    "{consumer:?} practical-exam second runtime descriptor grammar changed"
                );
            }
            offset => anyhow::bail!(
                "{consumer:?} practical-exam runtime descriptor writer +0x{offset:04x} is uncatalogued"
            ),
        },
        ScheduledDescriptorSelection::UnitStrideSequence {
            first_descriptor_index: 10,
            last_descriptor_index: 14,
        } => {
            let expected = [
                (
                    0x2740,
                    Instruction::Addu {
                        rd: Register::S1,
                        rs: Register::ZERO,
                        rt: Register::ZERO,
                    },
                ),
                (
                    0x27a0,
                    Instruction::Addiu {
                        rt: Register::A0,
                        rs: Register::S1,
                        immediate: 10,
                    },
                ),
                (
                    0x27b4,
                    Instruction::Addiu {
                        rt: Register::S1,
                        rs: Register::S1,
                        immediate: 1,
                    },
                ),
                (
                    0x27b8,
                    Instruction::Slti {
                        rt: Register::V0,
                        rs: Register::S1,
                        immediate: 5,
                    },
                ),
                (
                    0x27bc,
                    Instruction::Bne {
                        rs: Register::V0,
                        rt: Register::ZERO,
                        target: 0x800a_479c,
                    },
                ),
            ];
            ensure!(
                guard.writer_offset == 0x27a0
                    && expected.into_iter().all(|(offset, expected)| {
                        decode_instruction(source_overlay, offset, consumer)
                            .is_ok_and(|actual| actual == expected)
                    }),
                "{consumer:?} practical-exam descriptor 10..=14 unit-stride grammar changed"
            );
        }
        selection => anyhow::bail!(
            "{consumer:?} practical-exam descriptor selection {selection:?} has no source grammar"
        ),
    }
    Ok(())
}

fn nearest_argument_writer(
    source_overlay: &[u8],
    call_offset: usize,
    lower_bound: usize,
    register: Register,
    consumer: PracticalExamConsumer,
) -> Result<usize> {
    let delay_offset = call_offset
        .checked_add(4)
        .context("practical-exam renderer delay-slot offset overflow")?;
    if decode_instruction(source_overlay, delay_offset, consumer)?.written_gpr() == Some(register) {
        return Ok(delay_offset);
    }
    for offset in (lower_bound..call_offset).step_by(4).rev() {
        if decode_instruction(source_overlay, offset, consumer)?.written_gpr() == Some(register) {
            return Ok(offset);
        }
    }
    anyhow::bail!(
        "{consumer:?} practical-exam renderer call +0x{call_offset:04x} has no audited {register:?} writer"
    )
}

fn decode_instruction(
    overlay: &[u8],
    offset: usize,
    consumer: PracticalExamConsumer,
) -> Result<Instruction> {
    let bytes = instruction_bytes(overlay, offset, consumer)?;
    decode_bytes(&bytes, runtime_address(offset)?).with_context(|| {
        format!("{consumer:?} practical-exam instruction +0x{offset:04x} failed typed decoding")
    })
}

fn ensure_non_overlapping_write_ranges(
    ranges: &[[usize; 2]],
    consumer: PracticalExamConsumer,
) -> Result<()> {
    let mut ordered = ranges.to_vec();
    ordered.sort_unstable();
    ensure!(
        ordered
            .windows(2)
            .all(|window| window[0][1] <= window[1][0]),
        "{consumer:?} practical-exam primitive schedule has overlapping Expected Writes"
    );
    Ok(())
}

fn instruction_bytes(
    overlay: &[u8],
    offset: usize,
    consumer: PracticalExamConsumer,
) -> Result<[u8; 4]> {
    overlay
        .get(offset..offset + 4)
        .with_context(|| {
            format!("{consumer:?} practical-exam caller instruction +0x{offset:04x} is truncated")
        })?
        .try_into()
        .context("practical-exam caller instruction is not four bytes")
}

fn runtime_address(offset: usize) -> Result<u32> {
    OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("practical-exam caller runtime address overflow")
}

fn caller_layout(consumer: PracticalExamConsumer) -> CallerLayout {
    match consumer {
        PracticalExamConsumer::BasicsReview => CallerLayout {
            consumer,
            source_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
            overlay_size: 0x7794,
            descriptor_count: 49,
            renderer_runtime_address: 0x800a_3f74,
            renderer_call_offsets: &BASICS_RENDERER_CALL_OFFSETS,
            descriptor_call_guards: &BASICS_DESCRIPTOR_CALL_GUARDS,
            hint_call_offsets: &BASICS_HINT_CALL_OFFSETS,
            zero_start_guard_offsets: &BASICS_ZERO_START_GUARDS,
        },
        PracticalExamConsumer::Exam1999 => CallerLayout {
            consumer,
            source_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
            overlay_size: 0x7840,
            descriptor_count: 8,
            renderer_runtime_address: 0x800a_3890,
            renderer_call_offsets: &EXAM_1999_RENDERER_CALL_OFFSETS,
            descriptor_call_guards: &EXAM_1999_DESCRIPTOR_CALL_GUARDS,
            hint_call_offsets: &EXAM_1999_HINT_CALL_OFFSETS,
            zero_start_guard_offsets: &EXAM_1999_ZERO_START_GUARDS,
        },
    }
}
