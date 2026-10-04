pub(crate) mod action_spacing;
#[path = "diary_header/assets.rs"]
mod assets;
mod backup_consumer;
#[path = "diary_header/build.rs"]
mod build;
#[path = "diary_header/club_consumer.rs"]
mod club_consumer;
mod date_consumer;
#[path = "diary_header/disc.rs"]
mod disc;
mod indexed_art;
#[path = "diary_header/menu_consumer.rs"]
mod menu_consumer;
mod menu_palette;
#[path = "diary_header/model.rs"]
mod model;
#[path = "diary_header/source.rs"]
mod source;
mod status_consumer;

pub(crate) use build::build_diary_header_from_source;
pub use build::{DIARY_HEADER_BUILD_MANIFEST_FILE, build_diary_header};
pub use disc::build_diary_header_disc;
pub use model::{
    DiaryHeaderBuild, DiaryHeaderBuildConfig, DiaryHeaderBuildReport, DiaryHeaderDiscBuildConfig,
    DiaryHeaderDiscBuildReport, DiaryHeaderFontRole, DiaryHeaderFontSources, DiaryHeaderFontStyle,
    DiaryHeaderGlyphLayout, DiaryHeaderIndexedRendering,
};
pub use source::DIARY_HEADER_PATH;

#[cfg(test)]
#[path = "diary_header_tests.rs"]
mod tests;
