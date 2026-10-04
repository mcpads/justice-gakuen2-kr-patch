use std::collections::BTreeMap;

use super::command_sequence::{RECORD_SPECS, SOURCE_UNIT_SPECS, encode_exit_confirmation};
use super::model::{ShopExitManifest, ShopExitRecordSourceBinding};
use super::test_support::{authored_units, glyph_allocations, manifest};

#[test]
#[ignore = "requires assets/"]
fn six_json_units_reencode_both_runtime_selected_clerk_records() {
    let units = authored_units();
    let allocations = glyph_allocations();
    let encoded = encode_exit_confirmation(&units, &allocations).unwrap();

    assert_eq!(encoded.records.len(), RECORD_SPECS.len());
    for (spec, record) in RECORD_SPECS.iter().zip(&encoded.records) {
        let expected = independently_encode_assets_into_source_geometry(spec, &units, &allocations);
        assert_eq!(record.variant_id, spec.variant_id);
        assert_eq!(record.bytes, expected);
        assert_eq!(record.line_count, typed_line_count(&expected));
    }
    assert_eq!(
        encoded.unit_command_counts,
        units
            .iter()
            .map(|unit| {
                (
                    unit.id.clone(),
                    unit.korean_text.as_deref().unwrap().chars().count(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    );
}

#[test]
#[ignore = "requires assets/"]
fn either_clerk_prompt_overflow_and_unallocated_text_fail_closed() {
    let allocations = glyph_allocations();
    for spec in RECORD_SPECS {
        let mut overflow = authored_units();
        let prompt = overflow
            .iter_mut()
            .find(|unit| unit.id == spec.unit_ids[0])
            .unwrap();
        prompt.korean_text = Some(
            allocations[0]
                .text
                .to_string()
                .repeat(spec.prompt_slot_commands + 1),
        );
        assert!(
            encode_exit_confirmation(&overflow, &allocations)
                .unwrap_err()
                .to_string()
                .contains("source command capacity")
        );
    }

    let mut unallocated = authored_units();
    unallocated[1].korean_text.as_mut().unwrap().push('X');
    assert!(
        encode_exit_confirmation(&unallocated, &allocations)
            .unwrap_err()
            .to_string()
            .contains("no shop exit-confirmation glyph")
    );
}

#[test]
#[ignore = "requires assets/"]
fn concrete_manifest_schema_rejects_a_duplicate_known_key() {
    let json =
        crate::test_input::read_str("assets/menu/shop-ui/dynamic/exit-confirmation/manifest.json");
    let duplicate = json.replacen(
        "\"unit_id\": \"elder_prompt\",",
        "\"unit_id\": \"elder_prompt\", \"unit_id\": \"elder_yes\",",
        1,
    );
    let error = serde_json::from_str::<ShopExitManifest>(&duplicate).unwrap_err();
    assert!(error.to_string().contains("duplicate field `unit_id`"));
}

#[test]
fn record_binding_schema_rejects_unknown_fields() {
    let binding = r#"{
        "variant_id":"woman",
        "clerk_index":0,
        "record_offset":"0x2f0c",
        "record_length":90,
        "record_sha256":"x",
        "terminator_offset":"0x2f65",
        "pointer_storage_offset":"0x372c",
        "pointer_value":"0x800a4f0c",
        "extra":true
    }"#;
    assert!(serde_json::from_str::<ShopExitRecordSourceBinding>(binding).is_err());
}

#[test]
#[ignore = "requires assets/"]
fn manifest_allocates_each_authored_non_source_character_once() {
    let units = authored_units();
    let required = units
        .iter()
        .flat_map(|unit| unit.korean_text.as_deref().unwrap().chars())
        .filter(|character| *character != ' ' && *character != '?')
        .collect::<std::collections::BTreeSet<_>>();
    let allocated = glyph_allocations()
        .into_iter()
        .map(|allocation| allocation.text)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(allocated, required);
    assert_eq!(manifest().reused_question_mark.code, "0x0055");
}

fn independently_encode_assets_into_source_geometry(
    record: &super::command_sequence::ShopExitRecordSpec,
    units: &[super::model::ShopExitTextUnit],
    allocations: &[super::model::ShopExitGlyphAllocation],
) -> Vec<u8> {
    let source_specs = record
        .unit_ids
        .map(|id| SOURCE_UNIT_SPECS.iter().find(|spec| spec.id == id).unwrap());
    let newline_start = record
        .source_record
        .windows(2)
        .position(|bytes| bytes == [0x80, 0x80])
        .unwrap();
    let newline_end = newline_start + 2;
    let yes_start = source_specs[1].encoded_offset - record.record_offset;
    let yes_end = yes_start + source_specs[1].encoded_length;
    let no_start = source_specs[2].encoded_offset - record.record_offset;
    let no_end = no_start + source_specs[2].encoded_length;
    let terminator = record
        .source_record
        .iter()
        .position(|byte| *byte == 0x81)
        .unwrap();

    let codes = allocations
        .iter()
        .map(|allocation| (allocation.text, allocation.code))
        .collect::<BTreeMap<_, _>>();
    let question = u16::from_str_radix(
        manifest()
            .reused_question_mark
            .code
            .strip_prefix("0x")
            .unwrap(),
        16,
    )
    .unwrap();
    let texts = record.unit_ids.map(|id| {
        units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .korean_text
            .as_deref()
            .unwrap()
    });
    let mut output = Vec::new();
    append_slot(&mut output, texts[0], newline_start / 3, &codes, question);
    output.extend_from_slice(&record.source_record[newline_start..newline_end]);
    append_blank_commands(&mut output, (yes_start - newline_end) / 3);
    append_slot(
        &mut output,
        texts[1],
        source_specs[1].encoded_length / 3,
        &codes,
        question,
    );
    append_blank_commands(&mut output, (no_start - yes_end) / 3);
    append_slot(
        &mut output,
        texts[2],
        source_specs[2].encoded_length / 3,
        &codes,
        question,
    );
    append_blank_commands(&mut output, (terminator - no_end) / 3);
    output.push(0x81);
    output
}

fn append_slot(
    output: &mut Vec<u8>,
    text: &str,
    capacity: usize,
    codes: &BTreeMap<char, u16>,
    question: u16,
) {
    for character in text.chars() {
        match character {
            ' ' => output.extend_from_slice(&[0x63; 3]),
            '?' => append_code(output, question),
            character => append_code(output, codes[&character]),
        }
    }
    append_blank_commands(output, capacity - text.chars().count());
}

fn append_code(output: &mut Vec<u8>, code: u16) {
    output.extend_from_slice(&[
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]);
}

fn append_blank_commands(output: &mut Vec<u8>, count: usize) {
    for _ in 0..count {
        output.extend_from_slice(&[0x63; 3]);
    }
}

fn typed_line_count(bytes: &[u8]) -> usize {
    let mut cursor = 0;
    let mut lines = 1;
    loop {
        match bytes[cursor] {
            0x81 => {
                assert_eq!(cursor + 1, bytes.len());
                return lines;
            }
            0x80 => {
                lines += 1;
                cursor += 1;
            }
            0x63 => {
                assert_eq!(&bytes[cursor..cursor + 3], &[0x63; 3]);
                cursor += 3;
            }
            page => {
                assert!(page <= 3 && bytes[cursor + 1] <= 15 && bytes[cursor + 2] <= 15);
                cursor += 3;
            }
        }
    }
}
