//! Bind the transient battle renderer to the same font payload as name entry.
use super::name_entry_composed_build::ComposedNameInputBuild;
use crate::{
    compression::decompress,
    decoded_record_write_plan::{
        CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
    },
    disc::rebuild::DiscRecordSourceIdentity,
    font::rasterize_menu_glyphs,
    name_input::*,
    pipeline::sha256_bytes,
    psx_machine_code_sources::PsxMachineCodeSources,
    source_disc::{MAIN_EXECUTABLE_SHA256, SupportedSourceDisc},
    tim::parse_4bpp_prefix,
};
use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, encode_le_bytes, verify_placed_program};
use serde::Serialize;
use std::path::Path;

pub(super) const PATH: &str = "DAT2/COCKEDIT.TIZ";
const SOURCE_SHA: &str = "86f1a56f31fa4a13c0d6d46b7f86f430ee4a68146d2939fcb0008c2b9374e1ed";
#[derive(Serialize)]
pub(super) struct BattleNameBuildReport {
    source_stored_sha256: String,
    source_decoded_sha256: String,
    pack_sha256: String,
    ascii_font_sha256: String,
    ascii_font_px: u32,
    ascii_vertical_shift_px: i32,
    decoded_sha256: String,
    stored_sha256: String,
    decoded_bytes: usize,
    compressed_bytes: usize,
    record_bytes: usize,
    entry_address: String,
    generator_sha256: String,
    sprite_hook_sha256: String,
    pub cold_runtime_verified: bool,
}
pub(super) struct BattleNameBuild {
    pub stored: Vec<u8>,
    pub source: DiscRecordSourceIdentity,
    pub report: BattleNameBuildReport,
    generator: Vec<u8>,
    hook: Vec<u8>,
}
impl BattleNameBuild {
    pub fn build(
        source: &SupportedSourceDisc,
        names: &ComposedNameInputBuild,
        ascii_font: &Path,
    ) -> Result<Self> {
        let (_, original) = source.read_record(PATH)?;
        ensure!(
            original.len() == 8308 && sha256_bytes(&original) == SOURCE_SHA,
            "battle name supplier source changed"
        );
        let native = decompress(&original, true)?;
        ensure!(
            sha256_bytes(&native)
                == "63a16060112960f8817e46cfcd957fd04d75322e784fe19aa5b69c772ea09440",
            "battle name decoded source changed"
        );
        let font = decompress(&names.font_stored, true)?;
        let tim_offset = super::name_entry_font_build::FONT_TIM_OFFSET;
        let tim = parse_4bpp_prefix(&font[tim_offset..])?;
        let atlas = &names.report.font.runtime_atlas;
        ensure!(
            tim.image_word_width * 2 == atlas.font_atlas_row_bytes,
            "battle font row stride changed"
        );
        let mut storage = Vec::new();
        for &base in &atlas.pack_storage_cell_base_byte_offsets {
            for y in 0..20 {
                let start = tim_offset
                    + tim.pixel_offset
                    + usize::from(base)
                    + y * atlas.font_atlas_row_bytes;
                storage.extend_from_slice(
                    font.get(start..start + 10)
                        .context("battle font cell leaves resource")?,
                );
            }
        }
        let pack_len = names.report.font.glyph_pack.pack_bytes;
        ensure!(storage.len() >= pack_len, "battle pack readback truncated");
        storage.truncate(pack_len);
        ensure!(
            sha256_bytes(&storage) == names.report.font.glyph_pack.pack_sha256,
            "battle pack differs from installed name-entry font"
        );
        let pack = NameGlyphBandPack::parse(storage)?;
        let mut reference = rasterize_menu_glyphs(
            ascii_font,
            &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
            16.,
            3,
            13,
        )?;
        ensure!(
            reference.font_sha256
                == "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6",
            "battle ASCII requires admitted NeoDunggeunmo"
        );
        for glyph in &mut reference.glyphs {
            ensure!(
                glyph.pixels[..20].iter().all(|&v| v == 0),
                "battle ASCII shift loses pixels"
            );
            glyph.pixels.copy_within(20..400, 0);
            glyph.pixels[380..].fill(0);
        }
        let ascii = SelectorAsciiGlyphs::build(&reference, 4096)?;
        let direct = names
            .direct_glyphs
            .iter()
            .map(|g| (g.legacy_code, g.character))
            .collect::<Vec<_>>();
        let program = build_battle_name_runtime(&native, atlas, &pack, &ascii, &direct)?;
        let mut generator =
            build_battle_name_loader(program.decoded.len(), program.entry_address, font.len())?;
        let sprite_origin = BATTLE_NAME_GENERATOR_ORIGIN + generator.len() as u32;
        generator.extend(build_battle_name_sprite(
            sprite_origin,
            BATTLE_NAME_GENERATOR_CAPACITY - generator.len(),
        )?);
        generator.resize(BATTLE_NAME_GENERATOR_CAPACITY, 0);
        let hook = encode_le_bytes(
            &[
                Instruction::J {
                    target: sprite_origin,
                },
                Instruction::nop(),
            ],
            0x8005d288,
        )?;
        let compressed_bytes = program.encoded.len();
        let mut stored = program.encoded;
        stored.resize(original.len(), 0);
        ensure!(
            decompress(&stored, true)? == program.decoded,
            "battle supplier final padded readback differs"
        );
        let report = BattleNameBuildReport {
            source_stored_sha256: SOURCE_SHA.into(),
            source_decoded_sha256: sha256_bytes(&native),
            pack_sha256: sha256_bytes(pack.bytes()),
            ascii_font_sha256: reference.font_sha256,
            ascii_font_px: 16,
            ascii_vertical_shift_px: -1,
            decoded_sha256: sha256_bytes(&program.decoded),
            stored_sha256: sha256_bytes(&stored),
            decoded_bytes: program.decoded.len(),
            compressed_bytes,
            record_bytes: stored.len(),
            entry_address: format!("0x{:08x}", program.entry_address),
            generator_sha256: sha256_bytes(&generator),
            sprite_hook_sha256: sha256_bytes(&hook),
            cold_runtime_verified: false,
        };
        Ok(Self {
            stored,
            source: DiscRecordSourceIdentity::new(original.len(), SOURCE_SHA)?,
            report,
            generator,
            hook,
        })
    }
    pub fn validate_texture(
        &self,
        build: &crate::mode_descendant_graphics::ModeDescendantGraphicsBuild,
    ) -> Result<()> {
        let record = build
            .report
            .records
            .iter()
            .find(|r| r.source_path == "DAT2/CEFT1.BIZ")
            .context("battle names require authored CEFT1")?;
        let stored = build
            .stored_record(record.record)
            .context("missing authored CEFT1")?;
        ensure!(
            sha256_bytes(stored) == record.patched_stored_sha256,
            "battle texture differs from producer report"
        );
        let decoded = decompress(stored, true)?;
        let offset = 0x10800;
        let tim = parse_4bpp_prefix(
            decoded
                .get(offset..)
                .context("missing battle texture TIM")?,
        )?;
        ensure!(
            (
                tim.image_x,
                tim.image_y,
                tim.image_word_width,
                tim.image_height
            ) == (768, 256, 192, 256),
            "battle texture page geometry changed"
        );
        for y in 240..256 {
            let start = offset + tim.pixel_offset + y * 384 + 320;
            ensure!(
                decoded[start..start + 64].iter().all(|&b| b == 0),
                "authored CEFT1 occupies right EDIT name texture tail"
            );
        }
        Ok(())
    }
    pub fn register_executable(
        &self,
        source: &[u8],
        plan: &mut DecodedRecordWritePlan<'_>,
        machine: &mut PsxMachineCodeSources,
    ) -> Result<()> {
        ensure!(
            sha256_bytes(source) == MAIN_EXECUTABLE_SHA256,
            "battle name executable source changed"
        );
        let mut candidate = source.to_vec();
        let mut claims = Vec::new();
        for (id, origin, bytes) in [
            (
                "battle-name-generator",
                BATTLE_NAME_GENERATOR_ORIGIN,
                self.generator.as_slice(),
            ),
            ("battle-name-sprite-hook", 0x8005d288, self.hook.as_slice()),
        ] {
            let offset = (origin - 0x80010000) as usize + 0x800;
            let instructions = verify_placed_program(bytes, origin)?;
            let provenance = machine.register(id, origin, instructions)?;
            candidate[offset..offset + bytes.len()].copy_from_slice(bytes);
            claims.push(CandidateWriteClaim {
                id: id.into(),
                purpose: "render full-size EDIT names from shared name-entry glyphs".into(),
                range: offset..offset + bytes.len(),
                intent: WriteIntent::MachineCode(provenance),
            });
        }
        let prefix_offset = 0x7713c;
        let prefix = battle_catalog_prefix(&source[0x77134..0x77140], &self.stored)?;
        candidate[prefix_offset..prefix_offset + 4].copy_from_slice(&prefix);
        claims.push(CandidateWriteClaim {
            id: "battle-name-catalog-prefix".into(),
            purpose: "bind native CD first-sector validation to the rebuilt supplier".into(),
            range: prefix_offset..prefix_offset + 4,
            intent: WriteIntent::Data,
        });
        plan.register_candidate(CandidateRecordWrite {
            owner: "battle name runtime",
            source_sha256: MAIN_EXECUTABLE_SHA256,
            candidate: &candidate,
            claims,
        })
    }
}

