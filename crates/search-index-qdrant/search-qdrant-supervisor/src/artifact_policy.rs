//! Static admission of the exact baseline build, before live capability probing.
//!
//! The caller supplies an actual bounded file observation. Native launch still
//! re-measures the executable and runs its bounded version probe. This policy
//! neither executes probes nor issues process, query or release qualification.

use search_contracts::{
    ArtifactDigest, Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain,
    CanonicalText, CanonicalValue, DigestInputLimit, Sha256Digest32, blake3_canonical,
};

use crate::identity::{QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX, QUALIFIED_QDRANT_VERSION};
use crate::{ArtifactArchitecture, ArtifactCandidate, QualifiedArtifact, SupervisorError};

const BUILD: &str = "74f3e85b";
const TARGET: &str = "x86_64-pc-windows-msvc";
const LICENSE: &str = "Apache-2.0";
// Measured from the official v1.19.0 LICENSE on 2026-10-17. This is the
// compiled baseline license policy, not a caller-supplied qualification receipt.
const LICENSE_SHA256: &str = "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4";
const SOURCE: &str =
    "https://github.com/qdrant/qdrant/releases/download/v1.19.0/qdrant-x86_64-pc-windows-msvc.zip";
const ARCHIVE_SHA256: &str = "980cb2e1ae771155cf211da8c0a8a9206b6482bd4effdc4db994d3adb707b087";

/// Admit an observed executable against the compiled baseline artifact policy.
///
/// `file_identity_digest`, `sha256` and `bytes` must come from the enclosing
/// owner's actual file observation. This pure function does not measure them.
/// `ArtifactDigest` uses the declared executable-SHA256 identity profile; the
/// separate admission digest binds that observation to the complete policy.
/// No qualification manifest, receipt label or future benchmark is consumed.
/// The resulting artifact is a launch input, never proof of a running process.
pub fn admit_pinned_windows_artifact(
    file_identity_digest: Blake3Digest32,
    sha256: Sha256Digest32,
    bytes: u64,
) -> Result<QualifiedArtifact, SupervisorError> {
    let expected = Sha256Digest32::parse_hex(QUALIFIED_EXE_SHA256_HEX)
        .map_err(|_| SupervisorError::InvalidArtifact)?;
    if sha256 != expected || bytes != QUALIFIED_EXE_BYTES {
        return Err(SupervisorError::ArtifactDigestMismatch);
    }
    let value = CanonicalValue::Array(
        BoundedList::new(vec![
            CanonicalValue::U64(1),
            blob(file_identity_digest.as_bytes())?,
            blob(sha256.as_bytes())?,
            CanonicalValue::U64(bytes),
            text(QUALIFIED_QDRANT_VERSION)?,
            text(BUILD)?,
            text(TARGET)?,
            text(LICENSE)?,
            text(LICENSE_SHA256)?,
            text(SOURCE)?,
            text(ARCHIVE_SHA256)?,
        ])
        .map_err(|_| SupervisorError::InvalidArtifact)?,
    );
    let domain = CanonicalDigestDomain::parse("eliot/cbor/qdrant-static-artifact-policy/v1")
        .map_err(|_| SupervisorError::InvalidArtifact)?;
    let limit = DigestInputLimit::new(2048).map_err(|_| SupervisorError::InvalidArtifact)?;
    let qualification_digest =
        blake3_canonical(&domain, &value, limit).map_err(|_| SupervisorError::InvalidArtifact)?;
    Ok(QualifiedArtifact {
        candidate: ArtifactCandidate {
            file_identity_digest,
            sha256,
            artifact_digest: ArtifactDigest::from_bytes(*sha256.as_bytes()),
            version: QUALIFIED_QDRANT_VERSION.into(),
            build_identity: BUILD.into(),
            architecture: ArtifactArchitecture::X86_64Windows,
        },
        qualification_digest,
    })
}

fn text(value: &str) -> Result<CanonicalValue, SupervisorError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| SupervisorError::InvalidArtifact)
}
fn blob(value: &[u8]) -> Result<CanonicalValue, SupervisorError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| SupervisorError::InvalidArtifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_policy_rejects_changed_bytes_before_a_launch_plan_can_exist() {
        let file = Blake3Digest32::from_bytes([1; 32]);
        let expected = Sha256Digest32::parse_hex(QUALIFIED_EXE_SHA256_HEX).unwrap();
        assert_eq!(
            admit_pinned_windows_artifact(file, expected, QUALIFIED_EXE_BYTES - 1),
            Err(SupervisorError::ArtifactDigestMismatch)
        );
        assert_eq!(
            admit_pinned_windows_artifact(
                file,
                Sha256Digest32::from_bytes([0; 32]),
                QUALIFIED_EXE_BYTES
            ),
            Err(SupervisorError::ArtifactDigestMismatch)
        );
        let admitted = admit_pinned_windows_artifact(file, expected, QUALIFIED_EXE_BYTES).unwrap();
        assert_eq!(admitted.candidate().file_identity_digest, file);
        assert_eq!(admitted.candidate().version, QUALIFIED_QDRANT_VERSION);
        assert_eq!(
            admitted.candidate().architecture,
            ArtifactArchitecture::X86_64Windows
        );
    }
}
