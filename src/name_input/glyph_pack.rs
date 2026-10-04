#[path = "glyph_pack/copy_runtime.rs"]
mod copy_runtime;
pub(crate) use copy_runtime::emit_name_glyph_pack_copy;
#[path = "glyph_pack/correction_runtime.rs"]
mod correction_runtime;
pub use correction_runtime::build_name_fill_correction_program;
#[path = "glyph_pack/corrections.rs"]
mod corrections;
pub use corrections::NameGlyphFillCorrections;
#[path = "glyph_pack/build.rs"]
mod build;
#[path = "glyph_pack/decode.rs"]
mod decode;
#[path = "glyph_pack/format.rs"]
mod format;
#[path = "glyph_pack/storage.rs"]
mod storage;

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

pub use build::build_name_glyph_pack;
pub use decode::NameGlyphPackDecoder;
pub use storage::{
    NameGlyphPackCellPayload, NameGlyphPackRuntimeLookupInstallReport,
    NameGlyphPackRuntimeLookupPayload, NameGlyphPackStorageImage, NameGlyphPackTimInstallReport,
    encode_name_glyph_pack_cells, install_name_glyph_pack_auxiliary,
    install_name_glyph_pack_cells_in_tim, install_name_glyph_pack_runtime_lookup,
    read_name_glyph_pack_cells,
};

pub const MODERN_HANGUL_START: u32 = 0xac00;

#[derive(Clone, Debug)]
pub struct NameGlyphPack {
    pub bytes: Vec<u8>,
    pub runtime_coordinate_list: Vec<u8>,
    pub report: NameGlyphPackReport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NameGlyphMaterializationBundle {
    pub(crate) pack_bytes: Vec<u8>,
    pub(crate) runtime_coordinate_list: Vec<u8>,
    pub(crate) format: NameGlyphMaterializationFormat,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NameGlyphMaterializationFormat {
    Components(Box<NameGlyphPackReport>),
    Bands,
}

impl NameGlyphMaterializationBundle {
    pub fn from_bands(pack: &super::NameGlyphBandPack) -> Self {
        Self {
            pack_bytes: pack.bytes().to_vec(),
            runtime_coordinate_list: Vec::new(),
            format: NameGlyphMaterializationFormat::Bands,
        }
    }

    pub(crate) fn repertoire_membership_offset(&self) -> Result<usize> {
        match &self.format {
            NameGlyphMaterializationFormat::Components(_) => {
                Ok(NameGlyphPackDecoder::new(&self.pack_bytes)?.repertoire_membership_offset())
            }
            NameGlyphMaterializationFormat::Bands => {
                Ok(super::NameGlyphBandPack::parse(self.pack_bytes.clone())?
                    .repertoire_membership_offset())
            }
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        match &self.format {
            NameGlyphMaterializationFormat::Components(report) => anyhow::ensure!(
                self.pack_bytes.len() == report.pack_bytes
                    && self.runtime_coordinate_list.len()
                        == report.runtime_coordinate_list_byte_count,
                "name materialization bundle differs from its component report"
            ),
            NameGlyphMaterializationFormat::Bands => {
                anyhow::ensure!(
                    self.runtime_coordinate_list.is_empty(),
                    "band names have no component coordinates"
                );
                super::NameGlyphBandPack::parse(self.pack_bytes.clone())?;
            }
        }
        Ok(())
    }

    pub fn from_pack(pack: &NameGlyphPack) -> Self {
        Self {
            pack_bytes: pack.bytes.clone(),
            runtime_coordinate_list: pack.runtime_coordinate_list.clone(),
            format: NameGlyphMaterializationFormat::Components(Box::new(pack.report.clone())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NameGlyphPackReport {
    pub kind: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub repertoire: String,
    pub supported_syllable_count: usize,
    pub unsupported_modern_syllable_count: usize,
    pub crop: [usize; 4],
    pub occupied_coordinate_count: usize,
    pub bytes_per_component_mask: usize,
    pub no_final_base_component_count: usize,
    pub final_bearing_base_component_count: usize,
    pub final_component_count: usize,
    pub component_count: usize,
    pub no_final_rank_prefix_entry_bytes: usize,
    pub final_rank_prefix_entry_bytes: usize,
    pub no_final_rank_prefix_byte_range: [usize; 2],
    pub final_bearing_rank_prefix_byte_range: [usize; 2],
    pub final_rank_prefix_byte_range: [usize; 2],
    pub runtime_coordinate_list_byte_count: usize,
    pub component_mask_byte_range: [usize; 2],
    pub pack_bytes: usize,
    pub storage_capacity_bytes: usize,
    pub storage_bytes_remaining: usize,
    pub fits_storage: bool,
    pub exact_reference_glyph_count: usize,
    pub missing_fill_pixel_count: usize,
    pub unexpected_fill_pixel_count: usize,
    pub synthesized_blank_count: usize,
    pub runtime_consumer_installed: bool,
}

pub fn build_default_name_glyph_pack(font_path: &Path) -> Result<NameGlyphPack> {
    build_name_glyph_pack(font_path, 13.0, super::NAME_GLYPH_PACK_STORAGE_BYTES)
}
