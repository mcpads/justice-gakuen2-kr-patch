use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::{HANGUL_NAME_TAG, NAME_GLYPH_CACHE_SLOT_COUNT, NameInputRuntimeAtlasLayout};

#[path = "redisplay_runtime/compound_final_backspace.rs"]
mod compound_final_backspace;
#[path = "redisplay_runtime/final_tables.rs"]
mod final_tables;
#[path = "redisplay_runtime/selected_glyph_validation.rs"]
mod selected_glyph_validation;
#[path = "redisplay_runtime/selected_key_handler.rs"]
mod selected_key_handler;

use compound_final_backspace::emit_compound_final_backspace;
use final_tables::{compound_final_table, simple_final_table};
use selected_glyph_validation::emit_selected_glyph_validator;
use selected_key_handler::emit_selected_key_handler;

pub const NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN: u32 = 0x8010_12e0;
pub const NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY: usize = 0x0520;

const INSTRUCTION_BYTE_CAPACITY: usize = 0x04b4;
const CACHE_CELL_UPLOADER_OFFSET: usize = 0x0114;
const COMPOUND_FINAL_TABLE_OFFSET: usize = INSTRUCTION_BYTE_CAPACITY;
const COMPOUND_FINAL_TABLE_BYTE_CAPACITY: usize = 0x0018;
const SIMPLE_FINAL_TABLE_OFFSET: usize =
    COMPOUND_FINAL_TABLE_OFFSET + COMPOUND_FINAL_TABLE_BYTE_CAPACITY;
const SIMPLE_FINAL_TABLE_BYTE_CAPACITY: usize = 0x0014;
const CACHE_CELL_TABLE_OFFSET: usize = SIMPLE_FINAL_TABLE_OFFSET + SIMPLE_FINAL_TABLE_BYTE_CAPACITY;
const CACHE_CELL_TABLE_ENTRY_BYTES: usize = 4;
const CACHE_CELL_TABLE_BYTE_COUNT: usize =
    NAME_GLYPH_CACHE_SLOT_COUNT * CACHE_CELL_TABLE_ENTRY_BYTES;
pub(crate) const NAME_GLYPH_CACHE_TABLE_ADDRESS: u32 =
    NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + CACHE_CELL_TABLE_OFFSET as u32;
const UPLOAD_TIM_HEADER_BYTES: usize = 20;
const UPLOAD_TIM_PIXEL_BYTES: usize = 20 * 20 / 2;
const UPLOAD_TIM_BYTES: usize = UPLOAD_TIM_HEADER_BYTES + UPLOAD_TIM_PIXEL_BYTES;
const UPLOAD_STACK_FRAME_BYTES: i16 = 240;
const UPLOAD_STACK_RA_OFFSET: i16 = UPLOAD_STACK_FRAME_BYTES - 4;

const NAME_FONT_PIXEL_ADDRESS: u32 = 0x800e_92e0;
const NAME_FONT_VRAM_X_WORDS: u16 = 768;
const NAME_FONT_ROW_BYTES: u16 = 384;
const NAME_CODE_LOOKUP_TABLE_ADDRESS: u32 = 0x8017_ad14;
const FAMILY_NAME_SPRITE_BUFFER_ADDRESS: u32 = 0x801c_8118;
const GIVEN_NAME_SPRITE_BUFFER_ADDRESS: u32 = 0x801c_82d8;
const NICKNAME_SPRITE_BUFFER_ADDRESS: u32 = 0x801c_8498;
const SPRITE_BUFFER_RECORD_BYTES: i16 = 0x38;
const FAMILY_NAME_SPRITE_BUFFER_BYTES: i16 = 6 * SPRITE_BUFFER_RECORD_BYTES;
const GIVEN_NAME_SPRITE_BUFFER_BYTES: i16 = 6 * SPRITE_BUFFER_RECORD_BYTES;
const NICKNAME_SPRITE_BUFFER_BYTES: i16 = 4 * SPRITE_BUFFER_RECORD_BYTES;
pub(in crate::name_input) const ENGINE_RUNTIME_TABLE_POINTER_ADDRESS: u32 = 0x801f_6360;
pub(in crate::name_input) const TIM_LOADER_FUNCTION_OFFSET: i16 = 336;
const FIRST_CACHE_CODE: u16 = 0x032d;
const HANGUL_PAYLOAD_MASK: u16 = 0x3fff;
const TAG_MASK: u16 = 0xc000;
const FAILURE_CODE: u16 = u16::MAX;
const INCOMPLETE_INITIAL_TAG: u16 = 0xc000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameInputRedisplayRuntimeProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub tagged_code_resolver_address: u32,
    pub cache_cell_uploader_address: u32,
    pub selected_key_handler_address: u32,
    pub compound_final_backspace_address: u32,
    pub report: NameInputRedisplayRuntimeProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputRedisplayRuntimeProgramReport {
    pub origin: String,
    pub byte_count: usize,
    pub byte_capacity: usize,
    pub sha256: String,
    pub typed_instruction_count: usize,
    pub instruction_byte_count: usize,
    pub tagged_code_resolver_address: String,
    pub cache_cell_uploader_address: String,
    pub selected_key_handler_address: String,
    pub compound_final_backspace_address: String,
    pub compound_final_table_byte_range: [usize; 2],
    pub simple_final_table_byte_range: [usize; 2],
    pub cache_cell_table_byte_range: [usize; 2],
    pub upload_tim_stack_byte_count: usize,
    pub upload_tim_pixel_byte_count: usize,
    pub materializer_address: String,
    pub fits_runtime_region: bool,
    pub installed: bool,
    pub renderer_hook_installed: bool,
    pub selection_hook_installed: bool,
    pub runtime_execution_verified: bool,
}

