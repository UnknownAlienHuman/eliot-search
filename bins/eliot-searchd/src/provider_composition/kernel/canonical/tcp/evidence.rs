//! V2 records on the existing paired local stream and connection-owned session.
//!
//! Native named pipes enter through the same `SocketIo` used by the canonical
//! transport. This module creates no listener, TCP endpoint, issuer, ledger or
//! source authority. Successful input is protocol admission only; the query
//! owner must verify the original V2 issuer and current C18 state before work.

use std::task::Poll;
use std::time::Duration;

use search_contracts::{MessageKind, ProtocolRange};
use search_provider_protocol::{
    AdmittedEvidenceProviderRequestV2, ClientEnvelopeCodec, EvidenceResponseBodyV2,
    MonotonicMillis, ProtocolError, ProviderFrameTranscript, verify_proof,
};

use super::{CanonicalTcpConnection, CanonicalTcpError, io, keyed_frame_proof, monotonic_millis};

impl CanonicalTcpConnection {
    /// C26 prepares the exact V2 frame inside the session's sequence preview.
    /// Typed output/after-output errors preserve pending handles on uncertain I/O.
    pub(crate) fn deliver_evidence_prepared_before_v2<P>(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        valid_until: MonotonicMillis,
        prepare: impl FnOnce(&search_provider_protocol::ProviderEventPreparationV1)
            -> Result<(EvidenceResponseBodyV2, search_contracts::BoundedBytes<{search_contracts::MAX_FRAME_BYTES}>), P>,
    ) -> Result<(), search_provider_protocol::PreparedProviderDeliveryError<P, CanonicalTcpError>> {
        use search_provider_protocol::PreparedProviderDeliveryError as Error;
        let mut operation = super::Operation { state: &mut self.state, completed: false };
        let result = (|| {
            let state = operation.state.as_mut().ok_or(Error::Output(CanonicalTcpError::Closed))?;
            let guard = request.guard().clone();
            let deadline = guard.deadline().ok_or(Error::Protocol(ProtocolError::InvalidLimits))?
                .min(valid_until);
            if monotonic_millis() >= deadline { return Err(Error::Output(CanonicalTcpError::DeadlineExpired)); }
            let cancellation = guard.cancellation();
            let (connection, io) = (&mut state.connection, &mut state.io);
            let limits = connection.limits;
            let (session, key) = (&mut connection.session, &connection.key);
            session.deliver_evidence_provider_event_prepared_v2(request,
                &mut || Ok(monotonic_millis()), prepare, |transcript| {
                    let proof = keyed_frame_proof(key, transcript);
                    io.write_record(transcript.frame(), &proof, limits, deadline, Some(&cancellation))
                })
        })();
        operation.completed = result.is_ok();
        result
    }

