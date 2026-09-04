//! Windows CurrentUser DPAPI-backed immutable sealed-object storage.
//!
//! This module is a concrete platform adapter. It stores no plaintext on disk,
//! does not invent cryptography, and never treats logical deletion as proof of
//! physical media erasure. The persisted envelope is strict and bounded; the
//! object identifier is also supplied as DPAPI optional entropy so ciphertext
//! cannot be moved to another object name and still decrypt successfully.

use core::fmt;
use std::path::Path;

/// Maximum plaintext accepted for one sealed object.
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum complete persisted envelope accepted on readback.
pub const MAX_ENVELOPE_BYTES: usize = MAX_PLAINTEXT_BYTES + 2 * 1024 * 1024;
/// Maximum opaque object-identifier length.
pub const MAX_OBJECT_ID_BYTES: usize = 128;

const MAGIC: [u8; 8] = *b"ELSDPAPI";
const FORMAT_VERSION: u16 = 1;
const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 8;

/// Closed sealed-store failure. Display output never contains plaintext or paths.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedStoreError {
    /// DPAPI is unavailable on the current platform.
    UnsupportedPlatform,
    /// The opaque object identifier is malformed.
    InvalidObjectId,
    /// The configured data root is absent or not a directory.
    InvalidDataRoot,
    /// A symlink or Windows reparse point was encountered.
    ReparsePointDenied,
    /// The immutable target already exists.
    ObjectAlreadyExists,
    /// The requested object does not exist.
    ObjectNotFound,
    /// Plaintext is empty.
    EmptyPlaintext,
    /// Plaintext exceeds the finite limit.
    PlaintextTooLarge,
    /// Persisted envelope exceeds the finite limit.
    EnvelopeTooLarge,
    /// Persisted bytes do not match the strict envelope format.
    EnvelopeInvalid,
    /// Persisted object identity differs from the requested identity.
    ObjectBindingMismatch,
    /// The object changed between metadata observation and readback.
    ObjectChangedDuringRead,
    /// DPAPI rejected protection or unprotection.
    DpapiFailure,
    /// A local filesystem operation failed.
    IoFailure,
    /// Exact post-write readback differs from the intended envelope.
    ReadbackMismatch,
}

impl SealedStoreError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_STORE_UNSUPPORTED_PLATFORM",
            Self::InvalidObjectId => "SEALED_STORE_OBJECT_ID_INVALID",
            Self::InvalidDataRoot => "SEALED_STORE_DATA_ROOT_INVALID",
            Self::ReparsePointDenied => "SEALED_STORE_REPARSE_POINT_DENIED",
            Self::ObjectAlreadyExists => "SEALED_STORE_OBJECT_ALREADY_EXISTS",
            Self::ObjectNotFound => "SEALED_STORE_OBJECT_NOT_FOUND",
            Self::EmptyPlaintext => "SEALED_STORE_EMPTY_PLAINTEXT",
            Self::PlaintextTooLarge => "SEALED_STORE_PLAINTEXT_TOO_LARGE",
            Self::EnvelopeTooLarge => "SEALED_STORE_ENVELOPE_TOO_LARGE",
            Self::EnvelopeInvalid => "SEALED_STORE_ENVELOPE_INVALID",
            Self::ObjectBindingMismatch => "SEALED_STORE_OBJECT_BINDING_MISMATCH",
            Self::ObjectChangedDuringRead => "SEALED_STORE_OBJECT_CHANGED_DURING_READ",
            Self::DpapiFailure => "SEALED_STORE_DPAPI_FAILURE",
            Self::IoFailure => "SEALED_STORE_IO_FAILURE",
            Self::ReadbackMismatch => "SEALED_STORE_READBACK_MISMATCH",
        }
    }
}

impl fmt::Display for SealedStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedStoreError {}

/// Plaintext owner that is overwritten before its allocation is released.
///
/// The type is deliberately non-`Clone`; `Debug` never exposes bytes.
pub struct SensitiveBytes(Vec<u8>);

