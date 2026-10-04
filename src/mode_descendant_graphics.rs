#[path = "mode_descendant_graphics/assets.rs"]
mod assets;
#[path = "mode_descendant_graphics/atlas_packer.rs"]
mod atlas_packer;
#[path = "mode_descendant_graphics/audit.rs"]
mod audit;
#[path = "mode_descendant_graphics/battle_lettering_layout.rs"]
mod battle_lettering_layout;
#[path = "mode_descendant_graphics/build.rs"]
mod build;
#[path = "mode_descendant_graphics/catalog.rs"]
mod catalog;
#[path = "mode_descendant_graphics/continue_schools.rs"]
mod continue_schools;
#[path = "mode_descendant_graphics/edit_badges.rs"]
mod edit_badges;
#[path = "mode_descendant_graphics/edit_command_sheet_build.rs"]
mod edit_command_sheet_build;
#[path = "mode_descendant_graphics/edit_command_sheets.rs"]
mod edit_command_sheets;
#[path = "mode_descendant_graphics/edit_conditions.rs"]
mod edit_conditions;
#[path = "mode_descendant_graphics/edit_fixed_ui_consumers.rs"]
mod edit_fixed_ui_consumers;
#[path = "mode_descendant_graphics/edit_heading.rs"]
mod edit_heading;
#[path = "mode_descendant_graphics/edit_move_names.rs"]
mod edit_move_names;
mod edit_team_up_names;
#[path = "mode_descendant_graphics/edit_technique_archive.rs"]
mod edit_technique_archive;
#[path = "mode_descendant_graphics/edit_technique_names.rs"]
mod edit_technique_names;
#[path = "mode_descendant_graphics/gorin_heading.rs"]
mod gorin_heading;
use crate::indexed_member_archive;
#[path = "mode_descendant_graphics/model.rs"]
mod model;
#[path = "mode_descendant_graphics/practical_exam.rs"]
mod practical_exam;
#[path = "mode_descendant_graphics/practical_results.rs"]
mod practical_results;
#[path = "mode_descendant_graphics/record_compositor.rs"]
mod record_compositor;
#[path = "mode_descendant_graphics/source.rs"]
mod source;
#[path = "mode_descendant_graphics/texture_usage.rs"]
mod texture_usage;
pub(crate) use battle_lettering_layout::register_battle_lettering_layout;

pub use audit::audit_mode_descendant_graphics;
pub(crate) use build::build_mode_descendant_graphics_from_source;
pub use build::{MODE_DESCENDANT_BUILD_MANIFEST_FILE, build_mode_descendant_graphics};
pub use catalog::{EDIT_SHARED_UI_PATH, GORIN_MAIN_MENU_PATH};
pub use model::{
    ModeDescendantFontSources, ModeDescendantGraphicsAuditConfig,
    ModeDescendantGraphicsAuditReport, ModeDescendantGraphicsBuild,
    ModeDescendantGraphicsBuildConfig, ModeDescendantGraphicsBuildReport, ModeDescendantRecord,
    ModeDescendantRecordBuild, ModeDescendantStorageKind, ModeDescendantSurface,
    ModeDescendantTextureConsumer, ModeDescendantTextureUsage,
};
pub use practical_results::{
    PracticalResultBuildReport, PracticalResultDevelopmentStatus, PracticalResultReleaseStatus,
    PracticalResultSourceUsageStatus, PracticalResultStrategy, PracticalResultUnitBuild,
};

#[cfg(test)]
#[path = "mode_descendant_graphics/assets_tests.rs"]
mod assets_tests;

#[cfg(test)]
#[path = "mode_descendant_graphics/gorin_heading_tests.rs"]
mod gorin_heading_tests;

#[path = "mode_descendant_graphics/training_menu.rs"]
mod training_menu;

#[path = "mode_descendant_graphics/illustrated_panels.rs"]
mod illustrated_panels;

#[path = "mode_descendant_graphics/gorin_selector_names.rs"]
mod gorin_selector_names;

#[path = "mode_descendant_graphics/battle_announcements.rs"]
mod battle_announcements;
mod vertical_names;

#[path = "mode_descendant_graphics/gorin_retry.rs"]
mod gorin_retry;

#[path = "mode_descendant_graphics/gorin_dance_intro.rs"]
mod gorin_dance_intro;

#[path = "mode_descendant_graphics/gorin_hud_labels.rs"]
mod gorin_hud_labels;

#[path = "mode_descendant_graphics/gorin_gauge_labels.rs"]
mod gorin_gauge_labels;

#[path = "mode_descendant_graphics/gorin_announcements.rs"]
mod gorin_announcements;

#[path = "mode_descendant_graphics/main_title.rs"]
mod main_title;

#[path = "mode_descendant_graphics/gorin_home_run.rs"]
mod gorin_home_run;

#[path = "mode_descendant_graphics/gorin_sprint_announcements.rs"]
mod gorin_sprint_announcements;

#[path = "mode_descendant_graphics/gorin_dance_announcements.rs"]
mod gorin_dance_announcements;

#[path = "mode_descendant_graphics/gorin_dance_logo.rs"]
mod gorin_dance_logo;
