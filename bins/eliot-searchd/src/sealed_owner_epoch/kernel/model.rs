//! Non-cloneable owner-epoch guard retaining the exact root lease.

use core::fmt;
use std::path::Path;

use crate::sealed_digest::Sha256Digest;
use crate::sealed_root_lock::SealedRootLease;

use super::codec::OwnerEpochRecord;
use super::platform;
use super::spec::OwnerEpochError;

/// Process-local owner authority. It is deliberately non-cloneable.
pub struct OwnerEpochGuard {
    root_lease: SealedRootLease,
    record: OwnerEpochRecord,
    record_sha256: Sha256Digest,
    object_id: String,
    transaction_id: String,
}

impl OwnerEpochGuard {
    /// Acquires the data root and advances its monotone epoch exactly once.
    pub fn acquire(data_root: &Path) -> Result<Self, OwnerEpochError> {
        platform::acquire(data_root)
    }

    pub(super) fn new(
        root_lease: SealedRootLease,
        record: OwnerEpochRecord,
        record_sha256: Sha256Digest,
        object_id: String,
        transaction_id: String,
    ) -> Self {
        Self {
            root_lease,
            record,
            record_sha256,
            object_id,
            transaction_id,
        }
    }

    /// Current non-zero owner epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.record.epoch
    }

    /// Current physical-root binding digest.
    #[must_use]
    pub const fn root_binding_sha256(&self) -> Sha256Digest {
        self.record.root_binding_sha256
    }

    /// SHA-256 of exact current epoch-record plaintext.
    #[must_use]
    pub const fn record_sha256(&self) -> Sha256Digest {
        self.record_sha256
    }

    /// Whether the exact OS data-root lock remains held.
    #[must_use]
    pub const fn root_lock_held(&self) -> bool {
        self.root_lease.is_held()
    }
}

impl fmt::Debug for OwnerEpochGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerEpochGuard")
            .field("epoch", &self.epoch())
            .field("root_binding_sha256", &self.root_binding_sha256())
            .field("record_sha256", &self.record_sha256())
            .field("object_id", &self.object_id)
            .field("transaction_id", &self.transaction_id)
            .field("root_lock_held", &self.root_lock_held())
            .field("record", &"<redacted>")
            .field("root_lease", &"<redacted>")
            .finish()
    }
}
