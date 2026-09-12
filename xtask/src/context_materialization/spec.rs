//! Closed context-materialization schema and decision constants.

/// Materialization plan schema version.
pub const SCHEMA_VERSION: i64 = 1;
/// Plan record kind.
pub const RECORD_KIND: &str = "context_materialization_plan_v1";
/// Plans are advisory and never authoritative.
pub const STATUS: &str = "ADVISORY_NON_AUTHORITATIVE";
/// Sole writable plan output directory.
pub const PLAN_ROOT: &str = "artifacts/context-materialization-plans";
/// Domain separator for `plan_digest` (includes trailing NUL).
pub const PLAN_DOMAIN: &[u8] = b"eliot-search/context-materialization-plan/v1\0";
/// Domain separator for `operation_id` (includes trailing NUL).
pub const OPERATION_DOMAIN: &[u8] = b"eliot-search/materialize-context/v1\0";
/// Prospective manifest instance status.
pub const INSTANCE_STATUS: &str = "MATERIALIZED";
/// Pinned source repository.
pub const REPOSITORY: &str = "UnknownAlienHuman/eliot-search";

/// No external selection input.
pub const DECISION_MISSING: &str = "BLOCKED_MISSING_EXTERNAL_INPUT";
/// Payload ready, signatures absent.
pub const DECISION_SIGNATURES: &str = "READY_FOR_DUAL_SIGNATURE_COLLECTION";
/// Both signatures present, ready for owner readback.
pub const DECISION_COMMIT: &str =
    "READY_FOR_INTEGRATION_OWNER_READBACK_AND_COMMIT";
/// Exactly one signature present.
pub const DECISION_PARTIAL_SIGNATURE: &str =
    "BLOCKED_PARTIAL_SIGNATURE_SET";

/// Selection-input reason code.
pub const REASON_MISSING_SELECTION: &str = "MATERIALIZATION_SELECTION_MISSING";
/// Partial-signature reason code.
pub const REASON_PARTIAL_SIGNATURE: &str =
    "MATERIALIZATION_SIGNATURE_SET_PARTIAL";

/// Authority ceiling fields (order-pinned, always `false`).
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
