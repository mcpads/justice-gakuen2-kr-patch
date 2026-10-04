#[path = "menu_runtime_probe/build.rs"]
mod build;
#[path = "menu_runtime_probe/model.rs"]
mod model;

pub use build::build_menu_runtime_probe;
pub use model::{MenuRuntimeProbeBuildConfig, MenuRuntimeProbeBuildReport};

#[cfg(test)]
#[path = "menu_runtime_probe/build_tests.rs"]
mod build_tests;
