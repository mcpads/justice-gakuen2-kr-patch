use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::{NAME_GLYPH_CELL_BYTES, NAME_GLYPH_PACK_STORAGE_BYTES, NameInputRuntimeAtlasLayout};

pub const NAME_INPUT_RUNTIME_ORIGIN: u32 = 0x8010_3340;
pub(super) const MEDIAL_TRANSITION_OFFSET: usize = 0x444;
pub(crate) const MEDIAL_TRANSITION_ADDRESS: u32 =
    NAME_INPUT_RUNTIME_ORIGIN + MEDIAL_TRANSITION_OFFSET as u32;
pub const NAME_INPUT_RUNTIME_BYTE_CAPACITY: usize = 0x04c0;
pub(super) const REDISPLAY_CACHE_TAG_OFFSET: usize = NAME_INPUT_RUNTIME_BYTE_CAPACITY - 32;
pub(super) const REDISPLAY_CACHE_TAG_ADDRESS: u32 =
    NAME_INPUT_RUNTIME_ORIGIN + REDISPLAY_CACHE_TAG_OFFSET as u32;

#[path = "runtime/cache_cell.rs"]
mod cache_cell;
#[path = "runtime/component_resolver.rs"]
mod component_resolver;
#[path = "runtime/glyph_fill_materializer.rs"]
mod glyph_fill_materializer;
#[path = "runtime/membership_rank.rs"]
mod membership_rank;
#[path = "runtime/pack_reader.rs"]
mod pack_reader;
#[path = "runtime/selected_code_writer.rs"]
mod selected_code_writer;

use super::NameInputRuntimePackLayout;
use cache_cell::emit_cache_cell_clearer;
pub(crate) use component_resolver::emit_component_resolver;
pub(crate) use glyph_fill_materializer::emit_glyph_fill_materializer;
use pack_reader::{
    emit_pack_byte_reader, emit_stored_table_pack_byte_reader, emit_table_pack_byte_reader,
};
use selected_code_writer::emit_selected_code_writer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameInputRuntimeProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub instruction_offset: usize,
    pub cache_cell_clearer_address: Option<u32>,
    pub cache_cell_resolver_address: Option<u32>,
    pub component_resolver_address: Option<u32>,
    pub glyph_fill_materializer_address: Option<u32>,
    pub selected_code_writer_address: u32,
    pub report: NameInputRuntimeProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputRuntimeProgramReport {
    pub origin: String,
    pub byte_count: usize,
    pub byte_capacity: usize,
    pub sha256: String,
    pub typed_instruction_count: usize,
    pub instruction_origin: String,
    pub instruction_byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redisplay_cache_tag_byte_range: Option<[usize; 2]>,
    pub medial_transition_table_byte_range: Option<[usize; 2]>,
    pub atlas_lookup_table_byte_count: usize,
    pub atlas_lookup_storage: String,
    pub pack_reader_uses_atlas_lookup: bool,
    pub pack_byte_reader_address: String,
    pub cache_cell_clearer_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_cell_resolver_address: Option<String>,
    pub component_resolver_address: Option<String>,
    pub glyph_fill_materializer_address: Option<String>,
    pub nickname_hud_store_address: Option<String>,
    pub selected_code_writer_address: String,
    pub pack_storage_byte_capacity: usize,
    pub fits_runtime_region: bool,
    pub overlay_hook_installed: bool,
    pub runtime_execution_verified: bool,
}

pub fn build_name_input_runtime_program() -> Result<NameInputRuntimeProgram> {
    build_runtime_program(PackReaderLayout::Arithmetic, None)
}

pub fn build_name_input_runtime_program_with_atlas(
    atlas: &NameInputRuntimeAtlasLayout,
) -> Result<NameInputRuntimeProgram> {
    build_runtime_program(PackReaderLayout::RuntimePrefix(atlas), None)
}

pub fn build_name_input_runtime_program_with_stored_atlas_and_outline(
    atlas: &NameInputRuntimeAtlasLayout,
    runtime_pack: &NameInputRuntimePackLayout,
    outline_pixel_address: u32,
    outline_cleanup_address: u32,
    nickname_hud_store_address: u32,
) -> Result<NameInputRuntimeProgram> {
    build_runtime_program(
        PackReaderLayout::GlyphPackCell(atlas, runtime_pack),
        Some((
            outline_pixel_address,
            outline_cleanup_address,
            nickname_hud_store_address,
        )),
    )
}

