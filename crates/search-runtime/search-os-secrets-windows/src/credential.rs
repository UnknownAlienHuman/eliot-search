//! Windows Credential Manager ownership for revision and provider-pairing keys.
//!
//! The legacy revision caller supplies the namespace identity and whether protected
//! revision objects already exist. This module owns target construction,
//! current-user credential read/create/readback, CSPRNG generation and the
//! cross-process vault lock. It owns no filesystem or source/catalog policy.

use core::fmt;
use core::time::Duration;

use super::model::clear_bytes;

mod context;
use context::{LegacyCheckpoint, RootSecretCheckpoint};
pub use context::{
    LegacyRevisionRootSecretContextError, load_existing_legacy_revision_root_secret_with_context,
    load_or_create_legacy_revision_root_secret_with_context,
};

/// Exact legacy revision platform root-secret byte length.
pub const LEGACY_REVISION_ROOT_SECRET_BYTES: usize = 32;

#[cfg(windows)]
mod windows;

#[cfg(feature = "test-credential-cleanup")]
pub(super) fn qualification_trace(stage: &str, status: i64) {
    if std::env::var_os("ELIOT_SEARCH_TEST_NATIVE_PHASE_TRACE").is_some() {
        eprintln!("native-credential:{stage}:{status}");
    }
}

mod pairing;
pub use pairing::{
    ProviderPairingCredential, ProviderPairingCredentialError, ProviderPairingCredentialIntent,
    load_provider_pairing_credential, load_provider_pairing_credential_record,
    publish_provider_pairing_credential,
};

mod qdrant;
pub use qdrant::{
    QdrantApiSecretLease, QdrantApiSecretPurpose, QdrantApiSecretReference,
    QdrantApiSecretStore, QdrantApiSecretStoreError,
};

mod revision_dek;
pub use revision_dek::*;
mod seal_open;
pub use seal_open::*;

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

    fn expose_secret_mut(&mut self) -> &mut [u8; LEGACY_REVISION_ROOT_SECRET_BYTES] {
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
    /// Acquisition exhausted its bound or a possible publication was not verified.
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
            Self::CredentialReadbackInvalid => "WINDOWS_REVISION_KEY_READBACK_INVALID",
            Self::CredentialWriteFailed(_) => "WINDOWS_REVISION_KEY_WRITE_FAILED",
            Self::CredentialTooLarge => "WINDOWS_REVISION_KEY_TOO_LARGE",
            Self::CredentialReadbackMismatch => "WINDOWS_REVISION_KEY_READBACK_MISMATCH",
            Self::CredentialWriteOutcomeUnknown => "WINDOWS_REVISION_KEY_WRITE_OUTCOME_UNKNOWN",
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
            Self::CredentialReadFailed(code) | Self::CredentialWriteFailed(code) => {
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
    read_with_platform(
        &mut platform,
        &credential_target(namespace_id),
        &LegacyCheckpoint,
    )
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
/// invocation attempts at most one write, serialized by the named mutex, after rechecking the
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
    load_or_create_checked(platform, namespace_id, requirement, &LegacyCheckpoint)
}

fn read_with_platform<P: RootSecretPlatform, C: RootSecretCheckpoint>(
    platform: &mut P,
    target: &[u16],
    context: &C,
) -> Result<Option<LegacyRevisionRootSecret>, C::Error> {
    context.check()?;
    let observed = platform.read_credential(target);
    context.check()?;
    observed.map_err(C::Error::from)
}

fn sleep_with_platform<P: RootSecretPlatform, C: RootSecretCheckpoint>(
    platform: &mut P,
    duration: Duration,
    context: &C,
) -> Result<(), C::Error> {
    context.check()?;
    platform.sleep(context.limit_sleep(duration));
    context.check()
}

fn load_or_create_checked<P: RootSecretPlatform, C: RootSecretCheckpoint>(
    platform: &mut P,
    namespace_id: &[u8; 32],
    requirement: LegacyRevisionRootSecretRequirement,
    context: &C,
) -> Result<LegacyRevisionRootSecret, C::Error> {
    context.check()?;
    let target = credential_target(namespace_id);
    if let Some(secret) = read_with_platform(platform, &target, context)? {
        context.check()?;
        return Ok(secret);
    }

    match requirement {
        LegacyRevisionRootSecretRequirement::RequireExisting => {
            for attempt in 0..ROOT_SECRET_ATTEMPTS {
                sleep_with_platform(platform, retry_delay(attempt), context)?;
                context.check()?;
                let acquired = platform.acquire_vault_lock();
                context.check()?;
                let Ok(guard) = acquired else {
                    continue;
                };
                if let Some(secret) = read_with_platform(platform, &target, context)? {
                    drop(guard);
                    context.check()?;
                    return Ok(secret);
                }
            }
            context.check()?;
            Err(LegacyRevisionRootSecretError::MissingExistingCredential.into())
        }
        LegacyRevisionRootSecretRequirement::CreateIfMissing => {
            context.check()?;
            let candidate = platform.generate_root_secret();
            context.check()?;
            let mut generated = candidate.map_err(C::Error::from)?;
            for attempt in 0..ROOT_SECRET_ATTEMPTS {
                if attempt > 0 {
                    sleep_with_platform(platform, retry_delay(attempt), context)?;
                }
                context.check()?;
                let acquired = platform.acquire_vault_lock();
                context.check()?;
                let Ok(guard) = acquired else {
                    continue;
                };

                // Another process may have created the credential after our
                // initial read but before this lock acquisition. Re-read while
                // serialized and adopt that key instead of overwriting it.
                context.check()?;
                let observed = platform.read_credential(&target);
                context.check()?;
                match observed {
                    Ok(Some(existing)) => {
                        drop(guard);
                        context.check()?;
                        return Ok(existing);
                    }
                    Ok(None) => {}
                    Err(error) if error.is_transient_vault_outcome() => continue,
                    Err(error) => return Err(error.into()),
                }

                // Publication is terminal: after dispatch, no path re-enters the
                // acquisition loop, regardless of the write/readback outcome.
                publish_once(platform, &target, &mut generated, context)?;
                drop(guard);
                context.check().map_err(|_| {
                    C::Error::from(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
                })?;
                return Ok(generated);
            }
            context.check()?;
            Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown.into())
        }
    }
}

fn publish_once<P: RootSecretPlatform, C: RootSecretCheckpoint>(
    platform: &mut P,
    target: &[u16],
    generated: &mut LegacyRevisionRootSecret,
    context: &C,
) -> Result<(), C::Error> {
    context.check()?;
    // Calling the native seam crosses the possible-effect boundary, even when
    // it reports failure. All subsequent failures conservatively remain unknown.
    let written = platform.write_credential(target, generated);
    let checked = context.check();
    if written.is_err() || checked.is_err() {
        return Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown.into());
    }
    context.check().map_err(|_| {
        C::Error::from(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
    })?;
    let observed = platform.read_credential(target);
    let checked = context.check();
    #[cfg(feature = "test-credential-cleanup")]
    qualification_trace("readback-shape", match &observed { Ok(Some(_)) => 1, Ok(None) => 0, Err(_) => -1 });
    if checked.is_err() {
        return Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown.into());
    }
    match observed {
        Ok(Some(observed))
            if constant_time_equal(generated.expose_secret(), observed.expose_secret()) => {}
        _ => return Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown.into()),
    }
    context
        .check()
        .map_err(|_| C::Error::from(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown))
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
