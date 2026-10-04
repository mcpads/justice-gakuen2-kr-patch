use std::ops::Range;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, verify_placed_program};

use crate::pipeline::sha256_bytes;

use super::descriptor::PracticalExamConsumer;
use super::primitive_pool::{
    FIRST_SLOT_OFFSET as PRIMITIVE_FIRST_SLOT_OFFSET, RENDERER_SLOT_SCALE_SHIFT,
};

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const RENDERER_SIZE: usize = 0x01f4;
const ORIGINAL_INSTRUCTION_COUNT: usize = RENDERER_SIZE / 4;
const EXPECTED_INDIRECT_CALL_COUNT: usize = 6;

#[derive(Debug, Clone, Copy)]
struct RendererLayout {
    renderer_offset: usize,
    renderer_runtime_address: u32,
    renderer_sha256: &'static str,
    pointer_table_runtime_address: u32,
    primitive_pool_pointer_address: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PracticalExamRendererInstallReport {
    pub(super) consumer: PracticalExamConsumer,
    pub(super) allowed_range: Range<usize>,
    pub(super) renderer_runtime_address: u32,
    pub(super) coordinate_table_runtime_address: u32,
    pub(super) pointer_table_runtime_address: u32,
    pub(super) core_instruction_count: usize,
    pub(super) core_byte_count: usize,
    pub(super) nop_padding_instruction_count: usize,
    pub(super) installed_byte_count: usize,
    pub(super) original_renderer_sha256: String,
    pub(super) installed_renderer_sha256: String,
    pub(super) installed_instructions: Vec<Instruction>,
}

pub(super) fn install_compact_glyph_renderer(
    overlay: &mut [u8],
    consumer: PracticalExamConsumer,
    coordinate_table_runtime_address: u32,
) -> Result<PracticalExamRendererInstallReport> {
    let layout = renderer_layout(consumer);
    let allowed_range = layout.renderer_offset..layout.renderer_offset + RENDERER_SIZE;
    let original = overlay
        .get(allowed_range.clone())
        .with_context(|| format!("{consumer:?} practical-exam renderer is truncated"))?;
    let original_renderer_sha256 = sha256_bytes(original);
    ensure!(
        original_renderer_sha256 == layout.renderer_sha256,
        "{consumer:?} practical-exam renderer source changed: {original_renderer_sha256}"
    );

    let original_instructions = verify_placed_program(original, layout.renderer_runtime_address)
        .with_context(|| format!("{consumer:?} source renderer is not valid placed R3000A code"))?;
    ensure!(
        original_instructions.len() == ORIGINAL_INSTRUCTION_COUNT,
        "{consumer:?} source renderer instruction denominator changed"
    );
    ensure!(
        indirect_call_count(&original_instructions) == EXPECTED_INDIRECT_CALL_COUNT,
        "{consumer:?} source renderer GPU-call denominator changed"
    );

    ensure!(
        coordinate_table_runtime_address >= OVERLAY_RUNTIME_BASE
            && coordinate_table_runtime_address < layout.pointer_table_runtime_address,
        "{consumer:?} compact coordinate table is outside its descriptor arena"
    );
    let coordinate_table_offset = usize::try_from(
        coordinate_table_runtime_address
            .checked_sub(OVERLAY_RUNTIME_BASE)
            .context("coordinate table address is below the overlay")?,
    )?;
    ensure!(
        coordinate_table_offset < overlay.len(),
        "{consumer:?} compact coordinate table is outside its overlay"
    );
    ensure!(
        !ranges_overlap(
            coordinate_table_offset..coordinate_table_offset + 1,
            allowed_range.clone(),
        ),
        "{consumer:?} compact coordinate table overlaps its renderer"
    );

    let pointer_table_delta = signed_address_delta(
        layout.pointer_table_runtime_address,
        coordinate_table_runtime_address,
        "descriptor pointer table",
    )?;
    let primitive_pool_delta = signed_address_delta(
        layout.primitive_pool_pointer_address,
        coordinate_table_runtime_address,
        "primitive-pool pointer",
    )?;

    let mut assembler = compact_glyph_renderer(
        coordinate_table_runtime_address,
        pointer_table_delta,
        primitive_pool_delta,
    );
    let core = assembler
        .assemble(layout.renderer_runtime_address)
        .with_context(|| format!("failed to assemble {consumer:?} compact glyph renderer"))?;
    ensure!(
        core.bytes().len() <= RENDERER_SIZE,
        "{consumer:?} compact glyph renderer is {} bytes, exceeding the {RENDERER_SIZE}-byte slot",
        core.bytes().len()
    );
    ensure!(
        indirect_call_count(core.instructions()) == EXPECTED_INDIRECT_CALL_COUNT,
        "{consumer:?} compact renderer changed the GPU-call denominator"
    );
    ensure_material_addresses(
        core.instructions(),
        coordinate_table_runtime_address,
        pointer_table_delta,
        primitive_pool_delta,
    )?;

    let core_instruction_count = core.instructions().len();
    let core_byte_count = core.bytes().len();
    let nop_padding_instruction_count = (RENDERER_SIZE - core_byte_count) / 4;
    ensure!(
        core_byte_count + nop_padding_instruction_count * 4 == RENDERER_SIZE,
        "{consumer:?} compact renderer has a non-instruction-aligned remainder"
    );
    assembler.emit_all(std::iter::repeat_n(
        Instruction::nop(),
        nop_padding_instruction_count,
    ));
    let installed = assembler
        .assemble(layout.renderer_runtime_address)
        .with_context(|| format!("failed to pad {consumer:?} compact glyph renderer"))?;
    ensure!(
        installed.bytes().len() == RENDERER_SIZE,
        "{consumer:?} compact renderer did not fill its guarded slot"
    );
    let decoded = verify_placed_program(installed.bytes(), layout.renderer_runtime_address)
        .with_context(|| format!("{consumer:?} installed renderer failed placed-code readback"))?;
    ensure!(
        decoded == installed.instructions(),
        "{consumer:?} installed renderer semantic readback changed"
    );

    let prefix_sha256 = sha256_bytes(&overlay[..allowed_range.start]);
    let suffix_sha256 = sha256_bytes(&overlay[allowed_range.end..]);
    overlay[allowed_range.clone()].copy_from_slice(installed.bytes());
    ensure!(
        &overlay[allowed_range.clone()] == installed.bytes(),
        "{consumer:?} compact renderer byte readback changed"
    );
    ensure!(
        sha256_bytes(&overlay[..allowed_range.start]) == prefix_sha256
            && sha256_bytes(&overlay[allowed_range.end..]) == suffix_sha256,
        "{consumer:?} compact renderer escaped its allowed write range"
    );
    let installed_renderer_sha256 = sha256_bytes(&overlay[allowed_range.clone()]);

    Ok(PracticalExamRendererInstallReport {
        consumer,
        allowed_range,
        renderer_runtime_address: layout.renderer_runtime_address,
        coordinate_table_runtime_address,
        pointer_table_runtime_address: layout.pointer_table_runtime_address,
        core_instruction_count,
        core_byte_count,
        nop_padding_instruction_count,
        installed_byte_count: RENDERER_SIZE,
        original_renderer_sha256,
        installed_renderer_sha256,
        installed_instructions: installed.instructions().to_vec(),
    })
}

fn compact_glyph_renderer(
    coordinate_table_runtime_address: u32,
    pointer_table_delta: i16,
    primitive_pool_delta: i16,
) -> Assembler {
    const ZERO: Register = Register::ZERO;
    const AT: Register = Register::AT;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const T0: Register = Register::T0;
    const T1: Register = Register::T1;
    const T2: Register = Register::T2;
    const T3: Register = Register::T3;
    const T4: Register = Register::T4;
    const S0: Register = Register::S0;
    const S1: Register = Register::S1;
    const S2: Register = Register::S2;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S5: Register = Register::S5;
    const S6: Register = Register::S6;
    const S7: Register = Register::S7;
    const SP: Register = Register::SP;
    const FP: Register = Register::FP;
    const RA: Register = Register::RA;

    let mut assembler = Assembler::new();
    assembler
        .emit(addiu(SP, SP, -0x40))
        .emit(sw(RA, SP, 0x3c))
        .emit(sw(FP, SP, 0x38))
        .emit(sw(S7, SP, 0x34))
        .emit(sw(S6, SP, 0x30))
        .emit(sw(S5, SP, 0x2c))
        .emit(sw(S4, SP, 0x28))
        .emit(sw(S3, SP, 0x24))
        .emit(sw(S2, SP, 0x20))
        .emit(sw(S1, SP, 0x1c))
        .emit(sw(S0, SP, 0x18))
        .emit(move_register(S7, A1))
        .emit(move_register(S4, A2))
        .emit(sw(A3, SP, 0x10))
        .emit(sll(A0, A0, 2))
        .emit_all(load_address(S3, coordinate_table_runtime_address))
        .emit(addu(AT, S3, A0))
        .emit(lw(S2, AT, pointer_table_delta))
        .emit(lui(AT, 0x801f))
        .emit(lw(S1, AT, 0x6360))
        .emit(lw(FP, AT, 0x6370))
        .emit(lbu(S5, S2, 0))
        .emit(addiu(S2, S2, 1))
        .beq(S5, ZERO, "done")
        .emit(Instruction::nop())
        .label("loop")
        .emit(lbu(T0, S2, 0))
        .emit(addiu(S2, S2, 1))
        .emit(sll(T1, T0, 1))
        .emit(addu(T1, T1, T0))
        .emit(addu(S6, T1, S3))
        .emit(lbu(A2, S6, 0))
        .emit(sll(V0, S7, RENDERER_SLOT_SCALE_SHIFT))
        .emit(andi(A2, A2, 0x0060))
        .emit(sll(A2, A2, 1))
        .emit(addiu(A2, A2, 0x0300))
        .emit(subu(V0, V0, S7))
        .emit(sll(V0, V0, RENDERER_SLOT_SCALE_SHIFT))
        .emit(addiu(T2, V0, PRIMITIVE_FIRST_SLOT_OFFSET as i16))
        .emit(lw(S0, S3, primitive_pool_delta))
        .emit(lui(AT, 0x801f))
        .emit(lw(V1, AT, 0x608c))
        .emit(addu(S0, S0, T2))
        .emit(sll(V0, V1, 3))
        .emit(subu(V0, V0, V1))
        .emit(sll(V0, V0, 2))
        .emit(addu(S0, S0, V0))
        .emit(lw(V1, FP, 0x18))
        .emit(move_register(A0, ZERO))
        .emit(move_register(A1, ZERO))
        .emit(jalr(V1))
        .emit(move_register(A3, ZERO))
        .emit(move_register(A0, S0))
        .emit(move_register(A1, ZERO))
        .emit(lw(V1, S1, 0x34))
        .emit(addiu(A2, ZERO, 1))
        .emit(jalr(V1))
        .emit(andi(A3, V0, 0xffff))
        .emit(lw(V0, S1, 0x68))
        .emit(addiu(A0, S0, 8))
        .emit(jalr(V0))
        .emit(addiu(S5, S5, -1))
        .emit(lw(V0, FP, 0x14))
        .emit(lw(A0, SP, 0x50))
        .emit(jalr(V0))
        .emit(addiu(A1, ZERO, 0x01e1))
        .emit(sh(V0, S0, 0x16))
        .emit(addiu(T0, ZERO, 0x80))
        .emit(sb(T0, S0, 0x0c))
        .emit(sb(T0, S0, 0x0d))
        .emit(sb(T0, S0, 0x0e))
        .emit(lbu(T0, S6, 1))
        .emit(lbu(T1, S6, 2))
        .emit(lbu(T4, S6, 0))
        .emit(sb(T0, S0, 0x14))
        .emit(sb(T1, S0, 0x15))
        .emit(addiu(T0, ZERO, 0x40))
        .bne(T4, T0, "normal_dimensions")
        .emit(andi(T2, T4, 0x001f))
        .emit(addiu(T2, ZERO, 104))
        .jump("dimensions_ready")
        .emit(addiu(T3, ZERO, 64))
        .label("normal_dimensions")
        .emit(addiu(T2, T2, 1))
        .emit(andi(T0, T4, 0x0080))
        .beq(T0, ZERO, "dimensions_ready")
        .emit(addiu(T3, ZERO, 20))
        .emit(addiu(T3, ZERO, 27))
        .label("dimensions_ready")
        .emit(sh(S4, S0, 0x10))
        .emit(lw(T4, SP, 0x10))
        .emit(sh(T2, S0, 0x18))
        .emit(sh(T3, S0, 0x1a))
        .emit(addiu(T4, T4, 2))
        .emit(sh(T4, S0, 0x12))
        .emit(move_register(A0, S0))
        .emit(lw(V0, S1, 0x20))
        .emit(addiu(S7, S7, 1))
        .emit(jalr(V0))
        .emit(addiu(A1, S0, 8))
        .emit(move_register(A1, S0))
        .emit(lw(V0, S1, 0))
        .emit(lui(A0, 0x801f))
        .emit(lw(A0, A0, 0x6090))
        .emit(Instruction::nop())
        .emit(addiu(A0, A0, 0x1094))
        .emit(jalr(V0))
        .emit(Instruction::nop())
        .emit(Instruction::Lh {
            rt: V0,
            base: S0,
            offset: 0x18,
        })
        .emit(Instruction::nop())
        .emit(addu(S4, S4, V0))
        .bne(S5, ZERO, "loop")
        .emit(Instruction::nop())
        .label("done")
        .emit(lw(RA, SP, 0x3c))
        .emit(lw(FP, SP, 0x38))
        .emit(lw(S7, SP, 0x34))
        .emit(lw(S6, SP, 0x30))
        .emit(lw(S5, SP, 0x2c))
        .emit(lw(S4, SP, 0x28))
        .emit(lw(S3, SP, 0x24))
        .emit(lw(S2, SP, 0x20))
        .emit(lw(S1, SP, 0x1c))
        .emit(lw(S0, SP, 0x18))
        .emit(Instruction::Jr { rs: RA })
        .emit(addiu(SP, SP, 0x40));
    assembler
}

fn renderer_layout(consumer: PracticalExamConsumer) -> RendererLayout {
    match consumer {
        PracticalExamConsumer::BasicsReview => RendererLayout {
            renderer_offset: 0x1f74,
            renderer_runtime_address: 0x800a_3f74,
            renderer_sha256: "a14ce534f52cc26986243e5cf3f9d5bb68828070d42678dc1188290a7e08021b",
            pointer_table_runtime_address: 0x800a_2420,
            primitive_pool_pointer_address: 0x800a_9790,
        },
        PracticalExamConsumer::Exam1999 => RendererLayout {
            renderer_offset: 0x1890,
            renderer_runtime_address: 0x800a_3890,
            renderer_sha256: "3a510490dfb0bf87b309669a4b6bf7cf233d2351fb7fb07431edfb47d141aa99",
            pointer_table_runtime_address: 0x800a_2210,
            primitive_pool_pointer_address: 0x800a_97ac,
        },
    }
}

fn ensure_material_addresses(
    instructions: &[Instruction],
    coordinate_table_runtime_address: u32,
    pointer_table_delta: i16,
    primitive_pool_delta: i16,
) -> Result<()> {
    let coordinate_load = load_address(Register::S3, coordinate_table_runtime_address);
    ensure!(
        instructions
            .windows(coordinate_load.len())
            .any(|window| window == coordinate_load),
        "compact renderer does not materially encode its coordinate-table address"
    );
    ensure!(
        instructions.windows(2).any(|window| {
            window
                == [
                    addu(Register::AT, Register::S3, Register::A0),
                    lw(Register::S2, Register::AT, pointer_table_delta),
                ]
        }),
        "compact renderer does not materially encode its descriptor-pointer table"
    );
    ensure!(
        instructions.contains(&lw(Register::S0, Register::S3, primitive_pool_delta,)),
        "compact renderer does not materially encode its primitive-pool pointer"
    );
    Ok(())
}

fn indirect_call_count(instructions: &[Instruction]) -> usize {
    instructions
        .iter()
        .filter(|instruction| matches!(instruction, Instruction::Jalr { .. }))
        .count()
}

fn signed_address_delta(target: u32, base: u32, role: &str) -> Result<i16> {
    let delta = i64::from(target) - i64::from(base);
    i16::try_from(delta).with_context(|| {
        format!("practical-exam {role} is outside compact renderer signed-offset reach")
    })
}

fn ranges_overlap(left: Range<usize>, right: Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn load_address(register: Register, address: u32) -> [Instruction; 2] {
    let upper = address.wrapping_add(0x8000) >> 16;
    let lower = address as u16 as i16;
    [
        lui(register, upper as u16),
        addiu(register, register, lower),
    ]
}

fn move_register(destination: Register, source: Register) -> Instruction {
    addu(destination, source, Register::ZERO)
}

fn addiu(rt: Register, rs: Register, immediate: i16) -> Instruction {
    Instruction::Addiu { rt, rs, immediate }
}

fn addu(rd: Register, rs: Register, rt: Register) -> Instruction {
    Instruction::Addu { rd, rs, rt }
}

fn subu(rd: Register, rs: Register, rt: Register) -> Instruction {
    Instruction::Subu { rd, rs, rt }
}

fn andi(rt: Register, rs: Register, immediate: u16) -> Instruction {
    Instruction::Andi { rt, rs, immediate }
}

fn lui(rt: Register, immediate: u16) -> Instruction {
    Instruction::Lui { rt, immediate }
}

fn sll(rd: Register, rt: Register, shift: u8) -> Instruction {
    Instruction::Sll { rd, rt, shift }
}

fn lw(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lw { rt, base, offset }
}

fn lbu(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lbu { rt, base, offset }
}

fn sw(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sw { rt, base, offset }
}

fn sb(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sb { rt, base, offset }
}

fn sh(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sh { rt, base, offset }
}

fn jalr(rs: Register) -> Instruction {
    Instruction::Jalr {
        rd: Register::RA,
        rs,
    }
}