#[derive(Clone, Copy)]
enum PackReaderLayout<'a> {
    Arithmetic,
    RuntimePrefix(&'a NameInputRuntimeAtlasLayout),
    GlyphPackCell(
        &'a NameInputRuntimeAtlasLayout,
        &'a NameInputRuntimePackLayout,
    ),
}

fn build_runtime_program(
    layout: PackReaderLayout<'_>,
    outline_helpers: Option<(u32, u32, u32)>,
) -> Result<NameInputRuntimeProgram> {
    let mut prefix = Vec::new();
    let instruction_offset = if let PackReaderLayout::RuntimePrefix(atlas) = layout {
        prefix.extend_from_slice(&atlas.lookup_table_bytes);
        let aligned = prefix.len().next_multiple_of(16);
        prefix.resize(aligned, 0);
        aligned
    } else {
        0
    };
    let instruction_origin = NAME_INPUT_RUNTIME_ORIGIN
        .checked_add(u32::try_from(instruction_offset)?)
        .context("name-input runtime instruction origin overflow")?;

    let mut prefix_assembler = Assembler::new();
    emit_pack_reader(&mut prefix_assembler, &layout)?;
    let reader = prefix_assembler
        .assemble(instruction_origin)
        .context("failed to place typed R3000A name glyph pack reader")?;
    let cache_cell_clearer_address = if matches!(layout, PackReaderLayout::GlyphPackCell(..)) {
        Some(
            instruction_origin
                .checked_add(u32::try_from(reader.bytes().len())?)
                .context("name-input cache-cell clearer address overflow")?,
        )
    } else {
        None
    };
    emit_cache_cell_clearer_for_layout(&mut prefix_assembler, &layout)?;
    let reader_and_clearer = prefix_assembler
        .assemble(instruction_origin)
        .context("failed to place typed R3000A name glyph cache-cell clearer")?;
    let component_resolver_address = if matches!(layout, PackReaderLayout::GlyphPackCell(..)) {
        Some(
            instruction_origin
                .checked_add(u32::try_from(reader_and_clearer.bytes().len())?)
                .context("name-input component resolver address overflow")?,
        )
    } else {
        None
    };
    emit_component_resolver_for_layout(&mut prefix_assembler, &layout)?;
    let reader_clearer_and_resolver = prefix_assembler
        .assemble(instruction_origin)
        .context("failed to place typed R3000A name glyph component resolver")?;
    let glyph_fill_materializer_address = if matches!(layout, PackReaderLayout::GlyphPackCell(..)) {
        Some(
            instruction_origin
                .checked_add(u32::try_from(reader_clearer_and_resolver.bytes().len())?)
                .context("name-input glyph-fill materializer address overflow")?,
        )
    } else {
        None
    };
    emit_glyph_fill_materializer_for_layout(&mut prefix_assembler, &layout, outline_helpers)?;
    let reader_clearer_resolver_and_materializer = prefix_assembler
        .assemble(instruction_origin)
        .context("failed to place typed R3000A name glyph fill materializer")?;
    let selected_code_writer_address = instruction_origin
        .checked_add(u32::try_from(
            reader_clearer_resolver_and_materializer.bytes().len(),
        )?)
        .context("name-input selected-code writer address overflow")?;

    let mut assembler = Assembler::new();
    emit_pack_reader(&mut assembler, &layout)?;
    emit_cache_cell_clearer_for_layout(&mut assembler, &layout)?;
    emit_component_resolver_for_layout(&mut assembler, &layout)?;
    emit_glyph_fill_materializer_for_layout(&mut assembler, &layout, outline_helpers)?;
    emit_selected_code_writer(&mut assembler);
    let program = assembler
        .assemble(instruction_origin)
        .context("failed to assemble typed R3000A name-input runtime")?;
    ensure!(
        instruction_offset + program.bytes().len() <= NAME_INPUT_RUNTIME_BYTE_CAPACITY,
        "typed name-input runtime exceeds the source-bound region"
    );
    ensure!(
        program.instruction_spans().len() == program.instructions().len(),
        "name-input runtime lost typed instruction placement evidence"
    );
    let instruction_byte_count = program.bytes().len();
    let instructions = program.instructions().to_vec();
    let mut bytes = prefix;
    bytes.extend_from_slice(program.bytes());

    Ok(NameInputRuntimeProgram {
        instructions,
        instruction_offset,
        cache_cell_clearer_address,
        cache_cell_resolver_address: None,
        component_resolver_address,
        glyph_fill_materializer_address,
        selected_code_writer_address,
        report: NameInputRuntimeProgramReport {
            origin: format!("0x{NAME_INPUT_RUNTIME_ORIGIN:08x}"),
            byte_count: bytes.len(),
            byte_capacity: NAME_INPUT_RUNTIME_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count: program.instruction_spans().len(),
            instruction_origin: format!("0x{instruction_origin:08x}"),
            instruction_byte_count,
            redisplay_cache_tag_byte_range: None,
            medial_transition_table_byte_range: None,
            atlas_lookup_table_byte_count: match layout {
                PackReaderLayout::Arithmetic => 0,
                PackReaderLayout::RuntimePrefix(atlas) => atlas.lookup_table_bytes.len(),
                PackReaderLayout::GlyphPackCell(atlas, _) => {
                    atlas.pack_storage_cell_base_byte_offsets.len() * 2
                }
            },
            atlas_lookup_storage: match layout {
                PackReaderLayout::Arithmetic => "none".to_string(),
                PackReaderLayout::RuntimePrefix(_) => "runtime_prefix".to_string(),
                PackReaderLayout::GlyphPackCell(..) => "glyph_pack_tail_cells".to_string(),
            },
            pack_reader_uses_atlas_lookup: !matches!(layout, PackReaderLayout::Arithmetic),
            pack_byte_reader_address: format!("0x{instruction_origin:08x}"),
            cache_cell_resolver_address: None,
            cache_cell_clearer_address: cache_cell_clearer_address
                .map(|address| format!("0x{address:08x}")),
            component_resolver_address: component_resolver_address
                .map(|address| format!("0x{address:08x}")),
            glyph_fill_materializer_address: glyph_fill_materializer_address
                .map(|address| format!("0x{address:08x}")),
            nickname_hud_store_address: outline_helpers
                .map(|(_, _, address)| format!("0x{address:08x}")),
            selected_code_writer_address: format!("0x{selected_code_writer_address:08x}"),
            pack_storage_byte_capacity: NAME_GLYPH_PACK_STORAGE_BYTES,
            fits_runtime_region: true,
            overlay_hook_installed: false,
            runtime_execution_verified: false,
        },
        bytes,
    })
}

