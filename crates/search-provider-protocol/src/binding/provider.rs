//! Typed recipe admission and output on the existing bound-session owner.

use search_contracts::{
    ProtocolRange, ProviderBodyV1, ProviderEnvelope, RequestBody,
};

use crate::error::ProtocolError;
use crate::frame::{ClientEnvelopeCodec, ServerEnvelopeCodec};
use crate::pairing::{ProofDigest, ServerNonce, SessionId, verify_proof};
use crate::request::{MonotonicMillis, RequestGuard};
use crate::session::{SequenceTracker, SessionState};
use crate::terminal::TerminalKind;

use super::BoundSession;

/// Borrowed proof input for a complete typed frame, distinct from shell/grant
/// transcripts. The adapter hashes the parts in order with its pairing key.
///
/// No wire field or negotiation is added by this value. The transport must
/// explicitly carry the separate proof; enabling a bare JSON route is unsafe.
/// Construction does not authenticate bytes or authorize source access.
pub struct ProviderFrameTranscript<'a> {
    domain: &'static [u8],
    ceremony: SessionId,
    nonce: ServerNonce,
    frame: &'a [u8],
}

impl<'a> ProviderFrameTranscript<'a> {
    /// Exact input for a client request proof, including the four-byte prefix.
    #[must_use]
    pub const fn request(ceremony: SessionId, nonce: ServerNonce, frame: &'a [u8]) -> Self {
        Self { domain: b"ELIOT-PROVIDER-FRAME-REQ-v1\0", ceremony, nonce, frame }
    }

    /// Exact input for a provider response proof; direction cannot be reflected.
    #[must_use]
    pub const fn response(ceremony: SessionId, nonce: ServerNonce, frame: &'a [u8]) -> Self {
        Self { domain: b"ELIOT-PROVIDER-FRAME-RESP-v1\0", ceremony, nonce, frame }
    }

    /// Hash these borrowed parts consecutively, without separators or copies.
    #[must_use]
    pub fn parts(&self) -> [&[u8]; 4] {
        [self.domain, self.ceremony.as_bytes(), self.nonce.as_bytes(), self.frame]
    }

    /// The exact frame authenticated by this transcript; never log its content.
    #[must_use]
    pub const fn frame(&self) -> &[u8] { self.frame }
}

/// One admitted typed recipe and its event cursor. It cannot be cloned or
/// constructed from caller-selected guards. Replay/slots stay in BoundSession.
///
/// The body remains untrusted claims until live grant, capability, scope and
/// disclosure checks succeed. Dropping this value does not complete work or
/// release its session slot; the owner must cancel or disconnect abandoned work.
#[must_use = "complete, cancel or disconnect the admitted request"]
pub struct AdmittedProviderRequest {
    body: RequestBody,
    guard: RequestGuard,
    next_event: Option<u64>,
    last_clock: MonotonicMillis,
}

impl AdmittedProviderRequest {
    /// Exact decoded request body, not an authorization permit.
    #[must_use]
    pub const fn body(&self) -> &RequestBody { &self.body }

    /// Original session-owned admission; clones share its cancellation signal.
    #[must_use]
    pub const fn guard(&self) -> &RequestGuard { &self.guard }

    /// Next progress/result event number, or None after terminal delivery.
    #[must_use]
    pub const fn next_event_sequence(&self) -> Option<u64> { self.next_event }
}

impl core::fmt::Debug for AdmittedProviderRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AdmittedProviderRequest")
            .field("request_id", self.guard.request_id())
            .field("recipe", &self.body.recipe_request.recipe)
            .field("next_event", &self.next_event)
            .finish_non_exhaustive()
    }
}

/// Separates refusal before output from output failure and a late completion.
#[derive(Debug)]
pub enum ProviderDeliveryError<E> {
    /// Validation or clock failure before output was attempted.
    Protocol(ProtocolError),
    /// The output callback failed; it may already have written bytes.
    Output(E),
    /// Output returned, but the original deadline/clock check failed.
    AfterOutput(ProtocolError),
}

