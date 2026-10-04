//! Mirrors the finite result-page cells consumed from the wide practical texture producers.

use anyhow::{Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    Cell, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::changed_ranges_are_within;

use super::RESIDENT_RESULT_PROVIDER_CELLS;

const RESULT_ATLAS_TIM_OFFSET: usize = 0;
const PRACTICAL_TEXTURE_TIM_OFFSET: usize = 0x3c800;
const RESIDENT_RESULT_SOURCE_CELLS: [Cell; 4] = [
    Cell {
        x: 0,
        y: 0,
        width: 240,
        height: 20,
    },
    Cell {
        x: 0,
        y: 20,
        width: 40,
        height: 20,
    },
    Cell {
        x: 200,
        y: 40,
        width: 56,
        height: 32,
    },
    Cell {
        x: 200,
        y: 72,
        width: 56,
        height: 32,
    },
];
const RESIDENT_RESULT_PROVIDER_PREIMAGE_SHA256: [&str; 4] = [
    "810445a603a42e641c74b144fd98861bcf8fbdfe03a26d75d421a9aa95626fb0",
    "1f8edee5753f172840127179afc48297cf053cbd0afb8250fcb2c8545fd4454c",
    "5dcbf948c482f9b5943973415fd8fb5390f9887566427db78f8cc66a7f5c4959",
    "3fb9ab8a309fe9dcc44aa59fafd734101c6a0d8d14e586a1edcc01fc760e5a8f",
];

pub(super) struct ResidentPageMirrorBuild {
    pub(super) decoded: Vec<u8>,
    pub(super) claims: Vec<DecodedDataClaim>,
    pub(super) mirrored_cell_count: usize,
}

pub(super) fn mirror_bound_result_cells(
    owner: &str,
    immutable_result_atlas: &[u8],
    patched_result_atlas: &[u8],
    immutable_practical_texture: &[u8],
    practical_texture_base: &[u8],
) -> Result<ResidentPageMirrorBuild> {
    ensure!(
        immutable_result_atlas.len() == patched_result_atlas.len()
            && immutable_practical_texture.len() == practical_texture_base.len(),
        "resident result-page mirror changed a record extent"
    );
    ensure!(
        read_4bpp_palette_words_in_prefix(immutable_result_atlas, RESULT_ATLAS_TIM_OFFSET, 0)?
            == read_4bpp_palette_words_in_prefix(
                immutable_practical_texture,
                PRACTICAL_TEXTURE_TIM_OFFSET,
                0,
            )?,
        "{owner} resident result-page provider no longer shares the result palette"
    );

    let mut patched = practical_texture_base.to_vec();
    let mut allowed_ranges = Vec::new();
    for ((source_cell, target_cell), expected_preimage_sha256) in RESIDENT_RESULT_SOURCE_CELLS
        .into_iter()
        .zip(RESIDENT_RESULT_PROVIDER_CELLS)
        .zip(RESIDENT_RESULT_PROVIDER_PREIMAGE_SHA256)
    {
        let immutable_source = read_indexed_cell_in_prefix(
            immutable_result_atlas,
            RESULT_ATLAS_TIM_OFFSET,
            source_cell,
        )?;
        let immutable_target = read_indexed_cell_in_prefix(
            immutable_practical_texture,
            PRACTICAL_TEXTURE_TIM_OFFSET,
            target_cell,
        )?;
        ensure!(
            sha256_bytes(&immutable_target) == expected_preimage_sha256,
            "{owner} resident result-page provider {target_cell:?} preimage changed"
        );
        if source_cell.y < 40 {
            ensure!(
                immutable_source == immutable_target,
                "{owner} resident action source {source_cell:?} no longer aliases provider {target_cell:?}"
            );
        }
        ensure!(
            read_indexed_cell_in_prefix(
                practical_texture_base,
                PRACTICAL_TEXTURE_TIM_OFFSET,
                target_cell,
            )? == immutable_target,
            "{owner} resident result-page target {target_cell:?} overlaps an earlier compositor"
        );

        let replacement = read_indexed_cell_in_prefix(
            patched_result_atlas,
            RESULT_ATLAS_TIM_OFFSET,
            source_cell,
        )?;
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched,
            PRACTICAL_TEXTURE_TIM_OFFSET,
            target_cell,
            &replacement,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "{owner} resident result-page mirror changed no pixels"
        );
        allowed_ranges.extend(write.allowed_ranges);
    }

    ensure!(
        changed_ranges_are_within(
            &difference_ranges(practical_texture_base, &patched),
            &allowed_ranges,
        ),
        "{owner} resident result-page mirror escaped its finite cells"
    );
    let claims = DecodedDataClaim::from_effective_ranges(
        &format!("mode-descendant:practical-result:{owner}:resident-page"),
        "mirror statically bound result cells into the main-overlay texture provider",
        practical_texture_base,
        &patched,
        allowed_ranges,
    )?;
    ensure!(
        !claims.is_empty(),
        "{owner} resident result-page mirror produced no Expected Writes"
    );
    Ok(ResidentPageMirrorBuild {
        decoded: patched,
        claims,
        mirrored_cell_count: RESIDENT_RESULT_PROVIDER_CELLS.len(),
    })
}