fn emit_glyph_fill_materializer_for_layout(
    assembler: &mut Assembler,
    layout: &PackReaderLayout<'_>,
    outline_helpers: Option<(u32, u32, u32)>,
) -> Result<()> {
    if let PackReaderLayout::GlyphPackCell(atlas, runtime_pack) = layout {
        let (outline_pixel_address, outline_cleanup_address, nickname_hud_store_address) =
            outline_helpers
                .context("stored-atlas runtime requires owned outline helper addresses")?;
        let lookup_storage_cell_count = runtime_metadata_storage_cell_count(
            atlas,
            runtime_pack.runtime_coordinate_list_byte_count,
        );
        let lookup_storage_offset = atlas
            .pack_storage_cell_base_byte_offsets
            .len()
            .checked_sub(lookup_storage_cell_count)
            .context("name glyph runtime metadata exceeds its trailing cells")?
            * NAME_GLYPH_CELL_BYTES;
        let coordinate_list_offset = lookup_storage_offset + atlas.lookup_table_bytes.len();
        emit_glyph_fill_materializer(
            assembler,
            runtime_pack,
            u16::try_from(coordinate_list_offset)?,
            384,
            outline_pixel_address,
            outline_cleanup_address,
            Some(nickname_hud_store_address),
        )?;
    }
    Ok(())
}

fn emit_component_resolver_for_layout(
    assembler: &mut Assembler,
    layout: &PackReaderLayout<'_>,
) -> Result<()> {
    if let PackReaderLayout::GlyphPackCell(_, pack) = layout {
        emit_component_resolver(assembler, pack)?;
    }
    Ok(())
}

