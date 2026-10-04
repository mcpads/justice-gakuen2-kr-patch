use psx_r3000a::{Instruction, Register, encode, load_address, verify_placed_program};

use super::assembly::{
    MENU_CATALOG_INDEX, MENU_LOAD_CALL_RUNTIME_ADDRESS, MENU_LOAD_DESTINATION, MENU_LOADER_ADDRESS,
    MENU_TIM_PARSER_ADDRESS, MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES, MENU_TIM_RUNTIME_ADDRESSES,
    OPTIONS_HOOK_OFFSET, OPTIONS_HOOK_RUNTIME_ADDRESS, OPTIONS_INITIALIZER_ADDRESS,
    OPTIONS_PROGRAM_BYTE_CAPACITY, OPTIONS_PROGRAM_RUNTIME_ADDRESS, PayloadDecompression,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS, RECORDS_CARD_OPERATION_RETURN_ADDRESS,
    RECORDS_ENTRY_HOOK_OFFSET, RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS,
    RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY, RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS,
    RECORDS_ENTRY_SETUP_ADDRESS, RECORDS_EXIT_HOOK_OFFSET, RECORDS_EXIT_HOOK_RUNTIME_ADDRESS,
    RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY, RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS,
    RECORDS_EXIT_TEARDOWN_ADDRESS, RECORDS_LOAD_RETURN_HOOK_OFFSET,
    RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS, RECORDS_SAVE_RETURN_HOOK_OFFSET,
    RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS, RUNTIME_DECOMPRESSOR_ADDRESS, UPLOAD_ROUTINE_ADDRESS,
    build_options_upload_program, build_records_card_operation_refresh_program,
    build_records_entry_upload_program, build_records_exit_restore_program,
    patch_options_initializer_call, patch_records_entry_setup_call,
    patch_records_exit_teardown_call, patch_records_load_return_call,
    patch_records_save_return_call, verify_native_menu_restore_sequence,
};
use super::options_context::{
    COMPRESSED_PAYLOAD_SLOT_INDICES, DESCRIPTOR_OFFSET, DESCRIPTOR_SLOT_INDEX, PADDING_BYTE_COUNT,
    SCRATCH_OFFSET, SOURCE_TIM_BYTE_COUNT, runtime_decompressor_uses_ring_buffer_window,
};
use super::records_context::{
    RECORDS_FIRST_STORAGE_START, RECORDS_STORAGE_END, RECORDS_STORAGE_SLOT_COUNT, allocate_storage,
};
use super::{
    PACKED_CELL_BYTE_COUNT, PRESERVED_SOURCE_GRAPHIC_CODES, PreparedGlyph,
    capture_records_source_graphics, pack_indexed_cell, select_records_glyphs,
};
use crate::options::description_source::ITEM_SLOT_SIZE;
use crate::options::model::OptionsFontRole;
use crate::source_disc::{MAIN_TEXT_RUNTIME_BASE, MAIN_TEXT_SIZE, PSX_EXE_HEADER_SIZE};
use crate::tim::Cell;

// This only exercises current-scale instruction and storage pressure. Production
// completeness is derived from output-code set equality, never from this count.
const REPRESENTATIVE_RECORDS_ENTRY_COUNT: usize = 53;

