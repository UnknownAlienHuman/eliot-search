//! Closed context-artifact candidate schema constants.

/// Candidate metadata schema version.
pub const SCHEMA_VERSION: i64 = 1;
/// Candidate metadata record kind.
pub const RECORD_KIND: &str = "context_artifact_candidate_v1";
/// Candidates are never stored or signed by the builder.
pub const STATUS: &str = "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED";
/// Bundle magic format token.
pub const ARTIFACT_FORMAT: &str = "ELIOT_SWARM_CONTEXT_1";
/// Sole writable candidate artifact directory.
pub const ARTIFACT_ROOT: &str = "artifacts/context-artifact-candidates";
/// Bundle magic prefix, including terminal LF.
pub const BUNDLE_MAGIC: &[u8] = b"ELIOT_SWARM_CONTEXT_1\n";
/// Bundle end marker, including terminal LF.
pub const BUNDLE_END: &[u8] = b"--- end-context-artifact ---\n";
/// Domain separator for `candidate_id`.
pub const CANDIDATE_ID_DOMAIN: &[u8] =
    b"eliot-search/context-artifact-candidate/v1\0";
/// Domain separator for candidate metadata digest.
pub const CANDIDATE_METADATA_DOMAIN: &[u8] =
    b"eliot-search/context-artifact-candidate-metadata/v1\0";
/// Maximum bundle size, 20 MiB.
pub const MAX_BUNDLE_BYTES: usize = 20 * 1024 * 1024;

/// Additional failure codes registered by the candidate schema.
pub const ADDITIONAL_FAILURE_CODES: [&str; 9] = [
    "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
    "CONTEXT_SOURCE_CONTAINS_NUL",
    "REGISTRY_FRAGMENT_NONCANONICAL",
    "BUNDLE_FORMAT_INVALID",
    "BUNDLE_SIZE_EXCEEDED",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "CANDIDATE_OUTPUT_CONFLICT",
    "CANDIDATE_OUTPUT_WRITE_FAILED",
];

/// Authority ceiling fields, order-pinned and always false.
pub const AUTHORITY_FIELDS: [&str; 9] = [
    "materializes_authoritative_context",
    "creates_context_manifest_record",
    "creates_immutable_artifact_ref",
    "creates_assignment_ticket",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "accepts_gate_or_wave",
    "advances_launch_state",
];

/// Unresolved prospective context-manifest fields, order-pinned.
pub const UNRESOLVED_MANIFEST_FIELDS: [&str; 14] = [
    "identity.context_id",
    "identity.operation_id",
    "artifact.ref",
    "verification.readback_verified",
    "signature.created_at",
    "signature.materializer_identity",
    "signature.reviewer_identity",
    "signature.record_sha256",
    "signature.materializer_signature_ref",
    "signature.reviewer_signature_ref",
    "record_path.context_record_sha256",
    "record_path.git_commit",
    "record_path.git_blob_id",
    "record_path.exact_record_file_sha256",
];
