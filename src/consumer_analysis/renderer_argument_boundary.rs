use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::model::{
    LoadedImageConsumerAudit, StaticRendererArgumentDynamicBoundaryAudit,
    StaticRendererCallSiteAudit,
};
use crate::source_disc::{LoadedImage, MAIN_EXECUTABLE_PATH};

const KANRI_PATH: &str = "DAT1/KANRI.BIN";
const KANRI_RUNTIME_BASE: u32 = 0x800a_2000;
const MAIN_RUNTIME_BASE: u32 = 0x8001_0000;
const KANRI_RENDERER_ADDRESS: u32 = 0x800a_3b3c;
const KANRI_SCREEN_FUNCTION_ADDRESS: u32 = 0x800a_9a74;
const KANRI_PRESENTATION_FUNCTION_ADDRESS: u32 = 0x800a_9638;
const KANRI_RUNTIME_ARGUMENT_CALL_OFFSET: usize = 0x78d8;

pub(super) fn resolve_renderer_argument_boundaries(
    images: &[LoadedImage],
    image_reports: &mut [LoadedImageConsumerAudit],
) -> Result<()> {
    let kanri = exactly_one_image(images, KANRI_PATH)?;
    let main = exactly_one_image(images, MAIN_EXECUTABLE_PATH)?;
    validate_kanri_runtime_character_argument(kanri)?;
    validate_main_character_record_persistence(main)?;

    let kanri_report = image_reports
        .iter_mut()
        .find(|report| report.path == KANRI_PATH)
        .context("KANRI report is missing for the runtime renderer argument audit")?;
    let census = exactly_one_mut(
        &mut kanri_report.declared_renderer_call_censuses,
        |census| census.id == "kanri_menu_text_renderer_direct_calls",
        "KANRI renderer census",
    )?;
    let call_site = exactly_one_mut(
        &mut census.call_sites,
        |site| site.instruction_offset == hex_offset(KANRI_RUNTIME_ARGUMENT_CALL_OFFSET),
        "KANRI runtime-indexed renderer call",
    )?;
    ensure!(
        call_site.entrypoint_reachable,
        "KANRI runtime-indexed renderer call is no longer entrypoint-reachable"
    );
    close_dynamic_boundary(call_site);
    census.source_domain_complete = census.call_sites.iter().all(|site| {
        site.source_domain_status == "finite_source_domain"
            || site.dynamic_source_boundary.is_some()
    });
    ensure!(
        census.source_domain_complete,
        "KANRI renderer source-domain analysis still has an unclosed call site"
    );
    Ok(())
}

