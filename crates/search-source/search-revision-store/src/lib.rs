//! Immutable encrypted retained-revision storage semantics.
//!
//! The public crate root is intentionally a thin facade. The private
//! `kernel` module owns the pure revision state machine and its contract
//! tests. `immutable_object` owns qualified legacy filesystem mechanics during
//! the T02 migration; encryption and secret-store composition remain outside.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod immutable_object;
mod kernel;

pub use immutable_object::*;
pub use kernel::*;
