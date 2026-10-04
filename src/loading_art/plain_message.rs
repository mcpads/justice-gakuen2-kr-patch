//! Source-bound tiles and local consumers for the separate OVER-font loading path.
use super::*;
use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedDataClaim, DecodedRecordWritePlan,
};
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::tim::RgbaImage;
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register, encode_le_bytes};

pub(crate) struct PlainLoadingBuild {
    texture: Vec<u8>,
    texture_source_sha256: String,
    texture_ranges: Vec<[usize; 2]>,
    executable: Vec<u8>,
    stream: Vec<u8>,
    columns: usize,
}

impl PlainLoadingBuild {
    pub(crate) fn register_texture(&self, plan: &mut DecodedRecordWritePlan<'_>) -> Result<()> {
        let claims = DecodedDataClaim::from_ranges(
            "plain-loading-tiles",
            "dedicated loading glyphs in the OVER atlas",
            self.texture_ranges.iter().copied(),
        );
        plan.register_data_candidate(
            "plain loading text",
            &self.texture_source_sha256,
            &self.texture,
            &claims,
        )
    }

    pub(crate) fn register_executable(
        &self,
        plan: &mut DecodedRecordWritePlan<'_>,
        sources: &mut PsxMachineCodeSources,
    ) -> Result<()> {
        let mut candidate = self.executable.clone();
        candidate[0x74afc..0x74b0c].copy_from_slice(&self.stream);
        let mut claims = vec![CandidateWriteClaim {
            id: "plain-loading-stream".into(),
            purpose: "loading-local tile indices, retaining source space semantics".into(),
            range: 0x74afc..0x74b0c,
            intent: WriteIntent::Data,
        }];
        let center = 240 + (14 - self.columns) as i16 * 8;
        let mut writes = vec![
            (
                0x48c4,
                Instruction::Addiu {
                    rt: Register::S5,
                    rs: Register::ZERO,
                    immediate: center,
                },
            ),
            (
                0x4af8,
                Instruction::Addiu {
                    rt: Register::S5,
                    rs: Register::ZERO,
                    immediate: center,
                },
            ),
        ];
        // Both loops keep their original packet, CLUT, ordering and loader flow.
        // Only the loading-local UV calculation replaces the shared ASCII lookup.
        for offset in [0, 0x224] {
            writes.extend([
                (
                    0x4968 + offset,
                    Instruction::Srl {
                        rd: Register::V1,
                        rt: Register::S2,
                        shift: 2,
                    },
                ),
                (
                    0x4970 + offset,
                    Instruction::Andi {
                        rt: Register::V0,
                        rs: Register::S2,
                        immediate: 3,
                    },
                ),
                (
                    0x4974 + offset,
                    Instruction::Sll {
                        rd: Register::V0,
                        rt: Register::V0,
                        shift: 4,
                    },
                ),
                (
                    0x4978 + offset,
                    Instruction::Addiu {
                        rt: Register::V0,
                        rs: Register::V0,
                        immediate: 192,
                    },
                ),
                (
                    0x4984 + offset,
                    Instruction::Sll {
                        rd: Register::V1,
                        rt: Register::V1,
                        shift: 4,
                    },
                ),
                (
                    0x4988 + offset,
                    Instruction::Addiu {
                        rt: Register::V1,
                        rs: Register::V1,
                        immediate: 144,
                    },
                ),
                (
                    0x498c + offset,
                    Instruction::Sll {
                        rd: Register::ZERO,
                        rt: Register::ZERO,
                        shift: 0,
                    },
                ),
            ]);
        }
        for (offset, instruction) in writes {
            let origin = 0x8000_f800 + offset as u32;
            let id = format!("plain-loading-instruction-{offset:x}");
            let instructions = vec![instruction];
            let encoded = encode_le_bytes(&instructions, origin)?;
            candidate[offset..offset + 4].copy_from_slice(&encoded);
            let provenance = sources.register(&id, origin, instructions)?;
            claims.push(CandidateWriteClaim {
                id,
                purpose: "loading-local coordinates".into(),
                range: offset..offset + 4,
                intent: WriteIntent::MachineCode(provenance),
            });
        }
        plan.register_candidate(CandidateRecordWrite {
            owner: "plain loading text",
            source_sha256: crate::source_disc::MAIN_EXECUTABLE_SHA256,
            candidate: &candidate,
            claims,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    source_record: String,
    source_decoded_sha256: String,
    text: String,
    font_role: String,
}

pub(crate) fn prepare(
    source: &SupportedSourceDisc,
    assets: &Path,
    fonts: &BTreeMap<String, SizedFontSource>,
    output: &Path,
) -> Result<PlainLoadingBuild> {
    std::fs::create_dir_all(output)?;
    let asset_bytes = std::fs::read(assets.join("plain-message.json"))?;
    let message: Message = serde_json::from_slice(&asset_bytes)?;
    ensure!(
        message.source_record == "DAT2/OVER.TIZ",
        "plain loading producer changed"
    );
    let (_, stored) = source.read_record(&message.source_record)?;
    let decoded = decompress(&stored, false)?;
    ensure!(
        sha256_bytes(&decoded) == message.source_decoded_sha256,
        "plain loading font source changed"
    );
    let (_, executable) = source.read_record(crate::source_disc::MAIN_EXECUTABLE_PATH)?;
    ensure!(
        executable.get(0x74afc..0x74b0b) == Some(b"NOW LOADING...\0"),
        "plain loading source text changed"
    );
    let style = fonts
        .get(&message.font_role)
        .context("missing plain loading font")?;
    let characters = message.text.chars().collect::<Vec<_>>();
    ensure!(
        !characters.is_empty() && characters.len() <= 14,
        "plain loading text exceeds source sprite count"
    );
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&style.path)?;
    // Render the whole phrase before tiling: Latin glyphs use their selected
    // font's advance, rather than consuming a full 16-pixel sprite each.
    // Seven tiles fit the existing source-bound allocation without extending it.
    let columns = 7;
    let width = columns * 16;
    let indices = rasterizer
        .rasterize(
            &message.text,
            width,
            16,
            style.font_px,
            0.0,
            0,
            None,
            3,
            HorizontalTextAlignment::Center,
        )?
        .pixels;
    let mut tiles = Vec::new();
    let mut texture = decoded.clone();
    let mut stream = Vec::new();
    let mut slot = 0usize;
    let mut texture_ranges = Vec::new();
    for column in 0..columns {
        let pixels = (0..16)
            .flat_map(|row| {
                indices[row * width + column * 16..row * width + (column + 1) * 16]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        if pixels.iter().all(|&pixel| pixel == 0) {
            stream.push(b' ');
            continue;
        }
        slot += 1;
        ensure!(
            slot <= 7,
            "plain loading reserved atlas has at most seven glyphs"
        );
        stream.push(0x20 + slot as u8);
        let x = 192 + (slot % 4) * 16;
        let y = 144 + (slot / 4) * 16;
        for row in 0..16 {
            let offset = 960 + (y + row) * 128 + x / 2;
            ensure!(
                decoded[offset..offset + 8].iter().all(|byte| *byte == 0),
                "plain loading allocation is not source-blank"
            );
            for column in 0..8 {
                texture[offset + column] =
                    pixels[row * 16 + column * 2] | pixels[row * 16 + column * 2 + 1] << 4;
            }
            if texture[offset..offset + 8] != decoded[offset..offset + 8] {
                texture_ranges.push([offset, offset + 8]);
            }
        }
        tiles.push(json!({"slot": slot, "column": column, "indexed_sha256":sha256_bytes(&pixels)}));
    }
    ensure!(slot > 0, "plain loading has no visible glyphs");
    stream.resize(16, 0);
    // Preserve the source loading CLUT. Index 3 is its neutral bright ink.
    let palette = decoded
        .get(628..660)
        .context("missing plain loading CLUT")?;
    let rgba = indices
        .iter()
        .flat_map(|index| {
            let color = u16::from_le_bytes([
                palette[usize::from(*index) * 2],
                palette[usize::from(*index) * 2 + 1],
            ]);
            [
                ((color & 31) * 255 / 31) as u8,
                (((color >> 5) & 31) * 255 / 31) as u8,
                (((color >> 10) & 31) * 255 / 31) as u8,
                255,
            ]
        })
        .collect();
    write_tim_preview(
        &output.join("plain-message.png"),
        &RgbaImage {
            width,
            height: 16,
            pixels: rgba,
        },
    )?;
    let packed = indices
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| pair[0] | pair[1] << 4)
        .collect::<Vec<_>>();
    std::fs::write(output.join("plain-message.4bpp"), &packed)?;
    std::fs::write(
        output.join("plain-message-authoring.json"),
        serde_json::to_vec_pretty(&json!({
            "text":message.text,"asset_sha256":sha256_bytes(&asset_bytes),"source_record":message.source_record,
            "source_decoded_sha256":message.source_decoded_sha256,"font_sha256":sha256_bytes(&std::fs::read(&style.path)?),
            "font_px":style.font_px,"width":width,"height":16,"tiles":tiles,"packed_sha256":sha256_bytes(&packed),
            "source_palette_sha256":sha256_bytes(palette),"runtime_verified":false,"contribution_prepared":true,
            "remaining":"OVER tile and EXE contributions require final disc readback and native verification; shared Latin tiles and the ASCII UV table are preserved."
        }))?,
    )?;
    Ok(PlainLoadingBuild {
        texture,
        texture_source_sha256: sha256_bytes(&decoded),
        texture_ranges,
        executable,
        stream,
        columns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the supported source disc and selected fonts"]
    fn english_loading_tiles_reconstruct_the_complete_phrase_in_reserved_atlas() -> Result<()> {
        let config = crate::development_build_spec::load_development_build_spec(Path::new(
            "assets/build/development.json",
        ))?;
        let source = SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let output = crate::test_support::temporary_directory("plain-loading-tiles");
        let build = prepare(
            &source,
            &config.assets.loading_art.unwrap(),
            &config.fonts.loading_art,
            &output,
        )?;
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output.join("plain-message-authoring.json"))?)?;
        assert_eq!(report["text"], "NOW LOADING...");
        let packed = std::fs::read(output.join("plain-message.4bpp"))?;
        let expected = packed
            .iter()
            .flat_map(|byte| [byte & 15, byte >> 4])
            .collect::<Vec<_>>();
        let width = build.columns * 16;
        let mut observed = vec![0; width * 16];
        for (column, &code) in build
            .stream
            .iter()
            .take_while(|&&code| code != 0)
            .enumerate()
        {
            if code == b' ' {
                continue;
            }
            let slot = usize::from(code - b' ');
            assert!((1..=7).contains(&slot));
            let (x, y) = (192 + slot % 4 * 16, 144 + slot / 4 * 16);
            for row in 0..16 {
                for pixel in 0..16 {
                    let byte = build.texture[960 + (y + row) * 128 + (x + pixel) / 2];
                    observed[row * width + column * 16 + pixel] = byte >> (pixel % 2 * 4) & 15;
                }
            }
        }
        assert_eq!(observed, expected);
        let (_, stored) = source.read_record("DAT2/OVER.TIZ")?;
        let original = decompress(&stored, false)?;
        for (offset, (&before, &after)) in original.iter().zip(&build.texture).enumerate() {
            if before == after {
                continue;
            }
            let row = (offset - 960) / 128;
            let x_byte = (offset - 960) % 128;
            assert!((144..176).contains(&row) && (96..128).contains(&x_byte));
            assert_eq!(before, 0);
        }
        std::fs::remove_dir_all(output)?;
        Ok(())
    }
}
