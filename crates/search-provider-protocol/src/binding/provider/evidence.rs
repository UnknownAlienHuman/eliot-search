//! Additive evidence output over the existing connection-owned delivery path.

use search_contracts::evidence::RecipeIdV2;
use search_contracts::evidence_frames::{
    EvidenceFrameV2, EvidenceTerminalFrame, MAX_EVIDENCE_EXCHANGE_FRAMES,
};
use search_contracts::{
    BoundedBytes, BoundedList, CancelledBody, Coverage, CoverageDenominatorKind, LegDescriptor,
    MAX_FRAME_BYTES, MAX_LIST_ITEMS,
};

use super::{
    AdmittedEvidenceProviderRequestV2, BoundSession, CheckedProviderEvent, EvidenceResponseBodyV2,
    MonotonicMillis, PreparedEvidenceDeliveryV2, PreparedProviderDeliveryError, ProtocolError,
    ProviderDeliveryError, ProviderEventPreparationV1, ProviderFrameTranscript,
    ServerEnvelopeCodec,
};
use crate::frame::{EvidenceRecipeRequestV2, EvidenceRecipeResultV2, EvidenceRequestEnvelopeV2};

/// Bounded output cursor attached to the original opaque admission, not a ledger.
#[derive(Clone, Default)]
pub(super) struct EvidenceResponseState {
    frames: usize,
    bytes: u64,
    declared_frames: Option<usize>,
    snapshot: Option<(
        CoverageDenominatorKind,
        BoundedList<LegDescriptor, MAX_LIST_ITEMS>,
    )>,
    result: Option<Coverage>,
    batches: [(u64, bool, bool); 3],
    tail: Option<usize>,
}

