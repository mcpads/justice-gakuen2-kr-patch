use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_register_is_preserved_between,
    runtime_address,
};

#[derive(Clone, Copy)]
struct ZeroFillCallProfile {
    call_offset: usize,
    count_producer_offset: usize,
    byte_count: ZeroFillByteCount,
    fixed_destination: Option<FixedZeroFillDestination>,
}

#[derive(Clone, Copy)]
enum ZeroFillByteCount {
    Addiu(i16),
    Ori(u16),
}

#[derive(Clone, Copy)]
struct FixedZeroFillDestination {
    address_offset: usize,
    runtime_address: u32,
}

#[derive(Clone, Copy)]
struct ZeroFillWriterProfile {
    image_path: &'static str,
    store_offset: usize,
    calls: &'static [ZeroFillCallProfile],
}

const PROFILED_ZERO_FILL_WRITERS: &[ZeroFillWriterProfile] = &[
    ZeroFillWriterProfile {
        image_path: "DAT1/KANRI.BIN",
        store_offset: 0x8238,
        calls: &[
            fixed_zero_fill_call(0x17ac, 0x17b4, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x17bc, 0x17c4, 0x801a_2000, 4480),
            dynamic_zero_fill_call(0x194c, 0x1944, 256),
            dynamic_zero_fill_call(0x3a54, 0x3a58, 512),
            dynamic_zero_fill_call(0x43a8, 0x4394, 32),
            dynamic_zero_fill_call(0x4418, 0x4404, 40),
            dynamic_zero_fill_call(0x6a64, 0x6a68, 1024),
            dynamic_zero_fill_call(0x6a84, 0x6a88, 256),
            dynamic_zero_fill_call(0x6e34, 0x6e38, 16),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME.BIN",
        store_offset: 0x27ff0,
        calls: &[
            fixed_zero_fill_call(0x4a68, 0x4a70, 0x801f_1a00, 256),
            dynamic_zero_fill_call(0xd26c, 0xd264, 20),
            dynamic_zero_fill_call(0xe3b4, 0xe3b8, 20),
            dynamic_zero_fill_call(0xe400, 0xe404, 20),
            dynamic_zero_fill_call(0xe554, 0xe558, 20),
            dynamic_zero_fill_call(0xf620, 0xf624, 2048),
            dynamic_zero_fill_call(0x205ac, 0x205b0, 28),
            dynamic_zero_fill_call(0x205cc, 0x205d0, 3968),
            dynamic_zero_fill_call(0x20794, 0x20798, 512),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME01.BIN",
        store_offset: 0x6308,
        calls: &[
            fixed_zero_fill_call(0x0880, 0x088c, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x0894, 0x089c, 0x801f_6480, 156),
            dynamic_zero_fill_ori_call(0x0d40, 0x0d44, 32768),
            dynamic_zero_fill_call(0x0e14, 0x0e10, 8192),
            dynamic_zero_fill_call(0x0f94, 0x0f98, 20),
            dynamic_zero_fill_call(0x0fa4, 0x0fa8, 20),
            dynamic_zero_fill_call(0x2cfc, 0x2cf4, 20),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME02.BIN",
        store_offset: 0x6078,
        calls: &[
            fixed_zero_fill_call(0x06fc, 0x0708, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x0710, 0x0718, 0x801f_6480, 156),
            dynamic_zero_fill_call(0x19c4, 0x19c8, 100),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME03.BIN",
        store_offset: 0x5e28,
        calls: &[
            fixed_zero_fill_call(0x06d8, 0x06e4, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x06ec, 0x06f4, 0x801f_6480, 156),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME05.BIN",
        store_offset: 0x4ee4,
        calls: &[
            fixed_zero_fill_call(0x0890, 0x089c, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x08a4, 0x08ac, 0x801f_6480, 156),
            dynamic_zero_fill_call(0x0d90, 0x0d8c, 8192),
            dynamic_zero_fill_call(0x0ebc, 0x0ec0, 20),
            dynamic_zero_fill_call(0x0ecc, 0x0ed0, 20),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME06.BIN",
        store_offset: 0x756c,
        calls: &[
            fixed_zero_fill_call(0x08b0, 0x08bc, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x08c4, 0x08cc, 0x801f_6480, 156),
            dynamic_zero_fill_call(0x1304, 0x12fc, 8192),
            dynamic_zero_fill_call(0x34f8, 0x34fc, 40),
            dynamic_zero_fill_call(0x62d4, 0x62cc, 18),
        ],
    },
    ZeroFillWriterProfile {
        image_path: "DAT1/MGAME07.BIN",
        store_offset: 0x68f8,
        calls: &[
            fixed_zero_fill_call(0x07c8, 0x07d4, 0x801f_1a00, 256),
            fixed_zero_fill_call(0x07dc, 0x07e4, 0x801f_6480, 156),
            dynamic_zero_fill_call(0x0eb8, 0x0eb0, 8192),
            dynamic_zero_fill_call(0x0ec8, 0x0ecc, 30),
            dynamic_zero_fill_call(0x0ed8, 0x0edc, 30),
            dynamic_zero_fill_call(0x4e0c, 0x4e00, 20),
        ],
    },
];

const fn fixed_zero_fill_call(
    address_offset: usize,
    call_offset: usize,
    destination_runtime_address: u32,
    byte_count: i16,
) -> ZeroFillCallProfile {
    ZeroFillCallProfile {
        call_offset,
        count_producer_offset: call_offset + 4,
        byte_count: ZeroFillByteCount::Addiu(byte_count),
        fixed_destination: Some(FixedZeroFillDestination {
            address_offset,
            runtime_address: destination_runtime_address,
        }),
    }
}

const fn dynamic_zero_fill_call(
    call_offset: usize,
    count_producer_offset: usize,
    byte_count: i16,
) -> ZeroFillCallProfile {
    ZeroFillCallProfile {
        call_offset,
        count_producer_offset,
        byte_count: ZeroFillByteCount::Addiu(byte_count),
        fixed_destination: None,
    }
}

const fn dynamic_zero_fill_ori_call(
    call_offset: usize,
    count_producer_offset: usize,
    byte_count: u16,
) -> ZeroFillCallProfile {
    ZeroFillCallProfile {
        call_offset,
        count_producer_offset,
        byte_count: ZeroFillByteCount::Ori(byte_count),
        fixed_destination: None,
    }
}

pub(super) fn validate_profiled_zero_fill_writer(
    image: &LoadedImage,
    store_offset: usize,
) -> Result<()> {
    let profile = PROFILED_ZERO_FILL_WRITERS
        .iter()
        .find(|profile| profile.image_path == image.path && profile.store_offset == store_offset)
        .with_context(|| {
            format!(
                "{} has no source-bound zero-fill profile for +0x{store_offset:x}",
                image.path
            )
        })?;
    validate_zero_fill_loop(image, profile.store_offset)?;
    let function_offset = profile.store_offset - 0x18;
    let call_offsets = profile
        .calls
        .iter()
        .map(|call| call.call_offset)
        .collect::<Vec<_>>();
    ensure_direct_call_offsets(
        image,
        runtime_address(image, function_offset)?,
        &call_offsets,
        "source-bound zero-fill",
    )?;
    for call in profile.calls {
        let byte_count_is_positive = match call.byte_count {
            ZeroFillByteCount::Addiu(byte_count) => byte_count > 0,
            ZeroFillByteCount::Ori(byte_count) => byte_count > 0,
        };
        ensure!(
            byte_count_is_positive,
            "{} source-bound zero-fill has a non-positive byte count",
            image.path
        );
        if let Some(destination) = call.fixed_destination {
            ensure_instruction(
                image,
                destination.address_offset,
                Instruction::Lui {
                    rt: Register::A0,
                    immediate: (destination.runtime_address >> 16) as u16,
                },
                "zero-fill destination high half",
            )?;
            ensure_instruction(
                image,
                destination.address_offset + 4,
                Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: destination.runtime_address as u16,
                },
                "zero-fill destination low half",
            )?;
            ensure_register_is_preserved_between(
                image,
                destination.address_offset + 8,
                call.call_offset,
                Register::A0,
                "zero-fill destination argument",
            )?;
        }
        ensure_instruction(
            image,
            call.call_offset,
            Instruction::Jal {
                target: runtime_address(image, function_offset)?,
            },
            "zero-fill call",
        )?;
        let count_instruction = match call.byte_count {
            ZeroFillByteCount::Addiu(byte_count) => Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: byte_count,
            },
            ZeroFillByteCount::Ori(byte_count) => Instruction::Ori {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: byte_count,
            },
        };
        ensure_instruction(
            image,
            call.count_producer_offset,
            count_instruction,
            "zero-fill byte count",
        )?;
        if call.count_producer_offset < call.call_offset {
            ensure_register_is_preserved_between(
                image,
                call.count_producer_offset + 4,
                call.call_offset,
                Register::A1,
                "zero-fill byte-count argument",
            )?;
        } else {
            ensure!(
                call.count_producer_offset == call.call_offset + 4,
                "{} zero-fill byte count must be prepared before the call or in its delay slot",
                image.path
            );
        }
    }
    Ok(())
}

pub(super) fn validate_zero_fill_loop(image: &LoadedImage, store_offset: usize) -> Result<()> {
    let expected = [
        Instruction::Sb {
            rt: Register::ZERO,
            base: Register::A0,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: -1,
        },
        Instruction::Bgtz {
            rs: Register::A1,
            target: runtime_address(image, store_offset)?,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            store_offset + index * 4,
            instruction,
            "selector zero-fill",
        )?;
    }
    Ok(())
}
