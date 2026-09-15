use super::*;

#[test]
fn recovery_issue_codes_match_the_stable_contract() {
    assert_eq!(
        RecoveryIssueCode::PreparedObjectMissing.as_str(),
        "PREPARED_OBJECT_MISSING"
    );
    assert_eq!(
        RecoveryIssueCode::CommittedObjectMissing.as_str(),
        "COMMITTED_OBJECT_MISSING"
    );
    assert_eq!(
        RecoveryIssueCode::TransactionConflict.as_str(),
        "TRANSACTION_CONFLICT"
    );
    assert_eq!(
        RecoveryIssueCode::PlaintextLengthMismatch.as_str(),
        "PLAINTEXT_LENGTH_MISMATCH"
    );
    assert_eq!(
        RecoveryIssueCode::CiphertextLengthMismatch.as_str(),
        "CIPHERTEXT_LENGTH_MISMATCH"
    );
    assert_eq!(
        RecoveryIssueCode::PlaintextDigestMismatch.as_str(),
        "PLAINTEXT_DIGEST_MISMATCH"
    );
}

#[test]
fn issue_report_is_bounded_and_fail_closed() {
    let mut report = SealedRecoveryReport::new(7);
    for index in 0..=MAX_RECOVERY_ISSUES {
        report.issue(
            format!("operation-{index}"),
            RecoveryIssueCode::TransactionConflict,
        );
    }
    assert_eq!(report.owner_epoch, 7);
    assert_eq!(report.issues.len(), MAX_RECOVERY_ISSUES);
    assert_eq!(report.omitted_issue_count, 1);
    assert!(!report.ready);
}
