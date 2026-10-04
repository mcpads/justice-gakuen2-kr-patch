use anyhow::{Context, Result, ensure};

use crate::pipeline::{sha256_bytes, sha256_file};

use super::corpus_model::{DialogueCorpusAsset, DialogueCorpusEntry};
use super::format::{hex_address, hex_offset};
use super::parser::DECODED_RUNTIME_BASE;
use super::script_model::{
    DialogueSceneAudit, DialogueSceneAuditConfig, DialogueSceneCommandAudit,
    DialogueSceneMessageReference, DialogueSceneRuntimeEvidence, DialogueSceneSegmentAudit,
};
use super::script_opcode::{command_kind, opcode_spec};
use super::script_topology::runtime_pointer_offset;

const SCENE_ASSET_PATH: &str = "DAT2/MGK04.BIZ";
const SCRIPT_OFFSET: usize = 0x4a000;
const OPCODE_DISPATCH_OFFSET: usize = 0x056c;
const RUNTIME_RAM_SIZE: usize = 0x20_0000;
const RUNTIME_RAM_SHA256: &str = "95e39902a2725f3065994d1ccf20773d377c015cf22cdedc299f55df51e6aee2";
const RUNTIME_FRAME_SHA256: &str =
    "e31b84c38977f66b5410024b30a8b180f0670ed8f44e0aa7ffe7f1c58428c5c8";
const BANK_SELECTOR_ADDRESS: u32 = 0x801f_1854;
const VARIANT_SELECTOR_ADDRESS: u32 = 0x801f_1b59;
const ACTIVE_VARIANT_TABLE_ADDRESS: u32 = 0x801f_1b48;
const ACTIVE_ROUTE_TABLE_ADDRESS: u32 = 0x801f_1b4c;
const INITIAL_COMMAND_ADDRESS: u32 = 0x801f_1b50;
const CURRENT_COMMAND_ADDRESS: u32 = 0x801f_1b54;
const CURRENT_OPCODE_ADDRESS: u32 = 0x801f_1b58;

pub(super) fn audit_admitted_runtime_scene(
    config: &DialogueSceneAuditConfig,
    decoded: &[u8],
    asset: &DialogueCorpusAsset,
    mgame: &[u8],
) -> Result<(DialogueSceneRuntimeEvidence, DialogueSceneAudit)> {
    let evidence = validate_runtime_evidence(config, decoded)?;
    let scene = parse_admitted_scene(
        decoded,
        asset,
        evidence.observed_bank_selector,
        evidence.observed_variant_selector,
        mgame,
    )?;
    Ok((evidence, scene))
}

fn validate_runtime_evidence(
    config: &DialogueSceneAuditConfig,
    decoded: &[u8],
) -> Result<DialogueSceneRuntimeEvidence> {
    let ram = std::fs::read(&config.runtime_ram)
        .with_context(|| format!("failed to read {}", config.runtime_ram.display()))?;
    ensure!(ram.len() == RUNTIME_RAM_SIZE, "unexpected emucap RAM size");
    let ram_sha256 = sha256_bytes(&ram);
    ensure!(
        ram_sha256 == RUNTIME_RAM_SHA256,
        "unsupported emucap RAM SHA-256: {ram_sha256}"
    );
    let frame_sha256 = sha256_file(&config.runtime_frame)?;
    ensure!(
        frame_sha256 == RUNTIME_FRAME_SHA256,
        "unsupported emucap frame SHA-256: {frame_sha256}"
    );
    let runtime_asset = ram_slice(&ram, DECODED_RUNTIME_BASE, decoded.len())?;
    let decoded_asset_ram_difference_count = runtime_asset
        .iter()
        .zip(decoded)
        .filter(|(runtime, source)| runtime != source)
        .count();
    ensure!(
        decoded_asset_ram_difference_count == 0,
        "emucap RAM differs from decoded MGK04"
    );

    let observed_bank_selector = usize::from(read_ram_u8(&ram, BANK_SELECTOR_ADDRESS)?);
    let observed_variant_selector = usize::from(read_ram_u8(&ram, VARIANT_SELECTOR_ADDRESS)?);
    let observed_variant_table = read_ram_u32(&ram, ACTIVE_VARIANT_TABLE_ADDRESS)?;
    let observed_route_table = read_ram_u32(&ram, ACTIVE_ROUTE_TABLE_ADDRESS)?;
    let observed_initial_command = read_ram_u32(&ram, INITIAL_COMMAND_ADDRESS)?;
    let observed_cursor = read_ram_u32(&ram, CURRENT_COMMAND_ADDRESS)?;
    let observed_opcode = read_ram_u8(&ram, CURRENT_OPCODE_ADDRESS)?;
    ensure!(
        observed_bank_selector == 0,
        "unexpected observed bank selector"
    );
    ensure!(
        observed_variant_selector == 0,
        "unexpected observed variant selector"
    );
    ensure!(
        observed_variant_table == 0x8011_a6d8,
        "unexpected observed variant table"
    );
    ensure!(
        observed_route_table == 0x8011_a654,
        "unexpected observed route table"
    );
    ensure!(
        observed_initial_command == 0x8011_a418,
        "unexpected observed initial command"
    );
    ensure!(
        observed_cursor == 0x8011_a438,
        "unexpected first-message cursor"
    );
    ensure!(observed_opcode == 0x1e, "unexpected observed opcode");

    Ok(DialogueSceneRuntimeEvidence {
        ram_path: config.runtime_ram.display().to_string(),
        ram_sha256,
        frame_path: config.runtime_frame.display().to_string(),
        frame_sha256,
        decoded_asset_ram_difference_count,
        observed_bank_selector,
        observed_variant_selector,
        observed_route_table: hex_address(observed_route_table),
        observed_initial_command: hex_address(observed_initial_command),
        observed_cursor_while_first_message_visible: hex_address(observed_cursor),
        observed_opcode: format!("0x{observed_opcode:02x}"),
    })
}

