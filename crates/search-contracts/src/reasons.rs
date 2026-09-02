use crate::{ContractError, ContractErrorKind};

macro_rules! closed_code_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Result<Self, ContractError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ContractError::new(
                        ContractErrorKind::InvalidCharacter,
                        stringify!($name),
                    )),
                }
            }
        }
    };
}

closed_code_enum!(SearchReasonCodeV1 {
    QdrantUnavailable => "QDRANT_UNAVAILABLE",
    QdrantCapabilityMismatch => "QDRANT_CAPABILITY_MISMATCH",
    CollectionSchemaMismatch => "COLLECTION_SCHEMA_MISMATCH",
    PublicationBlocked => "PUBLICATION_BLOCKED",
    PublicationReadbackMismatch => "PUBLICATION_READBACK_MISMATCH",
    PointIdCollision => "POINT_ID_COLLISION",
    SourceUnstable => "SOURCE_UNSTABLE",
    ObservationGap => "OBSERVATION_GAP",
    SourceRevisionUnavailable => "SOURCE_REVISION_UNAVAILABLE",
    ScopeChangedOrRevisionUnavailable => "SCOPE_CHANGED_OR_REVISION_UNAVAILABLE",
    ReferenceScopeEmpty => "REFERENCE_SCOPE_EMPTY",
    AmbiguousSubject => "AMBIGUOUS_SUBJECT",
    UnsavedBufferUnobserved => "UNSAVED_BUFFER_UNOBSERVED",
    UnsavedSnapshotNotAdmitted => "UNSAVED_SNAPSHOT_NOT_ADMITTED",
    SourceNamespaceOwnershipConflict => "SOURCE_NAMESPACE_OWNERSHIP_CONFLICT",
    SourceOwnerCutoverRequired => "SOURCE_OWNER_CUTOVER_REQUIRED",
    ResidencyDomainMismatch => "RESIDENCY_DOMAIN_MISMATCH",
    ClientAdapterAuthorityViolation => "CLIENT_ADAPTER_AUTHORITY_VIOLATION",
    AccessRevoked => "ACCESS_REVOKED",
    Purged => "PURGED",
    SnapshotExpired => "SNAPSHOT_EXPIRED",
    IndexGap => "INDEX_GAP",
    IncompleteCoverage => "INCOMPLETE_COVERAGE",
    MaterializationLoss => "MATERIALIZATION_LOSS",
    ControlStoreCorrupt => "CONTROL_STORE_CORRUPT",
    RestorePendingRevalidation => "RESTORE_PENDING_REVALIDATION",
    ResourceExhausted => "RESOURCE_EXHAUSTED",
    Cancelled => "CANCELLED",
    SecurityFailClosed => "SECURITY_FAIL_CLOSED",
    Stale => "STALE",
    Unreadable => "UNREADABLE",
});

impl SearchReasonCodeV1 {
    /// Gap-only reasons cannot label emitted evidence candidates.
    #[must_use]
    pub const fn is_candidate_forbidden(self) -> bool {
        matches!(
            self,
            Self::Stale
                | Self::Unreadable
                | Self::AccessRevoked
                | Self::Purged
                | Self::SourceRevisionUnavailable
        )
    }
}

closed_code_enum!(ProtocolErrorCode {
    ProtocolVersionMismatch => "PROTOCOL_VERSION_MISMATCH",
    FrameTooLarge => "FRAME_TOO_LARGE",
    InvalidEnvelope => "INVALID_ENVELOPE",
    ReplayDetected => "REPLAY_DETECTED",
    AuthFailed => "AUTH_FAILED",
    BindingMismatch => "BINDING_MISMATCH",
    SequenceGap => "SEQUENCE_GAP",
    InFlightLimitExceeded => "IN_FLIGHT_LIMIT_EXCEEDED",
    DeadlineExpired => "DEADLINE_EXPIRED",
    UnsupportedMessageKind => "UNSUPPORTED_MESSAGE_KIND",
});
