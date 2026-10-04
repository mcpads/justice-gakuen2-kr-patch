#[path = "title_menu/assets.rs"]
mod assets;
#[path = "title_menu/build.rs"]
mod build;
#[path = "title_menu/model.rs"]
mod model;
#[path = "title_menu/source.rs"]
mod source;

pub use crate::contextual_texture_upload::{
    ContextualMenuGlyphUploadEntry as TitleGlyphUploadEntry,
    ContextualMenuGlyphUploadReport as TitleGlyphUploadReport,
};
pub use build::{BUILD_MANIFEST_FILE, build_title_menu_assets};
pub(crate) use build::{
    build_title_menu_assets_with_plan, collect_menu_requests,
    compress_title_overlay_with_source_limits,
};
pub use model::{
    TitleMenuBuildConfig, TitleMenuBuildReport, TitleMenuFontStyle, TitleMenuRecordBuild,
};
pub const TITLE_MENU_OVERLAY_PATH: &str = source::OVERLAY_PATH;

#[cfg(test)]
#[path = "title_menu_tests.rs"]
mod tests;
