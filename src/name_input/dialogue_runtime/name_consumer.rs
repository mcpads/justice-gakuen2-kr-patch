use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::dialogue_runtime_bootstrap::DIRECT_NAME_HUD_STORE_ORIGIN;
use super::super::{ASCII_NAME_TAG, HANGUL_NAME_TAG};

const ORIGINAL_NAME_CONSUMER_ADDRESS: u32 = 0x800a_f9ac;
const FAMILY_NAME_ADDRESS: u32 = 0x801f_1866;
const GIVEN_NAME_ADDRESS: u32 = 0x801f_1876;
pub(super) const NICKNAME_ADDRESS: u32 = 0x801f_1896;
pub(super) const RELATIONSHIP_NAME_ADDRESS: u32 = 0x801f_18a6;
pub(super) const RELATIONSHIP_NAME_RECORD_CELL_CAPACITY: usize = 8;
pub(super) const RELATIONSHIP_NAME_FIELD_CATEGORY_REGISTER: &str = "t9";
const MESSAGE_END_CODE: u16 = 0x3001;
const TAG_MASK: u16 = 0xc000;
const HANGUL_PAYLOAD_MASK: u16 = 0x3fff;
const ASCII_PAYLOAD_MASK: u16 = 0x007f;
const FAILURE: u16 = u16::MAX;
const STACK_BYTES: i16 = 96;
const TEMPORARY_RECORD_OFFSET: i16 = 24;

