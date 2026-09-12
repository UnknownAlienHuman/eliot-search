//! Closed schema, reason and field registries for ticket issuance planning.

/// Schema-v2 planner record version.
pub const SCHEMA_VERSION: i64 = 2;
/// Advisory plan record kind.
pub const RECORD_KIND: &str = "ticket_issuance_plan_v2";
/// Plans are advisory and non-authoritative.
pub const STATUS: &str = "ADVISORY_NON_AUTHORITATIVE";
/// Domain separator prefixed before canonical plan bytes.
pub const DOMAIN_SEPARATOR: &[u8] = b"eliot-search/ticket-issuance-plan/v2\0";
/// Sole writable advisory artifact directory.
pub const PLAN_ARTIFACT_ROOT: &str = "artifacts/ticket-issuance-plans";
/// Expected repository identity in draft fences.
pub const REPOSITORY_NAME: &str = "UnknownAlienHuman/eliot-search";

/// Full selection with no blocking reasons.
pub const DECISION_READY: &str = "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW";
/// No issuance identity selected.
pub const DECISION_MISSING: &str = "BLOCKED_MISSING_SELECTION";
/// Accepted-handoff prerequisite unsatisfied.
pub const DECISION_PREREQUISITE: &str = "BLOCKED_PREREQUISITE";
/// Conflicting or partial selection.
pub const DECISION_CONFLICT: &str = "BLOCKED_CONFLICT";
/// Repository state is structurally invalid.
pub const DECISION_INVALID: &str = "INVALID_REPOSITORY_STATE";

/// Control-record roots that must stay empty before issuance.
pub const CONTROL_ROOTS: [&str; 8] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
];
/// Roots scanned for current-package control records.
pub const CURRENT_PACKAGE_RECORD_ROOTS: [&str; 6] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
];
/// Exact root metadata filenames.
pub const ROOT_METADATA_NAMES: [&str; 2] = ["README.md", ".gitkeep"];

/// Exact closed machine reason registry, order-pinned.
pub const CLOSED_REASON_CODES: [&str; 32] = [
    "GIT_REPOSITORY_INVALID",
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_UNKNOWN_FIELD",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "HANDOFF_SLOT_UNSATISFIED",
    "HANDOFF_SET_UNEXPECTED",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "PARTIAL_ISSUANCE_SELECTION",
    "BASE_COMMIT_INVALID",
    "ACTOR_IDENTITY_INVALID",
    "WRITER_REVIEWER_CONFLICT",
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "W0_ALREADY_ACCEPTED",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "OUTPUT_WRITE_FAILED",
];
/// Reasons forcing `INVALID_REPOSITORY_STATE`.
pub const INVALID_REASONS: [&str; 25] = [
    "ACTOR_IDENTITY_INVALID",
    "BASE_COMMIT_INVALID",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTROL_SCHEMA_MISMATCH",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_UNKNOWN_FIELD",
    "GIT_REPOSITORY_INVALID",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "PACKAGE_REGISTRY_MISMATCH",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_UNKNOWN",
    "WORKFLOW_POLICY_VIOLATION",
];
/// Reasons forcing `BLOCKED_CONFLICT`.
pub const CONFLICT_REASONS: [&str; 5] = [
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "HANDOFF_SET_UNEXPECTED",
    "PARTIAL_ISSUANCE_SELECTION",
    "W0_ALREADY_ACCEPTED",
    "WRITER_REVIEWER_CONFLICT",
];
/// Reasons forcing `BLOCKED_PREREQUISITE`.
pub const PREREQUISITE_REASONS: [&str; 1] = ["HANDOFF_SLOT_UNSATISFIED"];

/// Closed top-level ticket draft fields.
pub const TICKET_ALLOWED: [&str; 20] = [
    "authorizes_implementation",
    "claimable",
    "context",
    "creates_lease",
    "deliverables",
    "dependencies",
    "issuance_status",
    "launch_class",
    "launch_precondition",
    "limits",
    "may_be_writer_acknowledged",
    "package",
    "phase",
    "record_kind",
    "repository_fence",
    "schema_version",
    "stage",
    "status",
    "unresolved_identity",
    "wave",
];
/// Closed top-level context draft fields.
pub const CONTEXT_ALLOWED: [&str; 21] = [
    "accepted_handoff_slot_count",
    "authorizes_implementation",
    "base_commit",
    "canonicalization",
    "claimable",
    "content",
    "materialization_mode",
    "materialized_context_artifact_ref",
    "materialized_context_artifact_sha256",
    "materialized_context_manifest_ref",
    "materialized_context_record_sha256",
    "package",
    "phase",
    "record_kind",
    "registry_fragment_count",
    "schema_version",
    "source_file_count",
    "stage",
    "status",
    "wave",
    "writer_visible_artifact_count",
];

/// Closed `unresolved_identity` fields.
pub const TICKET_UNRESOLVED_IDENTITY_FIELDS: [&str; 9] = [
    "base_commit",
    "branch_or_worktree",
    "integration_signature_ref",
    "issued_at",
    "reviewer",
    "ticket_exact_record_file_sha256",
    "ticket_id",
    "ticket_signed_payload_sha256",
    "writer",
];
/// Closed `repository_fence` fields.
pub const TICKET_REPOSITORY_FENCE_FIELDS: [&str; 8] = [
    "feature_profile",
    "function_registry_path",
    "launch_state_path",
    "package_registry_path",
    "registry_digests",
    "repository",
    "stage_registry_path",
    "write_scope",
];
/// Closed ticket `context` fields.
pub const TICKET_CONTEXT_FIELDS: [&str; 6] = [
    "architecture_access",
    "context_artifact_ref",
    "context_artifact_sha256",
    "context_draft",
    "context_manifest_ref",
    "writer_visible_artifact_count",
];
/// Closed ticket `dependencies` fields.
pub const TICKET_DEPENDENCIES_FIELDS: [&str; 5] = [
    "accepted_handoff_refs",
    "required_contract_api_schema_digest",
    "required_contract_commit",
    "required_handoff_packages",
    "status",
];
/// Closed ticket `limits` fields.
pub const TICKET_LIMITS_FIELDS: [&str; 4] = [
    "hard_total_lines",
    "one_active_writer",
    "soft_src_lines",
    "split_review_total_lines",
];
/// Closed ticket `deliverables` fields.
pub const TICKET_DELIVERABLES_FIELDS: [&str; 3] = [
    "issuance_requirements",
    "required_evidence",
    "required_outputs",
];
/// Closed context `canonicalization` fields.
pub const CONTEXT_CANONICALIZATION_FIELDS: [&str; 7] = [
    "encoding",
    "line_endings",
    "path_header_format",
    "preserve_declared_order",
    "record_fragment_sha256",
    "record_source_sha256",
    "registry_header_format",
];
/// Closed context `content` fields.
pub const CONTEXT_CONTENT_FIELDS: [&str; 5] = [
    "accepted_handoff_slots",
    "forbidden_paths",
    "registry_fragments",
    "required_unavailable_checks",
    "source_files",
];
/// Split-review total-line budget.
pub const SPLIT_REVIEW_TOTAL_LINES: i64 = 8500;
/// Hard total-line budget.
pub const HARD_TOTAL_LINES: i64 = 10_000;
/// Per-file planner read ceiling.
pub const PLANNER_FILE_BYTE_CEILING: u64 = 4 * 1024 * 1024;
/// Declared-context total byte ceiling.
pub const CONTEXT_TOTAL_BYTE_CEILING: u64 = 16 * 1024 * 1024;
/// Canonical plan artifact byte ceiling.
pub const PLAN_BYTE_CEILING: usize = 262_144;
