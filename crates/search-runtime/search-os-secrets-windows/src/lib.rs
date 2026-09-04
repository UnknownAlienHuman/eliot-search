//! Windows current-user DPAPI adapter for short application secrets.
//!
//! This crate is the platform effect boundary for `CryptProtectData` and
//! `CryptUnprotectData`. It deliberately has no filesystem, registry, clock,
//! process, or network capability. Persistence remains the caller's job.
//!
//! The protected blob is bound twice:
//! - Windows DPAPI binds it to the current interactive/service user profile;
//! - required optional entropy binds it to one exact ELIOT Search scope.
//!
//! Plaintext is returned only as [`SecretBytes`], whose debug output is redacted
//! and whose owned buffer is overwritten before deallocation.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

use core::fmt;

/// Maximum plaintext accepted by one DPAPI operation.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
/// Maximum serialized scope entropy accepted by one DPAPI operation.
pub const MAX_SCOPE_BYTES: usize = 4 * 1024;
/// Maximum ciphertext accepted from persistence or returned by DPAPI.
pub const MAX_PROTECTED_BYTES: usize = 1024 * 1024;
/// Current protected-blob envelope version.
pub const PROTECTED_SECRET_VERSION: u16 = 1;

/// Closed failure from validation or the Windows DPAPI boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DpapiError {
    /// The adapter was called on a non-Windows target.
    UnsupportedPlatform,
    /// Plaintext was empty.
    EmptySecret,
    /// Plaintext exceeded [`MAX_SECRET_BYTES`].
    SecretTooLarge,
    /// A scope component was empty, malformed, or too large.
    InvalidScope,
    /// Protected bytes were empty, oversized, or used another version.
    InvalidProtectedSecret,
    /// A byte length could not be represented by the Windows ABI.
    LengthOverflow,
    /// Windows returned an invalid output pointer/length pair.
    InvalidPlatformOutput,
    /// Windows DPAPI failed with the captured `GetLastError` code.
    PlatformFailure(u32),
}

impl DpapiError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "DPAPI_UNSUPPORTED_PLATFORM",
            Self::EmptySecret => "DPAPI_SECRET_EMPTY",
            Self::SecretTooLarge => "DPAPI_SECRET_TOO_LARGE",
            Self::InvalidScope => "DPAPI_SCOPE_INVALID",
            Self::InvalidProtectedSecret => "DPAPI_PROTECTED_SECRET_INVALID",
            Self::LengthOverflow => "DPAPI_LENGTH_OVERFLOW",
            Self::InvalidPlatformOutput => "DPAPI_PLATFORM_OUTPUT_INVALID",
            Self::PlatformFailure(_) => "DPAPI_PLATFORM_FAILURE",
        }
    }
}

impl fmt::Display for DpapiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlatformFailure(code) => write!(formatter, "{}:{code}", self.code()),
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for DpapiError {}

/// Exact non-secret authority scope used as DPAPI optional entropy.
///
/// Components are length-prefixed before use, so concatenation cannot create a
/// second valid scope. Scope text is never placed in the protected envelope.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProtectionScope {
    encoded: Vec<u8>,
}

impl ProtectionScope {
    /// Creates the canonical `eliot-search.dpapi.scope.v1` binding.
    pub fn new(
        user_binding: &str,
        installation_id: &str,
        installation_incarnation_id: &str,
        purpose: &str,
    ) -> Result<Self, DpapiError> {
        let fields = [
            "eliot-search.dpapi.scope.v1",
            user_binding,
            installation_id,
            installation_incarnation_id,
            purpose,
        ];
        let mut encoded = Vec::new();
        for field in fields {
            validate_scope_field(field)?;
            let bytes = field.as_bytes();
            let length = u32::try_from(bytes.len()).map_err(|_| DpapiError::LengthOverflow)?;
            encoded.extend_from_slice(&length.to_be_bytes());
            encoded.extend_from_slice(bytes);
        }
        if encoded.len() > MAX_SCOPE_BYTES {
            return Err(DpapiError::InvalidScope);
        }
        Ok(Self { encoded })
    }

    /// Canonical entropy bytes passed to DPAPI.
    #[must_use]
    pub fn as_entropy(&self) -> &[u8] {
        &self.encoded
    }
}

fn validate_scope_field(value: &str) -> Result<(), DpapiError> {
    if value.is_empty()
        || value.len() > 1024
        || value.trim() != value
        || value
            .chars()
            .any(|character| character == '\0' || character.is_control())
    {
        return Err(DpapiError::InvalidScope);
    }
    Ok(())
}

/// Owned plaintext with redacted debug output and overwrite-on-drop semantics.
///
/// This type intentionally does not implement `Clone`, `Serialize`, `AsRef`, or
/// `Deref`; callers must use the explicit, auditable exposure method.
pub struct SecretBytes {
    bytes: Vec<u8>,
}

