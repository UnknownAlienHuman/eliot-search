//! Immutable encrypted retained-revision storage semantics.
//!
//! The public crate root is intentionally a thin facade. The private
//! `kernel` module owns the pure revision state machine and its contract
//! tests. `immutable_object` owns qualified legacy filesystem mechanics and
//! `legacy_inventory` owns the closed legacy layout/name grammar during T02;
//! encryption and secret-store composition remain outside.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[allow(
    clippy::missing_errors_doc,
    clippy::similar_names,
    clippy::too_many_lines
)]
mod immutable_object;
mod kernel;
mod legacy_inventory;

pub use immutable_object::*;
pub use kernel::*;
pub use legacy_inventory::*;