pub fn build_name_input_redisplay_runtime_program(
    atlas: &NameInputRuntimeAtlasLayout,
    materializer_address: u32,
    component_resolver_address: u32,
) -> Result<NameInputRedisplayRuntimeProgram> {
    build_redisplay_runtime(
        atlas,
        materializer_address,
        SelectedGlyphValidation::Components(component_resolver_address),
    )
}

pub fn build_name_input_band_redisplay_runtime_program(
    atlas: &NameInputRuntimeAtlasLayout,
    materializer_address: u32,
    pack: &super::NameGlyphBandPack,
) -> Result<NameInputRedisplayRuntimeProgram> {
    build_redisplay_runtime(
        atlas,
        materializer_address,
        SelectedGlyphValidation::Bands {
            reader: super::NAME_INPUT_RUNTIME_ORIGIN,
            membership: u16::try_from(pack.membership_byte_offset())?,
        },
    )
}

enum SelectedGlyphValidation {
    Components(u32),
    Bands { reader: u32, membership: u16 },
}

fn build_redisplay_runtime(
    atlas: &NameInputRuntimeAtlasLayout,
    materializer_address: u32,
    validation: SelectedGlyphValidation,
) -> Result<NameInputRedisplayRuntimeProgram> {
    ensure!(
        materializer_address.is_multiple_of(4),
        "name glyph materializer address is unaligned"
    );
    if let SelectedGlyphValidation::Components(address) = validation {
        ensure!(
            address.is_multiple_of(4),
            "name glyph component resolver address is unaligned"
        );
    }
    let cache_table = encode_cache_cell_table(atlas)?;
    let compound_final_table = compound_final_table();
    let simple_final_table = simple_final_table();
    let cache_cell_uploader_address = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
        .checked_add(CACHE_CELL_UPLOADER_OFFSET as u32)
        .context("name cache uploader address overflow")?;

    let mut assembler = Assembler::new();
    emit_tagged_code_resolver(
        &mut assembler,
        materializer_address,
        cache_cell_uploader_address,
        matches!(validation, SelectedGlyphValidation::Bands { .. }),
    );
    let resolver = assembler
        .assemble(NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN)
        .context("failed to place typed tagged-name redisplay resolver")?;
    ensure!(
        resolver.bytes().len() <= CACHE_CELL_UPLOADER_OFFSET,
        "typed tagged-name resolver overlaps the cache uploader"
    );
    for _ in 0..(CACHE_CELL_UPLOADER_OFFSET - resolver.bytes().len()) / 4 {
        assembler.emit(Instruction::nop());
    }
    emit_cache_cell_uploader(&mut assembler);
    let resolver_and_uploader = assembler
        .assemble(NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN)
        .context("failed to place typed tagged-name cache uploader")?;
    let selected_key_handler_address = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
        .checked_add(u32::try_from(resolver_and_uploader.bytes().len())?)
        .context("selected name-key handler address overflow")?;
    emit_selected_key_handler(
        &mut assembler,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + SIMPLE_FINAL_TABLE_OFFSET as u32,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + COMPOUND_FINAL_TABLE_OFFSET as u32,
        matches!(validation, SelectedGlyphValidation::Bands { .. })
            .then_some(super::runtime::MEDIAL_TRANSITION_ADDRESS),
    );
    match validation {
        SelectedGlyphValidation::Components(address) => {
            emit_selected_glyph_validator(&mut assembler, address)
        }
        SelectedGlyphValidation::Bands { reader, membership } => {
            selected_glyph_validation::emit_selected_band_glyph_validator(
                &mut assembler,
                reader,
                membership,
            )
        }
    }
    // Resolve the forward helper only for measuring this prefix. The final
    // assembly below binds it to the actual implementation.
    let selection_and_validation = assembler
        .clone()
        .label("split_name_final")
        .assemble(NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN)
        .context("failed to place typed selected-name validation")?;
    let compound_final_backspace_address = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
        .checked_add(u32::try_from(selection_and_validation.bytes().len())?)
        .context("compound-final backspace address overflow")?;
    emit_compound_final_backspace(
        &mut assembler,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + SIMPLE_FINAL_TABLE_OFFSET as u32,
        NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN + COMPOUND_FINAL_TABLE_OFFSET as u32,
        matches!(validation, SelectedGlyphValidation::Bands { .. })
            .then_some(super::NAME_INPUT_MEDIAL_BACKSPACE_ADDRESS),
    );
    let program = assembler
        .assemble(NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN)
        .context("failed to assemble typed tagged-name redisplay runtime")?;
    ensure!(
        program.bytes().len() <= INSTRUCTION_BYTE_CAPACITY,
        "typed tagged-name redisplay instructions require {:#x} bytes but own {INSTRUCTION_BYTE_CAPACITY:#x}",
        program.bytes().len()
    );
    ensure!(
        program.instruction_spans().len() == program.instructions().len(),
        "tagged-name redisplay runtime lost typed instruction placement evidence"
    );
    let instructions = program.instructions().to_vec();

    let mut bytes = program.bytes().to_vec();
    bytes.resize(COMPOUND_FINAL_TABLE_OFFSET, 0);
    bytes.extend_from_slice(&compound_final_table);
    bytes.resize(SIMPLE_FINAL_TABLE_OFFSET, 0);
    bytes.extend_from_slice(&simple_final_table);
    bytes.resize(CACHE_CELL_TABLE_OFFSET, 0);
    bytes.extend_from_slice(&cache_table);
    ensure!(
        bytes.len() <= NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY,
        "tagged-name redisplay runtime exceeds the source-bound padding"
    );

    Ok(NameInputRedisplayRuntimeProgram {
        instructions,
        tagged_code_resolver_address: NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
        cache_cell_uploader_address,
        selected_key_handler_address,
        compound_final_backspace_address,
        report: NameInputRedisplayRuntimeProgramReport {
            origin: format!("0x{NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN:08x}"),
            byte_count: bytes.len(),
            byte_capacity: NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count: program.instruction_spans().len(),
            instruction_byte_count: program.bytes().len(),
            tagged_code_resolver_address: format!("0x{NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN:08x}"),
            cache_cell_uploader_address: format!("0x{cache_cell_uploader_address:08x}"),
            selected_key_handler_address: format!("0x{selected_key_handler_address:08x}"),
            compound_final_backspace_address: format!("0x{compound_final_backspace_address:08x}"),
            compound_final_table_byte_range: [
                COMPOUND_FINAL_TABLE_OFFSET,
                COMPOUND_FINAL_TABLE_OFFSET + compound_final_table.len(),
            ],
            simple_final_table_byte_range: [
                SIMPLE_FINAL_TABLE_OFFSET,
                SIMPLE_FINAL_TABLE_OFFSET + simple_final_table.len(),
            ],
            cache_cell_table_byte_range: [
                CACHE_CELL_TABLE_OFFSET,
                CACHE_CELL_TABLE_OFFSET + cache_table.len(),
            ],
            upload_tim_stack_byte_count: UPLOAD_TIM_BYTES,
            upload_tim_pixel_byte_count: UPLOAD_TIM_PIXEL_BYTES,
            materializer_address: format!("0x{materializer_address:08x}"),
            fits_runtime_region: true,
            installed: false,
            renderer_hook_installed: false,
            selection_hook_installed: false,
            runtime_execution_verified: false,
        },
        bytes,
    })
}