// The boot autoload notice shares translated resident Records text. Its first
// draw must have a font supplier, and leaving it must restore the overwritten
// source cells before the opening/title can consume them.
#[test]
#[ignore = "requires the supported source disc"]
fn boot_notice_font_lifetime_wraps_the_native_menu_load_and_notice_exit() {
    let cue = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let source = crate::options::source::load_options_source(&cue).unwrap();
    let mut glyphs = [prepared_glyph(0x0236, 0, false)];
    glyphs[0].payload.fill(0x33);
    let mut restorations = [prepared_glyph(0x0236, 0, true)];
    restorations[0].payload.fill(0x14);
    let mut menu = source.menu_decoded.clone();
    let install = super::records_context::install(
        &glyphs,
        &restorations,
        &source.menu_decoded,
        &mut menu,
        &source.main_executable,
    )
    .unwrap();
    for address in [0x8001_d368u32, 0x8001_d398] {
        let selector_offset =
            0x8001d358usize - MAIN_TEXT_RUNTIME_BASE as usize + PSX_EXE_HEADER_SIZE;
        let selector = u32::from_le_bytes(
            install.main_executable_candidate[selector_offset..selector_offset + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            psx_r3000a::decode(selector, 0x8001d358).unwrap(),
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: MENU_CATALOG_INDEX as i16,
            },
            "the boot font-only record does not contain the uploader or descriptors"
        );
        let offset = (address - MAIN_TEXT_RUNTIME_BASE) as usize + PSX_EXE_HEADER_SIZE;
        assert_ne!(
            install.main_executable_candidate[offset..offset + 4],
            source.main_executable[offset..offset + 4],
            "boot notice has no font lifetime hook at {address:#x}"
        );
        assert_eq!(
            install.main_executable_candidate[offset + 4..offset + 8],
            source.main_executable[offset + 4..offset + 8]
        );
        let word = u32::from_le_bytes(
            install.main_executable_candidate[offset..offset + 4]
                .try_into()
                .unwrap(),
        );
        let Instruction::Jal { target } = psx_r3000a::decode(word, address).unwrap() else {
            panic!("missing uploader call")
        };
        let program_offset = (target - super::MENU_RUNTIME_BASE) as usize;
        let mut memory = vec![0u8; 0x200000];
        memory[0xd4000..0xd4000 + menu.len()].copy_from_slice(&menu);
        let mut r = [0u32; 32];
        r[4] = if address == 0x8001_d368 { 1 } else { 3 };
        r[16] = 0x1234;
        r[17] = 0x5678;
        r[29] = 0x801fff00;
        r[31] = 0x80004000;
        let preserved = r;
        let mut calls = Vec::new();
        crate::name_input::runtime_test_machine::execute_with_callbacks(
            &menu[program_offset..program_offset + 0x80],
            target,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, memory| {
                if pc == RECORDS_EXIT_TEARDOWN_ADDRESS {
                    assert!(calls.is_empty());
                    assert_eq!(r[4], preserved[4]);
                    calls.push(pc);
                    r[2] = 0x7654;
                    return true;
                }
                if pc == UPLOAD_ROUTINE_ADDRESS {
                    assert_eq!(calls, [RECORDS_EXIT_TEARDOWN_ADDRESS]);
                    let a = (r[5] & 0x1fffffff) as usize;
                    let expected = if address == 0x8001_d368 {
                        &glyphs[0].payload
                    } else {
                        &restorations[0].payload
                    };
                    assert_eq!(&memory[a..a + expected.len()], expected);
                    calls.push(pc);
                    r[2] = 0xdead;
                    return true;
                }
                false
            },
        );
        assert_eq!(
            calls,
            [RECORDS_EXIT_TEARDOWN_ADDRESS, UPLOAD_ROUTINE_ADDRESS]
        );
        assert_eq!(r[2], 0x7654);
        for register in [16, 17, 29, 31] {
            assert_eq!(r[register], preserved[register]);
        }
    }
    assert_eq!(
        &menu[..crate::pipeline::EMBEDDED_MOJI2_TIM_SIZE],
        &source.menu_decoded[..crate::pipeline::EMBEDDED_MOJI2_TIM_SIZE]
    );
    let mut drifted = source.main_executable;
    drifted[0x8001d358usize - MAIN_TEXT_RUNTIME_BASE as usize + PSX_EXE_HEADER_SIZE] ^= 1;
    let mut unchanged_menu = source.menu_decoded.clone();
    assert!(
        super::records_context::install(
            &glyphs,
            &restorations,
            &source.menu_decoded,
            &mut unchanged_menu,
            &drifted
        )
        .is_err()
    );
}

