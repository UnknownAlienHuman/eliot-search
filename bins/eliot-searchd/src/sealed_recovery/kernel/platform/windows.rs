//! Windows sealed-transaction enumeration and exact reconciliation.

use std::collections::BTreeSet;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::sealed_digest::sha256;
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_store::{
    SealedStoreError, SensitiveBytes, open_sealed, verify_sealed,
};
use crate::sealed_transaction::{
    TransactionBinding, TransactionObservation, TransactionStatus,
    inspect_transaction,
};
use crate::sealed_transaction_guard::put_idempotent_verified;

use super::super::report::SealedRecoveryReport;
use super::super::spec::{
    MAX_RECOVERY_OPERATIONS, RecoveryIssueCode, SealedRecoveryError,
};

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
                reconcile_operation(data_root, operation_id, observation, false, &mut report)?;
            }
            TransactionStatus::Committed
            | TransactionStatus::CommittedCleanupPending => {
                reconcile_operation(data_root, operation_id, observation, true, &mut report)?;
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

fn reconcile_operation(
    data_root: &Path,
    operation_id: String,
    observation: TransactionObservation,
    committed: bool,
    report: &mut SealedRecoveryReport,
) -> Result<(), SealedRecoveryError> {
    let Some(binding) = observation.binding else {
        report.issue(operation_id, RecoveryIssueCode::TransactionConflict);
        return Ok(());
    };
    let missing = if committed {
        RecoveryIssueCode::CommittedObjectMissing
    } else {
        RecoveryIssueCode::PreparedObjectMissing
    };
    match verify_binding(data_root, &binding) {
        Ok(plaintext) => {
            put_idempotent_verified(
                data_root,
                &binding.operation_id,
                &binding.object_id,
                &plaintext,
            )?;
            if committed
                && observation.status != TransactionStatus::CommittedCleanupPending
            {
                report.verified_committed =
                    report.verified_committed.saturating_add(1);
            } else {
                report.reconciled_operations =
                    report.reconciled_operations.saturating_add(1);
            }
        }
        Err(BindingFailure::Missing) => report.issue(operation_id, missing),
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
    Ok(())
}

fn verify_binding(
    data_root: &Path,
    binding: &TransactionBinding,
) -> Result<SensitiveBytes, BindingFailure> {
    let plaintext = match open_sealed(data_root, &binding.object_id) {
        Ok(value) => value,
        Err(SealedStoreError::ObjectNotFound) => return Err(BindingFailure::Missing),
        Err(error) => return Err(BindingFailure::Fatal(error.into())),
    };
    let length =
        u64::try_from(plaintext.len()).map_err(|_| BindingFailure::PlaintextLength)?;
    if length != binding.plaintext_bytes {
        return Err(BindingFailure::PlaintextLength);
    }
    let digest =
        sha256(plaintext.expose()).map_err(|error| BindingFailure::Fatal(error.into()))?;
    if digest != binding.plaintext_sha256 {
        return Err(BindingFailure::Digest);
    }
    let verified = verify_sealed(data_root, &binding.object_id)
        .map_err(|error| BindingFailure::Fatal(error.into()))?;
    if verified.plaintext_bytes != binding.plaintext_bytes || !verified.authenticated {
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
        && file_name.len() >= ".tmp".len()
        && file_name.as_bytes()[file_name.len() - ".tmp".len()..]
            .eq_ignore_ascii_case(b".tmp")
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
