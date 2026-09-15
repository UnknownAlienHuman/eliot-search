//! Content-free lifecycle receipts and typed mutation outcomes.

use search_contracts::{NonZeroRevision, ReceiptRef};
use search_os_secrets::SecretReference;

/// Content-free provisioning receipt: what was created, never its key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionReceipt {
    /// Opaque bound reference now active.
    pub reference: SecretReference,
    /// Durable record revision (1 on first provisioning).
    pub record_revision: NonZeroRevision,
    /// Vault-readback receipt naming the observed blob digest.
    pub receipt: ReceiptRef,
}

/// Content-free rotation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotationReceipt {
    /// Opaque bound reference after rotation.
    pub reference: SecretReference,
    /// Durable record revision advanced exactly once.
    pub record_revision: NonZeroRevision,
    /// Vault-readback receipt naming the observed replacement digest.
    pub receipt: ReceiptRef,
}

/// Content-free revocation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationReceipt {
    /// Opaque bound reference now durably deleted.
    pub reference: SecretReference,
    /// Final durable record revision.
    pub record_revision: NonZeroRevision,
    /// Absence-readback receipt.
    pub receipt: ReceiptRef,
}

/// How a mutation reached durability: cleanly or through exact recovery.
///
/// Partial/degraded outcomes stay typed here and are never relabeled as
/// plain success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationOutcome<R> {
    /// Platform write confirmed on the first attempt.
    Committed(R),
    /// Platform write was ambiguous; exact readback proved the outcome.
    Recovered(R),
}
