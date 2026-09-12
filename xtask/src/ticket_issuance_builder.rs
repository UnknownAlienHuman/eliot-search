//! Executable schema-v2 ticket-issuance advisory planner.
//!
//! Every repository input is read from one immutable Git commit. The planner
//! emits only canonical advisory JSON to stdout or below
//! `artifacts/ticket-issuance-plans/`; it has no control-record mutation path.

mod assemble;
mod context;
mod control;
mod drafts;
mod model;
mod repository;
mod util;
mod write;

pub use assemble::build_plan;
pub use model::{
    PlannerCheck, TicketIssuanceBuild, TicketIssuanceBuildError,
    TicketIssuanceBuildOptions,
};
pub use write::write_plan;
