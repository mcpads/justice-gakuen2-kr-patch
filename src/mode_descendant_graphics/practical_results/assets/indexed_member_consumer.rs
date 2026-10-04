//! Exact producer-path validation for the two indexed `SIKEN20` members.

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::tim::{Tim4bpp, Tim8bpp, parse_4bpp_prefix, parse_8bpp_prefix};

const RUNTIME_BASE: u32 = 0x800a_2000;
const LOAD_BUFFER: u32 = 0x800a_a000;
const KNOWN_POST_UPLOAD_DESCRIPTOR_ALIASES: [u16; 3] = [46, 47, 55];
const DIRECT_DESCRIPTOR_RENDER_CALL_OFFSETS: [usize; 2] = [0x3f34, 0x3f68];
const RESULT_TIM_UPLOAD_CALL_OFFSETS: [usize; 4] = [0x36f8, 0x3714, 0x3764, 0x37b0];
const DELEGATED_RESULT_CALLBACK_INDEX: usize = 1;
const DELEGATED_RESULT_DISPATCH_OFFSET: usize = 0x65d0;
const SIKEN20_MEMBER_SIZE: usize = 0x6d000;
const SIKEN20_MEMBER_COUNT: usize = 2;
const SIKENKK_MEMBER_SIZE: usize = 0x708;
const SIKENKK_MEMBER_COUNT: usize = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mode_descendant_graphics::practical_results) struct Siken20IndexedResultConsumerPath {
    pub(in crate::mode_descendant_graphics::practical_results) selected_member_count: usize,
    pub(in crate::mode_descendant_graphics::practical_results) known_post_upload_descriptor_aliases:
        [u16; 3],
    pub(in crate::mode_descendant_graphics::practical_results) direct_descriptor_render_call_offsets:
        [usize; 2],
    pub(in crate::mode_descendant_graphics::practical_results) delegated_result_callback_index:
        usize,
    pub(in crate::mode_descendant_graphics::practical_results) delegated_result_dispatch_offset:
        usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mode_descendant_graphics::practical_results) struct Siken20IndexedResultTextureLifetime
{
    pub(in crate::mode_descendant_graphics::practical_results) tim_upload_call_offsets: [usize; 4],
    pub(in crate::mode_descendant_graphics::practical_results) validated_siken20_member_count:
        usize,
    pub(in crate::mode_descendant_graphics::practical_results) validated_result_title_member_count:
        usize,
}

pub(in crate::mode_descendant_graphics::practical_results) fn validate_siken20_indexed_result_consumer_path(
    consumer: &[u8],
    occurrence_id: &str,
) -> Result<Siken20IndexedResultConsumerPath> {
    for (offset, expected_instruction) in expected_consumer_instructions() {
        let bytes = consumer.get(offset..offset + 4).with_context(|| {
            format!(
                "indexed-member texture consumer {occurrence_id} instruction at +0x{offset:04x} is truncated"
            )
        })?;
        let actual = decode(
            u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
            RUNTIME_BASE + offset as u32,
        )
        .with_context(|| {
            format!(
                "indexed-member texture consumer {occurrence_id} instruction at +0x{offset:04x} did not decode"
            )
        })?;
        ensure!(
            actual == expected_instruction,
            "indexed-member texture consumer {occurrence_id} instruction at +0x{offset:04x} changed: expected {expected_instruction:?}, found {actual:?}"
        );
    }
    Ok(Siken20IndexedResultConsumerPath {
        selected_member_count: 2,
        known_post_upload_descriptor_aliases: KNOWN_POST_UPLOAD_DESCRIPTOR_ALIASES,
        direct_descriptor_render_call_offsets: DIRECT_DESCRIPTOR_RENDER_CALL_OFFSETS,
        delegated_result_callback_index: DELEGATED_RESULT_CALLBACK_INDEX,
        delegated_result_dispatch_offset: DELEGATED_RESULT_DISPATCH_OFFSET,
    })
}

