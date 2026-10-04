use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::model::{
    BonusShopTextSourceToken, BonusShopTextSourceTokenKind, BonusShopTextSourceUnit, ShopTextRole,
};
use crate::bonus_shop_source::{
    GLYPH_TIM_OFFSET, OVERLAY_RUNTIME_BASE, POINTER_COMMAND_RECORD_RANGES,
    POINTER_COMMAND_TABLE_RANGES,
};
use crate::pipeline::sha256_bytes;
use crate::tim::parse_4bpp_prefix;

const GLYPH_CELL_WIDTH: usize = 20;
const GLYPH_CELL_HEIGHT: usize = 20;
const TEXTURE_PAGE_WIDTH: usize = 256;

#[derive(Debug, Clone, Copy)]
pub(super) struct ShopTextTableSpec {
    pub(super) role: ShopTextRole,
    pub(super) pointer_table_range: [usize; 2],
    pub(super) record_region: [usize; 2],
}

pub(super) const TABLE_SPECS: [ShopTextTableSpec; 3] = [
    ShopTextTableSpec {
        role: ShopTextRole::ProductDescription,
        pointer_table_range: POINTER_COMMAND_TABLE_RANGES[0],
        record_region: POINTER_COMMAND_RECORD_RANGES[0],
    },
    ShopTextTableSpec {
        role: ShopTextRole::ProductLabel,
        pointer_table_range: POINTER_COMMAND_TABLE_RANGES[1],
        record_region: POINTER_COMMAND_RECORD_RANGES[1],
    },
    ShopTextTableSpec {
        role: ShopTextRole::ClerkDialogue,
        pointer_table_range: POINTER_COMMAND_TABLE_RANGES[2],
        record_region: POINTER_COMMAND_RECORD_RANGES[2],
    },
];

pub(super) struct ParsedShopTextTable {
    pub(super) spec: ShopTextTableSpec,
    pub(super) records: Vec<ParsedShopTextRecord>,
    pub(super) total_glyph_count: usize,
    pub(super) resolved_glyph_count: usize,
    pub(super) fully_decoded_record_count: usize,
}

pub(super) struct ParsedShopTextRecord {
    pub(super) source_offset: usize,
    pub(super) unit: BonusShopTextSourceUnit,
}

pub(super) fn parse_shop_text_tables(
    overlay: &[u8],
    shop_ui_decoded: &[u8],
    verified_pixel_texts: &BTreeMap<String, String>,
) -> Result<Vec<ParsedShopTextTable>> {
    TABLE_SPECS
        .into_iter()
        .map(|spec| parse_table(overlay, shop_ui_decoded, verified_pixel_texts, spec))
        .collect()
}

fn parse_table(
    overlay: &[u8],
    shop_ui_decoded: &[u8],
    verified_pixel_texts: &BTreeMap<String, String>,
    spec: ShopTextTableSpec,
) -> Result<ParsedShopTextTable> {
    ensure!(
        (spec.pointer_table_range[1] - spec.pointer_table_range[0]).is_multiple_of(4),
        "KOUBAI {:?} pointer table lost word alignment",
        spec.role
    );
    let pointer_offsets = (spec.pointer_table_range[0]..spec.pointer_table_range[1])
        .step_by(4)
        .collect::<Vec<_>>();
    ensure!(
        pointer_offsets.len() == spec.role.expected_record_count(),
        "KOUBAI {:?} pointer-table cardinality changed",
        spec.role
    );
    let targets = pointer_offsets
        .iter()
        .map(|&pointer_offset| pointer_target(overlay, pointer_offset, spec))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        targets.windows(2).all(|pair| pair[0] < pair[1]),
        "KOUBAI {:?} record pointers are not strictly increasing and unique",
        spec.role
    );
    ensure!(
        targets.iter().copied().collect::<BTreeSet<_>>().len() == targets.len(),
        "KOUBAI {:?} record pointers repeat a source target",
        spec.role
    );

    let mut records = Vec::with_capacity(targets.len());
    let mut total_glyph_count = 0usize;
    let mut resolved_glyph_count = 0usize;
    let mut fully_decoded_record_count = 0usize;
    for (record_index, (&pointer_storage_offset, &source_offset)) in
        pointer_offsets.iter().zip(&targets).enumerate()
    {
        let record_limit = targets
            .get(record_index + 1)
            .copied()
            .unwrap_or(spec.record_region[1]);
        let unit = parse_record(
            overlay,
            shop_ui_decoded,
            verified_pixel_texts,
            ShopRecordLocation {
                role: spec.role,
                record_index,
                pointer_storage_offset,
                source_offset,
                record_limit,
            },
        )?;
        total_glyph_count += unit.glyph_count;
        resolved_glyph_count += unit.resolved_glyph_count;
        fully_decoded_record_count += usize::from(unit.exact_source_text.is_some());
        records.push(ParsedShopTextRecord {
            source_offset,
            unit,
        });
    }
    Ok(ParsedShopTextTable {
        spec,
        records,
        total_glyph_count,
        resolved_glyph_count,
        fully_decoded_record_count,
    })
}

