#[path = "options/allocation.rs"]
mod allocation;
#[path = "options/assets.rs"]
mod assets;
#[path = "options/background_assets.rs"]
mod background_assets;
#[path = "options/background_build.rs"]
mod background_build;
#[path = "options/background_model.rs"]
mod background_model;
#[path = "options/build.rs"]
mod build;
#[path = "options/contextual_glyphs.rs"]
mod contextual_glyphs;
#[path = "options/description_assets.rs"]
mod description_assets;
#[path = "options/description_model.rs"]
mod description_model;
#[path = "options/description_rebuild.rs"]
mod description_rebuild;
#[path = "options/description_source.rs"]
mod description_source;
#[path = "options/description_stream.rs"]
mod description_stream;
#[path = "options/main_executable_contributions.rs"]
mod main_executable_contributions;
#[path = "options/menu_textures.rs"]
mod menu_textures;
#[path = "options/model.rs"]
mod model;
#[path = "options/record_build.rs"]
mod record_build;
#[path = "options/records_main.rs"]
mod records_main;
#[path = "options/records_runtime.rs"]
mod records_runtime;
#[path = "options/runtime_glyph_upload.rs"]
mod runtime_glyph_upload;
#[path = "options/source.rs"]
mod source;
#[path = "options/source_asset_sync.rs"]
mod source_asset_sync;
#[path = "options/source_catalog.rs"]
mod source_catalog;
#[path = "options/text_rebuild.rs"]
mod text_rebuild;
#[path = "options/texture_survey.rs"]
mod texture_survey;

pub use assets::audit_options_assets;
pub use build::build_options_assets;

/// Build and verify Options records without creating a development disc.
pub fn build_options_components(
    config: &OptionsBuildConfig,
) -> anyhow::Result<OptionsRecordBuildReport> {
    Ok(record_build::build_options_records(config)?.report)
}
pub(crate) use description_source::OPTINFO_PATH as OPTIONS_INFO_PATH;
pub(crate) use main_executable_contributions::register_main_executable_candidates;
pub use model::{
    OptionsAssetAuditConfig, OptionsAssetAuditReport, OptionsBackgroundFontStyles,
    OptionsBuildConfig, OptionsBuildReport, OptionsFontStyle, OptionsFontStyles,
    OptionsRecordBuildReport, OptionsSourceAssetSyncConfig, OptionsSourceAssetSyncReport,
    RecordsMainFontStyles, RecordsStatusFontStyles,
};
pub(crate) use record_build::{
    OptionsRecordBuild, build_options_records_with_plan, collect_menu_requests,
};
pub(crate) use source::OVERLAY_PATH as OPTIONS_OVERLAY_PATH;
pub use source_asset_sync::sync_options_source_assets;
pub use texture_survey::{
    OptionsSpriteTextureSurveyConfig, OptionsSpriteTextureSurveyReport,
    survey_options_sprite_texture,
};

#[cfg(test)]
#[path = "options/assets_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "options/allocation_tests.rs"]
mod allocation_tests;

#[cfg(test)]
#[path = "options/menu_textures_tests.rs"]
mod menu_textures_tests;

#[cfg(test)]
#[path = "options/text_rebuild_tests.rs"]
mod text_rebuild_tests;

#[cfg(test)]
#[path = "options/records_main_tests.rs"]
mod records_main_tests;

#[cfg(test)]
#[path = "options/source_catalog_tests.rs"]
mod source_catalog_tests;

#[cfg(test)]
#[path = "options/description_source_tests.rs"]
mod description_source_tests;

#[cfg(test)]
#[path = "options/description_rebuild_tests.rs"]
mod description_rebuild_tests;

#[cfg(test)]
#[path = "options/description_stream_tests.rs"]
mod description_stream_tests;

#[cfg(test)]
#[path = "options/background_tests.rs"]
mod background_tests;

#[cfg(test)]
#[path = "options/records_runtime_tests.rs"]
mod records_runtime_tests;
