//! Windows Credential Manager ownership for revision and provider-pairing keys.
//!
//! The legacy revision caller supplies the namespace identity and whether protected
//! revision objects already exist. This module owns target construction,
//! current-user credential read/create/readback, CSPRNG generation and the
//! cross-process vault lock. It owns no filesystem or source/catalog policy.

use core::fmt;
use core::time::Duration;

use super::model::clear_bytes;

/// Exact legacy revision platform root-secret byte length.
pub const LEGACY_REVISION_ROOT_SECRET_BYTES: usize = 32;

#[cfg(windows)]
mod windows;

mod pairing;
pub use pairing::{
    ProviderPairingCredential, ProviderPairingCredentialError,
    ProviderPairingCredentialIntent, load_provider_pairing_credential,
    load_provider_pairing_credential_record, publish_provider_pairing_credential,
};

#[cfg(test)]
mod tests;

const CREDENTIAL_TARGET_PREFIX: &str = "ELIOT Search/revision-key/";
const ROOT_SECRET_ATTEMPTS: u32 = 8;
const RETRY_BASE_MILLIS: u64 = 10;

/// Whether a missing namespace credential may be created.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionRootSecretRequirement {
    /// No protected revision object exists, so a missing root secret may be created.
    CreateIfMissing,
    /// Protected revision objects exist and therefore require the original key.
    RequireExisting,
}

/// Zeroizing legacy revision root secret returned by the Windows credential owner.
///
/// This type intentionally does not implement `Clone`, `AsRef`, `Deref`, or any
/// serialization trait. Callers must explicitly expose the exact fixed-size
/// bytes to the immediate derivation boundary.
pub struct LegacyRevisionRootSecret {
    bytes: [u8; LEGACY_REVISION_ROOT_SECRET_BYTES],
}

impl LegacyRevisionRootSecret {
    fn from_bytes(bytes: [u8; LEGACY_REVISION_ROOT_SECRET_BYTES]) -> Self {
        Self { bytes }
    }

    /// Exposes the fixed-size root secret to the immediate key-derivation boundary.
    #[must_use]
    pub const fn expose_secret(&self) -> &[u8; LEGACY_REVISION_ROOT_SECRET_BYTES] {
        &self.bytes
    }

    fn expose_secret_mut(
        &mut self,
    ) -> &mut [u8; LEGACY_REVISION_ROOT_SECRET_BYTES] {
        &mut self.bytes
    }
}

impl fmt::Debug for LegacyRevisionRootSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacyRevisionRootSecret")
            .field("bytes", &"<redacted>")
            .field("length", &LEGACY_REVISION_ROOT_SECRET_BYTES)
            .finish()
    }
}

impl Drop for LegacyRevisionRootSecret {
    fn drop(&mut self) {
        clear_bytes(&mut self.bytes);
    }
}

/// Closed failure from the legacy revision Credential Manager owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionRootSecretError {
    /// The adapter was called on a non-Windows target.
    UnsupportedPlatform,
    /// `CredReadW` failed with the captured `GetLastError` code.
    CredentialReadFailed(u32),
    /// The credential record did not match the exact expected type, persistence, or size.
    CredentialReadbackInvalid,
    /// `CredWriteW` failed with the captured `GetLastError` code.
    CredentialWriteFailed(u32),
    /// The credential blob length could not be represented by the Windows ABI.
    CredentialTooLarge,
    /// A successful write read back different secret bytes.
    CredentialReadbackMismatch,
    /// Bounded write/readback retries ended without a verified credential.
    CredentialWriteOutcomeUnknown,
    /// Existing protected objects require a credential that remained absent.
    MissingExistingCredential,
    /// `BCryptGenRandom` failed with the returned NTSTATUS value.
    RandomGenerationFailed(i32),
}

impl LegacyRevisionRootSecretError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "WINDOWS_REVISION_KEY_UNSUPPORTED_PLATFORM",
            Self::CredentialReadFailed(_) => "WINDOWS_REVISION_KEY_READ_FAILED",
            Self::CredentialReadbackInvalid => {
                "WINDOWS_REVISION_KEY_READBACK_INVALID"
            }
            Self::CredentialWriteFailed(_) => "WINDOWS_REVISION_KEY_WRITE_FAILED",
            Self::CredentialTooLarge => "WINDOWS_REVISION_KEY_TOO_LARGE",
            Self::CredentialReadbackMismatch => {
                "WINDOWS_REVISION_KEY_READBACK_MISMATCH"
            }
            Self::CredentialWriteOutcomeUnknown => {
                "WINDOWS_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"
            }
            Self::MissingExistingCredential => "WINDOWS_REVISION_KEY_MISSING",
            Self::RandomGenerationFailed(_) => "WINDOWS_REVISION_RNG_FAILED",
        }
    }

    const fn is_transient_vault_outcome(self) -> bool {
        matches!(
            self,
            Self::CredentialReadFailed(_)
                | Self::CredentialWriteFailed(_)
                | Self::CredentialWriteOutcomeUnknown
        )
    }
}

impl fmt::Display for LegacyRevisionRootSecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CredentialReadFailed(code)
            | Self::CredentialWriteFailed(code) => {
                write!(formatter, "{}:{code}", self.code())
            }
            Self::RandomGenerationFailed(status) => {
                write!(formatter, "{}:{status}", self.code())
            }
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for LegacyRevisionRootSecretError {}

