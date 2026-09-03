//! Stable, content-minimized identities used by root ownership.

use core::fmt;
use core::num::{NonZeroU32, NonZeroU64};

use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationId, InstallationIncarnationId,
    OwnerEpoch,
};
use search_ports::MutationIdentity;

use crate::OwnerError;

/// Mutually exclusive daemon composition mode for one data root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeMode {
    /// Search owns its standalone local composition.
    Standalone,
    /// Search is managed by an external ELIOT composition owner.
    Managed,
}

/// Accepted local storage class for a resolved data root.
///
/// Remote shares, device namespaces, and unresolved reparse targets have no
/// representable success variant and must be rejected by the path adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataRootLocationClass {
    /// Stable local fixed storage.
    LocalFixed,
    /// Explicitly accepted local removable storage.
    LocalRemovable,
    /// Explicitly accepted process-local or machine-local memory-backed storage.
    LocalMemoryBacked,
}

/// Content-minimized identity of one canonical local data root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataRootIdentity {
    data_root_id: DataRootId,
    location_class: DataRootLocationClass,
    canonical_path_digest: Blake3Digest32,
    volume_identity_digest: Blake3Digest32,
}

impl DataRootIdentity {
    /// Creates an already-resolved local data-root identity.
    #[must_use]
    pub const fn new(
        data_root_id: DataRootId,
        location_class: DataRootLocationClass,
        canonical_path_digest: Blake3Digest32,
        volume_identity_digest: Blake3Digest32,
    ) -> Self {
        Self {
            data_root_id,
            location_class,
            canonical_path_digest,
            volume_identity_digest,
        }
    }

    /// Stable data-root identifier.
    #[must_use]
    pub const fn data_root_id(self) -> DataRootId {
        self.data_root_id
    }

    /// Accepted local location class.
    #[must_use]
    pub const fn location_class(self) -> DataRootLocationClass {
        self.location_class
    }

    /// Digest of the canonical path spelling; the path itself is not exposed.
    #[must_use]
    pub const fn canonical_path_digest(self) -> Blake3Digest32 {
        self.canonical_path_digest
    }

    /// Stable local volume/file-system identity digest.
    #[must_use]
    pub const fn volume_identity_digest(self) -> Blake3Digest32 {
        self.volume_identity_digest
    }
}

/// Reuse-resistant process creation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProcessCreationIdentity {
    process_id: NonZeroU32,
    creation_marker: NonZeroU64,
    identity_digest: Blake3Digest32,
}

impl ProcessCreationIdentity {
    /// Creates a process identity from an OS process ID, a reuse-resistant
    /// creation marker, and a digest of exact platform identity evidence.
    ///
    /// # Errors
    ///
    /// Zero process IDs or creation markers are rejected.
    pub fn new(
        process_id: u32,
        creation_marker: u64,
        identity_digest: Blake3Digest32,
    ) -> Result<Self, OwnerError> {
        let process_id =
            NonZeroU32::new(process_id).ok_or(OwnerError::OwnerProcessIdentityMismatch)?;
        let creation_marker =
            NonZeroU64::new(creation_marker).ok_or(OwnerError::OwnerProcessIdentityMismatch)?;
        Ok(Self {
            process_id,
            creation_marker,
            identity_digest,
        })
    }

    /// OS process identifier.
    #[must_use]
    pub const fn process_id(self) -> NonZeroU32 {
        self.process_id
    }

    /// Reuse-resistant process creation marker.
    #[must_use]
    pub const fn creation_marker(self) -> NonZeroU64 {
        self.creation_marker
    }

    /// Digest of exact platform process-identity evidence.
    #[must_use]
    pub const fn identity_digest(self) -> Blake3Digest32 {
        self.identity_digest
    }
}

/// Immutable executable identity bound to an owner incarnation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExecutableIdentity {
    artifact_digest: ArtifactDigest,
    file_identity_digest: Blake3Digest32,
}

impl ExecutableIdentity {
    /// Creates an exact executable identity.
    #[must_use]
    pub const fn new(
        artifact_digest: ArtifactDigest,
        file_identity_digest: Blake3Digest32,
    ) -> Self {
        Self {
            artifact_digest,
            file_identity_digest,
        }
    }

    /// Immutable executable artifact digest.
    #[must_use]
    pub const fn artifact_digest(self) -> ArtifactDigest {
        self.artifact_digest
    }

    /// Stable executable file-identity digest.
    #[must_use]
    pub const fn file_identity_digest(self) -> Blake3Digest32 {
        self.file_identity_digest
    }
}

