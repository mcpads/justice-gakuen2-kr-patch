#[path = "development_build_spec/document.rs"]
mod document;
#[path = "development_build_spec/load.rs"]
mod load;
#[path = "development_build_spec/model.rs"]
mod model;

pub use load::load_development_build_spec;
pub use model::{
    DevelopmentAssetPaths, DevelopmentFontSources, DevelopmentSpecificationPaths,
    LoadedDevelopmentBuildSpec, NameEntryFontSources, ShiftedSizedFontSource, SizedFontSource,
};

#[cfg(test)]
#[path = "development_build_spec_tests.rs"]
mod tests;
