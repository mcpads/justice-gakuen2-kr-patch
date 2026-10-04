//! Product placement for the source-bound PLSEL1/SELP1 selector consumer.
use super::*;
use crate::{
    decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan},
    pipeline::sha256_bytes,
};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct SelectorRuntimeInstallation {
    pub program: SelectorRuntimeProgram,
    pub layout: SelectorRuntimeLayout,
    pub(super) atlas: NameInputRuntimeAtlasLayout,
    pub(super) pack: NameGlyphBandPack,
    pub(super) ascii: SelectorAsciiGlyphs,
    pub(super) roster_font: Option<RosterNameFont>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SelectorRuntimeInstallReport {
    pub resident_sha256: String,
    pub bootstrap_sha256: String,
    pub resident_byte_count: usize,
    pub ascii_storage_offset: Option<usize>,
    pub ascii_source_address: String,
    pub glyph_rectangle: [usize; 4],
    pub native_verified: bool,
}
impl SelectorRuntimeInstallation {
    pub(crate) fn build(
        atlas: &NameInputRuntimeAtlasLayout,
        pack: &NameGlyphBandPack,
        ascii: &SelectorAsciiGlyphs,
    ) -> Result<Self> {
        let layout = SelectorRuntimeLayout {
            loader: SelectorNameLoaderLayout {
                origin: 0x800a8000,
                byte_capacity: 3008,
                native_loader_address: 0x80015414,
                font: NameAssetLoad {
                    catalog_index: 718,
                    destination: 0x800c0000,
                    decoded_byte_count: 210944,
                },
                textures: NameAssetLoad {
                    catalog_index: 53,
                    destination: 0x800d4000,
                    decoded_byte_count: 328192,
                },
                font_pixel_address: 0x800d92e0,
                pack_destination: 0x800b0000,
                font_copies: Vec::new(),
                restore_selector_service_table: true,
                restored_zero_ranges: Vec::new(),
            },
            scratch_address: 0x800b4a38,
            cache_x_words: 512,
            cache_y: 0,
            cache_slot_count: 16,
            clut_x_words: 0,
            clut_y: 482,
            bootstrap_origin: 0xa00dc340,
            bootstrap_capacity: 1216,
            payload_source: 0x800eb020,
            payload_capacity: 2016,
            extra_payload_source: Some(0x8010bc20),
            extra_payload_capacity: 992,
            ascii_source: Some(0x8012_2c00),
            data_copies: Vec::new(),
            enter_critical_address: 0x80060058,
            flush_cache_address: 0x80060008,
            exit_critical_address: 0x80060068,
        };
        let program = build_selector_runtime(
            &layout,
            atlas,
            pack,
            ascii,
            &build_shared_name_outline_runtime_program()?,
        )?;
        Ok(Self {
            program,
            layout,
            atlas: atlas.clone(),
            pack: pack.clone(),
            ascii: ascii.clone(),
            roster_font: None,
        })
    }
    pub(crate) fn install_roster_font(&mut self, font: RosterNameFont) -> Result<()> {
        let mut layout = self.layout.clone();
        layout.loader.font.decoded_byte_count = font.report.decoded_byte_count;
        let program = build_selector_runtime(
            &layout,
            &self.atlas,
            &self.pack,
            &self.ascii,
            &build_shared_name_outline_runtime_program()?,
        )?;
        self.layout = layout;
        self.program = program;
        self.roster_font = Some(font);
        Ok(())
    }
    pub(crate) fn report(&self) -> SelectorRuntimeInstallReport {
        SelectorRuntimeInstallReport {
            resident_sha256: sha256_bytes(&self.program.resident_bytes),
            bootstrap_sha256: sha256_bytes(&self.program.bootstrap_bytes),
            resident_byte_count: self.program.resident_bytes.len(),
            ascii_storage_offset: self.program.ascii_storage_offset,
            ascii_source_address: format!("0x{:08x}", self.program.ascii_source_address),
            glyph_rectangle: self.program.glyph_rectangle,
            native_verified: false,
        }
    }
    pub(crate) fn overlay(&self, source: &[u8]) -> Result<SelectorSourceHooks> {
        build_selector_source_hooks(
            source,
            self.layout.bootstrap_origin,
            self.program.dispatch_address,
        )
    }
    pub(crate) fn register_staging(
        &self,
        source: &[u8],
        plan: &mut DecodedRecordWritePlan<'_>,
    ) -> Result<()> {
        let hash = sha256_bytes(source);
        ensure!(
            hash == "17336b981e3fb8b0c8af8a6e2c4290b7a27cefb1012c2b28a63158a890091257",
            "SELP1 staging source changed"
        );
        let first = self
            .program
            .bootstrap_layout
            .payload_split
            .as_ref()
            .map_or(self.program.resident_bytes.len(), |s| s.first_byte_count);
        let pieces = [
            (0x8340, 1216, self.program.bootstrap_bytes.as_slice()),
            (0x17020, 2016, &self.program.resident_bytes[..first]),
            (0x37c20, 992, &self.program.resident_bytes[first..]),
            (0x4ec00, 0x1600, self.program.ascii_bytes.as_slice()),
        ];
        let mut candidate = source.to_vec();
        let mut claims = Vec::new();
        for (start, capacity, bytes) in pieces {
            ensure!(
                bytes.len() <= capacity
                    && source
                        .get(start..start + capacity)
                        .is_some_and(|b| b.iter().all(|&v| v == 0)),
                "SELP1 staging reservation is not source-zero padding"
            );
            if bytes.is_empty() {
                continue;
            }
            candidate[start..start + bytes.len()].copy_from_slice(bytes);
            claims.extend(DecodedDataClaim::from_ranges(
                &format!("selector-staging-{start:x}"),
                "transport verified selector program through unused resource padding",
                [[start, start + bytes.len()]],
            ));
        }
        plan.register_data_candidate("selector runtime staging", &hash, &candidate, &claims)
    }
}
