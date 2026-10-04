use super::glyph_slots::FIXED_RECORD_SPACE_CODE;
use super::overlay::patch_confirmation_overlay;
use super::records::{
    ALTERNATE_PROMPT_RECORD_LENGTH, ALTERNATE_PROMPT_RECORD_OFFSET, EXIT_PROMPT_RECORD_LENGTH,
    EXIT_PROMPT_RECORD_OFFSET, SHARED_CHOICE_RECORD_LENGTH, SHARED_CHOICE_RECORD_OFFSET,
};
use super::test_support::{authored_units, overlay_fixture};
use super::text_units::TEXT_UNIT_SPECS;

#[test]
#[ignore = "requires assets/"]
fn counted_prompts_and_shared_choices_use_component_owned_glyphs() {
    let patched = patch_confirmation_overlay(&overlay_fixture(), &authored_units()).unwrap();

    assert_eq!(
        &patched.bytes
            [EXIT_PROMPT_RECORD_OFFSET..EXIT_PROMPT_RECORD_OFFSET + EXIT_PROMPT_RECORD_LENGTH],
        &[
            6, 3, 1, 3, 3, 2, 3, 3, 3, 3, 3, 4, 3, 3, 5, 3, 0, 5, 5, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(
        &patched.bytes[ALTERNATE_PROMPT_RECORD_OFFSET
            ..ALTERNATE_PROMPT_RECORD_OFFSET + ALTERNATE_PROMPT_RECORD_LENGTH],
        &[
            10, 3, 1, 5, 3, 11, 5, 3, 2, 5, 3, 3, 5, 3, 4, 5, 3, 11, 5, 3, 3, 3, 3, 4, 3, 3, 5, 3,
            0, 5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(FIXED_RECORD_SPACE_CODE, 0x035b);
    assert_eq!(
        &patched.bytes[SHARED_CHOICE_RECORD_OFFSET
            ..SHARED_CHOICE_RECORD_OFFSET + SHARED_CHOICE_RECORD_LENGTH],
        &[3, 6, 3, 3, 7, 3, 3, 8, 3, 3, 5, 3, 0, 0, 0, 0]
    );
}

#[test]
#[ignore = "requires assets/"]
fn all_six_meanings_remain_independent_authored_units() {
    let source = overlay_fixture();
    let units = authored_units();

    for (unit, spec) in units.iter().zip(TEXT_UNIT_SPECS) {
        validate_unit(unit, spec, &source).unwrap();
    }
    assert_eq!(
        units
            .iter()
            .map(|unit| unit.korean_text.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "종료할까요?",
            "이 카드로 할까요?",
            "이 메모리 카드에",
            "복사할까요?",
            "예",
            "아니요",
        ]
    );
}
use super::assets::validate_unit;