pub(in crate::mode_descendant_graphics::practical_results) fn validate_siken20_indexed_result_texture_lifetime(
    siken20: &[u8],
    sikenkk: &[u8],
) -> Result<Siken20IndexedResultTextureLifetime> {
    ensure!(
        siken20.len() == SIKEN20_MEMBER_SIZE * SIKEN20_MEMBER_COUNT,
        "SIKEN20 indexed-result member denominator changed"
    );
    ensure!(
        sikenkk.len() == SIKENKK_MEMBER_SIZE * SIKENKK_MEMBER_COUNT,
        "SIKENKK result-title member denominator changed"
    );

    for member_index in 0..SIKEN20_MEMBER_COUNT {
        let member_base = member_index * SIKEN20_MEMBER_SIZE;
        let member = &siken20[member_base..member_base + SIKEN20_MEMBER_SIZE];
        let atlas = parse_4bpp_prefix(member)
            .with_context(|| format!("SIKEN20 member {member_index} result atlas did not parse"))?;
        validate_4bpp_geometry(
            atlas,
            Tim4bppGeometry {
                total_size: 33_088,
                clut_x: 0,
                clut_y: 483,
                clut_width: 144,
                clut_height: 1,
                image_x: 896,
                image_y: 256,
                image_word_width: 64,
                image_height: 256,
            },
            &format!("SIKEN20 member {member_index} result atlas"),
        )?;

        let backdrop = parse_8bpp_prefix(&member[0x08800..]).with_context(|| {
            format!("SIKEN20 member {member_index} result backdrop did not parse")
        })?;
        validate_8bpp_geometry(
            backdrop,
            Tim8bppGeometry {
                total_size: 246_304,
                clut_x: 0,
                clut_y: 484,
                clut_width: 256,
                clut_height: 1,
                image_x: 512,
                image_y: 0,
                image_word_width: 256,
                image_height: 480,
            },
            &format!("SIKEN20 member {member_index} result backdrop"),
        )?;
        let pass_stamp = parse_4bpp_prefix(&member[0x45000..])
            .with_context(|| format!("SIKEN20 member {member_index} pass stamp did not parse"))?;
        validate_4bpp_geometry(
            pass_stamp,
            Tim4bppGeometry {
                total_size: 78_784,
                clut_x: 0,
                clut_y: 481,
                clut_width: 16,
                clut_height: 1,
                image_x: 768,
                image_y: 0,
                image_word_width: 82,
                image_height: 480,
            },
            &format!("SIKEN20 member {member_index} pass stamp"),
        )?;
        let fail_stamp = parse_4bpp_prefix(&member[0x58800..])
            .with_context(|| format!("SIKEN20 member {member_index} fail stamp did not parse"))?;
        validate_4bpp_geometry(
            fail_stamp,
            Tim4bppGeometry {
                total_size: 82_624,
                clut_x: 0,
                clut_y: 481,
                clut_width: 16,
                clut_height: 1,
                image_x: 768,
                image_y: 0,
                image_word_width: 86,
                image_height: 480,
            },
            &format!("SIKEN20 member {member_index} fail stamp"),
        )?;

        let retained = tim4bpp_rectangles(atlas);
        for (role, uploaded) in [
            ("result backdrop", tim8bpp_rectangles(backdrop)),
            ("pass stamp", tim4bpp_rectangles(pass_stamp)),
            ("fail stamp", tim4bpp_rectangles(fail_stamp)),
        ] {
            ensure_rectangles_do_not_overlap(
                retained,
                uploaded,
                &format!("SIKEN20 member {member_index} result atlas"),
                role,
            )?;
        }

        for title_index in 0..SIKENKK_MEMBER_COUNT {
            let title_base = title_index * SIKENKK_MEMBER_SIZE;
            let title = parse_4bpp_prefix(&sikenkk[title_base..title_base + SIKENKK_MEMBER_SIZE])
                .with_context(|| format!("SIKENKK member {title_index} did not parse"))?;
            validate_4bpp_geometry(
                title,
                Tim4bppGeometry {
                    total_size: SIKENKK_MEMBER_SIZE,
                    clut_x: 0,
                    clut_y: 486,
                    clut_width: 16,
                    clut_height: 1,
                    image_x: 896,
                    image_y: 0,
                    image_word_width: 31,
                    image_height: 28,
                },
                &format!("SIKENKK member {title_index}"),
            )?;
            ensure_rectangles_do_not_overlap(
                retained,
                tim4bpp_rectangles(title),
                &format!("SIKEN20 member {member_index} result atlas"),
                &format!("SIKENKK member {title_index}"),
            )?;
        }
    }

    Ok(Siken20IndexedResultTextureLifetime {
        tim_upload_call_offsets: RESULT_TIM_UPLOAD_CALL_OFFSETS,
        validated_siken20_member_count: SIKEN20_MEMBER_COUNT,
        validated_result_title_member_count: SIKENKK_MEMBER_COUNT,
    })
}

