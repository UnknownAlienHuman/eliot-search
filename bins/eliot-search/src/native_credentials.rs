//! Platform-owned provider pairing credential lookup for the standalone client.
//!
//! The caller supplies only the canonical locator digest derived from an
//! independently trusted registration expectation and authenticated endpoint
//! descriptor. On Windows this adapter performs one bounded current-user
//! Credential Manager read through `search-os-secrets-windows`. Absence remains
//! absence; no key is generated, adopted, replaced, deleted, read from a file or
//! accepted through argv/environment input.

#![allow(dead_code)]

use core::fmt;
use std::time::Duration;

use search_provider_protocol::BindingKey;

use crate::native_registered::NativePairingKeySource;

/// Concrete platform owner for one immutable provider pairing credential.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlatformNativePairingKeySource;

/// Closed, content-free failure from the platform credential owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlatformNativePairingKeyError {
    /// No qualified platform credential owner exists on this target.
    UnsupportedPlatform,
    /// Windows Credential Manager refused or could not complete the exact read.
    #[cfg(windows)]
    Credential(search_os_secrets_windows::ProviderPairingCredentialError),
    /// The stored credential did not contain one valid nonzero 32-byte key.
    InvalidKey,
}

impl PlatformNativePairingKeyError {
    /// Stable content-free reason code.
    #[must_use]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "NATIVE_CLIENT_CREDENTIAL_UNSUPPORTED_PLATFORM",
            #[cfg(windows)]
            Self::Credential(error) => error.code(),
            Self::InvalidKey => "NATIVE_CLIENT_CREDENTIAL_KEY_INVALID",
        }
    }
}

impl fmt::Display for PlatformNativePairingKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PlatformNativePairingKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        #[cfg(windows)]
        if let Self::Credential(error) = self {
            return Some(error);
        }
        None
    }
}

impl NativePairingKeySource for PlatformNativePairingKeySource {
    type Error = PlatformNativePairingKeyError;

    fn load_key(
        &mut self,
        locator: &[u8; 32],
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<BindingKey>, Self::Error> {
        #[cfg(windows)]
        {
            let secret = search_os_secrets_windows::load_provider_pairing_credential(
                locator,
                remaining,
            )
            .map_err(PlatformNativePairingKeyError::Credential)?;
            secret.map(binding_key).transpose()
        }
        #[cfg(not(windows))]
        {
            let _ = (locator, remaining);
            Err(PlatformNativePairingKeyError::UnsupportedPlatform)
        }
    }
}

#[cfg(windows)]
fn binding_key(
    secret: search_os_secrets_windows::SecretBytes,
) -> Result<BindingKey, PlatformNativePairingKeyError> {
    let observed = secret.expose_secret();
    let mut bytes = [0_u8; 32];
    if observed.len() != bytes.len() {
        return Err(PlatformNativePairingKeyError::InvalidKey);
    }
    bytes.copy_from_slice(observed);
    BindingKey::from_bytes(bytes).map_err(|_| PlatformNativePairingKeyError::InvalidKey)
}
