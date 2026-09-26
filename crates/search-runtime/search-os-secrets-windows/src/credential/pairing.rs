//! Immutable provider-generation credentials, separate from revision root keys.

use core::fmt;
use core::time::Duration;

use crate::SecretBytes;
use super::LegacyRevisionRootSecretError;

/// Closed credential failures. Unknown writes require exact readback, not a new key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderPairingCredentialError {
    /// No platform credential store exists on this target; no memory fallback.
    UnsupportedPlatform,
    /// The caller's original cancellation/deadline budget is exhausted.
    Interrupted,
    /// A pairing key must contain exactly 32 nonzero-as-a-whole bytes.
    InvalidKey,
    /// Another key already occupies this immutable registration generation.
    KeyConflict,
    /// The per-registration native mutex could not be created/opened or waited on.
    LockUnavailable,
    /// Original side-effect-free platform read failure.
    Read(LegacyRevisionRootSecretError),
    /// A write was dispatched, but exact readback/budget completion failed.
    /// The optional original platform error is retained; None denotes interrupted,
    /// absent or contradictory readback. Never infer rollback from this error.
    WriteOutcomeUnknown(Option<LegacyRevisionRootSecretError>),
}

impl ProviderPairingCredentialError {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM",
            Self::Interrupted => "PAIRING_CREDENTIAL_INTERRUPTED",
            Self::InvalidKey => "PAIRING_CREDENTIAL_KEY_INVALID",
            Self::KeyConflict => "PAIRING_CREDENTIAL_CONFLICT",
            Self::LockUnavailable => "PAIRING_CREDENTIAL_LOCK_UNAVAILABLE",
            Self::Read(_) => "PAIRING_CREDENTIAL_READ_FAILED",
            Self::WriteOutcomeUnknown(_) => "PAIRING_CREDENTIAL_WRITE_OUTCOME_UNKNOWN",
        }
    }

    /// Whether a possible native write requires authoritative readback.
    #[must_use]
    pub const fn outcome_unknown(self) -> bool {
        matches!(self, Self::WriteOutcomeUnknown(_))
    }
}

impl fmt::Display for ProviderPairingCredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.code()) }
}
impl std::error::Error for ProviderPairingCredentialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Read(error) | Self::WriteOutcomeUnknown(Some(error)) => Some(error), _ => None }
    }
}