impl SensitiveBytes {
    /// Creates a finite non-empty plaintext buffer.
    pub fn new(bytes: Vec<u8>) -> Result<Self, SealedStoreError> {
        if bytes.is_empty() {
            return Err(SealedStoreError::EmptyPlaintext);
        }
        if bytes.len() > MAX_PLAINTEXT_BYTES {
            return Err(SealedStoreError::PlaintextTooLarge);
        }
        Ok(Self(bytes))
    }

    /// Borrows plaintext for the shortest possible caller-owned interval.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Plaintext byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the plaintext buffer is empty. A valid instance is never empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SensitiveBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveBytes")
            .field("bytes", &"<redacted>")
            .field("length", &self.0.len())
            .finish()
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Content-free successful immutable write receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealReceipt {
    /// Opaque object identity.
    pub object_id: String,
    /// Exact plaintext byte count protected by DPAPI.
    pub plaintext_bytes: u64,
    /// Exact DPAPI ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Strict envelope format version.
    pub format_version: u16,
    /// DPAPI scope used by this adapter.
    pub protection_scope: &'static str,
    /// Exact persisted readback completed.
    pub readback_verified: bool,
}

/// Content-free successful verification receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyReceipt {
    /// Opaque object identity.
    pub object_id: String,
    /// Exact decrypted plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact DPAPI ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Strict envelope format version.
    pub format_version: u16,
    /// DPAPI scope used by this adapter.
    pub protection_scope: &'static str,
    /// DPAPI authentication and identity-bound entropy both verified.
    pub authenticated: bool,
}

/// Content-free logical-deletion receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteReceipt {
    /// Opaque object identity.
    pub object_id: String,
    /// Whether the directory entry was removed.
    pub logical_delete_complete: bool,
    /// This adapter never claims physical media erasure.
    pub physical_erasure_guaranteed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Envelope {
    object_id: String,
    plaintext_bytes: u64,
    ciphertext: Vec<u8>,
}

impl Envelope {
    fn encode(&self) -> Result<Vec<u8>, SealedStoreError> {
        validate_object_id(&self.object_id)?;
        let id = self.object_id.as_bytes();
        let id_len = u16::try_from(id.len()).map_err(|_| SealedStoreError::InvalidObjectId)?;
        let ciphertext_len =
            u64::try_from(self.ciphertext.len()).map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
        let total = HEADER_BYTES
            .checked_add(id.len())
            .and_then(|value| value.checked_add(self.ciphertext.len()))
            .ok_or(SealedStoreError::EnvelopeTooLarge)?;
        if total > MAX_ENVELOPE_BYTES || self.ciphertext.is_empty() {
            return Err(SealedStoreError::EnvelopeTooLarge);
        }
        let mut output = Vec::with_capacity(total);
        output.extend_from_slice(&MAGIC);
        output.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        output.extend_from_slice(&id_len.to_be_bytes());
        output.extend_from_slice(&self.plaintext_bytes.to_be_bytes());
        output.extend_from_slice(&ciphertext_len.to_be_bytes());
        output.extend_from_slice(id);
        output.extend_from_slice(&self.ciphertext);
        Ok(output)
    }

