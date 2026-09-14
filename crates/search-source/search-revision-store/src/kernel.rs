//! Immutable encrypted retained-revision storage semantics.
//!
//! This package performs no filesystem, database, encryption, or secret-store
//! I/O. Callers provide already encrypted finite payloads and exact backend
//! readback. The bounded owners below enforce source-revision monotonicity,
//! immutable residency/CAS identity, replay fencing, unknown-outcome recovery,
//! purge tombstones, exact lifecycle deletion, and content-free receipts.

#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

mod address;
mod backend;
mod binding;
mod error;
mod limits;
mod model;
mod residency;
mod store;

pub use address::*;
pub use backend::*;
pub use binding::*;
pub use error::*;
pub use limits::*;
pub use model::*;
pub use residency::*;
pub use store::*;

#[cfg(test)]
mod tests;
