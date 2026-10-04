#[path = "menu_texture_variant/build.rs"]
mod build;
#[path = "menu_texture_variant/model.rs"]
mod model;

pub use build::build_menu_texture_variant;
pub use model::{MenuTextureVariantBuildConfig, MenuTextureVariantBuildReport};

#[cfg(test)]
#[path = "menu_texture_variant/build_tests.rs"]
mod build_tests;
