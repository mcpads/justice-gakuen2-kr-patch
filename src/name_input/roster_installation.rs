//! Mode-owned transport placement; temporary template padding is restored in RAM.
use super::*;
use crate::{
    decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan},
    pipeline::sha256_bytes,
};
use anyhow::{Result, ensure};

pub(crate) struct RosterRuntimeInstallation {
    pub program: SelectorRuntimeProgram,
    pub layout: SelectorRuntimeLayout,
    source_hash: &'static str,
}

impl SelectorRuntimeInstallation {
    pub(crate) fn roster(&self, number: u8, overlay: &[u8]) -> Result<RosterRuntimeInstallation> {
        let source_hash = match number {
            2 => "a6e4987af2c92720f6df7c8887c484b306f982bf290c61042e2a5ac54c7826a8",
            3 => "0d9fce797c2be8f2d63dd405b0dee4df94f17b072e82b9f73b82dac06b453449",
            4 => "2a5fbcfa15aa5cb29e226fd31fc7e02743b70435eae636e034baac6b97e7d728",
            5 => "59f3d05cd1c03fd1530361a99e0a1a613eefa0a5dd0c26c27974fe4cdb1957d9",
            _ => anyhow::bail!("unsupported roster resource"),
        };
        let mut layout = self.layout.clone();
        layout.loader.origin = 0x800b6000;
        layout.loader.byte_capacity = 3008;
        layout.loader.textures.catalog_index = 52 + u16::from(number);
        layout.loader.textures.decoded_byte_count = 0x38000;

        layout.cache_slot_count = 68;
        layout.bootstrap_capacity = 0x1c0;
        layout.ascii_source = Some(0x800b4c00);
        let (pack, ascii) = if let Some(font) = &self.roster_font {
            layout.loader.font.decoded_byte_count = font.report.decoded_byte_count;
            layout.loader.font_copies = font.loader_copies(
                layout.loader.font.destination,
                layout.loader.pack_destination,
                0x800b4c00,
            )?;
            layout.loader.restored_zero_ranges.clear();
            layout.data_copies.clear();
            (&font.pack, &font.ascii)
        } else {
            let first = 1968;
            let ascii_len = self.ascii.bytes.len().next_multiple_of(4);
            ensure!(
                ascii_len > first && ascii_len - first <= 0x300,
                "roster ASCII changed its transport extent"
            );
            layout.loader.restored_zero_ranges = vec![[0x50, 0x800]];
            layout.data_copies = vec![
                SelectorDataCopy {
                    source: 0x800d4050,
                    destination: 0x800b4c00,
                    byte_count: first,
                },
                SelectorDataCopy {
                    source: 0x800dc500,
                    destination: 0x800b4c00 + first as u32,
                    byte_count: ascii_len - first,
                },
            ];
            (&self.pack, &self.ascii)
        };
        let program = build_roster_runtime(
            &layout,
            &self.atlas,
            pack,
            ascii,
            &build_shared_name_outline_runtime_program()?,
            number,
            overlay,
        )?;
        Ok(RosterRuntimeInstallation {
            program,
            layout,
            source_hash,
        })
    }
}