    /// Poll one V2 request or an authenticated common cancel on the retained stream.
    ///
    /// The pre-prefix deadline, partial record, MAC, sequence, replay state and
    /// in-flight slot stay with the original connection. No V1 recipe is used as
    /// a substitute. `Ready(None)` means the common cancel acknowledgement was
    /// actually delivered, not that the target work or its cleanup completed.
    /// Any invalid frame or incomplete output closes the same session and stream.
    ///
    /// # Errors
    /// Refuses malformed, unauthenticated, replayed, expired or truncated records,
    /// V1 recipe requests and failed cancel output without dispatching query work.
    pub fn poll_receive_evidence_v2(
        &mut self,
        maximum_deadline_ms: u64,
        quantum: Duration,
    ) -> Result<Poll<Option<AdmittedEvidenceProviderRequestV2>>, CanonicalTcpError> {
        self.with_state(|state| {
            let limits = state.connection.limits;
            let Poll::Ready(io::ReceivedRecord {
                frame,
                proof: observed,
                started,
                maximum_deadline_ms,
            }) = state.io.poll_record(limits, maximum_deadline_ms, quantum)?
            else {
                return Ok(Poll::Pending);
            };
            let (connection, io) = (&mut state.connection, &mut state.io);
            let session = &mut connection.session;
            let expected = keyed_frame_proof(
                &connection.key,
                &ProviderFrameTranscript::request(
                    session.pairing().session(),
                    *session.server_nonce(),
                    &frame,
                ),
            );
            if !verify_proof(&expected, &observed) {
                return Err(CanonicalTcpError::Protocol(
                    ProtocolError::AuthenticationFailed,
                ));
            }
            let version = session.binding_context().version();
            let versions = ProtocolRange {
                minimum: version,
                maximum: version,
            };
            let mut first_clock = Some(started);
            let mut clock = || Ok(first_clock.take().unwrap_or_else(monotonic_millis));
            if ClientEnvelopeCodec::decode_evidence_request_v2(&frame, limits, versions).is_ok() {
                let request = session
                    .admit_evidence_provider_request_v2(
                        &frame,
                        &observed,
                        maximum_deadline_ms,
                        &mut clock,
                        |_| Ok(expected),
                    )
                    .map_err(CanonicalTcpError::Protocol)?;
                session
                    .revalidate_request_guard(request.guard(), monotonic_millis())
                    .map_err(CanonicalTcpError::Protocol)?;
                return Ok(Poll::Ready(Some(request)));
            }
            // Only the existing closed common Cancel schema may share this path.
            // Malformed V2 and V1 recipe requests cannot fall through to a handler.
            let kind = ClientEnvelopeCodec::decode(&frame, limits, versions)
                .map_err(CanonicalTcpError::Protocol)?
                .message_kind;
            if kind != MessageKind::Cancel {
                return Err(CanonicalTcpError::Protocol(ProtocolError::InvalidEnvelope));
            }
            let key = &connection.key;
            session
                .cancel_provider_request(
                    &frame,
                    &observed,
                    maximum_deadline_ms,
                    &mut clock,
                    |_| Ok(expected),
                    |transcript, cancel_deadline| {
                        let proof = keyed_frame_proof(key, transcript);
                        io.write_record(transcript.frame(), &proof, limits, cancel_deadline, None)
                    },
                )
                .map_err(CanonicalTcpError::from)?;
            Ok(Poll::Ready(None))
        })
    }

    /// Deliver a V2 event while the caller holds its live disclosure checkpoint.
    ///
    /// The existing protocol owner validates exact encoded output, coverage,
    /// cumulative exchange limits and the sole terminal before committing its
    /// event/sequence/slot state. Every partial write uses the admitted absolute
    /// request deadline capped by the live owner's `valid_until`; nonterminal
    /// output also observes the same request cancellation. Failure never retries
    /// or repairs a partially delivered frame.
    ///
    /// # Errors
    /// Refuses invalid protocol output, expired request/owner lifetime, cancelled
    /// nonterminal delivery or any failed write, closing the same connection.
    pub fn deliver_evidence_event_before_v2(
        &mut self,
        request: &mut AdmittedEvidenceProviderRequestV2,
        body: EvidenceResponseBodyV2,
        valid_until: MonotonicMillis,
    ) -> Result<(), CanonicalTcpError> {
        self.with_state(|state| {
            let guard = request.guard().clone();
            let deadline = guard
                .deadline()
                .ok_or(CanonicalTcpError::DeadlineExpired)?
                .min(valid_until);
            if monotonic_millis() >= deadline {
                return Err(CanonicalTcpError::DeadlineExpired);
            }
            let cancellation = guard.cancellation();
            let terminal = body.terminal().is_some();
            let (connection, io) = (&mut state.connection, &mut state.io);
            let key = &connection.key;
            let limits = connection.limits;
            connection
                .session
                .deliver_evidence_provider_event_v2(
                    request,
                    body,
                    &mut || Ok(monotonic_millis()),
                    |transcript| {
                        let proof = keyed_frame_proof(key, transcript);
                        let cancel = if terminal {
                            None
                        } else {
                            Some(&cancellation)
                        };
                        io.write_record(transcript.frame(), &proof, limits, deadline, cancel)
                    },
                )
                .map_err(CanonicalTcpError::from)
        })
    }
}