fn close_dynamic_boundary(call_site: &mut StaticRendererCallSiteAudit) {
    call_site.source_domain_status = "closed_dynamic_native_save_record_byte".to_string();
    call_site.evidence = "the selected record is bounded to 0..=15 under the screen's nonempty-active-mask state, but its unsigned character byte can be restored byte-for-byte from the native save payload; both unsigned-byte machine domains and the complete renderer argument chain are explicit".to_string();
    call_site.dynamic_source_boundary = Some(StaticRendererArgumentDynamicBoundaryAudit {
        status: "closed_at_native_save_or_runtime_value_boundary".to_string(),
        active_record_mask_runtime_address: hex_address(0x801f_5800),
        selected_record_index_object_offset: hex_offset(0x0e),
        active_record_count_object_offset: hex_offset(0x1c),
        valid_state_selected_record_index_range: [0, 15],
        valid_state_precondition: "the KANRI screen state enters presentation only after observing a nonzero active-record mask; the overlay initializes both selection halfwords to zero and clamps them below the 16-bit-mask population count".to_string(),
        record_table_runtime_address: hex_address(0x801f_5804),
        record_stride_bytes: 40,
        character_selector_record_offset: 0,
        character_selector_machine_value_range: [0, 255],
        mapping_table_image_offset: hex_offset(0x1634),
        mapping_table_runtime_address: hex_address(0x800a_3634),
        mapping_result_machine_value_range: [0, 255],
        placement_table_image_offset: hex_offset(0x098c),
        placement_table_runtime_address: hex_address(0x800a_298c),
        placement_record_stride_bytes: 12,
        native_save_restore_source: "call SLPS_021.20 +0x0000d530 copies 1024 bytes from *(0x801f63f0)+896 into 0x801f5800..0x801f5bff".to_string(),
        native_save_persistence_destination: "call SLPS_021.20 +0x0000d9fc copies 1024 bytes from 0x801f5800..0x801f5bff back to the native save work buffer at base+896".to_string(),
        evidence: vec![
            "KANRI +0x653c..+0x6580 computes object +0x1c as the population count of the low 16 bits at 0x801f5800".to_string(),
            "KANRI +0x65d4..+0x65e8 initializes the selection halfwords, including object +0x0e, to zero".to_string(),
            "KANRI +0x6830..+0x6858 clamps two selection halfwords below object +0x1c".to_string(),
            "KANRI +0x7ad0..+0x7af8 sends a zero active mask to state 10 instead of the state-1 presentation path".to_string(),
            "the sole direct call to the KANRI screen function is +0x66bc, and that function's sole direct call to the presentation function is +0x7b4c".to_string(),
            "KANRI +0x770c..+0x774c forms 0x801f5804 + signed_index*40; +0x78ac..+0x78dc reads the unsigned character byte, performs both unsigned mapping loads, scales the result by 12, and calls the renderer with a record rooted at 0x800a298c".to_string(),
            "SLPS_021.20 +0x0d3bc..+0x0d534 restores the complete 1024-byte character-record block through the validated byte-copy routine at 0x80060088".to_string(),
            "SLPS_021.20 +0x0d9f0..+0x0da00 persists the same complete block back into the native save work buffer".to_string(),
        ],
        limitations: vec![
            "The 0..=15 record-index range is a valid-screen-state contract, not a claim that arbitrary external mutation of the KANRI object is safe.".to_string(),
            "The native save restore path admits an unsigned character byte without a source-visible semantic range check. Static analysis therefore stops at the exact 0..=255 machine domain and does not infer the nearby mapping table's intended character count.".to_string(),
            "This closes the static argument-analysis boundary. It does not prove which character record is selected on a route, that every mapped byte denotes an authored placement record, or that the resulting surface is visually approved.".to_string(),
        ],
    });
}

