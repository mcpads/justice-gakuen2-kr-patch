#[path = "bonus_shop_exit_confirmation/assets.rs"]
mod assets;
#[path = "bonus_shop_exit_confirmation/build.rs"]
mod build;
#[path = "bonus_shop_exit_confirmation/command_sequence.rs"]
mod command_sequence;
#[path = "bonus_shop_exit_confirmation/consumer.rs"]
mod consumer;
#[path = "bonus_shop_exit_confirmation/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_shop_exit_confirmation/glyph_ownership.rs"]
mod glyph_ownership;
#[path = "bonus_shop_exit_confirmation/glyph_slots.rs"]
mod glyph_slots;
#[path = "bonus_shop_exit_confirmation/model.rs"]
mod model;
#[path = "bonus_shop_exit_confirmation/output.rs"]
mod output;
#[path = "bonus_shop_exit_confirmation/overlay.rs"]
mod overlay;
#[path = "bonus_shop_exit_confirmation/source.rs"]
mod source;

pub(crate) use build::build_bonus_shop_exit_confirmation_from_source;
pub use build::{
    BONUS_SHOP_EXIT_CONFIRMATION_BUILD_MANIFEST_FILE,
    BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE, build_bonus_shop_exit_confirmation,
};
pub(crate) use glyph_slots::reserved_glyph_codes as bonus_shop_exit_reserved_glyph_codes;
pub use model::{
    BonusShopExitConfirmationBuild, BonusShopExitConfirmationBuildConfig,
    BonusShopExitConfirmationBuildReport, BonusShopExitConfirmationFontSource,
    ShopExitGlyphOwnershipEvidenceReport,
};

#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/command_sequence_tests.rs"]
mod command_sequence_tests;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/consumer_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/overlay_tests.rs"]
mod overlay_tests;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/raw_source_tests.rs"]
mod raw_source_tests;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "bonus_shop_exit_confirmation/writer_chain_tests.rs"]
mod writer_chain_tests;
