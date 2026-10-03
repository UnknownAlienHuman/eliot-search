//! Isolated legacy immutable-snapshot harness.
//!
//! The production daemon entry is `src/entry.rs`. This crate root is retained
//! only for the `eliot-search-snapshotd` regression target.

#![forbid(unsafe_code)]

mod control_store;
mod lexical;
mod snapshot;
mod snapshot_harness;

fn main() {
    snapshot_harness::main();
}
