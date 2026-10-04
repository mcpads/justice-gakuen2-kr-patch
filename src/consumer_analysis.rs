//! Investigation-only inputs and shared machinery for static consumer analysis.
//!
//! Product builders must consume reviewed target specifications instead of
//! calling this module to discover patch locations or select assets.

#[path = "consumer_analysis/audit.rs"]
mod audit;
#[path = "consumer_analysis/catalog.rs"]
mod catalog;
#[path = "consumer_analysis/catalog_selector_domain.rs"]
mod catalog_selector_domain;
#[path = "consumer_analysis/control_transfer_profiles.rs"]
mod control_transfer_profiles;
#[path = "consumer_analysis/model.rs"]
mod model;
#[path = "consumer_analysis/pointer_run_references.rs"]
mod pointer_run_references;
#[path = "consumer_analysis/profiles.rs"]
pub(crate) mod profiles;
#[path = "consumer_analysis/program_analysis.rs"]
mod program_analysis;
#[path = "consumer_analysis/renderer_argument_boundary.rs"]
mod renderer_argument_boundary;
#[path = "consumer_analysis/renderer_call_census.rs"]
mod renderer_call_census;
#[path = "consumer_analysis/selector_state.rs"]
mod selector_state;

pub use audit::audit_static_consumers;
pub use model::{
    DEFAULT_POINTER_RUN_VALUE_FLOW_STATE_BUDGET, DEFAULT_STATIC_CONSUMER_VALUE_FLOW_STATE_BUDGET,
    StaticConsumerAuditConfig, StaticConsumerAuditReport,
};
pub(crate) use program_analysis::analyze_loaded_program;

mod memory_references;
pub use memory_references::{MemoryReferenceAuditConfig, audit_memory_references};
