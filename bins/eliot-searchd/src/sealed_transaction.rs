//! Idempotent intent/readback/reconciliation around DPAPI sealed objects.
//!
//! A retry must provide the exact plaintext again. If an earlier write crossed
//! the mutation boundary, the adapter decrypts the existing object and compares
//! the bytes before recording a reconciled receipt. It never accepts matching
//! length, object name, or ciphertext presence as proof of an exact replay.

use core::fmt;
use std::path::Path;

use crate::sealed_store::{
    SealReceipt, SealedStoreError, SensitiveBytes, open_sealed, seal_immutable,
};

/// Maximum operation-identifier length.
pub const MAX_OPERATION_ID_BYTES: usize = 128;

/// Closed transaction failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedTransactionError {
    /// The current platform does not provide the required adapter.
    UnsupportedPlatform,
    /// Operation identity is malformed.
    InvalidOperationId,
    /// Operation is already executing in another process.
    OperationBusy,
    /// Durable intent is malformed or conflicts with the request.
    IntentConflict,
    /// Durable receipt is malformed or conflicts with the request.
    ReceiptConflict,
    /// An object exists without the exact operation intent/receipt.
    ObjectConflict,
    /// Existing decrypted bytes differ from retry input.
    ReplayContentMismatch,
    /// External effect cannot be classified safely.
    OutcomeUnknown,
    /// Filesystem operation failed.
    IoFailure,
    /// Exact metadata readback failed.
    ReadbackMismatch,
    /// Underlying sealed-store operation failed.
    SealedStore(SealedStoreError),
}

impl SealedTransactionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_TRANSACTION_UNSUPPORTED_PLATFORM",
            Self::InvalidOperationId => "SEALED_TRANSACTION_OPERATION_ID_INVALID",
            Self::OperationBusy => "SEALED_TRANSACTION_OPERATION_BUSY",
            Self::IntentConflict => "SEALED_TRANSACTION_INTENT_CONFLICT",
            Self::ReceiptConflict => "SEALED_TRANSACTION_RECEIPT_CONFLICT",
            Self::ObjectConflict => "SEALED_TRANSACTION_OBJECT_CONFLICT",
            Self::ReplayContentMismatch => "SEALED_TRANSACTION_REPLAY_CONTENT_MISMATCH",
            Self::OutcomeUnknown => "SEALED_TRANSACTION_OUTCOME_UNKNOWN",
            Self::IoFailure => "SEALED_TRANSACTION_IO_FAILURE",
            Self::ReadbackMismatch => "SEALED_TRANSACTION_READBACK_MISMATCH",
            Self::SealedStore(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedTransactionError {}

impl From<SealedStoreError> for SealedTransactionError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

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
    /// Immutable operation identity.
    pub operation_id: String,
    /// Immutable sealed object identity.
    pub object_id: String,
    /// Exact plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Invocation disposition.
    pub disposition: PutDisposition,
    /// Exact sealed-object readback succeeded.
    pub sealed_readback_verified: bool,
    /// Exact transaction-receipt readback succeeded.
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
    /// Intent and receipt coexist or metadata is contradictory.
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
            Self::Conflicted => "CONFLICTED",
        }
    }
}

/// Creates or exactly reconciles one immutable sealed object.
pub fn put_idempotent(
    data_root: &Path,
    operation_id: &str,
    object_id: &str,
    plaintext: SensitiveBytes,
) -> Result<SealedTransactionReceipt, SealedTransactionError> {
    platform::put_idempotent(data_root, operation_id, object_id, plaintext)
}

/// Reads content-free durable transaction state.
pub fn transaction_status(
    data_root: &Path,
    operation_id: &str,
) -> Result<TransactionStatus, SealedTransactionError> {
    platform::transaction_status(data_root, operation_id)
}

fn validate_operation_id(value: &str) -> Result<(), SealedTransactionError> {
    if value.is_empty()
        || value.len() > MAX_OPERATION_ID_BYTES
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(SealedTransactionError::InvalidOperationId);
    }
    Ok(())
}

