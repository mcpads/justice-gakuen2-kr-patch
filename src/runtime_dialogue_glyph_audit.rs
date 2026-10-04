use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::pipeline::sha256_file;
use crate::tim::parse_4bpp_prefix;

const VRAM_WIDTH_WORDS: usize = 1024;
const VRAM_HEIGHT: usize = 512;
const VRAM_WORD_BYTES: usize = 2;
const VRAM_ROW_STRIDE_BYTES: usize = VRAM_WIDTH_WORDS * VRAM_WORD_BYTES;
const PSX_RAM_BYTES: usize = 2 * 1024 * 1024;
const PSX_RAM_RUNTIME_BASE: u32 = 0x8000_0000;
const DIALOGUE_RUNTIME_IMAGE_BASE: u32 = 0x800d_0000;
const GLYPH_WIDTH_PIXELS: usize = 20;
const GLYPH_HEIGHT_PIXELS: usize = 20;
const GLYPH_WIDTH_WORDS: usize = GLYPH_WIDTH_PIXELS / 4;
const GLYPH_ROW_BYTES: usize = GLYPH_WIDTH_WORDS * VRAM_WORD_BYTES;
const GLYPH_BYTES: usize = GLYPH_ROW_BYTES * GLYPH_HEIGHT_PIXELS;
const SPRT_PACKET_BYTES: usize = 20;
const DR_TPAGE_PACKET_BYTES: usize = 8;
const SPRT_COMMAND: u8 = 0x64;
const DR_TPAGE_COMMAND: u8 = 0xe1;

