use psx_r3000a::{Instruction, Register, decode, encode};

use super::assets::validate_asset;
use super::build::apply_overlay_patch;
use super::model::BonusPageIndicatorAsset;
use super::overlay::{
    CURRENT_PAGE_LOOKUP_OFFSET, OUTPUT_SUFFIX_SPRITE_COUNT, OVERLAY_RUNTIME_BASE,
    SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET, SUFFIX_TABLE_OFFSET, TOTAL_PAGE_LOOKUP_OFFSET,
    patch_page_indicator_overlay, source_consumer_instructions,
};
use super::source::pack_indexed_pixels;

#[test]
fn page_suffix_patch_keeps_dynamic_digits_and_disables_only_the_japanese_tail() {
    let source = overlay_fixture();

    let patched = patch_page_indicator_overlay(&source).unwrap();

    assert_eq!(
        &patched.bytes[CURRENT_PAGE_LOOKUP_OFFSET..CURRENT_PAGE_LOOKUP_OFFSET + 9],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(patched.bytes[TOTAL_PAGE_LOOKUP_OFFSET], 3);
    assert_eq!(
        &patched.bytes[SUFFIX_TABLE_OFFSET..SUFFIX_TABLE_OFFSET + 8],
        &[1, 8, 9, 5, 3, 9, 10, 7]
    );
    assert_eq!(
        decode_instruction(&patched.bytes, SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET),
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::S0,
            immediate: OUTPUT_SUFFIX_SPRITE_COUNT as i16,
        }
    );
    assert_eq!(
        patched.expected_write_ranges,
        vec![[
            SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
            SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET + 4
        ]]
    );
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET <= *start
            && *end <= SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET + 4
    }));
}

#[test]
fn rejects_a_changed_dynamic_current_page_lookup() {
    let mut source = overlay_fixture();
    write_instruction(&mut source, 0x8758, Instruction::nop());

    let error = patch_page_indicator_overlay(&source).unwrap_err();

    assert!(error.to_string().contains("consumer changed at +0x8758"));
}

#[test]
fn rejects_a_second_consumer_of_the_repurposed_suffix_glyph() {
    let mut source = overlay_fixture();
    source[0x1000..0x1003].copy_from_slice(&[9, 10, 7]);

    let error = patch_page_indicator_overlay(&source).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("exclusive to the page-indicator")
    );
}

#[test]
fn overlay_composition_preserves_a_disjoint_writer() {
    let mut overlay = overlay_fixture();
    overlay[0x1000] = 0x5a;

    let ranges = apply_overlay_patch(&mut overlay).unwrap();

    assert_eq!(overlay[0x1000], 0x5a);
    assert_eq!(
        ranges,
        [[
            SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET,
            SUFFIX_LOOP_BOUND_INSTRUCTION_OFFSET + 4
        ]]
    );
}

#[test]
#[ignore = "requires assets/"]
fn tracked_suffix_is_one_authored_hangul_syllable() {
    let document =
        crate::test_input::read_str("assets/menu/bonus-inventory/dynamic/page-indicator.json");
    let mut asset: BonusPageIndicatorAsset = serde_json::from_str(document).unwrap();
    validate_asset(&asset).unwrap();

    asset.korean_text = Some("페이지".to_string());
    let error = validate_asset(&asset).unwrap_err();

    assert!(error.to_string().contains("exactly one Korean glyph"));
}

#[test]
fn packs_page_suffix_pixels_in_psx_low_nibble_first_order() {
    let packed = pack_indexed_pixels(&[1, 15, 3, 8]).unwrap();

    assert_eq!(packed, [0xf1, 0x83]);
}

fn overlay_fixture() -> Vec<u8> {
    let instructions = source_consumer_instructions();
    let mut overlay = vec![
        0;
        instructions
            .iter()
            .map(|(offset, _)| offset + 4)
            .max()
            .unwrap()
    ];
    overlay[CURRENT_PAGE_LOOKUP_OFFSET..CURRENT_PAGE_LOOKUP_OFFSET + 9]
        .copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8]);
    overlay[SUFFIX_TABLE_OFFSET..SUFFIX_TABLE_OFFSET + 16]
        .copy_from_slice(&[1, 8, 9, 5, 3, 9, 10, 7, 4, 8, 4, 6, 5, 9, 5, 6]);
    for (offset, instruction) in instructions {
        write_instruction(&mut overlay, offset, instruction);
    }
    overlay
}

fn write_instruction(overlay: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    overlay[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn decode_instruction(overlay: &[u8], offset: usize) -> Instruction {
    let word = u32::from_le_bytes(overlay[offset..offset + 4].try_into().unwrap());
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32).unwrap()
}
