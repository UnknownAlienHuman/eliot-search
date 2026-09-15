//! Stable transaction dispositions, receipts and inspection models.

use crate::sealed_digest::Sha256Digest;

/// Exact terminal disposition of one idempotent put.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PutDisposition {
    /// Intent and sealed object were created in this invocation.
    Created,
    /// An earlier unknown write was proven by decrypt-and-compare.
    Reconciled,
    /// A complete receipt and exact object were replayed.
    Replay,
}

impl PutDisposition {
    /// Stable wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Reconciled => "RECONCILED",
            Self::Replay => "REPLAY",
        }
    }
}

/// Content-free terminal transaction receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedTransactionReceipt {
    pub operation_id: String,
    pub object_id: String,
    pub plaintext_bytes: u64,
    pub plaintext_sha256: Sha256Digest,
    pub ciphertext_bytes: u64,
    pub disposition: PutDisposition,
    pub sealed_readback_verified: bool,
    pub receipt_readback_verified: bool,
}

/// Durable local transaction status.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransactionStatus {
    /// No intent or receipt exists.
    Absent,
    /// Intent exists and requires exact retry input for reconciliation.
    Prepared,
    /// Terminal receipt exists.
    Committed,
    /// Receipt exists with a matching intent whose cleanup is pending.
    CommittedCleanupPending,
    /// Intent and receipt coexist inconsistently or metadata is contradictory.
    Conflicted,
}

impl TransactionStatus {
    /// Stable wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "ABSENT",
            Self::Prepared => "PREPARED",
            Self::Committed => "COMMITTED",
            Self::CommittedCleanupPending => "COMMITTED_CLEANUP_PENDING",
            Self::Conflicted => "CONFLICTED",
        }
    }
}

/// Exact durable binding observed for one operation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionBinding {
    pub operation_id: String,
    pub object_id: String,
    pub plaintext_bytes: u64,
    pub plaintext_sha256: Sha256Digest,
    pub ciphertext_bytes: Option<u64>,
}

/// Content-free inspection of one operation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionObservation {
    pub status: TransactionStatus,
    pub binding: Option<TransactionBinding>,
}
