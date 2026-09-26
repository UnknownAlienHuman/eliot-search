//! Immutable provider-generation credentials, separate from revision root keys.

use core::fmt;
use core::time::Duration;

use crate::SecretBytes;
use super::LegacyRevisionRootSecretError;

const RECORD_MAGIC: &[u8; 8] = b"ELPPCR01";
const RECORD_BYTES: usize = 8 + 32 + 32 + 8 + 32;

/// Stable non-secret identity of the exact administrative registration command.
///
/// This record is stored beside the provider key in Credential Manager so a
/// restart can distinguish recovery of the original command from a new key for
/// different input. It contains no binding key, peer name or source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderPairingCredentialIntent {
    operation_id: [u8; 32],
    command_digest: [u8; 32],
    expected_generation: u64,
}

impl ProviderPairingCredentialIntent {
    /// Creates an intent from one immutable journal operation.
    #[must_use]
    pub const fn new(
        operation_id: [u8; 32],
        command_digest: [u8; 32],
        expected_generation: u64,
    ) -> Self {
        Self { operation_id, command_digest, expected_generation }
    }

    /// Exact operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] { self.operation_id }

    /// Digest of the exact registration command.
    #[must_use]
    pub const fn command_digest(self) -> [u8; 32] { self.command_digest }

    /// Journal generation expected by the command.
    #[must_use]
    pub const fn expected_generation(self) -> u64 { self.expected_generation }
}

/// One validated provider key and the exact command identity that published it.
///
/// The key remains behind [`SecretBytes`] and is cleared on drop. Debug output is
/// redacted. This value grants no binding, pairing or source authority.
pub struct ProviderPairingCredential {
    intent: ProviderPairingCredentialIntent,
    secret: SecretBytes,
}

impl ProviderPairingCredential {
    /// Non-secret command identity stored with this key.
    #[must_use]
    pub const fn intent(&self) -> ProviderPairingCredentialIntent { self.intent }

    /// Explicit immediate access to the zeroizing key owner.
    #[must_use]
    pub const fn secret(&self) -> &SecretBytes { &self.secret }

    /// Consumes the record and returns only its zeroizing key owner.
    #[must_use]
    pub fn into_secret(self) -> SecretBytes { self.secret }
}

