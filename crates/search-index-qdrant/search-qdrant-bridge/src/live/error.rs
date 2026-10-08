use std::fmt;

use crate::qualified::QualificationError;

/// Closed live-path failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveError {
    EndpointNotLoopback,
    ExecutableUnreadable,
    ArtifactDigestMismatch,
    ArtifactSizeMismatch,
    StorageSetupFailed,
    SpawnFailed,
    ServerNotReady,
    AuthenticationNotEnforced,
    TransportFailed,
    ServerVersionUnexpected,
    ServerBuildUnexpected,
    FixtureNotRepresentable,
    ProbeFailed { probe: &'static str },
}

impl LiveError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EndpointNotLoopback => "QDRANT_LIVE_ENDPOINT_NOT_LOOPBACK",
            Self::ExecutableUnreadable => "QDRANT_LIVE_EXECUTABLE_UNREADABLE",
            Self::ArtifactDigestMismatch => QualificationError::ArtifactDigestMismatch.code(),
            Self::ArtifactSizeMismatch => QualificationError::ArtifactSizeMismatch.code(),
            Self::StorageSetupFailed => "QDRANT_LIVE_STORAGE_SETUP_FAILED",
            Self::SpawnFailed => "QDRANT_LIVE_SPAWN_FAILED",
            Self::ServerNotReady => "QDRANT_LIVE_SERVER_NOT_READY",
            Self::AuthenticationNotEnforced => "QDRANT_LIVE_AUTHENTICATION_NOT_ENFORCED",
            Self::TransportFailed => "QDRANT_LIVE_TRANSPORT_FAILED",
            Self::ServerVersionUnexpected => "QDRANT_LIVE_SERVER_VERSION_UNEXPECTED",
            Self::ServerBuildUnexpected => "QDRANT_LIVE_SERVER_BUILD_UNEXPECTED",
            Self::FixtureNotRepresentable => "QDRANT_LIVE_FIXTURE_NOT_REPRESENTABLE",
            Self::ProbeFailed { .. } => "QDRANT_LIVE_PROBE_FAILED",
        }
    }
}

impl fmt::Display for LiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LiveError {}
