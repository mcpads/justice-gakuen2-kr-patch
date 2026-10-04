//! Loads adopted, source-bound surface traversal specifications.
//!
//! Translation assets remain authoritative in their feature directories. This
//! module owns only traversal membership, consumer resolution, and the finite
//! boundary used by cumulative builds and later runtime coverage.

#[path = "surface_inventory/load.rs"]
mod load;
#[path = "surface_inventory/mode_select_binding.rs"]
mod mode_select_binding;
#[path = "surface_inventory/mode_select_character_select_routes.rs"]
mod mode_select_character_select_routes;
#[path = "surface_inventory/mode_select_direct_entry_routes.rs"]
mod mode_select_direct_entry_routes;
#[path = "surface_inventory/mode_select_dispatcher.rs"]
mod mode_select_dispatcher;
#[path = "surface_inventory/mode_select_options_records.rs"]
mod mode_select_options_records;
#[path = "surface_inventory/mode_select_practical_entry.rs"]
mod mode_select_practical_entry;
#[path = "surface_inventory/model.rs"]
mod model;
#[path = "surface_inventory/validation.rs"]
mod validation;

pub(crate) use load::load_surface_inventory;
pub(crate) use mode_select_binding::{
    ModeSelectInventorySources, validate_mode_select_descendant_inventory,
    validate_mode_select_inventory_source_binding,
};
pub(crate) use mode_select_direct_entry_routes::ModeSelectDirectEntrySources;
pub(crate) use model::ModeSelectPanelSelection;
pub use model::{ModeSelectDispatcherBuildReport, SurfaceInventoryBuildReport};

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
