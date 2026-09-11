//! Atomic revisioned source registry for the W2 direct-source spine.
//!
//! The registry owns admitted roots, sources, memberships, reference
//! portfolios, coherent source/workspace views and source-namespace owner
//! transitions. It persists accepted identity/admission decisions through the
//! vendor-neutral [`error::RegistryControlPort`] and never reimplements those
//! semantics. Canonical product operations enter through [`api`]. The bounded
//! legacy DIRECT journal compatibility surface is re-exported at the crate root;
//! the daemon remains the bounded filesystem integration owner.

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
pub mod membership;
pub mod portfolio;
pub mod recovery;
pub mod root;
pub mod snapshot;
pub mod source;
pub mod view;

#[path = "source/legacy_direct.rs"]
mod source_legacy_direct;

pub use api::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, JournalEntryKind, RegistryControlPort,
    RegistryError, RegistryJournalEntry, RegistryLimits, RegistryPortError,
};
pub use membership::{MembershipKey, MembershipLifecycle, MembershipRecord, NewMembership};
pub use portfolio::PortfolioItem;
pub use recovery::{
    NamespaceCutover, RegistryBatch, RegistryChange, RegistryOperation, RegistryReceipt,
    SourceRegistry,
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
