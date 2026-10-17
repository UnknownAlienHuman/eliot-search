//! Typed request/cancellation admission and output on the bound-session owner.

use search_contracts::{
    BoundedBytes, BindingId, InstallationIncarnationId, MAX_FRAME_BYTES, MessageKind,
    ProtocolRange, ProtocolVersion, ProviderBodyV1, ProviderEnvelope, RequestBody, RequestId,
};

use crate::error::ProtocolError;
use crate::frame::{ClientEnvelopeCodec, ServerEnvelopeCodec};
use crate::pairing::{ProofDigest, ServerNonce, SessionId, verify_proof};
use crate::request::{MonotonicMillis, RequestGuard};
use crate::session::{SequenceTracker, SessionState};
use crate::terminal::TerminalKind;

use super::BoundSession;

mod cancellation;
mod evidence;

use crate::frame::evidence_response_schema::{
    EvidenceResponseBodyV2, EvidenceResponseEnvelopeV2,
};

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

/// Finite response metadata previewed under the exclusive bound-session borrow.
///
/// The event and connection sequences remain uncommitted until output succeeds.
/// This value carries no session, key, reservation, or authority and cannot be
/// used to emit a frame by itself.
pub struct ProviderEventPreparationV1 {
    protocol_version: ProtocolVersion,
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    request_id: RequestId,
    event_sequence: u64,
    connection_sequence: u64,
    limits: crate::config::ProtocolLimits,
}

impl ProviderEventPreparationV1 {
    /// Request-local event sequence selected for this attempted delivery.
    #[must_use]
    pub const fn event_sequence(&self) -> u64 { self.event_sequence }

    /// Connection-local provider sequence selected for this attempted delivery.
    #[must_use]
    pub const fn connection_sequence(&self) -> u64 { self.connection_sequence }

    /// Existing negotiated protocol version for the response envelope.
    #[must_use]
    pub const fn protocol_version(&self) -> ProtocolVersion { self.protocol_version }

    /// Existing bounded frame limits for `ServerEnvelopeCodec`.
    #[must_use]
    pub const fn limits(&self) -> crate::config::ProtocolLimits { self.limits }

    /// Exact negotiated range accepted by the existing provider codec.
    #[must_use]
    pub const fn supported_versions(&self) -> ProtocolRange {
        ProtocolRange { minimum: self.protocol_version, maximum: self.protocol_version }
    }

    /// Build the envelope skeleton bound to this attempted delivery.
    ///
    /// The caller supplies only its typed body. The protocol validates the
    /// resulting encoded frame against this complete envelope before output.
    #[must_use]
    pub fn envelope(&self, body: ProviderBodyV1) -> ProviderEnvelope {
        ProviderEnvelope {
            protocol_major: self.protocol_version.major,
            protocol_minor: self.protocol_version.minor,
            installation_incarnation_id: self.installation_incarnation_id,
            binding_id: self.binding_id,
            connection_sequence: self.connection_sequence,
            request_id: self.request_id,
            message_kind: body.message_kind(),
            relative_deadline_ms: None,
            body,
        }
    }

    /// Build the additive V2 evidence response envelope skeleton bound to this
    /// attempted delivery.
    ///
    /// Only the body is supplied by the caller. The protocol validates the
    /// encoded frame against this complete envelope before any output begins.
    #[must_use]
    pub fn envelope_v2(&self, body: EvidenceResponseBodyV2) -> EvidenceResponseEnvelopeV2 {
        EvidenceResponseEnvelopeV2 {
            protocol_major: self.protocol_version.major,
            protocol_minor: self.protocol_version.minor,
            installation_incarnation_id: self.installation_incarnation_id,
            binding_id: self.binding_id,
            connection_sequence: self.connection_sequence,
            request_id: self.request_id,
            event_sequence: self.event_sequence,
            body,
        }
    }
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
    custody: AdmittedRequestCustody,
}

/// Shared sequenced request custody for legacy and additive provider requests.
struct AdmittedRequestCustody {
    guard: RequestGuard,
    next_event: Option<u64>,
    last_clock: MonotonicMillis,
}

/// One strictly decoded V2 evidence request admitted by a real `BoundSession`.
///
/// Fields are private so callers cannot forge an admission or replace its
/// session-owned request guard. Grant authority is verified by the daemon owner.
#[must_use = "complete, cancel or disconnect the admitted request"]
pub struct AdmittedEvidenceProviderRequestV2 {
    envelope: crate::frame::EvidenceRequestEnvelopeV2,
    custody: AdmittedRequestCustody,
    response: evidence::EvidenceResponseState,
}

