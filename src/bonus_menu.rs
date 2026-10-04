#[path = "bonus_menu/assets.rs"]
mod assets;
#[path = "bonus_menu/build.rs"]
mod build;
#[path = "bonus_menu/heading.rs"]
mod heading;
#[path = "bonus_menu/model.rs"]
mod model;
#[path = "bonus_menu/ownership.rs"]
mod ownership;
#[path = "bonus_menu/raster.rs"]
mod raster;
#[path = "bonus_menu/source.rs"]
mod source;

pub(crate) use build::build_bonus_menu_from_source;
pub use build::{BONUS_MENU_BUILD_MANIFEST_FILE, build_bonus_menu};
pub use model::{
    BonusMenuBuild, BonusMenuBuildConfig, BonusMenuBuildReport, BonusMenuFontRole,
    BonusMenuFontSources,
};
pub use source::BONUS_MENU_PATH;

#[cfg(test)]
#[path = "bonus_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bonus_menu/heading_tests.rs"]
mod heading_tests;

#[cfg(test)]
#[path = "bonus_menu/raster_tests.rs"]
mod raster_tests;

#[cfg(test)]
#[path = "bonus_menu/ownership_tests.rs"]
mod ownership_tests;

#[cfg(test)]
#[path = "bonus_menu/source_build_tests.rs"]
mod source_build_tests;
