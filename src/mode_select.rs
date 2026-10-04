#[path = "mode_select/logo.rs"]
mod logo;
#[path = "mode_select/assets.rs"]
mod assets;
#[path = "mode_select/build.rs"]
mod build;
#[path = "mode_select/images.rs"]
mod images;
#[path = "mode_select/model.rs"]
mod model;
#[path = "mode_select/source.rs"]
pub(crate) mod source;

pub use build::build_mode_select_assets;
pub(crate) use build::build_mode_select_assets_from_source;
pub(crate) use model::ModeSelectRecordBuild;
pub use model::{
    ModeSelectBuildConfig, ModeSelectBuildReport, ModeSelectFontBuild, ModeSelectFontSources,
};
pub const MODE_SELECT_MENU_PATH: &str = source::MENU_PATH;

#[cfg(test)]
#[path = "mode_select_tests.rs"]
mod tests;
