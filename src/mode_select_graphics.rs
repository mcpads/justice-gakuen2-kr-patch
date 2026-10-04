#[path = "mode_select_graphics/audit.rs"]
mod audit;
#[path = "mode_select_graphics/inventory.rs"]
mod inventory;
#[path = "mode_select_graphics/model.rs"]
mod model;
#[path = "mode_select_graphics/preview.rs"]
mod preview;
#[path = "mode_select_graphics/source.rs"]
mod source;
#[path = "mode_select_graphics/tim16.rs"]
mod tim16;

pub use audit::audit_mode_select_graphics;
pub use model::{ModeSelectGraphicsAuditConfig, ModeSelectGraphicsAuditReport};

#[cfg(test)]
#[path = "mode_select_graphics/audit_tests.rs"]
mod audit_tests;
