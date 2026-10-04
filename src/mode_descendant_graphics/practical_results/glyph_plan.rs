//! Derives Korean glyph demand without assigning any JP source cell as output.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::super::model::ModeDescendantFontRole;
use super::model::{
    PracticalResultDevelopmentStatus, PracticalResultEntry, PracticalResultStrategy,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PracticalResultGlyphKey {
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) glyph: char,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KoreanSequenceToken {
    Glyph(PracticalResultGlyphKey),
    Advance,
}

pub(super) struct KoreanSequenceDemand {
    pub(super) entry_id: String,
    pub(super) tokens: Vec<KoreanSequenceToken>,
}

pub(super) struct PracticalResultGlyphDemandPlan {
    pub(super) glyphs: BTreeSet<PracticalResultGlyphKey>,
    pub(super) sequences: Vec<KoreanSequenceDemand>,
}

impl PracticalResultGlyphDemandPlan {
    pub(super) fn sequence_token_count(&self) -> usize {
        self.sequences
            .iter()
            .map(|sequence| sequence.tokens.len())
            .sum()
    }

    fn validate(&self) -> Result<()> {
        let mut entry_ids = BTreeSet::new();
        for sequence in &self.sequences {
            ensure!(
                !sequence.entry_id.is_empty()
                    && !sequence.tokens.is_empty()
                    && entry_ids.insert(sequence.entry_id.as_str()),
                "practical-result Korean sequence demand is duplicated or empty"
            );
            for token in &sequence.tokens {
                if let KoreanSequenceToken::Glyph(key) = token {
                    ensure!(
                        self.glyphs.contains(key) && !key.glyph.is_whitespace(),
                        "practical-result Korean sequence references an unplanned glyph"
                    );
                }
            }
        }
        Ok(())
    }
}

pub(super) fn derive_glyph_demand(
    entries: &[PracticalResultEntry],
) -> Result<PracticalResultGlyphDemandPlan> {
    let mut glyphs = BTreeSet::new();
    let mut sequences = Vec::new();
    for entry in entries.iter().filter(|entry| {
        entry.strategy == PracticalResultStrategy::GlyphSequence
            && entry.development_status == PracticalResultDevelopmentStatus::Authored
    }) {
        let korean_text = entry
            .korean_text
            .as_deref()
            .context("authored practical-result glyph sequence lost Korean text")?;
        let tokens = korean_text
            .chars()
            .map(|glyph| {
                if glyph.is_whitespace() {
                    KoreanSequenceToken::Advance
                } else {
                    let key = PracticalResultGlyphKey {
                        font_role: entry.font_role,
                        glyph,
                    };
                    glyphs.insert(key);
                    KoreanSequenceToken::Glyph(key)
                }
            })
            .collect::<Vec<_>>();
        ensure!(
            !tokens.is_empty(),
            "practical-result Korean glyph sequence {} is empty",
            entry.id
        );
        sequences.push(KoreanSequenceDemand {
            entry_id: entry.id.clone(),
            tokens,
        });
    }
    let plan = PracticalResultGlyphDemandPlan { glyphs, sequences };
    plan.validate()?;
    Ok(plan)
}