/// Reads the existing namespace root secret without creating or mutating it.
#[cfg(windows)]
pub fn load_existing_legacy_revision_root_secret(
    namespace_id: &[u8; 32],
) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError> {
    let mut platform = windows::WindowsCredentialPlatform;
    platform.read_credential(&credential_target(namespace_id))
}

/// Reads the existing namespace root secret without creating or mutating it.
#[cfg(not(windows))]
pub fn load_existing_legacy_revision_root_secret(
    _namespace_id: &[u8; 32],
) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError> {
    Err(LegacyRevisionRootSecretError::UnsupportedPlatform)
}

/// Loads or creates one namespace root secret under the exact bounded policy.
///
/// A missing secret is created only when the caller has already established
/// [`LegacyRevisionRootSecretRequirement::CreateIfMissing`]. Every candidate
/// write is serialized by the package-owned named mutex and rechecks the
/// credential after acquiring that lock, so a concurrently published key is
/// never overwritten by a stale generated candidate.
#[cfg(windows)]
pub fn load_or_create_legacy_revision_root_secret(
    namespace_id: &[u8; 32],
    requirement: LegacyRevisionRootSecretRequirement,
) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError> {
    let mut platform = windows::WindowsCredentialPlatform;
    load_or_create_with_platform(&mut platform, namespace_id, requirement)
}

/// Loads or creates one namespace root secret under the exact bounded policy.
#[cfg(not(windows))]
pub fn load_or_create_legacy_revision_root_secret(
    _namespace_id: &[u8; 32],
    _requirement: LegacyRevisionRootSecretRequirement,
) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError> {
    Err(LegacyRevisionRootSecretError::UnsupportedPlatform)
}

trait RootSecretPlatform {
    type VaultGuard;

    fn read_credential(
        &mut self,
        target: &[u16],
    ) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError>;

    fn write_credential(
        &mut self,
        target: &[u16],
        secret: &mut LegacyRevisionRootSecret,
    ) -> Result<(), LegacyRevisionRootSecretError>;

    fn generate_root_secret(
        &mut self,
    ) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError>;

    fn acquire_vault_lock(&mut self) -> Result<Self::VaultGuard, ()>;

    fn sleep(&mut self, duration: Duration);
}

fn load_or_create_with_platform<P: RootSecretPlatform>(
    platform: &mut P,
    namespace_id: &[u8; 32],
    requirement: LegacyRevisionRootSecretRequirement,
) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError> {
    let target = credential_target(namespace_id);
    if let Some(secret) = platform.read_credential(&target)? {
        return Ok(secret);
    }

    match requirement {
        LegacyRevisionRootSecretRequirement::RequireExisting => {
            for attempt in 0..ROOT_SECRET_ATTEMPTS {
                platform.sleep(retry_delay(attempt));
                let Ok(_guard) = platform.acquire_vault_lock() else {
                    continue;
                };
                if let Some(secret) = platform.read_credential(&target)? {
                    return Ok(secret);
                }
            }
            Err(LegacyRevisionRootSecretError::MissingExistingCredential)
        }
        LegacyRevisionRootSecretRequirement::CreateIfMissing => {
            let mut generated = platform.generate_root_secret()?;
            for attempt in 0..ROOT_SECRET_ATTEMPTS {
                if attempt > 0 {
                    platform.sleep(retry_delay(attempt));
                }
                let Ok(_guard) = platform.acquire_vault_lock() else {
                    continue;
                };

                // Another process may have created the credential after our
                // initial read but before this lock acquisition. Re-read while
                // serialized and adopt that key instead of overwriting it.
                match platform.read_credential(&target) {
                    Ok(Some(existing)) => return Ok(existing),
                    Ok(None) => {}
                    Err(error) if error.is_transient_vault_outcome() => continue,
                    Err(error) => return Err(error),
                }

                if let Err(error) =
                    platform.write_credential(&target, &mut generated)
                {
                    if error.is_transient_vault_outcome() {
                        continue;
                    }
                    return Err(error);
                }
                match platform.read_credential(&target) {
                    Ok(Some(observed)) => {
                        if !constant_time_equal(
                            generated.expose_secret(),
                            observed.expose_secret(),
                        ) {
                            return Err(
                                LegacyRevisionRootSecretError::CredentialReadbackMismatch,
                            );
                        }
                        return Ok(generated);
                    }
                    Ok(None) => {}
                    Err(error) if error.is_transient_vault_outcome() => {}
                    Err(error) => return Err(error),
                }
            }
            Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
        }
    }
}

fn credential_target(namespace_id: &[u8; 32]) -> Vec<u16> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut target = String::with_capacity(CREDENTIAL_TARGET_PREFIX.len() + 64);
    target.push_str(CREDENTIAL_TARGET_PREFIX);
    for byte in namespace_id {
        let byte = *byte;
        target.push(char::from(HEX[usize::from(byte >> 4)]));
        target.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    target.encode_utf16().chain(core::iter::once(0)).collect()
}

fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BASE_MILLIS << attempt.min(5))
}

fn constant_time_equal(
    left: &[u8; LEGACY_REVISION_ROOT_SECRET_BYTES],
    right: &[u8; LEGACY_REVISION_ROOT_SECRET_BYTES],
) -> bool {
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}
