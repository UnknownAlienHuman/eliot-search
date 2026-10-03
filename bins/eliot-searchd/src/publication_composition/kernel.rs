//! Publication composition behind the stable module facade.

#[path = "kernel/compensation.rs"]
mod compensation;
#[path = "kernel/guards.rs"]
mod guards;
#[path = "kernel/publisher.rs"]
mod publisher;
#[path = "kernel/recovery.rs"]
mod recovery;
#[path = "kernel/retirement.rs"]
mod retirement;
#[path = "kernel/spec.rs"]
mod spec;

pub use compensation::{
    CompensateError, CompensateMutation, CompensatePointId,
    CompensateReadback, CompensateReceipt, QdrantCompensate,
};
pub use guards::{FakeGuards, LiveGuardRead};
pub use publisher::{ProposeRequest, ProposedCommit, Publisher};
pub use recovery::{RecoveryDecision, RecoveryHead};
pub use retirement::{MembershipFence, RetiredManifest};
pub use spec::{CommitKind, MAX_RETIRED_IDS, PublisherError};

pub use search_contracts::PublicationGuards;
pub use search_control_redb::publication_codec::{
    FileJournal, JournalPersistOutcome, PublicationCodecError,
    PublicationFloor, cas,
};
