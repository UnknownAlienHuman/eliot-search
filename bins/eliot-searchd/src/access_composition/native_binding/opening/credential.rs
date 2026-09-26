//! Native credential resolution for the exact registered peer and generation.

use search_contracts::protocol::PeerRole;
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingKey, TransportPeer};

use super::NativeBindingExpectation;

/// Non-secret identity of the exact immutable registration command that owns a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePairingCredentialIntent {
    operation_id: [u8; 32],
    command_digest: [u8; 32],
    expected_generation: u64,
}

impl NativePairingCredentialIntent {
    /// Creates one exact operation identity for native credential persistence.
    #[must_use]
    pub const fn new(
        operation_id: [u8; 32],
        command_digest: [u8; 32],
        expected_generation: u64,
    ) -> Self {
        Self { operation_id, command_digest, expected_generation }
    }

    /// Exact control-journal operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] { self.operation_id }

    /// Digest of the exact registration command.
    #[must_use]
    pub const fn command_digest(self) -> [u8; 32] { self.command_digest }

    /// Journal generation expected by that command.
    #[must_use]
    pub const fn expected_generation(self) -> u64 { self.expected_generation }

    #[cfg(windows)]
    fn platform(self) -> search_os_secrets_windows::ProviderPairingCredentialIntent {
        search_os_secrets_windows::ProviderPairingCredentialIntent::new(
            self.operation_id, self.command_digest, self.expected_generation,
        )
    }
}

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

struct NativePairingCredential {
    intent: NativePairingCredentialIntent,
    key: BindingKey,
}

impl NativeBindingExpectation {
    /// Resolve the existing key from Windows Credential Manager before pairing.
    ///
    /// The stored command intent is validated structurally but opening relies on
    /// the current published binding and ceremony, not on that historical command.
    /// Missing/invalid keys never trigger generation or a fallback store.
    pub fn load_pairing_key<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        context: &OperationContext<C>,
    ) -> Result<BindingKey, NativePairingCredentialError> {
        self.read_pairing_credential(peer, context)?
            .map(|record| record.key)
            .ok_or(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_MISSING"))
    }

    /// Restore the exact candidate only when its persisted intent matches.
    ///
    /// This is the restart seam for an immutable administrative command. It does
    /// not generate a replacement, adopt a key belonging to another operation or
    /// prove that the corresponding journal transaction committed.
    pub(in crate::access_composition) fn restore_pairing_key<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        intent: NativePairingCredentialIntent,
        context: &OperationContext<C>,
    ) -> Result<Option<BindingKey>, NativePairingCredentialError> {
        let Some(record) = self.read_pairing_credential(peer, context)? else { return Ok(None); };
        if record.intent != intent {
            return Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_CONFLICT"));
        }
        Ok(Some(record.key))
    }

    // Readback is deliberately separate from publish: absence never causes a
    // write, and a conflicting key/intent is not adopted as recovered input.
    pub(in crate::access_composition) fn pairing_key_matches<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        candidate: &BindingKey,
        intent: NativePairingCredentialIntent,
        context: &OperationContext<C>,
    ) -> Result<bool, NativePairingCredentialError> {
        let Some(observed) = self.read_pairing_credential(peer, context)? else { return Ok(false); };
        if observed.intent != intent {
            return Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_CONFLICT"));
        }
        let matches = candidate.with_bytes(|expected| observed.key.with_bytes(|actual| {
            expected.iter().zip(actual).fold(0_u8, |difference, (left, right)| difference | (left ^ right)) == 0
        }));
        if !matches { return Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_CONFLICT")); }
        Ok(true)
    }

    fn read_pairing_credential<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        context: &OperationContext<C>,
    ) -> Result<Option<NativePairingCredential>, NativePairingCredentialError> {
        let locator = self.credential_locator(peer)?;
        #[cfg(windows)]
        {
            let (started, deadline) = super::begin(context)
                .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_INTERRUPTED"))?;
            let mut remaining = || remaining(context, started, deadline);
            let record = search_os_secrets_windows::load_provider_pairing_credential_record(
                &locator, &mut remaining,
            )?;
            record.map(|record| {
                let platform = record.intent();
                let intent = NativePairingCredentialIntent::new(
                    platform.operation_id(), platform.command_digest(), platform.expected_generation(),
                );
                let secret = record.into_secret();
                let bytes = secret.expose_secret().try_into()
                    .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_KEY_INVALID"))?;
                let key = BindingKey::from_bytes(bytes)
                    .map_err(|_| NativePairingCredentialError::refused("PAIRING_CREDENTIAL_KEY_INVALID"))?;
                Ok(NativePairingCredential { intent, key })
            }).transpose()
        }
        #[cfg(not(windows))]
        {
            let _ = (locator, context);
            Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM"))
        }
    }

    /// Publish a retained candidate for one immutable command and generation.
    ///
    /// Intent and key are persisted/read back together. Equal records are
    /// idempotent; any different operation, digest, generation or key conflicts.
    /// This method performs no generation, retry, deletion or journal mutation.
    pub fn publish_pairing_key<C: CancellationProbe>(
        &self,
        peer: &TransportPeer,
        key: &BindingKey,
        intent: NativePairingCredentialIntent,
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
            search_os_secrets_windows::publish_provider_pairing_credential(
                &locator, intent.platform(), &candidate, &mut remaining,
            ).map_err(Into::into)
        }
        #[cfg(not(windows))]
        {
            let _ = (locator, key, intent, context);
            Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM"))
        }
    }

    fn credential_locator(&self, peer: &TransportPeer) -> Result<[u8; 32], NativePairingCredentialError> {
        let role = match peer.role {
            PeerRole::StandaloneCli => 1,
            PeerRole::ClientAdapter => 2,
            _ => return Err(NativePairingCredentialError::refused("PAIRING_CREDENTIAL_PEER_INVALID")),
        };
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
