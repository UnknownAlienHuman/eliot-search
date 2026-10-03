//! Legacy immutable-snapshot implementation behind the harness-only facade.

mod capture;
mod fingerprint;
mod manifest;
mod model;
mod policy;
mod search;
mod spec;
mod storage;

pub use fingerprint::{fingerprint, hex32};
pub use model::{
    SnapshotIndex, SnapshotLimits, SnapshotMatch, SnapshotSearchResult,
    SnapshotStats,
};