#[test]
fn options_uploader_decompresses_context_payload_before_the_gpu_upload() {
    let descriptor_address = 0x800e_4b18;
    let streams = [
        PayloadDecompression {
            input_start: 0x800d_c040,
            input_end: 0x800d_c440,
            output_start: 0x800f_e800,
        },
        PayloadDecompression {
            input_start: 0x800e_d040,
            input_end: 0x800e_d440,
            output_start: 0x8010_0800,
        },
    ];
    let entry_count = 104;
    let program = build_options_upload_program(descriptor_address, entry_count, &streams)
        .expect("build OPTIONS contextual texture uploader");
    assert!(program.bytes.len() <= OPTIONS_PROGRAM_BYTE_CAPACITY);
    let instructions = verify_placed_program(&program.bytes, OPTIONS_PROGRAM_RUNTIME_ADDRESS)
        .expect("decode OPTIONS contextual texture uploader");

    let native_upload_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: OPTIONS_INITIALIZER_ADDRESS,
                }
        })
        .expect("native OPTIONS atlas upload");
    let decompression_indices = instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            (*instruction
                == Instruction::Jal {
                    target: RUNTIME_DECOMPRESSOR_ADDRESS,
                })
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let upload_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: UPLOAD_ROUTINE_ADDRESS,
                }
        })
        .expect("contextual texture upload");
    assert_eq!(decompression_indices.len(), streams.len());
    assert!(
        native_upload_index < decompression_indices[0] && decompression_indices[1] < upload_index
    );
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::V0,
        base: Register::SP,
        offset: 24,
    }));
    assert!(instructions.contains(&Instruction::Lw {
        rt: Register::V0,
        base: Register::SP,
        offset: 24,
    }));
    for instruction in load_address(Register::S0, descriptor_address) {
        assert!(instructions.contains(&instruction));
    }
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::S1,
        rs: Register::ZERO,
        immediate: entry_count as u16,
    }));
    assert_eq!(
        instructions[instructions.len() - 2],
        Instruction::Jr { rs: Register::RA }
    );
    assert_eq!(
        instructions[instructions.len() - 1],
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 40,
        }
    );
}

#[test]
fn options_hook_replaces_only_the_native_call_and_preserves_its_delay_slot() {
    let mut source = vec![0u8; OPTIONS_HOOK_OFFSET + 8];
    let original = encode(
        &Instruction::Jal {
            target: OPTIONS_INITIALIZER_ADDRESS,
        },
        OPTIONS_HOOK_RUNTIME_ADDRESS,
    )
    .unwrap();
    source[OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4].copy_from_slice(&original.to_le_bytes());
    let mut output = source.clone();
    patch_options_initializer_call(&source, &mut output).unwrap();

    assert_eq!(
        &output[..OPTIONS_HOOK_OFFSET],
        &source[..OPTIONS_HOOK_OFFSET]
    );
    assert_eq!(
        &output[OPTIONS_HOOK_OFFSET + 4..],
        &source[OPTIONS_HOOK_OFFSET + 4..]
    );
    assert_ne!(
        &output[OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4],
        &source[OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4]
    );

    let mut wrong_source = source;
    wrong_source[OPTIONS_HOOK_OFFSET] ^= 1;
    assert!(patch_options_initializer_call(&wrong_source, &mut output).is_err());
}

#[test]
fn options_exit_relies_on_the_source_bound_native_menu_reload() {
    let mut source = native_menu_restore_executable();
    verify_native_menu_restore_sequence(&source).expect("verify native MENU reload");

    let call_offset = PSX_EXE_HEADER_SIZE
        + usize::try_from(MENU_LOAD_CALL_RUNTIME_ADDRESS - MAIN_TEXT_RUNTIME_BASE).unwrap();
    source[call_offset] ^= 1;
    assert!(verify_native_menu_restore_sequence(&source).is_err());
}

