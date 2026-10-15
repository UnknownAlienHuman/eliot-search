//! Atomic revisioned source registry for the W2 direct-source spine.
//!
//! The registry owns admitted roots, sources, memberships, reference
//! portfolios, coherent source/workspace views and source-namespace owner
//! transitions. It persists accepted identity/admission decisions through the
//! vendor-neutral [`error::RegistryControlPort`] and never reimplements those
//! semantics. Canonical product operations enter through [`api`]. The bounded
//! legacy DIRECT journal, source-root catalog and root-currentness compatibility
//! surfaces are re-exported at the crate root; the daemon remains the bounded
//! filesystem integration owner.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

pub mod api;
pub mod cutover;
pub mod error;
pub use search_source_records::legacy_root_catalog;
pub mod membership;
pub mod portfolio;
pub use search_source_records::portfolio_history;
pub mod recovery;
pub use search_source_history::revision;
pub mod root;
pub use search_source_records::root_currentness;
pub mod snapshot;
pub mod source;
pub mod view;

use search_source_records::legacy_direct as source_legacy_direct;

pub use api::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, JournalEntryKind, RegistryControlPort,
    RegistryError, RegistryJournalEntry, RegistryLimits, RegistryPortError,
};
pub use legacy_root_catalog::{
    LEGACY_SOURCE_ROOT_CATALOG_HEADER, LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES,
    LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS, LegacySourceRootCatalogError,
    decode_legacy_source_root_catalog, encode_legacy_source_root_catalog,
};
pub use membership::{MembershipKey, MembershipLifecycle, MembershipRecord, NewMembership};
pub use portfolio::PortfolioItem;
pub use portfolio_history::{
    ImmutablePortfolioRevision, PortfolioCurrentPointer, PortfolioHistory, PortfolioRevisionKey,
    PortfolioRevisionRecord,
};
pub use recovery::{
    MAX_REGISTRY_RESTORE_BYTES, NamespaceCutover, ObservedRegistryRestore, RegistryBatch,
    RegistryChange, RegistryOperation, RegistryReceipt, RegistryRestoreOperation,
    RegistryRestoreState, SourceRegistry, preflight_registry_restore_inputs,
    restore_registry_state,
};
pub use revision::{
    SourceRevisionControlPort, SourceRevisionCurrentness, SourceRevisionHead, SourceRevisionIdPort,
    SourceRevisionMutation, SourceRevisionMutationReadback, SourceRevisionRegistrationReceipt,
    SourceRevisionRegistrationRequest, SourceRevisionSourceState,
    decode_source_revision_mutation_readback, decode_source_revision_record,
    encode_source_revision_mutation_readback, encode_source_revision_record,
    register_source_revision,
};
pub use root_currentness::{
    CurrentWorkspaceTruth, MAX_SOURCE_ROOT_OBSERVATION_GAPS, MAX_SOURCE_ROOT_WATCHER_HINTS,
    ObservationGap, ObservationGapReason, ReconciliationCursor, SourceRootCurrentness,
    SourceRootCurrentnessError, SourceRootState, WatcherHint, WatcherHintKind,
};
pub use source::{AdmissionBindingProof, RegisteredSource, SourceLifecycle};
pub use source_legacy_direct::{
    LEGACY_DIRECT_LOG_HEADER, LEGACY_DIRECT_ZERO_DIGEST, LegacyDirectAppendPlan,
    LegacyDirectDigest, LegacyDirectIdentityStrength, LegacyDirectJournalError,
    LegacyDirectRecordDraft, LegacyDirectRegistryState, LegacyDirectSourceRecord,
    LegacyDirectSourceState, verify_legacy_direct_revision_identity,
};

#[cfg(test)]
mod tests;