impl fmt::Debug for ProviderPairingCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderPairingCredential")
            .field("intent", &self.intent)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// Closed credential failures. Unknown writes require exact readback, not a new key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderPairingCredentialError {
    /// No platform credential store exists on this target; no memory fallback.
    UnsupportedPlatform,
    /// The caller's original cancellation/deadline budget is exhausted.
    Interrupted,
    /// A pairing key must contain exactly 32 nonzero-as-a-whole bytes.
    InvalidKey,
    /// Stored provider bytes are malformed, truncated or use another version.
    InvalidRecord,
    /// Another key or command intent already occupies this immutable generation.
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
            Self::InvalidRecord => "PAIRING_CREDENTIAL_RECORD_INVALID",
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
/// The versioned credential record and its stored command identity are validated,
/// but this convenience entry returns only the key for opening a currently
/// published binding. Recovery code should use
/// [`load_provider_pairing_credential_record`] and compare the intent.
pub fn load_provider_pairing_credential(
    locator: &[u8; 32],
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<Option<SecretBytes>, ProviderPairingCredentialError> {
    load_provider_pairing_credential_record(locator, remaining)
        .map(|value| value.map(ProviderPairingCredential::into_secret))
}

/// Reads one provider key together with its durable administrative intent.
///
/// `locator` is a daemon-derived digest, never a credential name supplied by a
/// peer. The callback observes the original monotonic deadline/cancellation;
/// native calls themselves cannot be preempted. Absence is returned explicitly.
pub fn load_provider_pairing_credential_record(
    locator: &[u8; 32],
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<Option<ProviderPairingCredential>, ProviderPairingCredentialError> {
    #[cfg(windows)]
    { native::load(locator, remaining) }
    #[cfg(not(windows))]
    { let _ = (locator, remaining); Err(ProviderPairingCredentialError::UnsupportedPlatform) }
}

/// Publish one immutable generation with exact intent/key readback and no replacement.
///
/// The candidate must already be retained by the native administrative operation.
/// Equal existing intent and key are idempotent success; any difference is a
/// conflict. The same cross-session mutex protects absence check, one write and
/// readback. Post-dispatch interruption remains outcome-unknown. No key is
/// generated, adopted, replaced or deleted by this operation.
pub fn publish_provider_pairing_credential(
    locator: &[u8; 32],
    intent: ProviderPairingCredentialIntent,
    candidate: &SecretBytes,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<(), ProviderPairingCredentialError> {
    #[cfg(windows)]
    { native::publish(locator, intent, candidate, remaining) }
    #[cfg(not(windows))]
    { let _ = (locator, intent, candidate, remaining); Err(ProviderPairingCredentialError::UnsupportedPlatform) }
}

#[cfg(windows)]
mod native {
    use super::{
        Duration, ProviderPairingCredential, ProviderPairingCredentialError as Error,
        ProviderPairingCredentialIntent, RECORD_BYTES, RECORD_MAGIC, SecretBytes,
    };
    use super::super::windows::{ProviderCredentialBlob, WindowsCredentialPlatform, WindowsVaultLock};

    const PREFIX: &str = "ELIOT Search/provider-pairing/v1/";
    const MUTEX_PREFIX: &str = "Global\\ELIOT-Search-ProviderPairing-v1-";

    fn checkpoint(remaining: &mut dyn FnMut() -> Option<Duration>) -> Result<u32, Error> {
        let millis = remaining().ok_or(Error::Interrupted)?.as_millis();
        if millis == 0 { return Err(Error::Interrupted); }
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

    fn encode(
        intent: ProviderPairingCredentialIntent,
        candidate: &SecretBytes,
    ) -> Result<ProviderCredentialBlob, Error> {
        let key = valid_key(candidate.expose_secret())?;
        let mut bytes = Vec::with_capacity(RECORD_BYTES);
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&intent.operation_id());
        bytes.extend_from_slice(&intent.command_digest());
        bytes.extend_from_slice(&intent.expected_generation().to_be_bytes());
        bytes.extend_from_slice(key);
        ProviderCredentialBlob::new(bytes).map_err(|_| Error::InvalidRecord)
    }

    fn decode(blob: &ProviderCredentialBlob) -> Result<ProviderPairingCredential, Error> {
        let bytes = blob.as_slice();
        if bytes.len() != RECORD_BYTES || bytes.get(..8) != Some(RECORD_MAGIC) {
            return Err(Error::InvalidRecord);
        }
        let operation_id = bytes[8..40].try_into().map_err(|_| Error::InvalidRecord)?;
        let command_digest = bytes[40..72].try_into().map_err(|_| Error::InvalidRecord)?;
        let expected_generation = u64::from_be_bytes(
            bytes[72..80].try_into().map_err(|_| Error::InvalidRecord)?,
        );
        let key = valid_key(&bytes[80..112])?;
        let secret = SecretBytes::new(key.to_vec()).map_err(|_| Error::InvalidKey)?;
        Ok(ProviderPairingCredential {
            intent: ProviderPairingCredentialIntent::new(
                operation_id, command_digest, expected_generation,
            ),
            secret,
        })
    }

    fn same_record(left: &ProviderCredentialBlob, right: &ProviderCredentialBlob) -> bool {
        if left.as_slice().len() != right.as_slice().len() { return false; }
        left.as_slice().iter().zip(right.as_slice()).fold(0_u8, |difference, (a, b)| {
            difference | (a ^ b)
        }) == 0
    }

    pub(super) fn load(
        locator: &[u8; 32],
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<ProviderPairingCredential>, Error> {
        checkpoint(remaining)?;
        let observed = WindowsCredentialPlatform.read_provider_credential(&name(PREFIX, locator))
            .map_err(Error::Read)?;
        checkpoint(remaining)?;
        let record = observed.as_ref().map(decode).transpose()?;
        checkpoint(remaining)?;
        Ok(record)
    }

    pub(super) fn publish(
        locator: &[u8; 32],
        intent: ProviderPairingCredentialIntent,
        candidate: &SecretBytes,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<(), Error> {
        checkpoint(remaining)?;
        let mut expected = encode(intent, candidate)?;
        let target = name(PREFIX, locator);
        let mutex = name(MUTEX_PREFIX, locator);
        let _lock = loop {
            let wait = checkpoint(remaining)?;
            if let Some(lock) = WindowsVaultLock::acquire_named(&mutex, wait)
                .map_err(|()| Error::LockUnavailable)? { break lock; }
        };
        checkpoint(remaining)?;
        let mut platform = WindowsCredentialPlatform;
        let prior = platform.read_provider_credential(&target).map_err(Error::Read)?;
        checkpoint(remaining)?;
        if let Some(prior) = prior {
            return if same_record(&expected, &prior) { Ok(()) } else { Err(Error::KeyConflict) };
        }
        checkpoint(remaining)?;
        platform.write_provider_credential(&target, &mut expected)
            .map_err(|error| Error::WriteOutcomeUnknown(Some(error)))?;
        checkpoint(remaining).map_err(|_| Error::WriteOutcomeUnknown(None))?;
        let observed = platform.read_provider_credential(&target)
            .map_err(|error| Error::WriteOutcomeUnknown(Some(error)))?;
        checkpoint(remaining).map_err(|_| Error::WriteOutcomeUnknown(None))?;
        if !observed.as_ref().is_some_and(|value| same_record(&expected, value)) {
            return Err(Error::WriteOutcomeUnknown(None));
        }
        Ok(())
    }
}
