#[path = "menu_composition_probe/build.rs"]
mod build;
#[path = "menu_composition_probe/model.rs"]
mod model;

pub use build::build_menu_composition_probe;
pub use model::{MenuCompositionProbeBuildConfig, MenuCompositionProbeBuildReport};

#[cfg(test)]
#[path = "menu_composition_probe/build_tests.rs"]
mod build_tests;
