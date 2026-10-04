//! Exact runtime mappings for loaded executable images on the supported disc.
//!
//! These are target facts, not filename-discovery rules. The finite exception
//! set stays explicit so a newly encountered file cannot inherit a runtime
//! address merely because its name resembles a known image.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::pipeline::sha256_bytes;

use super::{
    MAIN_EXECUTABLE_PATH, MAIN_EXECUTABLE_SHA256, MAIN_TEXT_RUNTIME_BASE,
    main_executable_entrypoint, main_executable_text,
};

#[derive(Debug)]
pub(crate) struct LoadedImage {
    pub(crate) path: String,
    pub(crate) data: Vec<u8>,
    pub(crate) runtime_base: Option<u32>,
    pub(crate) entrypoints: Vec<LoadedImageEntrypoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoadedImageEntrypoint {
    pub(crate) role: &'static str,
    pub(crate) source_reference_kind: &'static str,
    pub(crate) source_reference_offset: usize,
    pub(crate) runtime_address: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LoadedImageProfile {
    pub(crate) path: &'static str,
    pub(crate) runtime_base: Option<u32>,
    additional_entrypoints: &'static [AdditionalEntrypoint],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdditionalEntrypoint {
    role: &'static str,
    instruction_offset: usize,
    source: AdditionalEntrypointSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdditionalEntrypointSource {
    LoadedImageHeaderPointer {
        pointer_offset: usize,
    },
    IndexedTaskDispatchTablePointer {
        pointer_offset: usize,
        dispatch: IndexedTaskDispatch,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IndexedTaskDispatch {
    dispatcher_call_offset: usize,
    dispatcher_offset: usize,
    table_base_offset: usize,
    selector: u8,
    selector_stores: &'static [TaskSelectorStore],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TaskSelectorStore {
    literal_offset: usize,
    store_offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MainDispatchedEntrypoint {
    role: &'static str,
    call_instruction_offset: usize,
    instruction_offset: usize,
}

// This finite analysis-root profile is intentionally explicit. Do not derive it
// from address or filename patterns, and do not make product construction read
// an analysis report. Every entry is revalidated against the exact source JAL.
const MAIN_DISPATCHED_ENTRYPOINTS: [MainDispatchedEntrypoint; 14] = [
    main_dispatched_entrypoint("mode_select_solo_setup", 0x7080, 0x71ac),
    main_dispatched_entrypoint("mode_select_versus_setup", 0x7090, 0x7324),
    main_dispatched_entrypoint("mode_select_practical_exam_99_setup", 0x70a0, 0x7564),
    main_dispatched_entrypoint("mode_select_options_setup", 0x70b0, 0xcd68),
    main_dispatched_entrypoint("mode_select_records_setup", 0x70c0, 0xcdac),
    main_dispatched_entrypoint("mode_select_bonus_setup", 0x70d0, 0xec24),
    main_dispatched_entrypoint("mode_select_diary_setup", 0x70e0, 0x7798),
    main_dispatched_entrypoint("mode_select_cooperative_setup", 0x70f0, 0x7824),
    main_dispatched_entrypoint("mode_select_team_setup", 0x7100, 0x7958),
    main_dispatched_entrypoint("mode_select_tournament_setup", 0x7110, 0x7a94),
    main_dispatched_entrypoint("mode_select_league_setup", 0x7120, 0x7b7c),
    main_dispatched_entrypoint("mode_select_gorin_festival_setup", 0x7130, 0x7c4c),
    main_dispatched_entrypoint("mode_select_edit_registration_setup", 0x7140, 0x8070),
    main_dispatched_entrypoint("mode_select_training_setup", 0x7150, 0x7420),
];

const COMMON_OVERLAY_BASE: Option<u32> = Some(0x800a_2000);

const fn image(path: &'static str, runtime_base: Option<u32>) -> LoadedImageProfile {
    LoadedImageProfile {
        path,
        runtime_base,
        additional_entrypoints: &[],
    }
}

const fn image_with_entrypoints(
    path: &'static str,
    runtime_base: u32,
    additional_entrypoints: &'static [AdditionalEntrypoint],
) -> LoadedImageProfile {
    LoadedImageProfile {
        path,
        runtime_base: Some(runtime_base),
        additional_entrypoints,
    }
}

pub(crate) const DAT1_LOADED_IMAGE_PROFILES: [LoadedImageProfile; 61] = [
    image("DAT1/CDEMO.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/EM_ETBL.BIN", None),
    image("DAT1/END00.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END01.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END02.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END03.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END04.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END05.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END06.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END07.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END08.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END09.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END10.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END11.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END12.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END13.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END14.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END15.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END16.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END18.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END19.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END20.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END21.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END22.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END23.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/END24.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/KANRI.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/KOUBAI.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/KOUBAI2.BIN", COMMON_OVERLAY_BASE),
    image_with_entrypoints(
        "DAT1/MGAME.BIN",
        0x800a_2000,
        &[AdditionalEntrypoint {
            role: "task_dispatch_index_15",
            instruction_offset: 0x16a48,
            source: AdditionalEntrypointSource::IndexedTaskDispatchTablePointer {
                pointer_offset: 0x1048,
                dispatch: IndexedTaskDispatch {
                    dispatcher_call_offset: 0x481c,
                    dispatcher_offset: 0xf508,
                    table_base_offset: 0x100c,
                    selector: 15,
                    selector_stores: &[
                        TaskSelectorStore {
                            literal_offset: 0x1767c,
                            store_offset: 0x17684,
                        },
                        TaskSelectorStore {
                            literal_offset: 0x17704,
                            store_offset: 0x1770c,
                        },
                    ],
                },
            },
        }],
    ),
    image("DAT1/MGAME01.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGAME02.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGAME03.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGAME05.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGAME06.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGAME061.BIN", Some(0x8015_0000)),
    image("DAT1/MGAME07.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGENT.BIN", Some(0x8017_a000)),
    image("DAT1/MGFIN.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/MGTIT.BIN", COMMON_OVERLAY_BASE),
    image_with_entrypoints(
        "DAT1/MINISEL.BIN",
        0x800a_2000,
        &[AdditionalEntrypoint {
            role: "secondary_header_entrypoint",
            instruction_offset: 0x1554,
            source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                pointer_offset: 0x04,
            },
        }],
    ),
    image("DAT1/MODESEL.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/NEWOPT.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/NIKKI.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/ODEMO.BIN", COMMON_OVERLAY_BASE),
    image_with_entrypoints(
        "DAT1/PASS.BIN",
        0x8017_a000,
        &[
            AdditionalEntrypoint {
                role: "password_display_entrypoint",
                instruction_offset: 0x1ad0,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x04,
                },
            },
            AdditionalEntrypoint {
                role: "cpu_edit_entrypoint",
                instruction_offset: 0x5840,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x08,
                },
            },
        ],
    ),
    image("DAT1/PLSEL1.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/PLSEL2.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/PLSEL3.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/PLSEL4.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/PLSEL5.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/RKDEMO.BIN", COMMON_OVERLAY_BASE),
    image_with_entrypoints(
        "DAT1/SIKEN.BIN",
        0x800a_2000,
        &[AdditionalEntrypoint {
            role: "secondary_header_entrypoint",
            instruction_offset: 0x0f8c,
            source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                pointer_offset: 0x04,
            },
        }],
    ),
    image_with_entrypoints(
        "DAT1/SIKEN2.BIN",
        0x800a_2000,
        &[AdditionalEntrypoint {
            role: "secondary_header_entrypoint",
            instruction_offset: 0x0c3c,
            source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                pointer_offset: 0x04,
            },
        }],
    ),
    image_with_entrypoints(
        "DAT1/SIKENG21.BIN",
        0x8015_2000,
        &[
            AdditionalEntrypoint {
                role: "outcome_renderer",
                instruction_offset: 0x1e9c,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x04,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_08",
                instruction_offset: 0x3364,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x08,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_0c",
                instruction_offset: 0x37c0,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x0c,
                },
            },
            AdditionalEntrypoint {
                role: "static_tile_stream_callback",
                instruction_offset: 0x3d64,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x18,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_1c",
                instruction_offset: 0x3f98,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x1c,
                },
            },
        ],
    ),
    image_with_entrypoints(
        "DAT1/SIKENG22.BIN",
        0x8017_a000,
        &[
            AdditionalEntrypoint {
                role: "outcome_renderer",
                instruction_offset: 0x1e9c,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x04,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_08",
                instruction_offset: 0x3364,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x08,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_0c",
                instruction_offset: 0x37c0,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x0c,
                },
            },
            AdditionalEntrypoint {
                role: "static_tile_stream_callback",
                instruction_offset: 0x3d64,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x18,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_1c",
                instruction_offset: 0x3f98,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x1c,
                },
            },
        ],
    ),
    image_with_entrypoints(
        "DAT1/SIKENGO1.BIN",
        0x8015_2000,
        &[
            AdditionalEntrypoint {
                role: "outcome_renderer",
                instruction_offset: 0x1fcc,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x04,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_08",
                instruction_offset: 0x36c4,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x08,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_0c",
                instruction_offset: 0x3b20,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x0c,
                },
            },
            AdditionalEntrypoint {
                role: "static_tile_stream_callback",
                instruction_offset: 0x474c,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x18,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_1c",
                instruction_offset: 0x4980,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x1c,
                },
            },
        ],
    ),
    image_with_entrypoints(
        "DAT1/SIKENGO2.BIN",
        0x8017_a000,
        &[
            AdditionalEntrypoint {
                role: "outcome_renderer",
                instruction_offset: 0x1fcc,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x04,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_08",
                instruction_offset: 0x36c4,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x08,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_0c",
                instruction_offset: 0x3b20,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x0c,
                },
            },
            AdditionalEntrypoint {
                role: "static_tile_stream_callback",
                instruction_offset: 0x474c,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x18,
                },
            },
            AdditionalEntrypoint {
                role: "external_header_callback_1c",
                instruction_offset: 0x4980,
                source: AdditionalEntrypointSource::LoadedImageHeaderPointer {
                    pointer_offset: 0x1c,
                },
            },
        ],
    ),
    image("DAT1/SKHAY.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/STAFF.BIN", COMMON_OVERLAY_BASE),
    image("DAT1/TRAIN.BIN", COMMON_OVERLAY_BASE),
];