/// Exact identity of one candidate or live owner incarnation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerIdentity {
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    process: ProcessCreationIdentity,
    executable: ExecutableIdentity,
    mode: RuntimeMode,
}

impl OwnerIdentity {
    /// Creates an owner identity from already-qualified components.
    #[must_use]
    pub const fn new(
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        process: ProcessCreationIdentity,
        executable: ExecutableIdentity,
        mode: RuntimeMode,
    ) -> Self {
        Self {
            installation_id,
            installation_incarnation_id,
            process,
            executable,
            mode,
        }
    }

    /// Stable Search installation identifier.
    #[must_use]
    pub const fn installation_id(self) -> InstallationId {
        self.installation_id
    }

    /// Current installation-incarnation identifier.
    #[must_use]
    pub const fn installation_incarnation_id(self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Exact process creation identity.
    #[must_use]
    pub const fn process(self) -> ProcessCreationIdentity {
        self.process
    }

    /// Exact executable identity.
    #[must_use]
    pub const fn executable(self) -> ExecutableIdentity {
        self.executable
    }

    /// Exclusive runtime mode.
    #[must_use]
    pub const fn mode(self) -> RuntimeMode {
        self.mode
    }
}

/// Complete owner fence for one root and epoch.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerBinding {
    root: DataRootIdentity,
    owner: OwnerIdentity,
    epoch: OwnerEpoch,
}

impl OwnerBinding {
    /// Creates a complete owner binding.
    #[must_use]
    pub const fn new(root: DataRootIdentity, owner: OwnerIdentity, epoch: OwnerEpoch) -> Self {
        Self { root, owner, epoch }
    }

    /// Bound data root.
    #[must_use]
    pub const fn root(self) -> DataRootIdentity {
        self.root
    }

    /// Bound owner identity.
    #[must_use]
    pub const fn owner(self) -> OwnerIdentity {
        self.owner
    }

    /// Bound monotone owner epoch.
    #[must_use]
    pub const fn epoch(self) -> OwnerEpoch {
        self.epoch
    }
}

/// Immutable mutation identity plus digest of its canonical request.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerOperation {
    mutation: MutationIdentity,
    request_digest: Blake3Digest32,
}

impl OwnerOperation {
    /// Creates an operation fence.
    #[must_use]
    pub fn new(mutation: MutationIdentity, request_digest: Blake3Digest32) -> Self {
        Self {
            mutation,
            request_digest,
        }
    }

    /// Shared immutable mutation identity.
    #[must_use]
    pub const fn mutation(&self) -> &MutationIdentity {
        &self.mutation
    }

    /// Digest of exact canonical request bytes.
    #[must_use]
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }

    /// Verifies idempotent replay compatibility.
    ///
    /// # Errors
    ///
    /// Reusing an operation identity with another request digest is rejected.
    pub fn verify_replay(&self, other: &Self) -> Result<(), OwnerError> {
        if self.mutation.operation_id == other.mutation.operation_id
            && self.request_digest != other.request_digest
        {
            return Err(OwnerError::OwnerOperationConflict);
        }
        Ok(())
    }

    /// Returns whether both operation identity and canonical request are equal.
    #[must_use]
    pub fn is_same_request(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Debug for OwnerOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerOperation")
            .field("operation_id", &self.mutation.operation_id)
            .field("idempotency", &self.mutation.idempotency)
            .field("request_digest", &self.request_digest)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{Blake3Digest32, OpaqueId};
    use search_ports::{IdempotencyClass, MutationIdentity};

    use super::{OwnerOperation, ProcessCreationIdentity};
    use crate::OwnerError;

    #[test]
    fn process_identity_rejects_reusable_zero_markers() {
        assert_eq!(
            ProcessCreationIdentity::new(0, 1, Blake3Digest32::from_bytes([1; 32])),
            Err(OwnerError::OwnerProcessIdentityMismatch)
        );
        assert_eq!(
            ProcessCreationIdentity::new(1, 0, Blake3Digest32::from_bytes([1; 32])),
            Err(OwnerError::OwnerProcessIdentityMismatch)
        );
    }

    #[test]
    fn operation_reuse_with_another_request_is_rejected() {
        let mutation = MutationIdentity::new(
            OpaqueId::new("owner-operation:one").expect("opaque id"),
            IdempotencyClass::RetrySameIdentity,
        );
        let first = OwnerOperation::new(mutation.clone(), Blake3Digest32::from_bytes([1; 32]));
        let conflict = OwnerOperation::new(mutation, Blake3Digest32::from_bytes([2; 32]));
        assert_eq!(
            first.verify_replay(&conflict),
            Err(OwnerError::OwnerOperationConflict)
        );
    }
}
