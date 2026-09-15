//! Sealed access mutation, snapshot, receipt and live-authority models.

use core::fmt;

use crate::sealed_access_codec::{
    AccessFenceRecord, AccessFenceState, validate_fence_id,
    validate_identifier,
};
use crate::sealed_digest::Sha256Digest;

use super::spec::SealedAccessError;

/// Exact operator mutation request. Generation and access generation are
/// assigned by the validated chain, never by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceMutation {
    /// Stable fence-chain identity.
    pub fence_id: String,
    /// Immutable idempotency identity.
    pub mutation_id: String,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable source revision.
    pub source_revision_id: String,
    /// Exact catalog object.
    pub catalog_object_id: String,
    /// Stable scope identity.
    pub scope_id: String,
    /// Monotone scope revision.
    pub scope_revision: u64,
    /// Stable policy identity.
    pub policy_id: String,
    /// Monotone policy revision.
    pub policy_revision: u64,
    /// Monotone purge-ledger generation.
    pub purge_generation: u64,
    /// Desired terminal state.
    pub state: AccessFenceState,
}

impl AccessFenceMutation {
    pub(super) fn validate(&self) -> Result<(), SealedAccessError> {
        validate_fence_id(&self.fence_id)?;
        for value in [
            &self.mutation_id,
            &self.source_id,
            &self.source_revision_id,
            &self.catalog_object_id,
            &self.scope_id,
            &self.policy_id,
        ] {
            validate_identifier(value)?;
        }
        if self.scope_revision == 0 || self.policy_revision == 0 {
            return Err(SealedAccessError::RevisionRegression);
        }
        Ok(())
    }
}

/// Access-fence append disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessAppendDisposition {
    /// A new immutable generation was appended.
    Created,
    /// An existing exact mutation was replayed without appending.
    Replay,
}

impl AccessAppendDisposition {
    /// Stable wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Replay => "REPLAY",
        }
    }
}

/// Current exact fence snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceSnapshot {
    /// Current record.
    pub record: AccessFenceRecord,
    /// SHA-256 of exact current record bytes.
    pub record_sha256: Sha256Digest,
    /// Current sealed object identity.
    pub object_id: String,
    /// Current transaction identity.
    pub transaction_id: String,
}

/// Append/replay receipt including the current head after the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceReceipt {
    /// Record created or originally associated with a replayed mutation.
    pub affected: AccessFenceSnapshot,
    /// Current chain head after the operation.
    pub current: AccessFenceSnapshot,
    /// Whether a generation was created or replayed.
    pub disposition: AccessAppendDisposition,
    /// Transaction and DPAPI readback were exact.
    pub readback_verified: bool,
}

/// Non-cloneable current read authority.
pub struct ActiveAccessFence {
    snapshot: AccessFenceSnapshot,
}

impl ActiveAccessFence {
    pub(super) const fn new(snapshot: AccessFenceSnapshot) -> Self {
        Self { snapshot }
    }

    /// Current exact fence record.
    #[must_use]
    pub const fn record(&self) -> &AccessFenceRecord {
        &self.snapshot.record
    }
}

impl fmt::Debug for ActiveAccessFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveAccessFence")
            .field("fence_id", &self.snapshot.record.fence_id)
            .field("generation", &self.snapshot.record.generation)
            .field(
                "access_generation",
                &self.snapshot.record.access_generation,
            )
            .field("scope_revision", &self.snapshot.record.scope_revision)
            .field("policy_revision", &self.snapshot.record.policy_revision)
            .field("purge_generation", &self.snapshot.record.purge_generation)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LoadedFence {
    pub(super) snapshot: AccessFenceSnapshot,
}