/// Protocol-private prepared value accepted only after full-envelope readback.
///
/// A prepared output can never be constructed by a caller: the frame bytes are
/// decoded with the existing server codec and required to equal the
/// context-derived expected envelope, so an emitted frame cannot be forged.
struct PreparedEvidenceDeliveryV2 {
    envelope: EvidenceResponseEnvelopeV2,
    frame: BoundedBytes<MAX_FRAME_BYTES>,
}

impl PreparedEvidenceDeliveryV2 {
    fn validate(
        context: &ProviderEventPreparationV1,
        body: EvidenceResponseBodyV2,
        frame: BoundedBytes<MAX_FRAME_BYTES>,
    ) -> Result<Self, ProtocolError> {
        let envelope = context.envelope_v2(body);
        let decoded = ServerEnvelopeCodec::decode_evidence_response_v2(
            frame.as_slice(), context.limits(), context.supported_versions(),
        )?;
        if decoded != envelope {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(Self { envelope, frame })
    }
}

impl AdmittedProviderRequest {
    /// Exact decoded request body, not an authorization permit.
    #[must_use]
    pub const fn body(&self) -> &RequestBody { &self.body }

    /// Original session-owned admission; clones share its cancellation signal.
    #[must_use]
    pub const fn guard(&self) -> &RequestGuard { &self.custody.guard }

    /// Next progress/result event number, or None after terminal delivery.
    #[must_use]
    pub const fn next_event_sequence(&self) -> Option<u64> { self.custody.next_event }
}

impl AdmittedEvidenceProviderRequestV2 {
    /// Decoded request and issued claims, still subject to live grant and scope checks.
    #[must_use]
    pub const fn envelope(&self) -> &crate::frame::EvidenceRequestEnvelopeV2 {
        &self.envelope
    }

    /// Original connection-owned admission guard.
    #[must_use]
    pub const fn guard(&self) -> &RequestGuard { &self.custody.guard }

    /// Next event sequence under the shared provider-request lifecycle.
    #[must_use]
    pub const fn next_event_sequence(&self) -> Option<u64> { self.custody.next_event }
}

impl core::fmt::Debug for AdmittedProviderRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AdmittedProviderRequest")
            .field("request_id", self.custody.guard.request_id())
            .field("recipe", &self.body.recipe_request.recipe)
            .field("next_event", &self.custody.next_event)
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for AdmittedEvidenceProviderRequestV2 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AdmittedEvidenceProviderRequestV2")
            .field("request_id", self.custody.guard.request_id())
            .field("recipe", &self.envelope.recipe_request.recipe_id())
            .field("next_event", &self.custody.next_event)
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

/// Separates preparation refusal from protocol validation and actual output.
#[derive(Debug)]
pub enum PreparedProviderDeliveryError<P, E> {
    /// Session, body, sequence, or prepared-frame validation failed.
    Protocol(ProtocolError),
    /// The one-shot caller preparation callback failed before output.
    Preparation(P),
    /// The output callback failed; it may already have written bytes.
    Output(E),
    /// Output returned, but the original deadline/clock check failed.
    AfterOutput(ProtocolError),
}

/// Protocol-private frame/body pair accepted only after full-envelope readback.
struct PreparedProviderDeliveryV1 {
    envelope: ProviderEnvelope,
    frame: BoundedBytes<MAX_FRAME_BYTES>,
}

impl PreparedProviderDeliveryV1 {
    fn validate(
        context: &ProviderEventPreparationV1,
        body: ProviderBodyV1,
        frame: BoundedBytes<MAX_FRAME_BYTES>,
    ) -> Result<Self, ProtocolError> {
        let expected = context.envelope(body);
        let decoded = ServerEnvelopeCodec::decode(
            frame.as_slice(), context.limits(), context.supported_versions(),
        )?;
        if decoded != expected {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(Self { envelope: expected, frame })
    }
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
        let (envelope, now, deadline) = self.authenticate_provider_frame(
            frame, observed_proof, maximum_deadline_ms, MessageKind::Request, clock, prove,
        )?;
        let ProviderBodyV1::Request(body) = envelope.body else {
            return Err(ProtocolError::InvalidEnvelope);
        };
        let guard = self.commit_checked_request(
            envelope.request_id, envelope.connection_sequence, now,
            Some(deadline.get() - now.get()),
        )?;
        Ok(AdmittedProviderRequest {
            body,
            custody: AdmittedRequestCustody {
                guard,
                next_event: Some(1),
                last_clock: now,
            },
        })
    }