fn parse_admitted_scene(
    decoded: &[u8],
    asset: &DialogueCorpusAsset,
    bank_selector: usize,
    variant_selector: usize,
    mgame: &[u8],
) -> Result<DialogueSceneAudit> {
    let variant_table_offset = runtime_pointer_offset(
        read_u32(decoded, SCRIPT_OFFSET + bank_selector * 4)?,
        decoded.len(),
    )?;
    let route_table_offset = runtime_pointer_offset(
        read_u32(decoded, variant_table_offset + variant_selector * 4)?,
        decoded.len(),
    )?;
    let next_route_table_offset = runtime_pointer_offset(
        read_u32(decoded, variant_table_offset + (variant_selector + 1) * 4)?,
        decoded.len(),
    )?;
    ensure!(
        route_table_offset < next_route_table_offset,
        "admitted route table bounds are reversed"
    );
    ensure!(
        (next_route_table_offset - route_table_offset).is_multiple_of(4),
        "admitted route table is not word-aligned"
    );
    let segment_starts: Vec<_> = (route_table_offset..next_route_table_offset)
        .step_by(4)
        .map(|offset| runtime_pointer_offset(read_u32(decoded, offset)?, decoded.len()))
        .collect::<Result<_>>()?;
    ensure!(!segment_starts.is_empty(), "admitted route has no segments");
    ensure!(
        segment_starts.windows(2).all(|pair| pair[0] < pair[1]),
        "admitted route segments are not strictly ascending"
    );
    ensure!(
        segment_starts.last().copied().unwrap() < route_table_offset,
        "admitted route overlaps its segment table"
    );

    let mut segments = Vec::with_capacity(segment_starts.len());
    let mut message_reference_count = 0usize;
    for (segment_index, &start) in segment_starts.iter().enumerate() {
        let end = segment_starts
            .get(segment_index + 1)
            .copied()
            .unwrap_or(route_table_offset);
        let commands = parse_segment(
            decoded,
            start,
            end,
            bank_selector,
            segment_starts.len(),
            asset,
            mgame,
        )?;
        let segment_message_count = commands
            .iter()
            .filter(|command| command.message.is_some())
            .count();
        message_reference_count += segment_message_count;
        segments.push(DialogueSceneSegmentAudit {
            segment_index,
            decoded_offset: hex_offset(start),
            runtime_address: hex_address(DECODED_RUNTIME_BASE + start as u32),
            command_count: commands.len(),
            message_reference_count: segment_message_count,
            commands,
        });
    }
    ensure!(
        message_reference_count == 39,
        "admitted enrollment message denominator changed"
    );

    Ok(DialogueSceneAudit {
        scene_id: "mgk04-enrollment-introduction".to_string(),
        source_path: SCENE_ASSET_PATH.to_string(),
        bank_selector,
        variant_selector,
        top_level_table_offset: hex_offset(SCRIPT_OFFSET),
        variant_table_offset: hex_offset(variant_table_offset),
        route_table_offset: hex_offset(route_table_offset),
        initial_command_offset: hex_offset(segment_starts[0]),
        speaker_identity: "unresolved".to_string(),
        speaker_evidence: "The emucap frame proves one visible woman presents the first message; exact opcode bytes are retained, but no source-backed character-ID mapping has yet been admitted."
            .to_string(),
        segment_count: segments.len(),
        message_reference_count,
        segments,
    })
}