/// Read exactly one current-user provider key; absence never triggers creation.
///
/// `locator` is the daemon's domain-separated digest of the exact installation,
/// incarnation, binding, peer, role and pairing generation, never a credential
/// name supplied by a client. The platform adds its own closed target prefix.
/// `remaining` must observe the original monotonic deadline and cancellation;
/// `None` or less than one millisecond refuses work. Checks surround native calls,
/// which cannot themselves be preempted. `SecretBytes` clears the returned key.
/// This effect neither authenticates a peer nor reads a binding/policy database.
pub fn load_provider_pairing_credential(
    locator: &[u8; 32],
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<Option<SecretBytes>, ProviderPairingCredentialError> {
    #[cfg(windows)]
    { native::load(locator, remaining) }
    #[cfg(not(windows))]
    { let _ = (locator, remaining); Err(ProviderPairingCredentialError::UnsupportedPlatform) }
}

/// Publish one immutable generation with exact readback and no replacement.
///
/// The candidate must already be retained by the native administrative operation.
/// Generate it once using qualified entropy, never again after an uncertain write.
/// An equal existing key is idempotent success; a different key is a conflict.
/// Rotation uses a NEW pairing-generation locator, not overwrite. The same
/// cross-session named mutex protects the absence check, write and readback.
/// This API performs at most one write and never repairs/deletes other entries.
/// This serializes participating ELIOT writers, not arbitrary external `CredWrite`
/// calls. The root owner still coordinates exclusivity, recovery and revocation.
/// The callback contract and native-call limitation match the read operation.
pub fn publish_provider_pairing_credential(
    locator: &[u8; 32],
    candidate: &SecretBytes,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<(), ProviderPairingCredentialError> {
    #[cfg(windows)]
    { native::publish(locator, candidate, remaining) }
    #[cfg(not(windows))]
    { let _ = (locator, candidate, remaining); Err(ProviderPairingCredentialError::UnsupportedPlatform) }
}

#[cfg(windows)]
mod native {
    use super::{Duration, ProviderPairingCredentialError as Error, SecretBytes};
    use super::super::{LegacyRevisionRootSecret, RootSecretPlatform, constant_time_equal};
    use super::super::windows::{WindowsCredentialPlatform, WindowsVaultLock};

    const PREFIX: &str = "ELIOT Search/provider-pairing/v1/";
    const MUTEX_PREFIX: &str = "Global\\ELIOT-Search-ProviderPairing-v1-";

    fn checkpoint(remaining: &mut dyn FnMut() -> Option<Duration>) -> Result<u32, Error> {
        let millis = remaining().ok_or(Error::Interrupted)?.as_millis();
        if millis == 0 { return Err(Error::Interrupted); }
        // Never round up past the original caller budget or wait indefinitely.
        u32::try_from(millis.min(25)).map_err(|_| Error::Interrupted)
    }

    fn name(prefix: &str, locator: &[u8; 32]) -> Vec<u16> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(prefix.len() + 64);
        text.push_str(prefix);
        for byte in locator {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 15)]));
        }
        text.encode_utf16().chain(core::iter::once(0)).collect()
    }

    fn valid_key(bytes: &[u8]) -> Result<&[u8; 32], Error> {
        let key: &[u8; 32] = bytes.try_into().map_err(|_| Error::InvalidKey)?;
        if key.iter().all(|byte| *byte == 0) { return Err(Error::InvalidKey); }
        Ok(key)
    }

    pub(super) fn load(
        locator: &[u8; 32],
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<SecretBytes>, Error> {
        checkpoint(remaining)?;
        let observed = WindowsCredentialPlatform.read_credential(&name(PREFIX, locator))
            .map_err(Error::Read)?;
        checkpoint(remaining)?;
        // Reuse the existing validated 32-byte credential allocation owner.
        // No secret-bearing Vec escapes without the existing zeroizing wrapper.
        let secret = observed.map(|value| {
            let bytes = valid_key(value.expose_secret())?;
            SecretBytes::new(bytes.to_vec()).map_err(|_| Error::InvalidKey)
        }).transpose()?;
        checkpoint(remaining)?;
        Ok(secret)
    }

    pub(super) fn publish(
        locator: &[u8; 32],
        candidate: &SecretBytes,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<(), Error> {
        checkpoint(remaining)?;
        let bytes = valid_key(candidate.expose_secret())?;
        let target = name(PREFIX, locator);
        let mutex = name(MUTEX_PREFIX, locator);
        let _lock = loop {
            let wait = checkpoint(remaining)?;
            if let Some(lock) = WindowsVaultLock::acquire_named(&mutex, wait)
                .map_err(|()| Error::LockUnavailable)? { break lock; }
        };
        checkpoint(remaining)?;
        let mut platform = WindowsCredentialPlatform;
        let prior = platform.read_credential(&target).map_err(Error::Read)?;
        checkpoint(remaining)?;
        if let Some(prior) = prior {
            return if constant_time_equal(bytes, prior.expose_secret()) { Ok(()) }
                else { Err(Error::KeyConflict) };
        }
        let mut secret = LegacyRevisionRootSecret::from_bytes(*bytes);
        checkpoint(remaining)?;
        // After dispatch even an error is potentially mutating. Do not retry or
        // downgrade a later deadline/cancellation failure to pre-write refusal.
        platform.write_credential(&target, &mut secret)
            .map_err(|error| Error::WriteOutcomeUnknown(Some(error)))?;
        checkpoint(remaining).map_err(|_| Error::WriteOutcomeUnknown(None))?;
        let observed = platform.read_credential(&target)
            .map_err(|error| Error::WriteOutcomeUnknown(Some(error)))?;
        checkpoint(remaining).map_err(|_| Error::WriteOutcomeUnknown(None))?;
        if !observed.is_some_and(|value| constant_time_equal(bytes, value.expose_secret())) {
            return Err(Error::WriteOutcomeUnknown(None));
        }
        Ok(())
    }
}
