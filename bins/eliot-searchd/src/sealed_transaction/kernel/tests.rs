use std::path::Path;

use super::api::{inspect_transaction, transaction_status};
use super::model::{PutDisposition, TransactionStatus};
use super::spec::SealedTransactionError;

#[test]
fn disposition_wire_values_match_the_stable_contract() {
    assert_eq!(PutDisposition::Created.as_str(), "CREATED");
    assert_eq!(PutDisposition::Reconciled.as_str(), "RECONCILED");
    assert_eq!(PutDisposition::Replay.as_str(), "REPLAY");
}

#[test]
fn status_wire_values_match_the_stable_contract() {
    assert_eq!(TransactionStatus::Absent.as_str(), "ABSENT");
    assert_eq!(TransactionStatus::Prepared.as_str(), "PREPARED");
    assert_eq!(TransactionStatus::Committed.as_str(), "COMMITTED");
    assert_eq!(
        TransactionStatus::CommittedCleanupPending.as_str(),
        "COMMITTED_CLEANUP_PENDING"
    );
    assert_eq!(TransactionStatus::Conflicted.as_str(), "CONFLICTED");
}

#[test]
fn status_and_inspection_reject_a_malformed_operation_identity() {
    let root = Path::new(".");
    let status = transaction_status(root, "")
        .expect_err("empty operation is rejected");
    let observed = inspect_transaction(root, "")
        .expect_err("empty operation is rejected");
    #[cfg(windows)]
    {
        assert_eq!(status, SealedTransactionError::InvalidOperationId);
        assert_eq!(observed, SealedTransactionError::InvalidOperationId);
    }
    #[cfg(not(windows))]
    {
        assert_eq!(status, SealedTransactionError::UnsupportedPlatform);
        assert_eq!(observed, SealedTransactionError::UnsupportedPlatform);
    }
}