impl RosterRuntimeInstallation {
    pub(crate) fn register_staging(
        &self,
        source: &[u8],
        plan: &mut DecodedRecordWritePlan<'_>,
    ) -> Result<()> {
        ensure!(
            source.len() == 0x38000 && sha256_bytes(source) == self.source_hash,
            "roster texture source changed"
        );
        // 0x50..0x800 belongs to the native template-copy input. It is transport
        // only: the loader must restore this exact source-zero range before return.
        ensure!(
            if self.layout.loader.font_copies.is_empty() {
                self.layout.loader.restored_zero_ranges == [[0x50, 0x800]]
            } else {
                self.layout.loader.restored_zero_ranges.is_empty()
                    && self.layout.data_copies.is_empty()
            },
            "roster template transport lacks restoration"
        );
        let first = self
            .program
            .bootstrap_layout
            .payload_split
            .as_ref()
            .map_or(self.program.resident_bytes.len(), |s| s.first_byte_count);
        let mut ascii = self.program.ascii_bytes.clone();
        ascii.resize(ascii.len().next_multiple_of(4), 0);
        let mut pieces = vec![
            (0x8340, 0x1c0, self.program.bootstrap_bytes.as_slice()),
            (0x17020, 2016, &self.program.resident_bytes[..first]),
            (0x37c20, 992, &self.program.resident_bytes[first..]),
        ];
        if self.layout.loader.font_copies.is_empty() {
            pieces.extend([
                (0x50, 1968, &ascii[..1968]),
                (0x8500, 0x300, &ascii[1968..]),
            ]);
        }
        let mut candidate = source.to_vec();
        let mut claims = Vec::new();
        for (start, capacity, bytes) in pieces {
            ensure!(
                bytes.len() <= capacity && source[start..start + capacity].iter().all(|&b| b == 0),
                "roster transport exceeds source-zero reservation"
            );
            if bytes.is_empty() {
                continue;
            }
            candidate[start..start + bytes.len()].copy_from_slice(bytes);
            claims.extend(DecodedDataClaim::from_ranges(
                &format!("roster-staging-{start:x}"),
                "transport roster runtime and its declared bootstrap data",
                [[start, start + bytes.len()]],
            ));
        }
        plan.register_data_candidate(
            "roster runtime staging",
            self.source_hash,
            &candidate,
            &claims,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires private source resources and installed name font"]
    fn roster_installation_fits_actual_font_and_all_source_resources() -> Result<()> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let names =
            std::path::PathBuf::from(std::env::var("JUSTICE_NAME_INPUT_DIR").map_err(|_| {
                anyhow::anyhow!(
                    "set JUSTICE_NAME_INPUT_DIR to an explicit composed name-input build"
                )
            })?);
        let report: serde_json::Value = serde_json::from_slice(&std::fs::read(
            names.join("composed-name-input-build.json"),
        )?)?;
        let atlas: NameInputRuntimeAtlasLayout =
            serde_json::from_value(report["font"]["runtime_atlas"].clone())?;
        let font = crate::compression::decompress(&std::fs::read(names.join("MA_ENT.BIZ"))?, true)?;
        let tim = crate::tim::parse_4bpp_prefix(&font[0x19000..])?;
        let mut storage = Vec::new();
        for &base in &atlas.pack_storage_cell_base_byte_offsets {
            for y in 0..20 {
                let start =
                    0x19000 + tim.pixel_offset + usize::from(base) + y * atlas.font_atlas_row_bytes;
                storage.extend_from_slice(&font[start..start + 10]);
            }
        }
        storage.truncate(report["font"]["glyph_pack"]["pack_bytes"].as_u64().unwrap() as usize);
        assert_eq!(
            sha256_bytes(&storage),
            report["font"]["glyph_pack"]["pack_sha256"]
                .as_str()
                .unwrap()
        );
        let pack = NameGlyphBandPack::parse(storage)?;
        let reference = crate::font::rasterize_menu_glyphs(
            &root.join("../fonts/galmuri/Galmuri14.ttf"),
            &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
            15.,
            3,
            13,
        )?;
        let ascii = SelectorAsciiGlyphs::build(&reference, 0x1600)?;
        let mut installation = SelectorRuntimeInstallation::build(&atlas, &pack, &ascii)?;
        for compact in [false, true] {
            if compact {
                let compact_font =
                    RosterNameFont::build(&root.join("../fonts/galmuri/Galmuri11.ttf"), 12.)?;
                let expanded = compact_font.clone();
                let mut decoded = font[..0x33800].to_vec();
                let prefix = decoded.clone();
                expanded.append_to(&mut decoded)?;
                assert_eq!(&decoded[..prefix.len()], &prefix);
                assert_eq!(decoded.len(), 229168);
                assert_eq!(expanded.report.ascii_offset, 227884);
                assert_eq!(
                    &decoded[expanded.report.pack_offset
                        ..expanded.report.pack_offset + expanded.pack.bytes().len()],
                    expanded.pack.bytes()
                );
                assert_eq!(
                    &decoded[expanded.report.ascii_offset
                        ..expanded.report.ascii_offset + expanded.ascii.bytes.len()],
                    &expanded.ascii.bytes
                );
                let before = decoded.clone();
                assert!(expanded.append_to(&mut decoded).is_err());
                assert_eq!(decoded, before);
                installation.install_roster_font(compact_font)?;
            }
            for number in 2..=5 {
                let disc = crate::source_disc::test_source_disc();
                let overlay = disc.read_record(&format!("DAT1/PLSEL{number}.BIN"))?.1;
                let source = crate::compression::decompress(
                    &disc.read_record(&format!("DAT2/SELP{number}.BIZ"))?.1,
                    true,
                )?;
                let runtime = installation.roster(number, &overlay)?;
                let hash = sha256_bytes(&source);
                let mut plan = DecodedRecordWritePlan::new("roster-test", &source, &hash)?;
                runtime.register_staging(&source, &mut plan)?;
                let written = plan.apply(None)?;
                if compact {
                    assert_eq!(&written[0x50..0x800], &source[0x50..0x800]);
                    assert_eq!(&written[0x8500..0x8800], &source[0x8500..0x8800]);
                    assert!(runtime.layout.data_copies.is_empty());
                    assert!(runtime.layout.loader.restored_zero_ranges.is_empty());
                    assert_eq!(runtime.layout.loader.font_copies.len(), 2);
                    assert_eq!(runtime.program.glyph_rectangle[2], 13);
                }
                eprintln!(
                    "PLSEL{number} compact={compact}: resident={} bootstrap={} ASCII={}",
                    runtime.program.resident_bytes.len(),
                    runtime.program.bootstrap_bytes.len(),
                    runtime.program.ascii_bytes.len()
                );
            }
        }
        Ok(())
    }
}