#[derive(Debug, Clone)]
pub struct RuntimeDialogueGlyphAuditConfig {
    pub source_decoded: PathBuf,
    pub active_decoded: Option<PathBuf>,
    pub allocation: PathBuf,
    pub codebook: PathBuf,
    pub gpu_dump: PathBuf,
    pub ram_dump: PathBuf,
    pub source_path: String,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDialogueGlyphAuditReport {
    pub kind: String,
    pub implementation: String,
    pub source_path: String,
    pub source_decoded_sha256: String,
    pub active_decoded_sha256: Option<String>,
    pub allocation_sha256: String,
    pub codebook_sha256: String,
    pub gpu_dump_sha256: String,
    pub ram_dump_sha256: String,
    pub vram_width_words: usize,
    pub vram_height: usize,
    pub vram_row_stride_bytes: usize,
    pub glyph_width_pixels: usize,
    pub glyph_height_pixels: usize,
    pub glyph_width_words: usize,
    pub fixed_source_cell_count: usize,
    pub nonblank_source_cell_count: usize,
    pub codebook_labeled_source_cell_count: usize,
    pub allocation_assignment_count: usize,
    pub source_binding_complete: bool,
    pub active_runtime_base: Option<String>,
    pub active_runtime_image_byte_count: Option<usize>,
    pub active_runtime_image_matches_active_decoded: Option<bool>,
    pub active_runtime_image_difference_count: Option<usize>,
    pub stride_aware_vram_scan_complete: bool,
    pub resident_location_count: usize,
    pub resident_source_code_occurrence_count: usize,
    pub allocation_changed_code_resident_pixel_match_count: usize,
    pub draw_consumer_count: usize,
    pub draw_packet_copy_count: usize,
    pub allocation_changed_code_draw_pixel_match_count: usize,
    pub source_script_draw_consumer_count: usize,
    pub allocation_changed_code_pixels_resident: bool,
    pub allocation_changed_code_pixels_in_draw_packets: bool,
    pub source_asset_provenance_established: bool,
    pub resident_matches: Vec<RuntimeGlyphResidencyMatch>,
    pub draw_consumers: Vec<RuntimeGlyphDrawConsumer>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeGlyphResidencyMatch {
    pub vram_word_x: usize,
    pub vram_y: usize,
    pub width_words: usize,
    pub height: usize,
    pub pixel_sha256: String,
    pub source_glyphs: Vec<RuntimeSourceGlyph>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeGlyphDrawConsumer {
    pub packet_addresses: Vec<String>,
    pub screen_x: i32,
    pub screen_y: i32,
    pub sprite_width_pixels: usize,
    pub sprite_height_pixels: usize,
    pub cell_offset_x_pixels: usize,
    pub cell_offset_y_pixels: usize,
    pub texture_vram_word_x: usize,
    pub texture_vram_y: usize,
    pub texture_page_word_x: usize,
    pub texture_page_y: usize,
    pub texture_u: u8,
    pub texture_v: u8,
    pub clut: String,
    pub draw_mode: String,
    pub pixel_sha256: String,
    pub source_glyphs: Vec<RuntimeSourceGlyph>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSourceGlyph {
    pub source_code: String,
    pub source_text: Option<String>,
    pub source_semantic_id: Option<String>,
    pub codebook_status: Option<String>,
    pub current_character: Option<String>,
    pub same_code_allocation_relation: String,
    pub source_text_uses_japanese_script: bool,
}

#[derive(Debug, Deserialize)]
struct AllocationReportInput {
    assets: Vec<AllocationAssetInput>,
}

#[derive(Debug, Deserialize)]
struct AllocationAssetInput {
    source_path: String,
    fixed_cell_count: usize,
    assignments: Vec<AllocationAssignmentInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct AllocationAssignmentInput {
    character: String,
    code: String,
    source_glyph_reused: bool,
    requires_glyph_install: bool,
    target_was_fixed_source_cell: bool,
    target_source_cell_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodebookInput {
    entries: Vec<CodebookEntryInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodebookEntryInput {
    pixel_sha256: String,
    meaning: CodebookMeaningInput,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CodebookMeaningInput {
    Character { text: String },
    Symbol { id: String, display: String },
}

#[derive(Debug, Clone)]
struct SourceGlyphIndex {
    by_digest: HashMap<[u8; 32], Vec<RuntimeSourceGlyph>>,
    fixed_cell_count: usize,
    nonblank_cell_count: usize,
    codebook_labeled_cell_count: usize,
    allocation_assignment_count: usize,
}

#[derive(Debug, Clone)]
struct ActiveRuntimeImageBinding {
    runtime_base: String,
    byte_count: usize,
    matches: bool,
    difference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DrawConsumerKey {
    screen_x: i32,
    screen_y: i32,
    sprite_width_pixels: usize,
    sprite_height_pixels: usize,
    cell_offset_x_pixels: usize,
    cell_offset_y_pixels: usize,
    texture_vram_word_x: usize,
    texture_vram_y: usize,
    texture_page_word_x: usize,
    texture_page_y: usize,
    texture_u: u8,
    texture_v: u8,
    clut: u16,
    draw_mode: u32,
    digest: [u8; 32],
}

pub fn audit_runtime_dialogue_glyphs(
    config: &RuntimeDialogueGlyphAuditConfig,
) -> Result<RuntimeDialogueGlyphAuditReport> {
    let source_decoded = read_file(&config.source_decoded)?;
    let active_decoded = config
        .active_decoded
        .as_ref()
        .map(|path| read_file(path))
        .transpose()?;
    let allocation_bytes = read_file(&config.allocation)?;
    let codebook_bytes = read_file(&config.codebook)?;
    let gpu_dump = read_file(&config.gpu_dump)?;
    let ram_dump = read_file(&config.ram_dump)?;
    ensure!(
        gpu_dump.len() == VRAM_ROW_STRIDE_BYTES * VRAM_HEIGHT,
        "GPU dump must be an exact 1024-word by 512-row PS1 VRAM image"
    );
    ensure!(
        ram_dump.len() == PSX_RAM_BYTES,
        "RAM dump must be an exact 2 MiB PS1 RAM image"
    );
    let active_runtime_binding = active_decoded
        .as_deref()
        .map(|active| audit_active_runtime_image(&ram_dump, active))
        .transpose()?;

    let allocation: AllocationReportInput = serde_json::from_slice(&allocation_bytes)
        .with_context(|| format!("failed to parse {}", config.allocation.display()))?;
    let codebook: CodebookInput = serde_json::from_slice(&codebook_bytes)
        .with_context(|| format!("failed to parse {}", config.codebook.display()))?;
    let allocation_asset = allocation
        .assets
        .iter()
        .find(|asset| asset.source_path == config.source_path)
        .with_context(|| {
            format!(
                "{} is absent from {}",
                config.source_path,
                config.allocation.display()
            )
        })?;
    let source_index = build_source_glyph_index(&source_decoded, allocation_asset, &codebook)?;
    let resident_matches = scan_vram_residency(&gpu_dump, &source_index.by_digest);
    let draw_consumers = scan_draw_consumers(&ram_dump, &gpu_dump, &source_index.by_digest);

    let resident_source_code_occurrence_count = resident_matches
        .iter()
        .map(|matched| matched.source_glyphs.len())
        .sum();
    let allocation_changed_code_resident_pixel_match_count = resident_matches
        .iter()
        .flat_map(|matched| &matched.source_glyphs)
        .filter(|glyph| glyph.same_code_allocation_relation == "same_code_repurposed_in_allocation")
        .count();
    let draw_packet_copy_count = draw_consumers
        .iter()
        .map(|consumer| consumer.packet_addresses.len())
        .sum();
    let allocation_changed_code_draw_pixel_match_count = draw_consumers
        .iter()
        .filter(|consumer| {
            consumer.source_glyphs.iter().any(|glyph| {
                glyph.same_code_allocation_relation == "same_code_repurposed_in_allocation"
            })
        })
        .count();
    let source_script_draw_consumer_count = draw_consumers
        .iter()
        .filter(|consumer| {
            consumer
                .source_glyphs
                .iter()
                .any(|glyph| glyph.source_text_uses_japanese_script)
        })
        .count();

    let report = RuntimeDialogueGlyphAuditReport {
        kind: "runtime_dialogue_glyph_audit".to_string(),
        implementation: "stride-aware PS1 VRAM source-cell scan plus adjacent DR_TPAGE/SPRT packet decoding".to_string(),
        source_path: config.source_path.clone(),
        source_decoded_sha256: sha256_file(&config.source_decoded)?,
        active_decoded_sha256: config
            .active_decoded
            .as_ref()
            .map(|path| sha256_file(path))
            .transpose()?,
        allocation_sha256: sha256_file(&config.allocation)?,
        codebook_sha256: sha256_file(&config.codebook)?,
        gpu_dump_sha256: sha256_file(&config.gpu_dump)?,
        ram_dump_sha256: sha256_file(&config.ram_dump)?,
        vram_width_words: VRAM_WIDTH_WORDS,
        vram_height: VRAM_HEIGHT,
        vram_row_stride_bytes: VRAM_ROW_STRIDE_BYTES,
        glyph_width_pixels: GLYPH_WIDTH_PIXELS,
        glyph_height_pixels: GLYPH_HEIGHT_PIXELS,
        glyph_width_words: GLYPH_WIDTH_WORDS,
        fixed_source_cell_count: source_index.fixed_cell_count,
        nonblank_source_cell_count: source_index.nonblank_cell_count,
        codebook_labeled_source_cell_count: source_index.codebook_labeled_cell_count,
        allocation_assignment_count: source_index.allocation_assignment_count,
        source_binding_complete: true,
        active_runtime_base: active_runtime_binding
            .as_ref()
            .map(|binding| binding.runtime_base.clone()),
        active_runtime_image_byte_count: active_runtime_binding
            .as_ref()
            .map(|binding| binding.byte_count),
        active_runtime_image_matches_active_decoded: active_runtime_binding
            .as_ref()
            .map(|binding| binding.matches),
        active_runtime_image_difference_count: active_runtime_binding
            .as_ref()
            .map(|binding| binding.difference_count),
        stride_aware_vram_scan_complete: true,
        resident_location_count: resident_matches.len(),
        resident_source_code_occurrence_count,
        allocation_changed_code_resident_pixel_match_count,
        draw_consumer_count: draw_consumers.len(),
        draw_packet_copy_count,
        allocation_changed_code_draw_pixel_match_count,
        source_script_draw_consumer_count,
        allocation_changed_code_pixels_resident:
            allocation_changed_code_resident_pixel_match_count > 0,
        allocation_changed_code_pixels_in_draw_packets:
            allocation_changed_code_draw_pixel_match_count > 0,
        source_asset_provenance_established: false,
        resident_matches,
        draw_consumers,
        limitations: vec![
            "A VRAM residency match proves byte-exact source pixels at one frame, not that the rectangle was submitted for display.".to_string(),
            "A draw consumer requires an adjacent 4-bpp DR_TPAGE and SPRT packet plus a byte-exact source-cell match; a RAM packet may still belong to an inactive double buffer.".to_string(),
            "The allocation relation describes what the same numeric code means in one selected dialogue asset. Pixel equality does not prove that the runtime sprite came from that asset or used that logical code.".to_string(),
            "This report covers one exact RAM/GPU snapshot and does not claim whole-route or whole-game presentation coverage.".to_string(),
        ],
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

fn audit_active_runtime_image(
    ram_dump: &[u8],
    active_decoded: &[u8],
) -> Result<ActiveRuntimeImageBinding> {
    let ram_offset = usize::try_from(DIALOGUE_RUNTIME_IMAGE_BASE - PSX_RAM_RUNTIME_BASE)
        .context("dialogue runtime base does not fit the RAM dump")?;
    let runtime = ram_dump
        .get(ram_offset..ram_offset + active_decoded.len())
        .context("active decoded dialogue image exceeds the RAM dump")?;
    let difference_count = runtime
        .iter()
        .zip(active_decoded)
        .filter(|(runtime, decoded)| runtime != decoded)
        .count();
    Ok(ActiveRuntimeImageBinding {
        runtime_base: format!("0x{DIALOGUE_RUNTIME_IMAGE_BASE:08x}"),
        byte_count: active_decoded.len(),
        matches: difference_count == 0,
        difference_count,
    })
}

fn build_source_glyph_index(
    source_decoded: &[u8],
    allocation: &AllocationAssetInput,
    codebook: &CodebookInput,
) -> Result<SourceGlyphIndex> {
    let tim = parse_4bpp_prefix(source_decoded)?;
    ensure!(
        tim.pixel_width() == GLYPH_WIDTH_PIXELS,
        "source dialogue TIM is not one 20-pixel glyph column"
    );
    ensure!(
        tim.row_bytes() == GLYPH_ROW_BYTES,
        "source dialogue TIM has an unexpected glyph row width"
    );
    ensure!(
        tim.image_height.is_multiple_of(GLYPH_HEIGHT_PIXELS),
        "source dialogue TIM contains a partial glyph cell"
    );
    let fixed_cell_count = tim.image_height / GLYPH_HEIGHT_PIXELS;
    ensure!(
        fixed_cell_count == allocation.fixed_cell_count,
        "source dialogue TIM and allocation disagree on fixed-cell count"
    );
    let source_pixel_end = tim
        .pixel_offset
        .checked_add(fixed_cell_count * GLYPH_BYTES)
        .context("source dialogue TIM cell range overflow")?;
    ensure!(
        source_pixel_end == tim.total_size,
        "source dialogue TIM pixels are not contiguous 20x20 cells"
    );

    let codebook_by_hash = codebook
        .entries
        .iter()
        .map(|entry| (entry.pixel_sha256.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let assignments_by_code = allocation
        .assignments
        .iter()
        .map(|assignment| Ok((parse_code(&assignment.code)?, assignment)))
        .collect::<Result<HashMap<_, _>>>()?;
    ensure!(
        assignments_by_code.len() == allocation.assignments.len(),
        "allocation assigns one fixed code more than once"
    );

    let mut by_digest = HashMap::<[u8; 32], Vec<RuntimeSourceGlyph>>::new();
    let mut nonblank_cell_count = 0usize;
    let mut codebook_labeled_cell_count = 0usize;
    for code in 0..fixed_cell_count {
        let start = tim.pixel_offset + code * GLYPH_BYTES;
        let cell = &source_decoded[start..start + GLYPH_BYTES];
        let digest = sha256_digest(cell);
        let pixel_sha256 = hex_digest(digest);
        let assignment = assignments_by_code.get(&code).copied();
        if let Some(assignment) = assignment
            && assignment.target_was_fixed_source_cell
        {
            ensure!(
                assignment.target_source_cell_sha256.as_deref() == Some(pixel_sha256.as_str()),
                "allocation source-cell binding changed for code 0x{code:04x}"
            );
        }
        if cell.iter().all(|byte| *byte == 0) {
            continue;
        }
        nonblank_cell_count += 1;
        let codebook_entry = codebook_by_hash.get(pixel_sha256.as_str()).copied();
        if codebook_entry.is_some() {
            codebook_labeled_cell_count += 1;
        }
        let (source_text, source_semantic_id, codebook_status) = match codebook_entry {
            Some(entry) => match &entry.meaning {
                CodebookMeaningInput::Character { text } => {
                    (Some(text.clone()), None, Some(entry.status.clone()))
                }
                CodebookMeaningInput::Symbol { id, display } => (
                    Some(display.clone()),
                    Some(id.clone()),
                    Some(entry.status.clone()),
                ),
            },
            None => (None, None, None),
        };
        let same_code_allocation_relation = match assignment {
            Some(assignment) if assignment.requires_glyph_install => {
                "same_code_repurposed_in_allocation"
            }
            Some(assignment) if assignment.source_glyph_reused => {
                "same_code_preserves_source_mapping"
            }
            Some(_) => "same_code_assigned_without_source_reuse",
            None => "source_code_not_assigned",
        };
        by_digest
            .entry(digest)
            .or_default()
            .push(RuntimeSourceGlyph {
                source_code: format!("0x{code:04x}"),
                source_text_uses_japanese_script: source_text
                    .as_deref()
                    .is_some_and(uses_japanese_script),
                source_text,
                source_semantic_id,
                codebook_status,
                current_character: assignment.map(|assignment| assignment.character.clone()),
                same_code_allocation_relation: same_code_allocation_relation.to_string(),
            });
    }
    for glyphs in by_digest.values_mut() {
        glyphs.sort_by(|left, right| left.source_code.cmp(&right.source_code));
    }
    Ok(SourceGlyphIndex {
        by_digest,
        fixed_cell_count,
        nonblank_cell_count,
        codebook_labeled_cell_count,
        allocation_assignment_count: allocation.assignments.len(),
    })
}

fn scan_vram_residency(
    gpu_dump: &[u8],
    source_glyphs: &HashMap<[u8; 32], Vec<RuntimeSourceGlyph>>,
) -> Vec<RuntimeGlyphResidencyMatch> {
    let mut matches = Vec::new();
    for y in 0..=VRAM_HEIGHT - GLYPH_HEIGHT_PIXELS {
        for word_x in 0..=VRAM_WIDTH_WORDS - GLYPH_WIDTH_WORDS {
            let digest = hash_vram_glyph(gpu_dump, word_x, y);
            let Some(glyphs) = source_glyphs.get(&digest) else {
                continue;
            };
            matches.push(RuntimeGlyphResidencyMatch {
                vram_word_x: word_x,
                vram_y: y,
                width_words: GLYPH_WIDTH_WORDS,
                height: GLYPH_HEIGHT_PIXELS,
                pixel_sha256: hex_digest(digest),
                source_glyphs: glyphs.clone(),
            });
        }
    }
    matches
}

fn scan_draw_consumers(
    ram_dump: &[u8],
    gpu_dump: &[u8],
    source_glyphs: &HashMap<[u8; 32], Vec<RuntimeSourceGlyph>>,
) -> Vec<RuntimeGlyphDrawConsumer> {
    let mut grouped = BTreeMap::<DrawConsumerKey, BTreeSet<u32>>::new();
    for packet_offset in (DR_TPAGE_PACKET_BYTES..=ram_dump.len() - SPRT_PACKET_BYTES).step_by(4) {
        if ram_dump[packet_offset + 7] != SPRT_COMMAND {
            continue;
        }
        let draw_mode = read_u32(ram_dump, packet_offset - 4);
        if (draw_mode >> 24) as u8 != DR_TPAGE_COMMAND {
            continue;
        }
        let tpage = (draw_mode & 0x1ff) as usize;
        let texture_depth = (tpage >> 7) & 0x3;
        if texture_depth != 0 {
            continue;
        }
        let texture_page_word_x = (tpage & 0x0f) * 64;
        let texture_page_y = ((tpage >> 4) & 1) * 256;
        let texture_u = ram_dump[packet_offset + 12];
        let texture_v = ram_dump[packet_offset + 13];
        let sprite_width_pixels = usize::from(read_u16(ram_dump, packet_offset + 16));
        let sprite_height_pixels = usize::from(read_u16(ram_dump, packet_offset + 18));
        if sprite_width_pixels < GLYPH_WIDTH_PIXELS
            || sprite_height_pixels < GLYPH_HEIGHT_PIXELS
            || sprite_width_pixels > 256
            || sprite_height_pixels > 256
        {
            continue;
        }
        let screen_x = i32::from(read_i16(ram_dump, packet_offset + 8));
        let screen_y = i32::from(read_i16(ram_dump, packet_offset + 10));
        let clut = read_u16(ram_dump, packet_offset + 14);
        for cell_offset_y_pixels in
            (0..=sprite_height_pixels - GLYPH_HEIGHT_PIXELS).step_by(GLYPH_HEIGHT_PIXELS)
        {
            for cell_offset_x_pixels in
                (0..=sprite_width_pixels - GLYPH_WIDTH_PIXELS).step_by(GLYPH_WIDTH_PIXELS)
            {
                let cell_u = usize::from(texture_u) + cell_offset_x_pixels;
                let cell_v = usize::from(texture_v) + cell_offset_y_pixels;
                if !cell_u.is_multiple_of(4) {
                    continue;
                }
                let texture_vram_word_x = texture_page_word_x + cell_u / 4;
                let texture_vram_y = texture_page_y + cell_v;
                if texture_vram_word_x + GLYPH_WIDTH_WORDS > VRAM_WIDTH_WORDS
                    || texture_vram_y + GLYPH_HEIGHT_PIXELS > VRAM_HEIGHT
                {
                    continue;
                }
                let digest = hash_vram_glyph(gpu_dump, texture_vram_word_x, texture_vram_y);
                if !source_glyphs.contains_key(&digest) {
                    continue;
                }
                let key = DrawConsumerKey {
                    screen_x: screen_x + cell_offset_x_pixels as i32,
                    screen_y: screen_y + cell_offset_y_pixels as i32,
                    sprite_width_pixels,
                    sprite_height_pixels,
                    cell_offset_x_pixels,
                    cell_offset_y_pixels,
                    texture_vram_word_x,
                    texture_vram_y,
                    texture_page_word_x,
                    texture_page_y,
                    texture_u,
                    texture_v,
                    clut,
                    draw_mode,
                    digest,
                };
                grouped
                    .entry(key)
                    .or_default()
                    .insert(PSX_RAM_RUNTIME_BASE + packet_offset as u32);
            }
        }
    }

    grouped
        .into_iter()
        .map(|(key, packet_addresses)| RuntimeGlyphDrawConsumer {
            packet_addresses: packet_addresses
                .into_iter()
                .map(|address| format!("0x{address:08x}"))
                .collect(),
            screen_x: key.screen_x,
            screen_y: key.screen_y,
            sprite_width_pixels: key.sprite_width_pixels,
            sprite_height_pixels: key.sprite_height_pixels,
            cell_offset_x_pixels: key.cell_offset_x_pixels,
            cell_offset_y_pixels: key.cell_offset_y_pixels,
            texture_vram_word_x: key.texture_vram_word_x,
            texture_vram_y: key.texture_vram_y,
            texture_page_word_x: key.texture_page_word_x,
            texture_page_y: key.texture_page_y,
            texture_u: key.texture_u,
            texture_v: key.texture_v,
            clut: format!("0x{:04x}", key.clut),
            draw_mode: format!("0x{:08x}", key.draw_mode),
            pixel_sha256: hex_digest(key.digest),
            source_glyphs: source_glyphs[&key.digest].clone(),
        })
        .collect()
}

fn hash_vram_glyph(gpu_dump: &[u8], word_x: usize, y: usize) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for row in y..y + GLYPH_HEIGHT_PIXELS {
        let start = row * VRAM_ROW_STRIDE_BYTES + word_x * VRAM_WORD_BYTES;
        hasher.update(&gpu_dump[start..start + GLYPH_ROW_BYTES]);
    }
    hasher.finalize().into()
}

fn parse_code(code: &str) -> Result<usize> {
    let digits = code
        .strip_prefix("0x")
        .with_context(|| format!("allocation code is not hexadecimal: {code}"))?;
    usize::from_str_radix(digits, 16)
        .with_context(|| format!("allocation code is not hexadecimal: {code}"))
}

fn uses_japanese_script(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{f900}'..='\u{faff}'
        )
    })
}

fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_digest(digest: [u8; 32]) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated packet"),
    )
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated packet"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated packet"),
    )
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))
}

fn write_report(path: &Path, report: &RuntimeDialogueGlyphAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
#[path = "runtime_dialogue_glyph_audit_tests.rs"]
mod tests;