fn parse_segment(
    decoded: &[u8],
    start: usize,
    end: usize,
    bank_selector: usize,
    segment_count: usize,
    asset: &DialogueCorpusAsset,
    mgame: &[u8],
) -> Result<Vec<DialogueSceneCommandAudit>> {
    ensure!(
        start < end && end <= decoded.len(),
        "invalid segment bounds"
    );
    let mut cursor = start;
    let mut commands = Vec::new();
    while cursor < end {
        let opcode = decoded[cursor];
        let spec = opcode_spec(opcode)
            .with_context(|| format!("invalid opcode 0x{opcode:02x} at +0x{cursor:05x}"))?;
        let command_end = cursor
            .checked_add(spec.width)
            .context("script command end overflow")?;
        ensure!(
            command_end <= end,
            "opcode 0x{opcode:02x} crosses segment boundary at +0x{cursor:05x}"
        );
        let raw = &decoded[cursor..command_end];
        let target_segment_index = (opcode == 0x47).then(|| usize::from(raw[2]));
        if let Some(target) = target_segment_index {
            ensure!(
                target < segment_count,
                "route segment target is out of range"
            );
        }
        let message_index = spec.message_index_bytes.map(|(high_offset, low_offset)| {
            u16::from_be_bytes([raw[high_offset], raw[low_offset]])
        });
        let message = message_index
            .map(|entry_index| message_reference(asset, bank_selector, usize::from(entry_index)))
            .transpose()?;
        let handler_runtime_address =
            read_u32(mgame, OPCODE_DISPATCH_OFFSET + usize::from(opcode) * 4)?;
        commands.push(DialogueSceneCommandAudit {
            decoded_offset: hex_offset(cursor),
            runtime_address: hex_address(DECODED_RUNTIME_BASE + cursor as u32),
            opcode: format!("0x{opcode:02x}"),
            handler_runtime_address: hex_address(handler_runtime_address),
            command_kind: command_kind(opcode).to_string(),
            raw_bytes: raw
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
            target_segment_index,
            message,
        });
        cursor = command_end;
    }
    ensure!(cursor == end, "script segment parse did not end exactly");
    Ok(commands)
}

fn message_reference(
    asset: &DialogueCorpusAsset,
    bank_selector: usize,
    entry_index: usize,
) -> Result<DialogueSceneMessageReference> {
    let bank = asset
        .banks
        .iter()
        .find(|bank| bank.selector_index == bank_selector)
        .with_context(|| format!("missing MGK04 bank {bank_selector}"))?;
    let entry: &DialogueCorpusEntry = bank
        .entries
        .get(entry_index)
        .with_context(|| format!("missing MGK04 bank {bank_selector} entry {entry_index}"))?;
    ensure!(
        entry.entry_index == entry_index,
        "MGK04 corpus entry index changed"
    );
    Ok(DialogueSceneMessageReference {
        bank_selector,
        entry_index,
        coordinate_id: entry.coordinate_id.clone(),
        semantic_source_sha256: entry.semantic_source_sha256.clone(),
        source_markup: entry.source_markup.clone(),
    })
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated word at +0x{offset:x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn ram_offset(address: u32) -> Result<usize> {
    let offset = usize::try_from(address & 0x001f_ffff)?;
    ensure!(offset < RUNTIME_RAM_SIZE, "RAM address is outside main RAM");
    Ok(offset)
}

fn ram_slice(ram: &[u8], address: u32, len: usize) -> Result<&[u8]> {
    let start = ram_offset(address)?;
    ram.get(start..start + len)
        .with_context(|| format!("truncated RAM range at {}", hex_address(address)))
}

fn read_ram_u8(ram: &[u8], address: u32) -> Result<u8> {
    ram_slice(ram, address, 1).map(|bytes| bytes[0])
}

fn read_ram_u32(ram: &[u8], address: u32) -> Result<u32> {
    let bytes = ram_slice(ram, address, 4)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