fn native_menu_restore_executable() -> Vec<u8> {
    let mut executable = vec![0u8; PSX_EXE_HEADER_SIZE + MAIN_TEXT_SIZE];
    executable[..8].copy_from_slice(b"PS-X EXE");
    executable[0x18..0x1c].copy_from_slice(&MAIN_TEXT_RUNTIME_BASE.to_le_bytes());
    executable[0x1c..0x20].copy_from_slice(&(MAIN_TEXT_SIZE as u32).to_le_bytes());
    let mut instructions = vec![
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS - 8,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (MENU_LOAD_DESTINATION >> 16) as u16,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS - 4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: MENU_LOAD_DESTINATION as u16,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: MENU_LOADER_ADDRESS,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS + 4,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: MENU_CATALOG_INDEX as i16,
            },
        ),
    ];
    for (call_address, tim_address) in MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES
        .into_iter()
        .zip(MENU_TIM_RUNTIME_ADDRESSES)
    {
        instructions.extend([
            (
                call_address - 4,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (tim_address >> 16) as u16,
                },
            ),
            (
                call_address,
                Instruction::Jal {
                    target: MENU_TIM_PARSER_ADDRESS,
                },
            ),
            (
                call_address + 4,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: tim_address as u16,
                },
            ),
        ]);
    }
    for (address, instruction) in instructions {
        let offset =
            PSX_EXE_HEADER_SIZE + usize::try_from(address - MAIN_TEXT_RUNTIME_BASE).unwrap();
        let word = encode(&instruction, address).unwrap();
        executable[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    executable
}

#[test]
fn records_uploader_runs_the_entry_setup_before_upload_and_preserves_its_result() {
    let descriptor_address = 0x8015_9140;
    let entry_count = REPRESENTATIVE_RECORDS_ENTRY_COUNT;
    let program = build_records_entry_upload_program(descriptor_address, entry_count)
        .expect("build Records contextual glyph uploader");
    assert!(program.bytes.len() <= RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY);
    let instructions = verify_placed_program(&program.bytes, RECORDS_ENTRY_PROGRAM_RUNTIME_ADDRESS)
        .expect("decode Records contextual glyph uploader");

    let entry_setup_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: RECORDS_ENTRY_SETUP_ADDRESS,
                }
        })
        .expect("native Records entry setup call");
    let upload_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: UPLOAD_ROUTINE_ADDRESS,
                }
        })
        .expect("native GPU upload call");
    assert!(entry_setup_index < upload_index);
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
    assert!(instructions.contains(&Instruction::Lw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
    for instruction in load_address(Register::S0, descriptor_address) {
        assert!(instructions.contains(&instruction));
    }
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::S1,
        rs: Register::ZERO,
        immediate: entry_count as u16,
    }));
    assert_eq!(
        instructions[instructions.len() - 2],
        Instruction::Jr { rs: Register::RA }
    );
    assert_eq!(
        instructions[instructions.len() - 1],
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 32,
        }
    );
}

#[test]
fn records_entry_hook_replaces_only_the_setup_call_and_preserves_a0_delay_slot() {
    let mut source = vec![0u8; RECORDS_ENTRY_HOOK_OFFSET + 8];
    source[..8].copy_from_slice(b"PS-X EXE");
    let original = encode(
        &Instruction::Jal {
            target: RECORDS_ENTRY_SETUP_ADDRESS,
        },
        RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS,
    )
    .unwrap();
    source[RECORDS_ENTRY_HOOK_OFFSET..RECORDS_ENTRY_HOOK_OFFSET + 4]
        .copy_from_slice(&original.to_le_bytes());
    let delay_slot = encode(
        &Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 5,
        },
        RECORDS_ENTRY_HOOK_RUNTIME_ADDRESS + 4,
    )
    .unwrap();
    source[RECORDS_ENTRY_HOOK_OFFSET + 4..RECORDS_ENTRY_HOOK_OFFSET + 8]
        .copy_from_slice(&delay_slot.to_le_bytes());
    let mut output = source.clone();
    patch_records_entry_setup_call(&source, &mut output).unwrap();

    assert_eq!(
        &output[..RECORDS_ENTRY_HOOK_OFFSET],
        &source[..RECORDS_ENTRY_HOOK_OFFSET]
    );
    assert_eq!(
        &output[RECORDS_ENTRY_HOOK_OFFSET + 4..],
        &source[RECORDS_ENTRY_HOOK_OFFSET + 4..]
    );
    assert_ne!(
        &output[RECORDS_ENTRY_HOOK_OFFSET..RECORDS_ENTRY_HOOK_OFFSET + 4],
        &source[RECORDS_ENTRY_HOOK_OFFSET..RECORDS_ENTRY_HOOK_OFFSET + 4]
    );

    let mut wrong_source = source;
    wrong_source[RECORDS_ENTRY_HOOK_OFFSET] ^= 1;
    let mut wrong_output = wrong_source.clone();
    assert!(patch_records_entry_setup_call(&wrong_source, &mut wrong_output).is_err());
}

