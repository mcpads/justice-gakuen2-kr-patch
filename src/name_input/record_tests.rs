use super::{
    ASCII_NAME_TAG, CanonicalNameRecord, EMPTY_NAME_SLOT, HANGUL_NAME_TAG, NameField, NameSlotCode,
};

#[test]
fn mixed_name_roundtrips_in_the_original_six_slot_shape() {
    let record = CanonicalNameRecord::from_text(NameField::GivenName, "한A2@").unwrap();

    let words = record.to_words().unwrap();
    let decoded = CanonicalNameRecord::from_words(NameField::GivenName, &words).unwrap();

    assert_eq!(words.len(), 6);
    assert_eq!(words[0], HANGUL_NAME_TAG | 10_588);
    assert_eq!(words[1], ASCII_NAME_TAG | u16::from(b'A'));
    assert_eq!(words[2], ASCII_NAME_TAG | u16::from(b'2'));
    assert_eq!(words[3], ASCII_NAME_TAG | u16::from(b'@'));
    assert_eq!(words[4..], [EMPTY_NAME_SLOT, EMPTY_NAME_SLOT]);
    assert_eq!(decoded, record);
}

#[test]
fn nickname_keeps_four_slots_and_rejects_a_fifth_character() {
    let record = CanonicalNameRecord::from_text(NameField::Nickname, "가나다라").unwrap();

    assert_eq!(record.to_words().unwrap().len(), 4);
    assert!(CanonicalNameRecord::from_text(NameField::Nickname, "가나다라마").is_err());
}

#[test]
fn legacy_low_codes_remain_distinguishable_from_tagged_names() {
    let words = [
        0x000b,
        0x0025,
        EMPTY_NAME_SLOT,
        EMPTY_NAME_SLOT,
        EMPTY_NAME_SLOT,
        EMPTY_NAME_SLOT,
    ];

    let record = CanonicalNameRecord::from_words(NameField::FamilyName, &words).unwrap();

    assert_eq!(
        record.slots(),
        &[
            NameSlotCode::LegacyGlyph(0x000b),
            NameSlotCode::LegacyGlyph(0x0025)
        ]
    );
    assert_eq!(record.to_words().unwrap(), words);
}

#[test]
fn unknown_tags_and_data_after_empty_slot_are_rejected() {
    let unknown_tag = [0xc000, EMPTY_NAME_SLOT, EMPTY_NAME_SLOT, EMPTY_NAME_SLOT];
    let gap = [
        ASCII_NAME_TAG | u16::from(b'A'),
        EMPTY_NAME_SLOT,
        ASCII_NAME_TAG | u16::from(b'B'),
        EMPTY_NAME_SLOT,
    ];

    assert!(CanonicalNameRecord::from_words(NameField::Nickname, &unknown_tag).is_err());
    assert!(CanonicalNameRecord::from_words(NameField::Nickname, &gap).is_err());
}

#[test]
fn characters_outside_the_keyboard_are_rejected() {
    assert!(CanonicalNameRecord::from_text(NameField::GivenName, "あ").is_err());
    assert!(CanonicalNameRecord::from_text(NameField::GivenName, "é").is_err());
    assert!(CanonicalNameRecord::from_text(NameField::GivenName, "갂").is_err());
}

#[test]
fn composition_predecessors_have_canonical_name_roundtrips() {
    for c in "뢔쌰쎼쓔쬬".chars() {
        let code = super::NameSlotCode::Hangul(c);
        assert_eq!(
            super::NameSlotCode::decode(code.encode().unwrap()).unwrap(),
            code
        );
    }
}