#[cfg(not(windows))]
mod platform {
    use super::{
        SealedTransactionError, SealedTransactionReceipt, SensitiveBytes,
        TransactionStatus,
    };
    use std::path::Path;

    pub(super) fn put_idempotent(
        _data_root: &Path,
        _operation_id: &str,
        _object_id: &str,
        _plaintext: SensitiveBytes,
    ) -> Result<SealedTransactionReceipt, SealedTransactionError> {
        Err(SealedTransactionError::UnsupportedPlatform)
    }

    pub(super) fn transaction_status(
        _data_root: &Path,
        _operation_id: &str,
    ) -> Result<TransactionStatus, SealedTransactionError> {
        Err(SealedTransactionError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        PutDisposition, SealReceipt, SealedStoreError, SealedTransactionError,
        SealedTransactionReceipt, SensitiveBytes, TransactionStatus,
        open_sealed, seal_immutable, validate_operation_id,
    };
    use std::fs::{self, File, OpenOptions, TryLockError};
    use std::io::{self, Read, Write};
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TRANSACTION_DIRECTORY: &str = "sealed-transactions";
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const MAX_METADATA_BYTES: usize = 4 * 1024;
    const INTENT_MAGIC: &str = "ELIOT-SEALED-INTENT-V1";
    const RECEIPT_MAGIC: &str = "ELIOT-SEALED-RECEIPT-V1";

    struct OperationLock {
        file: File,
    }

    impl Drop for OperationLock {
        fn drop(&mut self) {
            let _ = self.file.unlock();
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Intent {
        operation_id: String,
        object_id: String,
        plaintext_bytes: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Receipt {
        operation_id: String,
        object_id: String,
        plaintext_bytes: u64,
        ciphertext_bytes: u64,
    }

    pub(super) fn put_idempotent(
        data_root: &Path,
        operation_id: &str,
        object_id: &str,
        plaintext: SensitiveBytes,
    ) -> Result<SealedTransactionReceipt, SealedTransactionError> {
        validate_operation_id(operation_id)?;
        let directory = ensure_transaction_directory(data_root, true)?;
        let _lock = acquire_operation_lock(&directory, operation_id)?;
        let intent = Intent {
            operation_id: operation_id.to_owned(),
            object_id: object_id.to_owned(),
            plaintext_bytes: u64::try_from(plaintext.len())
                .map_err(|_| SealedTransactionError::IntentConflict)?,
        };
        let intent_path = metadata_path(&directory, operation_id, "intent");
        let receipt_path = metadata_path(&directory, operation_id, "receipt");

        if receipt_path.exists() {
            if intent_path.exists() {
                return Err(SealedTransactionError::ReceiptConflict);
            }
            let receipt = read_receipt(&receipt_path)?;
            require_receipt_matches(&receipt, &intent)?;
            require_exact_plaintext(data_root, object_id, &plaintext)?;
            return Ok(public_receipt(receipt, PutDisposition::Replay));
        }

        let disposition = if intent_path.exists() {
            let observed = read_intent(&intent_path)?;
            if observed != intent {
                return Err(SealedTransactionError::IntentConflict);
            }
            match open_sealed(data_root, object_id) {
                Ok(existing) => {
                    if existing.expose() != plaintext.expose() {
                        return Err(SealedTransactionError::ReplayContentMismatch);
                    }
                    PutDisposition::Reconciled
                }
                Err(SealedStoreError::ObjectNotFound) => PutDisposition::Created,
                Err(error) => return Err(SealedTransactionError::SealedStore(error)),
            }
        } else {
            match open_sealed(data_root, object_id) {
                Ok(_) => return Err(SealedTransactionError::ObjectConflict),
                Err(SealedStoreError::ObjectNotFound) => {}
                Err(error) => return Err(SealedTransactionError::SealedStore(error)),
            }
            write_once(&directory, &intent_path, encode_intent(&intent).as_bytes())?;
            let readback = read_intent(&intent_path)?;
            if readback != intent {
                return Err(SealedTransactionError::ReadbackMismatch);
            }
            PutDisposition::Created
        };

        let seal = if disposition == PutDisposition::Reconciled {
            let verified = crate::sealed_store::verify_sealed(data_root, object_id)?;
            SealReceipt {
                object_id: verified.object_id,
                plaintext_bytes: verified.plaintext_bytes,
                ciphertext_bytes: verified.ciphertext_bytes,
                format_version: verified.format_version,
                protection_scope: verified.protection_scope,
                readback_verified: verified.authenticated,
            }
        } else {
            seal_immutable(data_root, object_id, plaintext)?
        };
        let receipt = Receipt {
            operation_id: operation_id.to_owned(),
            object_id: object_id.to_owned(),
            plaintext_bytes: seal.plaintext_bytes,
            ciphertext_bytes: seal.ciphertext_bytes,
        };
        write_once(&directory, &receipt_path, encode_receipt(&receipt).as_bytes())?;
        let receipt_readback = read_receipt(&receipt_path)?;
        if receipt_readback != receipt {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        fs::remove_file(&intent_path).map_err(|_| SealedTransactionError::IoFailure)?;
        if intent_path.exists() {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        Ok(public_receipt(receipt, disposition))
    }

    pub(super) fn transaction_status(
        data_root: &Path,
        operation_id: &str,
    ) -> Result<TransactionStatus, SealedTransactionError> {
        validate_operation_id(operation_id)?;
        let directory = ensure_transaction_directory(data_root, false)?;
        let intent = metadata_path(&directory, operation_id, "intent");
        let receipt = metadata_path(&directory, operation_id, "receipt");
        match (intent.exists(), receipt.exists()) {
            (false, false) => Ok(TransactionStatus::Absent),
            (true, false) => {
                let _ = read_intent(&intent)?;
                Ok(TransactionStatus::Prepared)
            }
            (false, true) => {
                let _ = read_receipt(&receipt)?;
                Ok(TransactionStatus::Committed)
            }
            (true, true) => Ok(TransactionStatus::Conflicted),
        }
    }

    fn public_receipt(
        receipt: Receipt,
        disposition: PutDisposition,
    ) -> SealedTransactionReceipt {
        SealedTransactionReceipt {
            operation_id: receipt.operation_id,
            object_id: receipt.object_id,
            plaintext_bytes: receipt.plaintext_bytes,
            ciphertext_bytes: receipt.ciphertext_bytes,
            disposition,
            sealed_readback_verified: true,
            receipt_readback_verified: true,
        }
    }

    fn require_receipt_matches(
        receipt: &Receipt,
        intent: &Intent,
    ) -> Result<(), SealedTransactionError> {
        if receipt.operation_id != intent.operation_id
            || receipt.object_id != intent.object_id
            || receipt.plaintext_bytes != intent.plaintext_bytes
        {
            return Err(SealedTransactionError::ReceiptConflict);
        }
        Ok(())
    }

    fn require_exact_plaintext(
        data_root: &Path,
        object_id: &str,
        expected: &SensitiveBytes,
    ) -> Result<(), SealedTransactionError> {
        let observed = open_sealed(data_root, object_id)?;
        if observed.expose() != expected.expose() {
            return Err(SealedTransactionError::ReplayContentMismatch);
        }
        Ok(())
    }

    fn acquire_operation_lock(
        directory: &Path,
        operation_id: &str,
    ) -> Result<OperationLock, SealedTransactionError> {
        let path = metadata_path(directory, operation_id, "lock");
        let file = OpenOptions::new()
            .create(true)
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

    fn ensure_transaction_directory(
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
        let metadata = fs::symlink_metadata(path).map_err(|_| SealedTransactionError::IoFailure)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedTransactionError::IoFailure);
        }
        Ok(())
    }

    fn metadata_path(directory: &Path, operation_id: &str, suffix: &str) -> PathBuf {
        directory.join(format!("{operation_id}.{suffix}"))
    }

    fn write_once(
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

    fn read_small(path: &Path) -> Result<String, SealedTransactionError> {
        let metadata = fs::symlink_metadata(path).map_err(|_| SealedTransactionError::IoFailure)?;
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

    fn encode_intent(intent: &Intent) -> String {
        format!(
            "{INTENT_MAGIC}\noperation={}\nobject={}\nplaintext_bytes={}\n",
            intent.operation_id, intent.object_id, intent.plaintext_bytes
        )
    }

    fn read_intent(path: &Path) -> Result<Intent, SealedTransactionError> {
        let fields = parse_metadata(&read_small(path)?, INTENT_MAGIC)?;
        let operation_id = field(&fields, "operation")?.to_owned();
        validate_operation_id(&operation_id)?;
        let object_id = field(&fields, "object")?.to_owned();
        let plaintext_bytes = parse_u64(field(&fields, "plaintext_bytes")?)?;
        if fields.len() != 3 || plaintext_bytes == 0 {
            return Err(SealedTransactionError::IntentConflict);
        }
        Ok(Intent {
            operation_id,
            object_id,
            plaintext_bytes,
        })
    }

    fn encode_receipt(receipt: &Receipt) -> String {
        format!(
            concat!(
                "{}\noperation={}\nobject={}\n",
                "plaintext_bytes={}\nciphertext_bytes={}\n"
            ),
            RECEIPT_MAGIC,
            receipt.operation_id,
            receipt.object_id,
            receipt.plaintext_bytes,
            receipt.ciphertext_bytes
        )
    }

    fn read_receipt(path: &Path) -> Result<Receipt, SealedTransactionError> {
        let fields = parse_metadata(&read_small(path)?, RECEIPT_MAGIC)?;
        let operation_id = field(&fields, "operation")?.to_owned();
        validate_operation_id(&operation_id)?;
        let object_id = field(&fields, "object")?.to_owned();
        let plaintext_bytes = parse_u64(field(&fields, "plaintext_bytes")?)?;
        let ciphertext_bytes = parse_u64(field(&fields, "ciphertext_bytes")?)?;
        if fields.len() != 4 || plaintext_bytes == 0 || ciphertext_bytes == 0 {
            return Err(SealedTransactionError::ReceiptConflict);
        }
        Ok(Receipt {
            operation_id,
            object_id,
            plaintext_bytes,
            ciphertext_bytes,
        })
    }

    fn parse_metadata(
        value: &str,
        magic: &str,
    ) -> Result<std::collections::BTreeMap<String, String>, SealedTransactionError> {
        if !value.ends_with('\n') {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        let mut lines = value.lines();
        if lines.next() != Some(magic) {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        let mut fields = std::collections::BTreeMap::new();
        for line in lines {
            let Some((key, field_value)) = line.split_once('=') else {
                return Err(SealedTransactionError::ReadbackMismatch);
            };
            if key.is_empty()
                || field_value.is_empty()
                || fields.insert(key.to_owned(), field_value.to_owned()).is_some()
            {
                return Err(SealedTransactionError::ReadbackMismatch);
            }
        }
        Ok(fields)
    }

    fn field<'a>(
        fields: &'a std::collections::BTreeMap<String, String>,
        key: &str,
    ) -> Result<&'a str, SealedTransactionError> {
        fields
            .get(key)
            .map(String::as_str)
            .ok_or(SealedTransactionError::ReadbackMismatch)
    }

    fn parse_u64(value: &str) -> Result<u64, SealedTransactionError> {
        if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        value
            .parse::<u64>()
            .map_err(|_| SealedTransactionError::ReadbackMismatch)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn receipt_encoding_retains_the_exact_legacy_wire_bytes() {
            let receipt = Receipt {
                operation_id: "operation-1".to_owned(),
                object_id: "object-1".to_owned(),
                plaintext_bytes: 12,
                ciphertext_bytes: 256,
            };
            let encoded = encode_receipt(&receipt);
            assert_eq!(encoded, concat!(
                "ELIOT-SEALED-RECEIPT-V1\noperation=operation-1\nobject=object-1\n",
                "plaintext_bytes=12\nciphertext_bytes=256\n",
            ));
            let fields = parse_metadata(&encoded, RECEIPT_MAGIC).unwrap();
            assert_eq!(fields.len(), 4);
            assert_eq!(parse_u64(field(&fields, "plaintext_bytes").unwrap()).unwrap(), 12);
        }
    }
}