#[test]
fn records_exit_restores_source_graphics_after_native_teardown() {
    let descriptor_address = 0x8015_9218;
    let entry_count = REPRESENTATIVE_RECORDS_ENTRY_COUNT;
    let program = build_records_exit_restore_program(descriptor_address, entry_count)
        .expect("build Records exit source-graphic restore");
    assert!(program.bytes.len() <= RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY);
    let instructions =
        verify_placed_program(&program.bytes, RECORDS_EXIT_RESTORE_PROGRAM_RUNTIME_ADDRESS)
            .expect("decode Records exit source-graphic restore");

    let teardown_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: RECORDS_EXIT_TEARDOWN_ADDRESS,
                }
        })
        .expect("native Records exit teardown call");
    let restore_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: UPLOAD_ROUTINE_ADDRESS,
                }
        })
        .expect("source-graphic restore upload");
    assert!(teardown_index < restore_index);
    for instruction in load_address(Register::S0, descriptor_address) {
        assert!(instructions.contains(&instruction));
    }
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::S1,
        rs: Register::ZERO,
        immediate: entry_count as u16,
    }));
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
    assert!(instructions.contains(&Instruction::Lw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
}

#[test]
fn records_exit_hook_replaces_only_teardown_call_and_preserves_a0_delay_slot() {
    let mut source = vec![0u8; RECORDS_EXIT_HOOK_OFFSET + 8];
    source[..8].copy_from_slice(b"PS-X EXE");
    let original = encode(
        &Instruction::Jal {
            target: RECORDS_EXIT_TEARDOWN_ADDRESS,
        },
        RECORDS_EXIT_HOOK_RUNTIME_ADDRESS,
    )
    .unwrap();
    source[RECORDS_EXIT_HOOK_OFFSET..RECORDS_EXIT_HOOK_OFFSET + 4]
        .copy_from_slice(&original.to_le_bytes());
    let delay_slot = encode(
        &Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 3,
        },
        RECORDS_EXIT_HOOK_RUNTIME_ADDRESS + 4,
    )
    .unwrap();
    source[RECORDS_EXIT_HOOK_OFFSET + 4..RECORDS_EXIT_HOOK_OFFSET + 8]
        .copy_from_slice(&delay_slot.to_le_bytes());
    let mut output = source.clone();
    patch_records_exit_teardown_call(&source, &mut output).unwrap();

    assert_eq!(
        &output[..RECORDS_EXIT_HOOK_OFFSET],
        &source[..RECORDS_EXIT_HOOK_OFFSET]
    );
    assert_eq!(
        &output[RECORDS_EXIT_HOOK_OFFSET + 4..],
        &source[RECORDS_EXIT_HOOK_OFFSET + 4..]
    );
    assert_ne!(
        &output[RECORDS_EXIT_HOOK_OFFSET..RECORDS_EXIT_HOOK_OFFSET + 4],
        &source[RECORDS_EXIT_HOOK_OFFSET..RECORDS_EXIT_HOOK_OFFSET + 4]
    );

    let mut wrong_source = source;
    wrong_source[RECORDS_EXIT_HOOK_OFFSET] ^= 1;
    let mut wrong_output = wrong_source.clone();
    assert!(patch_records_exit_teardown_call(&wrong_source, &mut wrong_output).is_err());
}