impl SecretBytes {
    /// Takes ownership of finite non-empty plaintext.
    pub fn new(bytes: Vec<u8>) -> Result<Self, DpapiError> {
        validate_secret(&bytes)?;
        Ok(Self { bytes })
    }

    /// Exposes plaintext to the immediate cryptographic/application boundary.
    #[must_use]
    pub fn expose_secret(&self) -> &[u8] {
        &self.bytes
    }

    /// Plaintext byte length without exposing its content.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the owned plaintext is empty.
    ///
    /// A successfully constructed value always returns `false`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretBytes")
            .field("bytes", &"<redacted>")
            .field("length", &self.bytes.len())
            .finish()
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        // `write_volatile` prevents this security-relevant clear from being
        // optimized away as a dead store before Vec deallocation.
        for byte in &mut self.bytes {
            // SAFETY: `byte` is a valid unique pointer to one initialized u8.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// Versioned DPAPI ciphertext suitable for caller-owned durable persistence.
#[derive(Clone, Eq, PartialEq)]
pub struct ProtectedSecret {
    version: u16,
    bytes: Vec<u8>,
}

impl ProtectedSecret {
    /// Validates persisted protected bytes before attempting decryption.
    pub fn from_bytes(version: u16, bytes: Vec<u8>) -> Result<Self, DpapiError> {
        if version != PROTECTED_SECRET_VERSION
            || bytes.is_empty()
            || bytes.len() > MAX_PROTECTED_BYTES
        {
            return Err(DpapiError::InvalidProtectedSecret);
        }
        Ok(Self { version, bytes })
    }

    /// Current envelope version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Exact protected bytes for durable persistence.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Protected byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether protected bytes are empty.
    ///
    /// A successfully constructed value always returns `false`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for ProtectedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedSecret")
            .field("version", &self.version)
            .field(
                "bytes",
                &format_args!("<{} protected bytes>", self.bytes.len()),
            )
            .finish()
    }
}

fn validate_secret(bytes: &[u8]) -> Result<(), DpapiError> {
    if bytes.is_empty() {
        return Err(DpapiError::EmptySecret);
    }
    if bytes.len() > MAX_SECRET_BYTES {
        return Err(DpapiError::SecretTooLarge);
    }
    Ok(())
}

/// Protects plaintext for the current Windows user and exact authority scope.
#[cfg(windows)]
pub fn protect_current_user(
    secret: &SecretBytes,
    scope: &ProtectionScope,
) -> Result<ProtectedSecret, DpapiError> {
    windows::protect(secret, scope)
}

/// Protects plaintext for the current Windows user and exact authority scope.
#[cfg(not(windows))]
pub fn protect_current_user(
    _secret: &SecretBytes,
    _scope: &ProtectionScope,
) -> Result<ProtectedSecret, DpapiError> {
    Err(DpapiError::UnsupportedPlatform)
}

/// Decrypts ciphertext for the current Windows user and exact authority scope.
#[cfg(windows)]
pub fn unprotect_current_user(
    protected: &ProtectedSecret,
    scope: &ProtectionScope,
) -> Result<SecretBytes, DpapiError> {
    windows::unprotect(protected, scope)
}

/// Decrypts ciphertext for the current Windows user and exact authority scope.
#[cfg(not(windows))]
pub fn unprotect_current_user(
    _protected: &ProtectedSecret,
    _scope: &ProtectionScope,
) -> Result<SecretBytes, DpapiError> {
    Err(DpapiError::UnsupportedPlatform)
}

#[cfg(windows)]
mod windows {
    use core::ffi::c_void;
    use core::ptr::{null, null_mut};
    use core::slice;

    use super::{
        DpapiError, MAX_PROTECTED_BYTES, PROTECTED_SECRET_VERSION, ProtectedSecret,
        ProtectionScope, SecretBytes, validate_secret,
    };

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const DataBlob,
            description: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            input: *const DataBlob,
            description: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    fn blob(bytes: &[u8]) -> Result<DataBlob, DpapiError> {
        Ok(DataBlob {
            size: u32::try_from(bytes.len()).map_err(|_| DpapiError::LengthOverflow)?,
            data: bytes.as_ptr().cast_mut(),
        })
    }

    fn platform_error() -> DpapiError {
        // SAFETY: `GetLastError` has no preconditions and immediately captures
        // the calling thread's error after the failed DPAPI call.
        DpapiError::PlatformFailure(unsafe { GetLastError() })
    }

    fn validate_output(output: &DataBlob, maximum: usize) -> Result<usize, DpapiError> {
        let length = usize::try_from(output.size).map_err(|_| DpapiError::LengthOverflow)?;
        if output.data.is_null() || length == 0 || length > maximum {
            return Err(DpapiError::InvalidPlatformOutput);
        }
        Ok(length)
    }

    unsafe fn free_local(memory: *mut c_void) {
        if !memory.is_null() {
            // SAFETY: caller passes only pointers allocated by DPAPI/LocalAlloc
            // and transfers their ownership exactly once to this function.
            let _ = unsafe { LocalFree(memory) };
        }
    }