impl EvidenceResponseState {
    fn advance(
        &mut self,
        request: &EvidenceRequestEnvelopeV2,
        body: &EvidenceResponseBodyV2,
        frame_bytes: usize,
        cancelled: bool,
    ) -> Result<(), ProtocolError> {
        let (profile, progressive, budget, scope, source_view) = match &request.recipe_request {
            EvidenceRecipeRequestV2::RetrieveEvidence(value) => (
                &value.profile_id,
                value.progressive,
                &value.budgets,
                &value.scope,
                &value.source_view,
            ),
            EvidenceRecipeRequestV2::OrientScope(value) => (
                &value.profile_id,
                value.progressive,
                &value.budgets,
                &value.scope,
                &value.source_view,
            ),
        };
        if cancelled && !matches!(body, EvidenceResponseBodyV2::Cancelled(_)) {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        let terminal = body.terminal();
        match body {
            EvidenceResponseBodyV2::Frame(frame) => {
                if &frame.header().profile_id != profile {
                    return Err(ProtocolError::InvalidBody);
                }
                match frame {
                    EvidenceFrameV2::Accepted(value) => {
                        if self.frames != 0 || value.progressive != progressive {
                            return Err(ProtocolError::InvalidBody);
                        }
                        let count = usize::try_from(value.declared_frame_budget)
                            .map_err(|_| ProtocolError::ResourceExhausted)?;
                        if !(3..=MAX_EVIDENCE_EXCHANGE_FRAMES).contains(&count) {
                            return Err(ProtocolError::ResourceExhausted);
                        }
                        self.declared_frames = Some(count);
                    }
                    EvidenceFrameV2::CoverageSnapshot(value) => {
                        if self.declared_frames.is_none() || self.snapshot.is_some() {
                            return Err(ProtocolError::InvalidBody);
                        }
                        self.snapshot = Some((
                            value.coverage.denominator_kind,
                            value.coverage.requested_legs.clone(),
                        ));
                    }
                    EvidenceFrameV2::CardBatch(value) => {
                        if request.recipe_request.recipe_id() != RecipeIdV2::RetrieveEvidence {
                            return Err(ProtocolError::InvalidBody);
                        }
                        self.batch(
                            0,
                            value.batch_offset,
                            value.last_batch,
                            value.cards.len(),
                            progressive,
                        )?;
                    }
                    EvidenceFrameV2::OrientationBatch(value) => {
                        if request.recipe_request.recipe_id() != RecipeIdV2::OrientScope {
                            return Err(ProtocolError::InvalidBody);
                        }
                        self.batch(
                            1,
                            value.batch_offset,
                            value.last_batch,
                            value.cards.len(),
                            progressive,
                        )?;
                    }
                    EvidenceFrameV2::SuggestedCheckBatch(value) => {
                        self.batch(
                            2,
                            value.batch_offset,
                            value.last_batch,
                            value.checks.len(),
                            progressive,
                        )?;
                    }
                    EvidenceFrameV2::Terminal(value) => {
                        if self.snapshot.is_none()
                            || self.batches.iter().any(|(_, last, seen)| *seen && !last)
                            || (!progressive
                                && self.result.is_none()
                                && terminal != Some(super::TerminalKind::Failed))
                        {
                            return Err(ProtocolError::InvalidBody);
                        }
                        self.validate_denominator(&value.coverage)?;
                        // A final result is already emitted evidence. Terminal
                        // validation cannot replace its incomplete work or gaps.
                        // The shared terminal contract still permits missing
                        // executions for Partial/Failure; only Success requires
                        // every requested leg completed without gaps.
                        if self
                            .result
                            .as_ref()
                            .is_some_and(|coverage| coverage != &value.coverage)
                        {
                            return Err(ProtocolError::InvalidBody);
                        }
                    }
                }
            }
            EvidenceResponseBodyV2::Result(result) => {
                if self.snapshot.is_none()
                    || self.result.is_some()
                    || result.recipe_id() != request.recipe_request.recipe_id()
                    || self.batches.iter().any(|(_, last, seen)| *seen && !last)
                {
                    return Err(ProtocolError::InvalidBody);
                }
                let (resolved, fence, coverage) = match result {
                    EvidenceRecipeResultV2::RetrieveEvidence(value) => {
                        (&value.resolved_scope, &value.result_fence, &value.coverage)
                    }
                    EvidenceRecipeResultV2::OrientScope(value) => {
                        (&value.resolved_scope, &value.result_fence, &value.coverage)
                    }
                };
                if resolved != scope || &fence.planned_snapshot.source_view != source_view {
                    return Err(ProtocolError::InvalidBody);
                }
                self.validate_denominator(coverage)?;
                self.result = Some(coverage.clone());
            }
            EvidenceResponseBodyV2::Progress(_) => {
                if self.declared_frames.is_none() {
                    return Err(ProtocolError::InvalidBody);
                }
            }
            EvidenceResponseBodyV2::Error(_) => {}
            EvidenceResponseBodyV2::Cancelled(_) => {
                if !cancelled {
                    return Err(ProtocolError::InvalidSessionTransition);
                }
            }
        }
        let frames = self
            .frames
            .checked_add(1)
            .ok_or(ProtocolError::ResourceExhausted)?;
        let ceiling = self.declared_frames.unwrap_or(MAX_EVIDENCE_EXCHANGE_FRAMES);
        // Keep one declared event for the sole terminal, including progress/results.
        if frames > ceiling || (terminal.is_none() && frames >= ceiling) {
            return Err(ProtocolError::ResourceExhausted);
        }
        let bytes = self
            .bytes
            .checked_add(u64::try_from(frame_bytes).map_err(|_| ProtocolError::ResourceExhausted)?)
            .ok_or(ProtocolError::ResourceExhausted)?;
        // The codec reserves one separately bounded terminal frame, so exhausting
        // the data budget cannot prevent failure/cancellation acknowledgement.
        // Total output is at most the data ceiling plus one 1 MiB terminal.
        if terminal.is_none() && bytes > budget.max_materialized_result_bytes {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.frames = frames;
        self.bytes = bytes;
        Ok(())
    }

    fn validate_denominator(&self, coverage: &Coverage) -> Result<(), ProtocolError> {
        let Some((kind, requested)) = &self.snapshot else {
            return Err(ProtocolError::InvalidBody);
        };
        if *kind != coverage.denominator_kind || requested != &coverage.requested_legs {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }

    fn batch(
        &mut self,
        kind: usize,
        offset: u64,
        last: bool,
        count: usize,
        progressive: bool,
    ) -> Result<(), ProtocolError> {
        if !progressive
            || self.snapshot.is_none()
            || self.result.is_some()
            || self
                .tail
                .is_some_and(|tail| tail != kind && !self.batches[tail].1)
        {
            return Err(ProtocolError::InvalidBody);
        }
        let state = &mut self.batches[kind];
        if state.1 || offset != state.0 {
            return Err(ProtocolError::InvalidBody);
        }
        state.0 = offset
            .checked_add(u64::try_from(count).map_err(|_| ProtocolError::ResourceExhausted)?)
            .ok_or(ProtocolError::ResourceExhausted)?;
        state.1 = last;
        state.2 = true;
        self.tail = Some(kind);
        Ok(())
    }
}

impl BoundSession {
    /// Prepares a V2 response under the original paired session and request custody.
    ///
    /// Correlation, exact encoded bytes, recipe/profile/view, ordering and cumulative
    /// budgets are checked before output. Refusal spends no event, bytes or sequence.
    /// Terminal class is derived from the typed body, never a caller proof flag.
    /// Output failure, unwind or a late clock disconnects through the shared V1 path.
    pub fn deliver_evidence_provider_event_prepared_v2<P, E>(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        prepare: impl FnOnce(
            &ProviderEventPreparationV1,
        )
            -> Result<(EvidenceResponseBodyV2, BoundedBytes<MAX_FRAME_BYTES>), P>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), PreparedProviderDeliveryError<P, E>> {
        use PreparedProviderDeliveryError::{Preparation, Protocol};
        let envelope = &request.envelope;
        let guard = request.custody.guard.clone();
        let mut next = request.response.clone();
        self.deliver_sequenced_provider_event(
            &mut request.custody,
            clock,
            |context| {
                let (body, frame) = prepare(context).map_err(Preparation)?;
                let prepared =
                    PreparedEvidenceDeliveryV2::validate(context, body, frame).map_err(Protocol)?;
                let body = &prepared.envelope.body;
                next.advance(envelope, body, prepared.frame.len(), guard.is_cancelled())
                    .map_err(Protocol)?;
                Ok(CheckedProviderEvent {
                    frame: prepared.frame,
                    progress: match body {
                        EvidenceResponseBodyV2::Progress(value) => Some(value.clone()),
                        _ => None,
                    },
                    terminal: body.terminal(),
                })
            },
            output,
        )?;
        request.response = next;
        Ok(())
    }

    /// Encodes and delivers one V2 event with the existing strict server codec.
    pub fn deliver_evidence_provider_event_v2<E>(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        body: EvidenceResponseBodyV2,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        match self.deliver_evidence_provider_event_prepared_v2(
            request,
            clock,
            |context| {
                let frame = ServerEnvelopeCodec::encode_evidence_response_v2(
                    &context.envelope_v2(body.clone()),
                    context.limits(),
                    context.supported_versions(),
                )?;
                Ok((body, frame))
            },
            output,
        ) {
            Ok(()) => Ok(()),
            Err(
                PreparedProviderDeliveryError::Protocol(error)
                | PreparedProviderDeliveryError::Preparation(error),
            ) => Err(ProviderDeliveryError::Protocol(error)),
            Err(PreparedProviderDeliveryError::Output(error)) => {
                Err(ProviderDeliveryError::Output(error))
            }
            Err(PreparedProviderDeliveryError::AfterOutput(error)) => {
                Err(ProviderDeliveryError::AfterOutput(error))
            }
        }
    }

    /// Delivers the one evidence terminal and releases the original session slot.
    pub fn finish_evidence_provider_request_v2<E>(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        terminal: EvidenceTerminalFrame,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        self.deliver_evidence_provider_event_v2(
            request,
            EvidenceResponseBodyV2::Frame(EvidenceFrameV2::Terminal(terminal)),
            clock,
            output,
        )
    }

    /// Acknowledges actual cancellation of the original guard, never success.
    /// Authenticated cancel controls already use `cancel_provider_request` and
    /// the same replay ledger for V1 and V2 targets; this is the target terminal.
    pub fn finish_evidence_provider_cancelled_v2<E>(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        clock: &mut impl FnMut() -> Result<MonotonicMillis, ProtocolError>,
        output: impl FnOnce(&ProviderFrameTranscript<'_>) -> Result<(), E>,
    ) -> Result<(), ProviderDeliveryError<E>> {
        let body = EvidenceResponseBodyV2::Cancelled(CancelledBody {
            target_request_id: *request.guard().request_id(),
            terminal: true,
        });
        self.deliver_evidence_provider_event_v2(request, body, clock, output)
    }
}
