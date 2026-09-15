//! Bounded content-free recovery report accounting.

use super::spec::{MAX_RECOVERY_ISSUES, RecoveryIssueCode};

/// Bounded content-free issue record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryIssue {
    /// Opaque operation identity.
    pub operation_id: String,
    /// Closed failure classification.
    pub code: RecoveryIssueCode,
}

/// Complete bounded startup recovery report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedRecoveryReport {
    /// Owner epoch under which recovery executed.
    pub owner_epoch: u64,
    /// Number of transaction identities inspected.
    pub scanned_operations: usize,
    /// Terminal operations whose exact object was freshly replay-verified.
    pub verified_committed: usize,
    /// Prepared/cleanup-pending operations exactly reconciled.
    pub reconciled_operations: usize,
    /// Non-authoritative private temporary files removed.
    pub removed_temporary_files: usize,
    /// Bounded individual issue records.
    pub issues: Vec<RecoveryIssue>,
    /// Additional issue count omitted after the report ceiling.
    pub omitted_issue_count: usize,
    /// True only when every transaction is exact and terminal.
    pub ready: bool,
}

impl SealedRecoveryReport {
    pub(super) const fn new(owner_epoch: u64) -> Self {
        Self {
            owner_epoch,
            scanned_operations: 0,
            verified_committed: 0,
            reconciled_operations: 0,
            removed_temporary_files: 0,
            issues: Vec::new(),
            omitted_issue_count: 0,
            ready: true,
        }
    }

    pub(super) fn issue(&mut self, operation_id: String, code: RecoveryIssueCode) {
        self.ready = false;
        if self.issues.len() < MAX_RECOVERY_ISSUES {
            self.issues.push(RecoveryIssue { operation_id, code });
        } else {
            self.omitted_issue_count = self.omitted_issue_count.saturating_add(1);
        }
    }
}