#[derive(Clone, Copy)]
struct ShopRecordLocation {
    role: ShopTextRole,
    record_index: usize,
    pointer_storage_offset: usize,
    source_offset: usize,
    record_limit: usize,
}

fn parse_record(
    overlay: &[u8],
    shop_ui_decoded: &[u8],
    verified_pixel_texts: &BTreeMap<String, String>,
    location: ShopRecordLocation,
) -> Result<BonusShopTextSourceUnit> {
    let ShopRecordLocation {
        role,
        record_index,
        pointer_storage_offset,
        source_offset,
        record_limit,
    } = location;
    ensure!(
        source_offset < record_limit,
        "empty KOUBAI pointer interval"
    );
    let mut cursor = source_offset;
    let mut tokens = Vec::new();
    let mut source_text = String::new();
    let mut source_text_exact = true;
    let mut line_count = 1usize;
    let mut glyph_count = 0usize;
    let mut resolved_glyph_count = 0usize;
    loop {
        ensure!(
            cursor < record_limit,
            "KOUBAI {role:?} record {record_index:03} is unterminated before its next pointer"
        );
        let opcode = overlay[cursor];
        match opcode {
            0x81 => {
                tokens.push(control_token(
                    cursor,
                    &[opcode],
                    BonusShopTextSourceTokenKind::Terminator,
                ));
                cursor += 1;
                break;
            }
            0x80 => {
                tokens.push(control_token(
                    cursor,
                    &[opcode],
                    BonusShopTextSourceTokenKind::LineBreak,
                ));
                source_text.push('\n');
                line_count += 1;
                cursor += 1;
            }
            0x63 => {
                let raw = overlay
                    .get(cursor..cursor + 3)
                    .context("truncated KOUBAI blank command")?;
                ensure!(
                    raw == [0x63; 3],
                    "KOUBAI {role:?} record {record_index:03} has a malformed blank command"
                );
                tokens.push(control_token(
                    cursor,
                    raw,
                    BonusShopTextSourceTokenKind::Blank,
                ));
                source_text.push(' ');
                cursor += 3;
            }
            page => {
                let raw = overlay
                    .get(cursor..cursor + 3)
                    .context("truncated KOUBAI glyph command")?;
                let column = raw[1];
                let row = raw[2];
                ensure!(
                    page <= 3 && column <= 15 && row <= 15,
                    "KOUBAI {role:?} record {record_index:03} contains an invalid page/column/row glyph command"
                );
                let code = u16::from(page) << 8 | u16::from(row) << 4 | u16::from(column);
                let pixels = packed_glyph_pixels(shop_ui_decoded, code)?;
                let pixel_sha256 = sha256_bytes(&pixels);
                let exact_source_text = verified_pixel_texts.get(&pixel_sha256).cloned();
                if let Some(text) = &exact_source_text {
                    source_text.push_str(text);
                    resolved_glyph_count += 1;
                } else {
                    source_text_exact = false;
                }
                glyph_count += 1;
                tokens.push(BonusShopTextSourceToken {
                    source_offset: hex_offset(cursor),
                    raw_hex: bytes_hex(raw),
                    kind: BonusShopTextSourceTokenKind::Glyph,
                    code: Some(format!("0x{code:04x}")),
                    page: Some(page),
                    column: Some(column),
                    row: Some(row),
                    source_pixel_sha256: Some(pixel_sha256),
                    exact_source_text,
                });
                cursor += 3;
            }
        }
    }
    let raw_record = &overlay[source_offset..cursor];
    let trailing_interval = &overlay[cursor..record_limit];
    Ok(BonusShopTextSourceUnit {
        kind: "Justice Gakuen 2 source-bound bonus-shop text unit".to_string(),
        unit_id: format!("{}-{record_index:03}", role.unit_stem()),
        role,
        record_index,
        pointer_storage_offset: hex_offset(pointer_storage_offset),
        source_offset: hex_offset(source_offset),
        source_runtime_address: format!("0x{:08x}", OVERLAY_RUNTIME_BASE + source_offset as u32),
        source_record_end_offset: hex_offset(cursor),
        source_record_byte_count: raw_record.len(),
        source_record_sha256: sha256_bytes(raw_record),
        source_record_hex: bytes_hex(raw_record),
        source_pointer_interval_end_offset: hex_offset(record_limit),
        source_pointer_interval_byte_count: record_limit - source_offset,
        trailing_interval_byte_count: trailing_interval.len(),
        trailing_interval_sha256: sha256_bytes(trailing_interval),
        trailing_interval_all_zero: trailing_interval.iter().all(|byte| *byte == 0),
        line_count,
        glyph_count,
        resolved_glyph_count,
        unresolved_glyph_count: glyph_count - resolved_glyph_count,
        exact_source_text: source_text_exact.then_some(source_text),
        tokens,
    })
}

