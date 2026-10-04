#[path = "bonus_j_bank_return_label/assets.rs"]
mod assets;
#[path = "bonus_j_bank_return_label/build.rs"]
mod build;
#[path = "bonus_j_bank_return_label/command_sequence.rs"]
mod command_sequence;
#[path = "bonus_j_bank_return_label/consumer.rs"]
mod consumer;
#[path = "bonus_j_bank_return_label/glyph_atlas.rs"]
mod glyph_atlas;
#[path = "bonus_j_bank_return_label/glyph_ownership.rs"]
mod glyph_ownership;
#[path = "bonus_j_bank_return_label/glyph_slots.rs"]
mod glyph_slots;
#[path = "bonus_j_bank_return_label/model.rs"]
mod model;
#[path = "bonus_j_bank_return_label/output.rs"]
mod output;
#[path = "bonus_j_bank_return_label/overlay.rs"]
mod overlay;
#[path = "bonus_j_bank_return_label/source.rs"]
mod source;

pub(crate) use build::build_bonus_j_bank_return_label_from_inventory_source;
pub use build::{
    BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE, BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE,
    build_bonus_j_bank_return_label,
};
pub(crate) use glyph_slots::reserved_glyph_codes as bonus_j_bank_return_label_reserved_glyph_codes;
pub use model::{
    BonusJBankReturnLabelBuild, BonusJBankReturnLabelBuildConfig, BonusJBankReturnLabelBuildReport,
    BonusJBankReturnLabelFontSource, JBankReturnLabelGlyphOwnershipEvidenceReport,
};

#[cfg(test)]
#[path = "bonus_j_bank_return_label/command_sequence_tests.rs"]
mod command_sequence_tests;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/consumer_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/overlay_tests.rs"]
mod overlay_tests;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/raw_source_tests.rs"]
mod raw_source_tests;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/source_build_tests.rs"]
mod source_build_tests;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "bonus_j_bank_return_label/writer_chain_tests.rs"]
mod writer_chain_tests;