fn validate_kanri_runtime_character_argument(image: &LoadedImage) -> Result<()> {
    ensure!(
        image.runtime_base == Some(KANRI_RUNTIME_BASE),
        "KANRI runtime renderer argument profile moved from its source-bound base"
    );
    ensure_direct_calls(
        image,
        KANRI_SCREEN_FUNCTION_ADDRESS,
        &[0x66bc],
        "KANRI screen function",
    )?;
    ensure_direct_calls(
        image,
        KANRI_PRESENTATION_FUNCTION_ADDRESS,
        &[0x7b4c],
        "KANRI presentation function",
    )?;
    ensure_sequence(
        image,
        &[
            (0x653c, lui(Register::V1, 0x801f)),
            (0x6540, ori(Register::V1, Register::V1, 0x5800)),
            (0x6544, lhu(Register::V0, Register::V1, 0)),
            (0x6548, addu(Register::A0, Register::ZERO, Register::ZERO)),
            (0x654c, addu(Register::A1, Register::V0, Register::ZERO)),
            (0x6550, sw(Register::A1, Register::V1, 0)),
            (0x6554, sw(Register::ZERO, Register::S0, 0x1c)),
            (0x6558, andi(Register::V0, Register::A1, 1)),
            (0x655c, beq(Register::V0, Register::ZERO, 0x800a_8574)),
            (0x6564, lw(Register::V0, Register::S0, 0x1c)),
            (0x6568, srl(Register::A1, Register::A1, 1)),
            (0x656c, addiu(Register::V0, Register::V0, 1)),
            (0x6570, sw(Register::V0, Register::S0, 0x1c)),
            (0x6574, addiu(Register::A0, Register::A0, 1)),
            (0x6578, slti(Register::V0, Register::A0, 16)),
            (0x657c, bne(Register::V0, Register::ZERO, 0x800a_855c)),
            (0x6580, andi(Register::V0, Register::A1, 1)),
        ],
        "KANRI active-record population count",
    )?;
    ensure_sequence(
        image,
        &[
            (0x65d4, addiu(Register::A0, Register::ZERO, 7)),
            (0x65d8, addiu(Register::V0, Register::S0, 0x0e)),
            (0x65dc, sh(Register::ZERO, Register::V0, 0x0c)),
            (0x65e0, addiu(Register::A0, Register::A0, -1)),
            (0x65e4, bgez(Register::A0, 0x800a_85dc)),
            (0x65e8, addiu(Register::V0, Register::V0, -2)),
        ],
        "KANRI selection-halfword initialization",
    )?;
    ensure_sequence(
        image,
        &[
            (0x6830, lh(Register::V0, Register::A1, 0x0e)),
            (0x6834, lw(Register::V1, Register::S0, 0x1c)),
            (0x683c, slt(Register::V0, Register::V0, Register::V1)),
            (0x6840, bne(Register::V0, Register::ZERO, 0x800a_884c)),
            (0x6844, addiu(Register::V0, Register::V1, -1)),
            (0x6848, sh(Register::V0, Register::A1, 0x0e)),
            (0x684c, addiu(Register::A0, Register::A0, 1)),
            (0x6850, slti(Register::V0, Register::A0, 2)),
            (0x6854, bne(Register::V0, Register::ZERO, 0x800a_8830)),
            (0x6858, addiu(Register::A1, Register::A1, 2)),
        ],
        "KANRI selected-record clamp",
    )?;
    ensure_sequence(
        image,
        &[
            (0x7ad0, sh(Register::ZERO, Register::S0, 0x16)),
            (0x7ad4, lui(Register::V0, 0x801f)),
            (0x7ad8, lw(Register::V0, Register::V0, 0x5800)),
            (0x7ae0, beq(Register::V0, Register::ZERO, 0x800a_9af4)),
            (0x7ae4, addiu(Register::V0, Register::ZERO, 10)),
            (0x7ae8, lbu(Register::V0, Register::S0, 1)),
            (0x7af0, addiu(Register::V0, Register::V0, 1)),
            (0x7af4, sb(Register::V0, Register::S0, 1)),
            (0x7af8, jump(0x800a_a148)),
        ],
        "KANRI nonempty-active-mask presentation gate",
    )?;
    ensure_sequence(
        image,
        &[
            (0x770c, lui(Register::S2, 0x801f)),
            (0x7710, ori(Register::S2, Register::S2, 0x5804)),
            (0x772c, lh(Register::V1, Register::S5, 0x0e)),
            (0x7740, sll(Register::S0, Register::V1, 2)),
            (0x7744, addu(Register::S0, Register::S0, Register::V1)),
            (0x7748, sll(Register::S0, Register::S0, 3)),
            (0x774c, addu(Register::S2, Register::S0, Register::S2)),
            (0x78ac, lbu(Register::V0, Register::S2, 0)),
            (0x78b4, lui(Register::AT, 0x800a)),
            (0x78b8, addu(Register::AT, Register::AT, Register::V0)),
            (0x78bc, lbu(Register::V0, Register::AT, 0x3634)),
            (0x78c4, sll(Register::A1, Register::V0, 1)),
            (0x78c8, addu(Register::A1, Register::A1, Register::V0)),
            (0x78cc, sll(Register::A1, Register::A1, 2)),
            (0x78d0, lui(Register::V0, 0x800a)),
            (0x78d4, addiu(Register::V0, Register::V0, 0x298c)),
            (
                0x78d8,
                Instruction::Jal {
                    target: KANRI_RENDERER_ADDRESS,
                },
            ),
            (0x78dc, addu(Register::A1, Register::A1, Register::V0)),
        ],
        "KANRI runtime character renderer argument",
    )
}

