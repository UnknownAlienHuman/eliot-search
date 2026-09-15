//! Publication composition behind the stable module facade.

mod compensation;
mod guards;
mod publisher;
mod recovery;
mod retirement;
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
