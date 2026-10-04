use super::{
    EMPTY_NAME_SLOT, NameField, NameRecordSlotCursor, NameRecordSlotSelection, NameSlotCode,
};

#[test]
fn selection_overwrites_the_current_slot_then_advances() {
    let mut cursor = NameRecordSlotCursor::empty(NameField::FamilyName);

    assert_eq!(
        cursor.select(NameSlotCode::Ascii('A')).unwrap(),
        NameRecordSlotSelection::AdvancedToSlot(1)
    );
    assert_eq!(cursor.slot(), 1);
    assert_eq!(
        cursor.words()[0],
        NameSlotCode::Ascii('A').encode().unwrap()
    );
    assert_eq!(cursor.words()[1], EMPTY_NAME_SLOT);
}

#[test]
fn final_slot_stays_selected_and_redirects_to_confirm() {
    let mut cursor = NameRecordSlotCursor::empty(NameField::Nickname);
    for character in ['A', 'B', 'C'] {
        cursor.select(NameSlotCode::Ascii(character)).unwrap();
    }

    assert_eq!(
        cursor.select(NameSlotCode::Ascii('D')).unwrap(),
        NameRecordSlotSelection::RedirectedToConfirm
    );
    assert_eq!(cursor.slot(), 3);
}

#[test]
fn delete_clears_the_current_slot_before_moving_left() {
    let mut cursor = NameRecordSlotCursor::empty(NameField::FamilyName);
    for character in ['A', 'B', 'C', 'D', 'E', 'F'] {
        cursor.select(NameSlotCode::Ascii(character)).unwrap();
    }

    assert_eq!(cursor.delete(), 4);
    assert_eq!(cursor.words()[5], EMPTY_NAME_SLOT);
    assert_eq!(
        cursor.words()[4],
        NameSlotCode::Ascii('E').encode().unwrap()
    );
    assert_eq!(cursor.delete(), 3);
    assert_eq!(cursor.words()[4], EMPTY_NAME_SLOT);
}

#[test]
fn advance_moves_the_cursor_without_writing_a_slot() {
    let mut cursor = NameRecordSlotCursor::empty(NameField::GivenName);

    assert_eq!(cursor.advance(), 1);
    assert!(cursor.words().iter().all(|word| *word == EMPTY_NAME_SLOT));
    cursor.select(NameSlotCode::Ascii('Z')).unwrap();
    assert_eq!(cursor.words()[0], EMPTY_NAME_SLOT);
    assert_eq!(
        cursor.words()[1],
        NameSlotCode::Ascii('Z').encode().unwrap()
    );
}

#[test]
fn selection_after_moving_left_replaces_the_existing_slot() {
    let words = ['A', 'B', 'C', 'D', 'E', 'F']
        .map(|character| NameSlotCode::Ascii(character).encode().unwrap());
    let mut cursor = NameRecordSlotCursor::from_words(NameField::FamilyName, 4, &words).unwrap();

    cursor
        .select(NameSlotCode::Ascii('X'))
        .expect("replacement fits before the final slot");

    assert_eq!(
        cursor.words()[4],
        NameSlotCode::Ascii('X').encode().unwrap()
    );
    assert_eq!(
        cursor.words()[5],
        NameSlotCode::Ascii('F').encode().unwrap()
    );
    assert_eq!(cursor.slot(), 5);
}
