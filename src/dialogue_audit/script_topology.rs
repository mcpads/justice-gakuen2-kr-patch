use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::corpus_model::{DialogueCorpusAsset, DialogueCorpusEntry};
use super::format::{hex_address, hex_offset};
use super::parser::DECODED_RUNTIME_BASE;
use super::script_model::{
    DialogueScriptAssetAudit, DialogueScriptBankTableAudit, DialogueScriptCommandAudit,
    DialogueScriptMessageBinding, DialogueScriptRouteAudit, DialogueScriptRouteSegmentAudit,
};
use super::script_opcode::{command_kind, opcode_spec};

pub(super) const PRIMARY_SCRIPT_OFFSET: usize = 0x4a000;
const PRIMARY_SCRIPT_LIMIT: usize = 0x50000;
const OPCODE_DISPATCH_OFFSET: usize = 0x056c;

pub(super) fn audit_script_asset(
    decoded: &[u8],
    corpus_asset: &DialogueCorpusAsset,
    mgame: &[u8],
) -> Result<DialogueScriptAssetAudit> {
    ensure!(
        decoded.len() >= PRIMARY_SCRIPT_LIMIT,
        "{} decoded image is too small for its primary script region",
        corpus_asset.source_path
    );
    let primary_region_end = primary_region_end(decoded)?;
    let raw_bank_table_offsets = [
        runtime_pointer_offset(read_u32(decoded, PRIMARY_SCRIPT_OFFSET)?, decoded.len())?,
        runtime_pointer_offset(read_u32(decoded, PRIMARY_SCRIPT_OFFSET + 4)?, decoded.len())?,
    ];
    let bank_table_offsets = resolve_bank_table_aliases(raw_bank_table_offsets, primary_region_end)
        .with_context(|| format!("{} has invalid bank tables", corpus_asset.source_path))?;

    let mut route_bindings =
        parse_route_bindings(decoded, &bank_table_offsets, primary_region_end)?;
    let raw_route_starts = route_bindings
        .iter()
        .map(|binding| binding.route_table_offset)
        .collect::<BTreeSet<_>>();
    let canonical_route_starts =
        canonical_route_table_offsets(decoded, &raw_route_starts, bank_table_offsets[0])?;
    for binding in &mut route_bindings {
        binding.route_table_offset = *canonical_route_starts
            .get(&binding.route_table_offset)
            .context("route table canonicalization lost a binding")?;
    }
    let unique_route_starts: BTreeSet<_> = route_bindings
        .iter()
        .map(|binding| binding.route_table_offset)
        .collect();
    ensure!(
        !unique_route_starts.is_empty(),
        "{} has no route tables",
        corpus_asset.source_path
    );
    let first_route_table_offset = *unique_route_starts.first().unwrap();
    ensure!(
        first_route_table_offset < bank_table_offsets[0],
        "{} route tables do not precede bank tables",
        corpus_asset.source_path
    );
    let route_entrypoints = parse_route_entrypoints(
        decoded,
        &unique_route_starts,
        &raw_route_starts,
        first_route_table_offset,
        bank_table_offsets[0],
    )?;

    let unique_entrypoints: BTreeSet<_> = route_entrypoints
        .values()
        .flat_map(|entrypoints| entrypoints.iter().flatten().copied())
        .collect();
    ensure!(
        !unique_entrypoints.is_empty(),
        "{} has no command entrypoints",
        corpus_asset.source_path
    );

    let mut commands = BTreeMap::new();
    let mut streams = BTreeMap::new();
    for entrypoint in &unique_entrypoints {
        let stream = parse_command_stream(
            decoded,
            *entrypoint,
            first_route_table_offset,
            bank_table_offsets[0],
            mgame,
            &mut commands,
        )?;
        streams.insert(*entrypoint, stream);
    }

    let mut bank_entrypoints: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for binding in &route_bindings {
        let entrypoints = route_entrypoints
            .get(&binding.route_table_offset)
            .context("route binding lost its route table")?;
        bank_entrypoints
            .entry(binding.bank_selector)
            .or_default()
            .extend(entrypoints.iter().flatten().copied());
    }
    let message_bindings = bind_messages(
        decoded,
        corpus_asset,
        &bank_entrypoints,
        &streams,
        &commands,
    )?;
    let message_binding_by_context: BTreeMap<_, _> = message_bindings
        .iter()
        .map(|binding| {
            (
                (binding.bank_selector, binding.command_offset.as_str()),
                binding,
            )
        })
        .collect();

    let bank_tables = (0..bank_table_offsets.len())
        .map(|bank_selector| {
            let routes = route_bindings
                .iter()
                .filter(|binding| binding.bank_selector == bank_selector)
                .map(|binding| {
                    let entrypoints = route_entrypoints
                        .get(&binding.route_table_offset)
                        .context("route table entrypoints disappeared")?;
                    let segments = entrypoints
                        .iter()
                        .enumerate()
                        .map(|(slot_index, entrypoint)| {
                            let Some(entrypoint) = entrypoint else {
                                return Ok(DialogueScriptRouteSegmentAudit {
                                    slot_index,
                                    entrypoint: None,
                                    command_count: 0,
                                    message_reference_count: 0,
                                    messages: Vec::new(),
                                });
                            };
                            let stream = streams
                                .get(entrypoint)
                                .context("route segment stream disappeared")?;
                            let messages = stream
                                .iter()
                                .filter_map(|command_offset| {
                                    let offset = hex_offset(*command_offset);
                                    message_binding_by_context
                                        .get(&(bank_selector, offset.as_str()))
                                        .map(|binding| (*binding).clone())
                                })
                                .collect::<Vec<_>>();
                            Ok(DialogueScriptRouteSegmentAudit {
                                slot_index,
                                entrypoint: Some(hex_offset(*entrypoint)),
                                command_count: stream.len(),
                                message_reference_count: messages.len(),
                                messages,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(DialogueScriptRouteAudit {
                        variant_selector: binding.variant_selector,
                        decoded_offset: hex_offset(binding.route_table_offset),
                        slot_count: entrypoints.len(),
                        entrypoint_count: entrypoints.iter().flatten().count(),
                        entrypoints: entrypoints
                            .iter()
                            .map(|offset| offset.map(hex_offset))
                            .collect(),
                        segments,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(DialogueScriptBankTableAudit {
                bank_selector,
                decoded_offset: hex_offset(bank_table_offsets[bank_selector]),
                route_bindings: routes,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let referenced_coordinates: BTreeSet<_> = message_bindings
        .iter()
        .map(|binding| binding.coordinate_id.as_str())
        .collect();
    let referenced_semantic_groups: BTreeSet<_> = message_bindings
        .iter()
        .map(|binding| binding.semantic_source_sha256.as_str())
        .collect();
    let route_entry_reference_count = bank_tables
        .iter()
        .flat_map(|bank| &bank.route_bindings)
        .map(|route| route.entrypoint_count)
        .sum();
    let unique_route_entry_count = route_entrypoints
        .values()
        .map(|entrypoints| entrypoints.iter().flatten().count())
        .sum();
    let route_slot_count = bank_tables
        .iter()
        .flat_map(|bank| &bank.route_bindings)
        .map(|route| route.slot_count)
        .sum();

    Ok(DialogueScriptAssetAudit {
        kind: "Justice Gakuen 2 complete primary dialogue script asset audit".to_string(),
        source_path: corpus_asset.source_path.clone(),
        decoded_sha256: sha256_bytes(decoded),
        primary_region_start: hex_offset(PRIMARY_SCRIPT_OFFSET),
        primary_region_end: hex_offset(primary_region_end),
        bank_tables,
        unique_route_table_count: unique_route_starts.len(),
        route_entry_reference_count,
        unique_route_entry_count,
        route_slot_count,
        unique_entrypoint_count: unique_entrypoints.len(),
        reachable_command_count: commands.len(),
        contextual_message_reference_count: message_bindings.len(),
        referenced_coordinate_count: referenced_coordinates.len(),
        referenced_semantic_group_count: referenced_semantic_groups.len(),
        commands: commands.into_values().collect(),
        message_bindings,
    })
}

pub(super) fn canonical_route_table_offsets(
    decoded: &[u8],
    raw_route_starts: &BTreeSet<usize>,
    first_bank_table_offset: usize,
) -> Result<BTreeMap<usize, usize>> {
    let starts = raw_route_starts.iter().copied().collect::<Vec<_>>();
    let mut canonical = BTreeMap::new();
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts
            .get(index + 1)
            .copied()
            .unwrap_or(first_bank_table_offset);
        ensure!(
            start < end && (end - start).is_multiple_of(4),
            "invalid raw route table bounds at {}",
            hex_offset(start)
        );
        let route_start = if end - start == 4 {
            let pointed_offset = runtime_pointer_offset(read_u32(decoded, start)?, decoded.len())?;
            canonical.get(&pointed_offset).copied().unwrap_or(start)
        } else {
            start
        };
        canonical.insert(start, route_start);
    }
    Ok(canonical)
}

pub(super) fn serialized_asset_bytes(asset: &DialogueScriptAssetAudit) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(asset)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Debug, Clone, Copy)]
struct RouteBinding {
    bank_selector: usize,
    variant_selector: usize,
    route_table_offset: usize,
}

fn parse_route_bindings(
    decoded: &[u8],
    bank_table_offsets: &[usize],
    primary_region_end: usize,
) -> Result<Vec<RouteBinding>> {
    let mut bindings = Vec::new();
    for (bank_selector, start) in bank_table_offsets.iter().copied().enumerate() {
        let end = bank_table_offsets
            .get(bank_selector + 1)
            .copied()
            .unwrap_or(primary_region_end);
        ensure!(
            (end - start).is_multiple_of(4),
            "bank {bank_selector} table is not word sized"
        );
        for (variant_selector, offset) in (start..end).step_by(4).enumerate() {
            let route_table_offset =
                runtime_pointer_offset(read_u32(decoded, offset)?, decoded.len())?;
            ensure!(
                (PRIMARY_SCRIPT_OFFSET + 8..bank_table_offsets[0]).contains(&route_table_offset),
                "bank {bank_selector} variant {variant_selector} route table is outside primary script tables"
            );
            ensure!(
                route_table_offset.is_multiple_of(4),
                "route table at {} is not word aligned",
                hex_offset(route_table_offset)
            );
            bindings.push(RouteBinding {
                bank_selector,
                variant_selector,
                route_table_offset,
            });
        }
    }
    Ok(bindings)
}

pub(super) fn resolve_bank_table_aliases(
    raw_offsets: [usize; 2],
    primary_region_end: usize,
) -> Result<Vec<usize>> {
    ensure!(
        raw_offsets[0] <= raw_offsets[1] && raw_offsets[1] < primary_region_end,
        "bank tables are neither ordered nor an exact alias"
    );
    if raw_offsets[0] == raw_offsets[1] {
        Ok(vec![raw_offsets[0]])
    } else {
        Ok(raw_offsets.into())
    }
}

fn parse_route_entrypoints(
    decoded: &[u8],
    unique_route_starts: &BTreeSet<usize>,
    raw_route_starts: &BTreeSet<usize>,
    first_route_table_offset: usize,
    first_bank_table_offset: usize,
) -> Result<BTreeMap<usize, Vec<Option<usize>>>> {
    let raw_starts = raw_route_starts.iter().copied().collect::<Vec<_>>();
    let mut routes = BTreeMap::new();
    for start in unique_route_starts {
        let raw_index = raw_starts
            .binary_search(start)
            .expect("canonical route start must remain in the raw population");
        let end = raw_starts
            .get(raw_index + 1)
            .copied()
            .unwrap_or(first_bank_table_offset);
        ensure!(
            *start < end && (end - *start).is_multiple_of(4),
            "invalid route table bounds at {}",
            hex_offset(*start)
        );
        let mut entrypoints = Vec::new();
        for offset in (*start..end).step_by(4) {
            let value = read_u32(decoded, offset)?;
            if value == 0 {
                entrypoints.push(None);
                continue;
            }
            let entrypoint = runtime_pointer_offset(value, decoded.len())?;
            ensure!(
                (PRIMARY_SCRIPT_OFFSET + 8..first_route_table_offset).contains(&entrypoint),
                "route entrypoint {} from route table {} record {} is outside command roots",
                hex_offset(entrypoint),
                hex_offset(*start),
                hex_offset(offset),
            );
            entrypoints.push(Some(entrypoint));
        }
        ensure!(
            entrypoints.iter().any(Option::is_some),
            "route table at {} is empty",
            hex_offset(*start)
        );
        routes.insert(*start, entrypoints);
    }
    Ok(routes)
}

fn parse_command_stream(
    decoded: &[u8],
    entrypoint: usize,
    first_route_table_offset: usize,
    first_bank_table_offset: usize,
    mgame: &[u8],
    commands: &mut BTreeMap<usize, DialogueScriptCommandAudit>,
) -> Result<Vec<usize>> {
    let mut cursor = entrypoint;
    let mut visited = BTreeSet::new();
    let mut stream = Vec::new();
    loop {
        ensure!(
            (PRIMARY_SCRIPT_OFFSET + 8..first_route_table_offset).contains(&cursor),
            "stream {} reached non-command start {}",
            hex_offset(entrypoint),
            hex_offset(cursor)
        );
        ensure!(
            visited.insert(cursor),
            "stream {} loops at {}",
            hex_offset(entrypoint),
            hex_offset(cursor)
        );
        let opcode = decoded[cursor];
        let spec = opcode_spec(opcode)
            .with_context(|| format!("invalid opcode at {}", hex_offset(cursor)))?;
        let command_end = cursor
            .checked_add(spec.width)
            .context("command end overflow")?;
        if is_zero_tail_padding(decoded, cursor, first_route_table_offset, spec.width) {
            return Ok(stream);
        }
        ensure!(
            command_end <= first_bank_table_offset,
            "opcode 0x{opcode:02x} at {} crosses the primary command/table region",
            hex_offset(cursor)
        );
        let raw = &decoded[cursor..command_end];
        let dispatch_offset = OPCODE_DISPATCH_OFFSET + usize::from(opcode) * 4;
        let handler_runtime_address = read_u32(mgame, dispatch_offset)?;
        ensure!(
            (0x800a_2000..0x800a_e800).contains(&handler_runtime_address),
            "opcode 0x{opcode:02x} handler is outside MGAME executable text"
        );
        let command = DialogueScriptCommandAudit {
            decoded_offset: hex_offset(cursor),
            runtime_address: hex_address(DECODED_RUNTIME_BASE + cursor as u32),
            opcode: format!("0x{opcode:02x}"),
            width: spec.width,
            handler_runtime_address: hex_address(handler_runtime_address),
            flow: spec.flow.label().to_string(),
            command_kind: command_kind(opcode).to_string(),
            raw_bytes: raw
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
        };
        if let Some(existing) = commands.get(&cursor) {
            ensure!(
                existing.opcode == command.opcode
                    && existing.width == command.width
                    && existing.raw_bytes == command.raw_bytes,
                "overlapping streams disagree at {}",
                hex_offset(cursor)
            );
        } else {
            commands.insert(cursor, command);
        }
        stream.push(cursor);
        if spec.flow.ends_linear_stream() {
            return Ok(stream);
        }
        ensure!(
            command_end <= first_route_table_offset,
            "stream {} fallthrough opcode 0x{opcode:02x} at {} ends at {} beyond first route table {}",
            hex_offset(entrypoint),
            hex_offset(cursor),
            hex_offset(command_end),
            hex_offset(first_route_table_offset),
        );
        cursor = command_end;
    }
}

pub(super) fn is_zero_tail_padding(
    decoded: &[u8],
    cursor: usize,
    boundary: usize,
    command_width: usize,
) -> bool {
    let Some(tail) = decoded.get(cursor..boundary) else {
        return false;
    };
    !tail.is_empty() && tail.len() < command_width && tail.iter().all(|byte| *byte == 0)
}

fn bind_messages(
    decoded: &[u8],
    corpus_asset: &DialogueCorpusAsset,
    bank_entrypoints: &BTreeMap<usize, BTreeSet<usize>>,
    streams: &BTreeMap<usize, Vec<usize>>,
    commands: &BTreeMap<usize, DialogueScriptCommandAudit>,
) -> Result<Vec<DialogueScriptMessageBinding>> {
    let mut bindings: BTreeMap<(usize, usize), DialogueScriptMessageBinding> = BTreeMap::new();
    for (bank_selector, entrypoints) in bank_entrypoints {
        let bank = corpus_asset
            .banks
            .iter()
            .find(|bank| bank.selector_index == *bank_selector)
            .with_context(|| {
                format!(
                    "{} corpus is missing bank {bank_selector}",
                    corpus_asset.source_path
                )
            })?;
        for entrypoint in entrypoints {
            let stream = streams
                .get(entrypoint)
                .context("command stream disappeared")?;
            for command_offset in stream {
                let opcode = decoded[*command_offset];
                let spec = opcode_spec(opcode)?;
                let Some((high_offset, low_offset)) = spec.message_index_bytes else {
                    continue;
                };
                let entry_index = usize::from(u16::from_be_bytes([
                    decoded[*command_offset + high_offset],
                    decoded[*command_offset + low_offset],
                ]));
                let entry = bank.entries.get(entry_index).with_context(|| {
                    let command_position = stream
                        .iter()
                        .position(|offset| offset == command_offset)
                        .unwrap_or_default();
                    let preceding_commands = stream
                        [command_position.saturating_sub(7)..=command_position]
                        .iter()
                        .map(|offset| {
                            commands
                                .get(offset)
                                .map(|command| {
                                    format!("{} {}", command.decoded_offset, command.raw_bytes)
                                })
                                .unwrap_or_else(|| hex_offset(*offset))
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "{} bank {bank_selector} message {entry_index} from opcode 0x{opcode:02x} at {} in stream {} is outside its source corpus; preceding commands={preceding_commands:?}",
                        corpus_asset.source_path,
                        hex_offset(*command_offset),
                        hex_offset(*entrypoint),
                    )
                })?;
                ensure!(
                    entry.entry_index == entry_index,
                    "source corpus entry ordering changed"
                );
                let command = commands
                    .get(command_offset)
                    .context("message command disappeared")?;
                let binding =
                    message_binding(*bank_selector, *command_offset, opcode, entry_index, entry);
                if let Some(existing) = bindings.insert((*bank_selector, *command_offset), binding)
                {
                    ensure!(
                        existing.coordinate_id == entry.coordinate_id,
                        "contextual message binding changed at {}",
                        hex_offset(*command_offset)
                    );
                }
                ensure!(
                    command.command_kind != "opaque_runtime_command",
                    "message opcode lost its semantic command kind"
                );
            }
        }
    }
    Ok(bindings.into_values().collect())
}

fn message_binding(
    bank_selector: usize,
    command_offset: usize,
    opcode: u8,
    entry_index: usize,
    entry: &DialogueCorpusEntry,
) -> DialogueScriptMessageBinding {
    DialogueScriptMessageBinding {
        bank_selector,
        command_offset: hex_offset(command_offset),
        opcode: format!("0x{opcode:02x}"),
        entry_index,
        coordinate_id: entry.coordinate_id.clone(),
        semantic_source_sha256: entry.semantic_source_sha256.clone(),
        source_markup: entry.source_markup.clone(),
    }
}

fn primary_region_end(decoded: &[u8]) -> Result<usize> {
    let relative_end = decoded[PRIMARY_SCRIPT_OFFSET..PRIMARY_SCRIPT_LIMIT]
        .iter()
        .rposition(|byte| *byte != 0)
        .context("primary script region is empty")?
        + 1;
    let end = PRIMARY_SCRIPT_OFFSET + relative_end;
    ensure!(
        end.is_multiple_of(4),
        "primary script region does not end on a word boundary"
    );
    Ok(end)
}

pub(super) fn runtime_pointer_offset(pointer: u32, image_len: usize) -> Result<usize> {
    let relative = pointer.checked_sub(DECODED_RUNTIME_BASE).with_context(|| {
        format!(
            "script pointer {} is below the MGK runtime image",
            hex_address(pointer)
        )
    })?;
    let offset = usize::try_from(relative)?;
    ensure!(
        offset + 4 <= image_len,
        "script pointer is outside MGK image"
    );
    Ok(offset)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated word at +0x{offset:x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
