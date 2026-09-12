//! Context-materialization planner state and normalized external selection.

use serde_json::Value;

use crate::context_materialization::{
    ArtifactRef, OptionalSignature,
};

/// Fully assembled non-authoritative materialization plan.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterializationBuild {
    plan: Value,
    plan_bytes: Vec<u8>,
    payload_bytes: Option<Vec<u8>>,
    manifest_bytes: Option<Vec<u8>>,
    output_directory: String,
}

impl MaterializationBuild {
    pub(super) fn new(
        plan: Value,
        plan_bytes: Vec<u8>,
        payload_bytes: Option<Vec<u8>>,
        manifest_bytes: Option<Vec<u8>>,
        output_directory: String,
    ) -> Self {
        Self {
            plan,
            plan_bytes,
            payload_bytes,
            manifest_bytes,
            output_directory,
        }
    }

    /// Canonical plan value.
    #[must_use]
    pub fn plan(&self) -> &Value {
        &self.plan
    }

    /// Exact canonical JSON plan bytes.
    #[must_use]
    pub fn plan_bytes(&self) -> &[u8] {
        &self.plan_bytes
    }

    /// Exact prospective manifest bytes before the signature table.
    #[must_use]
    pub fn payload_bytes(&self) -> Option<&[u8]> {
        self.payload_bytes.as_deref()
    }

    /// Complete prospective manifest bytes when both signatures are present.
    #[must_use]
    pub fn manifest_bytes(&self) -> Option<&[u8]> {
        self.manifest_bytes.as_deref()
    }

    /// Repository-relative ordinary artifact directory.
    #[must_use]
    pub fn output_directory(&self) -> &str {
        &self.output_directory
    }
}

/// Normalized external materialization selection.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Selection {
    pub(super) context_id: String,
    pub(super) created_at: String,
    pub(super) materializer_identity: String,
    pub(super) reviewer_identity: String,
    pub(super) artifact_ref: ArtifactRef,
    pub(super) readback: ArtifactReadback,
    pub(super) materializer_signature: OptionalSignature,
    pub(super) reviewer_signature: OptionalSignature,
}

/// Exact immutable-artifact readback declaration.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ArtifactReadback {
    pub(super) verifier_identity: String,
    pub(super) verified_at: String,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}

/// Candidate metadata, bundle and parsed semantic blocks.
#[derive(Clone, Debug)]
pub(super) struct CandidateInput {
    pub(super) candidate_path: String,
    pub(super) bundle_path: String,
    pub(super) candidate: Value,
    pub(super) bundle: Vec<u8>,
    pub(super) preamble: Value,
    pub(super) blocks: Vec<crate::context_artifact::BundleBlock>,
}
