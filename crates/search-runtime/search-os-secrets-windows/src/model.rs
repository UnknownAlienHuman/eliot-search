//! Finite public records and failures for the Windows DPAPI adapter.

use core::fmt;

/// Maximum plaintext accepted by one short-secret operation.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
/// Maximum serialized scope entropy accepted by one short-secret operation.
pub const MAX_SCOPE_BYTES: usize = 4 * 1024;
/// Maximum ciphertext accepted from persistence or returned for one short secret.
pub const MAX_PROTECTED_BYTES: usize = 1024 * 1024;
/// Current short-secret protected-blob envelope version.
pub const PROTECTED_SECRET_VERSION: u16 = 1;
/// Maximum legacy revision inner/ciphertext bytes accepted by native DPAPI.
pub const MAX_LEGACY_REVISION_DPAPI_BYTES: usize = 65 * 1024 * 1024;
/// Exact legacy revision optional-entropy length.
pub const LEGACY_REVISION_DPAPI_ENTROPY_BYTES: usize = 32;

/// Closed failure from short-secret validation or the Windows DPAPI boundary.
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

/// Closed failure from the frozen legacy revision DPAPI effect boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionDpapiError {
    /// The adapter was called on a non-Windows target.
    UnsupportedPlatform,
    /// Input exceeded the finite compatibility ceiling or Windows ABI length.
    InputTooLarge,
    /// Native output exceeded the finite compatibility ceiling.
    OutputTooLarge,
    /// Windows returned an empty or invalid output pointer/length pair.
    InvalidPlatformOutput,
    /// `CryptProtectData` failed with the captured `GetLastError` code.
    ProtectFailed(u32),
    /// `CryptUnprotectData` failed with the captured `GetLastError` code.
    UnprotectFailed(u32),
}

impl LegacyRevisionDpapiError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => {
                "DPAPI_LEGACY_REVISION_UNSUPPORTED_PLATFORM"
            }
            Self::InputTooLarge => "DPAPI_LEGACY_REVISION_INPUT_TOO_LARGE",
            Self::OutputTooLarge => "DPAPI_LEGACY_REVISION_OUTPUT_TOO_LARGE",
            Self::InvalidPlatformOutput => {
                "DPAPI_LEGACY_REVISION_OUTPUT_INVALID"
            }
            Self::ProtectFailed(_) => "DPAPI_LEGACY_REVISION_PROTECT_FAILED",
            Self::UnprotectFailed(_) => {
                "DPAPI_LEGACY_REVISION_UNPROTECT_FAILED"
            }
        }
    }
}

impl fmt::Display for LegacyRevisionDpapiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProtectFailed(code) | Self::UnprotectFailed(code) => {
                write!(formatter, "{}:{code}", self.code())
            }
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for LegacyRevisionDpapiError {}

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
            let length =
                u32::try_from(bytes.len()).map_err(|_| DpapiError::LengthOverflow)?;
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
        clear_bytes(&mut self.bytes);
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

pub(super) fn validate_secret(bytes: &[u8]) -> Result<(), DpapiError> {
    if bytes.is_empty() {
        return Err(DpapiError::EmptySecret);
    }
    if bytes.len() > MAX_SECRET_BYTES {
        return Err(DpapiError::SecretTooLarge);
    }
    Ok(())
}

pub(super) fn clear_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: `byte` is a valid unique pointer to one initialized u8.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
