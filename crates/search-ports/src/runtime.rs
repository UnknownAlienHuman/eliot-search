//! Runtime-facing vendor-neutral ports.

use search_contracts::UtcTimestamp;

use crate::{MutationIdentity, OperationContext, PackageOpaque, Port};

/// Process-local monotonic instant.
///
/// Values are meaningful only within one clock implementation and process
/// incarnation. They are not timestamps and must never be serialized.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicInstant(u64);

impl MonotonicInstant {
    /// Creates an instant from a clock-owned monotonic tick value.
    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Returns the opaque clock-local tick value.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }

    /// Computes a checked elapsed tick count.
    #[must_use]
    pub const fn checked_elapsed_since(self, earlier: Self) -> Option<u64> {
        self.0.checked_sub(earlier.0)
    }
}

/// Wall and monotonic clock abstraction.
pub trait ClockPort: Port {
    /// Returns canonical UTC wall-clock time for records and diagnostics.
    fn utc_now(&self) -> Result<UtcTimestamp, Self::Error>;

    /// Returns a process-local monotonic instant for deadlines and ordering.
    fn monotonic_now(&self) -> Result<MonotonicInstant, Self::Error>;
}

/// Opaque secret-store lifecycle abstraction.
pub trait SecretStorePort: Port {
    /// Captured create request, including a process-local secret input capability.
    type CreateRequest: Send + Sync + 'static;
    /// Bound secret purpose.
    type Purpose: Send + Sync + 'static;
    /// Opaque serializable secret reference; possession does not grant access.
    type SecretRef: Send + Sync + 'static;
    /// Purpose/incarnation-bound plaintext lease capability.
    type SecretLease: PackageOpaque;
    /// Content-free rotation receipt.
    type RotationReceipt: Send + Sync + 'static;
    /// Content-free deletion receipt.
    type DeletionReceipt: Send + Sync + 'static;

    /// Creates a secret under one immutable mutation identity.
    fn create_secret(
        &mut self,
        request: &Self::CreateRequest,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::SecretRef, Self::Error>;

    /// Leases plaintext only inside a finite purpose-bound capability.
    fn lease_secret(
        &self,
        secret_ref: &Self::SecretRef,
        purpose: &Self::Purpose,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SecretLease, Self::Error>;

    /// Rotates the exact referenced secret.
    fn rotate_secret(
        &mut self,
        secret_ref: &Self::SecretRef,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::RotationReceipt, Self::Error>;

    /// Deletes the exact referenced secret without making a physical-erasure claim.
    fn delete_secret(
        &mut self,
        secret_ref: &Self::SecretRef,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::DeletionReceipt, Self::Error>;
}

/// Qualified child-process lifecycle abstraction.
pub trait ProcessSupervisorPort: Port {
    /// Immutable artifact candidate descriptor.
    type ArtifactCandidate: Send + Sync + 'static;
    /// Exact qualified artifact descriptor.
    type QualifiedArtifact: Send + Sync + 'static;
    /// Exact owner fence for the child process.
    type OwnerFence: Send + Sync + 'static;
    /// Optional purpose-bound secret lease supplied to the process.
    type SecretLease: PackageOpaque;
    /// Process guard retaining exact process, executable, and containment identity.
    type ProcessGuard: PackageOpaque;
    /// Content-free exact process-identity receipt.
    type ProcessIdentityReceipt: Send + Sync + 'static;
    /// Truthful readiness state.
    type ProcessReadiness: Send + Sync + 'static;
    /// Closed graceful or forced shutdown mode.
    type ShutdownMode: Send + Sync + 'static;
    /// Content-free shutdown receipt.
    type ShutdownReceipt: Send + Sync + 'static;

    /// Qualifies an exact immutable process artifact.
    fn qualify_artifact(
        &self,
        candidate: &Self::ArtifactCandidate,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::QualifiedArtifact, Self::Error>;

    /// Starts one contained process under an exact owner fence.
    fn start_process(
        &mut self,
        owner_fence: &Self::OwnerFence,
        artifact: &Self::QualifiedArtifact,
        secret_lease: Option<&Self::SecretLease>,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ProcessGuard, Self::Error>;

    /// Revalidates process and executable identity from the guard.
    fn verify_process_identity(
        &self,
        guard: &Self::ProcessGuard,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ProcessIdentityReceipt, Self::Error>;

    /// Reads truthful bounded process readiness.
    fn readiness(
        &self,
        guard: &Self::ProcessGuard,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ProcessReadiness, Self::Error>;

    /// Shuts down the exact guarded process.
    fn shutdown_process(
        &mut self,
        guard: &Self::ProcessGuard,
        mode: &Self::ShutdownMode,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ShutdownReceipt, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::MonotonicInstant;

    #[test]
    fn monotonic_elapsed_rejects_reverse_time() {
        assert_eq!(
            MonotonicInstant::from_ticks(5).checked_elapsed_since(MonotonicInstant::from_ticks(3)),
            Some(2)
        );
        assert_eq!(
            MonotonicInstant::from_ticks(3).checked_elapsed_since(MonotonicInstant::from_ticks(5)),
            None
        );
    }
}
