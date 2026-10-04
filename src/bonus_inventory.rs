#[path = "bonus_inventory/card_back.rs"]
mod card_back;
#[path = "bonus_inventory/category_sprites.rs"]
mod category_sprites;
#[path = "bonus_inventory/device_text.rs"]
mod device_text;
#[path = "bonus_inventory/item_labels.rs"]
mod item_labels;
pub(crate) use category_sprites::apply_category_sprite_geometry;
#[path = "bonus_inventory/assets.rs"]
mod assets;
#[path = "bonus_inventory/build.rs"]
mod build;
#[path = "bonus_inventory/catalog.rs"]
mod catalog;
#[path = "bonus_inventory/model.rs"]
mod model;
#[path = "bonus_inventory/raster.rs"]
mod raster;
#[path = "bonus_inventory/source.rs"]
mod source;

pub(crate) use build::build_bonus_inventory_from_inventory_source;
pub use build::{BONUS_INVENTORY_BUILD_MANIFEST_FILE, build_bonus_inventory};
pub use model::{
    BonusInventoryBuild, BonusInventoryBuildConfig, BonusInventoryBuildReport,
    BonusInventoryFontSources, BonusInventoryTextStyleSource,
};
pub use source::BONUS_INVENTORY_PATH;

#[cfg(test)]
#[path = "bonus_inventory_tests.rs"]
mod tests;
