//! Enlarge adjacent private glyph pairs inside the native title packet arena.
use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::menu_atlas_plan::MenuGlyphAllocation;
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Assembler, Instruction as I, Register as R};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const START: usize = 0x1f10;
pub(super) const END: usize = 0x2114;
const ORIGIN: u32 = 0x8017a000 + START as u32;
const TEMPLATE_OFFSET: usize = 0x150;
const PACKET_BASE: u32 = 0x801d05d4;
const PACKET_CAPACITY: usize = 7 * 56;
const QUAD_BYTES: usize = 40;

fn groups(text: &str) -> Result<Vec<(Vec<char>, u16)>> {
    let length = text.chars().count();
    ensure!(
        (1..=7).contains(&length),
        "PASS title exceeds its native heading width"
    );
    let mut result = Vec::new();
    let mut x = (512 - length * 40) / 2;
    for word in text.split(' ') {
        ensure!(!word.is_empty(), "PASS title has an empty word");
        let chars = word.chars().collect::<Vec<_>>();
        for pair in chars.chunks(2) {
            result.push((pair.to_vec(), u16::try_from(x)?));
            x += pair.len() * 40;
        }
        x += 40;
    }
    ensure!(
        result.len() * QUAD_BYTES * 2 <= PACKET_CAPACITY,
        "PASS title exceeds both native packet buffers"
    );
    let characters = result
        .iter()
        .flat_map(|(chars, _)| chars.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        characters.iter().copied().collect::<BTreeSet<_>>().len() == characters.len(),
        "PASS title pair allocation requires distinct glyphs"
    );
    Ok(result)
}

pub(super) fn templates(
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    text: &str,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    for (characters, x) in groups(text)? {
        let code = allocation
            .get(&characters[0])
            .context("missing PASS title glyph")?
            .code;
        for (i, c) in characters.iter().enumerate() {
            ensure!(
                allocation.get(c).map(|g| g.code) == Some(code + u16::try_from(i)?),
                "PASS title glyph pair is not adjacent"
            );
        }
        let u = (code & 15) * 20;
        let v = ((code >> 4) & 15) * 20;
        let width = u16::try_from(characters.len() * 20)?;
        ensure!(
            code < 0x400 && u + width <= 240 && v + 20 <= 240,
            "PASS title texture crosses its page"
        );
        let mut packet = [0u8; QUAD_BYTES];
        packet[..4].copy_from_slice(&0x09000000u32.to_le_bytes());
        // Modulate the existing white-filled EDIT palette to the source title's
        // full green channel; no shared palette or original title pixel changes.
        packet[4..8].copy_from_slice(&[0, 128, 0, 0x2c]);
        let clut = (481u16 << 6) | (240 / 16);
        let page = 12 + (code >> 8);
        for (i, (dx, dy)) in [(0, 0), (width, 0), (0, 20), (width, 20)]
            .into_iter()
            .enumerate()
        {
            let o = 8 + i * 8;
            packet[o..o + 2].copy_from_slice(&(x + dx * 2).to_le_bytes());
            packet[o + 2..o + 4].copy_from_slice(&(28u16 + dy * 2).to_le_bytes());
            // Quad endpoints are inclusive texels; including the next cell
            // produces visible neighboring glyph pixels when enlarged.
            packet[o + 4] = u8::try_from(u + dx.saturating_sub(1))?;
            packet[o + 5] = u8::try_from(v + dy.saturating_sub(1))?;
            let attribute = if i == 0 {
                clut
            } else if i == 1 {
                page
            } else {
                0
            };
            packet[o + 6..o + 8].copy_from_slice(&attribute.to_le_bytes());
        }
        result.extend_from_slice(&packet);
    }
    Ok(result)
}

