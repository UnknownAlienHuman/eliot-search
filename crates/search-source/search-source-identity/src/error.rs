//! Closed, content-free source-identity failures.

use core::fmt;

/// Typed failure returned by source identity, binding, and lineage decisions.
///
/// Variants carry no unrestricted path, remote URL, source body, or foreign
/// workspace detail.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityError {
    /// Filesystem identity behavior is unsupported or incompletely specified.
    FilesystemProfileUnsupported,
    /// A source identity observation is malformed or internally contradictory.
    IdentityObservationInvalid,
    /// Stable evidence is insufficient to match or create a durable identity.
    SourceIdentityInsufficientEvidence,
    /// More than one prior source remains materially plausible.
    SourceIdentityAmbiguous,
    /// One exact stable identity key maps to multiple source identities.
    SourceIdentityCollision,
    /// A claimed source identity conflicts with exact stable evidence.
    SourceIdentityConflict,
    /// A path binding conflicts with another active source.
    PathBindingConflict,
    /// Binding history has overlapping intervals, revision regression, or gaps.
    PathBindingHistoryInvalid,
    /// A path observation escapes its admitted root.
    PathEscapesAdmittedRoot,
    /// Reused path text requires a fresh identity resolution.
    PathReuseRequiresNewResolution,
    /// Hard-link grouping lacks exact accepted physical identity evidence.
    HardlinkIdentityUnproved,
    /// Repository lineage evidence remains ambiguous.
    LineageIdentityAmbiguous,
    /// Nested repository, worktree, or submodule boundaries conflict.
    RepositoryBoundaryConflict,
    /// Workspace identity or view-fence input is invalid.
    WorkspaceIdentityInvalid,
    /// A catalog or workspace revision is zero, stale, skipped, or exhausted.
    IdentityRevisionInvalid,
    /// A finite collection or byte ceiling was exceeded.
    IdentityCapacityExceeded,
    /// The configured comparison budget was exhausted.
    IdentityBudgetExhausted,
    /// Cancellation was observed before completing the decision.
    IdentityCancelled,
    /// A required mutation or lineage authorization is absent.
    IdentityAuthorizationRequired,
    /// Exact post-transition readback evidence is absent.
    IdentityReadbackRequired,
    /// A shared contract value could not be constructed or advanced.
    ContractExhausted,
}

impl IdentityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::FilesystemProfileUnsupported => "FILESYSTEM_PROFILE_UNSUPPORTED",
            Self::IdentityObservationInvalid => "IDENTITY_OBSERVATION_INVALID",
            Self::SourceIdentityInsufficientEvidence => "SOURCE_IDENTITY_INSUFFICIENT_EVIDENCE",
            Self::SourceIdentityAmbiguous => "SOURCE_IDENTITY_AMBIGUOUS",
            Self::SourceIdentityCollision => "SOURCE_IDENTITY_COLLISION",
            Self::SourceIdentityConflict => "SOURCE_IDENTITY_CONFLICT",
            Self::PathBindingConflict => "PATH_BINDING_CONFLICT",
            Self::PathBindingHistoryInvalid => "PATH_BINDING_HISTORY_INVALID",
            Self::PathEscapesAdmittedRoot => "PATH_ESCAPES_ADMITTED_ROOT",
            Self::PathReuseRequiresNewResolution => "PATH_REUSE_REQUIRES_NEW_RESOLUTION",
            Self::HardlinkIdentityUnproved => "HARDLINK_IDENTITY_UNPROVED",
            Self::LineageIdentityAmbiguous => "LINEAGE_IDENTITY_AMBIGUOUS",
            Self::RepositoryBoundaryConflict => "REPOSITORY_BOUNDARY_CONFLICT",
            Self::WorkspaceIdentityInvalid => "WORKSPACE_IDENTITY_INVALID",
            Self::IdentityRevisionInvalid => "IDENTITY_REVISION_INVALID",
            Self::IdentityCapacityExceeded => "IDENTITY_CAPACITY_EXCEEDED",
            Self::IdentityBudgetExhausted => "IDENTITY_BUDGET_EXHAUSTED",
            Self::IdentityCancelled => "IDENTITY_CANCELLED",
            Self::IdentityAuthorizationRequired => "IDENTITY_AUTHORIZATION_REQUIRED",
            Self::IdentityReadbackRequired => "IDENTITY_READBACK_REQUIRED",
            Self::ContractExhausted => "IDENTITY_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for IdentityError {}
