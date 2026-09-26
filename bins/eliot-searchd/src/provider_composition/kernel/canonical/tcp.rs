//! Concrete typed TCP I/O after authoritative pairing/bootstrap handoff.

mod io;
mod handshake;
#[cfg(feature = "wave4-query")]
mod serving;

#[cfg(feature = "wave4-query")]
pub use serving::{
    CanonicalRecipeHost, CanonicalRecipeTask, CanonicalServingAuthority,
    CanonicalServingError, CanonicalServingLimits, CanonicalServingOwner,
    CanonicalWorkBudget, CanonicalWorkOutput,
};

use std::task::Poll;
use std::time::Duration;

use search_contracts::{MessageKind, ProtocolRange, ProviderBodyV1};
use search_provider_protocol::{
    AdmittedProviderRequest, BoundSession, ClientEnvelopeCodec,
    ProtocolError, ProviderDeliveryError, ProviderFrameTranscript, TerminalKind,
    verify_proof,
};

use super::{CanonicalProviderConnection, keyed_frame_proof, monotonic_millis};
use io::SocketIo;

/// Closed failure classes. No peer-supplied body, token or proof is rendered.
#[derive(Debug)]
pub enum CanonicalTcpError {
    /// Protocol/schema/authentication refusal before output.
    Protocol(ProtocolError),
    /// Actual socket operation failed; never treated as an operation result.
    Io(std::io::Error),
    /// The absolute setup, input or request deadline expired.
    DeadlineExpired,
    /// The peer closed before a complete record was received.
    PeerClosed,
    /// A non-empty output made no progress.
    WriteZero,
    /// The explicit typed/MAC transport profile was not offered.
    ProfileMismatch,
    /// Both endpoints of this local-provider transport must be loopback.
    NonLoopback,
    /// Bounded allocation could not be reserved.
    Allocation,
    /// Cooperative cancellation interrupted nonterminal output.
    Cancelled,
    /// Output returned, but the session's final validation failed.
    AfterOutput(ProtocolError),
    /// The connection has already failed or been closed.
    Closed,
}

impl CanonicalTcpError {
    /// Stable diagnostic reason, excluding untrusted frame contents.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Protocol(error) => error.code(),
            Self::AfterOutput(_) => "PROVIDER_TCP_AFTER_OUTPUT_INVALID",
            Self::Io(_) => "PROVIDER_TCP_IO_ERROR",
            Self::DeadlineExpired => "PROVIDER_TCP_DEADLINE_EXPIRED",
            Self::PeerClosed => "PROVIDER_TCP_FRAME_TRUNCATED",
            Self::WriteZero => "PROVIDER_TCP_WRITE_ZERO",
            Self::ProfileMismatch => "PROVIDER_TCP_PROFILE_MISMATCH",
            Self::NonLoopback => "PROVIDER_TCP_NOT_LOOPBACK",
            Self::Allocation => "PROVIDER_TCP_RESOURCE_EXHAUSTED",
            Self::Cancelled => "PROVIDER_TCP_CANCELLED",
            Self::Closed => "PROVIDER_TCP_CLOSED",
        }
    }
}

impl std::fmt::Display for CanonicalTcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.code()) }
}

impl std::error::Error for CanonicalTcpError {}

impl From<ProviderDeliveryError<CanonicalTcpError>> for CanonicalTcpError {
    fn from(error: ProviderDeliveryError<CanonicalTcpError>) -> Self {
        match error {
            ProviderDeliveryError::Protocol(error) => Self::Protocol(error),
            ProviderDeliveryError::Output(error) => error,
            ProviderDeliveryError::AfterOutput(error) => Self::AfterOutput(error),
        }
    }
}

struct TcpState {
    connection: CanonicalProviderConnection,
    io: SocketIo,
}

impl Drop for TcpState {
    fn drop(&mut self) { let _ = self.connection.disconnect(); }
}

/// Sole owner of one negotiated typed socket, pairing key and BoundSession.
///
/// A failed operation or unwind drops all three together. No stream clone,
/// detached reader, second request registry, reconnect or replay is introduced.
/// `receive` is synchronous: the caller schedules reads while its request
/// workers run. This does not itself launch workers or install a listener.
/// Source/grant/disclosure authorization stays with the serving composition.
pub struct CanonicalTcpConnection {
    state: Option<TcpState>,
}

impl CanonicalTcpConnection {
    /// Reads exactly one bounded authenticated record, then dispatches it.
    ///
    /// Some is an admitted recipe, still requiring live authorization before
    /// execution. None is an authenticated cancel whose acknowledgement was
    /// actually written; it does not imply that the target finished or rolled
    /// back. No ordinary acknowledgement from the legacy shim is appended.
    ///
    /// Length is rejected before body allocation. The server's finite budget
    /// starts BEFORE the first prefix read; the client's smaller relative budget
    /// is measured from that same origin, including all decoding/proof work.
    /// Refusal, truncated record or any output failure closes the entire connection.
    /// This blocking entry uses the same retained reader as poll_receive.
    pub fn receive(
        &mut self,
        maximum_deadline_ms: u64,
    ) -> Result<Option<AdmittedProviderRequest>, CanonicalTcpError> {
        loop {
            if let Poll::Ready(result) = self.poll_receive(maximum_deadline_ms, io::POLL)? {
                return Ok(result);
            }
        }
    }

