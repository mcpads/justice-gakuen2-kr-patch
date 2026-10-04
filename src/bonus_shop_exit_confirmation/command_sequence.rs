use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::glyph_slots::QUESTION_MARK_CODE;
use super::model::{ShopExitGlyphAllocation, ShopExitTextUnit};

const WOMAN_SOURCE_RECORD: [u8; 90] = [
    0x00, 0x04, 0x07, 0x00, 0x06, 0x0a, 0x00, 0x00, 0x07, 0x00, 0x02, 0x0a, 0x00, 0x06, 0x07, 0x02,
    0x05, 0x04, 0x00, 0x08, 0x0a, 0x00, 0x04, 0x09, 0x00, 0x05, 0x05, 0x80, 0x80, 0x63, 0x63, 0x63,
    0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x00, 0x05, 0x09, 0x00,
    0x05, 0x07, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63,
    0x63, 0x63, 0x63, 0x63, 0x00, 0x05, 0x07, 0x00, 0x05, 0x07, 0x00, 0x07, 0x07, 0x63, 0x63, 0x63,
    0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x81,
];

const ELDER_SOURCE_RECORD: [u8; 96] = [
    0x00, 0x01, 0x0b, 0x00, 0x05, 0x05, 0x63, 0x63, 0x63, 0x00, 0x02, 0x0a, 0x00, 0x06, 0x07, 0x02,
    0x05, 0x04, 0x00, 0x08, 0x0a, 0x00, 0x04, 0x09, 0x00, 0x09, 0x07, 0x00, 0x05, 0x07, 0x00, 0x05,
    0x05, 0x80, 0x80, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63,
    0x63, 0x63, 0x00, 0x05, 0x09, 0x00, 0x05, 0x07, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63,
    0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x00, 0x05, 0x07, 0x00, 0x05, 0x07,
    0x00, 0x07, 0x07, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x81,
];

const LEADING_CHOICE_BLANK_COMMANDS: usize = 5;
const YES_SLOT_COMMANDS: usize = 2;
const BETWEEN_CHOICE_BLANK_COMMANDS: usize = 6;
const NO_SLOT_COMMANDS: usize = 3;
const TRAILING_CHOICE_BLANK_COMMANDS: usize = 4;
const NEWLINE_COUNT: usize = 2;
const BLANK_COMMAND: [u8; 3] = [0x63; 3];

