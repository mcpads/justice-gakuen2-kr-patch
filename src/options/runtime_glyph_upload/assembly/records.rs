use std::ops::Range;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};

use super::{UploadProgram, build_upload_after_native_call};
use crate::options::runtime_glyph_upload::MENU_RUNTIME_BASE;

pub(crate) const RECORDS_ENTRY_HOOK_OFFSET: usize = 0x0d5e0;
pub(crate) const RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS: u32 = 0x8001_cde0;
pub(crate) const RECORDS_ENTRY_SETUP_ADDRESS: u32 = 0x8001_4aa4;
pub(crate) const RECORDS_ENTRY_PROGRAM_OFFSET: usize = 0x85040;
pub(crate) const RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS: u32 =
    MENU_RUNTIME_BASE + RECORDS_ENTRY_PROGRAM_OFFSET as u32;
pub(crate) const RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY: usize = 0x80;

pub(crate) const RECORDS_EXIT_HOOK_OFFSET: usize = 0x0d664;
pub(crate) const RECORDS_EXIT_HOOK_RUNTIME_ADDRESS: u32 = 0x8001_ce64;
pub(crate) const RECORDS_EXIT_TEARDOWN_ADDRESS: u32 = 0x8001_0664;
pub(crate) const RECORDS_EXIT_RESTORE_PROGRAM_OFFSET: usize =
    RECORDS_ENTRY_PROGRAM_OFFSET + RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY;
pub(crate) const RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS: u32 =
    MENU_RUNTIME_BASE + RECORDS_EXIT_RESTORE_PROGRAM_OFFSET as u32;
pub(crate) const RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY: usize = 0x80;

pub(crate) const RECORDS_LOAD_RETURN_HOOK_OFFSET: usize = 0x0da84;
pub(crate) const RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS: u32 = 0x8001_d284;
pub(crate) const RECORDS_SAVE_RETURN_HOOK_OFFSET: usize = 0x0de78;
pub(crate) const RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS: u32 = 0x8001_d678;
pub(crate) const RECORDS_CARD_OPERATION_RETURN_ADDRESS: u32 = 0x8001_4d54;
pub(crate) const RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET: usize =
    RECORDS_EXIT_RESTORE_PROGRAM_OFFSET + RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY;
pub(crate) const RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS: u32 =
    MENU_RUNTIME_BASE + RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET as u32;
pub(crate) const RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY: usize = 0x80;

pub(crate) const BOOT_NOTICE_ENTRY_HOOK_OFFSET: usize = 0x0db68;
pub(crate) const BOOT_NOTICE_LOAD_ARGUMENT_OFFSET: usize = 0x0db58;
pub(crate) const BOOT_NOTICE_ENTRY_HOOK_ADDRESS: u32 = 0x8001_d368;
pub(crate) const BOOT_NOTICE_EXIT_HOOK_OFFSET: usize = 0x0db98;
pub(crate) const BOOT_NOTICE_EXIT_HOOK_ADDRESS: u32 = 0x8001_d398;
pub(crate) const BOOT_NOTICE_PROGRAM_OFFSET: usize = RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET
    + RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY;
pub(crate) const BOOT_NOTICE_PROGRAM_ADDRESS: u32 =
    MENU_RUNTIME_BASE + BOOT_NOTICE_PROGRAM_OFFSET as u32;
pub(crate) const BOOT_NOTICE_PROGRAM_CAPACITY: usize = 0x80;

pub(crate) fn build_boot_notice_upload_program(
    descriptor: u32,
    count: usize,
) -> Result<UploadProgram> {
    let program = build_upload_after_native_call(
        BOOT_NOTICE_PROGRAM_ADDRESS,
        RECORDS_EXIT_TEARDOWN_ADDRESS,
        descriptor,
        count,
        "upload_boot_notice_glyph",
        "boot autoload notice glyph upload",
    )?;
    ensure!(
        program.bytes.len() <= BOOT_NOTICE_PROGRAM_CAPACITY,
        "boot notice uploader exceeds its slot"
    );
    Ok(program)
}

