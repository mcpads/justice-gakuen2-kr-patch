use std::path::Path;

use psx_r3000a::{Instruction, Register};

use crate::cue::CueSheet;
use crate::disc::rebuild::read_record;
use crate::name_input::{
    NicknameHudGlyphStyle, build_default_name_glyph_pack, load_name_input_keyboard,
    plan_name_glyph_consumer_layout, plan_name_input_runtime_pack, plan_nickname_hud_glyph_layout,
};
use crate::pipeline::sha256_bytes;
use crate::source_disc::MAIN_EXECUTABLE_PATH;

use super::dialogue_name_runtime_build::build_dialogue_name_runtime_hook;
use super::script_source::{MGAME_PATH, MGAME_SIZE};
use super::shared_name_runtime_build::build_shared_name_runtime;

#[test]
#[ignore = "requires assets/, fonts in ../fonts/ and the original disc in roms/"]
fn direct_and_relationship_names_install_the_shared_runtime_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, source) = read_record(&cue.image_path, MGAME_PATH).unwrap();
    let (_, main_executable) = read_record(&cue.image_path, MAIN_EXECUTABLE_PATH).unwrap();
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let pack = build_default_name_glyph_pack(
        &root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf"),
    )
    .unwrap();
    let runtime_pack = plan_name_input_runtime_pack(&pack.report).unwrap();
    let nickname_hud_layout = plan_nickname_hud_glyph_layout(
        &runtime_pack,
        NicknameHudGlyphStyle {
            scale_percent: 115,
            vertical_shift_px: -1,
        },
    )
    .unwrap();
    let shared = build_shared_name_runtime(
        &main_executable,
        &layout,
        &crate::name_input::NameGlyphMaterializationBundle::from_pack(&pack),
        &nickname_hud_layout,
    )
    .unwrap();

    let mut plan = crate::decoded_record_write_plan::DecodedRecordWritePlan::new(
        MAIN_EXECUTABLE_PATH,
        &main_executable,
        crate::source_disc::MAIN_EXECUTABLE_SHA256,
    )
    .unwrap();
    let mut machine = crate::psx_machine_code_sources::PsxMachineCodeSources::default();
    super::shared_name_runtime_build::register_shared_name_runtime_candidate(
        &shared,
        &mut plan,
        &mut machine,
    )
    .unwrap();
    assert_eq!(plan.apply(Some(&machine)).unwrap(), shared.bytes);

    let build = build_dialogue_name_runtime_hook(
        &source,
        &shared.dialogue_program,
        MAIN_EXECUTABLE_PATH,
        super::backup_slot_text::BackupSlotTextCounts {
            empty: 6,
            error: 6,
            clear: 4,
            translated_empty: true,
            translated_error: true,
        },
        super::stat_result_layout::source_layout(),
    )
    .unwrap();

    assert_eq!(build.bytes.len(), MGAME_SIZE);
    for (offset, capacity) in [(0x2638, 6), (0x2644, 9)] {
        let positions: Vec<_> = build.bytes[offset..offset + capacity * 2]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(
            positions,
            (0..capacity as u16).map(|i| i * 20).collect::<Vec<_>>()
        );
    }
    // Every field must reach tagged-name materialization. A raw nickname
    // pointer makes the native loader multiply a Hangul tag by 200 as an atlas
    // index; that path stalls the GPU after new-game face selection.
    assert_eq!(&build.bytes[0x1c44..0x1c74], &source[0x1c44..0x1c74]);
    let table = 0x80010b80u32;
    let file = (table - 0x80010000) as usize + 0x800;
    let words = shared.bytes[file..file + 20]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    assert_eq!(words, [0x30012002, 0x30012000, 0x30012001, 0, 0]);
    assert_eq!(
        psx_r3000a::decode(
            u32::from_le_bytes(build.bytes[0x13d50..0x13d54].try_into().unwrap()),
            0x800b5d50
        )
        .unwrap(),
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::AT,
            immediate: table as i16
        }
    );
    for (field, control) in [(0u8, 0x2002u16), (1, 0x2000), (2, 0x2001)] {
        let mut memory = vec![0u8; 0x200000];
        memory[0xa2000..0xa2000 + build.bytes.len()].copy_from_slice(&build.bytes);
        memory[0x10b80..0x10b94].copy_from_slice(&shared.bytes[file..file + 20]);
        memory[0x1c0003] = field;
        // The reproduced nickname starts with a valid but large Hangul tag.
        memory[0x1f1896..0x1f189a].copy_from_slice(&[0x4a, 0x9b, 0x01, 0x30]);
        let mut registers = [0u32; 32];
        registers[4] = 0x801c0000;
        registers[29] = 0x801fe000;
        registers[31] = 0x80010000;
        let mut calls = 0;
        crate::name_input::runtime_test_machine::execute_with_callbacks(
            &build.bytes[0x13d34..0x13d78],
            0x800b5d34,
            &mut registers,
            &mut memory,
            None,
            &mut |pc, r, m| {
                if pc != 0x800af578 {
                    return false;
                }
                calls += 1;
                let record = (r[5] & 0x1fff_ffff) as usize;
                assert_eq!(
                    u16::from_le_bytes(m[record..record + 2].try_into().unwrap()),
                    control
                );
                assert_eq!(&m[record + 2..record + 4], &[1, 0x30]);
                assert_eq!(r[4], 0x800d0000);
                let geometry = 0x1c44 + usize::from(field) * 4;
                assert_eq!(
                    r[6],
                    u32::from_le_bytes(source[geometry..geometry + 4].try_into().unwrap())
                );
                true
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(registers[29], 0x801fe000);
        assert_eq!(registers[31], 0x80010000);
    }
    assert_ne!(sha256_bytes(&build.bytes), sha256_bytes(&source));
    assert_eq!(
        build
            .report
            .name_consumer_hooks
            .clone()
            .map(|hook| hook.address),
        ["0x800af750", "0x800af8dc"]
    );
    assert!(
        build
            .report
            .name_consumer_hooks
            .iter()
            .all(|hook| hook.source_verified && hook.installed)
    );
    assert_eq!(
        build
            .report
            .relationship_name_field_capture
            .sites
            .each_ref()
            .map(|site| site.address.as_str()),
        [
            "0x800afbc4",
            "0x800afc34",
            "0x800afc9c",
            "0x800afd18",
            "0x800afd94",
            "0x800afdf0",
            "0x800afe58",
        ]
    );
    assert_eq!(
        build.report.relationship_name_field_capture.sites[0].replacement_instruction,
        format!(
            "{:?}",
            Instruction::Ori {
                rt: Register::T9,
                rs: Register::ZERO,
                immediate: 1,
            }
        )
    );
    assert_eq!(
        build.report.relationship_name_field_capture.sites[1].replacement_instruction,
        format!(
            "{:?}",
            Instruction::Addu {
                rd: Register::T9,
                rs: Register::V1,
                rt: Register::ZERO,
            }
        )
    );
    assert!(build.report.relationship_name_field_capture.source_verified);
    assert!(build.report.relationship_name_field_capture.installed);
    assert!(
        build
            .report
            .relationship_name_field_capture
            .given_name_is_the_default_category
    );
    assert!(build.report.mgame_reload_entry_hook.source_verified);
    assert!(build.report.mgame_reload_entry_hook.installed);
    assert!(build.report.nickname_hud_render_hook.source_verified);
    assert!(build.report.nickname_hud_render_hook.installed);
    assert!(
        build
            .report
            .nickname_hud_render_hook
            .native_loader_called_by_wrapper
    );
    assert!(
        build
            .report
            .nickname_hud_render_hook
            .returns_to_native_renderer_continuation
    );
    assert!(
        build
            .report
            .nickname_hud_render_hook
            .uploads_after_each_native_texture_load
    );
    assert!(
        build
            .report
            .nickname_hud_render_hook
            .caller_return_address_preserved
    );
    assert_eq!(
        build.report.nickname_hud_render_hook.wrapper_address,
        build.report.program.nickname_hud_render_wrapper_address
    );
    assert!(
        build
            .report
            .mgame_reload_entry_hook
            .source_entry_pointer_preserved
    );
    assert!(
        build
            .report
            .mgame_reload_entry_hook
            .selects_valid_source_copy_or_destination_repair_before_entry_continuation
    );
    assert!(
        build
            .report
            .mgame_reload_entry_hook
            .displaced_instructions_preserved_by_wrapper
    );
    assert!(
        build
            .report
            .mgame_reload_entry_hook
            .caller_return_address_preserved
    );
    assert!(build.report.nickname_hud_lookup.source_entries_verified);
    assert_eq!(
        build.report.nickname_hud_lookup.cache_codes,
        ["0x0339", "0x033a", "0x033b", "0x033c"]
    );
    assert_eq!(
        build.report.nickname_hud_lookup.glyph_indices,
        ["0x0203", "0x0204", "0x0205", "0x0207"]
    );
    assert!(build.report.program.installed);
    assert_eq!(
        build.report.runtime_bootstrap_owner_path,
        MAIN_EXECUTABLE_PATH
    );
    assert_eq!(build.report.runtime_repair_owner_path, MAIN_EXECUTABLE_PATH);
    assert_eq!(build.report.runtime_reload_source_owner_path, MGAME_PATH);
    assert_eq!(
        build.report.program_reload_byte_range,
        [0x0002_8d94, MGAME_SIZE]
    );
    assert_eq!(
        build.report.program_reload_source_address_range,
        ["0x800cad94", "0x800cb6a4"]
    );
    assert_eq!(
        build.report.program_reload_installed_byte_count,
        shared.dialogue_program.bytes.len()
    );
    assert!(build.report.program_reload_installed);
    assert_eq!(
        &build.bytes[0x0002_8d94..0x0002_8d94 + shared.dialogue_program.bytes.len()],
        shared.dialogue_program.bytes.as_slice()
    );
    assert_eq!(
        build.report.program.relationship_name_buffer_address,
        "0x801f18a6"
    );
    assert_eq!(
        build
            .report
            .program
            .relationship_name_field_category_register,
        "t9"
    );
    assert_eq!(
        build.report.program.relationship_name_cache_slot_offsets,
        [0, 6, 12]
    );
    assert_eq!(
        build.report.program.relationship_name_record_cell_capacity,
        8
    );
    assert_eq!(
        build.report.mgame_reload_entry_hook.wrapper_address,
        "0x80010cac"
    );
    assert_eq!(
        &build.bytes[..4],
        &source[..4],
        "the loader-owned MGAME entry pointer must remain unchanged"
    );
    assert!(!build.report.runtime_execution_verified);
}
