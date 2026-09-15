//! Closed persisted-format constants and sealed-store failures.

use core::fmt;

/// Maximum plaintext accepted for one sealed object.
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum complete persisted envelope accepted on readback.
pub const MAX_ENVELOPE_BYTES: usize = MAX_PLAINTEXT_BYTES + 2 * 1024 * 1024;
/// Maximum opaque object-identifier length.
pub const MAX_OBJECT_ID_BYTES: usize = 128;

pub(crate) const MAGIC: [u8; 8] = *b"ELSDPAPI";
pub(crate) const FORMAT_VERSION: u16 = 1;
pub(crate) const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 8;

/// Closed sealed-store failure. Display output never contains plaintext or paths.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedStoreError {
    #[cfg(not(windows))]
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
            #[cfg(not(windows))]
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

pub(crate) fn validate_object_id(value: &str) -> Result<(), SealedStoreError> {
    if value.is_empty()
        || value.len() > MAX_OBJECT_ID_BYTES
        || matches!(value, "." | "..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(SealedStoreError::InvalidObjectId);
    }
    Ok(())
}

#[must_use]
pub(crate) fn entropy_for(object_id: &str) -> Vec<u8> {
    let mut entropy = b"eliot-search/sealed-object/current-user/v1\0".to_vec();
    entropy.extend_from_slice(object_id.as_bytes());
    entropy
}
