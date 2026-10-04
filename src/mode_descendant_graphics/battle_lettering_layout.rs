//! Keeps native battle lettering descriptors aligned with selected texture artwork.
use anyhow::{Result, ensure};

use super::model::ModeDescendantGraphicsBuild;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::pipeline::difference_ranges;

const DESCRIPTORS: [(usize, [i16; 15]); 2] = [
    (
        0x7d880,
        [
            16, 0, 0, 832, 256, 480, 501, 176, 0, 79, 23, 216, 200, 79, 20,
        ],
    ),
    (
        0x7d8dc,
        [
            16, 0, 0, 832, 256, 480, 501, 176, 24, 79, 47, 216, 200, 79, 48,
        ],
    ),
];

fn layout_candidate(source: &[u8]) -> Result<Vec<u8>> {
    let mut candidate = source.to_vec();
    for (index, (offset, words)) in DESCRIPTORS.iter().enumerate() {
        let original: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        ensure!(
            source.get(*offset..*offset + 30) == Some(original.as_slice()),
            "native round numeral descriptor changed"
        );
        // 0x80058e8c reads texture extents through 0x80050600, then centers
        // the display quad from the final two halfwords. Keep native position,
        // animation flags, palette, selection table and timing unchanged.
        for (word, value) in [
            (7, 176 + index as u16 * 40),
            (8, 0),
            (9, 39),
            (10, 63),
            (13, 39),
            (14, 64),
        ] {
            candidate[offset + word * 2..offset + word * 2 + 2]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(candidate)
}

pub(crate) fn register_battle_lettering_layout(
    source: &[u8],
    build: &ModeDescendantGraphicsBuild,
    plan: &mut DecodedRecordWritePlan<'_>,
) -> Result<()> {
    let Some(family) = &build.report.battle_announcements else {
        return Ok(());
    };
    if family
        .artworks
        .iter()
        .any(|art| art.vertical_name_id.is_some())
    {
        let ids: Vec<_> = family
            .artworks
            .iter()
            .filter_map(|art| art.vertical_name_id)
            .collect();
        if let Some(candidate) = super::vertical_names::layout_candidate(source, &ids)? {
            plan.register_data_candidate(
                "vertical HUD name layout",
                crate::source_disc::MAIN_EXECUTABLE_SHA256,
                &candidate,
                &DecodedDataClaim::from_ranges(
                    "vertical-hud-name-layout",
                    "give three-syllable Nagare the taller owned name cell",
                    difference_ranges(source, &candidate),
                ),
            )?;
        }
    }
    let finish: Vec<_> = family
        .artworks
        .iter()
        .filter(|a| a.special_finish_trimmed)
        .collect();
    if !finish.is_empty() {
        ensure!(
            finish.len() == 1 && finish[0].source_records == ["DAT2/CEFT1.BIZ"],
            "special finish layout requires its composed texture"
        );
        let candidate = special_finish_candidate(source)?;
        plan.register_data_candidate(
            "special finish texture extent",
            crate::source_disc::MAIN_EXECUTABLE_SHA256,
            &candidate,
            &DecodedDataClaim::from_ranges(
                "special-finish-extent",
                "exclude the adjacent attack banner from the finishing subtitle",
                difference_ranges(source, &candidate),
            ),
        )?;
    }
    let selected: Vec<_> = family
        .artworks
        .iter()
        .filter(|a| a.round_numerals_repacked)
        .collect();
    if selected.is_empty() {
        return Ok(());
    }
    ensure!(
        selected.len() == 1 && selected[0].source_records == ["DAT2/CEFT1.BIZ", "DAT2/CEFT3.BIZ"],
        "round layout requires both repacked texture consumers"
    );
    let candidate = layout_candidate(source)?;
    plan.register_data_candidate(
        "battle round numerals",
        crate::source_disc::MAIN_EXECUTABLE_SHA256,
        &candidate,
        &DecodedDataClaim::from_ranges(
            "battle-round-numerals",
            "sample both repacked Arabic numerals at their authored size",
            difference_ranges(source, &candidate),
        ),
    )
}

fn special_finish_candidate(source: &[u8]) -> Result<Vec<u8>> {
    const START: usize = 0x7e3bc;
    const WORDS: [u16; 14] = [1, 896, 256, 64, 502, 64, 144, 87, 16, 0, 104, 127, 16, 0];
    let original: Vec<u8> = WORDS.iter().flat_map(|w| w.to_le_bytes()).collect();
    ensure!(
        source.get(START..START + 28) == Some(original.as_slice()),
        "native special finish descriptor changed"
    );
    let mut candidate = source.to_vec();
    // The original 17th texture row belongs to the next banner. Keep the
    // native 16-pixel display height and sample only the owned 16 rows.
    candidate[START + 16..START + 18].copy_from_slice(&15u16.to_le_bytes());
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finish_excludes_the_next_banner_without_moving_its_display() {
        let mut source = vec![0xa5; 0x7e400];
        let words: [u16; 14] = [1, 896, 256, 64, 502, 64, 144, 87, 16, 0, 104, 127, 16, 0];
        for (i, w) in words.iter().enumerate() {
            source[0x7e3bc + i * 2..0x7e3bc + i * 2 + 2].copy_from_slice(&w.to_le_bytes());
        }
        let candidate = special_finish_candidate(&source).unwrap();
        assert_eq!(
            difference_ranges(&source, &candidate),
            vec![[0x7e3cc, 0x7e3cd]]
        );
        let last_v = u16::from_le_bytes(candidate[0x7e3c8..0x7e3ca].try_into().unwrap())
            + u16::from_le_bytes(candidate[0x7e3cc..0x7e3ce].try_into().unwrap());
        assert_eq!(last_v, 159);
        source[0x7e3c2] ^= 1;
        assert!(special_finish_candidate(&source).is_err());
    }
    fn source() -> Vec<u8> {
        let mut source = vec![0xa5; 0x7d900];
        for (offset, words) in DESCRIPTORS {
            for (i, w) in words.iter().enumerate() {
                source[offset + i * 2..offset + i * 2 + 2].copy_from_slice(&w.to_le_bytes());
            }
        }
        source
    }
    #[test]
    fn repacked_digits_have_separate_uvs_and_equal_display_height() {
        let source = source();
        let candidate = layout_candidate(&source).unwrap();
        for (i, (offset, _)) in DESCRIPTORS.iter().enumerate() {
            let word = |n: usize| {
                u16::from_le_bytes(
                    candidate[offset + n * 2..offset + n * 2 + 2]
                        .try_into()
                        .unwrap(),
                )
            };
            assert_eq!(
                (word(7), word(8), word(9) + 1, word(10) + 1),
                (176 + i as u16 * 40, 0, 40, 64)
            );
            // Match the native signed half-extent rounding, not just stored bytes.
            assert_eq!(
                (word(13).div_ceil(2) * 2, word(14).div_ceil(2) * 2),
                (40, 64)
            );
        }
        for (offset, (&before, &after)) in source.iter().zip(&candidate).enumerate() {
            if before != after {
                assert!(DESCRIPTORS.iter().any(|(start, _)| {
                    [7, 8, 9, 10, 13, 14]
                        .iter()
                        .any(|word| (*start + word * 2..*start + word * 2 + 2).contains(&offset))
                }));
            }
        }
    }
    #[test]
    fn changed_native_palette_or_geometry_is_rejected() {
        for word in [5, 7, 14] {
            let mut source = source();
            source[0x7d880 + word * 2] ^= 1;
            assert!(layout_candidate(&source).is_err());
        }
    }
}
