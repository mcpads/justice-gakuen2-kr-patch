use std::collections::BTreeSet;

use super::runtime_insertions::{DIALOGUE_CONTROL_SPECS, dialogue_control_spec};
use super::tokens::{DialogueTokenKind, tokenize_dialogue_message};

#[test]
fn control_spec_codes_and_names_are_unique() {
    let codes: BTreeSet<_> = DIALOGUE_CONTROL_SPECS
        .iter()
        .map(|spec| spec.code)
        .collect();
    let names: BTreeSet<_> = DIALOGUE_CONTROL_SPECS
        .iter()
        .map(|spec| spec.semantic_name)
        .collect();

    assert_eq!(codes.len(), DIALOGUE_CONTROL_SPECS.len());
    assert_eq!(names.len(), DIALOGUE_CONTROL_SPECS.len());
    assert_eq!(
        dialogue_control_spec(0x200a).unwrap().semantic_name,
        "current_month"
    );
}

#[test]
fn school_name_insertion_consumes_its_reserved_argument() {
    let message = tokenize_dialogue_message(&[0x2009, 0xabcd, 0x3001], 0x02ea, 0x03e9).unwrap();

    assert_eq!(
        message.tokens[0].kind,
        DialogueTokenKind::ParameterizedRuntimeInsertion
    );
    assert_eq!(message.tokens[0].arguments, [0xabcd]);
    assert_eq!(message.tokens[1].kind, DialogueTokenKind::MessageEnd);
}