impl BoundSession {
    /// Decodes and authenticates one typed recipe under this exact paired session.
    ///
    /// The key owner computes the expected proof from the supplied transcript,
    /// not from a reconstructed body. Binding/incarnation, both request IDs,
    /// schema, proof, finite deadline and capacity precede replay/sequence commit.
    /// The same registration path is used by shell and grant admissions.
    ///
    /// `clock` uses this session's monotonic origin. Decoding/proof time is
    /// charged to the smaller client/server budget; overflow/regression fails.
    /// A valid result proves transport admission only, never grant authority.
    /// Hello and cancel use their own lifecycle; neither masquerades as a recipe.
    pub fn admit_provider_request(
        &mut self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prove: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<ProofDigest, ProtocolError>,
    ) -> Result<AdmittedProviderRequest, ProtocolError> {
        self.validate_session_header(self.binding.version(), &self.server_nonce)?;
        if maximum_deadline_ms == 0 { return Err(ProtocolError::InvalidLimits); }
        let started = clock()?;
        let versions = ProtocolRange { minimum: self.binding.version(), maximum: self.binding.version() };
        let envelope = ClientEnvelopeCodec::decode(frame, self.limits, versions)?;
        if envelope.binding_id != self.binding.binding_id()
            || envelope.installation_incarnation_id != self.binding.incarnation()
        {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let ProviderBodyV1::Request(body) = envelope.body else {
            return Err(ProtocolError::InvalidEnvelope);
        };
        let budget = envelope.relative_deadline_ms.unwrap_or(maximum_deadline_ms)
            .min(maximum_deadline_ms);
        let deadline = started.get().checked_add(budget)
            .filter(|end| *end > started.get()).ok_or(ProtocolError::DeadlineExpired)?;
        let transcript = ProviderFrameTranscript::request(self.pairing.session(), self.server_nonce, frame);
        let expected = prove(&transcript)?;
        if !verify_proof(&expected, observed_proof) { return Err(ProtocolError::AuthenticationFailed); }
        let now = clock()?;
        if now < started || now.get() >= deadline { return Err(ProtocolError::DeadlineExpired); }
        let guard = self.commit_checked_request(
            envelope.request_id, envelope.connection_sequence, now, Some(deadline - now.get()),
        )?;
        Ok(AdmittedProviderRequest { body, guard, next_event: Some(1), last_clock: now })
    }

    /// Encodes and delivers one correlated typed event, then commits its cursors.
    ///
    /// Progress is nonterminal; results require Success or Partial, ordinary
    /// errors Failed, and cancelled bodies Cancelled. Unknown mutation outcomes
    /// have no P00 error code: they are refused here, never downgraded to Failed.
    /// Their dedicated shell/grant recovery path must be used or transport closed.
    ///
    /// `output` must sign/send the exact frame and finish every required transport
    /// acknowledgement under the original deadline and live disclosure barrier.
    /// It must check cancellation/deadline around actual I/O, not just return Ok.
    /// This method cannot preempt blocking callbacks. Cancellation after terminal
    /// selection cannot rewrite emitted bytes. Local output is not peer receipt.
    ///
    /// Errors after output starts and unwinding disconnect the session. Validation
    /// errors before output do not consume provider/event sequence or terminal state.
    pub fn deliver_provider_event<E>(
        &mut self,
        request: &mut AdmittedProviderRequest,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        use ProviderDeliveryError::{AfterOutput, Output, Protocol};

        let started = clock().map_err(Protocol)?;
        self.validate_provider_guard(request, started).map_err(Protocol)?;
        let event = request.next_event.ok_or(Protocol(ProtocolError::DuplicateTerminal))?;
        validate_event(request, &body, terminal, event).map_err(Protocol)?;
        let next_event = if terminal.is_some() { None } else {
            Some(event.checked_add(1).ok_or(Protocol(ProtocolError::SequenceExhausted))?)
        };
        let mut progress_guard = self.guards.get(request.guard.request_id())
            .ok_or(Protocol(ProtocolError::InvalidSessionTransition))?.clone();
        if let ProviderBodyV1::Progress(progress) = &body {
            progress_guard.advance_progress(
                u64::from(progress.bounded_counts.total_planned_legs),
                u64::from(progress.bounded_counts.completed_legs), self.limits,
            ).map_err(Protocol)?;
        }
        if let Some(kind) = terminal { progress_guard.finish(kind, self.limits).map_err(Protocol)?; }
        let mut sequence = self.session.sequences().provider();
        let assigned = sequence.next_expected().ok_or(Protocol(ProtocolError::SequenceExhausted))?;
        SequenceTracker::require_accepted(sequence.observe(assigned)).map_err(Protocol)?;
        let envelope = ProviderEnvelope {
            protocol_major: self.binding.version().major,
            protocol_minor: self.binding.version().minor,
            installation_incarnation_id: self.binding.incarnation(),
            binding_id: self.binding.binding_id(),
            connection_sequence: assigned,
            request_id: *request.guard.request_id(),
            message_kind: body.message_kind(),
            relative_deadline_ms: None,
            body,
        };
        let versions = ProtocolRange { minimum: self.binding.version(), maximum: self.binding.version() };
        let frame = ServerEnvelopeCodec::encode(&envelope, self.limits, versions).map_err(Protocol)?;
        let before_output = clock().map_err(Protocol)?;
        if before_output < started { return Err(Protocol(ProtocolError::DeadlineExpired)); }
        self.validate_provider_guard(request, before_output).map_err(Protocol)?;
        if terminal.is_none() && request.guard.is_cancelled() {
            return Err(Protocol(ProtocolError::InvalidSessionTransition));
        }
        let transcript = ProviderFrameTranscript::response(
            self.pairing.session(), self.server_nonce, frame.as_slice(),
        );
        let mut pending = PendingOutput { session: self, committed: false };
        let mut finished_at = None;
        let deliver = || {
            output(&transcript).map_err(Output)?;
            let after = clock().map_err(AfterOutput)?;
            if after < before_output || request.guard.is_expired(after) {
                return Err(AfterOutput(ProtocolError::DeadlineExpired));
            }
            finished_at = Some(after);
            Ok(())
        };
        if let Some(kind) = terminal {
            pending.session.prepare_request_terminal(request.guard.request_id(), kind)
                .map_err(Protocol)?.deliver(|_| deliver())?;
        } else {
            deliver()?;
            *pending.session.guards.get_mut(request.guard.request_id())
                .expect("retained progress guard under exclusive session borrow") = progress_guard;
        }
        // This exact next sequence was previewed before output; exclusive
        // borrowing prevents admission, disconnect or another emission here.
        pending.session.session.accept_provider_sequence(assigned)
            .expect("provider sequence reserved under exclusive session borrow");
        request.next_event = next_event;
        request.last_clock = finished_at.expect("successful output sampled the clock");
        pending.committed = true;
        Ok(())
    }

    fn validate_provider_guard(
        &self,
        request: &AdmittedProviderRequest,
        now: MonotonicMillis,
    ) -> Result<(), ProtocolError> {
        match self.session.state() {
            SessionState::Active | SessionState::Draining => {}
            SessionState::Closed => return Err(ProtocolError::SessionClosed),
            SessionState::Quarantined => return Err(ProtocolError::Quarantined),
            _ => return Err(ProtocolError::AuthenticationRequired),
        }
        let stored = self.guards.get(request.guard.request_id())
            .ok_or(ProtocolError::InvalidSessionTransition)?;
        if stored.sequence() != request.guard.sequence()
            || stored.admitted_at() != request.guard.admitted_at()
            || stored.deadline() != request.guard.deadline()
            || !stored.cancellation().same_signal(&request.guard.cancellation())
        {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        if request.next_event.is_none()
            || stored.progress().is_some_and(|p| p.terminal().is_some())
        {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if !self.inflight.contains(stored.request_id()) && !stored.is_cancelled() {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        if now < request.last_clock || now < stored.admitted_at() || stored.is_expired(now) {
            return Err(ProtocolError::DeadlineExpired);
        }
        Ok(())
    }
}

fn validate_event(
    request: &AdmittedProviderRequest,
    body: &ProviderBodyV1,
    terminal: Option<TerminalKind>,
    event: u64,
) -> Result<(), ProtocolError> {
    match (body, terminal) {
        (ProviderBodyV1::Progress(value), None) if value.event_sequence == event => Ok(()),
        (ProviderBodyV1::Result(value), Some(TerminalKind::Success | TerminalKind::Partial))
            if value.event_sequence == event
                && value.result.recipe_id() == request.body.recipe_request.recipe => Ok(()),
        (ProviderBodyV1::Error(_), Some(TerminalKind::Failed)) => Ok(()),
        (ProviderBodyV1::Cancelled(value), Some(TerminalKind::Cancelled))
            if value.terminal && &value.target_request_id == request.guard.request_id()
                && request.guard.is_cancelled() => Ok(()),
        _ => Err(ProtocolError::InvalidBody),
    }
}

struct PendingOutput<'a> {
    session: &'a mut BoundSession,
    committed: bool,
}

impl Drop for PendingOutput<'_> {
    fn drop(&mut self) {
        if !self.committed { let _ = self.session.disconnect(); }
    }
}
