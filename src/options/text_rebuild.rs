use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::text::{SKIP_GLYPH_CODE, read_length_prefixed_codes};

use super::allocation::{OptionsGlyphAllocation, TRANSPARENT_ADVANCE_CODE};
use super::assets::parse_hex_usize;
use super::model::{OptionsAuthoredUnit, OptionsFontRole, OptionsTextBuild};
use super::records_main;
use super::source::OVERLAY_RUNTIME_BASE;

const SHARED_POOL_START: usize = 0x036c;
const SHARED_POOL_END: usize = 0x04b2;
const RELOCATED_UNIT_ID: &str = "cpu_round_character_change";
const EXPECTED_SHARED_POOL_UNIT_COUNT: usize = 22;
const IN_PLACE_UNIT_IDS: [&str; 47] = [
    "defaults",
    "heading",
    "exit",
    "key_control_heading",
    "game_system_heading",
    "player_one",
    "player_two",
    "no_assignment",
    "weak_punch",
    "strong_punch",
    "weak_kick",
    "strong_kick",
    "complete_burn_attack_1",
    "complete_burn_attack_2",
    "complete_burn_attack_3",
    "complete_burn_attack_4",
    "axis_move",
    "throw",
    "two_platoon",
    "breakfall",
    "spirit_gauge",
    "guard",
    "back_dash",
    "back_jump",
    "normal",
    "automatic",
    "records_no",
    "records_yes",
    "records_load_confirmation",
    "records_save_confirmation",
    "records_horizontal_help",
    "records_vertical_help",
    "records_await_input",
    "records_save_complete",
    "records_load_complete",
    "records_data_error",
    "records_memory_card_not_inserted",
    "records_memory_card_corrupt",
    "records_data_file_missing",
    "records_memory_card_full",
    "records_memory_card_unformatted",
    "records_initialize_confirmation",
    "records_save_failed",
    "records_memory_card_check",
    "records_saving",
    "records_keep_memory_card_inserted",
    "records_overwrite_confirmation",
];