pub(super) fn emit_dialogue_name_consumer(
    assembler: &mut Assembler,
    current_atlas_pointer_address: u32,
    cache_code_start: u16,
) {
    assembler
        .label("consume_shared_dialogue_name")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -STACK_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 92,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 88,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 84,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 80,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: 76,
        })
        .emit(Instruction::Sw {
            rt: Register::S4,
            base: Register::SP,
            offset: 72,
        })
        .emit(Instruction::Sw {
            rt: Register::S5,
            base: Register::SP,
            offset: 68,
        })
        .emit(Instruction::Sw {
            rt: Register::S6,
            base: Register::SP,
            offset: 64,
        })
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::S1,
            rs: Register::A1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::SP,
            immediate: TEMPORARY_RECORD_OFFSET,
        })
        .emit(Instruction::Addu {
            rd: Register::S5,
            rs: Register::A2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::S6,
            rs: Register::A3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::SP,
            offset: STACK_BYTES + 16,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::S0,
            immediate: 12,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::SP,
            offset: 16,
        })
        .emit_all(load_address(Register::T1, current_atlas_pointer_address))
        .emit(Instruction::Sw {
            rt: Register::T2,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Lui {
            rt: Register::T0,
            immediate: (FAMILY_NAME_ADDRESS >> 16) as u16,
        })
        .emit(Instruction::Subu {
            rd: Register::T0,
            rs: Register::S1,
            rt: Register::T0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: -(FAMILY_NAME_ADDRESS as i16),
        })
        .beq(Register::T0, Register::ZERO, "consume_six_cell_name")
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: -((GIVEN_NAME_ADDRESS - FAMILY_NAME_ADDRESS) as i16),
        })
        .beq(Register::T0, Register::ZERO, "consume_six_cell_name")
        .emit(Instruction::Ori {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: 6,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: -((NICKNAME_ADDRESS - GIVEN_NAME_ADDRESS) as i16),
        })
        .beq(Register::T0, Register::ZERO, "consume_nickname")
        .emit(Instruction::Ori {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: 12,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: -((RELATIONSHIP_NAME_ADDRESS - NICKNAME_ADDRESS) as i16),
        })
        // The only dynamic-record caller is the backup nickname loader.
        // It supplies T9=0: four cells use family cache slots, leaving the
        // live nickname's persistent HUD cells unchanged.
        .bne(Register::T0, Register::ZERO, "consume_nickname")
        .emit(Instruction::Sll {
            rd: Register::S3,
            rt: Register::T9,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::S3,
            rt: Register::T9,
        })
        .emit(Instruction::Sll {
            rd: Register::S3,
            rt: Register::S3,
            shift: 1,
        })
        .jump("shared_name_field_ready")
        .emit(Instruction::Ori {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: RELATIONSHIP_NAME_RECORD_CELL_CAPACITY as u16,
        })
        .label("consume_six_cell_name")
        .jump("shared_name_field_ready")
        .emit(Instruction::Ori {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 6,
        })
        .label("consume_nickname")
        .emit(Instruction::Ori {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 4,
        })
        .label("shared_name_field_ready")
        // Never issue a load in the full-field branch delay: its delayed value
        // would overwrite the terminator when the source has an excess cell.
        .beq(Register::S4, Register::ZERO, "finish_shared_name_record")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: MESSAGE_END_CODE,
        })
        .emit(Instruction::Lhu {
            rt: Register::T0,
            base: Register::S1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 2,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: MESSAGE_END_CODE,
        })
        .beq(Register::T0, Register::T1, "finish_shared_name_record")
        .emit(Instruction::Andi {
            rt: Register::T1,
            rs: Register::T0,
            immediate: TAG_MASK,
        })
        .beq(Register::T1, Register::ZERO, "store_direct_name_code")
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .beq(Register::T1, Register::T2, "materialize_shared_hangul")
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: ASCII_NAME_TAG,
        })
        .bne(Register::T1, Register::T2, "terminate_shared_name_record")
        .emit(Instruction::nop())
        .label("resolve_shared_ascii")
        .call("resolve_ascii_name_code")
        .emit(Instruction::Andi {
            rt: Register::A0,
            rs: Register::T0,
            immediate: ASCII_PAYLOAD_MASK,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .beq(Register::V0, Register::T1, "terminate_shared_name_record")
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .jump("store_direct_name_code")
        .emit(Instruction::nop())
        .label("materialize_shared_hangul")
        .emit(Instruction::Andi {
            rt: Register::A0,
            rs: Register::T0,
            immediate: HANGUL_PAYLOAD_MASK,
        })
        .call("materialize_name_glyph_fill")
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .beq(Register::V0, Register::ZERO, "terminate_shared_name_record")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::S3,
            immediate: cache_code_start as i16,
        })
        .label("store_shared_name_code")
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S2,
            offset: 0,
        })
        .jump("advance_shared_name_slot")
        .emit(Instruction::nop())
        .label("store_direct_name_code")
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S2,
            offset: 0,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::T0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: DIRECT_NAME_HUD_STORE_ORIGIN,
        })
        .emit(Instruction::Addiu {
            rt: Register::A2,
            rs: Register::S0,
            immediate: 12,
        })
        .label("advance_shared_name_slot")
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::S3,
            rs: Register::S3,
            immediate: 1,
        })
        .jump("shared_name_field_ready")
        .emit(Instruction::Addiu {
            rt: Register::S4,
            rs: Register::S4,
            immediate: -1,
        })
        .label("terminate_shared_name_record")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: MESSAGE_END_CODE,
        })
        .label("finish_shared_name_record")
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S2,
            offset: 0,
        })
        .label("call_original_name_consumer")
        .emit(Instruction::Addu {
            rd: Register::A3,
            rs: Register::S6,
            rt: Register::ZERO,
        })
        .beq(Register::S5, Register::ZERO, "restore_shared_name_consumer")
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::SP,
            immediate: TEMPORARY_RECORD_OFFSET,
        })
        .label("call_original_with_selected_record")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::S5,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: ORIGINAL_NAME_CONSUMER_ADDRESS,
        })
        .emit(Instruction::nop())
        .label("restore_shared_name_consumer")
        .emit(Instruction::Lw {
            rt: Register::S6,
            base: Register::SP,
            offset: 64,
        })
        .emit(Instruction::Lw {
            rt: Register::S5,
            base: Register::SP,
            offset: 68,
        })
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::SP,
            offset: 72,
        })
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: 76,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 80,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 84,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 92,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 88,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: STACK_BYTES,
        });
}