    fn decode(bytes: &[u8]) -> Result<Self, SealedStoreError> {
        if bytes.len() < HEADER_BYTES {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        if bytes.len() > MAX_ENVELOPE_BYTES || bytes[..8] != MAGIC {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let version = u16::from_be_bytes(
            bytes[8..10]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        );
        if version != FORMAT_VERSION {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let id_len = usize::from(u16::from_be_bytes(
            bytes[10..12]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        ));
        let plaintext_bytes = u64::from_be_bytes(
            bytes[12..20]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        );
        let ciphertext_len = usize::try_from(u64::from_be_bytes(
            bytes[20..28]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        ))
        .map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
        let ciphertext_start = HEADER_BYTES
            .checked_add(id_len)
            .ok_or(SealedStoreError::EnvelopeInvalid)?;
        let expected = ciphertext_start
            .checked_add(ciphertext_len)
            .ok_or(SealedStoreError::EnvelopeTooLarge)?;
        if id_len == 0
            || id_len > MAX_OBJECT_ID_BYTES
            || ciphertext_len == 0
            || expected != bytes.len()
            || plaintext_bytes == 0
            || plaintext_bytes > u64::try_from(MAX_PLAINTEXT_BYTES).unwrap_or(u64::MAX)
        {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let object_id = core::str::from_utf8(&bytes[HEADER_BYTES..ciphertext_start])
            .map_err(|_| SealedStoreError::EnvelopeInvalid)?
            .to_owned();
        validate_object_id(&object_id)?;
        Ok(Self {
            object_id,
            plaintext_bytes,
            ciphertext: bytes[ciphertext_start..].to_vec(),
        })
    }
}

/// Protects plaintext and creates one immutable sealed object.
pub fn seal_immutable(
    data_root: &Path,
    object_id: &str,
    plaintext: SensitiveBytes,
) -> Result<SealReceipt, SealedStoreError> {
    platform::seal_immutable(data_root, object_id, plaintext)
}

/// Opens and authenticates one sealed object.
pub fn open_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<SensitiveBytes, SealedStoreError> {
    platform::open_sealed(data_root, object_id)
}

/// Authenticates one sealed object without returning plaintext to the caller.
pub fn verify_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<VerifyReceipt, SealedStoreError> {
    platform::verify_sealed(data_root, object_id)
}

/// Removes one sealed-object directory entry without claiming physical erasure.
pub fn delete_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<DeleteReceipt, SealedStoreError> {
    platform::delete_sealed(data_root, object_id)
}

fn validate_object_id(value: &str) -> Result<(), SealedStoreError> {
    if value.is_empty()
        || value.len() > MAX_OBJECT_ID_BYTES
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(SealedStoreError::InvalidObjectId);
    }
    Ok(())
}

fn entropy_for(object_id: &str) -> Vec<u8> {
    let mut entropy = b"eliot-search/sealed-object/current-user/v1\0".to_vec();
    entropy.extend_from_slice(object_id.as_bytes());
    entropy
}

fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: `byte` is a valid unique pointer for the duration of this
        // call. Volatile writes prevent the explicit wipe from being removed.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(not(windows))]
mod platform {
    use super::{
        DeleteReceipt, SealReceipt, SealedStoreError, SensitiveBytes, VerifyReceipt,
    };
    use std::path::Path;

    pub(super) fn seal_immutable(
        _data_root: &Path,
        _object_id: &str,
        _plaintext: SensitiveBytes,
    ) -> Result<SealReceipt, SealedStoreError> {
        Err(SealedStoreError::UnsupportedPlatform)
    }

    pub(super) fn open_sealed(
        _data_root: &Path,
        _object_id: &str,
    ) -> Result<SensitiveBytes, SealedStoreError> {
        Err(SealedStoreError::UnsupportedPlatform)
    }

    pub(super) fn verify_sealed(
        _data_root: &Path,
        _object_id: &str,
    ) -> Result<VerifyReceipt, SealedStoreError> {
        Err(SealedStoreError::UnsupportedPlatform)
    }

    pub(super) fn delete_sealed(
        _data_root: &Path,
        _object_id: &str,
    ) -> Result<DeleteReceipt, SealedStoreError> {
        Err(SealedStoreError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        DeleteReceipt, Envelope, FORMAT_VERSION, MAX_ENVELOPE_BYTES,
        MAX_PLAINTEXT_BYTES, SealReceipt, SealedStoreError, SensitiveBytes,
        VerifyReceipt, entropy_for, validate_object_id, wipe,
    };
    use core::ffi::c_void;
    use core::ptr::null_mut;
    use core::slice;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const STORE_DIRECTORY: &str = "sealed-revisions";
    const OBJECT_SUFFIX: &str = ".els-dpapi";

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *mut DataBlob,
            description: *const u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            data_in: *mut DataBlob,
            description: *mut *mut u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    struct LocalAllocation {
        pointer: *mut u8,
        length: usize,
        wipe_before_free: bool,
    }

    impl LocalAllocation {
        unsafe fn from_blob(
            blob: DataBlob,
            wipe_before_free: bool,
        ) -> Result<Self, SealedStoreError> {
            if blob.pb_data.is_null() || blob.cb_data == 0 {
                return Err(SealedStoreError::DpapiFailure);
            }
            Ok(Self {
                pointer: blob.pb_data,
                length: blob.cb_data as usize,
                wipe_before_free,
            })
        }

        fn copy(&self) -> Vec<u8> {
            // SAFETY: DPAPI returned a non-null allocation of exactly `length`
            // bytes and this guard retains ownership until after the copy.
            unsafe { slice::from_raw_parts(self.pointer, self.length).to_vec() }
        }
    }

    impl Drop for LocalAllocation {
        fn drop(&mut self) {
            if self.pointer.is_null() {
                return;
            }
            if self.wipe_before_free {
                // SAFETY: the DPAPI allocation remains live and uniquely owned.
                let bytes = unsafe { slice::from_raw_parts_mut(self.pointer, self.length) };
                wipe(bytes);
            }
            // SAFETY: the allocation was returned by DPAPI/LocalAlloc and has
            // not previously been released.
            unsafe {
                let _ = LocalFree(self.pointer.cast());
            }
            self.pointer = null_mut();
            self.length = 0;
        }
    }

    pub(super) fn seal_immutable(
        data_root: &Path,
        object_id: &str,
        plaintext: SensitiveBytes,
    ) -> Result<SealReceipt, SealedStoreError> {
        validate_object_id(object_id)?;
        let directory = ensure_store_directory(data_root, true)?;
        let target = object_path(&directory, object_id);
        if target.exists() {
            return Err(SealedStoreError::ObjectAlreadyExists);
        }
        let ciphertext = protect_current_user(object_id, plaintext.expose())?;
        let envelope = Envelope {
            object_id: object_id.to_owned(),
            plaintext_bytes: u64::try_from(plaintext.len())
                .map_err(|_| SealedStoreError::PlaintextTooLarge)?,
            ciphertext,
        };
        let ciphertext_bytes = u64::try_from(envelope.ciphertext.len())
            .map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
        let encoded = envelope.encode()?;
        write_immutable(&directory, &target, &encoded)?;
        let readback = read_envelope(&target)?;
        if readback != envelope {
            return Err(SealedStoreError::ReadbackMismatch);
        }
        Ok(SealReceipt {
            object_id: object_id.to_owned(),
            plaintext_bytes: envelope.plaintext_bytes,
            ciphertext_bytes,
            format_version: FORMAT_VERSION,
            protection_scope: "windows_current_user_dpapi",
            readback_verified: true,
        })
    }

    pub(super) fn open_sealed(
        data_root: &Path,
        object_id: &str,
    ) -> Result<SensitiveBytes, SealedStoreError> {
        validate_object_id(object_id)?;
        let directory = ensure_store_directory(data_root, false)?;
        let envelope = read_envelope(&object_path(&directory, object_id))?;
        if envelope.object_id != object_id {
            return Err(SealedStoreError::ObjectBindingMismatch);
        }
        let plaintext = unprotect_current_user(object_id, &envelope.ciphertext)?;
        if u64::try_from(plaintext.len()).map_err(|_| SealedStoreError::PlaintextTooLarge)?
            != envelope.plaintext_bytes
        {
            return Err(SealedStoreError::ReadbackMismatch);
        }
        SensitiveBytes::new(plaintext)
    }

    pub(super) fn verify_sealed(
        data_root: &Path,
        object_id: &str,
    ) -> Result<VerifyReceipt, SealedStoreError> {
        validate_object_id(object_id)?;
        let directory = ensure_store_directory(data_root, false)?;
        let envelope = read_envelope(&object_path(&directory, object_id))?;
        if envelope.object_id != object_id {
            return Err(SealedStoreError::ObjectBindingMismatch);
        }
        let plaintext = SensitiveBytes::new(unprotect_current_user(
            object_id,
            &envelope.ciphertext,
        )?)?;
        let plaintext_bytes =
            u64::try_from(plaintext.len()).map_err(|_| SealedStoreError::PlaintextTooLarge)?;
        if plaintext_bytes != envelope.plaintext_bytes {
            return Err(SealedStoreError::ReadbackMismatch);
        }
        Ok(VerifyReceipt {
            object_id: object_id.to_owned(),
            plaintext_bytes,
            ciphertext_bytes: u64::try_from(envelope.ciphertext.len())
                .map_err(|_| SealedStoreError::EnvelopeTooLarge)?,
            format_version: FORMAT_VERSION,
            protection_scope: "windows_current_user_dpapi",
            authenticated: true,
        })
    }

    pub(super) fn delete_sealed(
        data_root: &Path,
        object_id: &str,
    ) -> Result<DeleteReceipt, SealedStoreError> {
        validate_object_id(object_id)?;
        let directory = ensure_store_directory(data_root, false)?;
        let target = object_path(&directory, object_id);
        validate_regular_non_reparse(&target, false)?;
        std::fs::remove_file(&target).map_err(map_not_found)?;
        if target.exists() {
            return Err(SealedStoreError::ReadbackMismatch);
        }
        Ok(DeleteReceipt {
            object_id: object_id.to_owned(),
            logical_delete_complete: true,
            physical_erasure_guaranteed: false,
        })
    }

    fn ensure_store_directory(
        data_root: &Path,
        create: bool,
    ) -> Result<PathBuf, SealedStoreError> {
        validate_directory_non_reparse(data_root)?;
        let directory = data_root.join(STORE_DIRECTORY);
        if !directory.exists() {
            if !create {
                return Err(SealedStoreError::ObjectNotFound);
            }
            fs::create_dir(&directory).map_err(|_| SealedStoreError::IoFailure)?;
        }
        validate_directory_non_reparse(&directory)?;
        Ok(directory)
    }

    fn validate_directory_non_reparse(path: &Path) -> Result<(), SealedStoreError> {
        let metadata = fs::symlink_metadata(path).map_err(|_| SealedStoreError::InvalidDataRoot)?;
        if !metadata.is_dir() {
            return Err(SealedStoreError::InvalidDataRoot);
        }
        if metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedStoreError::ReparsePointDenied);
        }
        Ok(())
    }

    fn validate_regular_non_reparse(
        path: &Path,
        allow_absent: bool,
    ) -> Result<(), SealedStoreError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if allow_absent && error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(SealedStoreError::ObjectNotFound);
            }
            Err(_) => return Err(SealedStoreError::IoFailure),
        };
        if !metadata.is_file() {
            return Err(SealedStoreError::IoFailure);
        }
        if metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedStoreError::ReparsePointDenied);
        }
        Ok(())
    }

    fn object_path(directory: &Path, object_id: &str) -> PathBuf {
        directory.join(format!("{object_id}{OBJECT_SUFFIX}"))
    }

    fn read_envelope(path: &Path) -> Result<Envelope, SealedStoreError> {
        validate_regular_non_reparse(path, false)?;
        let mut file = File::open(path).map_err(map_not_found)?;
        let before = file.metadata().map_err(|_| SealedStoreError::IoFailure)?;
        if before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(SealedStoreError::ReparsePointDenied);
        }
        if before.len() > u64::try_from(MAX_ENVELOPE_BYTES).unwrap_or(u64::MAX) {
            return Err(SealedStoreError::EnvelopeTooLarge);
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(before.len()).map_err(|_| SealedStoreError::EnvelopeTooLarge)?,
        );
        (&mut file)
            .take(u64::try_from(MAX_ENVELOPE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| SealedStoreError::IoFailure)?;
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(SealedStoreError::EnvelopeTooLarge);
        }
        let after = file.metadata().map_err(|_| SealedStoreError::IoFailure)?;
        if before.len() != after.len()
            || before.last_write_time() != after.last_write_time()
            || before.creation_time() != after.creation_time()
            || before.volume_serial_number() != after.volume_serial_number()
            || before.file_index() != after.file_index()
        {
            return Err(SealedStoreError::ObjectChangedDuringRead);
        }
        Envelope::decode(&bytes)
    }

    fn write_immutable(
        directory: &Path,
        target: &Path,
        bytes: &[u8],
    ) -> Result<(), SealedStoreError> {
        validate_regular_non_reparse(target, true)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SealedStoreError::IoFailure)?
            .as_nanos();
        for attempt in 0_u8..32 {
            let temporary = directory.join(format!(
                ".sealed-{}-{timestamp}-{attempt}.tmp",
                std::process::id()
            ));
            let mut file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(SealedStoreError::IoFailure),
            };
            let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
            drop(file);
            if write_result.is_err() {
                let _ = fs::remove_file(&temporary);
                return Err(SealedStoreError::IoFailure);
            }
            match fs::hard_link(&temporary, target) {
                Ok(()) => {
                    let _ = fs::remove_file(&temporary);
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let _ = fs::remove_file(&temporary);
                    return Err(SealedStoreError::ObjectAlreadyExists);
                }
                Err(_) => {
                    let _ = fs::remove_file(&temporary);
                    return Err(SealedStoreError::IoFailure);
                }
            }
        }
        Err(SealedStoreError::IoFailure)
    }

    fn protect_current_user(
        object_id: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SealedStoreError> {
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(if plaintext.is_empty() {
                SealedStoreError::EmptyPlaintext
            } else {
                SealedStoreError::PlaintextTooLarge
            });
        }
        let entropy = entropy_for(object_id);
        let mut input = blob(plaintext)?;
        let mut entropy_blob = blob(&entropy)?;
        let description = format!("ELIOT Search sealed object {object_id}\0")
            .encode_utf16()
            .collect::<Vec<_>>();
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };
        // SAFETY: all pointers refer to live bounded slices for the duration of
        // the call; output ownership is transferred to `LocalAllocation`.
        let succeeded = unsafe {
            CryptProtectData(
                &mut input,
                description.as_ptr(),
                &mut entropy_blob,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if succeeded == 0 {
            return Err(SealedStoreError::DpapiFailure);
        }
        // SAFETY: successful DPAPI output is a LocalAlloc allocation.
        let allocation = unsafe { LocalAllocation::from_blob(output, false)? };
        if allocation.length > MAX_ENVELOPE_BYTES {
            return Err(SealedStoreError::EnvelopeTooLarge);
        }
        Ok(allocation.copy())
    }

    fn unprotect_current_user(
        object_id: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SealedStoreError> {
        if ciphertext.is_empty() || ciphertext.len() > MAX_ENVELOPE_BYTES {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let entropy = entropy_for(object_id);
        let mut input = blob(ciphertext)?;
        let mut entropy_blob = blob(&entropy)?;
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };
        let mut description: *mut u16 = null_mut();
        // SAFETY: all input pointers remain valid; output and optional
        // description allocations are released below with `LocalFree`.
        let succeeded = unsafe {
            CryptUnprotectData(
                &mut input,
                &mut description,
                &mut entropy_blob,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if !description.is_null() {
            // SAFETY: DPAPI documents this output as LocalAlloc memory.
            unsafe {
                let _ = LocalFree(description.cast());
            }
        }
        if succeeded == 0 {
            return Err(SealedStoreError::DpapiFailure);
        }
        // SAFETY: successful DPAPI output is a LocalAlloc allocation. It is
        // wiped before release because it contains plaintext.
        let allocation = unsafe { LocalAllocation::from_blob(output, true)? };
        if allocation.length > MAX_PLAINTEXT_BYTES {
            return Err(SealedStoreError::PlaintextTooLarge);
        }
        Ok(allocation.copy())
    }

    fn blob(bytes: &[u8]) -> Result<DataBlob, SealedStoreError> {
        Ok(DataBlob {
            cb_data: u32::try_from(bytes.len())
                .map_err(|_| SealedStoreError::PlaintextTooLarge)?,
            pb_data: if bytes.is_empty() {
                null_mut()
            } else {
                bytes.as_ptr().cast_mut()
            },
        })
    }

    fn map_not_found(error: io::Error) -> SealedStoreError {
        if error.kind() == io::ErrorKind::NotFound {
            SealedStoreError::ObjectNotFound
        } else {
            SealedStoreError::IoFailure
        }
    }
}
