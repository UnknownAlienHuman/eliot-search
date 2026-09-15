//! Isolated legacy immutable-snapshot harness.
//!
//! The production daemon is `src/entry.rs`; this module is compiled only by
//! the `eliot-search-snapshotd` test target retained for compatibility and
//! regression comparison.

#[path = "snapshot/kernel.rs"]
mod kernel;

pub(crate) use kernel::*;
