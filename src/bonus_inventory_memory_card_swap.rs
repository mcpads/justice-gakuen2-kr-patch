#[path = "bonus_inventory_memory_card_swap/assets.rs"]
mod assets;
#[path = "bonus_inventory_memory_card_swap/build.rs"]
mod build;
#[path = "bonus_inventory_memory_card_swap/command_sequences.rs"]
mod command_sequences;
#[path = "bonus_inventory_memory_card_swap/consumer.rs"]
mod consumer;
#[path = "bonus_inventory_memory_card_swap/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_inventory_memory_card_swap/glyph_ownership.rs"]
mod glyph_ownership;
#[path = "bonus_inventory_memory_card_swap/glyph_slots.rs"]
mod glyph_slots;
#[path = "bonus_inventory_memory_card_swap/model.rs"]
mod model;
#[path = "bonus_inventory_memory_card_swap/output.rs"]
mod output;
#[path = "bonus_inventory_memory_card_swap/overlay.rs"]
mod overlay;
#[path = "bonus_inventory_memory_card_swap/source.rs"]
mod source;

pub(crate) use build::build_bonus_inventory_memory_card_swap_from_inventory_source;
#[cfg(test)]
pub(crate) use build::build_bonus_inventory_memory_card_swap_from_source;
pub use build::{
    BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE, build_bonus_inventory_memory_card_swap,
};
pub(crate) use glyph_slots::reserved_glyph_codes as bonus_memory_card_swap_reserved_glyph_codes;
pub use model::{
    BonusInventoryMemoryCardSwapBuild, BonusInventoryMemoryCardSwapBuildConfig,
    BonusInventoryMemoryCardSwapBuildReport, BonusInventoryMemoryCardSwapFontSource,
    MemoryCardSwapGlyphOwnershipEvidenceReport,
};

#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/command_sequences_tests.rs"]
mod command_sequences_tests;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/consumer_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/overlay_tests.rs"]
mod overlay_tests;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/raw_source_tests.rs"]
mod raw_source_tests;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "bonus_inventory_memory_card_swap/writer_chain_tests.rs"]
mod writer_chain_tests;