pub(crate) fn patch_boot_notice_calls(
    source: &[u8],
    output: &mut [u8],
) -> Result<Vec<RecordsHookInstall>> {
    // The native branch loads only the font (catalog 43). MENU (catalog 42)
    // starts with the identical TIM and also contains our upload/restore data.
    // Bind its existing native loader before redirecting this boot-only load.
    super::verify_native_menu_restore_sequence(source)?;
    for (address, expected) in [
        (
            0x8001_d354,
            Instruction::Jal {
                target: 0x8001_5414,
            },
        ),
        (
            0x8001_d358,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 0x2b,
            },
        ),
        (
            0x8001_d360,
            Instruction::Jal {
                target: 0x8001_5fcc,
            },
        ),
        (
            0x8001_d364,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x4000,
            },
        ),
        (
            0x8001_d390,
            Instruction::Jal {
                target: 0x8001_e774,
            },
        ),
        (
            0x8001_d394,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
    ] {
        let offset = (address - crate::source_disc::MAIN_TEXT_RUNTIME_BASE) as usize
            + crate::source_disc::PSX_EXE_HEADER_SIZE;
        ensure!(
            decode_records_instruction(source, offset, address)? == expected,
            "boot notice MENU load/display sequence changed at {address:#x}"
        );
        ensure!(
            output.get(offset..offset + 4) == source.get(offset..offset + 4),
            "boot notice MENU load/display sequence overlaps another writer"
        );
    }
    let mut writes = Vec::new();
    let load_argument = Instruction::Addiu {
        rt: Register::A1,
        rs: Register::ZERO,
        immediate: super::MENU_CATALOG_INDEX as i16,
    };
    let address = BOOT_NOTICE_ENTRY_HOOK_ADDRESS - 0x10;
    output[BOOT_NOTICE_LOAD_ARGUMENT_OFFSET..BOOT_NOTICE_LOAD_ARGUMENT_OFFSET + 4]
        .copy_from_slice(&encode(&load_argument, address)?.to_le_bytes());
    ensure!(
        decode_records_instruction(output, BOOT_NOTICE_LOAD_ARGUMENT_OFFSET, address)?
            == load_argument,
        "boot notice MENU selector failed typed readback"
    );
    writes.push(RecordsHookInstall {
        id: "slps:options:boot-notice-menu-load",
        purpose: "load the complete MENU record so boot glyph code and payloads are resident",
        range: BOOT_NOTICE_LOAD_ARGUMENT_OFFSET..BOOT_NOTICE_LOAD_ARGUMENT_OFFSET + 4,
        runtime_address: address,
        instructions: vec![load_argument],
    });
    for (offset, address, wrapper, argument, id, purpose) in [
        (
            BOOT_NOTICE_ENTRY_HOOK_OFFSET,
            BOOT_NOTICE_ENTRY_HOOK_ADDRESS,
            BOOT_NOTICE_PROGRAM_ADDRESS,
            1,
            "slps:options:boot-notice-entry-hook",
            "upload Korean card glyphs after boot MENU atlas setup",
        ),
        (
            BOOT_NOTICE_EXIT_HOOK_OFFSET,
            BOOT_NOTICE_EXIT_HOOK_ADDRESS,
            RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS,
            3,
            "slps:options:boot-notice-exit-hook",
            "restore source glyph cells when the boot notice returns",
        ),
    ] {
        writes.push(patch_records_call(
            source,
            output,
            RecordsHook {
                offset,
                runtime_address: address,
                native_call_address: RECORDS_EXIT_TEARDOWN_ADDRESS,
                wrapper_address: wrapper,
                delay_slot: Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::ZERO,
                    immediate: argument,
                },
                id,
                purpose,
                context: "boot autoload notice",
            },
        )?);
    }
    Ok(writes)
}

#[derive(Debug)]
pub(crate) struct RecordsHookInstall {
    pub(crate) id: &'static str,
    pub(crate) purpose: &'static str,
    pub(crate) range: Range<usize>,
    pub(crate) runtime_address: u32,
    pub(crate) instructions: Vec<Instruction>,
}

pub(crate) fn build_records_entry_upload_program(
    descriptor_runtime_address: u32,
    entry_count: usize,
) -> Result<UploadProgram> {
    let program = build_upload_after_native_call(
        RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS,
        RECORDS_ENTRY_SETUP_ADDRESS,
        descriptor_runtime_address,
        entry_count,
        "upload_records_contextual_glyph",
        "Records entry contextual-glyph upload",
    )?;
    ensure!(
        program.bytes.len() <= RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY,
        "Records entry glyph upload program exceeds its fixed storage"
    );
    Ok(program)
}

pub(crate) fn build_records_exit_restore_program(
    descriptor_runtime_address: u32,
    entry_count: usize,
) -> Result<UploadProgram> {
    let program = build_upload_after_native_call(
        RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS,
        RECORDS_EXIT_TEARDOWN_ADDRESS,
        descriptor_runtime_address,
        entry_count,
        "restore_records_source_graphic",
        "Records exit source-graphic restore",
    )?;
    ensure!(
        program.bytes.len() <= RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
        "Records exit source-graphic restore program exceeds its fixed storage"
    );
    Ok(program)
}

pub(crate) fn build_records_card_operation_refresh_program(
    descriptor_runtime_address: u32,
    entry_count: usize,
) -> Result<UploadProgram> {
    let program = build_upload_after_native_call(
        RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS,
        RECORDS_CARD_OPERATION_RETURN_ADDRESS,
        descriptor_runtime_address,
        entry_count,
        "refresh_records_after_card_operation",
        "Records card-operation return contextual-glyph refresh",
    )?;
    ensure!(
        program.bytes.len() <= RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
        "Records card-operation refresh program exceeds its fixed storage"
    );
    Ok(program)
}

