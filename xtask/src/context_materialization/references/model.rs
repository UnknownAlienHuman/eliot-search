//! Normalized materialization reference values.

/// Immutable artifact reference bound to exact candidate bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Store profile reference.
    pub store_profile_ref: String,
    /// Artifact identifier.
    pub artifact_id: String,
    /// Exact byte length.
    pub bytes: u64,
    /// Exact SHA-256 hex digest.
    pub sha256: String,
}

/// Normalized present signature value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureValue {
    /// Approval profile reference.
    pub approval_profile_ref: String,
    /// Approval artifact reference.
    pub approval_artifact_ref: ArtifactRef,
    /// Digest of the signed payload.
    pub signed_payload_sha256: String,
    /// Signing actor.
    pub actor_identity: String,
}

/// Normalized optional signature reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSignature {
    /// `ABSENT` or `PRESENT`.
    pub state: String,
    /// `None` for `ABSENT`, normalized value for `PRESENT`.
    pub value: Option<SignatureValue>,
}
