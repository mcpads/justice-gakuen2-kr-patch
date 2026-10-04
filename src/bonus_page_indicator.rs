#[path = "bonus_page_indicator/assets.rs"]
mod assets;
#[path = "bonus_page_indicator/build.rs"]
mod build;
#[path = "bonus_page_indicator/model.rs"]
mod model;
#[path = "bonus_page_indicator/overlay.rs"]
mod overlay;
#[path = "bonus_page_indicator/source.rs"]
mod source;

pub(crate) use build::build_bonus_page_indicator_from_inventory_source;
pub use build::{
    BONUS_PAGE_INDICATOR_BUILD_MANIFEST_FILE, BONUS_PAGE_INDICATOR_OVERLAY_OUTPUT_FILE,
    build_bonus_page_indicator,
};
pub use model::{
    BonusPageIndicatorBuild, BonusPageIndicatorBuildConfig, BonusPageIndicatorBuildReport,
    BonusPageIndicatorFontSource,
};
pub(crate) use source::reserved_glyph_codes as bonus_page_indicator_reserved_glyph_codes;
pub use source::{BONUS_PAGE_INDICATOR_INVENTORY_PATH, BONUS_PAGE_INDICATOR_OVERLAY_PATH};

#[cfg(test)]
#[path = "bonus_page_indicator_tests.rs"]
mod tests;