pub(super) fn rebuild_options_text(
    source_overlay: &[u8],
    units: &[OptionsAuthoredUnit],
    allocation: &OptionsGlyphAllocation,
) -> Result<(Vec<u8>, Vec<OptionsTextBuild>)> {
    let mut encoded = units
        .iter()
        .map(|unit| {
            Ok((
                parse_hex_usize(&unit.source_offset, "source offset")?,
                unit,
                encode_unit(unit, allocation)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    encoded.sort_by_key(|(source_offset, _, _)| *source_offset);

    let mut pooled = encoded
        .iter()
        .filter(|(source_offset, unit, _)| {
            (SHARED_POOL_START..SHARED_POOL_END).contains(source_offset)
                || unit.id == RELOCATED_UNIT_ID
        })
        .map(|(_, unit, codes)| (*unit, codes))
        .collect::<Vec<_>>();
    ensure!(
        pooled.len() == EXPECTED_SHARED_POOL_UNIT_COUNT,
        "options shared string-pool membership changed"
    );
    pooled.sort_by_key(|(unit, _)| {
        if unit.id == RELOCATED_UNIT_ID {
            usize::MAX
        } else {
            parse_hex_usize(&unit.source_offset, "source offset").unwrap()
        }
    });
    let packed = pack_records(
        &pooled
            .iter()
            .map(|(unit, codes)| (unit.id.as_str(), codes.as_slice()))
            .collect::<Vec<_>>(),
        SHARED_POOL_END - SHARED_POOL_START,
    )?;
    ensure!(
        packed.bytes.len() == SHARED_POOL_END - SHARED_POOL_START,
        "options Korean shared string pool no longer exactly fills its source-owned range"
    );

    let mut output = source_overlay.to_vec();
    output[SHARED_POOL_START..SHARED_POOL_END].copy_from_slice(&packed.bytes);
    let mut output_offsets = BTreeMap::new();
    for (id, relative_offset) in packed.offsets {
        output_offsets.insert(id, SHARED_POOL_START + relative_offset);
    }
    let records_main_records = encoded
        .iter()
        .filter(|(_, unit, _)| records_main::contains_unit(&unit.id))
        .map(|(_, unit, codes)| (unit.id.as_str(), codes.as_slice()))
        .collect::<Vec<_>>();
    output_offsets.extend(records_main::rebuild_pool(
        &mut output,
        &records_main_records,
    )?);

    let pooled_ids = pooled
        .iter()
        .map(|(unit, _)| unit.id.as_str())
        .chain(records_main::UNIT_IDS)
        .collect::<BTreeSet<_>>();
    let in_place = encoded
        .iter()
        .filter(|(_, unit, _)| IN_PLACE_UNIT_IDS.contains(&unit.id.as_str()))
        .map(|(_, unit, codes)| (*unit, codes))
        .collect::<Vec<_>>();
    ensure!(
        in_place
            .iter()
            .map(|(unit, _)| unit.id.as_str())
            .collect::<BTreeSet<_>>()
            == IN_PLACE_UNIT_IDS.into_iter().collect(),
        "options in-place string membership changed"
    );
    for (unit, codes) in in_place {
        ensure!(
            codes.len() <= unit.source_codes.len(),
            "options {} no longer fits its source record",
            unit.id
        );
        let source_offset = parse_hex_usize(&unit.source_offset, "source offset")?;
        write_record_prefix(&mut output, source_offset, codes)?;
        output_offsets.insert(unit.id.as_str(), source_offset);
    }
    let relocated = encoded
        .iter()
        .filter(|(_, unit, _)| {
            !pooled_ids.contains(unit.id.as_str()) && !IN_PLACE_UNIT_IDS.contains(&unit.id.as_str())
        })
        .map(|(_, unit, codes)| (*unit, codes))
        .collect::<Vec<_>>();
    for (id, offset) in pack_owned_regions(source_overlay, &mut output, &relocated)? {
        output_offsets.insert(id, offset);
    }

    let mut reports = Vec::with_capacity(encoded.len());
    for (_, unit, codes) in encoded {
        let output_offset = *output_offsets
            .get(unit.id.as_str())
            .with_context(|| format!("options {} has no rebuilt record", unit.id))?;
        let output_pointer = OVERLAY_RUNTIME_BASE
            .checked_add(u32::try_from(output_offset)?)
            .context("options rebuilt pointer overflow")?;
        for pointer_offset in &unit.pointer_offsets {
            let pointer_offset = parse_hex_usize(pointer_offset, "pointer offset")?;
            write_u32(&mut output, pointer_offset, output_pointer)?;
        }
        ensure!(
            read_length_prefixed_codes(&output, output_offset)? == codes,
            "options {} rebuilt codes differ from their record",
            unit.id
        );
        reports.push(OptionsTextBuild {
            id: unit.id.clone(),
            source_offset: unit.source_offset.clone(),
            output_offset: format!("0x{output_offset:04x}"),
            source_text: unit.source_text.clone(),
            korean_text: unit.korean_text.clone(),
            font_role: unit.font_role.key().to_string(),
            output_codes: codes.iter().map(|code| format!("0x{code:04x}")).collect(),
            pointer_offsets: unit.pointer_offsets.clone(),
        });
    }
    reports.sort_by(|left, right| left.id.cmp(&right.id));
    Ok((output, reports))
}

pub(super) fn options_text_expected_write_ranges(
    units: &[OptionsAuthoredUnit],
) -> Result<Vec<[usize; 2]>> {
    let mut ranges = vec![
        [SHARED_POOL_START, SHARED_POOL_END],
        records_main::expected_write_range(),
    ];
    for unit in units {
        let source_offset = parse_hex_usize(&unit.source_offset, "source offset")?;
        if IN_PLACE_UNIT_IDS.contains(&unit.id.as_str()) {
            ranges.push([
                source_offset,
                source_offset + 2 + unit.source_codes.len() * 2,
            ]);
        } else if source_offset >= SHARED_POOL_END
            && !records_main::UNIT_IDS.contains(&unit.id.as_str())
        {
            ranges.push([
                source_offset,
                (source_offset + 2 + unit.source_codes.len() * 2).next_multiple_of(4),
            ]);
        }
        for pointer_offset in &unit.pointer_offsets {
            let pointer_offset = parse_hex_usize(pointer_offset, "pointer offset")?;
            ranges.push([pointer_offset, pointer_offset + 4]);
        }
    }
    Ok(ranges)
}

fn pack_owned_regions<'a>(
    source: &[u8],
    output: &mut [u8],
    records: &[(&'a OptionsAuthoredUnit, &Vec<u16>)],
) -> Result<Vec<(&'a str, usize)>> {
    let mut ranges = Vec::new();
    for (unit, _) in records {
        let start = parse_hex_usize(&unit.source_offset, "source offset")?;
        let text_end = start + 2 + unit.source_codes.len() * 2;
        let end = text_end.next_multiple_of(4);
        ensure!(
            source
                .get(text_end..end)
                .is_some_and(|pad| pad.iter().all(|b| *b == 0)),
            "options {} source alignment padding is not empty",
            unit.id
        );
        ranges.push([start, end]);
    }
    ranges.sort_unstable();
    let mut pools: Vec<[usize; 2]> = Vec::new();
    for [start, end] in ranges {
        if let Some(last) = pools.last_mut() {
            ensure!(start >= last[1], "Options source text ownership overlaps");
            if start == last[1] {
                last[1] = end;
                continue;
            }
        }
        pools.push([start, end]);
    }
    for [start, end] in &pools {
        output[*start..*end].fill(0);
    }
    let mut ordered = records.to_vec();
    ordered.sort_by_key(|(unit, codes)| (std::cmp::Reverse(codes.len()), unit.id.as_str()));
    let mut offsets = Vec::new();
    for (unit, codes) in ordered {
        let size = 2 + codes.len() * 2;
        let index = pools
            .iter()
            .enumerate()
            .filter(|(_, p)| p[1] - p[0] >= size)
            .min_by_key(|(_, p)| (p[1] - p[0], p[0]))
            .map(|(i, _)| i)
            .with_context(|| {
                format!(
                    "no owned Options text region fits {} ({size} bytes)",
                    unit.id
                )
            })?;
        let start = pools[index][0];
        write_record_prefix(output, start, codes)?;
        pools[index][0] += size;
        offsets.push((unit.id.as_str(), start));
    }
    Ok(offsets)
}

pub(super) fn encode_unit(
    unit: &OptionsAuthoredUnit,
    allocation: &OptionsGlyphAllocation,
) -> Result<Vec<u16>> {
    let mut codes = Vec::new();
    for (index, character) in unit.korean_text.chars().enumerate() {
        if matches!(
            unit.font_role,
            OptionsFontRole::Heading | OptionsFontRole::RecordsMainHeading
        ) && index > 0
            && character != ' '
        {
            codes.push(TRANSPARENT_ADVANCE_CODE);
        }
        let code = if character == ' ' {
            SKIP_GLYPH_CODE
        } else {
            allocation.code_for(unit.font_role, character)?
        };
        codes.push(code);
    }
    ensure!(!codes.is_empty(), "options {} encodes no glyphs", unit.id);
    Ok(codes)
}

struct PackedRecords<'a> {
    bytes: Vec<u8>,
    offsets: Vec<(&'a str, usize)>,
}

fn pack_records<'a>(records: &[(&'a str, &[u16])], capacity: usize) -> Result<PackedRecords<'a>> {
    let mut bytes = Vec::new();
    let mut offsets = Vec::with_capacity(records.len());
    for (id, codes) in records {
        ensure!(
            codes.len() <= usize::from(u16::MAX),
            "options {id} code count exceeds u16"
        );
        offsets.push((*id, bytes.len()));
        bytes.extend_from_slice(&(codes.len() as u16).to_le_bytes());
        for code in *codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
    }
    ensure!(
        bytes.len() <= capacity,
        "options records need {} bytes but their pool owns {capacity}",
        bytes.len()
    );
    bytes.resize(capacity, 0);
    Ok(PackedRecords { bytes, offsets })
}

fn write_record_prefix(output: &mut [u8], offset: usize, codes: &[u16]) -> Result<()> {
    let byte_count = 2 + codes.len() * 2;
    ensure!(
        offset + byte_count <= output.len(),
        "options record write is out of bounds"
    );
    output[offset..offset + 2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let code_offset = offset + 2 + index * 2;
        output[code_offset..code_offset + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    let bytes = output
        .get_mut(offset..offset + 4)
        .context("options pointer write is out of bounds")?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
pub(super) fn pack_records_for_test(
    records: &[(&str, &[u16])],
    capacity: usize,
) -> Result<(Vec<u8>, Vec<usize>)> {
    let packed = pack_records(records, capacity)?;
    Ok((
        packed.bytes,
        packed
            .offsets
            .into_iter()
            .map(|(_, offset)| offset)
            .collect(),
    ))
}

#[cfg(test)]
mod owned_region_tests {
    use super::super::model::OptionsReleaseStatus;
    use super::*;

    fn unit(id: &str, offset: usize) -> OptionsAuthoredUnit {
        OptionsAuthoredUnit {
            id: id.into(),
            source_offset: format!("0x{offset:04x}"),
            pointer_offsets: vec![],
            source_codes: vec!["0x0100".into(), "0x0101".into()],
            source_text: "source".into(),
            korean_text: "번역".into(),
            font_role: OptionsFontRole::Label,
            release_status: OptionsReleaseStatus::NeedsHumanReview,
        }
    }

    #[test]
    fn adjacent_owned_records_allow_longer_text_without_borrowing_a_gap() {
        let mut source = vec![0; 32];
        source[16..24].fill(0x5a);
        let a = unit("long", 0);
        let b = unit("short", 8);
        let c = unit("separate", 24);
        let long = vec![0x101; 4]; // Ten bytes cannot fit one original eight-byte slot.
        let short = vec![0x102];
        let separate = vec![0x103; 2];
        let mut output = source.clone();
        let offsets = pack_owned_regions(
            &source,
            &mut output,
            &[(&a, &long), (&b, &short), (&c, &separate)],
        )
        .unwrap();
        for (id, codes) in [("long", &long), ("short", &short), ("separate", &separate)] {
            let offset = offsets.iter().find(|(name, _)| *name == id).unwrap().1;
            assert_eq!(&read_length_prefixed_codes(&output, offset).unwrap(), codes);
        }
        assert_eq!(&output[16..24], &source[16..24]);
    }

    #[test]
    fn nonzero_alignment_bytes_are_rejected_before_writing() {
        let mut source = vec![0; 8];
        source[6] = 1;
        let mut output = source.clone();
        assert!(
            pack_owned_regions(&source, &mut output, &[(&unit("label", 0), &vec![0x101])]).is_err()
        );
        assert_eq!(output, source);
    }
}