#[test]
fn records_card_operation_refresh_runs_after_the_native_return_transition() {
    let descriptor_address = 0x8015_91c0;
    let entry_count = REPRESENTATIVE_RECORDS_ENTRY_COUNT;
    let program = build_records_card_operation_refresh_program(descriptor_address, entry_count)
        .expect("build Records card-operation return refresh");
    assert!(program.bytes.len() <= RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY);
    let instructions = verify_placed_program(
        &program.bytes,
        RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS,
    )
    .expect("decode Records card-operation return refresh");

    let native_return_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: RECORDS_CARD_OPERATION_RETURN_ADDRESS,
                }
        })
        .expect("native Records card-operation return transition");
    let refresh_index = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: UPLOAD_ROUTINE_ADDRESS,
                }
        })
        .expect("Records contextual texture refresh");
    assert!(native_return_index < refresh_index);
    for instruction in load_address(Register::S0, descriptor_address) {
        assert!(instructions.contains(&instruction));
    }
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::S1,
        rs: Register::ZERO,
        immediate: entry_count as u16,
    }));
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
    assert!(instructions.contains(&Instruction::Lw {
        rt: Register::V0,
        base: Register::SP,
        offset: 16,
    }));
}

#[test]
fn records_load_and_save_returns_share_the_bounded_refresh_program() {
    let mut source = vec![0u8; RECORDS_SAVE_RETURN_HOOK_OFFSET + 8];
    source[..8].copy_from_slice(b"PS-X EXE");
    let zero_a0 = Instruction::Addu {
        rd: Register::A0,
        rs: Register::ZERO,
        rt: Register::ZERO,
    };
    for (offset, runtime_address) in [
        (
            RECORDS_LOAD_RETURN_HOOK_OFFSET,
            RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS,
        ),
        (
            RECORDS_SAVE_RETURN_HOOK_OFFSET,
            RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS,
        ),
    ] {
        let original = encode(
            &Instruction::Jal {
                target: RECORDS_CARD_OPERATION_RETURN_ADDRESS,
            },
            runtime_address,
        )
        .unwrap();
        source[offset..offset + 4].copy_from_slice(&original.to_le_bytes());
        let delay = encode(&zero_a0, runtime_address + 4).unwrap();
        source[offset + 4..offset + 8].copy_from_slice(&delay.to_le_bytes());
    }

    let mut output = source.clone();
    patch_records_load_return_call(&source, &mut output).unwrap();
    patch_records_save_return_call(&source, &mut output).unwrap();
    let wrapper = Instruction::Jal {
        target: RECORDS_CARD_OPERATION_REFRESH_PROGRAM_RUNTIME_ADDRESS,
    };
    for (offset, runtime_address) in [
        (
            RECORDS_LOAD_RETURN_HOOK_OFFSET,
            RECORDS_LOAD_RETURN_HOOK_RUNTIME_ADDRESS,
        ),
        (
            RECORDS_SAVE_RETURN_HOOK_OFFSET,
            RECORDS_SAVE_RETURN_HOOK_RUNTIME_ADDRESS,
        ),
    ] {
        assert_eq!(
            &output[offset..offset + 4],
            &encode(&wrapper, runtime_address).unwrap().to_le_bytes()
        );
        assert_eq!(
            &output[offset + 4..offset + 8],
            &source[offset + 4..offset + 8]
        );
    }
    for (index, (after, before)) in output.iter().zip(&source).enumerate() {
        let is_hook_instruction = [
            RECORDS_LOAD_RETURN_HOOK_OFFSET,
            RECORDS_SAVE_RETURN_HOOK_OFFSET,
        ]
        .iter()
        .any(|offset| (*offset..*offset + 4).contains(&index));
        if !is_hook_instruction {
            assert_eq!(
                after, before,
                "unexpected write at executable offset 0x{index:06x}"
            );
        }
    }

    let mut drifted = source;
    drifted[RECORDS_LOAD_RETURN_HOOK_OFFSET] ^= 1;
    let mut drifted_output = drifted.clone();
    assert!(patch_records_load_return_call(&drifted, &mut drifted_output).is_err());
}

