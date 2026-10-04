#[path = "bonus_inventory_action_labels/assets.rs"]
mod assets;
#[path = "bonus_inventory_action_labels/build.rs"]
mod build;
#[path = "bonus_inventory_action_labels/model.rs"]
mod model;
#[path = "bonus_inventory_action_labels/overlay.rs"]
mod overlay;
#[path = "bonus_inventory_action_labels/source.rs"]
mod source;

pub(crate) use build::build_bonus_inventory_action_labels_from_inventory_source;
pub use build::{
    BONUS_INVENTORY_ACTION_LABEL_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_ACTION_LABEL_OVERLAY_OUTPUT_FILE, build_bonus_inventory_action_labels,
};
pub use model::{
    BonusInventoryActionLabelBuild, BonusInventoryActionLabelBuildConfig,
    BonusInventoryActionLabelBuildReport, BonusInventoryActionLabelFontSource,
};
pub(crate) use source::reserved_glyph_codes as bonus_action_label_reserved_glyph_codes;

#[cfg(test)]
#[path = "bonus_inventory_action_labels_tests.rs"]
mod tests;