pub(crate) fn patch_records_entry_setup_call(
    source: &[u8],
    output: &mut [u8],
) -> Result<RecordsHookInstall> {
    patch_records_call(
        source,
        output,
        RecordsHook {
            offset: RECORDS_ENTRY_HOOK_OFFSET,
            runtime_address: RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS,
            native_call_address: RECORDS_ENTRY_SETUP_ADDRESS,
            wrapper_address: RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS,
            delay_slot: Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 5,
            },
            id: "slps:options:records-context-entry-hook",
            purpose: "call the typed Records contextual-glyph uploader",
            context: "Records entry",
        },
    )
}

pub(crate) fn patch_records_exit_teardown_call(
    source: &[u8],
    output: &mut [u8],
) -> Result<RecordsHookInstall> {
    patch_records_call(
        source,
        output,
        RecordsHook {
            offset: RECORDS_EXIT_HOOK_OFFSET,
            runtime_address: RECORDS_EXIT_HOOK_RUNTIME_ADDRESS,
            native_call_address: RECORDS_EXIT_TEARDOWN_ADDRESS,
            wrapper_address: RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS,
            delay_slot: Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 3,
            },
            id: "slps:options:records-context-exit-hook",
            purpose: "call the typed Records source-graphic restore routine",
            context: "Records exit",
        },
    )
}

pub(crate) fn patch_records_load_return_call(
    source: &[u8],
    output: &mut [u8],
) -> Result<RecordsHookInstall> {
    patch_records_card_operation_return_call(
        source,
        output,
        RECORDS_LOAD_RETURN_HOOK_OFFSET,
        RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS,
        "slps:options:records-load-return-refresh-hook",
        "refresh the Records contextual glyphs after the Load substate returns",
        "Records Load return",
    )
}

pub(crate) fn patch_records_save_return_call(
    source: &[u8],
    output: &mut [u8],
) -> Result<RecordsHookInstall> {
    patch_records_card_operation_return_call(
        source,
        output,
        RECORDS_SAVE_RETURN_HOOK_OFFSET,
        RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS,
        "slps:options:records-save-return-refresh-hook",
        "refresh the Records contextual glyphs after the Save substate returns",
        "Records Save return",
    )
}

fn patch_records_card_operation_return_call(
    source: &[u8],
    output: &mut [u8],
    offset: usize,
    runtime_address: u32,
    id: &'static str,
    purpose: &'static str,
    context: &'static str,
) -> Result<RecordsHookInstall> {
    patch_records_call(
        source,
        output,
        RecordsHook {
            offset,
            runtime_address,
            native_call_address: RECORDS_CARD_OPERATION_RETURN_ADDRESS,
            wrapper_address: RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS,
            delay_slot: Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
            id,
            purpose,
            context,
        },
    )
}

struct RecordsHook {
    offset: usize,
    runtime_address: u32,
    native_call_address: u32,
    wrapper_address: u32,
    delay_slot: Instruction,
    id: &'static str,
    purpose: &'static str,
    context: &'static str,
}

fn patch_records_call(
    source: &[u8],
    output: &mut [u8],
    hook: RecordsHook,
) -> Result<RecordsHookInstall> {
    ensure!(
        source.starts_with(b"PS-X EXE") && output.starts_with(b"PS-X EXE"),
        "{} hook requires PS-X EXE inputs",
        hook.context
    );
    ensure!(
        decode_records_instruction(source, hook.offset, hook.runtime_address)?
            == Instruction::Jal {
                target: hook.native_call_address,
            }
            && decode_records_instruction(source, hook.offset + 4, hook.runtime_address + 4)?
                == hook.delay_slot,
        "{} hook source changed",
        hook.context
    );
    ensure!(
        output.get(hook.offset..hook.offset + 4) == source.get(hook.offset..hook.offset + 4),
        "{} hook overlaps another writer",
        hook.context
    );
    let instruction = Instruction::Jal {
        target: hook.wrapper_address,
    };
    let encoded = encode(&instruction, hook.runtime_address)?;
    output[hook.offset..hook.offset + 4].copy_from_slice(&encoded.to_le_bytes());
    ensure!(
        decode_records_instruction(output, hook.offset, hook.runtime_address)? == instruction
            && decode_records_instruction(output, hook.offset + 4, hook.runtime_address + 4)?
                == hook.delay_slot,
        "{} hook failed final decode verification",
        hook.context
    );
    Ok(RecordsHookInstall {
        id: hook.id,
        purpose: hook.purpose,
        range: hook.offset..hook.offset + 4,
        runtime_address: hook.runtime_address,
        instructions: vec![instruction],
    })
}

fn decode_records_instruction(
    bytes: &[u8],
    offset: usize,
    runtime_address: u32,
) -> Result<Instruction> {
    let word = u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .with_context(|| format!("Records instruction at {offset:#x} is truncated"))?
            .try_into()
            .expect("four-byte instruction"),
    );
    decode(word, runtime_address).context("failed to decode Records instruction")
}