fn emit_tagged_code_resolver(
    assembler: &mut Assembler,
    materializer_address: u32,
    cache_cell_uploader_address: u32,
    cache_unchanged: bool,
) {
    assembler
        .label("resolve_tagged_name_for_redisplay")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -32,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::S2,
            rs: Register::A2,
            rt: Register::ZERO,
        })
        .emit_all(load_address(
            Register::T0,
            FAMILY_NAME_SPRITE_BUFFER_ADDRESS,
        ))
        .emit(Instruction::Subu {
            rd: Register::T1,
            rs: Register::A1,
            rt: Register::T0,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: FAMILY_NAME_SPRITE_BUFFER_BYTES,
        })
        .bne(Register::T0, Register::ZERO, "redisplay_field_ready")
        .emit(Instruction::Addu {
            rd: Register::S1,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -((GIVEN_NAME_SPRITE_BUFFER_ADDRESS - FAMILY_NAME_SPRITE_BUFFER_ADDRESS)
                as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: GIVEN_NAME_SPRITE_BUFFER_BYTES,
        })
        .bne(Register::T0, Register::ZERO, "redisplay_field_ready")
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -((NICKNAME_SPRITE_BUFFER_ADDRESS - GIVEN_NAME_SPRITE_BUFFER_ADDRESS)
                as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: NICKNAME_SPRITE_BUFFER_BYTES,
        })
        .beq(Register::T0, Register::ZERO, "redisplay_code_failed")
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: 2,
        })
        .label("redisplay_field_ready")
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::S0,
            immediate: TAG_MASK,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .beq(Register::T0, Register::T1, "redisplay_hangul_code")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: super::EMPTY_NAME_SLOT,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: INCOMPLETE_INITIAL_TAG,
        })
        .bne(Register::T0, Register::T1, "redisplay_code_ready")
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        // An unfinished initial uses the same static glyph as its keyboard key.
        .emit(Instruction::Andi {
            rt: Register::V0,
            rs: Register::S0,
            immediate: 0x001f,
        })
        .jump("redisplay_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: super::NAME_GLYPH_CODE_START as i16,
        })
        .label("redisplay_hangul_code")
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::S1,
            shift: 1,
        })
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::S1,
            shift: 2,
        })
        .emit(Instruction::Addu {
            rd: Register::S2,
            rs: Register::S2,
            rt: Register::T0,
        })
        .emit(Instruction::Addu {
            rd: Register::S2,
            rs: Register::S2,
            rt: Register::T1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::S2,
            immediate: NAME_GLYPH_CACHE_SLOT_COUNT as i16,
        })
        .beq(Register::T0, Register::ZERO, "redisplay_code_failed")
        .emit(Instruction::Andi {
            rt: Register::A0,
            rs: Register::S0,
            immediate: HANGUL_PAYLOAD_MASK,
        });
    if cache_unchanged {
        assembler
            .emit_all(load_address(
                Register::S1,
                super::runtime::REDISPLAY_CACHE_TAG_ADDRESS,
            ))
            .emit(Instruction::Sll {
                rd: Register::T0,
                rt: Register::S2,
                shift: 1,
            })
            .emit(Instruction::Addu {
                rd: Register::S1,
                rs: Register::S1,
                rt: Register::T0,
            })
            .emit(Instruction::Lhu {
                rt: Register::T0,
                base: Register::S1,
                offset: 0,
            })
            .emit(Instruction::nop())
            .beq(Register::T0, Register::S0, "redisplay_cached_code")
            .emit(Instruction::nop());
    }
    assembler
        .emit(Instruction::Jal {
            target: materializer_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::S2,
            rt: Register::ZERO,
        })
        .beq(Register::V0, Register::ZERO, "redisplay_code_failed")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: cache_cell_uploader_address,
        })
        .emit(Instruction::nop())
        .beq(Register::V0, Register::ZERO, "redisplay_code_failed")
        .emit(Instruction::nop());
    if cache_unchanged {
        assembler.emit(Instruction::Sh {
            rt: Register::S0,
            base: Register::S1,
            offset: 0,
        });
    }
    assembler
        .label("redisplay_cached_code")
        .jump("redisplay_code_ready")
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::S2,
            immediate: FIRST_CACHE_CODE as i16,
        })
        .label("redisplay_code_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: FAILURE_CODE,
        })
        .label("redisplay_code_ready")
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit_all(load_address(Register::A1, NAME_CODE_LOOKUP_TABLE_ADDRESS))
        .emit(Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 32,
        });
}

