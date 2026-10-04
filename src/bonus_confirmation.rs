#[path = "bonus_confirmation/assets.rs"]
mod assets;
#[path = "bonus_confirmation/build.rs"]
mod build;
#[path = "bonus_confirmation/command_sequences.rs"]
mod command_sequences;
#[path = "bonus_confirmation/consumer.rs"]
mod consumer;
#[path = "bonus_confirmation/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_confirmation/glyph_ownership.rs"]
mod glyph_ownership;
#[path = "bonus_confirmation/glyph_slots.rs"]
mod glyph_slots;
#[path = "bonus_confirmation/model.rs"]
mod model;
#[path = "bonus_confirmation/overlay.rs"]
mod overlay;
#[path = "bonus_confirmation/records.rs"]
mod records;
#[path = "bonus_confirmation/source.rs"]
mod source;
#[path = "bonus_confirmation/text_units.rs"]
mod text_units;

pub(crate) use build::build_bonus_confirmation_from_inventory_source;
#[cfg(test)]
pub(crate) use build::build_bonus_confirmation_from_source;
pub use build::{
    BONUS_CONFIRMATION_BUILD_MANIFEST_FILE, BONUS_CONFIRMATION_OVERLAY_OUTPUT_FILE,
    build_bonus_confirmation,
};
pub(crate) use glyph_atlas::validate_fixed_record_space_is_blank;
pub(crate) use glyph_slots::reserved_glyph_codes as bonus_confirmation_reserved_glyph_codes;
pub use model::{
    BonusConfirmationBuild, BonusConfirmationBuildConfig, BonusConfirmationBuildReport,
    BonusConfirmationFontSource,
};

#[cfg(test)]
#[path = "bonus_confirmation/command_sequences_tests.rs"]
mod command_sequences_tests;
#[cfg(test)]
#[path = "bonus_confirmation/consumer_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "bonus_confirmation/overlay_tests.rs"]
mod overlay_tests;
#[cfg(test)]
#[path = "bonus_confirmation/records_tests.rs"]
mod records_tests;
#[cfg(test)]
#[path = "bonus_confirmation/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_confirmation/test_support.rs"]
mod test_support;