    unsafe fn clear_and_free_local(memory: *mut u8, length: usize) {
        if !memory.is_null() {
            for offset in 0..length {
                // SAFETY: DPAPI returned a contiguous `length`-byte allocation.
                unsafe { core::ptr::write_volatile(memory.add(offset), 0) };
            }
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            // SAFETY: the same DPAPI allocation is released exactly once.
            unsafe { free_local(memory.cast()) };
        }
    }

    pub(super) fn protect(
        secret: &SecretBytes,
        scope: &ProtectionScope,
    ) -> Result<ProtectedSecret, DpapiError> {
        validate_secret(secret.expose_secret())?;
        let input = blob(secret.expose_secret())?;
        let entropy = blob(scope.as_entropy())?;
        let mut output = DataBlob {
            size: 0,
            data: null_mut(),
        };
        // SAFETY: every input blob points to a live immutable slice for the
        // duration of the call; optional pointers are null as required; output
        // is initialized and owned by the caller after successful return.
        let succeeded = unsafe {
            CryptProtectData(
                &input,
                null(),
                &entropy,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if succeeded == 0 {
            return Err(platform_error());
        }
        let length = match validate_output(&output, MAX_PROTECTED_BYTES) {
            Ok(length) => length,
            Err(error) => {
                // SAFETY: successful DPAPI output is LocalAlloc-owned.
                unsafe { free_local(output.data.cast()) };
                return Err(error);
            }
        };
        // SAFETY: validated non-null DPAPI allocation is live for `length` bytes.
        let protected = unsafe { slice::from_raw_parts(output.data, length) }.to_vec();
        // SAFETY: successful DPAPI output is released exactly once after copy.
        unsafe { free_local(output.data.cast()) };
        ProtectedSecret::from_bytes(PROTECTED_SECRET_VERSION, protected)
    }

    pub(super) fn unprotect(
        protected: &ProtectedSecret,
        scope: &ProtectionScope,
    ) -> Result<SecretBytes, DpapiError> {
        let input = blob(protected.as_bytes())?;
        let entropy = blob(scope.as_entropy())?;
        let mut output = DataBlob {
            size: 0,
            data: null_mut(),
        };
        let mut description: *mut u16 = null_mut();
        // SAFETY: every input blob points to a live immutable slice for the
        // duration of the call; output/description pointers are initialized and
        // become caller-owned LocalAlloc allocations only on success.
        let succeeded = unsafe {
            CryptUnprotectData(
                &input,
                &mut description,
                &entropy,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if succeeded == 0 {
            return Err(platform_error());
        }
        // No description was supplied during protection, but Windows may still
        // return an allocation. It contains no plaintext and is released now.
        unsafe { free_local(description.cast()) };
        let length = match validate_output(&output, super::MAX_SECRET_BYTES) {
            Ok(length) => length,
            Err(error) => {
                // SAFETY: successful DPAPI output is LocalAlloc-owned. Use the
                // ABI-reported length when bounded; otherwise release without a
                // potentially out-of-bounds clear.
                let reported = usize::try_from(output.size).unwrap_or(0);
                if reported <= super::MAX_SECRET_BYTES {
                    unsafe { clear_and_free_local(output.data, reported) };
                } else {
                    unsafe { free_local(output.data.cast()) };
                }
                return Err(error);
            }
        };
        // SAFETY: validated non-null DPAPI allocation is live for `length` bytes.
        let plaintext = unsafe { slice::from_raw_parts(output.data, length) }.to_vec();
        // SAFETY: copied plaintext is cleared in the OS allocation before free.
        unsafe { clear_and_free_local(output.data, length) };
        SecretBytes::new(plaintext)
    }
}

#[cfg(all(test, windows))]
mod windows_dpapi_tests {
    use super::{ProtectionScope, SecretBytes, protect_current_user, unprotect_current_user};

    #[test]
    fn current_user_round_trip_is_scope_bound() {
        let scope = ProtectionScope::new(
            "cargo-test-current-user",
            "eliot-search-ci",
            "dpapi-regression-v1",
            "revision-store-master-key",
        )
        .expect("scope");
        let wrong_scope = ProtectionScope::new(
            "cargo-test-current-user",
            "eliot-search-ci",
            "dpapi-regression-v1",
            "wrong-purpose",
        )
        .expect("wrong scope");
        let plaintext = SecretBytes::new(b"bounded-dpapi-round-trip".to_vec()).expect("plaintext");

        let protected = protect_current_user(&plaintext, &scope).expect("protect");
        assert_ne!(protected.as_bytes(), plaintext.expose_secret());
        let recovered = unprotect_current_user(&protected, &scope).expect("unprotect");
        assert_eq!(recovered.expose_secret(), plaintext.expose_secret());
        assert!(unprotect_current_user(&protected, &wrong_scope).is_err());
    }
}
