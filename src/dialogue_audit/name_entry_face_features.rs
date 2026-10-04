use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::tim::Cell;

use super::name_entry::{OVERLAY_RUNTIME_BASE, bounded_slice, decode_instruction};
use super::name_entry_fixed_graphics_model::{
    DialogueNameEntryFixedGraphicBuildReport, NameEntryFixedGraphicSurfaceSpec,
};

const DESCRIPTOR_OFFSET: usize = 0x0118;
// Native rows are screen x/y, texture page, u/v, width, palette. Height is 20.
// The last two rows share this consumer but are owned by candidate-controls.
const DESCRIPTORS: [[u16; 7]; 5] = [
    [74, 166, 12, 220, 160, 20, 12],
    [74, 196, 12, 180, 180, 20, 12],
    [74, 226, 12, 200, 180, 20, 12],
    [74, 276, 14, 144, 224, 48, 17],
    [74, 316, 14, 40, 100, 160, 8],
];
const LABEL_IDS: [&str; 3] = ["eyes", "nose", "mouth"];

pub(super) fn face_feature_labels_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "face-feature-labels",
        translation_file: "face-feature-labels.json",
        translation_kind: "Justice Gakuen 2 source-bound face-feature label translation",
        build_kind: "Justice Gakuen 2 face-feature label build",
        target: "face-feature Korean labels",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}

pub(super) fn validate_face_feature_consumer(overlay: &[u8]) -> Result<()> {
    for (index, expected) in DESCRIPTORS.iter().flatten().enumerate() {
        let value = u16::from_le_bytes(
            bounded_slice(overlay, DESCRIPTOR_OFFSET + index * 2, 2)?.try_into()?,
        );
        ensure!(
            value == *expected,
            "face-feature descriptor changed at word {index}"
        );
    }
    for (offset, expected) in producer_instructions() {
        ensure!(
            decode_instruction(overlay, offset)? == expected,
            "face-feature renderer changed at {offset:#x}"
        );
    }
    Ok(())
}

pub(super) fn validate_face_feature_cells(
    report: &DialogueNameEntryFixedGraphicBuildReport,
) -> Result<()> {
    ensure!(
        report.entries.len() == LABEL_IDS.len(),
        "face-feature labels are incomplete"
    );
    for (id, row) in LABEL_IDS.into_iter().zip(DESCRIPTORS) {
        let cell = Cell {
            x: row[3] as usize,
            y: row[4] as usize,
            width: row[5] as usize,
            height: 20,
        };
        ensure!(
            report
                .entries
                .iter()
                .any(|entry| entry.id == id && entry.cell == cell),
            "face-feature label {id} differs from its consumed cell"
        );
    }
    Ok(())
}

fn producer_instructions() -> Vec<(usize, Instruction)> {
    use Instruction::*;
    let mut instructions = vec![
        (
            0x5b74,
            Lui {
                rt: Register::FP,
                immediate: 0x8018,
            },
        ),
        (
            0x5b78,
            Addiu {
                rt: Register::FP,
                rs: Register::FP,
                immediate: -0x5ee8,
            },
        ),
        (
            0x5c20,
            Sll {
                rd: Register::A2,
                rt: Register::A2,
                shift: 6,
            },
        ),
        (
            0x5c6c,
            Sll {
                rd: Register::A0,
                rt: Register::S2,
                shift: 4,
            },
        ),
        (
            0x5c7c,
            Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 483,
            },
        ),
        (
            0x5c94,
            Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 20,
            },
        ),
        (
            0x5c98,
            Sb {
                rt: Register::S5,
                base: Register::S0,
                offset: 20,
            },
        ),
        (
            0x5c9c,
            Sb {
                rt: Register::S4,
                base: Register::S0,
                offset: 21,
            },
        ),
        (
            0x5ca0,
            Sh {
                rt: Register::S3,
                base: Register::S0,
                offset: 24,
            },
        ),
        (
            0x5ca4,
            Sh {
                rt: Register::V0,
                base: Register::S0,
                offset: 26,
            },
        ),
        (
            0x5d00,
            Slti {
                rt: Register::V0,
                rs: Register::S7,
                immediate: 5,
            },
        ),
        (
            0x5d04,
            Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: OVERLAY_RUNTIME_BASE + 0x5ba0,
            },
        ),
    ];
    for (offset, rt) in [
        (0x5ba0, Register::T0),
        (0x5bc4, Register::T0),
        (0x5bec, Register::A2),
        (0x5bf4, Register::S5),
        (0x5bfc, Register::S4),
        (0x5c04, Register::S3),
        (0x5c0c, Register::S2),
    ] {
        instructions.push((
            offset,
            Lh {
                rt,
                base: Register::FP,
                offset: 0,
            },
        ));
        instructions.push((
            offset + 4,
            Addiu {
                rt: Register::FP,
                rs: Register::FP,
                immediate: 2,
            },
        ));
    }
    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use psx_r3000a::encode;

    fn source_consumer() -> Vec<u8> {
        let mut overlay = vec![0; 0x9000];
        for (index, word) in DESCRIPTORS.iter().flatten().enumerate() {
            let offset = DESCRIPTOR_OFFSET + index * 2;
            overlay[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
        }
        for (offset, instruction) in producer_instructions() {
            overlay[offset..offset + 4].copy_from_slice(
                &encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32)
                    .unwrap()
                    .to_le_bytes(),
            );
        }
        overlay
    }

    #[test]
    fn face_feature_consumer_rejects_a_changed_texture_cell() {
        let mut source = source_consumer();
        validate_face_feature_consumer(&source).unwrap();
        source[DESCRIPTOR_OFFSET + 6] += 1;
        assert!(
            validate_face_feature_consumer(&source)
                .unwrap_err()
                .to_string()
                .contains("descriptor")
        );
    }

    #[test]
    fn face_feature_consumer_rejects_a_shortened_draw_loop() {
        let mut source = source_consumer();
        source[0x5d00..0x5d04].copy_from_slice(
            &encode(
                &Instruction::Slti {
                    rt: Register::V0,
                    rs: Register::S7,
                    immediate: 2,
                },
                OVERLAY_RUNTIME_BASE + 0x5d00,
            )
            .unwrap()
            .to_le_bytes(),
        );
        assert!(
            validate_face_feature_consumer(&source)
                .unwrap_err()
                .to_string()
                .contains("renderer")
        );
    }
}