fn program(quad_count: usize) -> Result<Vec<I>> {
    let mut a = Assembler::new();
    a.emit(I::Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -32,
    });
    for (reg, offset) in [(R::S0, 16), (R::S1, 20), (R::S2, 24), (R::RA, 28)] {
        a.emit(I::Sw {
            rt: reg,
            base: R::SP,
            offset,
        });
    }
    a.emit(I::Lui {
        rt: R::T0,
        immediate: 0x801f,
    })
    .emit(I::Lw {
        rt: R::T0,
        base: R::T0,
        offset: 0x608c,
    })
    .emit(I::Lui {
        rt: R::S0,
        immediate: 0x801d,
    })
    .emit(I::Addiu {
        rt: R::S0,
        rs: R::S0,
        immediate: (PACKET_BASE & 0xffff) as i16,
    })
    .emit(I::Sll {
        rd: R::T1,
        rt: R::T0,
        shift: 5,
    })
    .emit(I::Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 3,
    })
    .emit(I::Addu {
        rd: R::T0,
        rs: R::T0,
        rt: R::T1,
    });
    // Each quad retains adjacent front/back packets; advance by 80 per quad.
    a.emit(I::Addu {
        rd: R::S0,
        rs: R::S0,
        rt: R::T0,
    })
    .emit(I::Lui {
        rt: R::S1,
        immediate: 0x8018,
    })
    .emit(I::Addiu {
        rt: R::S1,
        rs: R::S1,
        immediate: (ORIGIN + TEMPLATE_OFFSET as u32) as i16,
    })
    .emit(I::Addiu {
        rt: R::S2,
        rs: R::ZERO,
        immediate: i16::try_from(quad_count)?,
    })
    .label("quad");
    for offset in (0..40).step_by(4) {
        a.emit(I::Lw {
            rt: R::T0,
            base: R::S1,
            offset,
        })
        .emit(I::nop())
        .emit(I::Sw {
            rt: R::T0,
            base: R::S0,
            offset,
        });
    }
    a.emit(I::Lui {
        rt: R::V0,
        immediate: 0x801f,
    })
    .emit(I::Lw {
        rt: R::V0,
        base: R::V0,
        offset: 0x6360,
    })
    .emit(I::Lui {
        rt: R::A0,
        immediate: 0x801f,
    })
    .emit(I::Lw {
        rt: R::A0,
        base: R::A0,
        offset: 0x6090,
    })
    .emit(I::Lw {
        rt: R::V0,
        base: R::V0,
        offset: 0,
    })
    .emit(I::Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: 0x1060,
    })
    .emit(I::Jalr {
        rd: R::RA,
        rs: R::V0,
    })
    .emit(I::Addu {
        rd: R::A1,
        rs: R::S0,
        rt: R::ZERO,
    })
    .emit(I::Addiu {
        rt: R::S0,
        rs: R::S0,
        immediate: 80,
    })
    .emit(I::Addiu {
        rt: R::S1,
        rs: R::S1,
        immediate: 40,
    })
    .emit(I::Addiu {
        rt: R::S2,
        rs: R::S2,
        immediate: -1,
    })
    .bne(R::S2, R::ZERO, "quad")
    .emit(I::nop())
    .emit(I::Lui {
        rt: R::V0,
        immediate: 0x801f,
    })
    .emit(I::Lw {
        rt: R::V0,
        base: R::V0,
        offset: 0x6360,
    })
    .emit(I::Addiu {
        rt: R::A0,
        rs: R::ZERO,
        immediate: 2,
    })
    .emit(I::Lw {
        rt: R::V0,
        base: R::V0,
        offset: 0x28c,
    })
    .emit(I::Addiu {
        rt: R::A1,
        rs: R::ZERO,
        immediate: 0x2a,
    })
    .emit(I::Jalr {
        rd: R::RA,
        rs: R::V0,
    })
    .emit(I::Addiu {
        rt: R::A2,
        rs: R::ZERO,
        immediate: 0x150,
    });
    for (reg, offset) in [(R::S0, 16), (R::S1, 20), (R::S2, 24), (R::RA, 28)] {
        a.emit(I::Lw {
            rt: reg,
            base: R::SP,
            offset,
        });
    }
    a.emit(I::Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 32,
    })
    .emit(I::Jr { rs: R::RA })
    .emit(I::nop());
    let placed = a.assemble(ORIGIN)?;
    ensure!(
        placed.bytes().len() <= TEMPLATE_OFFSET,
        "PASS title program overlaps its templates"
    );
    Ok(placed.instructions().to_vec())
}

pub(super) fn install(
    source: &[u8],
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    text: &str,
) -> Result<Vec<u8>> {
    super::pass_password_title::validate_password_title(source)?;
    let templates = templates(allocation, text)?;
    let instructions = program(templates.len() / QUAD_BYTES)?;
    let code_end = START + instructions.len() * 4;
    let data_start = START + TEMPLATE_OFFSET;
    let data_end = data_start + templates.len();
    ensure!(
        data_end <= END,
        "PASS title templates exceed original renderer"
    );
    let mut candidate = source.to_vec();
    for (i, instruction) in instructions.iter().enumerate() {
        candidate[START + i * 4..START + i * 4 + 4].copy_from_slice(
            &psx_r3000a::encode(instruction, ORIGIN + i as u32 * 4)?.to_le_bytes(),
        );
    }
    candidate[data_start..data_end].copy_from_slice(&templates);
    let mut sources = PsxMachineCodeSources::default();
    let provenance = sources.register("pass-password-title", ORIGIN, instructions)?;
    let hash = sha256_bytes(source);
    let mut plan = DecodedRecordWritePlan::new("DAT1/PASS.BIN", source, &hash)?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "PASS Korean password title",
        source_sha256: &hash,
        candidate: &candidate,
        claims: vec![
            CandidateWriteClaim {
                id: "pass-title-program".into(),
                purpose: "render enlarged private glyph pairs".into(),
                range: START..code_end,
                intent: WriteIntent::MachineCode(provenance),
            },
            CandidateWriteClaim {
                id: "pass-title-templates".into(),
                purpose: "source-bound GPU quad templates".into(),
                range: data_start..data_end,
                intent: WriteIntent::Data,
            },
        ],
    })?;
    plan.apply(Some(&sources))
}

pub(super) fn glyph_groups(text: &str) -> Result<Vec<Vec<char>>> {
    Ok(groups(text)?
        .into_iter()
        .map(|(characters, _)| characters)
        .collect())
}
