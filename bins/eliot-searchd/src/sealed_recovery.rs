//! Bounded startup recovery for DPAPI-sealed transactions.
//!
//! Recovery runs only while a verified [`OwnerEpochGuard`] retains the exclusive
//! data-root lock. It enumerates the strict transaction directory, removes only
//! non-authoritative private temporary files, and reconciles an operation only
//! when the existing DPAPI object matches the V2 intent/receipt byte count and
//! Windows CNG SHA-256. A prepared operation with no object remains unresolved;
//! recovery never recreates missing content without the original bytes.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::path::Path;

use crate::sealed_digest::DigestError;
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_store::SealedStoreError;
use crate::sealed_transaction::SealedTransactionError;

/// Maximum durable operation identities inspected during one startup.
pub const MAX_RECOVERY_OPERATIONS: usize = 1_000_000;
/// Maximum individual issue records retained in one report.
pub const MAX_RECOVERY_ISSUES: usize = 4_096;

/// Closed structural recovery failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedRecoveryError {
    /// Windows recovery is unavailable on the current platform.
    UnsupportedPlatform,
    /// Recovery was attempted without a live owner/root-lock guard.
    OwnerGuardRequired,
    /// Transaction directory or an entry is malformed or reparse-backed.
    TransactionDirectoryInvalid,
    /// Unknown or malformed transaction filename was observed.
    TransactionFilenameInvalid,
    /// Finite operation capacity was exceeded.
    OperationCapacityExceeded,
    /// Filesystem enumeration, readback, or cleanup failed.
    IoFailure,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// V2 transaction inspection or replay failed.
    Transaction(SealedTransactionError),
}

impl SealedRecoveryError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_RECOVERY_UNSUPPORTED_PLATFORM",
            Self::OwnerGuardRequired => "SEALED_RECOVERY_OWNER_GUARD_REQUIRED",
            Self::TransactionDirectoryInvalid => {
                "SEALED_RECOVERY_TRANSACTION_DIRECTORY_INVALID"
            }
            Self::TransactionFilenameInvalid => {
                "SEALED_RECOVERY_TRANSACTION_FILENAME_INVALID"
            }
            Self::OperationCapacityExceeded => {
                "SEALED_RECOVERY_OPERATION_CAPACITY_EXCEEDED"
            }
            Self::IoFailure => "SEALED_RECOVERY_IO_FAILURE",
            Self::Digest(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedRecoveryError {}

impl From<DigestError> for SealedRecoveryError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedStoreError> for SealedRecoveryError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for SealedRecoveryError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}

/// Non-success classification retained without source content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecoveryIssueCode {
    /// Durable intent exists but the external sealed object is absent.
    PreparedObjectMissing,
    /// A committed receipt exists but its sealed object is absent.
    CommittedObjectMissing,
    /// Intent and receipt coexist but do not bind the same exact request.
    TransactionConflict,
    /// DPAPI plaintext byte count differs from durable metadata.
    PlaintextLengthMismatch,
    /// DPAPI ciphertext byte count differs from the terminal receipt.
    CiphertextLengthMismatch,
    /// Exact plaintext SHA-256 differs from durable metadata.
    PlaintextDigestMismatch,
}

impl RecoveryIssueCode {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreparedObjectMissing => "PREPARED_OBJECT_MISSING",
            Self::CommittedObjectMissing => "COMMITTED_OBJECT_MISSING",
            Self::TransactionConflict => "TRANSACTION_CONFLICT",
            Self::PlaintextLengthMismatch => "PLAINTEXT_LENGTH_MISMATCH",
            Self::CiphertextLengthMismatch => "CIPHERTEXT_LENGTH_MISMATCH",
            Self::PlaintextDigestMismatch => "PLAINTEXT_DIGEST_MISMATCH",
        }
    }
}

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
    fn new(owner_epoch: u64) -> Self {
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

    fn issue(&mut self, operation_id: String, code: RecoveryIssueCode) {
        self.ready = false;
        if self.issues.len() < MAX_RECOVERY_ISSUES {
            self.issues.push(RecoveryIssue { operation_id, code });
        } else {
            self.omitted_issue_count = self.omitted_issue_count.saturating_add(1);
        }
    }
}

/// Performs exact bounded reconciliation under the current owner guard.
pub fn recover_all(
    data_root: &Path,
    owner: &OwnerEpochGuard,
) -> Result<SealedRecoveryReport, SealedRecoveryError> {
    if !owner.root_lock_held() || owner.epoch() == 0 {
        return Err(SealedRecoveryError::OwnerGuardRequired);
    }
    platform::recover_all(data_root, owner)
}

