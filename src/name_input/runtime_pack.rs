use anyhow::{Result, ensure};
use serde::Serialize;

use super::NameGlyphPackReport;

const HEADER_BYTES: usize = 24;
const MODERN_HANGUL_COUNT: usize = 11_172;
const BASE_KEY_COUNT: usize = 19 * 21;
const FINAL_KEY_COUNT: usize = 21 * 27;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputRuntimePackLayout {
    pub crop: [usize; 4],
    pub occupied_coordinate_count: usize,
    pub bytes_per_component_mask: usize,
    pub coordinate_membership_byte_range: [usize; 2],
    pub repertoire_membership_byte_range: [usize; 2],
    pub no_final_membership_byte_range: [usize; 2],
    pub final_bearing_membership_byte_range: [usize; 2],
    pub final_membership_byte_range: [usize; 2],
    pub no_final_rank_prefix_byte_range: [usize; 2],
    pub final_bearing_rank_prefix_byte_range: [usize; 2],
    pub final_rank_prefix_byte_range: [usize; 2],
    pub runtime_coordinate_list_byte_count: usize,
    pub component_mask_byte_range: [usize; 2],
    pub no_final_base_component_count: usize,
    pub final_bearing_base_component_count: usize,
    pub final_component_count: usize,
    pub pack_bytes: usize,
}

pub fn plan_name_input_runtime_pack(
    report: &NameGlyphPackReport,
) -> Result<NameInputRuntimePackLayout> {
    ensure!(
        report.supported_syllable_count + report.unsupported_modern_syllable_count
            == MODERN_HANGUL_COUNT,
        "name glyph pack runtime repertoire no longer spans modern Hangul"
    );
    ensure!(
        report.crop[0] + report.crop[2] <= 20 && report.crop[1] + report.crop[3] <= 20,
        "name glyph pack runtime crop exceeds one cache cell"
    );
    ensure!(
        report.bytes_per_component_mask == report.occupied_coordinate_count.div_ceil(8),
        "name glyph pack runtime component width changed"
    );
    ensure!(
        report.no_final_rank_prefix_entry_bytes == 2 && report.final_rank_prefix_entry_bytes == 1,
        "name glyph pack runtime rank-prefix width changed"
    );

    let coordinate_membership_byte_range = [
        HEADER_BYTES,
        HEADER_BYTES + (report.crop[2] * report.crop[3]).div_ceil(8),
    ];
    let repertoire_membership_byte_range = [
        coordinate_membership_byte_range[1],
        coordinate_membership_byte_range[1] + MODERN_HANGUL_COUNT.div_ceil(8),
    ];
    let no_final_membership_byte_range = [
        repertoire_membership_byte_range[1],
        repertoire_membership_byte_range[1] + BASE_KEY_COUNT.div_ceil(8),
    ];
    let final_bearing_membership_byte_range = [
        no_final_membership_byte_range[1],
        no_final_membership_byte_range[1] + BASE_KEY_COUNT.div_ceil(8),
    ];
    let final_membership_byte_range = [
        final_bearing_membership_byte_range[1],
        final_bearing_membership_byte_range[1] + FINAL_KEY_COUNT.div_ceil(8),
    ];
    ensure!(
        final_membership_byte_range[1] == report.no_final_rank_prefix_byte_range[0]
            && report.no_final_rank_prefix_byte_range[1]
                == report.final_bearing_rank_prefix_byte_range[0]
            && report.final_bearing_rank_prefix_byte_range[1]
                == report.final_rank_prefix_byte_range[0]
            && report.final_rank_prefix_byte_range[1] == report.component_mask_byte_range[0]
            && report.component_mask_byte_range[1] == report.pack_bytes,
        "name glyph pack runtime sections are not contiguous"
    );
    ensure!(
        report.runtime_coordinate_list_byte_count == report.occupied_coordinate_count,
        "name glyph runtime coordinate-list population changed"
    );
    ensure!(
        report.component_count
            == report.no_final_base_component_count
                + report.final_bearing_base_component_count
                + report.final_component_count
            && report.component_mask_byte_range[1] - report.component_mask_byte_range[0]
                == report.component_count * report.bytes_per_component_mask,
        "name glyph pack runtime component population changed"
    );

    Ok(NameInputRuntimePackLayout {
        crop: report.crop,
        occupied_coordinate_count: report.occupied_coordinate_count,
        bytes_per_component_mask: report.bytes_per_component_mask,
        coordinate_membership_byte_range,
        repertoire_membership_byte_range,
        no_final_membership_byte_range,
        final_bearing_membership_byte_range,
        final_membership_byte_range,
        no_final_rank_prefix_byte_range: report.no_final_rank_prefix_byte_range,
        final_bearing_rank_prefix_byte_range: report.final_bearing_rank_prefix_byte_range,
        final_rank_prefix_byte_range: report.final_rank_prefix_byte_range,
        runtime_coordinate_list_byte_count: report.runtime_coordinate_list_byte_count,
        component_mask_byte_range: report.component_mask_byte_range,
        no_final_base_component_count: report.no_final_base_component_count,
        final_bearing_base_component_count: report.final_bearing_base_component_count,
        final_component_count: report.final_component_count,
        pack_bytes: report.pack_bytes,
    })
}
