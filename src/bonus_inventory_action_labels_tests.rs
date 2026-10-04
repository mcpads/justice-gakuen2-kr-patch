use psx_r3000a::{Instruction, encode};

use super::model::{ActionLabelUnit, ActionSequenceBinding, DevelopmentStatus, ReleaseStatus};
use super::overlay::{
    ACTION_SEQUENCES, OVERLAY_RUNTIME_BASE, POINTER_TABLE_OFFSET, allocated_physical_alias_codes,
    patch_action_label_overlay, source_consumer_instructions, validate_allocated_glyph_ownership,
};

#[test]
fn action_sequences_keep_source_pointers_and_use_blank_commands_for_shorter_korean_labels() {
    let source = overlay_fixture();
    let units = fixture_units();

    let patched = patch_action_label_overlay(&source, &units).unwrap();

    assert_eq!(
        &patched.bytes[0x07e8..0x07fb],
        &[
            3, 0, 2, // 카
            3, 1, 2, // 드
            0x63, 0x63, 0x63, // blank
            3, 2, 2, // 보
            3, 3, 2, // 기
            0x63, 0x63, 0x63, // source-capacity padding
            0x81,
        ]
    );
    assert_eq!(
        &patched.bytes[0x07fc..0x0812],
        &[
            3, 0, 2, // 카
            3, 1, 2, // 드
            0x63, 0x63, 0x63, // blank
            3, 4, 2, // 주
            3, 3, 2, // 기
            0x63, 0x63, 0x63, // source-capacity padding
            0x63, 0x63, 0x63, // source-capacity padding
            0x81,
        ]
    );
    assert_eq!(
        read_u32(&patched.bytes, 0x133c),
        OVERLAY_RUNTIME_BASE + 0x07e8
    );
    assert_eq!(
        read_u32(&patched.bytes, 0x1340),
        OVERLAY_RUNTIME_BASE + 0x07fc
    );
    assert_eq!(
        patched.expected_write_ranges,
        [[0x07e8, 0x07fb], [0x07fc, 0x0812]]
    );
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        (0x07e8 <= *start && *end <= 0x07fb) || (0x07fc <= *start && *end <= 0x0812)
    }));
}

#[test]
fn action_overlay_composition_preserves_the_disjoint_page_indicator_instruction() {
    let mut source = overlay_fixture();
    source[0x86dc..0x86e0].copy_from_slice(&[0x5a, 0xa5, 0x5a, 0xa5]);

    let patched = patch_action_label_overlay(&source, &fixture_units()).unwrap();

    assert_eq!(&patched.bytes[0x86dc..0x86e0], &[0x5a, 0xa5, 0x5a, 0xa5]);
}

#[test]
fn rejects_a_source_command_whose_wrapped_cell_aliases_the_action_allocation() {
    let mut source = overlay_fixture();
    source[0x0100..0x0104].copy_from_slice(&[3, 12, 2, 0x81]);

    let error = validate_allocated_glyph_ownership(&source).unwrap_err();

    assert!(error.to_string().contains("aliases a source command glyph"));
}

#[test]
fn rejects_a_direct_sprite_whose_wrapped_cell_aliases_the_action_allocation() {
    let mut source = overlay_fixture();
    source[0x2000..0x2003].copy_from_slice(&[11, 0, 2]);

    let error = validate_allocated_glyph_ownership(&source).unwrap_err();

    assert!(error.to_string().contains("direct-sprite glyph"));
}

#[test]
fn rejects_a_changed_runtime_observed_action_caller() {
    let mut source = overlay_fixture();
    write_instruction(&mut source, 0x7600, Instruction::nop());

    let error = patch_action_label_overlay(&source, &fixture_units()).unwrap_err();

    assert!(error.to_string().contains("consumer changed at +0x7600"));
}

#[test]
fn action_glyph_cells_do_not_claim_the_unbound_stock_label_or_page_suffix() {
    let aliases = allocated_physical_alias_codes();

    assert!((0x0320..=0x0324).all(|code| aliases.contains(&code)));
    assert!(!aliases.contains(&0x0325));
    assert!(!aliases.contains(&0x017a));
}

fn fixture_units() -> Vec<ActionLabelUnit> {
    vec![
        ActionLabelUnit {
            kind: "fixture".to_string(),
            id: "view_card".to_string(),
            source_text: "カードをみる".to_string(),
            korean_text: Some("카드 보기".to_string()),
            source: ActionSequenceBinding {
                sequence_offset: "0x07e8".to_string(),
                terminator_offset: "0x07fa".to_string(),
                pointer_storage_offset: "0x133c".to_string(),
                encoded_sha256: "38e2d39041bb76150118e03948a608eeba9a9dc9ee70d6991efb7b4192d10aad"
                    .to_string(),
            },
            development_status: DevelopmentStatus::Authored,
            release_status: ReleaseStatus::NeedsHumanReview,
        },
        ActionLabelUnit {
            kind: "fixture".to_string(),
            id: "give_card".to_string(),
            source_text: "カードをあげる".to_string(),
            korean_text: Some("카드 주기".to_string()),
            source: ActionSequenceBinding {
                sequence_offset: "0x07fc".to_string(),
                terminator_offset: "0x0811".to_string(),
                pointer_storage_offset: "0x1340".to_string(),
                encoded_sha256: "60cf36b3d90394f1d1b5fa2b325999de54903b79ad6f8b8123d3254128eacfdc"
                    .to_string(),
            },
            development_status: DevelopmentStatus::Authored,
            release_status: ReleaseStatus::NeedsHumanReview,
        },
    ]
}

fn overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 0x9900];
    overlay[0x0100] = 0x81;
    for pointer_offset in (POINTER_TABLE_OFFSET..0x1418).step_by(4) {
        write_u32(&mut overlay, pointer_offset, OVERLAY_RUNTIME_BASE + 0x0100);
    }
    for spec in ACTION_SEQUENCES {
        overlay[spec.sequence_offset..=spec.terminator_offset].copy_from_slice(spec.source_bytes);
        write_u32(
            &mut overlay,
            spec.pointer_storage_offset,
            OVERLAY_RUNTIME_BASE + spec.sequence_offset as u32,
        );
    }
    write_u32(&mut overlay, 0x1344, OVERLAY_RUNTIME_BASE + 0x0814);
    overlay[0x0814] = 0x81;
    for (offset, instruction) in source_consumer_instructions() {
        write_instruction(&mut overlay, offset, instruction);
    }
    overlay
}

fn write_instruction(overlay: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    overlay[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn write_u32(overlay: &mut [u8], offset: usize, value: u32) {
    overlay[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(overlay: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(overlay[offset..offset + 4].try_into().unwrap())
}