#[cfg(not(windows))]
mod platform {
    use super::{OwnerEpochGuard, SealedRecoveryError, SealedRecoveryReport};
    use std::path::Path;

    pub(super) fn recover_all(
        _data_root: &Path,
        _owner: &OwnerEpochGuard,
    ) -> Result<SealedRecoveryReport, SealedRecoveryError> {
        Err(SealedRecoveryError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        MAX_RECOVERY_OPERATIONS, OwnerEpochGuard, RecoveryIssueCode,
        SealedRecoveryError, SealedRecoveryReport,
    };
    use crate::sealed_digest::sha256;
    use crate::sealed_store::{SealedStoreError, open_sealed, verify_sealed};
    use crate::sealed_transaction::{
        TransactionBinding, TransactionStatus, inspect_transaction,
    };
    use crate::sealed_transaction_guard::put_idempotent_verified;
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    const TRANSACTION_DIRECTORY: &str = "sealed-transactions";
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    pub(super) fn recover_all(
        data_root: &Path,
        owner: &OwnerEpochGuard,
    ) -> Result<SealedRecoveryReport, SealedRecoveryError> {
        let mut report = SealedRecoveryReport::new(owner.epoch());
        let directory = data_root.join(TRANSACTION_DIRECTORY);
        if !directory.exists() {
            return Ok(report);
        }
        validate_directory(&directory)?;
        let (operations, temporary_files) = enumerate(&directory)?;
        for temporary in temporary_files {
            fs::remove_file(&temporary).map_err(|_| SealedRecoveryError::IoFailure)?;
            if temporary.exists() {
                return Err(SealedRecoveryError::IoFailure);
            }
            report.removed_temporary_files =
                report.removed_temporary_files.saturating_add(1);
        }

        for operation_id in operations {
            report.scanned_operations = report.scanned_operations.saturating_add(1);
            let observation = inspect_transaction(data_root, &operation_id)?;
            match observation.status {
                TransactionStatus::Absent => {}
                TransactionStatus::Conflicted => {
                    report.issue(operation_id, RecoveryIssueCode::TransactionConflict);
                }
                TransactionStatus::Prepared => {
                    let Some(binding) = observation.binding else {
                        report.issue(operation_id, RecoveryIssueCode::TransactionConflict);
                        continue;
                    };
                    match verify_binding(data_root, &binding) {
                        Ok(plaintext) => {
                            put_idempotent_verified(
                                data_root,
                                &binding.operation_id,
                                &binding.object_id,
                                plaintext,
                            )?;
                            report.reconciled_operations =
                                report.reconciled_operations.saturating_add(1);
                        }
                        Err(BindingFailure::Missing) => report.issue(
                            operation_id,
                            RecoveryIssueCode::PreparedObjectMissing,
                        ),
                        Err(BindingFailure::PlaintextLength) => report.issue(
                            operation_id,
                            RecoveryIssueCode::PlaintextLengthMismatch,
                        ),
                        Err(BindingFailure::CiphertextLength) => report.issue(
                            operation_id,
                            RecoveryIssueCode::CiphertextLengthMismatch,
                        ),
                        Err(BindingFailure::Digest) => report.issue(
                            operation_id,
                            RecoveryIssueCode::PlaintextDigestMismatch,
                        ),
                        Err(BindingFailure::Fatal(error)) => return Err(error),
                    }
                }
                TransactionStatus::Committed
                | TransactionStatus::CommittedCleanupPending => {
                    let Some(binding) = observation.binding else {
                        report.issue(operation_id, RecoveryIssueCode::TransactionConflict);
                        continue;
                    };
                    match verify_binding(data_root, &binding) {
                        Ok(plaintext) => {
                            put_idempotent_verified(
                                data_root,
                                &binding.operation_id,
                                &binding.object_id,
                                plaintext,
                            )?;
                            if observation.status
                                == TransactionStatus::CommittedCleanupPending
                            {
                                report.reconciled_operations =
                                    report.reconciled_operations.saturating_add(1);
                            } else {
                                report.verified_committed =
                                    report.verified_committed.saturating_add(1);
                            }
                        }
                        Err(BindingFailure::Missing) => report.issue(
                            operation_id,
                            RecoveryIssueCode::CommittedObjectMissing,
                        ),
                        Err(BindingFailure::PlaintextLength) => report.issue(
                            operation_id,
                            RecoveryIssueCode::PlaintextLengthMismatch,
                        ),
                        Err(BindingFailure::CiphertextLength) => report.issue(
                            operation_id,
                            RecoveryIssueCode::CiphertextLengthMismatch,
                        ),
                        Err(BindingFailure::Digest) => report.issue(
                            operation_id,
                            RecoveryIssueCode::PlaintextDigestMismatch,
                        ),
                        Err(BindingFailure::Fatal(error)) => return Err(error),
                    }
                }
            }
        }
        report.ready = report.issues.is_empty() && report.omitted_issue_count == 0;
        Ok(report)
    }

    enum BindingFailure {
        Missing,
        PlaintextLength,
        CiphertextLength,
        Digest,
        Fatal(SealedRecoveryError),
    }

    fn verify_binding(
        data_root: &Path,
        binding: &TransactionBinding,
    ) -> Result<crate::sealed_store::SensitiveBytes, BindingFailure> {
        let plaintext = match open_sealed(data_root, &binding.object_id) {
            Ok(value) => value,
            Err(SealedStoreError::ObjectNotFound) => return Err(BindingFailure::Missing),
            Err(error) => return Err(BindingFailure::Fatal(error.into())),
        };
        let length = u64::try_from(plaintext.len())
            .map_err(|_| BindingFailure::PlaintextLength)?;
        if length != binding.plaintext_bytes {
            return Err(BindingFailure::PlaintextLength);
        }
        let digest = sha256(plaintext.expose())
            .map_err(|error| BindingFailure::Fatal(error.into()))?;
        if digest != binding.plaintext_sha256 {
            return Err(BindingFailure::Digest);
        }
        let verified = verify_sealed(data_root, &binding.object_id)
            .map_err(|error| BindingFailure::Fatal(error.into()))?;
        if verified.plaintext_bytes != binding.plaintext_bytes
            || !verified.authenticated
        {
            return Err(BindingFailure::PlaintextLength);
        }
        if binding
            .ciphertext_bytes
            .is_some_and(|expected| expected != verified.ciphertext_bytes)
        {
            return Err(BindingFailure::CiphertextLength);
        }
        Ok(plaintext)
    }

    fn enumerate(
        directory: &Path,
    ) -> Result<(BTreeSet<String>, Vec<PathBuf>), SealedRecoveryError> {
        let mut operations = BTreeSet::new();
        let mut temporary_files = Vec::new();
        for entry in fs::read_dir(directory).map_err(|_| SealedRecoveryError::IoFailure)? {
            let entry = entry.map_err(|_| SealedRecoveryError::IoFailure)?;
            let metadata = entry
                .metadata()
                .map_err(|_| SealedRecoveryError::IoFailure)?;
            if !metadata.is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(SealedRecoveryError::TransactionDirectoryInvalid);
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                return Err(SealedRecoveryError::TransactionFilenameInvalid);
            };
            if is_private_temporary(file_name) {
                temporary_files.push(entry.path());
                continue;
            }
            if let Some(operation_id) = transaction_operation_id(file_name) {
                validate_operation_token(operation_id)?;
                if file_name.ends_with(".intent") || file_name.ends_with(".receipt") {
                    operations.insert(operation_id.to_owned());
                    if operations.len() > MAX_RECOVERY_OPERATIONS {
                        return Err(SealedRecoveryError::OperationCapacityExceeded);
                    }
                }
                continue;
            }
            return Err(SealedRecoveryError::TransactionFilenameInvalid);
        }
        Ok((operations, temporary_files))
    }

    fn transaction_operation_id(file_name: &str) -> Option<&str> {
        file_name
            .strip_suffix(".intent")
            .or_else(|| file_name.strip_suffix(".receipt"))
            .or_else(|| file_name.strip_suffix(".lock"))
    }

    fn validate_operation_token(value: &str) -> Result<(), SealedRecoveryError> {
        if value.is_empty()
            || value.len() > 128
            || matches!(value, "." | "..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
            })
        {
            return Err(SealedRecoveryError::TransactionFilenameInvalid);
        }
        Ok(())
    }

    fn is_private_temporary(file_name: &str) -> bool {
        file_name.starts_with(".transaction-")
            && file_name.ends_with(".tmp")
            && file_name.len() <= 256
            && file_name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
            })
    }

    fn validate_directory(path: &Path) -> Result<(), SealedRecoveryError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| SealedRecoveryError::TransactionDirectoryInvalid)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedRecoveryError::TransactionDirectoryInvalid);
        }
        Ok(())
    }
}