#[test]
fn records_context_selects_its_complete_entry_scoped_population() {
    let protected_codes = PRESERVED_SOURCE_GRAPHIC_CODES.to_vec();
    let protected = protected_codes
        .iter()
        .copied()
        .enumerate()
        .map(|(index, code)| prepared_glyph(code, index, false))
        .collect::<Vec<_>>();
    let shared = [0x0100, 0x0101]
        .into_iter()
        .enumerate()
        .map(|(index, code)| prepared_glyph(code, index + protected.len(), false))
        .collect::<Vec<_>>();
    let mut records_output_codes = protected_codes.clone();
    records_output_codes.extend([0x0100, 0x0101, 0x0055, 0x0fff]);

    let selected = select_records_glyphs(&shared, &protected, &records_output_codes).unwrap();
    let mut expected_codes = protected_codes.clone();
    expected_codes.extend([0x0100, 0x0101]);
    expected_codes.sort_unstable();
    assert_eq!(
        selected.iter().map(|glyph| glyph.code).collect::<Vec<_>>(),
        expected_codes
    );

    let missing_protected_code = &records_output_codes[1..];
    assert!(
        select_records_glyphs(&shared, &protected, missing_protected_code)
            .unwrap_err()
            .to_string()
            .contains("population changed")
    );
    let incomplete_shared = &shared[..1];
    assert!(
        select_records_glyphs(incomplete_shared, &protected, &records_output_codes)
            .unwrap_err()
            .to_string()
            .contains("unavailable contextual glyph")
    );
}

#[test]
fn records_exit_restore_captures_every_selected_cell_geometry() {
    let source = menu_font_fixture();
    let output = source.clone();
    let records = vec![
        PreparedGlyph {
            role: OptionsFontRole::RecordsMainItem.key().to_string(),
            character: Some('저'),
            code: 0x0100,
            cell: Cell {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            },
            source_preservation: false,
            payload: vec![0; 200],
        },
        PreparedGlyph {
            role: OptionsFontRole::RecordsMainHeading.key().to_string(),
            character: Some('기'),
            code: 0x0102,
            cell: Cell {
                x: 40,
                y: 0,
                width: 40,
                height: 40,
            },
            source_preservation: false,
            payload: vec![0; 800],
        },
    ];

    let restorations = capture_records_source_graphics(&source, &output, &records).unwrap();
    assert_eq!(
        restorations
            .iter()
            .map(|glyph| glyph.code)
            .collect::<Vec<_>>(),
        [0x0100, 0x0102]
    );
    assert!(restorations.iter().all(|glyph| glyph.source_preservation));
    assert_eq!(restorations[0].payload.len(), 200);
    assert_eq!(restorations[1].payload.len(), 800);

    let mut changed_output = output;
    changed_output[64] ^= 1;
    assert!(
        capture_records_source_graphics(&source, &changed_output, &records)
            .unwrap_err()
            .to_string()
            .contains("overwrote Records source cell")
    );
}

