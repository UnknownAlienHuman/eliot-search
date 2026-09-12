//! Stable closed qualification errors.

use core::fmt;

/// Closed qualification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationError {
    /// Observed server version differs from the qualified version.
    ServerVersionMismatch,
    /// Observed server build differs from the qualified build.
    ServerBuildMismatch,
    /// Executable digest differs from the qualified artifact.
    ArtifactDigestMismatch,
    /// Executable length differs from the qualified artifact.
    ArtifactSizeMismatch,
    /// Executable architecture differs from the qualified target.
    ArchitectureMismatch,
    /// Executable operating system differs from the qualified target.
    OsMismatch,
    /// Client crate identity differs from `qdrant-client`.
    ClientCrateMismatch,
    /// Client version differs from the exact qualified version.
    ClientVersionMismatch,
    /// Client source checksum differs from the exact qualified source.
    ClientChecksumMismatch,
    /// Local IDF contribution was supplied next to Qdrant-side IDF.
    DoubleIdf,
    /// Sparse vector is not configured with the Qdrant IDF modifier.
    IdfModifierMissing,
    /// Retrieval and IDF corpus eligibility plans differ.
    CorpusEligibilityDiverged,
    /// Sparse vector name is empty.
    InvalidVectorName,
    /// A mandatory live probe is missing or did not pass.
    MandatoryProbeFailed,
    /// Live backend reports a different server/client identity.
    LiveIdentityMismatch,
}

impl QualificationError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ServerVersionMismatch => "QDRANT_QUAL_SERVER_VERSION_MISMATCH",
            Self::ServerBuildMismatch => "QDRANT_QUAL_SERVER_BUILD_MISMATCH",
            Self::ArtifactDigestMismatch => "QDRANT_QUAL_ARTIFACT_DIGEST_MISMATCH",
            Self::ArtifactSizeMismatch => "QDRANT_QUAL_ARTIFACT_SIZE_MISMATCH",
            Self::ArchitectureMismatch => "QDRANT_QUAL_ARCHITECTURE_MISMATCH",
            Self::OsMismatch => "QDRANT_QUAL_OS_MISMATCH",
            Self::ClientCrateMismatch => "QDRANT_QUAL_CLIENT_CRATE_MISMATCH",
            Self::ClientVersionMismatch => "QDRANT_QUAL_CLIENT_VERSION_MISMATCH",
            Self::ClientChecksumMismatch => "QDRANT_QUAL_CLIENT_CHECKSUM_MISMATCH",
            Self::DoubleIdf => "QDRANT_QUAL_DOUBLE_IDF",
            Self::IdfModifierMissing => "QDRANT_QUAL_IDF_MODIFIER_MISSING",
            Self::CorpusEligibilityDiverged => "QDRANT_QUAL_CORPUS_ELIGIBILITY_DIVERGED",
            Self::InvalidVectorName => "QDRANT_QUAL_INVALID_VECTOR_NAME",
            Self::MandatoryProbeFailed => "QDRANT_QUAL_MANDATORY_PROBE_FAILED",
            Self::LiveIdentityMismatch => "QDRANT_QUAL_LIVE_IDENTITY_MISMATCH",
        }
    }
}

impl fmt::Display for QualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for QualificationError {}