#[derive(Clone, Copy)]
struct Tim4bppGeometry {
    total_size: usize,
    clut_x: u16,
    clut_y: u16,
    clut_width: usize,
    clut_height: usize,
    image_x: u16,
    image_y: u16,
    image_word_width: usize,
    image_height: usize,
}

#[derive(Clone, Copy)]
struct Tim8bppGeometry {
    total_size: usize,
    clut_x: u16,
    clut_y: u16,
    clut_width: usize,
    clut_height: usize,
    image_x: u16,
    image_y: u16,
    image_word_width: usize,
    image_height: usize,
}

fn validate_4bpp_geometry(tim: Tim4bpp, expected: Tim4bppGeometry, role: &str) -> Result<()> {
    ensure!(
        tim.total_size == expected.total_size
            && tim.clut_x == expected.clut_x
            && tim.clut_y == expected.clut_y
            && tim.clut_width == expected.clut_width
            && tim.clut_height == expected.clut_height
            && tim.image_x == expected.image_x
            && tim.image_y == expected.image_y
            && tim.image_word_width == expected.image_word_width
            && tim.image_height == expected.image_height,
        "{role} TIM geometry changed"
    );
    Ok(())
}

fn validate_8bpp_geometry(tim: Tim8bpp, expected: Tim8bppGeometry, role: &str) -> Result<()> {
    ensure!(
        tim.total_size == expected.total_size
            && tim.clut_x == expected.clut_x
            && tim.clut_y == expected.clut_y
            && tim.clut_width == expected.clut_width
            && tim.clut_height == expected.clut_height
            && tim.image_x == expected.image_x
            && tim.image_y == expected.image_y
            && tim.image_word_width == expected.image_word_width
            && tim.image_height == expected.image_height,
        "{role} TIM geometry changed"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VramWordRectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

fn tim4bpp_rectangles(tim: Tim4bpp) -> [VramWordRectangle; 2] {
    [
        VramWordRectangle {
            x: usize::from(tim.image_x),
            y: usize::from(tim.image_y),
            width: tim.image_word_width,
            height: tim.image_height,
        },
        VramWordRectangle {
            x: usize::from(tim.clut_x),
            y: usize::from(tim.clut_y),
            width: tim.clut_width,
            height: tim.clut_height,
        },
    ]
}

fn tim8bpp_rectangles(tim: Tim8bpp) -> [VramWordRectangle; 2] {
    [
        VramWordRectangle {
            x: usize::from(tim.image_x),
            y: usize::from(tim.image_y),
            width: tim.image_word_width,
            height: tim.image_height,
        },
        VramWordRectangle {
            x: usize::from(tim.clut_x),
            y: usize::from(tim.clut_y),
            width: tim.clut_width,
            height: tim.clut_height,
        },
    ]
}

fn ensure_rectangles_do_not_overlap(
    retained: [VramWordRectangle; 2],
    uploaded: [VramWordRectangle; 2],
    retained_role: &str,
    uploaded_role: &str,
) -> Result<()> {
    ensure!(
        retained.iter().all(|left| uploaded
            .iter()
            .all(|right| !rectangles_overlap(*left, *right))),
        "{uploaded_role} overwrites {retained_role} VRAM image or CLUT"
    );
    Ok(())
}

fn rectangles_overlap(left: VramWordRectangle, right: VramWordRectangle) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

fn expected_consumer_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            0x3690,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x3694,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x3698,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + 0x36c0,
            },
        ),
        (
            0x369c,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 0x02b5,
            },
        ),
        (
            0x36a0,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (LOAD_BUFFER >> 16) as u16,
            },
        ),
        (
            0x36a4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: LOAD_BUFFER as u16,
            },
        ),
        (
            0x36b8,
            Instruction::J {
                target: RUNTIME_BASE + 0x36dc,
            },
        ),
        (
            0x36bc,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x36c0,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (LOAD_BUFFER >> 16) as u16,
            },
        ),
        (
            0x36c4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: LOAD_BUFFER as u16,
            },
        ),
        (
            0x36d8,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x36dc,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x36e4,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x36e8,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x6360,
            },
        ),
        (
            0x36ec,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (LOAD_BUFFER >> 16) as u16,
            },
        ),
        (
            0x36f0,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x0150,
            },
        ),
        (
            0x36f8,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x36fc,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: LOAD_BUFFER as u16,
            },
        ),
        (
            0x3448,
            Instruction::Lbu {
                rt: Register::A1,
                base: Register::A0,
                offset: 2,
            },
        ),
        (
            0x344c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x3450,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::A1,
                immediate: 0x00ff,
            },
        ),
        (
            0x3454,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + 0x382c,
            },
        ),
        (
            0x3458,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 2,
            },
        ),
        (
            0x345c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x3474,
            },
        ),
        (
            0x3464,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x3490,
            },
        ),
        (
            0x3468,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 1,
            },
        ),
        (
            0x3474,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x3478,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + 0x3868,
            },
        ),
        (
            0x347c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 3,
            },
        ),
        (
            0x3480,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + 0x3914,
            },
        ),
        (
            0x3490,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A0,
                offset: 2,
            },
        ),
        (
            0x3850,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x3854,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A0,
                offset: 2,
            },
        ),
        (
            0x38e4,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 3,
            },
        ),
        (
            0x38e8,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A0,
                offset: 2,
            },
        ),
        (
            0x3914,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3b50,
            },
        ),
        (
            0x3c34,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3ee0,
            },
        ),
        (
            0x3cdc,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3ee0,
            },
        ),
        (
            0x3d90,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3ee0,
            },
        ),
        (
            0x3db4,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3ee0,
            },
        ),
        (
            0x3ea8,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x3ee0,
            },
        ),
        (
            0x3eec,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x3f0c,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::S1,
                immediate: 0x002e,
            },
        ),
        (
            0x3f30,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 1,
            },
        ),
        (
            0x3f34,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x392c,
            },
        ),
        (
            0x3f3c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 2,
            },
        ),
        (
            0x3f40,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x3f0c,
            },
        ),
        (
            0x3f44,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 0x0037,
            },
        ),
        (
            0x3f68,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x392c,
            },
        ),
        (
            0x3708,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800b,
            },
        ),
        (
            0x3714,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x3718,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x2800,
            },
        ),
        (
            0x3730,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x3750,
            },
        ),
        (
            0x3740,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800e,
            },
        ),
        (
            0x3748,
            Instruction::J {
                target: RUNTIME_BASE + 0x3764,
            },
        ),
        (
            0x374c,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0xf000,
            },
        ),
        (
            0x3758,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x8010,
            },
        ),
        (
            0x3760,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x2800,
            },
        ),
        (
            0x3764,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x376c,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (LOAD_BUFFER >> 16) as u16,
            },
        ),
        (
            0x3770,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: LOAD_BUFFER as u16,
            },
        ),
        (
            0x3774,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 0x02b6,
            },
        ),
        (
            0x3788,
            Instruction::Lbu {
                rt: Register::A2,
                base: Register::V0,
                offset: 0x000c,
            },
        ),
        (
            0x378c,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V1,
                offset: 0x015c,
            },
        ),
        (
            0x3794,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x3798,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: 0x001e,
            },
        ),
        (
            0x37a4,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (LOAD_BUFFER >> 16) as u16,
            },
        ),
        (
            0x37a8,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x0150,
            },
        ),
        (
            0x37b0,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x37b4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: LOAD_BUFFER as u16,
            },
        ),
        (
            0x3904,
            Instruction::Jal {
                target: RUNTIME_BASE + 0x4218,
            },
        ),
        (
            0x4220,
            Instruction::Jal {
                target: RUNTIME_BASE + DELEGATED_RESULT_DISPATCH_OFFSET as u32,
            },
        ),
        (
            0x4224,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: DELEGATED_RESULT_CALLBACK_INDEX as i16,
            },
        ),
        (
            0x65d0,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x800b,
            },
        ),
        (
            0x65d4,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: -0x6858,
            },
        ),
        (
            0x65e0,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x000e,
            },
        ),
        (
            0x65e8,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x6608,
            },
        ),
        (
            0x65ec,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A0,
                shift: 2,
            },
        ),
        (
            0x65f0,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8018,
            },
        ),
        (
            0x65f4,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::V0,
                rt: Register::AT,
            },
        ),
        (
            0x65f8,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: -0x6000,
            },
        ),
        (
            0x65fc,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x8018,
            },
        ),
        (
            0x6600,
            Instruction::J {
                target: RUNTIME_BASE + 0x661c,
            },
        ),
        (
            0x6604,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x7000,
            },
        ),
        (
            0x6608,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8015,
            },
        ),
        (
            0x660c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::V0,
                rt: Register::AT,
            },
        ),
        (
            0x6610,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x2000,
            },
        ),
        (
            0x6614,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x8015,
            },
        ),
        (
            0x6618,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0xf000,
            },
        ),
        (
            0x661c,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use psx_r3000a::{Instruction, Register, encode};

    use super::{
        RUNTIME_BASE, expected_consumer_instructions, validate_siken20_indexed_result_consumer_path,
    };

    fn synthetic_consumer() -> Vec<u8> {
        let mut consumer = vec![0; 0x6620];
        for (offset, instruction) in expected_consumer_instructions() {
            consumer[offset..offset + 4].copy_from_slice(
                &encode(&instruction, RUNTIME_BASE + offset as u32)
                    .expect("fixture instruction should encode")
                    .to_le_bytes(),
            );
        }
        consumer
    }

    #[test]
    fn reports_the_bounded_indexed_result_consumer_path() {
        let validated =
            validate_siken20_indexed_result_consumer_path(&synthetic_consumer(), "test occurrence")
                .expect("bounded consumer path should validate");

        assert_eq!(validated.selected_member_count, 2);
        assert_eq!(validated.known_post_upload_descriptor_aliases, [46, 47, 55]);
        assert_eq!(
            validated.direct_descriptor_render_call_offsets,
            [0x3f34, 0x3f68]
        );
    }

    #[test]
    fn rejects_a_changed_post_upload_descriptor_selector() {
        let mut changed = synthetic_consumer();
        let offset = 0x3f44;
        let instruction = Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 0x0038,
        };
        changed[offset..offset + 4].copy_from_slice(
            &encode(&instruction, RUNTIME_BASE + offset as u32)
                .expect("fixture instruction should encode")
                .to_le_bytes(),
        );

        let error = validate_siken20_indexed_result_consumer_path(&changed, "test occurrence")
            .expect_err("changed selector must be rejected");
        assert!(error.to_string().contains("instruction at +0x3f44 changed"));
    }

    #[test]
    fn rejects_a_changed_delegated_result_callback_index() {
        let mut changed = synthetic_consumer();
        let offset = 0x4224;
        let instruction = Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 6,
        };
        changed[offset..offset + 4].copy_from_slice(
            &encode(&instruction, RUNTIME_BASE + offset as u32)
                .expect("fixture instruction should encode")
                .to_le_bytes(),
        );

        let error = validate_siken20_indexed_result_consumer_path(&changed, "test occurrence")
            .expect_err("changed delegated callback must be rejected");
        assert!(error.to_string().contains("instruction at +0x4224 changed"));
    }
}
