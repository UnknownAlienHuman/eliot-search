//! Native credential resolution for the exact registered peer and generation.

use search_contracts::protocol::PeerRole;
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingKey, TransportPeer};

use super::NativeBindingExpectation;

/// Content-free credential refusal or uncertain publication. No secret is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePairingCredentialError {
    code: &'static str,
    outcome_unknown: bool,
}

impl NativePairingCredentialError {
    /// Stable reason code; never contains a credential target or key.
    #[must_use]
    pub const fn code(self) -> &'static str { self.code }

    /// Whether the original candidate/locator must be retained for readback.
    #[must_use]
    pub const fn outcome_unknown(self) -> bool { self.outcome_unknown }

    const fn refused(code: &'static str) -> Self { Self { code, outcome_unknown: false } }
}
impl std::fmt::Display for NativePairingCredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.code) }
}
impl std::error::Error for NativePairingCredentialError {}

#[cfg(windows)]
impl From<search_os_secrets_windows::ProviderPairingCredentialError> for NativePairingCredentialError {
    fn from(error: search_os_secrets_windows::ProviderPairingCredentialError) -> Self {
        Self { code: error.code(), outcome_unknown: error.outcome_unknown() }
    }
}

impl NativeBindingExpectation {
    /// Resolve the existing key from Windows Credential Manager before pairing.
    ///
    /// Peer locators and this expectation must come from the native registration/
    /// identity owner. They are not grants or a verified BindingContext. The
    /// opener still validates current registration, profile, disclosure and proof.
    /// Missing/invalid keys never trigger generation, token-file or memory fallback.
    /// The returned key owns zeroization; native allocation and intermediate secret
    /// buffers remain with the existing platform owner.
    pub fn load_pairing_key<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        context: &OperationContext<C>,
    ) -> Result<BindingKey, NativePairingCredentialError> {
        let locator = self.credential_locator(peer)?;
        #[cfg(windows)]
        {
            let (started, deadline) = super::begin(context)
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_INTERRUPTED"))?;
            let mut remaining = || remaining(context, started, deadline);
            let secret = search_os_secrets_windows::load_provider_pairing_credential(&locator, &mut remaining)?
                .ok_or(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_MISSING"))?;
            let bytes = secret.expose_secret().try_into()
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_KEY_INVALID"))?;
            BindingKey::from_bytes(bytes)
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_KEY_INVALID"))
        }
        #[cfg(not(windows))]
        {
            let _ = (locator, context);
            Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM"))
        }
    }

    /// Publish a retained administrative candidate for one immutable generation.
    ///
    /// Supply a key generated ONCE by qualified OS entropy and retain it through
    /// any uncertain write. Matching readback is idempotent; a different stored
    /// key is never adopted or replaced. Rotation uses the successor generation.
    /// This is not binding/policy publication or a peer acknowledgement. Native
    /// administration retains the real root lock and coordinates the separate
    /// registration transaction and security barriers. This method generates no
    /// key and performs no automatic retry or credential deletion.
    pub fn publish_pairing_key<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        key: &BindingKey,
        context: &OperationContext<C>,
    ) -> Result<(), NativePairingCredentialError> {
        let locator = self.credential_locator(peer)?;
        #[cfg(windows)]
        {
            let (started, deadline) = super::begin(context)
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_INTERRUPTED"))?;
            let candidate = key.with_bytes(|bytes| search_os_secrets_windows::SecretBytes::new(bytes.to_vec()))
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_KEY_INVALID"))?;
            let mut remaining = || remaining(context, started, deadline);
            search_os_secrets_windows::publish_provider_pairing_credential(&locator, &candidate, &mut remaining)
                .map_err(Into::into)
        }
        #[cfg(not(windows))]
        {
            let _ = (locator, key, context);
            Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM"))
        }
    }

    fn credential_locator(&self, peer: &TransportPeer) -> Result<[u8; 32], NativePairingCredentialError> {
        let role = match peer.role {
            PeerRole::StandaloneCli => 1,
            PeerRole::ClientAdapter => 2,
            _ => return Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_PEER_INVALID")),
        };
        // Fixed-width fields and one closed role byte have unambiguous boundaries.
        // Profiles/disclosure references remain separately resolved authorization,
        // not aliases for a credential or for one another. No key enters the hash.
        let mut digest = blake3::Hasher::new();
        digest.update(b"ELIOT-NATIVE-PROVIDER-PAIRING-CREDENTIAL-v1\0");
        digest.update(self.installation_id.as_bytes());
        digest.update(peer.incarnation.as_bytes());
        digest.update(peer.binding.as_bytes());
        digest.update(self.peer_identity_digest.as_bytes());
        digest.update(&self.pairing_generation.get().to_be_bytes());
        digest.update(&[role]);
        Ok(*digest.finalize().as_bytes())
    }
}

#[cfg(windows)]
fn remaining<C: CancellationProbe>(
    context: &OperationContext<C>,
    started: search_provider_protocol::MonotonicMillis,
    deadline: search_provider_protocol::MonotonicMillis,
) -> Option<std::time::Duration> {
    let now = crate::provider_composition::monotonic_millis();
    if context.cancellation().is_cancelled() || now < started { return None; }
    deadline.get().checked_sub(now.get()).filter(|left| *left > 0)
        .map(std::time::Duration::from_millis)
}
