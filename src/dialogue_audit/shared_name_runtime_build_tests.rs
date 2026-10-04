use std::path::Path;

use crate::cue::CueSheet;
use crate::disc::rebuild::read_record;
use crate::name_input::{
    MGAME_RELOAD_WRAPPER_ORIGIN, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
    NAME_DIALOGUE_RUNTIME_ORIGIN, NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN, NicknameHudGlyphStyle,
    SHARED_NAME_OUTLINE_RUNTIME_ORIGIN, build_name_glyph_band_pack, load_name_input_keyboard,
    plan_name_glyph_consumer_layout, plan_nickname_hud_glyph_layout_for_crop,
};
use crate::pipeline::sha256_bytes;
use crate::source_disc::{MAIN_EXECUTABLE_PATH, MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE};

use super::shared_name_runtime_build::build_shared_name_runtime;

#[test]
#[ignore = "requires assets/, fonts in ../fonts/ and the original disc in roms/"]
fn source_bound_main_executable_accepts_both_shared_name_runtimes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, source) = read_record(&cue.image_path, MAIN_EXECUTABLE_PATH).unwrap();
    let keyboard =
        load_name_input_keyboard(&root.join("assets/dialogue/name-entry/keyboard.json")).unwrap();
    let layout = plan_name_glyph_consumer_layout(&keyboard).unwrap();
    let pack = build_name_glyph_band_pack(
        &root.join("../fonts/galmuri/Galmuri11.ttf"),
        12.0,
        crate::name_input::NAME_GLYPH_PACK_STORAGE_BYTES,
    )
    .unwrap();
    let nickname_hud_layout = plan_nickname_hud_glyph_layout_for_crop(
        pack.pack.crop(),
        NicknameHudGlyphStyle {
            scale_percent: 115,
            vertical_shift_px: -1,
        },
    )
    .unwrap();

    let build = build_shared_name_runtime(
        &source,
        &layout,
        &crate::name_input::NameGlyphMaterializationBundle::from_bands(&pack.pack),
        &nickname_hud_layout,
    )
    .unwrap();

    assert_eq!(build.bytes.len(), source.len());
    // The previous outline placement replaced zero-valued live floor entries.
    assert_eq!(&build.bytes[0x7ca30..0x7d330], &source[0x7ca30..0x7d330]);
    assert!(
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN as usize + build.bootstrap_program.bytes.len()
            <= SHARED_NAME_OUTLINE_RUNTIME_ORIGIN as usize
    );
    assert!(
        SHARED_NAME_OUTLINE_RUNTIME_ORIGIN as usize + build.outline_program.bytes.len()
            <= MGAME_RELOAD_WRAPPER_ORIGIN as usize
    );
    assert_ne!(sha256_bytes(&build.bytes), sha256_bytes(&source));
    assert_eq!(
        build.outline_program.outline_pixel_address,
        SHARED_NAME_OUTLINE_RUNTIME_ORIGIN
    );
    assert_eq!(
        build.dialogue_program.report.origin,
        format!("0x{NAME_DIALOGUE_RUNTIME_ORIGIN:08x}")
    );
    assert_eq!(
        build.bootstrap_program.report.origin,
        format!("0x{NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN:08x}")
    );
    assert_eq!(
        build.bootstrap_program.report.source_address,
        format!("0x{NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN:08x}")
    );
    assert_eq!(
        build.bootstrap_program.report.destination_address,
        format!("0x{NAME_DIALOGUE_RUNTIME_ORIGIN:08x}")
    );
    assert_eq!(
        installed_runtime_bytes(
            &build.bytes,
            MGAME_RELOAD_WRAPPER_ORIGIN,
            build.dialogue_program.mgame_reload_wrapper_bytes.len(),
        ),
        build.dialogue_program.mgame_reload_wrapper_bytes
    );
    assert_eq!(
        installed_runtime_bytes(
            &build.bytes,
            MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
            build
                .dialogue_program
                .mgame_runtime_repair
                .helper_bytes
                .len(),
        ),
        build.dialogue_program.mgame_runtime_repair.helper_bytes
    );
    assert_eq!(
        installed_runtime_bytes(
            &build.bytes,
            MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
            build
                .dialogue_program
                .mgame_runtime_repair
                .literal_bytes
                .len(),
        ),
        build.dialogue_program.mgame_runtime_repair.literal_bytes
    );
    assert!(build.report.entry_hook.source_verified);
    assert!(build.report.entry_hook.installed);
    assert!(build.report.outline_program.installed);
    assert!(build.report.bootstrap_program.installed);
    assert!(build.report.dialogue_program.installed);
    assert!(build.report.dialogue_program.mgame_runtime_repair.installed);
    assert!(build.dialogue_program.mgame_runtime_repair.report.installed);
    assert!(!build.report.runtime_execution_verified);
}

fn installed_runtime_bytes(bytes: &[u8], address: u32, byte_count: usize) -> &[u8] {
    let text_offset = usize::try_from(address - MAIN_TEXT_RUNTIME_BASE).unwrap();
    let file_offset = PSX_EXE_HEADER_SIZE + text_offset;
    &bytes[file_offset..file_offset + byte_count]
}
