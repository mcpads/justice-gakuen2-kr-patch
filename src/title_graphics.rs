#[path = "title_graphics/shared_logo.rs"]
pub(crate) mod shared_logo;
#[path = "title_graphics/assets.rs"]
mod assets;
#[path = "title_graphics/audit.rs"]
mod audit;
#[path = "title_graphics/authoring.rs"]
mod authoring;
#[path = "title_graphics/model.rs"]
mod model;
#[path = "title_graphics/preview.rs"]
mod preview;
#[path = "title_graphics/source.rs"]
mod source;

pub use authoring::TitleArtworkBuild;
pub(crate) use authoring::apply_title_graphics;

pub use audit::audit_title_graphics;
pub use model::{TitleGraphicsAuditConfig, TitleGraphicsAuditReport};

#[cfg(test)]
#[path = "title_graphics_tests.rs"]
mod tests;
