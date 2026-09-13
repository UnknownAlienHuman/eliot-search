//! Immutable encrypted retained-revision storage semantics.
//!
//! The public crate root is intentionally a thin facade. The private
//! `kernel` module owns the pure revision state machine and its contract
//! tests; concrete filesystem, encryption, and secret-store I/O remain
//! separate adapter responsibilities.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod kernel;

pub use kernel::*;
