#[path = "practical_instruction_graphics/counter_layout.rs"]
pub(crate) mod counter_layout;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::Serialize;

use crate::compression::decompress;
use crate::embedded_tim::{
    EmbeddedTimAudit, decode_embedded_tim_preview, detect_embedded_tim_images,
};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{RgbaImage, read_4bpp_indexed_image_in_prefix, read_4bpp_palette_words_in_prefix};
use crate::tim_preview::write_tim_preview;
use crate::tzz::parse_tzz;

#[path = "practical_instruction_graphics/build.rs"]
mod build;

pub(crate) use build::build_practical_instruction_graphics_from_source;
pub use build::{
    PracticalGameplayPromptBuildReport, PracticalGameplayPromptConsumerReport,
    PracticalGameplayPromptFragmentReport, PracticalGameplayPromptMemberReport,
    PracticalGameplayPromptRecordReport, PracticalInstructionGraphicsBuild,
    PracticalInstructionGraphicsBuildConfig, PracticalInstructionGraphicsBuildReport,
    PracticalInstructionPreservedGraphicReport, PracticalInstructionPromptConsumerBuild,
    build_practical_instruction_graphics,
};

pub const PRACTICAL_INSTRUCTION_PATH: &str = "DAT2/TESTMJ.TIZ";

const OUTPUT_MARKER_FILE: &str = ".practical-instruction-graphics-audit-output";
const OUTPUT_MARKER_TEXT: &str = "justice_gakuen2_practical_instruction_graphics_audit\n";
const REPORT_FILE: &str = "report.json";
const HTML_FILE: &str = "index.html";
const PSX_RAM_BYTE_COUNT: usize = 2 * 1024 * 1024;
const PSX_RAM_RUNTIME_BASE: u32 = 0x8000_0000;
const PSX_VRAM_WIDTH_WORDS: usize = 1024;
const PSX_VRAM_HEIGHT: usize = 512;
const PSX_VRAM_BYTE_COUNT: usize = PSX_VRAM_WIDTH_WORDS * PSX_VRAM_HEIGHT * 2;
const DR_TPAGE_PACKET_BYTE_COUNT: usize = 8;
const SPRT_PACKET_BYTE_COUNT: usize = 20;
const DR_TPAGE_COMMAND: u8 = 0xe1;
const SPRT_COMMAND: u8 = 0x64;
const RUNTIME_TILE_WIDTH: usize = 32;
const RUNTIME_TILE_HEIGHT: usize = 16;

