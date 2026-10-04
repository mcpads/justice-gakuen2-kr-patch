use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use psx_r3000a::{Instruction, Register, verify_placed_program};

use crate::psx_machine_code_sources::verify_r3000a_load_delays;

use super::{
    DIGIT_KEYS, LATIN_KEYS, MGAME_RELOAD_WRAPPER_BYTE_CAPACITY, MGAME_RELOAD_WRAPPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
    NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN, NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY,
    NAME_DIALOGUE_RUNTIME_ORIGIN, NicknameHudGlyphStyle, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
    SYMBOL_KEYS, build_name_dialogue_runtime_bootstrap_program,
    build_name_dialogue_runtime_program, build_shared_name_outline_runtime_program,
    load_name_input_keyboard, plan_name_glyph_consumer_layout, plan_name_input_runtime_pack,
    plan_nickname_hud_glyph_layout,
};

fn tracked_program() -> super::NameDialogueRuntimeProgram {
    program_for_font(&maplestory_light(), 13.0)
}

fn program_for_font(font: &Path, size: f32) -> super::NameDialogueRuntimeProgram {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let pack =
        super::build_name_glyph_pack(font, size, super::NAME_GLYPH_PACK_STORAGE_BYTES).unwrap();
    let runtime_pack = plan_name_input_runtime_pack(&pack.report).unwrap();
    let nickname_hud_layout = plan_nickname_hud_glyph_layout(
        &runtime_pack,
        NicknameHudGlyphStyle {
            scale_percent: 115,
            vertical_shift_px: -1,
        },
    )
    .unwrap();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    build_name_dialogue_runtime_program(&layout, &runtime_pack, &outline, &nickname_hud_layout)
        .unwrap()
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn shared_name_decoder_addresses_follow_the_selected_font_pack() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, size) in [
        (maplestory_light(), 13.0),
        (root.join("../fonts/galmuri/Galmuri11.ttf"), 12.0),
    ] {
        let pack = super::build_name_glyph_pack(&path, size, super::NAME_GLYPH_PACK_STORAGE_BYTES)
            .unwrap();
        let layout = plan_name_input_runtime_pack(&pack.report).unwrap();
        let program = program_for_font(&path, size);
        assert!(program.instructions.contains(&Instruction::Addiu {
            rt: Register::A0,
            rs: Register::T0,
            immediate: i16::try_from(layout.repertoire_membership_byte_range[0]).unwrap(),
        }));
        assert!(program.instructions.contains(&Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: i16::try_from(layout.component_mask_byte_range[0]).unwrap(),
        }));
        assert!(program.instructions.contains(&Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: u16::try_from(layout.bytes_per_component_mask).unwrap(),
        }));
        assert!(program.instructions.contains(&Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T7,
            immediate: i16::try_from(layout.runtime_coordinate_list_byte_count).unwrap(),
        }));
        verify_r3000a_load_delays(
            &program.instructions,
            NAME_DIALOGUE_RUNTIME_ORIGIN + program.instruction_offset as u32,
            "selected font pack",
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn shared_dialogue_name_runtime_fits_its_execution_and_repair_regions() {
    let program = tracked_program();
    let instructions = verify_placed_program(
        &program.bytes[program.instruction_offset..],
        NAME_DIALOGUE_RUNTIME_ORIGIN + program.instruction_offset as u32,
    )
    .unwrap();

    assert!(program.bytes.len() <= NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY);
    assert_eq!(NAME_DIALOGUE_RUNTIME_ORIGIN, 0x8009_a400);
    assert_eq!(NAME_DIALOGUE_RUNTIME_EXECUTION_BYTE_CAPACITY, 0x0900);
    assert_eq!(MGAME_RUNTIME_REPAIR_HELPER_ORIGIN, 0x8001_0b94);
    assert_eq!(MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY, 0x0060);
    assert_eq!(MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN, 0x8001_0ad8);
    assert_eq!(MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY, 0x0028);
    assert!(
        program.mgame_runtime_repair.helper_bytes.len()
            <= MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY
    );
    assert!(
        program.mgame_runtime_repair.literal_bytes.len()
            <= MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY
    );
    assert!(
        program
            .report
            .mgame_runtime_repair
            .first_entry_copy_roundtrip_verified
    );
    assert!(
        program
            .report
            .mgame_runtime_repair
            .destination_repair_roundtrip_verified
    );
    assert_eq!(instructions.len(), program.report.typed_instruction_count);
    assert_eq!(program.report.byte_count, program.bytes.len());
    assert_eq!(program.report.shared_cache_code_range, ["0x032d", "0x033c"]);
    assert_eq!(
        program.report.relationship_name_buffer_address,
        "0x801f18a6"
    );
    assert_eq!(
        program.report.relationship_name_field_category_register,
        "t9"
    );
    assert_eq!(
        program.report.relationship_name_cache_slot_offsets,
        [0, 6, 12]
    );
    assert_eq!(program.report.relationship_name_record_cell_capacity, 8);
    assert_eq!(
        program.report.nickname_hud_atlas_descriptor_address,
        "0x800d0174"
    );
    assert_eq!(
        program.report.nickname_hud_name_record_address,
        "0x801f1896"
    );
    assert!(program.report.nickname_hud_lazy_materialization);
    assert_eq!(
        program.report.shared_outline_pixel_address,
        format!("0x{SHARED_NAME_OUTLINE_RUNTIME_ORIGIN:08x}")
    );
    assert!(
        instructions.contains(&Instruction::Jal {
            target: u32::from_str_radix(
                program
                    .report
                    .nickname_hud_store_address
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap(),
        })
    );
    assert_eq!(
        program.report.nickname_hud_reused_scene_cell_addresses,
        ["0x801663a0", "0x80166468", "0x80166530", "0x801666c0"]
    );
    assert!(!program.report.nickname_hud_writes_reused_scene_cells);
    assert_eq!(
        program.report.nickname_hud_glyph_layout.source_bounds,
        [3, 5, 14, 15]
    );
    assert_eq!(
        program.report.nickname_hud_glyph_layout.target_bounds,
        [2, 2, 16, 17]
    );
    assert_eq!(
        program.report.nickname_hud_vram_cell_rects,
        [
            [768, 492, 5, 20],
            [773, 492, 5, 20],
            [778, 492, 5, 20],
            [783, 492, 5, 20],
        ]
    );
    let scaler_index = usize::try_from(
        (program.nickname_hud_scaler_address
            - NAME_DIALOGUE_RUNTIME_ORIGIN
            - u32::try_from(program.instruction_offset).unwrap())
            / 4,
    )
    .unwrap();
    let consumer_index = usize::try_from(
        (program.name_consumer_address
            - NAME_DIALOGUE_RUNTIME_ORIGIN
            - u32::try_from(program.instruction_offset).unwrap())
            / 4,
    )
    .unwrap();
    let consumer = &instructions[consumer_index..scaler_index];
    assert!(consumer.windows(3).any(|window| {
        window
            == [
                Instruction::Sll {
                    rd: Register::S3,
                    rt: Register::T9,
                    shift: 1,
                },
                Instruction::Addu {
                    rd: Register::S3,
                    rs: Register::S3,
                    rt: Register::T9,
                },
                Instruction::Sll {
                    rd: Register::S3,
                    rt: Register::S3,
                    shift: 1,
                },
            ]
    }));
    assert!(consumer.contains(&Instruction::Ori {
        rt: Register::S4,
        rs: Register::ZERO,
        immediate: 8,
    }));
    assert!(consumer.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Beq {
                rs: Register::S5,
                rt: Register::ZERO,
                ..
            }
        )
    }));
    assert!(consumer.windows(2).any(|window| {
        matches!(
            window[0],
            Instruction::Bne {
                rs: Register::T0,
                rt: Register::ZERO,
                ..
            }
        ) && window[1]
            == Instruction::Sll {
                rd: Register::S3,
                rt: Register::T9,
                shift: 1,
            }
    }));
    assert_eq!(
        instructions[scaler_index],
        Instruction::Addu {
            rd: Register::T0,
            rs: Register::A2,
            rt: Register::ZERO,
        }
    );
    assert!(instructions[scaler_index..].contains(&Instruction::Divu {
        rs: Register::T4,
        rt: Register::T5,
    }));
    let uploader_index = usize::try_from(
        (program.nickname_hud_uploader_address
            - NAME_DIALOGUE_RUNTIME_ORIGIN
            - u32::try_from(program.instruction_offset).unwrap())
            / 4,
    )
    .unwrap();
    let render_wrapper_index = usize::try_from(
        (program.nickname_hud_render_wrapper_address
            - NAME_DIALOGUE_RUNTIME_ORIGIN
            - u32::try_from(program.instruction_offset).unwrap())
            / 4,
    )
    .unwrap();
    let uploader = &instructions[uploader_index..render_wrapper_index];
    assert!(uploader.contains(&Instruction::Lw {
        rt: Register::S4,
        base: Register::S4,
        offset: 336,
    }));
    assert!(uploader.contains(&Instruction::Jalr {
        rd: Register::RA,
        rs: Register::S4,
    }));
    assert!(uploader.contains(&Instruction::Sh {
        rt: Register::S1,
        base: Register::S3,
        offset: 12,
    }));
    assert!(program.report.nickname_hud_upload_after_native_texture_load);
    let render_wrapper = verify_placed_program(
        &program.nickname_hud_render_wrapper_bytes,
        program.nickname_hud_render_wrapper_address,
    )
    .unwrap();
    let render_wrapper_byte_offset =
        usize::try_from(program.nickname_hud_render_wrapper_address - NAME_DIALOGUE_RUNTIME_ORIGIN)
            .unwrap();
    assert_eq!(
        &program.bytes[render_wrapper_byte_offset
            ..render_wrapper_byte_offset + program.nickname_hud_render_wrapper_bytes.len()],
        program.nickname_hud_render_wrapper_bytes
    );
    assert_eq!(
        render_wrapper,
        program.nickname_hud_render_wrapper_instructions
    );
    let native_loader_call = render_wrapper
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jalr {
                    rd: Register::RA,
                    rs: Register::V0,
                }
        })
        .unwrap();
    assert_eq!(render_wrapper[native_loader_call + 1], Instruction::nop());
    assert!(matches!(
        render_wrapper[native_loader_call + 2],
        Instruction::Lui { .. }
    ));
    assert!(render_wrapper.iter().any(|i| matches!(
        i,
        Instruction::Bne {
            rs: Register::S2,
            rt: Register::T0,
            ..
        }
    )));
    let lazy_materialization_call = render_wrapper
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: program.name_consumer_address,
                }
        })
        .unwrap();
    assert_eq!(
        render_wrapper[lazy_materialization_call + 1],
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        }
    );
    let upload_call = render_wrapper
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: program.nickname_hud_uploader_address,
                }
        })
        .unwrap();
    assert!(native_loader_call < lazy_materialization_call);
    assert!(lazy_materialization_call < upload_call);
    let upload_address =
        program.nickname_hud_render_wrapper_address + u32::try_from(upload_call * 4).unwrap();
    assert!(render_wrapper.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Bne {
                rs: Register::T0,
                rt: Register::T1,
                target,
            } if *target > upload_address
        )
    }));
    assert_eq!(
        render_wrapper[render_wrapper.len() - 2],
        Instruction::Jr { rs: Register::RA }
    );

    let mgame_reload_wrapper = verify_placed_program(
        &program.mgame_reload_wrapper_bytes,
        program.mgame_reload_wrapper_address,
    )
    .unwrap();
    assert_eq!(
        program.mgame_reload_wrapper_address,
        MGAME_RELOAD_WRAPPER_ORIGIN
    );
    assert!(program.mgame_reload_wrapper_bytes.len() <= MGAME_RELOAD_WRAPPER_BYTE_CAPACITY);
    assert_eq!(
        mgame_reload_wrapper,
        program.mgame_reload_wrapper_instructions
    );
    assert!(mgame_reload_wrapper.contains(&Instruction::Jal {
        target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    }));
    assert_eq!(
        mgame_reload_wrapper
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Jal { .. }))
            .collect::<Vec<_>>(),
        [&Instruction::Jal {
            target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
        }]
    );
    assert!(mgame_reload_wrapper.contains(&Instruction::J {
        target: 0x800a_5ce4,
    }));
    assert!(mgame_reload_wrapper.contains(&Instruction::Lbu {
        rt: Register::V1,
        base: Register::V1,
        offset: 0x1801,
    }));
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::T8,
        rs: Register::ZERO,
        immediate: 10,
    }));
    assert!(instructions.contains(&Instruction::Jal {
        target: 0x800a_f9ac,
    }));
    assert!(instructions.contains(&Instruction::Andi {
        rt: Register::A0,
        rs: Register::T0,
        immediate: 0x3fff,
    }));
    assert!(instructions.contains(&Instruction::Ori {
        rt: Register::T1,
        rs: Register::ZERO,
        immediate: u16::MAX,
    }));
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn ascii_name_resolver_exposes_every_legacy_mapping_index_it_can_return() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let expected = LATIN_KEYS
        .chars()
        .chain(DIGIT_KEYS.chars())
        .chain(SYMBOL_KEYS.chars())
        .map(|character| layout.code_for_active_character(character).unwrap())
        .collect::<BTreeSet<_>>();

    let program = tracked_program();

    assert_eq!(program.ascii_legacy_indices, expected);
    assert_eq!(program.ascii_legacy_indices.len(), 77);
    assert!(
        program
            .ascii_legacy_indices
            .iter()
            .all(|index| *index < 0x061e)
    );
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn generated_shared_name_programs_observe_scalar_gpr_load_delays() {
    let program = tracked_program();
    let outline = build_shared_name_outline_runtime_program().unwrap();
    let bootstrap = build_name_dialogue_runtime_bootstrap_program(
        program.bytes.len(),
        program.nickname_hud_scaler_address,
        &program.mgame_runtime_repair,
    )
    .unwrap();

    verify_r3000a_load_delays(
        &program.instructions,
        NAME_DIALOGUE_RUNTIME_ORIGIN + program.instruction_offset as u32,
        "dialogue runtime",
    )
    .unwrap();
    verify_r3000a_load_delays(
        &program.nickname_hud_render_wrapper_instructions,
        program.nickname_hud_render_wrapper_address,
        "nickname HUD render wrapper",
    )
    .unwrap();
    verify_r3000a_load_delays(
        &program.mgame_reload_wrapper_instructions,
        program.mgame_reload_wrapper_address,
        "MGAME reload wrapper",
    )
    .unwrap();
    verify_r3000a_load_delays(
        &outline.instructions,
        SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
        "outline runtime",
    )
    .unwrap();
    verify_r3000a_load_delays(
        &bootstrap.instructions,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        "runtime bootstrap and persistence",
    )
    .unwrap();
}

