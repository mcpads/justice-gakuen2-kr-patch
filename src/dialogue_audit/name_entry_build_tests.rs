use super::name_entry::{
    OVERLAY_SHA256, PADDING_CODE, PAGE_SPECS, SELECTABLE_CODE_SEQUENCE_OFFSET,
};
use super::name_entry_build::{
    DialogueNameEntryCandidatePage, GLOBAL_NAME_CODE_COUNT, GLOBAL_NAME_CODE_START,
    LOCALIZED_NAME_CODE_COUNT, patch_name_entry_overlay, validate_candidate_pages,
};

const CANDIDATE_KIND: &str = "Justice Gakuen 2 non-release development name-entry candidate page";

#[test]
fn candidate_pages_preserve_decimal_prefix_and_bound_localized_codes() {
    let candidates = validate_candidate_pages(candidate_pages(), "manifest".to_string()).unwrap();

    assert_eq!(candidates.assignments.len(), GLOBAL_NAME_CODE_COUNT);
    assert_eq!(candidates.assignments[0].code, "0x0305");
    assert_eq!(candidates.assignments[83].code, "0x0358");
    assert_eq!(candidates.assignments[84].code, "0x0359");
    assert_eq!(candidates.assignments[166].character, "0");
    assert_eq!(candidates.assignments[166].code, "0x000a");
    assert!(candidates.assignments[166].preserve_source_glyph);
    assert_eq!(candidates.assignments[175].character, "9");
    assert_eq!(candidates.assignments[175].code, "0x0009");
    assert_eq!(candidates.assignments[176].code, "0x03ab");
    assert_eq!(candidates.assignments[227].code, "0x03de");
    assert_eq!(
        candidates
            .assignments
            .iter()
            .filter(|assignment| !assignment.preserve_source_glyph)
            .count(),
        LOCALIZED_NAME_CODE_COUNT
    );
}

#[test]
fn changed_source_decimal_candidate_is_rejected() {
    let candidates = validate_candidate_pages(candidate_pages(), "manifest".to_string()).unwrap();
    let mut source = name_entry_page_fixture();
    let alphanumeric_page_offset = PAGE_SPECS[2].1;
    source[alphanumeric_page_offset..alphanumeric_page_offset + 2]
        .copy_from_slice(&0x000bu16.to_le_bytes());

    let error = patch_name_entry_overlay(&source, &candidates).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("preserved name-entry source glyph code changed")
    );
}

#[test]
fn duplicate_name_candidate_is_rejected_across_pages() {
    let mut pages = candidate_pages();
    let duplicate = pages[0].1.characters.chars().next().unwrap();
    pages[1]
        .1
        .characters
        .replace_range(..duplicate.len_utf8(), &duplicate.to_string());

    let error = validate_candidate_pages(pages, "manifest".to_string()).unwrap_err();

    assert!(error.to_string().contains("duplicated"));
}

#[test]
fn localized_pages_preserve_padding_and_mirror_the_selectable_sequence() {
    let candidates = validate_candidate_pages(candidate_pages(), "manifest".to_string()).unwrap();
    let source = name_entry_page_fixture();

    let patched = patch_name_entry_overlay(&source, &candidates).unwrap();

    let mut assignment_index = 0usize;
    for (_, offset, cell_count, _) in PAGE_SPECS {
        for cell_index in 0..cell_count {
            let cell_offset = offset + cell_index * 2;
            let source_code = read_u16(&source, cell_offset);
            let patched_code = read_u16(&patched, cell_offset);
            if source_code == PADDING_CODE {
                assert_eq!(patched_code, PADDING_CODE);
                continue;
            }
            let assignment = &candidates.assignments[assignment_index];
            let expected = if assignment.preserve_source_glyph {
                source_code
            } else {
                GLOBAL_NAME_CODE_START
                    + candidates.assignments[..assignment_index]
                        .iter()
                        .filter(|previous| !previous.preserve_source_glyph)
                        .count() as u16
            };
            assert_eq!(patched_code, expected);
            assert_eq!(
                read_u16(
                    &patched,
                    SELECTABLE_CODE_SEQUENCE_OFFSET + assignment_index * 2
                ),
                expected
            );
            assignment_index += 1;
        }
    }
    assert_eq!(assignment_index, GLOBAL_NAME_CODE_COUNT);
}

fn candidate_pages() -> Vec<(String, DialogueNameEntryCandidatePage)> {
    PAGE_SPECS
        .into_iter()
        .enumerate()
        .map(
            |(page_index, (source_page, _, cell_count, padding_count))| {
                let start = page_start(page_index);
                let characters: String = (0..cell_count - padding_count)
                    .map(|index| char::from_u32(start + index as u32).unwrap())
                    .collect();
                (
                    format!("page-{page_index}.json"),
                    DialogueNameEntryCandidatePage {
                        kind: CANDIDATE_KIND.to_string(),
                        source_overlay_sha256: OVERLAY_SHA256.to_string(),
                        source_page: source_page.to_string(),
                        selection_basis: "test candidates".to_string(),
                        preserved_source_characters: if page_index == 2 {
                            "0123456789".to_string()
                        } else {
                            String::new()
                        },
                        characters: if page_index == 2 {
                            characters.chars().skip(10).collect()
                        } else {
                            characters
                        },
                    },
                )
            },
        )
        .collect()
}

fn page_start(page_index: usize) -> u32 {
    match page_index {
        0 => 0xac00,
        1 => 0xad00,
        2 => 0xae00,
        _ => unreachable!(),
    }
}

fn name_entry_page_fixture() -> Vec<u8> {
    let mut data = vec![0u8; 36_796];
    let mut code = 1u16;
    for (page_index, (_, offset, cell_count, padding_count)) in PAGE_SPECS.into_iter().enumerate() {
        let mut selectable_index = 0usize;
        for cell_index in 0..cell_count {
            let value = if cell_index < padding_count {
                PADDING_CODE
            } else if page_index == 2 && selectable_index < 10 {
                let decimal_codes = [
                    0x000a, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009,
                ];
                let value = decimal_codes[selectable_index];
                selectable_index += 1;
                value
            } else {
                let value = code;
                code += 1;
                selectable_index += 1;
                value
            };
            data[offset + cell_index * 2..offset + cell_index * 2 + 2]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    data
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}