#[derive(Clone, Copy)]
pub(super) struct ShopExitSourceUnitSpec {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) encoded_offset: usize,
    pub(super) encoded_length: usize,
    pub(super) source_sha256: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct ShopExitRecordSpec {
    pub(super) variant_id: &'static str,
    pub(super) clerk_index: usize,
    pub(super) source_record: &'static [u8],
    pub(super) record_offset: usize,
    pub(super) source_record_sha256: &'static str,
    pub(super) terminator_offset: usize,
    pub(super) pointer_storage_offset: usize,
    pub(super) prompt_slot_commands: usize,
    pub(super) unit_ids: [&'static str; 3],
}

pub(super) const SOURCE_UNIT_SPECS: [ShopExitSourceUnitSpec; 6] = [
    ShopExitSourceUnitSpec {
        id: "woman_prompt",
        source_text: "あら、もう帰るの？",
        encoded_offset: 0x2f0c,
        encoded_length: 27,
        source_sha256: "4e1c68608173f0bcaac5b29f657826ae2138100eda2416fabd83784340b37b85",
    },
    ShopExitSourceUnitSpec {
        id: "woman_yes",
        source_text: "はい",
        encoded_offset: 0x2f38,
        encoded_length: 6,
        source_sha256: "f98516fdb6293503638d4c02d84a252b6233fc96e8480ba5e7a2aa11194e460b",
    },
    ShopExitSourceUnitSpec {
        id: "woman_no",
        source_text: "いいえ",
        encoded_offset: 0x2f50,
        encoded_length: 9,
        source_sha256: "9e0eaea691b11c67b4b1d7ba777b6987283f7734487522d535a4340ad2365bfb",
    },
    ShopExitSourceUnitSpec {
        id: "elder_prompt",
        source_text: "ん？ もう帰るのかい？",
        encoded_offset: 0x33d0,
        encoded_length: 33,
        source_sha256: "edcc098e43f973b82d6808b0f2b4226d4c78b106b9cc8e0ed2e85b82bb1fc566",
    },
    ShopExitSourceUnitSpec {
        id: "elder_yes",
        source_text: "はい",
        encoded_offset: 0x3402,
        encoded_length: 6,
        source_sha256: "f98516fdb6293503638d4c02d84a252b6233fc96e8480ba5e7a2aa11194e460b",
    },
    ShopExitSourceUnitSpec {
        id: "elder_no",
        source_text: "いいえ",
        encoded_offset: 0x341a,
        encoded_length: 9,
        source_sha256: "9e0eaea691b11c67b4b1d7ba777b6987283f7734487522d535a4340ad2365bfb",
    },
];

pub(super) const RECORD_SPECS: [ShopExitRecordSpec; 2] = [
    ShopExitRecordSpec {
        variant_id: "woman",
        clerk_index: 0,
        source_record: &WOMAN_SOURCE_RECORD,
        record_offset: 0x2f0c,
        source_record_sha256: "7728fa7ab1c819d7a9967e9da5330c90d3a88b4a41f3097fb58d60bf15d13364",
        terminator_offset: 0x2f65,
        pointer_storage_offset: 0x372c,
        prompt_slot_commands: 9,
        unit_ids: ["woman_prompt", "woman_yes", "woman_no"],
    },
    ShopExitRecordSpec {
        variant_id: "elder",
        clerk_index: 1,
        source_record: &ELDER_SOURCE_RECORD,
        record_offset: 0x33d0,
        source_record_sha256: "c02c439e16071de79e2df9513e5ee4295aaacd113a34ce71845ae896dfeab5ea",
        terminator_offset: 0x342f,
        pointer_storage_offset: 0x3754,
        prompt_slot_commands: 11,
        unit_ids: ["elder_prompt", "elder_yes", "elder_no"],
    },
];

#[derive(Debug)]
pub(super) struct EncodedShopExitRecord {
    pub(super) variant_id: &'static str,
    pub(super) bytes: Vec<u8>,
    pub(super) unit_command_counts: [usize; 3],
    pub(super) line_count: usize,
}

#[derive(Debug)]
pub(super) struct EncodedShopExitConfirmation {
    pub(super) records: Vec<EncodedShopExitRecord>,
    pub(super) unit_command_counts: BTreeMap<String, usize>,
}

pub(super) fn encode_exit_confirmation(
    units: &[ShopExitTextUnit],
    allocations: &[ShopExitGlyphAllocation],
) -> Result<EncodedShopExitConfirmation> {
    let glyph_codes = allocations
        .iter()
        .map(|allocation| (allocation.text, allocation.code))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::with_capacity(RECORD_SPECS.len());
    let mut unit_command_counts = BTreeMap::new();
    for spec in RECORD_SPECS {
        let prompt = unit_text(units, spec.unit_ids[0])?;
        let yes = unit_text(units, spec.unit_ids[1])?;
        let no = unit_text(units, spec.unit_ids[2])?;
        let prompt_commands = encode_text(prompt, &glyph_codes)?;
        let yes_commands = encode_text(yes, &glyph_codes)?;
        let no_commands = encode_text(no, &glyph_codes)?;
        let counts = [
            prompt_commands.len() / 3,
            yes_commands.len() / 3,
            no_commands.len() / 3,
        ];

        let mut bytes = Vec::with_capacity(spec.source_record.len());
        append_fixed_slot(
            &mut bytes,
            &prompt_commands,
            spec.prompt_slot_commands,
            spec.unit_ids[0],
        )?;
        bytes.extend(std::iter::repeat_n(0x80, NEWLINE_COUNT));
        append_blanks(&mut bytes, LEADING_CHOICE_BLANK_COMMANDS);
        append_fixed_slot(
            &mut bytes,
            &yes_commands,
            YES_SLOT_COMMANDS,
            spec.unit_ids[1],
        )?;
        append_blanks(&mut bytes, BETWEEN_CHOICE_BLANK_COMMANDS);
        append_fixed_slot(&mut bytes, &no_commands, NO_SLOT_COMMANDS, spec.unit_ids[2])?;
        append_blanks(&mut bytes, TRAILING_CHOICE_BLANK_COMMANDS);
        bytes.push(0x81);
        ensure!(
            bytes.len() == spec.source_record.len(),
            "shop exit-confirmation {} encoder no longer owns its exact source record geometry",
            spec.variant_id
        );
        for (id, count) in spec.unit_ids.into_iter().zip(counts) {
            ensure!(
                unit_command_counts.insert(id.to_string(), count).is_none(),
                "duplicate shop exit-confirmation unit {id}"
            );
        }
        records.push(EncodedShopExitRecord {
            variant_id: spec.variant_id,
            bytes,
            unit_command_counts: counts,
            line_count: NEWLINE_COUNT + 1,
        });
    }
    ensure!(
        unit_command_counts.len() == units.len(),
        "shop exit-confirmation encoder did not consume every authored unit"
    );
    Ok(EncodedShopExitConfirmation {
        records,
        unit_command_counts,
    })
}

fn unit_text<'a>(units: &'a [ShopExitTextUnit], id: &str) -> Result<&'a str> {
    let unit = units
        .iter()
        .find(|unit| unit.id == id)
        .with_context(|| format!("shop exit-confirmation unit {id} disappeared"))?;
    unit.korean_text
        .as_deref()
        .context("authored shop exit-confirmation text disappeared")
}

fn encode_text(text: &str, glyph_codes: &BTreeMap<char, u16>) -> Result<Vec<u8>> {
    let mut commands = Vec::with_capacity(text.chars().count() * 3);
    for character in text.chars() {
        ensure!(
            !character.is_control(),
            "shop exit-confirmation text does not accept control characters"
        );
        match character {
            ' ' => commands.extend_from_slice(&BLANK_COMMAND),
            '?' => commands.extend_from_slice(&glyph_command(QUESTION_MARK_CODE)),
            character => {
                let code = glyph_codes.get(&character).with_context(|| {
                    format!("no shop exit-confirmation glyph for {character:?}")
                })?;
                commands.extend_from_slice(&glyph_command(*code));
            }
        }
    }
    Ok(commands)
}

fn append_fixed_slot(
    output: &mut Vec<u8>,
    commands: &[u8],
    capacity: usize,
    id: &str,
) -> Result<()> {
    ensure!(
        commands.len().is_multiple_of(3) && commands.len() / 3 <= capacity,
        "shop exit-confirmation {id} exceeds its source command capacity"
    );
    output.extend_from_slice(commands);
    append_blanks(output, capacity - commands.len() / 3);
    Ok(())
}

fn append_blanks(output: &mut Vec<u8>, count: usize) {
    for _ in 0..count {
        output.extend_from_slice(&BLANK_COMMAND);
    }
}

pub(super) const fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