fn maplestory_light() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf")
}

// Every admitted direct key must reach the HUD pixels through both serialized
// forms. Family/given rendering must leave the live nickname cache untouched.
#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn direct_name_consumer_populates_hud_without_changing_saved_names() {
    use super::dialogue_runtime_bootstrap::{
        NICKNAME_HUD_MAGIC_ADDRESS, NICKNAME_HUD_PERSISTENT_CELL_ORIGIN,
    };
    use super::runtime_test_machine::execute_with_callbacks;
    use psx_r3000a::{Assembler, Instruction};
    let program = tracked_program();
    let bootstrap = build_name_dialogue_runtime_bootstrap_program(
        program.bytes.len(),
        program.nickname_hud_scaler_address,
        &program.mgame_runtime_repair,
    )
    .unwrap();
    let origin = NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN - 8;
    let mut code = vec![0; (NAME_DIALOGUE_RUNTIME_ORIGIN - origin) as usize + program.bytes.len()];
    let mut entry = Assembler::new();
    entry
        .emit(Instruction::J {
            target: program.name_consumer_address,
        })
        .emit(Instruction::nop());
    code[..8].copy_from_slice(entry.assemble(origin).unwrap().bytes());
    code[8..8 + bootstrap.bytes.len()].copy_from_slice(&bootstrap.bytes);
    code[(NAME_DIALOGUE_RUNTIME_ORIGIN - origin) as usize..].copy_from_slice(&program.bytes);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let [sx, sy, sw, sh] = program.report.nickname_hud_glyph_layout.source_bounds;
    let [tx, ty, tw, th] = program.report.nickname_hud_glyph_layout.target_bounds;
    let hud = (NICKNAME_HUD_PERSISTENT_CELL_ORIGIN & 0x1fff_ffff) as usize;
    for key in layout
        .active_glyphs
        .iter()
        .filter(|key| key.character.is_ascii())
    {
        let legacy = key.code;
        for tagged in [false, true] {
            for (record, capacity) in [(0x1f1866, 6), (0x1f1876, 6), (0x1f1896, 4)] {
                let mut memory = vec![0xee; 0x20_0000];
                memory[0x9a400..0x9a400 + program.bytes.len()].copy_from_slice(&program.bytes);
                let atlas = 0x0d0174;
                let cell = atlas + 12 + legacy as usize * 200;
                let pixels: Vec<u8> = (0..200)
                    .map(|i| ((i + legacy as usize) % 256) as u8)
                    .collect();
                memory[cell..cell + 200].copy_from_slice(&pixels);
                let input = if tagged {
                    0x4000 | u16::from(key.character.as_bytes()[0])
                } else {
                    legacy
                };
                // One excess cell proves the field capacity is enforced.
                for i in 0..=capacity {
                    memory[record + i * 2..record + i * 2 + 2]
                        .copy_from_slice(&input.to_le_bytes());
                }
                memory[record + (capacity + 1) * 2..record + (capacity + 2) * 2]
                    .copy_from_slice(&0x3001u16.to_le_bytes());
                let saved = memory[record..record + (capacity + 2) * 2].to_vec();
                memory[hud..hud + 800].fill(0xa5);
                let mut r = [0u32; 32];
                r[4] = 0x80000000 | atlas as u32;
                r[5] = 0x80000000 | record as u32;
                r[6] = 1;
                r[29] = 0x801df000;
                r[31] = 0x80010000;
                for (i, value) in r.iter_mut().enumerate().take(24).skip(16) {
                    *value = 0x11000000 + i as u32;
                }
                let before = r;
                let mut calls = 0;
                execute_with_callbacks(
                    &code,
                    origin,
                    &mut r,
                    &mut memory,
                    None,
                    &mut |pc, r, m| {
                        if pc != 0x800af9ac {
                            return false;
                        }
                        calls += 1;
                        let output = (r[5] & 0x1fff_ffff) as usize;
                        let words: Vec<_> = m[output..output + (capacity + 1) * 2]
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|x| u16::from_le_bytes(*x))
                            .collect();
                        assert_eq!(words, [vec![legacy; capacity], vec![0x3001]].concat());
                        for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                            r[reg] = 0xdead0000 + reg as u32;
                        }
                        let sp = (r[29] & 0x1fff_ffff) as usize;
                        m[sp..sp + 16].fill(0x99);
                        true
                    },
                );
                assert_eq!(calls, 1);
                assert_eq!(&r[16..24], &before[16..24]);
                assert_eq!(r[29], before[29]);
                assert_eq!(r[31], before[31]);
                assert_eq!(&memory[record..record + saved.len()], saved);
                assert_eq!(&memory[cell..cell + 200], pixels);
                if record == 0x1f1896 {
                    let magic = (NICKNAME_HUD_MAGIC_ADDRESS & 0x1fff_ffff) as usize;
                    assert_eq!(&memory[magic..magic + 4], b"NHUD");
                    let mut expected = [0u8; 200];
                    for y in ty..ty + th {
                        for x in tx..tx + tw {
                            let source_x = sx + (x - tx) * sw / tw;
                            let source_y = sy + (y - ty) * sh / th;
                            let value =
                                (pixels[source_y * 10 + source_x / 2] >> ((source_x % 2) * 4)) & 15;
                            expected[y * 10 + x / 2] |= value << ((x % 2) * 4);
                        }
                    }
                    for slot in 0..4 {
                        assert_eq!(
                            &memory[hud + slot * 200..hud + (slot + 1) * 200],
                            expected,
                            "key {} tagged {tagged}",
                            key.character
                        );
                    }
                } else {
                    assert_eq!(&memory[hud..hud + 800], &[0xa5; 800]);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn hud_texture_reload_rebuilds_changed_names_and_skips_profile_fields() {
    use super::dialogue_runtime_bootstrap::{
        NICKNAME_HUD_MAGIC_ADDRESS, NICKNAME_HUD_PERSISTENT_CELL_ORIGIN,
    };
    use super::runtime_test_machine::execute_with_callbacks;
    let program = tracked_program();
    let magic = (NICKNAME_HUD_MAGIC_ADDRESS & 0x1fff_ffff) as usize;
    let hud = (NICKNAME_HUD_PERSISTENT_CELL_ORIGIN & 0x1fff_ffff) as usize;
    let mut memory = vec![0u8; 0x20_0000];
    memory[magic..magic + 4].copy_from_slice(b"NHUD");
    memory[hud..hud + 800].fill(0x99);
    // Full direct name -> short Hangul name -> empty name, then a family load.
    for (name, record) in [
        (0x359u16, 0x801f1896u32),
        (0x8000, 0x801f1896),
        (0x3001, 0x801f1896),
        (0x8001, 0x801f1866),
    ] {
        memory[0x1f1896..0x1f1898].copy_from_slice(&name.to_le_bytes());
        let prior_hud = memory[hud..hud + 800].to_vec();
        let mut r = [0u32; 32];
        r[2] = 0x80062630;
        r[18] = record;
        r[29] = 0x801df000;
        r[31] = 0x80010000;
        let before = r;
        let mut calls = Vec::new();
        execute_with_callbacks(
            &program.nickname_hud_render_wrapper_bytes,
            program.nickname_hud_render_wrapper_address,
            &mut r,
            &mut memory,
            None,
            &mut |pc, r, m| {
                if ![
                    0x80062630,
                    program.name_consumer_address,
                    program.nickname_hud_uploader_address,
                ]
                .contains(&pc)
                {
                    return false;
                }
                calls.push(pc);
                if pc == 0x80062630 {
                    let sp = (r[29] & 0x1fff_ffff) as usize;
                    // Native callees may use all four outgoing argument slots.
                    m[sp..sp + 16].fill(0xa5);
                } else if pc == program.name_consumer_address {
                    assert_eq!(r[5], 0x801f1896);
                    assert_eq!(r[6], 0);
                    assert_eq!(&m[magic..magic + 4], &[0; 4]);
                    if name != 0x3001 {
                        m[hud..hud + 800].fill(name as u8);
                        m[magic..magic + 4].copy_from_slice(b"NHUD");
                    }
                } else if pc == program.nickname_hud_uploader_address {
                    assert_ne!(name, 0x3001);
                    assert_eq!(&m[hud..hud + 800], &[name as u8; 800]);
                } else {
                    panic!("unexpected external call {pc:08x}");
                }
                for reg in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                    r[reg] = 0xdead0000 + reg as u32;
                }
                true
            },
        );
        let expected = if record != 0x801f1896 {
            vec![0x80062630]
        } else if name == 0x3001 {
            vec![0x80062630, program.name_consumer_address]
        } else {
            vec![
                0x80062630,
                program.name_consumer_address,
                program.nickname_hud_uploader_address,
            ]
        };
        assert_eq!(calls, expected);
        if record != 0x801f1896 {
            assert_eq!(&memory[hud..hud + 800], prior_hud);
        }
        assert_eq!(&r[16..24], &before[16..24]);
        assert_eq!(r[29], before[29]);
        assert_eq!(r[31], before[31]);
    }
}
