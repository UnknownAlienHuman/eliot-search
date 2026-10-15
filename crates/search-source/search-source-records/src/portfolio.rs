//! Immutable portfolio request, result and current-view projection values.
use crate::membership::MembershipRecord;
use crate::portfolio_history::PortfolioRevisionKey;
use crate::source::RegisteredSource;
use search_contracts::{OpaqueId, ReceiptRef, ReferencePortfolioId, SourceMembershipId};
/// One active admitted portfolio item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioItem {
    /// Stable source record.
    pub source: RegisteredSource,
    /// Active membership record.
    pub membership: MembershipRecord,
}

/// Legacy reference portfolio record retained for the existing current view.
///
/// Storage authority is the immutable history in [`PortfolioHistory`]. This
/// record is a derived projection of the revision named by the current
/// pointer and is never written to independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencePortfolioRecord {
    /// Stable portfolio identity.
    pub portfolio_id: ReferencePortfolioId,
    /// Accepted portfolio revision.
    pub portfolio_revision: search_contracts::PortfolioRevision,
    /// Ordered membership precedence.
    pub membership_precedence: Vec<SourceMembershipId>,
    /// Registry revision that last changed this record.
    pub registry_revision: u64,
    /// Content-free receipt of the last portfolio mutation.
    pub last_receipt: ReceiptRef,
}

/// Explicit portfolio publication request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishPortfolioRequest {
    /// Stable portfolio identity.
    pub portfolio_id: ReferencePortfolioId,
    /// Exact portfolio revision to publish.
    pub portfolio_revision: search_contracts::PortfolioRevision,
    /// Exact current target observed by the proposer; None requires absence.
    /// This guard applies to new publication, not a historical replay.
    pub expected_current: Option<PortfolioRevisionKey>,
    /// Explicit ordered membership precedence.
    pub membership_precedence: Vec<SourceMembershipId>,
    /// Whether an explicitly empty scope is permitted.
    pub allow_empty: bool,
}

/// Content-free portfolio publication receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioReceipt {
    /// Stable portfolio identity.
    pub portfolio_id: ReferencePortfolioId,
    /// Portfolio revision after commit.
    pub portfolio_revision: search_contracts::PortfolioRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Operation that originally committed the returned publication.
    /// Identical-content reuse returns that original identity, not a new acknowledgment.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this returns an already committed immutable publication.
    pub replayed: bool,
}
