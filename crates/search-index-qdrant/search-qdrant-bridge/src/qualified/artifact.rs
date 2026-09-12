//! Exact qualified Qdrant server artifact verification.

use super::{
    QUALIFIED_ARCH, QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX,
    QUALIFIED_OS, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION,
    QualificationError,
};

/// Observed server artifact identity (measured, never assumed).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedArtifact {
    /// Reported server version.
    pub version: String,
    /// Reported server build identity.
    pub build: String,
    /// Executable SHA-256, uppercase hexadecimal.
    pub exe_sha256_hex: String,
    /// Exact executable byte length.
    pub exe_bytes: u64,
    /// Target architecture.
    pub arch: String,
    /// Target operating system.
    pub os: String,
}

/// Accepted server artifact receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactReceipt {
    /// Exact accepted server version.
    pub server_version: String,
    /// Exact accepted server build.
    pub server_build: String,
}

/// Verifies one observed server artifact against the exact qualified identity.
///
/// # Errors
///
/// Returns the first closed mismatch without fallback or partial admission.
pub fn verify_artifact(
    observed: &ObservedArtifact,
) -> Result<ArtifactReceipt, QualificationError> {
    if observed.version != QUALIFIED_SERVER_VERSION {
        return Err(QualificationError::ServerVersionMismatch);
    }
    if observed.build != QUALIFIED_SERVER_BUILD {
        return Err(QualificationError::ServerBuildMismatch);
    }
    if observed.exe_sha256_hex != QUALIFIED_EXE_SHA256_HEX {
        return Err(QualificationError::ArtifactDigestMismatch);
    }
    if observed.exe_bytes != QUALIFIED_EXE_BYTES {
        return Err(QualificationError::ArtifactSizeMismatch);
    }
    if observed.arch != QUALIFIED_ARCH {
        return Err(QualificationError::ArchitectureMismatch);
    }
    if observed.os != QUALIFIED_OS {
        return Err(QualificationError::OsMismatch);
    }
    Ok(ArtifactReceipt {
        server_version: observed.version.clone(),
        server_build: observed.build.clone(),
    })
}
