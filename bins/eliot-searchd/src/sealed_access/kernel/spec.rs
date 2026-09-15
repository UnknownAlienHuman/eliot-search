//! Closed sealed-access limits, persisted suffixes and error taxonomy.

use core::fmt;

use crate::sealed_access_codec::AccessCodecError;
use crate::sealed_digest::DigestError;
use crate::sealed_root_identity::RootIdentityError;
use crate::sealed_store::SealedStoreError;
use crate::sealed_transaction::SealedTransactionError;

/// Maximum generations in one access-fence chain.
pub const MAX_ACCESS_FENCE_GENERATIONS: usize = 1_000_000;
pub(super) const SEALED_SUFFIX: &str = ".els-dpapi";
pub(super) const SEALED_DIRECTORY: &str = "sealed-revisions";

/// Closed access authority failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedAccessError {
    #[cfg(not(windows))]
    /// The current platform cannot enumerate Windows sealed objects.
    UnsupportedPlatform,
    /// A fence has no generation.
    FenceNotFound,
    /// Current fence is terminally denied.
    AccessDenied,
    /// Source, revision, catalog, scope, or policy identity differs.
    AuthorityBindingMismatch,
    /// Mutation identity was reused with another exact payload.
    MutationConflict,
    /// Generation or access-generation continuity failed.
    GenerationConflict,
    /// Scope, policy, or purge revision regressed.
    RevisionRegression,
    /// A new record followed terminal `DENY`.
    DenyIsTerminal,
    /// Sealed access-fence object inventory is malformed or non-contiguous.
    ChainInvalid,
    /// Required transaction is not terminally committed.
    TransactionNotCommitted,
    /// Finite generation capacity was exhausted.
    CapacityExceeded,
    /// Filesystem inventory observation failed.
    IoFailure,
    /// Strict record codec failed.
    Codec(AccessCodecError),
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// Native physical-root verification failed.
    RootIdentity(RootIdentityError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// Transaction inspection or reconciliation failed.
    Transaction(SealedTransactionError),
}

impl SealedAccessError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "SEALED_ACCESS_UNSUPPORTED_PLATFORM",
            Self::FenceNotFound => "SEALED_ACCESS_FENCE_NOT_FOUND",
            Self::AccessDenied => "SEALED_ACCESS_DENIED",
            Self::AuthorityBindingMismatch => {
                "SEALED_ACCESS_AUTHORITY_BINDING_MISMATCH"
            }
            Self::MutationConflict => "SEALED_ACCESS_MUTATION_CONFLICT",
            Self::GenerationConflict => "SEALED_ACCESS_GENERATION_CONFLICT",
            Self::RevisionRegression => "SEALED_ACCESS_REVISION_REGRESSION",
            Self::DenyIsTerminal => "SEALED_ACCESS_DENY_IS_TERMINAL",
            Self::ChainInvalid => "SEALED_ACCESS_CHAIN_INVALID",
            Self::TransactionNotCommitted => {
                "SEALED_ACCESS_TRANSACTION_NOT_COMMITTED"
            }
            Self::CapacityExceeded => "SEALED_ACCESS_CAPACITY_EXCEEDED",
            Self::IoFailure => "SEALED_ACCESS_IO_FAILURE",
            Self::Codec(error) => error.code(),
            Self::Digest(error) => error.code(),
            Self::RootIdentity(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedAccessError {}

impl From<AccessCodecError> for SealedAccessError {
    fn from(error: AccessCodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<DigestError> for SealedAccessError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<RootIdentityError> for SealedAccessError {
    fn from(error: RootIdentityError) -> Self {
        Self::RootIdentity(error)
    }
}

impl From<SealedStoreError> for SealedAccessError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for SealedAccessError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}
