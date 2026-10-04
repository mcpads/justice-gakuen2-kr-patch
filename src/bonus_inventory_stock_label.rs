#[path = "bonus_inventory_stock_label/assets.rs"]
mod assets;
#[path = "bonus_inventory_stock_label/build.rs"]
mod build;
#[path = "bonus_inventory_stock_label/consumer.rs"]
mod consumer;
#[path = "bonus_inventory_stock_label/model.rs"]
mod model;
#[path = "bonus_inventory_stock_label/overlay.rs"]
mod overlay;
#[path = "bonus_inventory_stock_label/ownership.rs"]
mod ownership;
#[path = "bonus_inventory_stock_label/source.rs"]
mod source;

pub(crate) use build::build_bonus_inventory_stock_label_from_inventory_source;
pub use build::{
    BONUS_INVENTORY_STOCK_LABEL_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_STOCK_LABEL_OVERLAY_OUTPUT_FILE, build_bonus_inventory_stock_label,
};
pub use model::{
    BonusInventoryStockLabelBuild, BonusInventoryStockLabelBuildConfig,
    BonusInventoryStockLabelBuildReport, BonusInventoryStockLabelFontSource,
    StockLabelGlyphBuildReport,
};
pub(crate) use source::reserved_glyph_codes as bonus_stock_label_reserved_glyph_codes;
pub use source::{
    BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH, BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH,
};

#[cfg(test)]
#[path = "bonus_inventory_stock_label_tests.rs"]
mod tests;
