#[path = "bonus_inventory_card_acquisition/assets.rs"]
mod assets;
#[path = "bonus_inventory_card_acquisition/build.rs"]
mod build;
#[path = "bonus_inventory_card_acquisition/command_sequences.rs"]
mod command_sequences;
#[path = "bonus_inventory_card_acquisition/consumer.rs"]
mod consumer;
#[path = "bonus_inventory_card_acquisition/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_inventory_card_acquisition/glyph_ownership.rs"]
mod glyph_ownership;
#[path = "bonus_inventory_card_acquisition/glyph_slots.rs"]
mod glyph_slots;
#[path = "bonus_inventory_card_acquisition/model.rs"]
mod model;
#[path = "bonus_inventory_card_acquisition/output.rs"]
mod output;
#[path = "bonus_inventory_card_acquisition/overlay.rs"]
mod overlay;
#[path = "bonus_inventory_card_acquisition/source.rs"]
mod source;

pub(crate) use build::build_bonus_inventory_card_acquisition_from_inventory_source;
#[cfg(test)]
pub(crate) use build::build_bonus_inventory_card_acquisition_from_source;
pub use build::{
    BONUS_INVENTORY_CARD_ACQUISITION_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_OUTPUT_FILE, build_bonus_inventory_card_acquisition,
};
pub(crate) use glyph_slots::reserved_glyph_codes as bonus_card_acquisition_reserved_glyph_codes;
pub use model::{
    BonusInventoryCardAcquisitionBuild, BonusInventoryCardAcquisitionBuildConfig,
    BonusInventoryCardAcquisitionBuildReport, BonusInventoryCardAcquisitionFontSource,
    CardAcquisitionGlyphOwnershipEvidenceReport,
};

#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/command_sequences_tests.rs"]
mod command_sequences_tests;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/consumer_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/overlay_tests.rs"]
mod overlay_tests;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/raw_source_tests.rs"]
mod raw_source_tests;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "bonus_inventory_card_acquisition/writer_chain_tests.rs"]
mod writer_chain_tests;
