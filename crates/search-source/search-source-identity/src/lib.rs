//! Deterministic, I/O-free source, path, repository, and workspace identity.
//!
//! The package consumes already-captured bounded observations. It performs no
//! filesystem read, Git command, content hashing, registry mutation, clock,
//! network, ID generation, admission decision, or source retention.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

pub mod batch;
pub mod binding;
pub mod error;
pub mod evidence;
pub mod lineage;
pub mod redaction;
pub mod resolution;

pub use batch::{
    IdentityBatchControl, IdentityBatchDecision, IdentityBatchEntry, IdentityBatchItem,
    IdentityBatchOutcome, MAX_IDENTITY_BATCH_ITEMS, resolve_batch,
};
pub use binding::{
    BindingCloseReason, OpenBindingRequest, PathBindingEvent, PathBindingRecord,
    PathBindingState, PathBindingTransition, PathHistoryReceipt, MAX_BINDING_HISTORY_ENTRIES,
    MAX_HARDLINK_BINDINGS, open_path_binding, relate_hardlink_bindings,
    transition_path_binding, validate_binding_history,
};
pub use error::IdentityError;
pub use evidence::{
    CanonicalPathKey, CaseBehavior, FilesystemIdentityProfile, IdentityObservation, LinkBehavior,
    MAX_IDENTITY_PATH_BYTES, MissingIdentityEvidence, ObservationConfidence, PathObservation,
    ReparseBehavior, StableFieldPolicy, StableIdentityEvidence, StableIdentityKey,
    UnicodeBehavior, ValidatedIdentityObservation, derive_canonical_path_key,
    validate_identity_observation,
};
pub use lineage::{
    LineageProof, MAX_LINEAGE_CANDIDATES, MAX_REMOTE_FINGERPRINTS, PriorRepositoryLineage,
    ProvenLineageRelation, RepositoryBoundary, RepositoryLineageDecision,
    RepositoryLineageDraft, RepositoryLineageObservation, ValidatedRepositoryObservation,
    WorkspaceIdentityInput, WorkspaceViewFence, advance_workspace_view,
    classify_repository_lineage, derive_workspace_identity, validate_repository_observation,
};
pub use redaction::{
    RedactedIdentityState, RedactedIdentityView, redacted_binding_view,
    redacted_resolution_view,
};
pub use resolution::{
    CreationPolicy, ExistingIdentityEvidence, IdentityDraft, IdentityMatchDecision,
    IdentityResolution, MAX_IDENTITY_CANDIDATES, MAX_PATH_KEYS_PER_CANDIDATE,
    PriorIdentityCandidate, PriorIdentityCandidates, ResolutionGap, ResolutionPolicy,
    compare_identity, derive_source_identity, resolve_identity,
};
