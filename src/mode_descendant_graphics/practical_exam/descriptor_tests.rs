use super::{
    OVERLAY_RUNTIME_BASE, PracticalExamConsumer, PracticalExamSecondaryDescriptorFragment,
    PracticalExamSecondaryTextureBank, SecondaryDescriptorLayout, parse_secondary_descriptor_arena,
    secondary_descriptor_census,
};
use crate::mode_descendant_graphics::model::PracticalExamSecondaryTextureBankAudit;
use crate::pipeline::sha256_bytes;

const POINTER_TABLE_OFFSET: usize = 0x60;

#[test]
fn aliases_share_one_physical_descriptor_without_losing_fragment_order() {
    let mut overlay = vec![0; 0x80];
    overlay[0x20..0x34].copy_from_slice(&[
        2, 0x0c, 0, 3, 4, 10, 20, 8, 12, 0x0d, 1, 5, 6, 30, 40, 16, 18, 0, 0, 0,
    ]);
    overlay[0x40..0x4c].copy_from_slice(&[1, 0x0e, 0, 0, 0, 50, 60, 20, 20, 0, 0, 0]);
    write_pointer(&mut overlay, 0, 0x20);
    write_pointer(&mut overlay, 1, 0x20);
    write_pointer(&mut overlay, 2, 0x40);

    let descriptors =
        parse_secondary_descriptor_arena(&overlay, PracticalExamConsumer::BasicsReview, layout(3))
            .unwrap();

    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[0].descriptor_index_aliases, [0, 1]);
    assert_eq!(descriptors[0].offset, 0x20);
    assert_eq!(descriptors[0].capacity, 0x14);
    assert_eq!(
        descriptors[0].source_sha256,
        sha256_bytes(&overlay[0x20..0x34])
    );
    assert_eq!(
        descriptors[0].fragments,
        [
            PracticalExamSecondaryDescriptorFragment {
                texture_bank: PracticalExamSecondaryTextureBank::SharedProducer,
                texture_page: 0x0c,
                clut_x_index: 3,
                clut_y_offset: 4,
                u: 10,
                v: 20,
                width: 8,
                height: 12,
            },
            PracticalExamSecondaryDescriptorFragment {
                texture_bank: PracticalExamSecondaryTextureBank::External,
                texture_page: 0x0d,
                clut_x_index: 5,
                clut_y_offset: 6,
                u: 30,
                v: 40,
                width: 16,
                height: 18,
            },
        ]
    );
    assert_eq!(descriptors[1].descriptor_index_aliases, [2]);
}

#[test]
fn census_preserves_every_alias_and_fragment_in_parser_order() {
    let mut overlay = vec![0; 0x80];
    overlay[0x20..0x34].copy_from_slice(&[
        2, 0x0c, 0, 3, 4, 10, 20, 8, 12, 0x0e, 1, 5, 6, 30, 40, 16, 18, 0, 0, 0,
    ]);
    overlay[0x40..0x4c].copy_from_slice(&[1, 0x0e, 1, 7, 8, 50, 60, 20, 20, 0, 0, 0]);
    write_pointer(&mut overlay, 0, 0x20);
    write_pointer(&mut overlay, 1, 0x20);
    write_pointer(&mut overlay, 2, 0x40);
    let descriptor_layout = layout(3);
    let descriptors = parse_secondary_descriptor_arena(
        &overlay,
        PracticalExamConsumer::BasicsReview,
        descriptor_layout,
    )
    .unwrap();

    let census = secondary_descriptor_census(
        PracticalExamConsumer::BasicsReview,
        "DAT1/SIKEN.BIN",
        "source-hash",
        descriptor_layout,
        &descriptors,
    )
    .unwrap();

    assert_eq!(census.pointer_table_entry_count, 3);
    assert_eq!(census.physical_descriptor_count, 2);
    assert!(census.alias_partition_complete);
    assert_eq!(census.descriptors[0].aliases, [0, 1]);
    assert_eq!(census.descriptors[0].fragments.len(), 2);
    assert_eq!(
        census.descriptors[0].fragments[1].source_cell,
        crate::tim::Cell {
            x: 30,
            y: 40,
            width: 16,
            height: 18,
        }
    );
    assert_eq!(census.descriptors[1].aliases, [2]);
    assert_eq!(
        census.descriptors[1].fragments[0].texture_bank,
        PracticalExamSecondaryTextureBankAudit::External
    );

    let mut descriptors_with_duplicate_alias = descriptors;
    descriptors_with_duplicate_alias[0]
        .descriptor_index_aliases
        .push(0);
    assert!(
        secondary_descriptor_census(
            PracticalExamConsumer::BasicsReview,
            "DAT1/SIKEN.BIN",
            "source-hash",
            descriptor_layout,
            &descriptors_with_duplicate_alias,
        )
        .is_err()
    );
}

#[test]
fn unique_physical_descriptors_must_not_overlap() {
    let mut overlay = vec![0; 0x80];
    overlay[0x20..0x2c].copy_from_slice(&[1, 0x0c, 0, 0, 0, 10, 20, 8, 12, 0, 0, 0]);
    overlay[0x28..0x34].copy_from_slice(&[1, 0x0d, 0, 0, 0, 30, 40, 16, 18, 0, 0, 0]);
    write_pointer(&mut overlay, 0, 0x20);
    write_pointer(&mut overlay, 1, 0x28);

    let error =
        parse_secondary_descriptor_arena(&overlay, PracticalExamConsumer::BasicsReview, layout(2))
            .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("overlaps the next physical descriptor")
    );
}

fn layout(descriptor_count: usize) -> SecondaryDescriptorLayout {
    SecondaryDescriptorLayout {
        pointer_table_offset: POINTER_TABLE_OFFSET,
        descriptor_count,
        pointer_table_sha256: "unused-by-arena-parser",
        descriptor_arena_start: 0x20,
        descriptor_arena_end: POINTER_TABLE_OFFSET,
        renderer_offset: 0,
        renderer_size: 0,
        renderer_sha256: "unused-by-arena-parser",
        lookup_offset: 0,
        lookup_size: 0,
        lookup_sha256: "unused-by-arena-parser",
    }
}

fn write_pointer(overlay: &mut [u8], index: usize, descriptor_offset: usize) {
    let pointer_offset = POINTER_TABLE_OFFSET + index * 4;
    let address = OVERLAY_RUNTIME_BASE + u32::try_from(descriptor_offset).unwrap();
    overlay[pointer_offset..pointer_offset + 4].copy_from_slice(&address.to_le_bytes());
}
