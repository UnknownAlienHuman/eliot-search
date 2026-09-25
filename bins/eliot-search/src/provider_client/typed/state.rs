//! Client send history and response correlation, using shared protocol owners.

use std::collections::BTreeMap;
use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{
    CancelBody, ExactScanPlanRef, ProtocolRange, ProviderBodyV1, ProviderEnvelope,
    RecipeBodyV1, RecipeIdV1, RecipeResultV1, RequestBody, RequestId,
};
use search_provider_protocol::{
    BindingContext, BindingKey, ClientEnvelopeCodec, ProgressState, ProofDigest,
    ProtocolError, ProtocolLimits, ProviderFrameTranscript, SequenceTracker,
    ServerEnvelopeCodec, ServerNonce, SessionMachine, VerifiedPairing, verify_proof,
};

use super::io::{POLL_INTERVAL, SocketIo, budget, remaining};
use super::{TypedClientError, keyed_parts};

struct PendingRequest {
    recipe: RecipeIdV1,
    exact_plan: Option<ExactScanPlanRef>,
    deadline: Instant,
    events: SequenceTracker,
    progress: Option<ProgressState>,
    cancellation_acknowledged: bool,
}

struct PendingCancel {
    id: RequestId,
    target: RequestId,
    deadline: Instant,
}

pub(super) struct State {
    socket: SocketIo,
    binding: BindingContext,
    pairing: VerifiedPairing,
    key: BindingKey,
    nonce: ServerNonce,
    limits: ProtocolLimits,
    // Local sending/receive history only, not a claim of server admission.
    session: SessionMachine,
    requests: BTreeMap<RequestId, PendingRequest>,
    cancel: Option<PendingCancel>,
}

impl State {
    pub(super) fn new(
        socket: SocketIo,
        binding: BindingContext,
        pairing: VerifiedPairing,
        key: BindingKey,
        nonce: ServerNonce,
        limits: ProtocolLimits,
        session: SessionMachine,
    ) -> Self {
        Self { socket, binding, pairing, key, nonce, limits, session,
            requests: BTreeMap::new(), cancel: None }
    }

    pub(super) fn send_request(&mut self, body: RequestBody, timeout: Duration) -> Result<RequestId, TypedClientError> {
        let (deadline, millis) = budget(timeout)?;
        if self.requests.len() >= self.limits.max_in_flight_requests {
            return Err(ProtocolError::ResourceExhausted.into());
        }
        let id = body.recipe_request.request_id;
        let pending = PendingRequest {
            recipe: body.recipe_request.recipe,
            exact_plan: match &body.recipe_request.body {
                RecipeBodyV1::ExecuteExactScan(value) => Some(value.plan_ref),
                _ => None,
            },
            deadline, events: SequenceTracker::new(1), progress: None,
            cancellation_acknowledged: false,
        };
        let send_deadline = self.pending_deadline().map_or(deadline, |old| old.min(deadline));
        let sequence = self.next_client_sequence()?;
        let envelope = self.envelope(id, sequence, millis, ProviderBodyV1::Request(body));
        let frame = ClientEnvelopeCodec::encode(&envelope, self.limits, self.versions())?;
        let proof = self.request_proof(frame.as_slice());
        remaining(send_deadline)?;
        // Mutate only client bookkeeping before sending. Failure/unwind drops
        // the entire owner, so a partly sent operation is never retried here.
        self.session.admit_request(id, sequence)?;
        self.requests.insert(id, pending);
        self.socket.write_record(frame.as_slice(), &proof, self.limits, send_deadline)?;
        Ok(id)
    }

    pub(super) fn send_cancel(
        &mut self,
        id: RequestId,
        target: RequestId,
        timeout: Duration,
    ) -> Result<(), TypedClientError> {
        let (deadline, millis) = budget(timeout)?;
        if self.cancel.is_some() { return Err(TypedClientError::CancellationPending); }
        if id == target { return Err(ProtocolError::InvalidBody.into()); }
        let send_deadline = self.pending_deadline().map_or(deadline, |old| old.min(deadline));
        let sequence = self.next_client_sequence()?;
        let envelope = self.envelope(id, sequence, millis, ProviderBodyV1::Cancel(CancelBody {
            target_request_id: target,
        }));
        let frame = ClientEnvelopeCodec::encode(&envelope, self.limits, self.versions())?;
        let proof = self.request_proof(frame.as_slice());
        remaining(send_deadline)?;
        // SessionMachine owns bounded identity history, not execution slots.
        // Its normal admission suffices on this non-draining client sender.
        self.session.admit_request(id, sequence)?;
        self.cancel = Some(PendingCancel { id, target, deadline });
        self.socket.write_record(frame.as_slice(), &proof, self.limits, send_deadline)
    }

    pub(super) fn receive(&mut self) -> Result<ProviderEnvelope, TypedClientError> {
        loop {
            if let Poll::Ready(envelope) = self.poll_receive(POLL_INTERVAL)? { return Ok(envelope); }
        }
    }