pub(crate) fn dat1_runtime_base(path: &str) -> Result<Option<u32>> {
    DAT1_LOADED_IMAGE_PROFILES
        .iter()
        .find(|profile| profile.path == path)
        .map(|profile| profile.runtime_base)
        .with_context(|| format!("unsupported DAT1 loaded image {path}"))
}

pub(crate) fn load_dat1_images(image_path: &Path) -> Result<Vec<LoadedImage>> {
    let mut track = RawTrack::open(image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    read_dat1_images(&mut iso)
}

pub(crate) fn load_main_executable_image(image_path: &Path) -> Result<LoadedImage> {
    let mut track = RawTrack::open(image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let executable_record = iso.find(MAIN_EXECUTABLE_PATH)?;
    let executable = iso.read_record(&executable_record)?;
    loaded_main_executable(&executable)
}

pub(crate) fn loaded_main_executable(executable: &[u8]) -> Result<LoadedImage> {
    let executable_sha256 = sha256_bytes(executable);
    ensure!(
        executable_sha256 == MAIN_EXECUTABLE_SHA256,
        "unsupported {MAIN_EXECUTABLE_PATH} SHA-256: {executable_sha256}"
    );
    let entrypoint = main_executable_entrypoint(executable)?;
    let text = main_executable_text(executable)?;
    let mut entrypoints = vec![LoadedImageEntrypoint {
        role: "ps_x_exe_entrypoint",
        source_reference_kind: "ps_x_exe_header_pointer",
        source_reference_offset: 0x10,
        runtime_address: entrypoint,
    }];
    entrypoints.extend(validated_main_dispatched_entrypoints(
        text,
        &MAIN_DISPATCHED_ENTRYPOINTS,
    )?);
    Ok(LoadedImage {
        path: MAIN_EXECUTABLE_PATH.to_string(),
        data: text.to_vec(),
        runtime_base: Some(MAIN_TEXT_RUNTIME_BASE),
        entrypoints,
    })
}

fn validated_main_dispatched_entrypoints(
    text: &[u8],
    profiles: &[MainDispatchedEntrypoint],
) -> Result<Vec<LoadedImageEntrypoint>> {
    let mut entrypoints = Vec::with_capacity(profiles.len());
    for additional in profiles {
        let call_address = MAIN_TEXT_RUNTIME_BASE
            .checked_add(u32::try_from(additional.call_instruction_offset)?)
            .context("main dispatched entrypoint call address overflow")?;
        let target = MAIN_TEXT_RUNTIME_BASE
            .checked_add(u32::try_from(additional.instruction_offset)?)
            .context("main dispatched entrypoint target address overflow")?;
        let call = text
            .get(additional.call_instruction_offset..additional.call_instruction_offset + 4)
            .with_context(|| {
                format!(
                    "main dispatched entrypoint {} call leaves the executable text",
                    additional.role
                )
            })?;
        ensure!(
            decode(u32::from_le_bytes(call.try_into()?), call_address)?
                == (Instruction::Jal { target }),
            "main dispatched entrypoint {} call changed",
            additional.role
        );
        ensure!(
            additional
                .instruction_offset
                .checked_add(4)
                .is_some_and(|end| end <= text.len()),
            "main dispatched entrypoint {} leaves the executable text",
            additional.role
        );
        entrypoints.push(LoadedImageEntrypoint {
            role: additional.role,
            source_reference_kind: "direct_dispatch_call",
            source_reference_offset: additional.call_instruction_offset,
            runtime_address: target,
        });
    }
    Ok(entrypoints)
}

const fn main_dispatched_entrypoint(
    role: &'static str,
    call_instruction_offset: usize,
    instruction_offset: usize,
) -> MainDispatchedEntrypoint {
    MainDispatchedEntrypoint {
        role,
        call_instruction_offset,
        instruction_offset,
    }
}

pub(crate) fn read_dat1_images(iso: &mut Iso9660<'_>) -> Result<Vec<LoadedImage>> {
    let mut images = Vec::with_capacity(DAT1_LOADED_IMAGE_PROFILES.len());
    for profile in DAT1_LOADED_IMAGE_PROFILES {
        let record = iso.find(profile.path)?;
        let data = iso.read_record(&record)?;
        let entrypoints = loaded_image_entrypoints(&profile, &data)?;
        images.push(LoadedImage {
            path: profile.path.to_string(),
            data,
            runtime_base: profile.runtime_base,
            entrypoints,
        });
    }
    Ok(images)
}

fn loaded_image_entrypoints(
    profile: &LoadedImageProfile,
    data: &[u8],
) -> Result<Vec<LoadedImageEntrypoint>> {
    let Some(runtime_base) = profile.runtime_base else {
        return Ok(Vec::new());
    };
    let primary = little_u32(data, 0)
        .with_context(|| format!("{} primary entrypoint is truncated", profile.path))?;
    let mut entrypoints = vec![LoadedImageEntrypoint {
        role: "primary_header_entrypoint",
        source_reference_kind: "loaded_image_header_pointer",
        source_reference_offset: 0,
        runtime_address: primary,
    }];
    let mut seen = std::collections::BTreeSet::from([primary]);
    for additional in profile.additional_entrypoints {
        let (source_reference_kind, pointer_offset, indexed_task_dispatch) = match additional.source
        {
            AdditionalEntrypointSource::LoadedImageHeaderPointer { pointer_offset } => {
                ("loaded_image_header_pointer", pointer_offset, None)
            }
            AdditionalEntrypointSource::IndexedTaskDispatchTablePointer {
                pointer_offset,
                dispatch,
            } => (
                "indexed_task_dispatch_table_pointer",
                pointer_offset,
                Some(dispatch),
            ),
        };
        let expected = runtime_base
            .checked_add(u32::try_from(additional.instruction_offset)?)
            .context("loaded-image additional entrypoint address overflow")?;
        let declared = little_u32(data, pointer_offset).with_context(|| {
            format!(
                "{} additional entrypoint pointer +0x{:x} is truncated",
                profile.path, pointer_offset
            )
        })?;
        ensure!(
            declared == expected,
            "{} additional entrypoint pointer +0x{:x} changed",
            profile.path,
            pointer_offset
        );
        ensure!(
            additional
                .instruction_offset
                .checked_add(4)
                .is_some_and(|end| end <= data.len()),
            "{} additional entrypoint +0x{:x} is outside the loaded image",
            profile.path,
            additional.instruction_offset
        );
        if let Some(dispatch) = indexed_task_dispatch {
            validate_indexed_task_dispatch(
                profile,
                data,
                additional.role,
                pointer_offset,
                dispatch,
            )?;
        }
        ensure!(
            seen.insert(expected),
            "{} declares duplicate entrypoint 0x{expected:08x}",
            profile.path
        );
        entrypoints.push(LoadedImageEntrypoint {
            role: additional.role,
            source_reference_kind,
            source_reference_offset: pointer_offset,
            runtime_address: expected,
        });
    }
    Ok(entrypoints)
}

fn validate_indexed_task_dispatch(
    profile: &LoadedImageProfile,
    data: &[u8],
    entrypoint_role: &str,
    pointer_offset: usize,
    dispatch: IndexedTaskDispatch,
) -> Result<()> {
    let runtime_base = profile
        .runtime_base
        .context("indexed task dispatch requires a runtime base")?;
    let expected_pointer_offset = dispatch
        .table_base_offset
        .checked_add(usize::from(dispatch.selector) * 4)
        .context("indexed task dispatch table slot overflow")?;
    ensure!(
        pointer_offset == expected_pointer_offset,
        "{} indexed task entrypoint {} does not occupy selector {} in its declared table",
        profile.path,
        entrypoint_role,
        dispatch.selector
    );

    let dispatcher_address = runtime_base
        .checked_add(u32::try_from(dispatch.dispatcher_offset)?)
        .context("indexed task dispatcher address overflow")?;
    ensure_loaded_image_instruction(
        profile.path,
        data,
        runtime_base,
        dispatch.dispatcher_call_offset,
        Instruction::Jal {
            target: dispatcher_address,
        },
        "indexed task dispatcher caller",
    )?;

    let table_address = runtime_base
        .checked_add(u32::try_from(dispatch.table_base_offset)?)
        .context("indexed task dispatch table address overflow")?;
    let table_high = u16::try_from((table_address.wrapping_add(0x8000) >> 16) & 0xffff)?;
    let table_low = table_address as u16 as i16;
    for (relative_offset, expected) in [
        (
            0x1c,
            Instruction::Lui {
                rt: Register::S1,
                immediate: table_high,
            },
        ),
        (
            0x20,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: table_low,
            },
        ),
        (
            0x34,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: 2,
            },
        ),
        (
            0x3c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x40,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::S1,
            },
        ),
        (
            0x44,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x4c,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
    ] {
        let instruction_offset = dispatch
            .dispatcher_offset
            .checked_add(relative_offset)
            .context("indexed task dispatcher instruction offset overflow")?;
        ensure_loaded_image_instruction(
            profile.path,
            data,
            runtime_base,
            instruction_offset,
            expected,
            "indexed task dispatcher",
        )?;
    }

    ensure!(
        !dispatch.selector_stores.is_empty(),
        "{} indexed task entrypoint {} has no selector source",
        profile.path,
        entrypoint_role
    );
    for selector_store in dispatch.selector_stores {
        ensure_loaded_image_instruction(
            profile.path,
            data,
            runtime_base,
            selector_store.literal_offset,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: i16::from(dispatch.selector),
            },
            "indexed task selector literal",
        )?;
        let preserved_start = selector_store
            .literal_offset
            .checked_add(4)
            .context("indexed task selector preservation start overflow")?;
        for offset in (preserved_start..selector_store.store_offset).step_by(4) {
            let instruction = decode_loaded_image_instruction(data, runtime_base, offset)?;
            ensure!(
                instruction.written_gpr() != Some(Register::V0),
                "{} indexed task selector register changes at +0x{offset:x}",
                profile.path
            );
        }
        ensure_loaded_image_instruction(
            profile.path,
            data,
            runtime_base,
            selector_store.store_offset,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::V1,
                offset: 2,
            },
            "indexed task selector store",
        )?;
    }
    Ok(())
}

fn ensure_loaded_image_instruction(
    image_path: &str,
    data: &[u8],
    runtime_base: u32,
    instruction_offset: usize,
    expected: Instruction,
    role: &str,
) -> Result<()> {
    let actual = decode_loaded_image_instruction(data, runtime_base, instruction_offset)
        .with_context(|| format!("{image_path} {role} +0x{instruction_offset:x} is invalid"))?;
    ensure!(
        actual == expected,
        "{image_path} {role} +0x{instruction_offset:x} changed: expected {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn decode_loaded_image_instruction(
    data: &[u8],
    runtime_base: u32,
    instruction_offset: usize,
) -> Result<Instruction> {
    let instruction_address = runtime_base
        .checked_add(u32::try_from(instruction_offset)?)
        .context("loaded-image instruction address overflow")?;
    let encoded =
        little_u32(data, instruction_offset).context("loaded-image instruction is truncated")?;
    decode(encoded, instruction_address).context("loaded-image instruction failed to decode")
}

fn little_u32(data: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).context("u32 offset overflow")?;
    let bytes = data.get(offset..end).context("u32 is truncated")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

#[cfg(test)]
#[path = "loaded_image_tests.rs"]
mod tests;
