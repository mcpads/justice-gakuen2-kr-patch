#[path = "diary_scene/assets.rs"]
mod assets;
#[path = "diary_scene/audit.rs"]
mod audit;
#[cfg(test)]
#[path = "diary_scene/audit_tests.rs"]
mod audit_tests;
#[path = "diary_scene/build.rs"]
mod build;
#[path = "diary_scene/calendar.rs"]
mod calendar;
#[path = "diary_scene/embedded_catalogue.rs"]
mod embedded_catalogue;
#[path = "diary_scene/fixed_presentation.rs"]
mod fixed_presentation;
#[path = "diary_scene/model.rs"]
mod model;
#[path = "diary_scene/portraits.rs"]
mod portraits;
#[path = "diary_scene/runtime_bundle_build.rs"]
mod runtime_bundle_build;
#[path = "diary_scene/source.rs"]
mod source;

pub use audit::{
    DiaryLocationGraphicsAuditConfig, DiaryLocationGraphicsAuditReport,
    audit_diary_location_graphics,
};
pub(crate) use build::build_diary_scene_from_source;
pub use build::{DIARY_SCENE_BUILD_MANIFEST_FILE, build_diary_scene};
pub use model::{
    DiarySceneBuild, DiarySceneBuildConfig, DiarySceneFontSources, DiarySceneRuntimeBundleBuild,
    DiarySceneRuntimeBundleReport,
};
pub use source::DIARY_SCENE_PATH;

pub use artwork::{SceneArtworkArchiveBuild, SceneArtworkArchiveReport};

mod artwork;
mod month_overview;

mod scene_backgrounds;