fn emit_pack_reader(assembler: &mut Assembler, layout: &PackReaderLayout<'_>) -> Result<()> {
    match layout {
        PackReaderLayout::Arithmetic => emit_pack_byte_reader(assembler),
        PackReaderLayout::RuntimePrefix(_) => emit_table_pack_byte_reader(assembler),
        PackReaderLayout::GlyphPackCell(atlas, runtime_pack) => {
            let lookup_storage_cell_count = runtime_metadata_storage_cell_count(
                atlas,
                runtime_pack.runtime_coordinate_list_byte_count,
            );
            let lookup_storage_cell_index = atlas
                .pack_storage_cell_base_byte_offsets
                .len()
                .checked_sub(lookup_storage_cell_count)
                .context("name glyph pack runtime lookup exceeds its trailing cells")?;
            let lookup_cell_base_byte_offset =
                atlas.pack_storage_cell_base_byte_offsets[lookup_storage_cell_index];
            emit_stored_table_pack_byte_reader(assembler, lookup_cell_base_byte_offset);
        }
    }
    Ok(())
}

pub(super) fn runtime_metadata_storage_cell_count(
    atlas: &NameInputRuntimeAtlasLayout,
    coordinate_count: usize,
) -> usize {
    (atlas.lookup_table_bytes.len() + coordinate_count).div_ceil(NAME_GLYPH_CELL_BYTES)
}

fn emit_cache_cell_clearer_for_layout(
    assembler: &mut Assembler,
    layout: &PackReaderLayout<'_>,
) -> Result<()> {
    let PackReaderLayout::GlyphPackCell(..) = layout else {
        return Ok(());
    };
    emit_cache_cell_clearer(assembler);
    Ok(())
}