    /// Polls incoming work while allowing the owner to service existing workers.
    ///
    /// Pending keeps one partial record and its original pre-prefix deadline;
    /// larger budgets on later polls cannot extend it. Between calls the owner
    /// can emit completed worker results or close. Ready(None) means an actual
    /// authenticated cancel acknowledgement, not an empty socket or stopped work.
    /// Input waiting is capped at min(quantum, 25 ms) and 64 KiB per turn. Complete
    /// decoding/authentication and cancel-ack output retain their original budgets,
    /// not the polling quantum. Zero quantum is invalid. No async Waker is used.
    pub fn poll_receive(
        &mut self,
        maximum_deadline_ms: u64,
        quantum: Duration,
    ) -> Result<Poll<Option<AdmittedProviderRequest>>, CanonicalTcpError> {
        self.with_state(|state| {
            let limits = state.connection.limits;
            let Poll::Ready(io::ReceivedRecord { frame, proof: observed, started, maximum_deadline_ms }) =
                state.io.poll_record(limits, maximum_deadline_ms, quantum)? else {
                    return Ok(Poll::Pending);
                };
            let (connection, io) = (&mut state.connection, &mut state.io);
            let session = &mut connection.session;
            let transcript = ProviderFrameTranscript::request(
                session.pairing().session(), *session.server_nonce(), &frame,
            );
            let expected = keyed_frame_proof(&connection.key, &transcript);
            if !verify_proof(&expected, &observed) {
                return Err(CanonicalTcpError::Protocol(ProtocolError::AuthenticationFailed));
            }
            let version = session.binding_context().version();
            let versions = ProtocolRange { minimum: version, maximum: version };
            // Classify only through the existing closed codec, not an event-name
            // scan. Its temporary body is dropped here. Admission deliberately
            // revalidates with the existing API; the MAC is computed only once
            // over the same immutable frame under this exclusive session borrow.
            let kind = ClientEnvelopeCodec::decode(&frame, limits, versions)
                .map_err(CanonicalTcpError::Protocol)?.message_kind;
            let mut first_clock = Some(started);
            let mut clock = || Ok(first_clock.take().unwrap_or_else(monotonic_millis));
            match kind {
                MessageKind::Request => {
                    let request = session.admit_provider_request(
                        &frame, &observed, maximum_deadline_ms, &mut clock, |_| Ok(expected),
                    ).map_err(CanonicalTcpError::Protocol)?;
                    session.revalidate_request_guard(request.guard(), monotonic_millis())
                        .map_err(CanonicalTcpError::Protocol)?;
                    Ok(Poll::Ready(Some(request)))
                }
                MessageKind::Cancel => {
                    let key = &connection.key;
                    session.cancel_provider_request(
                        &frame, &observed, maximum_deadline_ms, &mut clock, |_| Ok(expected),
                        |transcript, cancel_deadline| {
                            let proof = keyed_frame_proof(key, transcript);
                            io.write_record(transcript.frame(), &proof, limits, cancel_deadline, None)
                        },
                    ).map_err(CanonicalTcpError::from)?;
                    Ok(Poll::Ready(None))
                }
                _ => Err(CanonicalTcpError::Protocol(ProtocolError::InvalidEnvelope)),
            }
        })
    }

    /// Emits an actual typed record and proof before committing the event.
    ///
    /// The serving owner MUST hold its live authorization/disclosure barrier
    /// across this call; transport admission is not a grant. The original guard
    /// deadline is enforced around each partial write/flush. Nonterminal output
    /// observes cancellation; terminal selection is frozen by BoundSession and
    /// cannot be rewritten after its first byte. Successful local I/O is not a
    /// remote acknowledgement or evidence of rollback.
    pub fn deliver_event(
        &mut self,
        request: &mut AdmittedProviderRequest,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
    ) -> Result<(), CanonicalTcpError> {
        self.deliver_event_before(request, body, terminal, None)
    }

    // The serving owner may only tighten the original deadline. This same
    // writer checks the cap before/after every frame/MAC write and flush.
    fn deliver_event_before(
        &mut self,
        request: &mut AdmittedProviderRequest,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
        authority_deadline: Option<search_provider_protocol::MonotonicMillis>,
    ) -> Result<(), CanonicalTcpError> {
        self.with_state(|state| {
            let deadline = request.guard().deadline()
                .ok_or(CanonicalTcpError::Protocol(ProtocolError::InvalidLimits))?;
            let deadline = authority_deadline.map_or(deadline, |authority| authority.min(deadline));
            let cancellation = request.guard().cancellation();
            let probe = terminal.is_none().then_some(&cancellation);
            let limits = state.connection.limits;
            let (connection, io) = (&mut state.connection, &mut state.io);
            connection.deliver_event(request, body, terminal, |frame, proof, _| {
                io.write_record(frame, proof, limits, deadline, probe)
            }).map_err(CanonicalTcpError::from)
        })
    }

    /// Read-only state for live authorization composition; None after teardown.
    #[must_use]
    pub fn session(&self) -> Option<&BoundSession> {
        self.state.as_ref().map(|state| state.connection.session())
    }

    /// Cancels local guards, closes the socket and drops the pairing key.
    pub fn close(&mut self) { drop(self.state.take()); }

    fn with_state<R>(
        &mut self,
        action: impl FnOnce(&mut TcpState) -> Result<R, CanonicalTcpError>,
    ) -> Result<R, CanonicalTcpError> {
        let mut operation = Operation { state: &mut self.state, completed: false };
        let state = operation.state.as_mut().ok_or(CanonicalTcpError::Closed)?;
        let result = action(state);
        operation.completed = result.is_ok();
        result
    }
}

impl Drop for CanonicalTcpConnection {
    fn drop(&mut self) { self.close(); }
}

struct Operation<'a> {
    state: &'a mut Option<TcpState>,
    completed: bool,
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if !self.completed { drop(self.state.take()); }
    }
}