fn validate_main_character_record_persistence(image: &LoadedImage) -> Result<()> {
    ensure!(
        image.runtime_base == Some(MAIN_RUNTIME_BASE),
        "main character-record persistence profile moved from its source-bound base"
    );
    ensure_sequence(
        image,
        &[
            (0x337c, lui(Register::A0, 0x801f)),
            (0x3380, ori(Register::A0, Register::A0, 0x5800)),
            (
                0x3384,
                Instruction::Jal {
                    target: 0x8006_00c8,
                },
            ),
            (0x3388, addiu(Register::A1, Register::ZERO, 0x1000)),
        ],
        "main character-record zero initialization",
    )?;
    ensure_sequence(
        image,
        &[
            (0xd3bc, lui(Register::V0, 0x801f)),
            (0xd3c0, lw(Register::V0, Register::V0, 0x63f0)),
            (0xd3d0, addiu(Register::S0, Register::V0, 0x0200)),
            (0xd524, addiu(Register::A0, Register::S0, 0x0180)),
            (0xd528, lui(Register::A1, 0x801f)),
            (0xd52c, ori(Register::A1, Register::A1, 0x5800)),
            (
                0xd530,
                Instruction::Jal {
                    target: 0x8006_0088,
                },
            ),
            (0xd534, addiu(Register::A2, Register::ZERO, 0x0400)),
        ],
        "main native-save character-record restore",
    )?;
    ensure_sequence(
        image,
        &[
            (0xd9f0, lui(Register::A0, 0x801f)),
            (0xd9f4, ori(Register::A0, Register::A0, 0x5800)),
            (0xd9f8, addiu(Register::A1, Register::S1, 0x0380)),
            (
                0xd9fc,
                Instruction::Jal {
                    target: 0x8006_0088,
                },
            ),
            (0xda00, addiu(Register::A2, Register::ZERO, 0x0400)),
        ],
        "main native-save character-record persistence",
    )?;
    ensure_sequence(
        image,
        &[
            (0x50088, beq(Register::A0, Register::ZERO, 0x8006_00b4)),
            (0x50090, blez(Register::A2, 0x8006_00b0)),
            (0x50098, lbu(Register::V0, Register::A0, 0)),
            (0x5009c, addiu(Register::A0, Register::A0, 1)),
            (0x500a0, addiu(Register::A2, Register::A2, -1)),
            (0x500a4, sb(Register::V0, Register::A1, 0)),
            (0x500a8, bgtz(Register::A2, 0x8006_0098)),
            (0x500ac, addiu(Register::A1, Register::A1, 1)),
        ],
        "main byte-copy routine",
    )
}

fn exactly_one_image<'a>(images: &'a [LoadedImage], path: &str) -> Result<&'a LoadedImage> {
    let mut matches = images.iter().filter(|image| image.path == path);
    let image = matches
        .next()
        .with_context(|| format!("required loaded image {path} is missing"))?;
    ensure!(
        matches.next().is_none(),
        "required loaded image {path} is duplicated"
    );
    Ok(image)
}

fn exactly_one_mut<'a, T>(
    values: &'a mut [T],
    predicate: impl Fn(&T) -> bool,
    role: &str,
) -> Result<&'a mut T> {
    let indexes = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| predicate(value).then_some(index))
        .collect::<Vec<_>>();
    ensure!(indexes.len() == 1, "expected exactly one {role}");
    Ok(&mut values[indexes[0]])
}

