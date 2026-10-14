//! Sealed owner-epoch composition behind the stable module facade.

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/identity.rs"]
mod identity;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/platform.rs"]
mod platform;
#[path = "kernel/spec.rs"]
mod spec;

pub use codec::OwnerEpochRecord;
pub use model::OwnerEpochGuard;
pub use spec::{MAX_OWNER_EPOCH_RECORDS, OwnerEpochError};

use std::path::Path;

/// Reads the sealed epoch head without acquiring live authority.
///
/// A returned record proves only that a sealed mirror exists and binds some
/// physical root; it never grants ownership.
#[allow(dead_code)]
pub fn latest_sealed_head(
    data_root: &Path,
) -> Result<Option<OwnerEpochRecord>, OwnerEpochError> {
    platform::latest_head(data_root)
}

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
