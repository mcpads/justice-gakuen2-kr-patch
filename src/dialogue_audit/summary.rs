use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::atlas::{
    GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH, ParsedDialogueAtlas,
};
use super::format::hex_code;
use super::model::{
    DialogueFontAudit, DialogueGlyphCodeAudit, DialogueGlyphVariantAudit, DialogueTokenCodeAudit,
};
use super::runtime_insertions::dialogue_control_spec;
use super::tokens::{DialogueTokenKind, ParsedDialogueToken};

#[derive(Debug, Default)]
struct GlyphUsage {
    occurrence_count: usize,
    assets: BTreeSet<String>,
}

#[derive(Debug)]
struct TokenCodeAccumulation {
    kind: DialogueTokenKind,
    argument_word_count: usize,
    occurrence_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SharedAtlasStructure {
    clut_vram_x: u16,
    clut_vram_y: u16,
    clut_color_count: usize,
    image_vram_x: u16,
    image_vram_y: u16,
    pixel_data_offset: usize,
    addressable_slot_count: usize,
    source_extension_end: usize,
}

#[derive(Debug, Default)]
pub(super) struct DialogueSummary {
    token_count: usize,
    fixed_glyph_occurrence_count: usize,
    runtime_extension_glyph_occurrence_count: usize,
    alignment_padding_word_count: usize,
    glyph_usage: BTreeMap<u16, GlyphUsage>,
    token_codes: BTreeMap<u16, TokenCodeAccumulation>,
    shared_atlas_structure: Option<SharedAtlasStructure>,
    fixed_cell_variants: Vec<BTreeMap<String, BTreeSet<String>>>,
    fixed_source_assets: Vec<BTreeSet<String>>,
    runtime_extension_source_assets: Vec<BTreeSet<String>>,
    used_fixed_asset_code_pairs: BTreeSet<(String, u16)>,
    used_runtime_extension_asset_code_pairs: BTreeSet<(String, u16)>,
    fixed_asset_code_pair_count: usize,
    minimum_fixed_cell_count: usize,
    maximum_fixed_cell_count: usize,
    source_extension_zero_in_every_asset: bool,
}

pub(super) struct FinalizedDialogueSummary {
    pub(super) token_count: usize,
    pub(super) glyph_occurrence_count: usize,
    pub(super) fixed_glyph_occurrence_count: usize,
    pub(super) runtime_extension_glyph_occurrence_count: usize,
    pub(super) alignment_padding_word_count: usize,
    pub(super) font: DialogueFontAudit,
    pub(super) token_codes: Vec<DialogueTokenCodeAudit>,
}

impl DialogueSummary {
    pub(super) fn new() -> Self {
        Self {
            minimum_fixed_cell_count: usize::MAX,
            source_extension_zero_in_every_asset: true,
            ..Self::default()
        }
    }

    pub(super) fn record_atlas(
        &mut self,
        source_path: &str,
        atlas: &ParsedDialogueAtlas,
    ) -> Result<()> {
        let structure = SharedAtlasStructure::from(atlas);
        if let Some(expected) = self.shared_atlas_structure {
            ensure!(
                structure == expected,
                "{source_path} dialogue atlas shared structure differs from the first MGK asset: expected {expected:?}, got {structure:?}",
            );
        } else {
            self.shared_atlas_structure = Some(structure);
            self.fixed_cell_variants
                .resize_with(atlas.addressable_slot_count, BTreeMap::new);
            self.fixed_source_assets
                .resize_with(atlas.addressable_slot_count, BTreeSet::new);
            self.runtime_extension_source_assets
                .resize_with(atlas.addressable_slot_count, BTreeSet::new);
        }
        ensure!(
            self.fixed_cell_variants.len() == atlas.addressable_slot_count,
            "{source_path} dialogue atlas addressable slot count differs from the shared structure"
        );

        self.fixed_asset_code_pair_count += atlas.fixed_cell_count;
        self.minimum_fixed_cell_count = self.minimum_fixed_cell_count.min(atlas.fixed_cell_count);
        self.maximum_fixed_cell_count = self.maximum_fixed_cell_count.max(atlas.fixed_cell_count);
        for code in 0..atlas.addressable_slot_count {
            if let Some(cell_sha256) = atlas.fixed_cell_sha256.get(code) {
                self.fixed_cell_variants[code]
                    .entry(cell_sha256.clone())
                    .or_default()
                    .insert(source_path.to_string());
                self.fixed_source_assets[code].insert(source_path.to_string());
            } else {
                self.runtime_extension_source_assets[code].insert(source_path.to_string());
            }
        }
        self.source_extension_zero_in_every_asset &= atlas.source_extension_nonzero_byte_count == 0;
        Ok(())
    }