fn pointer_target(overlay: &[u8], pointer_offset: usize, spec: ShopTextTableSpec) -> Result<usize> {
    let pointer = read_u32(overlay, pointer_offset)?;
    let target = pointer
        .checked_sub(OVERLAY_RUNTIME_BASE)
        .and_then(|value| usize::try_from(value).ok())
        .with_context(|| {
            format!(
                "KOUBAI {:?} pointer +0x{pointer_offset:04x} precedes the overlay runtime base",
                spec.role
            )
        })?;
    ensure!(
        (spec.record_region[0]..spec.record_region[1]).contains(&target),
        "KOUBAI {:?} pointer +0x{pointer_offset:04x} leaves its record region",
        spec.role
    );
    Ok(target)
}

pub(super) fn packed_glyph_pixels(decoded: &[u8], code: u16) -> Result<Vec<u8>> {
    let tim_data = decoded
        .get(GLYPH_TIM_OFFSET..)
        .context("KOUBAI glyph TIM offset left the decoded source")?;
    let tim = parse_4bpp_prefix(tim_data)?;
    ensure!(
        tim.pixel_width() == 1024 && tim.image_height == 256,
        "KOUBAI glyph atlas geometry changed"
    );
    let page = usize::from(code >> 8);
    let column = usize::from(code & 0x000f);
    let row = usize::from((code >> 4) & 0x000f);
    ensure!(page < 4, "KOUBAI glyph code left the four-page atlas");
    let start_x = (column * GLYPH_CELL_WIDTH) & 0xff;
    let start_y = (row * GLYPH_CELL_HEIGHT) & 0xff;
    let mut packed = Vec::with_capacity(GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT / 2);
    for local_y in 0..GLYPH_CELL_HEIGHT {
        let y = (start_y + local_y) & 0xff;
        for local_x in (0..GLYPH_CELL_WIDTH).step_by(2) {
            let left = indexed_pixel(tim_data, tim, page, start_x, y, local_x);
            let right = indexed_pixel(tim_data, tim, page, start_x, y, local_x + 1);
            packed.push(left | (right << 4));
        }
    }
    Ok(packed)
}

fn indexed_pixel(
    tim_data: &[u8],
    tim: crate::tim::Tim4bpp,
    page: usize,
    start_x: usize,
    y: usize,
    local_x: usize,
) -> u8 {
    let x = page * TEXTURE_PAGE_WIDTH + ((start_x + local_x) & 0xff);
    let byte = tim_data[tim.pixel_offset + y * tim.row_bytes() + x / 2];
    (byte >> (4 * (x & 1))) & 0x0f
}

fn control_token(
    source_offset: usize,
    raw: &[u8],
    kind: BonusShopTextSourceTokenKind,
) -> BonusShopTextSourceToken {
    BonusShopTextSourceToken {
        source_offset: hex_offset(source_offset),
        raw_hex: bytes_hex(raw),
        kind,
        code: None,
        page: None,
        column: None,
        row: None,
        source_pixel_sha256: None,
        exact_source_text: None,
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI pointer at +0x{offset:04x}"))?
            .try_into()?,
    ))
}

fn bytes_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}
