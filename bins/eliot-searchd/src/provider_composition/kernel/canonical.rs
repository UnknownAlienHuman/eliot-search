//! Key-owning adapter for typed frames and the single canonical BoundSession.
//!
//! Native startup must supply a live authoritative BindingContext and completed
//! ceremony. The development token-file router is deliberately not upgraded by
//! this adapter: it has no durable binding or source authority to contribute.

use search_contracts::{ProviderBodyV1, RequestId};
use search_provider_protocol::{
    AdmittedProviderRequest, BindingContext, BindingKey, BoundSession, CancelOutcome,
    DisconnectReceipt, MonotonicMillis, PairingMachine, ProofDigest, ProtocolError, ProtocolLimits,
    ProviderDeliveryError, ProviderFrameTranscript, RequestGuard, ServerNonce, TerminalKind,
    verify_proof,
};

use super::router::monotonic_millis;

/// Owns one exact paired key and one canonical protocol session; no independent
/// replay ledger, grant registry or output sequence is created here.
///
/// This is the native composition boundary, not a listener or an authority
/// shortcut. Bootstrap must resolve and revalidate the durable binding. Before
/// executing a recipe or emitting data, callers still run live access, scope,
/// currentness and disclosure checks through their canonical owners.
pub struct CanonicalProviderConnection {
    session: BoundSession,
    key: BindingKey,
}

impl CanonicalProviderConnection {
    /// Transfers the actual completed ceremony and its key into the connection.
    ///
    /// The key must reproduce the ceremony's provider proof, and BoundSession
    /// must match the ceremony retained in the authoritative binding context.
    /// No key replacement or token-file fallback is exposed after construction.
    pub fn open(
        binding: BindingContext,
        ceremony: PairingMachine,
        key: BindingKey,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        let transcript = ceremony.server_transcript()?;
        let pairing = ceremony.into_verified()?;
        let expected = key.with_bytes(|bytes| {
            ProofDigest::from_bytes(*blake3::keyed_hash(bytes, transcript.as_bytes()).as_bytes())
        });
        if !verify_proof(&expected, &pairing.provider_proof()) {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let session = BoundSession::open(binding, pairing, server_nonce, limits)?;
        Ok(Self { session, key })
    }

    /// Authenticates the exact complete request frame using the retained key.
    ///
    /// Reads the real process-local clock before and after decode/proof. The
    /// returned body/guard is protocol admission, not permission to retrieve.
    pub fn admit_request(
        &mut self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
    ) -> Result<AdmittedProviderRequest, ProtocolError> {
        let key = &self.key;
        self.session.admit_provider_request(
            frame, observed_proof, maximum_deadline_ms,
            &mut || Ok(monotonic_millis()),
            |transcript| Ok(keyed_frame_proof(key, transcript)),
        )
    }

    /// Encodes, authenticates and delivers one typed event before lifecycle commit.
    ///
    /// `output` receives the exact frame, its proof and the original guard for
    /// per-write deadline/cancellation checks. It must finish the real transport
    /// output and any acknowledgement under the live disclosure barrier. An error
    /// or unwind disconnects; there is no retry, partial-frame repair or rollback.
    /// The separately carried proof is not silently appended to the old wire shim.
    pub fn deliver_event<E>(
        &mut self,
        request: &mut AdmittedProviderRequest,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
        output: impl FnOnce(&[u8], &ProofDigest, &RequestGuard) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        let key = &self.key;
        let guard = request.guard().clone();
        self.session.deliver_provider_event(
            request, body, terminal, &mut || Ok(monotonic_millis()),
            |transcript| {
                let proof = keyed_frame_proof(key, transcript);
                output(transcript.frame(), &proof, &guard)
            },
        )
    }

    /// Authenticates, applies and acknowledges one complete typed cancel frame.
    ///
    /// Uses the retained key and the recipe path's exact-frame transcript. The
    /// command has its own replay identity but needs no free execution slot.
    /// `output` receives a signed acknowledgement and the cancel's original
    /// absolute deadline in the process-local monotonic clock, not a fresh timer
    /// or the target's deadline. It must enforce that deadline at actual writes
    /// and finish all required transport output before returning success.
    ///
    /// The acknowledgement does not claim that the target has stopped or rolled
    /// back. Output failure closes the protocol session and cannot undo a signal
    /// already set; the transport owner must then close its socket. This method
    /// does not enable a new wire mode on the development listener.
    pub fn cancel_frame<E>(
        &mut self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        output: impl FnOnce(&[u8], &ProofDigest, MonotonicMillis) -> Result<(), E>,
    ) -> Result<CancelOutcome, ProviderDeliveryError<E>> {
        let key = &self.key;
        self.session.cancel_provider_request(
            frame, observed_proof, maximum_deadline_ms,
            &mut || Ok(monotonic_millis()),
            |transcript| Ok(keyed_frame_proof(key, transcript)),
            |transcript, deadline| {
                let proof = keyed_frame_proof(key, transcript);
                output(transcript.frame(), &proof, deadline)
            },
        )
    }

    /// Read-only canonical session for grant/access composition and guard checks.
    /// Its metadata and transport liveness are not source authority.
    #[must_use]
    pub const fn session(&self) -> &BoundSession { &self.session }

    /// Host-local cancellation for trusted composition, not wire dispatch.
    /// Received controls must use `cancel_frame`, which authenticates their bytes.
    pub fn cancel(&mut self, target: &RequestId) -> CancelOutcome {
        self.session.cancel(target)
    }

    /// Closes request admission and cancels connection-local guards, idempotently.
    pub fn disconnect(&mut self) -> DisconnectReceipt { self.session.disconnect() }
}

fn keyed_frame_proof(key: &BindingKey, transcript: &ProviderFrameTranscript<'_>) -> ProofDigest {
    key.with_bytes(|bytes| {
        let mut hasher = blake3::Hasher::new_keyed(bytes);
        for part in transcript.parts() { hasher.update(part); }
        ProofDigest::from_bytes(*hasher.finalize().as_bytes())
    })
}