    pub(super) fn record_message(
        &mut self,
        source_path: &str,
        tokens: &[ParsedDialogueToken],
        alignment_padding_word_count: usize,
    ) -> Result<()> {
        self.alignment_padding_word_count += alignment_padding_word_count;
        self.token_count += tokens.len();
        for token in tokens {
            match token.kind {
                DialogueTokenKind::FixedGlyph => {
                    self.fixed_glyph_occurrence_count += 1;
                    self.used_fixed_asset_code_pairs
                        .insert((source_path.to_string(), token.code));
                    self.record_glyph(source_path, token.code);
                }
                DialogueTokenKind::RuntimeExtensionGlyph => {
                    self.runtime_extension_glyph_occurrence_count += 1;
                    self.used_runtime_extension_asset_code_pairs
                        .insert((source_path.to_string(), token.code));
                    self.record_glyph(source_path, token.code);
                }
                _ => {
                    let accumulation =
                        self.token_codes
                            .entry(token.code)
                            .or_insert(TokenCodeAccumulation {
                                kind: token.kind,
                                argument_word_count: token.arguments.len(),
                                occurrence_count: 0,
                            });
                    ensure!(
                        accumulation.kind == token.kind
                            && accumulation.argument_word_count == token.arguments.len(),
                        "dialogue token 0x{:04x} has inconsistent parsing",
                        token.code
                    );
                    accumulation.occurrence_count += 1;
                }
            }
        }
        Ok(())
    }

    fn record_glyph(&mut self, source_path: &str, code: u16) {
        let usage = self.glyph_usage.entry(code).or_default();
        usage.occurrence_count += 1;
        usage.assets.insert(source_path.to_string());
    }

