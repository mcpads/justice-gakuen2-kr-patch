#[path = "character_select_graphics/allocation.rs"]
mod allocation;
#[path = "character_select_graphics/assets.rs"]
mod assets;
#[path = "character_select_graphics/audit.rs"]
mod audit;
#[path = "character_select_graphics/auxiliary_record_build.rs"]
mod auxiliary_record_build;
#[path = "character_select_graphics/build.rs"]
mod build;
#[path = "character_select_graphics/compressed_record.rs"]
mod compressed_record;
#[path = "character_select_graphics/consumer.rs"]
mod consumer;
#[path = "character_select_graphics/episode_card_consumer.rs"]
mod episode_card_consumer;
#[path = "character_select_graphics/model.rs"]
mod model;
mod native_regions;
#[path = "character_select_graphics/pause_menu_consumer.rs"]
mod pause_menu_consumer;
#[path = "character_select_graphics/plan.rs"]
mod plan;
#[path = "character_select_graphics/planned_texture_targets.rs"]
mod planned_texture_targets;
#[path = "character_select_graphics/planning_sources.rs"]
mod planning_sources;
#[path = "character_select_graphics/preview.rs"]
mod preview;
mod ready_layout;
#[path = "character_select_graphics/record_build.rs"]
mod record_build;
#[path = "character_select_graphics/render.rs"]
mod render;
#[path = "character_select_graphics/resource_loads.rs"]
mod resource_loads;
#[path = "character_select_graphics/route_census.rs"]
mod route_census;
#[path = "character_select_graphics/small_logo.rs"]
mod small_logo;
#[path = "character_select_graphics/source.rs"]
mod source;
#[path = "character_select_graphics/texture_surface_build.rs"]
mod texture_surface_build;
#[path = "character_select_graphics/texture_targets.rs"]
mod texture_targets;

pub use allocation::plan_character_select_atlas;
pub use audit::audit_character_select_graphics;
pub(crate) use build::build_character_select_atlas_from_source;
pub use build::{CHARACTER_SELECT_BUILD_MANIFEST_FILE, build_character_select_atlas};
pub(crate) use compressed_record::decode_built_record_streams;
pub use model::{
    CharacterSelectAtlasBuild, CharacterSelectAtlasBuildConfig, CharacterSelectAtlasBuildReport,
    CharacterSelectAtlasPlan, CharacterSelectAtlasPlanConfig, CharacterSelectAtlasPlanReport,
    CharacterSelectConsumerLocationStatus, CharacterSelectConsumerPlan,
    CharacterSelectConsumerReferenceKind, CharacterSelectFontRole, CharacterSelectFontSources,
    CharacterSelectFontStyle, CharacterSelectGraphicsAuditConfig,
    CharacterSelectGraphicsAuditReport, CharacterSelectProducerBindingStatus,
    CharacterSelectRouteCensus, CharacterSelectRouteConsumerTarget, CharacterSelectRouteOccurrence,
    CharacterSelectRouteOwner, CharacterSelectRouteProducerTarget, CharacterSelectRouteReadiness,
    CharacterSelectTargetTimBuild,
};
pub use plan::write_character_select_atlas_plan;
pub(crate) use resource_loads::validate_consumer_resource_loads;

#[cfg(test)]
#[path = "character_select_graphics/audit_tests.rs"]
mod audit_tests;

#[cfg(test)]
#[path = "character_select_graphics/allocation_tests.rs"]
mod allocation_tests;

#[cfg(test)]
#[path = "character_select_graphics/assets_tests.rs"]
mod assets_tests;

#[cfg(test)]
#[path = "character_select_graphics/build_tests.rs"]
mod build_tests;

#[cfg(test)]
#[path = "character_select_graphics/consumer_tests.rs"]
mod consumer_tests;

#[cfg(test)]
#[path = "character_select_graphics/render_tests.rs"]
mod render_tests;

pub(crate) use allocation::register_battle_pause_return;