fn prepared_glyph(code: u16, index: usize, source_preservation: bool) -> PreparedGlyph {
    PreparedGlyph {
        role: if source_preservation {
            "source_menu_graphic_restore".to_string()
        } else {
            OptionsFontRole::Value.key().to_string()
        },
        character: (!source_preservation).then(|| char::from_u32(0xac00 + index as u32).unwrap()),
        code,
        cell: Cell {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
        source_preservation,
        payload: vec![u8::try_from(index).unwrap(); PACKED_CELL_BYTE_COUNT],
    }
}

fn menu_font_fixture() -> Vec<u8> {
    const CLUT_BLOCK_SIZE: usize = 44;
    const IMAGE_WORD_WIDTH: usize = 256;
    const IMAGE_HEIGHT: usize = 256;
    const IMAGE_BLOCK_SIZE: usize = 12 + IMAGE_WORD_WIDTH * IMAGE_HEIGHT * 2;
    let mut tim = vec![0x21; 8 + CLUT_BLOCK_SIZE + IMAGE_BLOCK_SIZE];
    write_u32(&mut tim, 0, 0x10);
    write_u32(&mut tim, 4, 0x08);
    write_u32(&mut tim, 8, CLUT_BLOCK_SIZE as u32);
    write_u16(&mut tim, 16, 16);
    write_u16(&mut tim, 18, 1);
    let image_offset = 8 + CLUT_BLOCK_SIZE;
    write_u32(&mut tim, image_offset, IMAGE_BLOCK_SIZE as u32);
    write_u16(&mut tim, image_offset + 4, 768);
    write_u16(&mut tim, image_offset + 8, IMAGE_WORD_WIDTH as u16);
    write_u16(&mut tim, image_offset + 10, IMAGE_HEIGHT as u16);
    tim
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn packed_cells_preserve_low_then_high_nibble_pixel_order() {
    let pixels = (0..20 * 20)
        .map(|index| (index % 16) as u8)
        .collect::<Vec<_>>();
    let packed = pack_indexed_cell(&pixels).unwrap();
    assert_eq!(packed.len(), PACKED_CELL_BYTE_COUNT);
    assert_eq!(packed[0], 0x10);
    assert_eq!(packed[1], 0x32);
    assert_eq!(packed[7], 0xfe);

    let mut invalid = pixels;
    invalid[0] = 16;
    assert!(pack_indexed_cell(&invalid).is_err());
}

#[test]
fn options_compressed_payloads_and_descriptors_have_independent_padding_slots() {
    assert_eq!(SOURCE_TIM_BYTE_COUNT, 0x8040);
    assert_eq!(ITEM_SLOT_SIZE, 0x8800);
    assert_eq!(PADDING_BYTE_COUNT, 0xfc0);
    assert_eq!(COMPRESSED_PAYLOAD_SLOT_INDICES, [0, 1, 4]);
    assert_eq!(DESCRIPTOR_SLOT_INDEX, 3);
    assert_eq!(DESCRIPTOR_OFFSET, 0x21040);
    assert_eq!(SCRATCH_OFFSET, 0x2a800);
    assert!(
        COMPRESSED_PAYLOAD_SLOT_INDICES
            .iter()
            .all(|slot| *slot != DESCRIPTOR_SLOT_INDEX)
    );
}

#[test]
fn options_payload_slots_avoid_the_decompressor_ring_buffer_window() {
    assert!(!runtime_decompressor_uses_ring_buffer_window(0x800d_c7de));
    assert!(!runtime_decompressor_uses_ring_buffer_window(0x800e_4fe2));
    assert!(!runtime_decompressor_uses_ring_buffer_window(0x800f_5b98));
    assert!(runtime_decompressor_uses_ring_buffer_window(0x800e_d7e2));
}

#[test]
fn mixed_size_records_entry_and_restore_fit_the_verified_menu_padding() {
    let mut payload_sizes = vec![800, 800];
    payload_sizes.extend([200; REPRESENTATIVE_RECORDS_ENTRY_COUNT - 2]);
    let entry_count = payload_sizes.len();
    let layout = allocate_storage(&payload_sizes, &payload_sizes).unwrap();

    assert_eq!(RECORDS_STORAGE_SLOT_COUNT, 14);
    assert_eq!(RECORDS_FIRST_STORAGE_START, 0x85040);
    assert!(layout.exit_restore_descriptor_offset > layout.entry_descriptor_offset);
    assert_eq!(layout.entry_payload_offsets.len(), entry_count);
    assert_eq!(layout.exit_restore_payload_offsets.len(), entry_count);
    assert!(
        layout
            .exit_restore_payload_offsets
            .last()
            .is_some_and(|offset| *offset + 200 <= RECORDS_STORAGE_END)
    );
    assert!(
        layout
            .data_write_ranges
            .iter()
            .any(|[start, _]| *start >= 0x8f040),
        "the full Records population must consume padding beyond the old five-slot allocation"
    );
    for [start, end] in &layout.data_write_ranges {
        assert!(*start < *end && *end <= RECORDS_STORAGE_END);
        assert!((*start..*end).all(|offset| {
            if offset < 0x85040 {
                return false;
            }
            let slot_offset = (offset - 0x83800) % 0x2000;
            slot_offset >= 0x1840
        }));
    }

    let oversized = vec![1_900; RECORDS_STORAGE_SLOT_COUNT];
    assert!(allocate_storage(&oversized, &oversized).is_err());
}