fn emit_cache_cell_uploader(assembler: &mut Assembler) {
    assembler
        .label("upload_name_glyph_cache_cell")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -UPLOAD_STACK_FRAME_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: UPLOAD_STACK_RA_OFFSET,
        })
        .emit_all(load_address(Register::T0, NAME_GLYPH_CACHE_TABLE_ADDRESS))
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::A0,
            shift: 2,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Lhu {
            rt: Register::T1,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Lbu {
            rt: Register::T2,
            base: Register::T0,
            offset: 2,
        })
        .emit(Instruction::Lbu {
            rt: Register::T3,
            base: Register::T0,
            offset: 3,
        })
        .emit_all(load_address(Register::T0, NAME_FONT_PIXEL_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: NAME_FONT_VRAM_X_WORDS as i16,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0x10,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::SP,
            offset: 0,
        })
        .emit(Instruction::Sw {
            rt: Register::ZERO,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: (12 + UPLOAD_TIM_PIXEL_BYTES) as u16,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Sh {
            rt: Register::T2,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Sh {
            rt: Register::T3,
            base: Register::SP,
            offset: 14,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 5,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 20,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::SP,
            offset: 18,
        })
        .emit(Instruction::Addu {
            rd: Register::T4,
            rs: Register::SP,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::T5,
            rs: Register::SP,
            immediate: UPLOAD_TIM_HEADER_BYTES as i16,
        })
        .emit(Instruction::Ori {
            rt: Register::T6,
            rs: Register::ZERO,
            immediate: 20,
        })
        .label("copy_cache_upload_row")
        .emit(Instruction::Ori {
            rt: Register::T7,
            rs: Register::ZERO,
            immediate: 10,
        })
        .label("copy_cache_upload_byte")
        .emit(Instruction::Lbu {
            rt: Register::T8,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 1,
        })
        .emit(Instruction::Sb {
            rt: Register::T8,
            base: Register::T5,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T5,
            rs: Register::T5,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T7,
            rs: Register::T7,
            immediate: -1,
        })
        .bgtz(Register::T7, "copy_cache_upload_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: (NAME_FONT_ROW_BYTES - 10) as i16,
        })
        .emit(Instruction::Addiu {
            rt: Register::T6,
            rs: Register::T6,
            immediate: -1,
        })
        .bgtz(Register::T6, "copy_cache_upload_row")
        .emit(Instruction::nop())
        .emit_all(load_address(
            Register::T0,
            ENGINE_RUNTIME_TABLE_POINTER_ADDRESS,
        ))
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::T0,
            offset: TIM_LOADER_FUNCTION_OFFSET,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Jalr {
            rd: Register::RA,
            rs: Register::T0,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T4,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: UPLOAD_STACK_RA_OFFSET,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: UPLOAD_STACK_FRAME_BYTES,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

fn encode_cache_cell_table(atlas: &NameInputRuntimeAtlasLayout) -> Result<Vec<u8>> {
    ensure!(
        atlas.cache_cell_base_byte_offsets.len() == NAME_GLYPH_CACHE_SLOT_COUNT,
        "name cache upload table requires all sixteen persistent slots"
    );
    let mut table = Vec::with_capacity(CACHE_CELL_TABLE_BYTE_COUNT);
    for offset in &atlas.cache_cell_base_byte_offsets {
        let row = *offset / NAME_FONT_ROW_BYTES;
        let byte_x = *offset % NAME_FONT_ROW_BYTES;
        ensure!(
            byte_x.is_multiple_of(2),
            "name cache cell is not aligned to a 4-bpp VRAM word"
        );
        let vram_x_offset = u8::try_from(byte_x / 2)?;
        let vram_y = u8::try_from(row)?;
        table.extend_from_slice(&offset.to_le_bytes());
        table.push(vram_x_offset);
        table.push(vram_y);
    }
    Ok(table)
}