#[derive(Debug, Clone)]
pub struct PracticalInstructionGraphicsAuditConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionGraphicsAuditReport {
    pub kind: String,
    pub source_cue: String,
    pub source_bin_sha256: String,
    pub record_path: String,
    pub stored_size: usize,
    pub stored_sha256: String,
    pub member_count: usize,
    pub embedded_tim_count: usize,
    pub contact_sheet_file: String,
    pub contact_sheet_sha256: String,
    pub consumers: Vec<PracticalInstructionConsumerAudit>,
    pub members: Vec<PracticalInstructionMemberAudit>,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionConsumerAudit {
    pub consumer_path: String,
    pub consumer_sha256: String,
    pub catalog_index: usize,
    pub selector_formula: String,
    pub reachable_member_indices: Vec<usize>,
    pub instruction_span_offset: usize,
    pub instruction_span_size: usize,
    pub instruction_span_sha256: String,
    pub typed_instruction_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionMemberAudit {
    pub member_index: usize,
    pub stored_offset: usize,
    pub compressed_size: usize,
    pub slot_size: usize,
    pub compressed_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub primary_palette_words: Vec<String>,
    pub primary_pixel_population: [usize; 16],
    pub reencoded_size: usize,
    pub compression_headroom: usize,
    pub embedded_tims: Vec<EmbeddedTimAudit>,
}

#[derive(Debug, Clone)]
pub struct PracticalInstructionRuntimeLayoutAuditConfig {
    pub source_cue: PathBuf,
    pub member_index: usize,
    pub ram_dump: PathBuf,
    pub gpu_dump: PathBuf,
    pub runtime_frame: PathBuf,
    pub output: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct PracticalInstructionRuntimeLayoutAuditReport {
    pub kind: String,
    pub source_cue: String,
    pub source_bin_sha256: String,
    pub record_path: String,
    pub member_index: usize,
    pub source_decoded_sha256: String,
    pub ram_dump: String,
    pub ram_dump_sha256: String,
    pub gpu_dump: String,
    pub gpu_dump_sha256: String,
    pub runtime_frame: String,
    pub runtime_frame_sha256: String,
    pub source_texture_resident: bool,
    pub tile_width: usize,
    pub tile_height: usize,
    pub source_tile_columns: usize,
    pub source_tile_range_inclusive: [usize; 2],
    pub source_tile_indices: Vec<usize>,
    pub source_tile_stream_is_consecutive: bool,
    pub raw_packet_count: usize,
    pub duplicate_packet_count: usize,
    pub unique_packet_count: usize,
    pub first_packet_address: String,
    pub last_packet_address: String,
    pub destination_rows: Vec<PracticalInstructionRuntimeRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PracticalInstructionRuntimeRow {
    pub x: i32,
    pub y: i32,
    pub tile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeTilePacket {
    packet_address: u32,
    screen_x: i32,
    screen_y: i32,
    source_tile: usize,
}

pub fn audit_practical_instruction_graphics(
    config: &PracticalInstructionGraphicsAuditConfig,
) -> Result<PracticalInstructionGraphicsAuditReport> {
    prepare_output_directory(&config.output_dir, config.force)?;

    let source = SupportedSourceDisc::open(&config.cue)?;
    let (_, archive) = source.read_record(PRACTICAL_INSTRUCTION_PATH)?;
    let members = parse_tzz(&archive).context("failed to parse TESTMJ.TIZ member table")?;
    let preview_dir = config.output_dir.join("previews");
    std::fs::create_dir_all(&preview_dir)
        .with_context(|| format!("failed to create {}", preview_dir.display()))?;

    let mut member_audits = Vec::with_capacity(members.len());
    let mut primary_previews = Vec::with_capacity(members.len());
    let mut embedded_tim_count = 0usize;
    for member in members {
        let compressed = &archive[member.compressed_range()];
        let decoded = decompress(compressed, true)
            .with_context(|| format!("failed to decode TESTMJ.TIZ member {}", member.index))?;
        let mut tims = detect_embedded_tim_images(&decoded);
        for (tim_index, tim) in tims.iter_mut().enumerate() {
            let rgba = decode_embedded_tim_preview(&decoded, tim).with_context(|| {
                format!(
                    "failed to decode TESTMJ.TIZ member {} TIM {} preview",
                    member.index, tim_index
                )
            })?;
            if tim_index == 0 {
                primary_previews.push(rgba.clone());
            }
            let relative_path =
                format!("previews/member-{:03}-tim-{tim_index:02}.png", member.index);
            let preview_path = config.output_dir.join(&relative_path);
            write_tim_preview(&preview_path, &rgba)?;
            tim.preview_file = relative_path;
            tim.preview_sha256 = sha256_file(&preview_path)?;
        }
        embedded_tim_count += tims.len();
        let primary = tims.first().context("TESTMJ member has no primary TIM")?;
        ensure!(
            primary.offset == 0 && primary.bits_per_pixel == 4,
            "TESTMJ primary TIM is no longer 4-bpp at offset zero"
        );
        let primary_palette_words = read_4bpp_palette_words_in_prefix(&decoded, 0, 0)?
            .into_iter()
            .map(|word| format!("0x{word:04x}"))
            .collect();
        let indexed = read_4bpp_indexed_image_in_prefix(&decoded, 0)?;
        let mut primary_pixel_population = [0usize; 16];
        for pixel in indexed.pixels {
            primary_pixel_population[usize::from(pixel)] += 1;
        }
        let reencoded = crate::compression::compress(&decoded, 0)?;
        ensure!(
            decompress(&reencoded, false)? == decoded,
            "TESTMJ member {} re-encoding changed decoded bytes",
            member.index
        );
        ensure!(
            reencoded.len() <= member.compressed_size,
            "TESTMJ member {} unchanged re-encoding exceeds its stored extent",
            member.index
        );
        member_audits.push(PracticalInstructionMemberAudit {
            member_index: member.index,
            stored_offset: member.offset,
            compressed_size: member.compressed_size,
            slot_size: member.slot_size,
            compressed_sha256: sha256_bytes(compressed),
            decoded_size: decoded.len(),
            decoded_sha256: sha256_bytes(&decoded),
            primary_palette_words,
            primary_pixel_population,
            reencoded_size: reencoded.len(),
            compression_headroom: member.compressed_size - reencoded.len(),
            embedded_tims: tims,
        });
    }

    ensure!(
        primary_previews.len() == member_audits.len(),
        "every TESTMJ member must expose a primary TIM"
    );
    let contact_sheet_file = "contact-sheet.png".to_string();
    let contact_sheet_path = config.output_dir.join(&contact_sheet_file);
    write_contact_sheet(&contact_sheet_path, &primary_previews)?;
    let consumers = audit_consumers(&source)?;
    let reachable_members = consumers
        .iter()
        .flat_map(|consumer| consumer.reachable_member_indices.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        reachable_members == (0..member_audits.len()).collect(),
        "TESTMJ consumer formulas no longer cover every member exactly"
    );

    let report = PracticalInstructionGraphicsAuditReport {
        kind: "justice_gakuen2_practical_instruction_graphics_audit".to_string(),
        source_cue: config.cue.display().to_string(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        record_path: PRACTICAL_INSTRUCTION_PATH.to_string(),
        stored_size: archive.len(),
        stored_sha256: sha256_bytes(&archive),
        member_count: member_audits.len(),
        embedded_tim_count,
        contact_sheet_file,
        contact_sheet_sha256: sha256_file(&contact_sheet_path)?,
        consumers,
        members: member_audits,
    };
    write_report(&config.output_dir, &report)?;
    write_html(&config.output_dir, &report)?;
    Ok(report)
}

pub fn audit_practical_instruction_runtime_layout(
    config: &PracticalInstructionRuntimeLayoutAuditConfig,
) -> Result<PracticalInstructionRuntimeLayoutAuditReport> {
    ensure_safe_output_file(&config.output)?;
    ensure!(
        config.force || !config.output.exists(),
        "practical-instruction runtime layout output exists; pass --force to replace it: {}",
        config.output.display()
    );

    let source = SupportedSourceDisc::open(&config.source_cue)?;
    let (_, archive) = source.read_record(PRACTICAL_INSTRUCTION_PATH)?;
    let members = parse_tzz(&archive).context("failed to parse TESTMJ.TIZ member table")?;
    let member = members.get(config.member_index).with_context(|| {
        format!(
            "practical-instruction member {} is outside the source denominator",
            config.member_index
        )
    })?;
    ensure!(
        member.index == config.member_index,
        "practical-instruction member table index changed"
    );
    let source_decoded = decompress(&archive[member.compressed_range()], true)?;
    let tim = detect_embedded_tim_images(&source_decoded)
        .into_iter()
        .next()
        .context("practical-instruction member has no primary TIM")?;
    ensure!(
        tim.offset == 0
            && tim.bits_per_pixel == 4
            && tim.pixel_width % RUNTIME_TILE_WIDTH == 0
            && tim.pixel_height % RUNTIME_TILE_HEIGHT == 0,
        "practical-instruction primary TIM no longer has aligned 4-bpp tile geometry"
    );
    let source_pixels = read_4bpp_indexed_image_in_prefix(&source_decoded, 0)?;
    let ram_dump = std::fs::read(&config.ram_dump)
        .with_context(|| format!("failed to read {}", config.ram_dump.display()))?;
    let gpu_dump = std::fs::read(&config.gpu_dump)
        .with_context(|| format!("failed to read {}", config.gpu_dump.display()))?;
    ensure!(
        ram_dump.len() == PSX_RAM_BYTE_COUNT,
        "runtime RAM dump must be an exact 2 MiB PS1 RAM image"
    );
    ensure!(
        gpu_dump.len() == PSX_VRAM_BYTE_COUNT,
        "runtime GPU dump must be an exact 1024-word by 512-row PS1 VRAM image"
    );
    ensure_source_texture_resident(&source_pixels.pixels, &tim, &gpu_dump)?;

    let packets = scan_runtime_tile_packets(&ram_dump, &tim)?;
    let (unique_packets, duplicate_packet_count, destination_rows) =
        derive_runtime_layout(&packets)?;
    let first = unique_packets
        .first()
        .context("practical-instruction runtime layout has no first packet")?;
    let last = unique_packets
        .last()
        .context("practical-instruction runtime layout has no last packet")?;
    let source_tile_columns = tim.pixel_width / RUNTIME_TILE_WIDTH;
    let source_tile_indices = unique_packets
        .iter()
        .map(|packet| packet.source_tile)
        .collect::<Vec<_>>();
    let source_tile_stream_is_consecutive = source_tile_indices
        .windows(2)
        .all(|pair| pair[1] == pair[0] + 1);
    let source_tile_min = source_tile_indices
        .iter()
        .copied()
        .min()
        .context("practical-instruction runtime layout has no source tile")?;
    let source_tile_max = source_tile_indices
        .iter()
        .copied()
        .max()
        .context("practical-instruction runtime layout has no source tile")?;

    let report = PracticalInstructionRuntimeLayoutAuditReport {
        kind: "justice_gakuen2_practical_instruction_runtime_layout_audit".to_string(),
        source_cue: config.source_cue.display().to_string(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        record_path: PRACTICAL_INSTRUCTION_PATH.to_string(),
        member_index: config.member_index,
        source_decoded_sha256: sha256_bytes(&source_decoded),
        ram_dump: config.ram_dump.display().to_string(),
        ram_dump_sha256: sha256_bytes(&ram_dump),
        gpu_dump: config.gpu_dump.display().to_string(),
        gpu_dump_sha256: sha256_bytes(&gpu_dump),
        runtime_frame: config.runtime_frame.display().to_string(),
        runtime_frame_sha256: sha256_file(&config.runtime_frame)?,
        source_texture_resident: true,
        tile_width: RUNTIME_TILE_WIDTH,
        tile_height: RUNTIME_TILE_HEIGHT,
        source_tile_columns,
        source_tile_range_inclusive: [source_tile_min, source_tile_max],
        source_tile_indices,
        source_tile_stream_is_consecutive,
        raw_packet_count: packets.len(),
        duplicate_packet_count,
        unique_packet_count: unique_packets.len(),
        first_packet_address: format!("0x{:08x}", first.packet_address),
        last_packet_address: format!("0x{:08x}", last.packet_address),
        destination_rows,
    };
    if let Some(parent) = config.output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    std::fs::write(&config.output, bytes)
        .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

fn ensure_source_texture_resident(
    source_pixels: &[u8],
    tim: &EmbeddedTimAudit,
    gpu_dump: &[u8],
) -> Result<()> {
    ensure!(
        source_pixels.len() == tim.pixel_width * tim.pixel_height,
        "practical-instruction source pixel geometry changed"
    );
    let origin_x = usize::from(tim.image_vram_word_x) * 4;
    let origin_y = usize::from(tim.image_vram_y);
    ensure!(
        origin_x + tim.pixel_width <= PSX_VRAM_WIDTH_WORDS * 4
            && origin_y + tim.pixel_height <= PSX_VRAM_HEIGHT,
        "practical-instruction source TIM leaves PS1 VRAM"
    );
    for y in 0..tim.pixel_height {
        for x in 0..tim.pixel_width {
            ensure!(
                source_pixels[y * tim.pixel_width + x]
                    == vram_four_bit_pixel(gpu_dump, origin_x + x, origin_y + y),
                "runtime GPU does not contain the selected practical-instruction source member"
            );
        }
    }
    Ok(())
}

fn scan_runtime_tile_packets(
    ram_dump: &[u8],
    tim: &EmbeddedTimAudit,
) -> Result<Vec<RuntimeTilePacket>> {
    let tim_origin_x = usize::from(tim.image_vram_word_x) * 4;
    let tim_origin_y = usize::from(tim.image_vram_y);
    let source_tile_columns = tim.pixel_width / RUNTIME_TILE_WIDTH;
    let mut packets = Vec::new();
    for packet_offset in
        (DR_TPAGE_PACKET_BYTE_COUNT..=ram_dump.len() - SPRT_PACKET_BYTE_COUNT).step_by(4)
    {
        if ram_dump[packet_offset + 7] != SPRT_COMMAND {
            continue;
        }
        let draw_mode = read_runtime_u32(ram_dump, packet_offset - 4);
        if (draw_mode >> 24) as u8 != DR_TPAGE_COMMAND {
            continue;
        }
        let tpage = (draw_mode & 0x1ff) as usize;
        if ((tpage >> 7) & 0x3) != 0
            || usize::from(read_runtime_u16(ram_dump, packet_offset + 16)) != RUNTIME_TILE_WIDTH
            || usize::from(read_runtime_u16(ram_dump, packet_offset + 18)) != RUNTIME_TILE_HEIGHT
        {
            continue;
        }
        let texture_page_x = (tpage & 0x0f) * 64 * 4;
        let texture_page_y = ((tpage >> 4) & 1) * 256;
        let packet_x = texture_page_x + usize::from(ram_dump[packet_offset + 12]);
        let packet_y = texture_page_y + usize::from(ram_dump[packet_offset + 13]);
        let Some(source_x) = packet_x.checked_sub(tim_origin_x) else {
            continue;
        };
        let Some(source_y) = packet_y.checked_sub(tim_origin_y) else {
            continue;
        };
        if source_x % RUNTIME_TILE_WIDTH != 0
            || source_y % RUNTIME_TILE_HEIGHT != 0
            || source_x + RUNTIME_TILE_WIDTH > tim.pixel_width
            || source_y + RUNTIME_TILE_HEIGHT > tim.pixel_height
        {
            continue;
        }
        packets.push(RuntimeTilePacket {
            packet_address: PSX_RAM_RUNTIME_BASE + u32::try_from(packet_offset)?,
            screen_x: i32::from(read_runtime_i16(ram_dump, packet_offset + 8)),
            screen_y: i32::from(read_runtime_i16(ram_dump, packet_offset + 10)),
            source_tile: source_y / RUNTIME_TILE_HEIGHT * source_tile_columns
                + source_x / RUNTIME_TILE_WIDTH,
        });
    }
    ensure!(
        !packets.is_empty(),
        "runtime RAM contains no aligned practical-instruction tile packets"
    );
    Ok(packets)
}

fn derive_runtime_layout(
    packets: &[RuntimeTilePacket],
) -> Result<(
    Vec<RuntimeTilePacket>,
    usize,
    Vec<PracticalInstructionRuntimeRow>,
)> {
    let mut by_destination: BTreeMap<(i32, i32), RuntimeTilePacket> = BTreeMap::new();
    for packet in packets {
        let destination = (packet.screen_y, packet.screen_x);
        if let Some(existing) = by_destination.get(&destination) {
            ensure!(
                existing.source_tile == packet.source_tile,
                "one practical-instruction destination reads conflicting source tiles"
            );
        } else {
            by_destination.insert(destination, packet.clone());
        }
    }
    let unique_packets = by_destination.into_values().collect::<Vec<_>>();

    let mut rows: Vec<PracticalInstructionRuntimeRow> = Vec::new();
    for packet in &unique_packets {
        match rows.last_mut() {
            Some(row) if row.y == packet.screen_y => {
                ensure!(
                    packet.screen_x == row.x + i32::try_from(row.tile_count * RUNTIME_TILE_WIDTH)?,
                    "practical-instruction destination row is not left-to-right consecutive"
                );
                row.tile_count += 1;
            }
            Some(row) => {
                ensure!(
                    packet.screen_y == row.y + i32::try_from(RUNTIME_TILE_HEIGHT)?,
                    "practical-instruction destination rows are not vertically consecutive"
                );
                rows.push(PracticalInstructionRuntimeRow {
                    x: packet.screen_x,
                    y: packet.screen_y,
                    tile_count: 1,
                });
            }
            None => rows.push(PracticalInstructionRuntimeRow {
                x: packet.screen_x,
                y: packet.screen_y,
                tile_count: 1,
            }),
        }
    }
    Ok((
        unique_packets,
        packets.len() - rows.iter().map(|row| row.tile_count).sum::<usize>(),
        rows,
    ))
}

fn read_runtime_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_runtime_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_runtime_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("bounded packet word"),
    )
}

fn vram_four_bit_pixel(gpu_dump: &[u8], global_pixel_x: usize, global_y: usize) -> u8 {
    let packed_offset = global_y * PSX_VRAM_WIDTH_WORDS * 2 + global_pixel_x / 2;
    let packed = gpu_dump[packed_offset];
    if global_pixel_x.is_multiple_of(2) {
        packed & 0x0f
    } else {
        packed >> 4
    }
}

fn ensure_safe_output_file(output: &Path) -> Result<()> {
    ensure!(
        !output.as_os_str().is_empty()
            && output != Path::new(".")
            && output != Path::new("..")
            && output != Path::new("/"),
        "refusing unsafe output file: {}",
        output.display()
    );
    Ok(())
}

fn audit_consumers(source: &SupportedSourceDisc) -> Result<Vec<PracticalInstructionConsumerAudit>> {
    const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
    const CATALOG_INDEX: usize = 695;
    let specs = [
        ConsumerSpec {
            path: "DAT1/SIKEN.BIN",
            expected_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
            span: 0x2b1c..0x2b54,
            selector_formula: "test_index * 5 + subject_index",
            reachable_member_indices: (0..30).collect(),
            expected: vec![
                (
                    0x2b1c,
                    Instruction::Lui {
                        rt: Register::V1,
                        immediate: 0x800b,
                    },
                ),
                (
                    0x2b20,
                    Instruction::Lw {
                        rt: Register::V1,
                        base: Register::V1,
                        offset: -26740,
                    },
                ),
                (
                    0x2b28,
                    Instruction::Lbu {
                        rt: Register::V0,
                        base: Register::V1,
                        offset: 12,
                    },
                ),
                (
                    0x2b2c,
                    Instruction::Addiu {
                        rt: Register::A1,
                        rs: Register::ZERO,
                        immediate: CATALOG_INDEX as i16,
                    },
                ),
                (
                    0x2b30,
                    Instruction::Sll {
                        rd: Register::A2,
                        rt: Register::V0,
                        shift: 2,
                    },
                ),
                (
                    0x2b34,
                    Instruction::Addu {
                        rd: Register::A2,
                        rs: Register::A2,
                        rt: Register::V0,
                    },
                ),
                (
                    0x2b40,
                    Instruction::Lbu {
                        rt: Register::V1,
                        base: Register::V1,
                        offset: 13,
                    },
                ),
                (
                    0x2b4c,
                    Instruction::Jalr {
                        rd: Register::RA,
                        rs: Register::V0,
                    },
                ),
                (
                    0x2b50,
                    Instruction::Addu {
                        rd: Register::A2,
                        rs: Register::A2,
                        rt: Register::V1,
                    },
                ),
            ],
        },
        ConsumerSpec {
            path: "DAT1/SIKEN2.BIN",
            expected_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
            span: 0x2114..0x2144,
            selector_formula: "30 + exam_index",
            reachable_member_indices: (30..33).collect(),
            expected: vec![
                (
                    0x211c,
                    Instruction::Addiu {
                        rt: Register::A1,
                        rs: Register::ZERO,
                        immediate: CATALOG_INDEX as i16,
                    },
                ),
                (
                    0x2120,
                    Instruction::Lui {
                        rt: Register::V0,
                        immediate: 0x800b,
                    },
                ),
                (
                    0x2124,
                    Instruction::Lw {
                        rt: Register::V0,
                        base: Register::V0,
                        offset: -26712,
                    },
                ),
                (
                    0x2130,
                    Instruction::Lbu {
                        rt: Register::A2,
                        base: Register::V0,
                        offset: 12,
                    },
                ),
                (
                    0x213c,
                    Instruction::Jalr {
                        rd: Register::RA,
                        rs: Register::V0,
                    },
                ),
                (
                    0x2140,
                    Instruction::Addiu {
                        rt: Register::A2,
                        rs: Register::A2,
                        immediate: 30,
                    },
                ),
            ],
        },
    ];
    specs
        .into_iter()
        .map(|spec| {
            let (_, overlay) = source.read_record(spec.path)?;
            let actual_sha256 = sha256_bytes(&overlay);
            ensure!(
                actual_sha256 == spec.expected_sha256,
                "{} source identity changed",
                spec.path
            );
            for (offset, expected) in &spec.expected {
                let bytes = overlay.get(*offset..*offset + 4).with_context(|| {
                    format!("{} instruction +0x{offset:04x} is truncated", spec.path)
                })?;
                let actual = decode(
                    u32::from_le_bytes(bytes.try_into()?),
                    OVERLAY_RUNTIME_BASE + u32::try_from(*offset)?,
                )
                .with_context(|| {
                    format!("failed to decode {} instruction +0x{offset:04x}", spec.path)
                })?;
                ensure!(
                    actual == *expected,
                    "{} TESTMJ selector instruction +0x{offset:04x} changed: expected {expected:?}, found {actual:?}",
                    spec.path
                );
            }
            let span = overlay.get(spec.span.clone()).with_context(|| {
                format!("{} TESTMJ selector span is truncated", spec.path)
            })?;
            Ok(PracticalInstructionConsumerAudit {
                consumer_path: spec.path.to_string(),
                consumer_sha256: actual_sha256,
                catalog_index: CATALOG_INDEX,
                selector_formula: spec.selector_formula.to_string(),
                reachable_member_indices: spec.reachable_member_indices,
                instruction_span_offset: spec.span.start,
                instruction_span_size: spec.span.len(),
                instruction_span_sha256: sha256_bytes(span),
                typed_instruction_count: spec.expected.len(),
            })
        })
        .collect()
}

struct ConsumerSpec {
    path: &'static str,
    expected_sha256: &'static str,
    span: std::ops::Range<usize>,
    selector_formula: &'static str,
    reachable_member_indices: Vec<usize>,
    expected: Vec<(usize, Instruction)>,
}

fn write_report(output_dir: &Path, report: &PracticalInstructionGraphicsAuditReport) -> Result<()> {
    let output = output_dir.join(REPORT_FILE);
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(&output, bytes).with_context(|| format!("failed to write {}", output.display()))
}

fn write_html(output_dir: &Path, report: &PracticalInstructionGraphicsAuditReport) -> Result<()> {
    let mut cards = String::new();
    for member in &report.members {
        if member.embedded_tims.is_empty() {
            writeln!(
                cards,
                "<article><h2>member-{0:03}</h2><p>No embedded TIM</p></article>",
                member.member_index
            )?;
            continue;
        }
        for (tim_index, tim) in member.embedded_tims.iter().enumerate() {
            writeln!(
                cards,
                "<article><h2>member-{member:03} / TIM {tim_index}</h2><img src=\"{preview}\" alt=\"member-{member:03} TIM {tim_index}\"><p>{bpp}-bpp, {width}x{height}, VRAM ({vram_x}, {vram_y}), CLUT ({clut_x}, {clut_y}), palettes {palettes}</p></article>",
                member = member.member_index,
                preview = tim.preview_file,
                bpp = tim.bits_per_pixel,
                width = tim.pixel_width,
                height = tim.pixel_height,
                vram_x = tim.image_vram_word_x,
                vram_y = tim.image_vram_y,
                clut_x = tim.clut_vram_x,
                clut_y = tim.clut_vram_y,
                palettes = tim.palette_count,
            )?;
        }
    }
    let html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>TESTMJ.TIZ member inventory</title><style>body{{font-family:system-ui;background:#161616;color:#eee;margin:24px}}main{{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}}article{{background:#252525;padding:12px;border-radius:8px}}img{{width:100%;height:auto;image-rendering:pixelated;background:#444}}h1,h2{{margin:.2em 0}}p{{color:#ccc}}.sheet{{max-width:100%;width:auto}}</style><h1>TESTMJ.TIZ member inventory</h1><p>{members} members, {tims} embedded TIMs. Exact identities are in <a href=\"report.json\">report.json</a>.</p><p><img class=\"sheet\" src=\"{sheet}\" alt=\"row-major TESTMJ member contact sheet\"></p><main>{cards}</main></html>\n",
        members = report.member_count,
        tims = report.embedded_tim_count,
        sheet = report.contact_sheet_file,
    );
    let output = output_dir.join(HTML_FILE);
    std::fs::write(&output, html).with_context(|| format!("failed to write {}", output.display()))
}

fn write_contact_sheet(path: &Path, images: &[RgbaImage]) -> Result<()> {
    ensure!(!images.is_empty(), "TESTMJ contact sheet has no images");
    const COLUMNS: usize = 6;
    const GAP: usize = 4;
    let cell_width = images.iter().map(|image| image.width).max().unwrap_or(0);
    let cell_height = images.iter().map(|image| image.height).max().unwrap_or(0);
    let rows = images.len().div_ceil(COLUMNS);
    let width = COLUMNS * cell_width + (COLUMNS - 1) * GAP;
    let height = rows * cell_height + (rows - 1) * GAP;
    let mut pixels = vec![0x28u8; width * height * 4];
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 0xff;
    }
    for (index, image) in images.iter().enumerate() {
        let origin_x = index % COLUMNS * (cell_width + GAP);
        let origin_y = index / COLUMNS * (cell_height + GAP);
        for y in 0..image.height {
            let source_start = y * image.width * 4;
            let target_start = ((origin_y + y) * width + origin_x) * 4;
            pixels[target_start..target_start + image.width * 4]
                .copy_from_slice(&image.pixels[source_start..source_start + image.width * 4]);
        }
    }
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    Ok(())
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    ensure_safe_output_path(output_dir)?;
    let marker = output_dir.join(OUTPUT_MARKER_FILE);
    if output_dir.exists() {
        if !force {
            bail!(
                "practical-instruction graphics audit output exists; pass --force to replace it: {}",
                output_dir.display()
            );
        }
        let marker_text = std::fs::read_to_string(&marker).with_context(|| {
            format!(
                "refusing to replace an unowned output directory without {OUTPUT_MARKER_FILE}: {}",
                output_dir.display()
            )
        })?;
        ensure!(
            marker_text == OUTPUT_MARKER_TEXT,
            "refusing to replace output directory with an unknown ownership marker: {}",
            output_dir.display()
        );
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    std::fs::write(output_dir.join(OUTPUT_MARKER_FILE), OUTPUT_MARKER_TEXT)?;
    Ok(())
}

fn ensure_safe_output_path(output_dir: &Path) -> Result<()> {
    ensure!(
        !output_dir.as_os_str().is_empty()
            && output_dir != Path::new(".")
            && output_dir != Path::new("..")
            && output_dir != Path::new("/"),
        "refusing unsafe output directory: {}",
        output_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod runtime_layout_tests {
    use super::{PracticalInstructionRuntimeRow, RuntimeTilePacket, derive_runtime_layout};

    #[test]
    fn runtime_layout_deduplicates_packet_buffers_and_preserves_rows() {
        let mut packets = Vec::new();
        for packet in [
            RuntimeTilePacket {
                packet_address: 0x801c_6040,
                screen_x: 32,
                screen_y: 28,
                source_tile: 5,
            },
            RuntimeTilePacket {
                packet_address: 0x801c_6078,
                screen_x: 64,
                screen_y: 28,
                source_tile: 6,
            },
            RuntimeTilePacket {
                packet_address: 0x801c_60b0,
                screen_x: 32,
                screen_y: 44,
                source_tile: 7,
            },
        ] {
            packets.push(packet.clone());
            packets.push(RuntimeTilePacket {
                packet_address: packet.packet_address + 0x1c,
                ..packet
            });
        }

        let (unique, duplicate_count, rows) = derive_runtime_layout(&packets).unwrap();

        assert_eq!(unique.len(), 3);
        assert_eq!(duplicate_count, 3);
        assert_eq!(
            rows,
            vec![
                PracticalInstructionRuntimeRow {
                    x: 32,
                    y: 28,
                    tile_count: 2,
                },
                PracticalInstructionRuntimeRow {
                    x: 32,
                    y: 44,
                    tile_count: 1,
                },
            ]
        );
    }

    #[test]
    fn runtime_layout_preserves_display_order_across_source_tile_gaps() {
        let packets = vec![
            RuntimeTilePacket {
                packet_address: 0x801c_6040,
                screen_x: 32,
                screen_y: 28,
                source_tile: 10,
            },
            RuntimeTilePacket {
                packet_address: 0x801c_6078,
                screen_x: 64,
                screen_y: 28,
                source_tile: 14,
            },
        ];

        let (unique, duplicate_count, rows) = derive_runtime_layout(&packets).unwrap();

        assert_eq!(
            unique
                .iter()
                .map(|packet| packet.source_tile)
                .collect::<Vec<_>>(),
            vec![10, 14]
        );
        assert_eq!(duplicate_count, 0);
        assert_eq!(
            rows,
            vec![PracticalInstructionRuntimeRow {
                x: 32,
                y: 28,
                tile_count: 2,
            }]
        );
    }
}
