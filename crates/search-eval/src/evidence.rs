//! Deterministic A/B/C scheduling and immutable attempt evidence.
//!
//! Scheduling, attempt validation, process-local replay fencing, and external
//! probes are separate bounded owners. The public crate-root API is preserved
//! through this facade.

mod attempt;
mod ledger;
mod probe;
mod schedule;

pub use attempt::*;
pub use ledger::*;
pub use probe::*;
pub use schedule::*;
