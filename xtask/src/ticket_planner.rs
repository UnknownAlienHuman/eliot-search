//! Bounded pure helpers for schema-v2 ticket-issuance planning.
//!
//! The public API is preserved while responsibilities are split into closed
//! schema constants, canonical serialization/digests, scalar grammars, path
//! fences, selection decisions, context topology and registry selectors.
//! Immutable Git-tree reads and full planner orchestration remain separate.

mod canonical;
mod context;
mod decision;
mod grammar;
mod path;
mod selectors;
mod spec;

pub use canonical::{
    canonical_json_bytes, exact_sha256_hex, plan_digest, signed_payload_digest,
};
pub use context::{
    context_total_bytes_ok, contract_pack_sources, expected_handoff_slots,
    expected_required_handoffs, line_limits_ok, select_ceiling,
};
pub use decision::{choose_decision, selection_state};
pub use grammar::{
    actor_identity_valid, opaque_id_valid, package_name_valid,
    sha256_hex_valid, tagged_git_valid, unknown_fields,
};
pub use path::{
    advisory_output_path_valid, advisory_output_selectable,
    context_source_forbidden, safe_path, under,
};
pub use selectors::{
    SelectorDocs, SelectorStatus, launch_membership_at_path, one_table,
    resolve_selector,
};
pub use spec::{
    CLOSED_REASON_CODES, CONFLICT_REASONS, CONTEXT_ALLOWED,
    CONTEXT_CANONICALIZATION_FIELDS, CONTEXT_CONTENT_FIELDS,
    CONTEXT_TOTAL_BYTE_CEILING, CONTROL_ROOTS, CURRENT_PACKAGE_RECORD_ROOTS,
    DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING,
    DECISION_PREREQUISITE, DECISION_READY, DOMAIN_SEPARATOR,
    HARD_TOTAL_LINES, INVALID_REASONS, PLAN_ARTIFACT_ROOT,
    PLAN_BYTE_CEILING, PLANNER_FILE_BYTE_CEILING, PREREQUISITE_REASONS,
    RECORD_KIND, REPOSITORY_NAME, ROOT_METADATA_NAMES, SCHEMA_VERSION,
    SPLIT_REVIEW_TOTAL_LINES, STATUS, TICKET_ALLOWED,
    TICKET_CONTEXT_FIELDS, TICKET_DELIVERABLES_FIELDS,
    TICKET_DEPENDENCIES_FIELDS, TICKET_LIMITS_FIELDS,
    TICKET_REPOSITORY_FENCE_FIELDS, TICKET_UNRESOLVED_IDENTITY_FIELDS,
};