    pub(super) fn finish(self, asset_count: usize) -> Result<FinalizedDialogueSummary> {
        let structure = self
            .shared_atlas_structure
            .context("dialogue asset population has no font atlas")?;
        let mut common_fixed_cells_identical_across_assets = 0usize;
        let mut common_fixed_cells_with_asset_variants = 0usize;
        let mut shared_identical_prefix_cell_count = 0usize;
        let mut prefix_is_shared = true;
        let mut all_source_pixel_hashes = BTreeSet::new();
        let mut used_source_pixel_hashes = BTreeSet::new();
        let mut glyph_codes = Vec::with_capacity(structure.addressable_slot_count);

        for (code, variants) in self.fixed_cell_variants.into_iter().enumerate() {
            all_source_pixel_hashes.extend(variants.keys().cloned());
            if code < self.minimum_fixed_cell_count {
                if variants.len() == 1 {
                    common_fixed_cells_identical_across_assets += 1;
                } else {
                    common_fixed_cells_with_asset_variants += 1;
                }
            }
            let usage = self.glyph_usage.get(&(code as u16));
            prefix_is_shared &=
                self.fixed_source_assets[code].len() == asset_count && variants.len() == 1;
            if prefix_is_shared {
                shared_identical_prefix_cell_count += 1;
            }
            if let Some(usage) = usage {
                for (cell_sha256, variant_assets) in &variants {
                    if variant_assets
                        .iter()
                        .any(|asset| usage.assets.contains(asset))
                    {
                        used_source_pixel_hashes.insert(cell_sha256.clone());
                    }
                }
            }
            glyph_codes.push(DialogueGlyphCodeAudit {
                code: hex_code(code as u16),
                occurrence_count: usage.map_or(0, |usage| usage.occurrence_count),
                message_assets: usage
                    .map(|usage| usage.assets.iter().cloned().collect())
                    .unwrap_or_default(),
                fixed_source_assets: self.fixed_source_assets[code].iter().cloned().collect(),
                runtime_extension_source_assets: self.runtime_extension_source_assets[code]
                    .iter()
                    .cloned()
                    .collect(),
                source_cell_variants: variants
                    .into_iter()
                    .map(|(cell_sha256, assets)| DialogueGlyphVariantAudit {
                        cell_sha256,
                        assets: assets.into_iter().collect(),
                    })
                    .collect(),
            });
        }
        ensure!(
            shared_identical_prefix_cell_count > 0,
            "dialogue atlases have no shared identical glyph prefix"
        );

        let font = DialogueFontAudit {
            bits_per_pixel: 4,
            cell_width: GLYPH_CELL_WIDTH,
            cell_height: GLYPH_CELL_HEIGHT,
            bytes_per_cell: GLYPH_CELL_BYTE_COUNT,
            minimum_fixed_cell_count: self.minimum_fixed_cell_count,
            maximum_fixed_cell_count: self.maximum_fixed_cell_count,
            addressable_slot_count_before_selector: structure.addressable_slot_count,
            minimum_runtime_extension_slot_count: structure.addressable_slot_count
                - self.maximum_fixed_cell_count,
            maximum_runtime_extension_slot_count: structure.addressable_slot_count
                - self.minimum_fixed_cell_count,
            used_glyph_code_count: self.glyph_usage.len(),
            used_fixed_asset_code_pair_count: self.used_fixed_asset_code_pairs.len(),
            unreferenced_fixed_asset_code_pair_count: self.fixed_asset_code_pair_count
                - self.used_fixed_asset_code_pairs.len(),
            used_runtime_extension_asset_code_pair_count: self
                .used_runtime_extension_asset_code_pairs
                .len(),
            shared_identical_prefix_cell_count,
            shared_identical_prefix_end_code: hex_code(u16::try_from(
                shared_identical_prefix_cell_count - 1,
            )?),
            common_fixed_cells_identical_across_assets,
            common_fixed_cells_with_asset_variants,
            all_source_pixel_hash_count: all_source_pixel_hashes.len(),
            used_source_pixel_hash_count: used_source_pixel_hashes.len(),
            source_extension_zero_in_every_asset: self.source_extension_zero_in_every_asset,
            glyph_codes,
            limitations: vec![
                "A fixed TIM cell hash identifies pixels, not a Unicode character; the codebook remains unresolved until glyph recognition and reversible known-text checks agree."
                    .to_string(),
                "Runtime-extension glyph slots are addressable by the renderer but zero-filled on supported source media; their producer and state lifetime must be recovered before Korean allocation."
                    .to_string(),
                "A fixed cell with zero message occurrences is only unreferenced by the declared MGK pointer-table population, not approved as reclaimable storage."
                    .to_string(),
            ],
        };
        let token_codes = self
            .token_codes
            .into_iter()
            .map(|(code, accumulation)| {
                let spec = dialogue_control_spec(code)
                    .expect("tokenization admits only specified non-glyph codes");
                DialogueTokenCodeAudit {
                    code: hex_code(code),
                    kind: accumulation.kind.name().to_string(),
                    semantic_name: spec.semantic_name.to_string(),
                    argument_word_count: accumulation.argument_word_count,
                    occurrence_count: accumulation.occurrence_count,
                    meaning_status: format!("{}; {}", spec.meaning, spec.renderer_evidence),
                }
            })
            .collect();

        Ok(FinalizedDialogueSummary {
            token_count: self.token_count,
            glyph_occurrence_count: self.fixed_glyph_occurrence_count
                + self.runtime_extension_glyph_occurrence_count,
            fixed_glyph_occurrence_count: self.fixed_glyph_occurrence_count,
            runtime_extension_glyph_occurrence_count: self.runtime_extension_glyph_occurrence_count,
            alignment_padding_word_count: self.alignment_padding_word_count,
            font,
            token_codes,
        })
    }
}

impl From<&ParsedDialogueAtlas> for SharedAtlasStructure {
    fn from(atlas: &ParsedDialogueAtlas) -> Self {
        Self {
            clut_vram_x: atlas.clut_vram_x,
            clut_vram_y: atlas.clut_vram_y,
            clut_color_count: atlas.clut_color_count,
            image_vram_x: atlas.image_vram_x,
            image_vram_y: atlas.image_vram_y,
            pixel_data_offset: atlas.pixel_data_offset,
            addressable_slot_count: atlas.addressable_slot_count,
            source_extension_end: atlas.source_extension_end,
        }
    }
}