/// Builds the name-entry consumer for a band payload and its trailing atlas lookup.
/// The caller must install this program and the matching pack/lookup atomically.
pub fn build_name_input_band_runtime_program(
    atlas: &NameInputRuntimeAtlasLayout,
    pack: &super::NameGlyphBandPack,
    outline: &super::SharedNameOutlineRuntimeProgram,
    nickname_hud_store: u32,
) -> Result<NameInputRuntimeProgram> {
    ensure!(
        atlas.font_atlas_row_bytes == 384
            && atlas.glyph_cell_width == 20
            && atlas.glyph_cell_height == 20
            && atlas.lookup_table_bytes.len()
                == atlas.pack_storage_cell_base_byte_offsets.len() * 2,
        "band name-entry atlas geometry or lookup size is invalid"
    );
    let lookup_cells = runtime_metadata_storage_cell_count(atlas, 0);
    let first = atlas
        .pack_storage_cell_base_byte_offsets
        .len()
        .checked_sub(lookup_cells)
        .context("band atlas lookup exceeds its cells")?;
    ensure!(
        atlas.pack_storage_cell_base_byte_offsets.len() * NAME_GLYPH_CELL_BYTES
            == NAME_GLYPH_PACK_STORAGE_BYTES
            && first < atlas.pack_storage_cell_base_byte_offsets.len()
            && pack.bytes().len() <= first * NAME_GLYPH_CELL_BYTES,
        "band pack overlaps atlas lookup"
    );
    let origin = NAME_INPUT_RUNTIME_ORIGIN;
    let mut a = Assembler::new();
    emit_stored_table_pack_byte_reader(&mut a, atlas.pack_storage_cell_base_byte_offsets[first]);
    let resolver_address = origin + u32::try_from(a.assemble(origin)?.bytes().len())?;
    cache_cell::emit_cache_cell_resolver(&mut a);
    let materializer_address = origin + u32::try_from(a.assemble(origin)?.bytes().len())?;
    let cache_program = pack.build_cache_program(
        materializer_address,
        origin,
        resolver_address,
        384,
        outline,
        Some(nickname_hud_store),
    )?;
    a.emit_all(psx_r3000a::verify_placed_program(
        &cache_program,
        materializer_address,
    )?);
    let writer_address = origin + u32::try_from(a.assemble(origin)?.bytes().len())?;
    emit_selected_code_writer(&mut a);
    let prefix_len = a.assemble(origin)?.bytes().len();
    ensure!(
        prefix_len <= MEDIAL_TRANSITION_OFFSET,
        "name renderer overlaps medial transitions"
    );
    for _ in prefix_len / 4..MEDIAL_TRANSITION_OFFSET / 4 {
        a.emit(Instruction::nop());
    }
    // A0: existing medial; A1: selected medial; V0: composed/replacement medial.
    // Preserve the pending edit transaction (T1..T5, T8/T9).
    use psx_r3000a::{Instruction::*, Register as R, load_address};
    let table_offset = MEDIAL_TRANSITION_OFFSET + 56;
    a.emit(Sll {
        rd: R::T6,
        rt: R::A1,
        shift: 5,
    })
    .emit(Or {
        rd: R::A0,
        rs: R::A0,
        rt: R::T6,
    })
    .emit_all(load_address(R::A2, origin + table_offset as u32))
    .label("medial_lookup")
    .emit(Lhu {
        rt: R::T0,
        base: R::A2,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::A2,
        rs: R::A2,
        immediate: 2,
    })
    .beq(R::T0, R::ZERO, "medial_replace")
    .emit(Andi {
        rt: R::T6,
        rs: R::T0,
        immediate: 1023,
    })
    .bne(R::T6, R::A0, "medial_lookup")
    .emit(Instruction::nop())
    .emit(Jr { rs: R::RA })
    .emit(Srl {
        rd: R::V0,
        rt: R::T0,
        shift: 10,
    })
    .label("medial_replace")
    .emit(Jr { rs: R::RA })
    .emit(Addu {
        rd: R::V0,
        rs: R::A1,
        rt: R::ZERO,
    });
    let placed = a.assemble(origin)?;
    ensure!(
        placed.bytes().len() == table_offset,
        "medial table placement changed"
    );
    let table: Vec<u8> = super::medial::COMBINATIONS
        .into_iter()
        .chain(
            super::medial::BACKSPACE
                .into_iter()
                .map(|(old, next)| (old, 31, next)),
        )
        .map(|(old, key, next)| old | key << 5 | next << 10)
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    ensure!(
        table_offset + table.len() <= REDISPLAY_CACHE_TAG_OFFSET,
        "medial table overlaps cache tags"
    );
    ensure!(
        placed.bytes().len() <= REDISPLAY_CACHE_TAG_OFFSET,
        "band name-entry runtime needs {} bytes, capacity is {}",
        placed.bytes().len(),
        NAME_INPUT_RUNTIME_BYTE_CAPACITY
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        placed.instructions(),
        origin,
        "band name entry",
    )?;
    let mut bytes = placed.bytes().to_vec();
    bytes.extend_from_slice(&table);
    bytes.resize(NAME_INPUT_RUNTIME_BYTE_CAPACITY, 0);
    Ok(NameInputRuntimeProgram {
        report: NameInputRuntimeProgramReport {
            origin: format!("0x{origin:08x}"),
            byte_count: bytes.len(),
            byte_capacity: NAME_INPUT_RUNTIME_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count: placed.instructions().len(),
            instruction_origin: format!("0x{origin:08x}"),
            instruction_byte_count: placed.bytes().len(),
            medial_transition_table_byte_range: Some([table_offset, table_offset + table.len()]),
            redisplay_cache_tag_byte_range: Some([
                REDISPLAY_CACHE_TAG_OFFSET,
                NAME_INPUT_RUNTIME_BYTE_CAPACITY,
            ]),
            atlas_lookup_table_byte_count: atlas.lookup_table_bytes.len(),
            atlas_lookup_storage: "glyph_pack_tail_cells".into(),
            pack_reader_uses_atlas_lookup: true,
            pack_byte_reader_address: format!("0x{origin:08x}"),
            cache_cell_clearer_address: None,
            cache_cell_resolver_address: Some(format!("0x{resolver_address:08x}")),
            component_resolver_address: None,
            glyph_fill_materializer_address: Some(format!("0x{materializer_address:08x}")),
            nickname_hud_store_address: Some(format!("0x{nickname_hud_store:08x}")),
            selected_code_writer_address: format!("0x{writer_address:08x}"),
            pack_storage_byte_capacity: NAME_GLYPH_PACK_STORAGE_BYTES,
            fits_runtime_region: true,
            overlay_hook_installed: false,
            runtime_execution_verified: false,
        },
        bytes,
        instructions: placed.instructions().to_vec(),
        instruction_offset: 0,
        cache_cell_clearer_address: None,
        cache_cell_resolver_address: Some(resolver_address),
        component_resolver_address: None,
        glyph_fill_materializer_address: Some(materializer_address),
        selected_code_writer_address: writer_address,
    })
}
