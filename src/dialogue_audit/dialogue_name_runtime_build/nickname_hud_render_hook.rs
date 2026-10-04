use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const NICKNAME_HUD_RENDER_ADDRESS: u32 = 0x800a_f5e0;
const NICKNAME_HUD_RENDER_BYTE_COUNT: usize = 8;
const NICKNAME_HUD_RENDER_SOURCE_SHA256: &str =
    "343b46d29d9b42e4e012fff19e34b22e53f1b2fc2d6025b0dfc2cb27385868f7";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NicknameHudRenderHookReport {
    pub byte_range: [usize; 2],
    pub address_range: [String; 2],
    pub source_sha256: String,
    pub source_instructions: Vec<String>,
    pub replacement_sha256: String,
    pub replacement_instructions: Vec<String>,
    pub wrapper_address: String,
    pub native_loader_called_by_wrapper: bool,
    pub returns_to_native_renderer_continuation: bool,
    pub uploads_after_each_native_texture_load: bool,
    pub caller_return_address_preserved: bool,
    pub source_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_nickname_hud_render_hook(
    source: &[u8],
    patched: &mut [u8],
    wrapper_address: u32,
) -> Result<NicknameHudRenderHookReport> {
    let offset = usize::try_from(
        NICKNAME_HUD_RENDER_ADDRESS
            .checked_sub(MGAME_RUNTIME_BASE)
            .context("nickname HUD render hook precedes MGAME runtime")?,
    )?;
    let end = offset + NICKNAME_HUD_RENDER_BYTE_COUNT;
    let source_bytes = source
        .get(offset..end)
        .context("nickname HUD renderer source is truncated")?;
    ensure!(
        sha256_bytes(source_bytes) == NICKNAME_HUD_RENDER_SOURCE_SHA256,
        "nickname HUD renderer source bytes changed"
    );
    ensure!(
        patched.get(offset..end) == Some(source_bytes),
        "another MGAME writer changed the nickname HUD renderer"
    );

    let source_instructions = [
        Instruction::Jalr {
            rd: Register::RA,
            rs: Register::V0,
        },
        Instruction::Addu {
            rd: Register::S0,
            rs: Register::S0,
            rt: Register::V1,
        },
    ];
    verify_instructions(
        source_bytes,
        NICKNAME_HUD_RENDER_ADDRESS,
        &source_instructions,
        "source",
    )?;

    let (replacement_address, replacement) =
        build_nickname_hud_render_replacement(wrapper_address)?;
    let replacement_bytes = encode_instructions(replacement_address, &replacement)?;
    patched
        .get_mut(offset..end)
        .context("nickname HUD renderer destination is truncated")?
        .copy_from_slice(&replacement_bytes);
    verify_instructions(
        &patched[offset..end],
        NICKNAME_HUD_RENDER_ADDRESS,
        &replacement,
        "replacement",
    )?;

    Ok(NicknameHudRenderHookReport {
        byte_range: [offset, end],
        address_range: [
            hex_address(NICKNAME_HUD_RENDER_ADDRESS),
            hex_address(NICKNAME_HUD_RENDER_ADDRESS + NICKNAME_HUD_RENDER_BYTE_COUNT as u32),
        ],
        source_sha256: NICKNAME_HUD_RENDER_SOURCE_SHA256.to_string(),
        source_instructions: source_instructions
            .iter()
            .map(|instruction| format!("{instruction:?}"))
            .collect(),
        replacement_sha256: sha256_bytes(&replacement_bytes),
        replacement_instructions: replacement
            .iter()
            .map(|instruction| format!("{instruction:?}"))
            .collect(),
        wrapper_address: hex_address(wrapper_address),
        native_loader_called_by_wrapper: true,
        returns_to_native_renderer_continuation: true,
        uploads_after_each_native_texture_load: true,
        caller_return_address_preserved: true,
        source_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn build_nickname_hud_render_replacement(
    wrapper_address: u32,
) -> Result<(u32, Vec<Instruction>)> {
    ensure!(
        wrapper_address.is_multiple_of(4),
        "nickname HUD render wrapper is unaligned"
    );
    Ok((
        NICKNAME_HUD_RENDER_ADDRESS,
        vec![
            Instruction::Jal {
                target: wrapper_address,
            },
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::S0,
                rt: Register::V1,
            },
        ],
    ))
}

fn encode_instructions(origin: u32, instructions: &[Instruction]) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for (index, instruction) in instructions.iter().enumerate() {
        let address = origin + u32::try_from(index * 4)?;
        bytes.extend_from_slice(&encode(instruction, address)?.to_le_bytes());
    }
    Ok(bytes)
}

fn verify_instructions(
    bytes: &[u8],
    origin: u32,
    expected: &[Instruction],
    role: &str,
) -> Result<()> {
    ensure!(
        bytes.len() == expected.len() * 4,
        "nickname HUD renderer {role} byte count changed"
    );
    for (index, instruction) in expected.iter().enumerate() {
        let offset = index * 4;
        let address = origin + u32::try_from(offset)?;
        ensure!(
            decode(
                u32::from_le_bytes(bytes[offset..offset + 4].try_into()?),
                address,
            )? == *instruction,
            "nickname HUD renderer {role} instruction changed"
        );
    }
    Ok(())
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