fn ensure_direct_calls(
    image: &LoadedImage,
    target: u32,
    expected_offsets: &[usize],
    role: &str,
) -> Result<()> {
    let runtime_base = image
        .runtime_base
        .context("loaded image has no runtime base")?;
    let observed = image
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(word_index, bytes)| {
            let offset = word_index * 4;
            let pc = runtime_base + offset as u32;
            (direct_jump_target(u32::from_le_bytes(*bytes), pc) == Some(target)).then_some(offset)
        })
        .collect::<BTreeSet<_>>();
    let expected = expected_offsets.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        observed == expected,
        "{role} direct-call set changed; expected {}, observed {}",
        display_offsets(&expected),
        display_offsets(&observed),
    );
    Ok(())
}

fn ensure_sequence(
    image: &LoadedImage,
    expected: &[(usize, Instruction)],
    role: &str,
) -> Result<()> {
    let runtime_base = image
        .runtime_base
        .context("loaded image has no runtime base")?;
    for (offset, expected_instruction) in expected {
        let bytes: [u8; 4] = image
            .data
            .get(*offset..*offset + 4)
            .with_context(|| format!("{role} instruction +0x{offset:04x} is truncated"))?
            .try_into()?;
        let instruction = decode(u32::from_le_bytes(bytes), runtime_base + *offset as u32)
            .with_context(|| format!("failed to decode {role} instruction +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "{role} grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn direct_jump_target(word: u32, pc: u32) -> Option<u32> {
    (word >> 26 == 0x03).then_some((pc.wrapping_add(4) & 0xf000_0000) | ((word & 0x03ff_ffff) << 2))
}

fn display_offsets(offsets: &BTreeSet<usize>) -> String {
    offsets
        .iter()
        .map(|offset| hex_offset(*offset))
        .collect::<Vec<_>>()
        .join(", ")
}

fn lui(rt: Register, immediate: u16) -> Instruction {
    Instruction::Lui { rt, immediate }
}

fn ori(rt: Register, rs: Register, immediate: u16) -> Instruction {
    Instruction::Ori { rt, rs, immediate }
}

fn addiu(rt: Register, rs: Register, immediate: i16) -> Instruction {
    Instruction::Addiu { rt, rs, immediate }
}

fn addu(rd: Register, rs: Register, rt: Register) -> Instruction {
    Instruction::Addu { rd, rs, rt }
}

fn sll(rd: Register, rt: Register, shift: u8) -> Instruction {
    Instruction::Sll { rd, rt, shift }
}

fn srl(rd: Register, rt: Register, shift: u8) -> Instruction {
    Instruction::Srl { rd, rt, shift }
}

fn slt(rd: Register, rs: Register, rt: Register) -> Instruction {
    Instruction::Slt { rd, rs, rt }
}

fn slti(rt: Register, rs: Register, immediate: i16) -> Instruction {
    Instruction::Slti { rt, rs, immediate }
}

fn andi(rt: Register, rs: Register, immediate: u16) -> Instruction {
    Instruction::Andi { rt, rs, immediate }
}

fn lw(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lw { rt, base, offset }
}

fn lhu(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lhu { rt, base, offset }
}

fn lh(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lh { rt, base, offset }
}

fn lbu(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Lbu { rt, base, offset }
}

fn sw(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sw { rt, base, offset }
}

fn sh(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sh { rt, base, offset }
}

fn sb(rt: Register, base: Register, offset: i16) -> Instruction {
    Instruction::Sb { rt, base, offset }
}

fn beq(rs: Register, rt: Register, target: u32) -> Instruction {
    Instruction::Beq { rs, rt, target }
}

fn bne(rs: Register, rt: Register, target: u32) -> Instruction {
    Instruction::Bne { rs, rt, target }
}

fn blez(rs: Register, target: u32) -> Instruction {
    Instruction::Blez { rs, target }
}

fn bgez(rs: Register, target: u32) -> Instruction {
    Instruction::Bgez { rs, target }
}

fn bgtz(rs: Register, target: u32) -> Instruction {
    Instruction::Bgtz { rs, target }
}

fn jump(target: u32) -> Instruction {
    Instruction::J { target }
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:08x}")
}
