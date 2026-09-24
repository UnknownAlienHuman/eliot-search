//! Authenticated cancel controls share the request ledger, not execution slots.

use search_contracts::{
    CancelledBody, MessageKind, ProtocolRange, ProviderBodyV1, ProviderEnvelope, RequestId,
};

use crate::cancel::CancelOutcome;
use crate::error::ProtocolError;
use crate::frame::ServerEnvelopeCodec;
use crate::pairing::ProofDigest;
use crate::request::MonotonicMillis;
use crate::session::SequenceTracker;

use super::{BoundSession, PendingOutput, ProviderDeliveryError, ProviderFrameTranscript};

impl BoundSession {
    /// Authenticates one typed cancel, signals its target and delivers its reply.
    ///
    /// The control uses a fresh request ID and the ordinary client sequence and
    /// replay ledger. It does not acquire a work slot, so full in-flight capacity
    /// cannot prevent cancellation. Replay capacity remains finite and mandatory.
    /// Cancellation is permitted in an authenticated draining session; it does
    /// not reopen admission for recipes. Self-targeting controls are rejected.
    ///
    /// The response belongs to the cancel's own ID. Its `terminal` flag is true
    /// only if the target's retained guard already records a terminal. Unknown
    /// and still-pending targets return false; setting the signal is not proof
    /// that work stopped. The target keeps its own event/terminal lifecycle.
    /// A new control ID may idempotently repeat a target; replaying an old control
    /// cannot signal it or consume another provider sequence.
    ///
    /// `output` must authenticate/send the exact response and finish any required
    /// transport acknowledgement under the supplied absolute cancel deadline.
    /// That budget starts before decode/proof and never renews. No target payload
    /// is emitted. The callback must check actual I/O; this method cannot preempt
    /// a blocked write or prove remote receipt.
    ///
    /// # Errors
    ///
    /// Normal pre-output refusal leaves target, counters and history unchanged.
    /// Contradictory target bookkeeping disconnects rather than claiming success.
    /// Once a control is accepted, its signal is irreversible: output error,
    /// post-output clock failure or unwind disconnects, without rollback/retry.
    pub fn cancel_provider_request<E>(
        &mut self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prove: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<ProofDigest, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>, MonotonicMillis) -> Result<(), E>,
    ) -> Result<CancelOutcome, ProviderDeliveryError<E>> {
        use ProviderDeliveryError::{AfterOutput, Output, Protocol};

        let (envelope, checked_at, deadline) = self.authenticate_provider_frame(
            frame, observed_proof, maximum_deadline_ms, MessageKind::Cancel, clock, prove,
        ).map_err(Protocol)?;
        let ProviderBodyV1::Cancel(cancel) = envelope.body else {
            return Err(Protocol(ProtocolError::InvalidEnvelope));
        };
        let target = cancel.target_request_id;
        if envelope.request_id == target {
            return Err(Protocol(ProtocolError::InvalidBody));
        }
        // Only authenticated callers reach target state. All response validation
        // and allocation precedes the irreversible signal and replay admission.
        let terminal = self.cancel_target_terminal(&target).map_err(Protocol)?;
        let mut sequence = self.session.sequences().provider();
        let assigned = sequence.next_expected().ok_or(Protocol(ProtocolError::SequenceExhausted))?;
        SequenceTracker::require_accepted(sequence.observe(assigned)).map_err(Protocol)?;
        let response = ProviderEnvelope {
            protocol_major: self.binding.version().major,
            protocol_minor: self.binding.version().minor,
            installation_incarnation_id: self.binding.incarnation(),
            binding_id: self.binding.binding_id(),
            connection_sequence: assigned,
            request_id: envelope.request_id,
            message_kind: MessageKind::Cancelled,
            relative_deadline_ms: None,
            body: ProviderBodyV1::Cancelled(CancelledBody { target_request_id: target, terminal }),
        };
        let versions = ProtocolRange { minimum: self.binding.version(), maximum: self.binding.version() };
        let response = ServerEnvelopeCodec::encode(&response, self.limits, versions).map_err(Protocol)?;
        let before_output = clock().map_err(Protocol)?;
        if before_output < checked_at || before_output >= deadline {
            return Err(Protocol(ProtocolError::DeadlineExpired));
        }
        let transcript = ProviderFrameTranscript::response(
            self.pairing.session(), self.server_nonce, response.as_slice(),
        );
        self.session.admit_cancellation(envelope.request_id, envelope.connection_sequence)
            .map_err(Protocol)?;
        // From admission until successful delivery, abandonment closes the
        // connection. No fallible allocation or external callback intervenes.
        let mut pending = PendingOutput { session: self, committed: false };
        let outcome = pending.session.cancel(&target);
        output(&transcript, deadline).map_err(Output)?;
        let after = clock().map_err(AfterOutput)?;
        if after < before_output || after >= deadline {
            return Err(AfterOutput(ProtocolError::DeadlineExpired));
        }
        pending.session.session.accept_provider_sequence(assigned)
            .expect("cancel response sequence reserved under exclusive session borrow");
        pending.committed = true;
        Ok(outcome)
    }

    fn cancel_target_terminal(&mut self, target: &RequestId) -> Result<bool, ProtocolError> {
        let in_flight = self.inflight.contains(target);
        if let Some(guard) = self.guards.get(target) {
            let terminal = guard.progress().is_some_and(|progress| progress.terminal().is_some());
            let consistent = if terminal { !in_flight } else { in_flight || guard.is_cancelled() };
            if guard.request_id() == target && consistent {
                return Ok(terminal);
            }
        } else if !in_flight {
            return Ok(false);
        }
        // A slot without its guard (or a completed guard still owning a slot)
        // cannot yield a truthful cancellation receipt. Fence all outstanding
        // work, without clearing replay history or fabricating target completion.
        let _ = self.disconnect();
        Err(ProtocolError::Quarantined)
    }
}
