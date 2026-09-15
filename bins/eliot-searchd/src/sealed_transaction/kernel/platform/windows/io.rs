//! Windows operation lock, metadata paths and immutable small-file I/O.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::super::spec::SealedTransactionError;

const TRANSACTION_DIRECTORY: &str = "sealed-transactions";
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
pub(super) const MAX_METADATA_BYTES: usize = 4 * 1024;

pub(super) struct OperationLock {
    file: File,
}

impl Drop for OperationLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub(super) fn acquire_operation_lock(
    directory: &Path,
    operation_id: &str,
) -> Result<OperationLock, SealedTransactionError> {
    let path = metadata_path(directory, operation_id, "lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| SealedTransactionError::IoFailure)?;
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => SealedTransactionError::OperationBusy,
        TryLockError::Error(_) => SealedTransactionError::IoFailure,
    })?;
    Ok(OperationLock { file })
}

pub(super) fn ensure_transaction_directory(
    data_root: &Path,
    create: bool,
) -> Result<PathBuf, SealedTransactionError> {
    validate_directory(data_root)?;
    let directory = data_root.join(TRANSACTION_DIRECTORY);
    if !directory.exists() {
        if !create {
            return Ok(directory);
        }
        fs::create_dir(&directory).map_err(|_| SealedTransactionError::IoFailure)?;
    }
    validate_directory(&directory)?;
    Ok(directory)
}

fn validate_directory(path: &Path) -> Result<(), SealedTransactionError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SealedTransactionError::IoFailure)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(SealedTransactionError::IoFailure);
    }
    Ok(())
}

pub(super) fn metadata_path(
    directory: &Path,
    operation_id: &str,
    suffix: &str,
) -> PathBuf {
    directory.join(format!("{operation_id}.{suffix}"))
}

pub(super) fn write_once(
    directory: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<(), SealedTransactionError> {
    if bytes.is_empty() || bytes.len() > MAX_METADATA_BYTES {
        return Err(SealedTransactionError::IoFailure);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SealedTransactionError::IoFailure)?
        .as_nanos();
    for attempt in 0_u8..32 {
        let temporary = directory.join(format!(
            ".transaction-{}-{timestamp}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(SealedTransactionError::IoFailure),
        };
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(SealedTransactionError::IoFailure);
        }
        match fs::hard_link(&temporary, target) {
            Ok(()) => {
                let _ = fs::remove_file(&temporary);
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary);
                return Err(SealedTransactionError::ReceiptConflict);
            }
            Err(_) => {
                let _ = fs::remove_file(&temporary);
                return Err(SealedTransactionError::IoFailure);
            }
        }
    }
    Err(SealedTransactionError::IoFailure)
}

pub(super) fn read_small(path: &Path) -> Result<String, SealedTransactionError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SealedTransactionError::IoFailure)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.len() > u64::try_from(MAX_METADATA_BYTES).unwrap_or(u64::MAX)
    {
        return Err(SealedTransactionError::IoFailure);
    }
    let mut file = File::open(path).map_err(|_| SealedTransactionError::IoFailure)?;
    let before = file.metadata().map_err(|_| SealedTransactionError::IoFailure)?;
    let before_identity = eliot_searchd::native_file::observe(&file)
        .map_err(|_| SealedTransactionError::IoFailure)?;
    let mut value = String::new();
    (&mut file)
        .take(u64::try_from(MAX_METADATA_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_string(&mut value)
        .map_err(|_| SealedTransactionError::IoFailure)?;
    if value.len() > MAX_METADATA_BYTES {
        return Err(SealedTransactionError::IoFailure);
    }
    let after = file.metadata().map_err(|_| SealedTransactionError::IoFailure)?;
    let after_identity = eliot_searchd::native_file::observe(&file)
        .map_err(|_| SealedTransactionError::IoFailure)?;
    if before.len() != after.len()
        || before.last_write_time() != after.last_write_time()
        || before_identity != after_identity
    {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    Ok(value)
}