// Native callbacks at 80015b08 and 80015c74 compare the sector's first
// word to catalog entry +8. A mismatch restarts CD loading indefinitely.
fn battle_catalog_prefix(entry: &[u8], stored: &[u8]) -> Result<[u8; 4]> {
    ensure!(
        entry == [1, 0x47, 0x68, 0, 0x74, 0x20, 0, 0, 0x62, 0x20, 0x10, 0],
        "battle supplier catalog location, extent or original signature changed"
    );
    let locator = crate::native_disc_resource::NativeDiscResource::parse(entry)?;
    ensure!(
        stored.len() == locator.byte_count as usize,
        "battle supplier catalog extent changed"
    );
    let replacement =
        crate::native_disc_resource::NativeDiscResource::replacement(locator.lba, stored)?;
    ensure!(
        replacement[..8] == entry[..8],
        "battle supplier location changed"
    );
    Ok(replacement[8..12].try_into().unwrap())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_signature_tracks_recompressed_supplier_and_rejects_changed_extent() {
        let entry = [1, 0x47, 0x68, 0, 0x74, 0x20, 0, 0, 0x62, 0x20, 0x10, 0];
        let mut stored = vec![0; 8308];
        stored[..4].copy_from_slice(&[0x6a, 0x20, 0x10, 0]);
        assert_eq!(
            battle_catalog_prefix(&entry, &stored).unwrap(),
            [0x6a, 0x20, 0x10, 0]
        );
        for i in 0..entry.len() {
            let mut changed = entry;
            changed[i] ^= 1;
            assert!(battle_catalog_prefix(&changed, &stored).is_err());
        }
        assert!(battle_catalog_prefix(&entry, &stored[..8307]).is_err());
        stored.push(0);
        assert!(battle_catalog_prefix(&entry, &stored).is_err());
    }
}
