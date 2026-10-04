//! Source-bound PLSEL1 hooks, composed through the common Expected Write path.
use super::{
    build_selector_bootstrap_entry,
    selector_loader::{overlaps, ram_range},
};
use crate::{
    decoded_record_write_plan::{
        CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
    },
    psx_machine_code_sources::PsxMachineCodeSources,
};
use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction::*, Register as R, encode_le_bytes, verify_placed_program};

const SOURCE_SHA256: &str = "3e791ff2c265bf85a2b11c36119e24586d41450a3a81647f77f18f7b6b9a5b35";
const ORIGIN: u32 = 0x800a2000;
const SOURCE_LEN: usize = 24020;

pub struct SelectorSourceHooks {
    pub bytes: Vec<u8>,
    pub code_ranges: Vec<[usize; 2]>,
}

pub fn build_selector_source_hooks(
    source: &[u8],
    bootstrap: u32,
    dispatch: u32,
) -> Result<SelectorSourceHooks> {
    build_hooks(source, SOURCE_SHA256, bootstrap, dispatch)
}

fn build_hooks(
    source: &[u8],
    expected_hash: &str,
    bootstrap: u32,
    dispatch: u32,
) -> Result<SelectorSourceHooks> {
    ensure!(source.len() == SOURCE_LEN, "PLSEL1 source extent changed");
    let mut plan = DecodedRecordWritePlan::new("PLSEL1.BIN", source, expected_hash)?;
    ensure!(
        dispatch.is_multiple_of(4)
            && !overlaps(&ram_range(dispatch, 4)?, &ram_range(ORIGIN, source.len())?),
        "selector dispatch target overlaps source overlay or leaves RAM"
    );
    let entry = build_selector_bootstrap_entry(&source[0xbb0..0xbc0], bootstrap)?;
    let patches = vec![
        (
            0xbb0,
            vec![0x3c02801fu32, 0x8c426360, 0x3c04800d, 0x8c420150],
            verify_placed_program(&entry, ORIGIN + 0xbb0)?,
        ),
        (
            0x2874,
            vec![0x2e820100, 0x1040005a],
            vec![J { target: dispatch }, psx_r3000a::Instruction::nop()],
        ),
        (
            0x2970,
            vec![0x00131040, 0x00531021, 0x00021080],
            vec![
                Sll {
                    rd: R::V0,
                    rt: R::S3,
                    shift: 4,
                },
                psx_r3000a::Instruction::nop(),
                psx_r3000a::Instruction::nop(),
            ],
        ),
        (
            0x29e4,
            vec![0x26d6000c],
            vec![Addiu {
                rt: R::S6,
                rs: R::S6,
                immediate: super::SELECTOR_NAME_ADVANCE,
            }],
        ),
    ];
    let mut bytes = source.to_vec();
    let mut claims = Vec::new();
    let mut code_ranges = Vec::new();
    let mut verifier = PsxMachineCodeSources::default();
    for (offset, expected, instructions) in patches {
        let original: Vec<u8> = expected.iter().flat_map(|v| v.to_le_bytes()).collect();
        let end = offset + original.len();
        ensure!(
            source[offset..end] == original,
            "PLSEL1 source hook at {offset:x} changed"
        );
        let id = format!("selector-name-hook-{offset:x}");
        let address = ORIGIN + u32::try_from(offset)?;
        let replacement = encode_le_bytes(&instructions, address)?;
        ensure!(
            replacement.len() == original.len(),
            "selector hook changed instruction extent"
        );
        let provenance = verifier.register(&id, address, instructions)?;
        bytes[offset..end].copy_from_slice(&replacement);
        claims.push(CandidateWriteClaim {
            id,
            purpose: "load and display complete selector names".into(),
            range: offset..end,
            intent: WriteIntent::MachineCode(provenance),
        });
        code_ranges.push([offset, end]);
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: "selector-name-runtime",
        source_sha256: expected_hash,
        candidate: &bytes,
        claims,
    })?;
    let bytes = plan.apply(Some(&verifier))?;
    Ok(SelectorSourceHooks { bytes, code_ranges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_input::runtime_test_machine::execute;
    use crate::pipeline::sha256_bytes;
    #[test]
    fn hooks_change_only_bound_sites_and_keep_four_character_spacing_consistent() {
        let mut source = vec![0x55; SOURCE_LEN];
        for (offset, words) in [
            (
                0xbb0,
                vec![0x3c02801fu32, 0x8c426360, 0x3c04800d, 0x8c420150],
            ),
            (0x2874, vec![0x2e820100, 0x1040005a]),
            (0x2970, vec![0x00131040, 0x00531021, 0x00021080]),
            (0x29e4, vec![0x26d6000c]),
        ] {
            let bytes: Vec<_> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
            source[offset..offset + bytes.len()].copy_from_slice(&bytes);
        }
        let hash = sha256_bytes(&source);
        let result = build_hooks(&source, &hash, 0xa00dc340, 0x800a87e8).unwrap();
        for (i, &b) in source.iter().enumerate() {
            if !result.code_ranges.iter().any(|&[a, z]| (a..z).contains(&i)) {
                assert_eq!(result.bytes[i], b);
            }
        }
        for length in 0..=4 {
            let mut code = result.bytes[0x2970..0x297c].to_vec();
            code.extend(
                encode_le_bytes(
                    &[Jr { rs: R::RA }, psx_r3000a::Instruction::nop()],
                    ORIGIN + 0x297c,
                )
                .unwrap(),
            );
            let mut r = [0; 32];
            r[19] = length;
            r[31] = 0x800f0000;
            let mut memory = vec![0; 0x200000];
            execute(&code, ORIGIN + 0x2970, &mut r, &mut memory);
            assert_eq!(r[2], length * 16);
            let mut code = result.bytes[0x29e4..0x29e8].to_vec();
            code.extend(
                encode_le_bytes(
                    &[Jr { rs: R::RA }, psx_r3000a::Instruction::nop()],
                    ORIGIN + 0x29e8,
                )
                .unwrap(),
            );
            r[22] = length * 16;
            execute(&code, ORIGIN + 0x29e4, &mut r, &mut memory);
            assert_eq!(r[22], (length + 1) * 16);
        }
        for [start, end] in result.code_ranges {
            for i in start..end {
                let mut bad = source.clone();
                bad[i] ^= 1;
                assert!(build_hooks(&bad, &sha256_bytes(&bad), 0xa00dc340, 0x800a87e8).is_err());
            }
        }
        assert!(build_selector_source_hooks(&source, 0xa00dc340, 0x800a87e8).is_err());
        assert!(build_hooks(&source, &hash, 0xa00dc340, 0x800a4874).is_err());
    }
}