    /// Authenticates and admits one additive V2 evidence request.
    ///
    /// The exact full frame is proven with the existing request transcript,
    /// then decoded under the strict `request_v2` schema. Successful admission
    /// consumes the same connection sequence and in-flight slot as a V1 request
    /// and returns the original session-owned guard. This is transport admission;
    /// the daemon must still verify the claim against its live issuer and policy.
    pub fn admit_evidence_provider_request_v2(
        &mut self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prove: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<ProofDigest, ProtocolError>,
    ) -> Result<AdmittedEvidenceProviderRequestV2, ProtocolError> {
        let (envelope, now, deadline) = self.authenticate_evidence_provider_frame_v2(
            frame,
            observed_proof,
            maximum_deadline_ms,
            clock,
            prove,
        )?;
        let guard = self.commit_checked_request(
            envelope.request_id,
            envelope.connection_sequence,
            now,
            Some(deadline.get() - now.get()),
        )?;
        Ok(AdmittedEvidenceProviderRequestV2 {
            envelope,
            custody: AdmittedRequestCustody {
                guard,
                next_event: Some(1),
                last_clock: now,
            },
            response: evidence::EvidenceResponseState::default(),
        })
    }

    // Both typed ingress paths authenticate the exact frame before touching
    // target guards, replay or sequences. Only Cancel may enter during drain;
    // the state machine checks this again when committing its control identity.
    fn authenticate_provider_frame(
        &self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        expected_kind: MessageKind,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prove: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<ProofDigest, ProtocolError>,
    ) -> Result<(ProviderEnvelope, MonotonicMillis, MonotonicMillis), ProtocolError> {
        if expected_kind != MessageKind::Cancel || self.session.state() != SessionState::Draining {
            self.validate_session_header(self.binding.version(), &self.server_nonce)?;
        }
        if maximum_deadline_ms == 0 { return Err(ProtocolError::InvalidLimits); }
        let started = clock()?;
        let versions = ProtocolRange { minimum: self.binding.version(), maximum: self.binding.version() };
        let envelope = ClientEnvelopeCodec::decode(frame, self.limits, versions)?;
        if envelope.binding_id != self.binding.binding_id()
            || envelope.installation_incarnation_id != self.binding.incarnation()
        {
            return Err(ProtocolError::AuthenticationFailed);
        }
        if envelope.message_kind != expected_kind {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let budget = envelope.relative_deadline_ms.unwrap_or(maximum_deadline_ms)
            .min(maximum_deadline_ms);
        let deadline = started.get().checked_add(budget)
            .filter(|end| *end > started.get()).ok_or(ProtocolError::DeadlineExpired)?;
        let transcript = ProviderFrameTranscript::request(self.pairing.session(), self.server_nonce, frame);
        let expected = prove(&transcript)?;
        if !verify_proof(&expected, observed_proof) { return Err(ProtocolError::AuthenticationFailed); }
        let now = clock()?;
        if now < started || now.get() >= deadline { return Err(ProtocolError::DeadlineExpired); }
        Ok((envelope, now, MonotonicMillis::new(deadline)))
    }

    fn authenticate_evidence_provider_frame_v2(
        &self,
        frame: &[u8],
        observed_proof: &ProofDigest,
        maximum_deadline_ms: u64,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prove: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<ProofDigest, ProtocolError>,
    ) -> Result<(
        crate::frame::EvidenceRequestEnvelopeV2,
        MonotonicMillis,
        MonotonicMillis,
    ), ProtocolError> {
        self.validate_session_header(self.binding.version(), &self.server_nonce)?;
        if maximum_deadline_ms == 0 {
            return Err(ProtocolError::InvalidLimits);
        }
        let started = clock()?;
        let versions = ProtocolRange {
            minimum: self.binding.version(),
            maximum: self.binding.version(),
        };
        let envelope = ClientEnvelopeCodec::decode_evidence_request_v2(
            frame,
            self.limits,
            versions,
        )?;
        if envelope.binding_id != self.binding.binding_id()
            || envelope.installation_incarnation_id != self.binding.incarnation()
        {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let query_deadline_ms = match &envelope.recipe_request {
            crate::frame::EvidenceRecipeRequestV2::RetrieveEvidence(value) => value.budgets.deadline_ms,
            crate::frame::EvidenceRecipeRequestV2::OrientScope(value) => value.budgets.deadline_ms,
        };
        let budget = envelope
            .relative_deadline_ms
            .unwrap_or(maximum_deadline_ms)
            .min(maximum_deadline_ms)
            .min(query_deadline_ms);
        let deadline = started
            .get()
            .checked_add(budget)
            .filter(|end| *end > started.get())
            .ok_or(ProtocolError::DeadlineExpired)?;
        let transcript = ProviderFrameTranscript::request(
            self.pairing.session(),
            self.server_nonce,
            frame,
        );
        let expected = prove(&transcript)?;
        if !verify_proof(&expected, observed_proof) {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let now = clock()?;
        if now < started || now.get() >= deadline {
            return Err(ProtocolError::DeadlineExpired);
        }
        Ok((envelope, now, MonotonicMillis::new(deadline)))
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
    ///
    /// # Panics
    ///
    /// Panics if the provider sequence previewed for this event, or a retained
    /// progress guard, is missing while replacing its entry after successful output.
    /// Both are unreachable under this exclusive session borrow: the sequence was
    /// observed from the same ledger before output, and the guard was read from it
    /// to validate this same request.
    pub fn deliver_provider_event<E>(
        &mut self,
        request: &mut AdmittedProviderRequest,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        let started = clock().map_err(ProviderDeliveryError::Protocol)?;
        self.validate_provider_guard(&request.custody, started)
            .map_err(ProviderDeliveryError::Protocol)?;
        let event = request
            .custody
            .next_event
            .ok_or(ProviderDeliveryError::Protocol(ProtocolError::DuplicateTerminal))?;
        validate_event(&request.body, &request.custody.guard, &body, terminal, event)
            .map_err(ProviderDeliveryError::Protocol)?;
        let mut initial_clock = Some(started);
        let mut delegated_clock = || match initial_clock.take() {
            Some(now) => Ok(now),
            None => clock(),
        };
        let result = self.deliver_provider_event_prepared(
            request,
            terminal,
            &mut delegated_clock,
            move |context| {
                let envelope = context.envelope(body.clone());
                let frame = ServerEnvelopeCodec::encode(
                    &envelope, context.limits(), context.supported_versions(),
                )?;
                Ok((body, frame))
            },
            output,
        );
        match result {
            Ok(()) => Ok(()),
            Err(PreparedProviderDeliveryError::Protocol(error)
                | PreparedProviderDeliveryError::Preparation(error)) => {
                Err(ProviderDeliveryError::Protocol(error))
            }
            Err(PreparedProviderDeliveryError::Output(error)) => {
                Err(ProviderDeliveryError::Output(error))
            }
            Err(PreparedProviderDeliveryError::AfterOutput(error)) => {
                Err(ProviderDeliveryError::AfterOutput(error))
            }
        }
    }

    /// Prepares and delivers one exact provider frame under this exclusive session borrow.
    ///
    /// The protocol previews both sequence spaces, then invokes `prepare` once
    /// with finite metadata and the envelope builder. The callback returns the
    /// typed body plus the complete length-prefixed frame encoded by the
    /// existing `ServerEnvelopeCodec`. This method decodes that frame and
    /// requires exact equality with the expected envelope and body, then builds
    /// the response transcript over those same bytes for the existing key-owning
    /// output adapter to sign and write.
    ///
    /// Preparation and frame-validation failures occur before output and do not
    /// commit either sequence or terminal state. Once output begins, the same
    /// pending-output disconnect, deadline, and post-write commit semantics as
    /// [`BoundSession::deliver_provider_event`] apply. The preparation context
    /// is a sequence preview, not a transferable reservation.
    ///
    /// # Panics
    ///
    /// Panics only if the retained progress guard disappears while this method
    /// holds the exclusive bound-session borrow after successful output.
    pub fn deliver_provider_event_prepared<P, E>(
        &mut self,
        request: &mut AdmittedProviderRequest,
        terminal: Option<TerminalKind>,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prepare: impl FnOnce(
            &ProviderEventPreparationV1,
        ) -> Result<(ProviderBodyV1, BoundedBytes<MAX_FRAME_BYTES>), P>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), PreparedProviderDeliveryError<P, E>> {
        use PreparedProviderDeliveryError::{Preparation, Protocol};

        let body = &request.body;
        let guard = request.custody.guard.clone();
        self.deliver_sequenced_provider_event(
            &mut request.custody, clock,
            |context| {
                let (value, frame) = prepare(context).map_err(Preparation)?;
                let prepared = PreparedProviderDeliveryV1::validate(context, value, frame)
                    .map_err(Protocol)?;
                validate_event(body, &guard, &prepared.envelope.body, terminal, context.event_sequence)
                    .map_err(Protocol)?;
                Ok(CheckedProviderEvent {
                    progress: match prepared.envelope.body {
                        ProviderBodyV1::Progress(progress) => Some(progress),
                        _ => None,
                    },
                    terminal, frame: prepared.frame,
                })
            }, output,
        )
    }

    // One sequence, active-guard, deadline, progress and terminal commit owner
    // for both wire generations. Typed validation is completed before output.
    fn deliver_sequenced_provider_event<P, E>(
        &mut self,
        request: &mut AdmittedRequestCustody,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prepare: impl FnOnce(&ProviderEventPreparationV1)
            -> Result<CheckedProviderEvent, PreparedProviderDeliveryError<P, E>>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), PreparedProviderDeliveryError<P, E>> {
        use PreparedProviderDeliveryError::{AfterOutput, Output, Protocol};

        let started = clock().map_err(Protocol)?;
        self.validate_provider_guard(request, started).map_err(Protocol)?;
        let event = request.next_event.ok_or(Protocol(ProtocolError::DuplicateTerminal))?;
        let mut sequence = self.session.sequences().provider();
        let assigned = sequence.next_expected().ok_or(Protocol(ProtocolError::SequenceExhausted))?;
        SequenceTracker::require_accepted(sequence.observe(assigned)).map_err(Protocol)?;
        let context = ProviderEventPreparationV1 {
            protocol_version: self.binding.version(),
            installation_incarnation_id: self.binding.incarnation(),
            binding_id: self.binding.binding_id(),
            request_id: *request.guard.request_id(),
            event_sequence: event,
            connection_sequence: assigned,
            limits: self.limits,
        };
        let prepared = prepare(&context)?;
        let terminal = prepared.terminal;
        let next_event = if terminal.is_some() { None } else {
            Some(event.checked_add(1).ok_or(Protocol(ProtocolError::SequenceExhausted))?)
        };

        let mut progress_guard = self.guards.get(request.guard.request_id())
            .ok_or(Protocol(ProtocolError::InvalidSessionTransition))?.clone();
        if let Some(progress) = &prepared.progress {
            progress_guard.advance_progress(
                u64::from(progress.bounded_counts.total_planned_legs),
                u64::from(progress.bounded_counts.completed_legs), self.limits,
            ).map_err(Protocol)?;
        }
        if let Some(kind) = terminal { progress_guard.finish(kind, self.limits).map_err(Protocol)?; }

        let before_output = clock().map_err(Protocol)?;
        if before_output < started { return Err(Protocol(ProtocolError::DeadlineExpired)); }
        self.validate_provider_guard(request, before_output).map_err(Protocol)?;
        if terminal.is_none() && request.guard.is_cancelled() {
            return Err(Protocol(ProtocolError::InvalidSessionTransition));
        }
        let transcript = ProviderFrameTranscript::response(
            self.pairing.session(), self.server_nonce, prepared.frame.as_slice(),
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
        request: &AdmittedRequestCustody,
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
    request: &RequestBody,
    guard: &RequestGuard,
    body: &ProviderBodyV1,
    terminal: Option<TerminalKind>,
    event: u64,
) -> Result<(), ProtocolError> {
    match (body, terminal) {
        (ProviderBodyV1::Progress(value), None) if value.event_sequence == event => Ok(()),
        (ProviderBodyV1::Result(value), Some(TerminalKind::Success | TerminalKind::Partial))
            if value.event_sequence == event
                && value.result.recipe_id() == request.recipe_request.recipe => Ok(()),
        (ProviderBodyV1::Error(_), Some(TerminalKind::Failed)) => Ok(()),
        (ProviderBodyV1::Cancelled(value), Some(TerminalKind::Cancelled))
            if value.terminal
                && &value.target_request_id == guard.request_id()
                && guard.is_cancelled() => Ok(()),
        _ => Err(ProtocolError::InvalidBody),
    }
}

struct CheckedProviderEvent {
    frame: BoundedBytes<MAX_FRAME_BYTES>,
    progress: Option<search_contracts::ProgressBody>,
    terminal: Option<TerminalKind>,
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
