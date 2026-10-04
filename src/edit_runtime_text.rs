#[path = "edit_runtime_text/assets.rs"]
mod assets;
mod atlas_requests;
pub(crate) use atlas_requests::collect_menu_requests;
#[path = "edit_runtime_text/build.rs"]
mod build;
#[path = "edit_runtime_text/button_layout.rs"]
mod button_layout;
#[path = "edit_runtime_text/kanri_name_runtime.rs"]
mod kanri_name_runtime;
#[path = "edit_runtime_text/model.rs"]
mod model;
mod pass_button_layout;
mod pass_cpu_summary;
#[path = "edit_runtime_text/pass_fixed_presentation.rs"]
mod pass_fixed_presentation;
#[path = "edit_runtime_text/pass_password_decision.rs"]
mod pass_password_decision;
#[path = "edit_runtime_text/pass_password_title.rs"]
mod pass_password_title;
#[path = "edit_runtime_text/pass_title_renderer.rs"]
mod pass_title_renderer;
mod password_alphabet;
mod password_codec;
mod registration_list_layout;
#[path = "edit_runtime_text/source.rs"]
mod source;

pub(crate) use build::{build_edit_runtime_text, validate_edit_runtime_text_source_bindings};
pub use kanri_name_runtime::KanriTaggedNameRuntimeReport;
pub(crate) use model::{EditRuntimeTextBuild, EditRuntimeTextBuildConfig};
pub(crate) use source::{OVERLAY_PATH, PASS_PATH};

#[cfg(test)]
#[path = "edit_runtime_text_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use atlas_requests::validate_menu_plan;
