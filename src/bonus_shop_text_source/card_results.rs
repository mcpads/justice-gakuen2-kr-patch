use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::model::{BonusShopFontRole, BonusShopReleaseStatus};
use super::overlay_plan::{BonusShopTextOverlayPlan, glyph_command};
use crate::pipeline::difference_ranges;

// These count-prefixed records use relative pages, unlike direct selectors.
// KOUBAI 0x800a9ed4 draws both records for single-card and ten-card results.
// Five prefix packet slots are reserved even when the translated prefix is shorter.
const RECORDS: [(usize, &[u8]); 2] = [
    (0x3bac, &[5, 1, 5, 2, 0, 1, 7, 1, 1, 7, 0, 11, 1, 0, 2, 4]),
    (
        0x3bbc,
        &[6, 0, 0, 11, 2, 1, 0, 0, 1, 9, 2, 2, 0, 0, 9, 10, 0, 7, 8],
    ),
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BonusShopCardResultText {
    pub prefix: String,
    pub suffix: String,
    pub release_status: BonusShopReleaseStatus,
}

impl BonusShopCardResultText {
    pub(super) fn load(assets: &Path) -> Result<Self> {
        let path = assets.join("card-results.json");
        let text: Self = serde_json::from_slice(&std::fs::read(&path)?)
            .with_context(|| format!("invalid card result text: {}", path.display()))?;
        text.validate()?;
        Ok(text)
    }

    fn validate(&self) -> Result<()> {
        for (text, (_, source)) in [&self.prefix, &self.suffix].into_iter().zip(RECORDS) {
            ensure!(
                !text.is_empty() && text.chars().count() <= usize::from(source[0]),
                "card result text exceeds its native glyph/packet capacity"
            );
            ensure!(
                text.chars().all(|c| !c.is_whitespace() && !c.is_control()),
                "counted card result text cannot contain blank or control commands"
            );
        }
        Ok(())
    }

    pub(super) fn required_characters(&self) -> BTreeSet<(BonusShopFontRole, char)> {
        self.prefix
            .chars()
            .chain(self.suffix.chars())
            .map(|c| (BonusShopFontRole::ClerkDialogue, c))
            .collect()
    }

    pub(super) fn append_to_plan(
        &self,
        source: &[u8],
        plan: &mut BonusShopTextOverlayPlan,
        codes: &BTreeMap<BonusShopFontRole, BTreeMap<char, u16>>,
    ) -> Result<()> {
        self.validate()?;
        let codes = codes
            .get(&BonusShopFontRole::ClerkDialogue)
            .context("card result text has no shared clerk glyph allocation")?;
        for (text, (start, original)) in [&self.prefix, &self.suffix].into_iter().zip(RECORDS) {
            let end = start + original.len();
            ensure!(
                source.get(start..end) == Some(original),
                "KOUBAI card result source record changed"
            );
            ensure!(
                plan.expected_write_ranges
                    .iter()
                    .all(|r| end <= r[0] || r[1] <= start),
                "card result writer overlaps another shop text writer"
            );
            let mut encoded = vec![text.chars().count() as u8];
            for c in text.chars() {
                encoded.extend_from_slice(&glyph_command(
                    *codes
                        .get(&c)
                        .with_context(|| format!("card result glyph {c:?} was not allocated"))?,
                )?);
            }
            plan.bytes[start..end].fill(0);
            plan.bytes[start..start + encoded.len()].copy_from_slice(&encoded);
            plan.expected_write_ranges.push([start, end]);
        }
        plan.expected_write_ranges.sort_unstable();
        plan.changed_byte_ranges = difference_ranges(source, &plan.bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counted_results_use_shared_relative_pages_and_reject_unsafe_records() {
        let mut text = BonusShopCardResultText {
            prefix: "열혈카드".into(),
            suffix: "획득했습니다".into(),
            release_status: BonusShopReleaseStatus::NeedsHumanReview,
        };
        let mut source = vec![0x55; 0x3bd0];
        for (start, bytes) in RECORDS {
            source[start..start + bytes.len()].copy_from_slice(bytes);
        }
        let codes = BTreeMap::from([(
            BonusShopFontRole::ClerkDialogue,
            text.required_characters()
                .into_iter()
                .enumerate()
                .map(|(i, (_, c))| (c, 0x0200 + i as u16))
                .collect(),
        )]);
        let plan = || BonusShopTextOverlayPlan {
            bytes: source.clone(),
            authored_unit_count: 0,
            untranslated_unit_count: 0,
            expected_write_ranges: vec![],
            changed_byte_ranges: vec![],
            units: vec![],
        };
        let mut output = plan();
        text.append_to_plan(&source, &mut output, &codes).unwrap();
        for (expected, (start, original)) in [&text.prefix, &text.suffix].into_iter().zip(RECORDS) {
            let count = usize::from(output.bytes[start]);
            let decoded: String = output.bytes[start + 1..start + 1 + 3 * count]
                .as_chunks::<3>()
                .0
                .iter()
                .map(|s| {
                    let code = (u16::from(s[0]) << 8) | (u16::from(s[2]) << 4) | u16::from(s[1]);
                    *codes[&BonusShopFontRole::ClerkDialogue]
                        .iter()
                        .find(|(_, v)| **v == code)
                        .unwrap()
                        .0
                })
                .collect();
            assert_eq!(&decoded, expected);
            assert!(
                output.bytes[start + 1 + 3 * count..start + original.len()]
                    .iter()
                    .all(|b| *b == 0)
            );
        }
        assert_eq!(&output.bytes[..0x3bac], &source[..0x3bac]);
        assert_eq!(output.bytes[0x3bcf], source[0x3bcf]);
        let mut collision = plan();
        collision.expected_write_ranges.push([0x3bab, 0x3bad]);
        assert!(
            text.append_to_plan(&source, &mut collision, &codes)
                .is_err()
        );
        assert!(
            text.append_to_plan(&source, &mut plan(), &BTreeMap::new())
                .is_err()
        );
        let mut changed = source.clone();
        changed[0x3bac] = 4;
        assert!(text.append_to_plan(&changed, &mut plan(), &codes).is_err());
        text.prefix = "여섯글자초과".into();
        assert!(text.validate().is_err());
        text.prefix = "열혈 카드".into();
        assert!(text.validate().is_err());
    }
}