    pub(super) fn poll_receive(&mut self, quantum: Duration) -> Result<Poll<ProviderEnvelope>, TypedClientError> {
        let deadline = self.pending_deadline().ok_or(TypedClientError::NothingPending)?;
        let Poll::Ready((frame, proof)) = self.socket.poll_record(self.limits, deadline, quantum)? else {
            return Ok(Poll::Pending);
        };
        let transcript = ProviderFrameTranscript::response(self.pairing.session(), self.nonce, &frame);
        let expected = keyed_parts(&self.key, transcript.parts());
        if !verify_proof(&expected, &proof) {
            return Err(ProtocolError::AuthenticationFailed.into());
        }
        let envelope = ServerEnvelopeCodec::decode(&frame, self.limits, self.versions())?;
        if envelope.binding_id != self.binding.binding_id()
            || envelope.installation_incarnation_id != self.binding.incarnation()
            || envelope.relative_deadline_ms.is_some()
        {
            return Err(TypedClientError::ResponseMismatch);
        }
        let mut sequence = self.session.sequences().provider();
        SequenceTracker::require_accepted(sequence.observe(envelope.connection_sequence))?;
        let update = self.preview_response(&envelope)?;
        remaining(deadline)?;
        // No callback, further allocation or socket access between validation
        // and bookkeeping commit. Payload is exposed only after both complete.
        self.session.accept_provider_sequence(envelope.connection_sequence)?;
        match update {
            ResponseUpdate::Progress(id, events, progress) => {
                let pending = self.requests.get_mut(&id).expect("validated pending request");
                pending.events = events;
                pending.progress = Some(progress);
            }
            ResponseUpdate::Complete(id) => { self.requests.remove(&id); }
            ResponseUpdate::CancelAcknowledged => {
                let cancel = self.cancel.take().expect("validated pending cancel");
                if let Some(pending) = self.requests.get_mut(&cancel.target) {
                    // Keep its deadline, event cursor and slot. The control's
                    // receipt is not the target's terminal or evidence of rollback.
                    pending.cancellation_acknowledged = true;
                }
            }
        }
        Ok(Poll::Ready(envelope))
    }

    fn preview_response(&self, envelope: &ProviderEnvelope) -> Result<ResponseUpdate, TypedClientError> {
        let id = envelope.request_id;
        if let Some(cancel) = &self.cancel {
            if cancel.id == id {
                return match &envelope.body {
                    ProviderBodyV1::Cancelled(value) if value.target_request_id == cancel.target => {
                        Ok(ResponseUpdate::CancelAcknowledged)
                    }
                    _ => Err(TypedClientError::ResponseMismatch),
                };
            }
        }
        let pending = self.requests.get(&id).ok_or(TypedClientError::ResponseMismatch)?;
        match &envelope.body {
            ProviderBodyV1::Progress(value) if !pending.cancellation_acknowledged => {
                let mut events = pending.events;
                SequenceTracker::require_accepted(events.observe(value.event_sequence))?;
                if events.next_expected().is_none() { return Err(ProtocolError::SequenceExhausted.into()); }
                let total = u64::from(value.bounded_counts.total_planned_legs);
                let mut progress = match pending.progress {
                    Some(progress) if progress.total() == total => progress,
                    Some(_) => return Err(TypedClientError::ResponseMismatch),
                    None => ProgressState::new(total, self.limits)?,
                };
                progress.advance(u64::from(value.bounded_counts.completed_legs))?;
                Ok(ResponseUpdate::Progress(id, events, progress))
            }
            ProviderBodyV1::Result(value) if !pending.cancellation_acknowledged => {
                let mut events = pending.events;
                SequenceTracker::require_accepted(events.observe(value.event_sequence))?;
                if value.result.recipe_id() != pending.recipe { return Err(TypedClientError::ResponseMismatch); }
                if let RecipeResultV1::ExecuteExactScan(report) = &value.result {
                    if pending.exact_plan != Some(report.plan_ref) { return Err(TypedClientError::ResponseMismatch); }
                }
                // P00 results carry coverage, not a universal success flag.
                // Return those typed fields unchanged; never guess exit-zero.
                Ok(ResponseUpdate::Complete(id))
            }
            ProviderBodyV1::Error(_) => Ok(ResponseUpdate::Complete(id)),
            ProviderBodyV1::Cancelled(value) if value.terminal && value.target_request_id == id => {
                Ok(ResponseUpdate::Complete(id))
            }
            _ => Err(TypedClientError::ResponseMismatch),
        }
    }

    fn envelope(&self, id: RequestId, sequence: u64, millis: u64, body: ProviderBodyV1) -> ProviderEnvelope {
        ProviderEnvelope {
            protocol_major: self.binding.version().major,
            protocol_minor: self.binding.version().minor,
            installation_incarnation_id: self.binding.incarnation(),
            binding_id: self.binding.binding_id(),
            connection_sequence: sequence, request_id: id,
            message_kind: body.message_kind(), relative_deadline_ms: Some(millis), body,
        }
    }

    fn next_client_sequence(&self) -> Result<u64, ProtocolError> {
        self.session.sequences().client().next_expected().ok_or(ProtocolError::SequenceExhausted)
    }

    fn versions(&self) -> ProtocolRange {
        ProtocolRange { minimum: self.binding.version(), maximum: self.binding.version() }
    }

    fn request_proof(&self, frame: &[u8]) -> ProofDigest {
        keyed_parts(&self.key, ProviderFrameTranscript::request(self.pairing.session(), self.nonce, frame).parts())
    }

    fn pending_deadline(&self) -> Option<Instant> {
        self.requests.values().map(|request| request.deadline)
            .chain(self.cancel.iter().map(|cancel| cancel.deadline)).min()
    }
}

impl Drop for State {
    fn drop(&mut self) { let _ = self.session.close(); }
}

enum ResponseUpdate {
    Progress(RequestId, SequenceTracker, ProgressState),
    Complete(RequestId),
    CancelAcknowledged,
}
